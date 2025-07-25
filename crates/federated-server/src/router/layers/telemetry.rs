use std::{
    fmt::Display,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use ::tower::Layer;
use engine::TelemetryExtension;
use grafbase_telemetry::{
    grafbase_client::Client,
    metrics::{RequestMetrics, RequestMetricsAttributes},
    otel::{
        opentelemetry::{self, propagation::Extractor},
        tracing_opentelemetry::OpenTelemetrySpanExt,
    },
    span::http_request::{HttpRequestSpan, HttpRequestSpanBuilder},
};
use http::{Request, Response};
use http_body::Body;
use tracing::Instrument;

#[derive(Clone)]
pub struct TelemetryLayer(Arc<TelemetryLayerInner>);

pub struct TelemetryLayerInner {
    metrics: RequestMetrics,
    listen_address: Option<SocketAddr>,
    route: Option<String>,
}

impl TelemetryLayer {
    pub fn new_from_global_meter_provider(listen_address: Option<SocketAddr>) -> Self {
        Self(Arc::new(TelemetryLayerInner {
            metrics: RequestMetrics::build(&grafbase_telemetry::metrics::meter_from_global_provider()),
            listen_address,
            route: None,
        }))
    }

    pub fn with_route(self, route: impl Into<String>) -> Self {
        Self(Arc::new(TelemetryLayerInner {
            metrics: self.0.metrics.clone(),
            listen_address: self.0.listen_address,
            route: Some(route.into()),
        }))
    }
}

impl<Service> Layer<Service> for TelemetryLayer
where
    Service: Send + Clone,
{
    type Service = TelemetryService<Service>;

    fn layer(&self, inner: Service) -> Self::Service {
        TelemetryService {
            inner,
            layer: self.0.clone(),
        }
    }
}

/// tower-http provides a TraceService as a convenient way to wrap the whole execution. However
/// it's only meant for tracing and doesn't provide a good way for metrics to access both the
/// request and the response. As such we end up needing to write a [tower::Service] ourselves.
/// [TelemetryService] is mostly inspired by how the [tower_http::trace::Trace] works.
#[derive(Clone)]
pub struct TelemetryService<Service>
where
    Service: Send + Clone,
{
    inner: Service,
    layer: Arc<TelemetryLayerInner>,
}

impl<Service> TelemetryService<Service>
where
    Service: Send + Clone,
{
    fn make_span<B: Body>(&self, request: &Request<B>) -> HttpRequestSpan {
        let parent_ctx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(request.headers()))
        });

        let span = HttpRequestSpanBuilder::from_http(request).build();
        span.set_parent(parent_ctx);

        span
    }
}

// From opentelemetry-http which still uses http 0.X as of 2024/05/17
struct HeaderExtractor<'a>(pub &'a http::HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    /// Get a value for a key from the HeaderMap.  If the value is not valid ASCII, returns None.
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    /// Collect all the keys from the HeaderMap.
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|value| value.as_str()).collect::<Vec<_>>()
    }
}

/// See [TelemetryService]
impl<Service, ReqBody, ResBody> tower::Service<Request<ReqBody>> for TelemetryService<Service>
where
    Service: tower::Service<Request<ReqBody>, Response = Response<ResBody>> + Send + Clone + 'static,
    Service::Future: Send,
    Service::Error: Display + 'static,
    ReqBody: Body + Send + 'static,
    ResBody: Body + Send + 'static,
{
    type Response = http::Response<ResBody>;
    type Error = Service::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response<ResBody>, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        let layer = self.layer.clone();
        let http_span = self.make_span(&req);

        layer.metrics.increment_connected_clients();

        let span = http_span.span.clone();
        let fut = async move {
            let start = Instant::now();

            let client = Client::extract_from(req.headers());
            let version = req.version();

            let method = req.method().clone();
            let url_scheme = req.uri().scheme_str().map(ToString::to_string);
            let route = layer.route.clone().or_else(|| Some(req.uri().path().to_string()));

            let mut result = inner.call(req).await;

            match result {
                Err(ref err) => {
                    layer.metrics.record_http_duration(
                        RequestMetricsAttributes {
                            status_code: 500,
                            client,
                            cache_status: None,
                            url_scheme,
                            route,
                            listen_address: layer.listen_address,
                            version: Some(version),
                            method: Some(method.clone()),
                            has_graphql_errors: false,
                        },
                        start.elapsed(),
                    );

                    http_span.record_internal_server_error();
                    tracing::error!("Internal server error: {err}");
                }
                Ok(ref mut response) => {
                    if let Some(size) = response.body().size_hint().exact() {
                        layer.metrics.record_response_body_size(size);
                    }
                    http_span.record_response(response);
                    let cache_status = response
                        .headers()
                        .get("x-grafbase-cache")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);

                    let mut attributes = RequestMetricsAttributes {
                        status_code: response.status().as_u16(),
                        client,
                        cache_status,
                        url_scheme,
                        route,
                        listen_address: layer.listen_address,
                        version: Some(version),
                        method: Some(method.clone()),
                        has_graphql_errors: false,
                    };

                    let telemetry = response
                        .extensions_mut()
                        .remove::<TelemetryExtension>()
                        .unwrap_or_default();

                    match telemetry {
                        TelemetryExtension::Ready(telemetry) => {
                            http_span.record_graphql_execution_telemetry(&telemetry);
                            // FIXME: Use a different metric for subscriptions, the latency doesn't
                            // have the same meaning.
                            if !telemetry.operations.iter().any(|(ty, _)| ty.is_subscription()) {
                                attributes.has_graphql_errors = telemetry.errors_count() > 0;
                                layer.metrics.record_http_duration(attributes, start.elapsed());
                            }
                        }
                        TelemetryExtension::Future(channel) => {
                            let span = http_span.span.clone();
                            let layer = layer.clone();
                            tokio::spawn(
                                async move {
                                    let telemetry = channel.await.unwrap_or_default();
                                    http_span.record_graphql_execution_telemetry(&telemetry);
                                    if !telemetry.operations.iter().any(|(ty, _)| ty.is_subscription()) {
                                        attributes.has_graphql_errors = telemetry.errors_count() > 0;
                                        layer.metrics.record_http_duration(attributes, start.elapsed());
                                    }
                                }
                                // Ensures the span will have the proper end time.
                                .instrument(span),
                            );
                        }
                    }
                }
            }

            layer.metrics.decrement_connected_clients();

            result
        };

        Box::pin(fut.instrument(span))
    }
}
