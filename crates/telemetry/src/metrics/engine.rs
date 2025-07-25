use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram, Meter, UpDownCounter},
};

use crate::{
    grafbase_client::Client,
    graphql::{GraphqlOperationAttributes, GraphqlResponseStatus, OperationName, SubgraphResponseStatus},
};

#[derive(Clone)]
pub struct EngineMetrics {
    graph_version: Option<String>,
    operation_latency: Histogram<u64>,
    subgraph_latency: Histogram<u64>,
    subgraph_retries: Counter<u64>,
    subgraph_request_body_size: Histogram<u64>,
    subgraph_response_body_size: Histogram<u64>,
    subgraph_requests_inflight: UpDownCounter<i64>,
    subgraph_cache_hits: Counter<u64>,
    subgraph_cache_partial_hits: Counter<u64>,
    subgraph_cache_misses: Counter<u64>,
    operation_cache_hits: Counter<u64>,
    operation_cache_misses: Counter<u64>,
    query_preparation_latency: Histogram<u64>,
    batch_sizes: Histogram<u64>,
    request_body_sizes: Histogram<u64>,
    graphql_errors: Counter<u64>,
}

#[derive(Debug)]
pub struct GraphqlRequestMetricsAttributes {
    pub operation: GraphqlOperationAttributes,
    pub status: GraphqlResponseStatus,
    pub client: Option<Client>,
}

#[derive(Debug)]
pub struct SubgraphRequestDurationAttributes {
    pub name: String,
    pub status: SubgraphResponseStatus,
    pub http_status_code: Option<http::StatusCode>,
}

#[derive(Debug)]
pub struct SubgraphRequestRetryAttributes {
    pub name: String,
    pub aborted: bool,
}

#[derive(Debug)]
pub struct SubgraphRequestBodySizeAttributes {
    pub name: String,
}

#[derive(Debug)]
pub struct SubgraphResponseBodySizeAttributes {
    pub name: String,
}

#[derive(Debug)]
pub struct SubgraphInFlightRequestAttributes {
    pub name: String,
}

#[derive(Debug)]
pub struct SubgraphCacheHitAttributes {
    pub name: String,
}

#[derive(Debug)]
pub struct SubgraphCacheMissAttributes {
    pub name: String,
}

#[derive(Debug)]
pub struct QueryPreparationAttributes {
    pub operation: Option<GraphqlOperationAttributes>,
    pub success: bool,
}

#[derive(Debug)]
pub struct GraphqlErrorAttributes {
    pub code: &'static str,
    pub operation_name: Option<String>,
    pub client: Option<Client>,
}

impl EngineMetrics {
    pub fn build(meter: &Meter, graph_version: Option<String>) -> Self {
        Self {
            graph_version,
            operation_latency: meter
                .u64_histogram("graphql.operation.duration")
                .with_unit("ms")
                .build(),
            subgraph_latency: meter
                .u64_histogram("graphql.subgraph.request.duration")
                .with_unit("ms")
                .build(),
            subgraph_retries: meter.u64_counter("graphql.subgraph.request.retries").build(),
            subgraph_request_body_size: meter.u64_histogram("graphql.subgraph.request.body.size").build(),
            subgraph_response_body_size: meter.u64_histogram("graphql.subgraph.response.body.size").build(),
            subgraph_requests_inflight: meter.i64_up_down_counter("graphql.subgraph.request.inflight").build(),
            subgraph_cache_hits: meter.u64_counter("graphql.subgraph.request.cache.hit").build(),
            subgraph_cache_partial_hits: meter.u64_counter("graphql.subgraph.request.cache.partial_hit").build(),
            subgraph_cache_misses: meter.u64_counter("graphql.subgraph.request.cache.miss").build(),
            operation_cache_hits: meter.u64_counter("graphql.operation.cache.hit").build(),
            operation_cache_misses: meter.u64_counter("graphql.operation.cache.miss").build(),
            query_preparation_latency: meter
                .u64_histogram("graphql.operation.prepare.duration")
                .with_unit("ms")
                .build(),
            batch_sizes: meter.u64_histogram("graphql.operation.batch.size").build(),
            request_body_sizes: meter.u64_histogram("http.server.request.body.size").build(),
            graphql_errors: meter.u64_counter("graphql.operation.errors").build(),
        }
    }

    fn create_operation_key_values(&self, operation: GraphqlOperationAttributes) -> Vec<KeyValue> {
        let mut attributes = vec![
            KeyValue::new("graphql.document", operation.sanitized_query.clone()),
            KeyValue::new("graphql.operation.type", operation.ty.as_str()),
        ];

        match operation.name {
            OperationName::Original(name) => {
                attributes.push(KeyValue::new("graphql.operation.name", name));
            }
            OperationName::Computed(name) => {
                attributes.push(KeyValue::new("grafbase.operation.computed_name", name));
            }
            OperationName::Unknown => (),
        }

        attributes
    }

    pub fn record_query_or_mutation_duration(
        &self,
        GraphqlRequestMetricsAttributes {
            operation,
            status,
            client,
        }: GraphqlRequestMetricsAttributes,
        latency: std::time::Duration,
    ) {
        if operation.ty.is_subscription() {
            return;
        }
        let mut attributes = self.create_operation_key_values(operation);

        if let Some(version) = self.graph_version.clone() {
            attributes.push(KeyValue::new("grafbase.graph.version", version))
        }

        attributes.push(KeyValue::new("graphql.response.status", status.as_str()));

        if let Some(client) = client {
            attributes.push(KeyValue::new("http.headers.x-grafbase-client-name", client.name));

            if let Some(version) = client.version {
                attributes.push(KeyValue::new("http.headers.x-grafbase-client-version", version));
            }
        }

        self.operation_latency.record(latency.as_millis() as u64, &attributes);
    }

    pub fn record_subgraph_request_duration(
        &self,
        SubgraphRequestDurationAttributes {
            name,
            status,
            http_status_code,
        }: SubgraphRequestDurationAttributes,
        duration: std::time::Duration,
    ) {
        let mut attributes = vec![
            KeyValue::new("graphql.subgraph.name", name),
            KeyValue::new("graphql.subgraph.response.status", status.as_str()),
        ];

        if let Some(code) = http_status_code {
            attributes.push(KeyValue::new("http.response.status_code", code.as_u16() as i64));
        }

        self.subgraph_latency.record(duration.as_millis() as u64, &attributes);
    }

    pub fn record_subgraph_retry(
        &self,
        SubgraphRequestRetryAttributes { name, aborted }: SubgraphRequestRetryAttributes,
    ) {
        let attributes = [
            KeyValue::new("graphql.subgraph.name", name),
            KeyValue::new("graphql.subgraph.aborted", aborted),
        ];

        self.subgraph_retries.add(1, &attributes);
    }

    pub fn record_subgraph_request_size(
        &self,
        SubgraphRequestBodySizeAttributes { name }: SubgraphRequestBodySizeAttributes,
        size: usize,
    ) {
        let attributes = [KeyValue::new("graphql.subgraph.name", name)];
        self.subgraph_request_body_size.record(size as u64, &attributes);
    }

    pub fn record_subgraph_response_size(
        &self,
        SubgraphResponseBodySizeAttributes { name }: SubgraphResponseBodySizeAttributes,
        size: usize,
    ) {
        let attributes = [KeyValue::new("graphql.subgraph.name", name)];
        self.subgraph_response_body_size.record(size as u64, &attributes);
    }

    pub fn increment_subgraph_inflight_requests(
        &self,
        SubgraphInFlightRequestAttributes { name }: SubgraphInFlightRequestAttributes,
    ) {
        let attributes = [KeyValue::new("graphql.subgraph.name", name)];
        self.subgraph_requests_inflight.add(1, &attributes);
    }

    pub fn decrement_subgraph_inflight_requests(
        &self,
        SubgraphInFlightRequestAttributes { name }: SubgraphInFlightRequestAttributes,
    ) {
        let attributes = [KeyValue::new("graphql.subgraph.name", name)];
        self.subgraph_requests_inflight.add(-1, &attributes);
    }

    pub fn record_subgraph_cache_hit(&self, SubgraphCacheHitAttributes { name }: SubgraphCacheHitAttributes) {
        let attributes = [KeyValue::new("graphql.subgraph.name", name)];
        self.subgraph_cache_hits.add(1, &attributes);
    }

    pub fn record_subgraph_cache_partial_hit(&self, subgraph_name: String) {
        let attributes = [KeyValue::new("graphql.subgraph.name", subgraph_name)];
        self.subgraph_cache_partial_hits.add(1, &attributes);
    }

    pub fn record_subgraph_cache_miss(&self, SubgraphCacheMissAttributes { name }: SubgraphCacheMissAttributes) {
        let attributes = [KeyValue::new("graphql.subgraph.name", name)];
        self.subgraph_cache_misses.add(1, &attributes);
    }

    pub fn record_operation_cache_hit(&self) {
        self.operation_cache_hits.add(1, &[]);
    }

    pub fn record_operation_cache_miss(&self) {
        self.operation_cache_misses.add(1, &[]);
    }

    pub fn record_failed_preparation_duration(
        &self,
        operation: Option<GraphqlOperationAttributes>,
        duration: std::time::Duration,
    ) {
        let mut attributes = operation
            .map(|op| self.create_operation_key_values(op))
            .unwrap_or_default();

        attributes.push(KeyValue::new("graphql.operation.success", false));

        self.query_preparation_latency
            .record(duration.as_millis() as u64, &attributes);
    }

    pub fn record_successful_preparation_duration(
        &self,
        operation: GraphqlOperationAttributes,
        duration: std::time::Duration,
    ) {
        let mut attributes = self.create_operation_key_values(operation);
        attributes.push(KeyValue::new("graphql.operation.success", true));
        self.query_preparation_latency
            .record(duration.as_millis() as u64, &attributes);
    }

    pub fn record_batch_size(&self, size: usize) {
        self.batch_sizes.record(size as u64, &[]);
    }

    pub fn record_request_body_size(&self, size: usize) {
        self.request_body_sizes.record(size as u64, &[]);
    }

    pub fn increment_graphql_errors(
        &self,
        GraphqlErrorAttributes {
            code,
            operation_name,
            client,
        }: GraphqlErrorAttributes,
    ) {
        let mut attributes = vec![KeyValue::new("graphql.response.error.code", code)];

        if let Some(name) = operation_name {
            attributes.push(KeyValue::new("graphql.operation.name", name));
        }

        if let Some(client) = client {
            attributes.push(KeyValue::new("http.headers.x-grafbase-client-name", client.name));

            if let Some(version) = client.version {
                attributes.push(KeyValue::new("http.headers.x-grafbase-client-version", version));
            }
        }

        self.graphql_errors.add(1, &attributes);
    }
}
