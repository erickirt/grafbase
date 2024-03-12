use std::{borrow::Cow, sync::Arc, time::Duration};

use super::gateway::{GatewayConfig, GatewaySender};
use ascii::AsciiString;
use http::{HeaderValue, StatusCode};
use tokio::time::MissedTickBehavior;
use tracing::Level;
use ulid::Ulid;
use url::Url;

/// How often we poll updates from the schema registry.
const TICK_INTERVAL: Duration = Duration::from_secs(10);

/// How long we wait for a response from the schema registry.
const UPLINK_TIMEOUT: Duration = Duration::from_secs(10);

/// How long we wait until a connection is successfully opened.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Sets an interval for HTTP2 Ping frames should be sent to keep a connection alive.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(1);

/// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(5);

/// Sets whether HTTP2 keep-alive should apply while the connection is idle.
const KEEPALIVE_WHILE_IDLE: bool = true;

/// The HTTP user-agent header we sent to the schema registry.
const USER_AGENT: &str = "grafbase-cli";

/// The CDN host we load the graphs from.
const UPLINK_HOST: &str = "https://gdn.grafbase.com";

/// An updater thread for polling graph changes from the API.
pub(super) struct GraphUpdater {
    graph_ref: String,
    uplink_url: Url,
    uplink_client: reqwest::Client,
    access_token: AsciiString,
    sender: GatewaySender,
    current_id: Option<Ulid>,
    gateway_config: GatewayConfig,
}

/// TODO: here you get the needed values for tracing Hugo!
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct UplinkResponse {
    account_id: Ulid,
    graph_id: Ulid,
    branch: String,
    sdl: String,
    version_id: Ulid,
}

impl GraphUpdater {
    pub fn new(
        graph_ref: &str,
        branch: Option<&str>,
        access_token: AsciiString,
        sender: GatewaySender,
        gateway_config: GatewayConfig,
    ) -> crate::Result<Self> {
        let uplink_client = reqwest::ClientBuilder::new()
            .gzip(true)
            .timeout(UPLINK_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .http2_keep_alive_interval(Some(KEEPALIVE_INTERVAL))
            .http2_keep_alive_timeout(KEEPALIVE_TIMEOUT)
            .http2_keep_alive_while_idle(KEEPALIVE_WHILE_IDLE)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| crate::Error::InternalError(e.to_string()))?;

        let uplink_host = match std::env::var("GRAFBASE_GDN_URL") {
            Ok(host) => Cow::Owned(host),
            Err(_) => Cow::Borrowed(UPLINK_HOST),
        };

        let uplink_url = match branch {
            Some(branch) => format!("{uplink_host}/graphs/{graph_ref}/{branch}/current"),
            None => format!("{uplink_host}/graphs/{graph_ref}/current"),
        };

        let uplink_url = uplink_url
            .parse::<Url>()
            .map_err(|e| crate::Error::InternalError(e.to_string()))?;

        Ok(Self {
            graph_ref: graph_ref.to_string(),
            uplink_url,
            uplink_client,
            access_token,
            sender,
            current_id: None,
            gateway_config,
        })
    }

    /// A poll loop for fetching the latest graph from the API. When started,
    /// fetches the graph immediately and after that every ten seconds. If we detect
    /// changes to the running graph, we create a new gateway and replace the running
    /// one with it.
    ///
    /// By having the gateway in a reference counter, we make sure the current requests
    /// are served before dropping.
    pub async fn poll(&mut self) {
        let mut interval = tokio::time::interval(TICK_INTERVAL);

        // if we have a slow connection, this prevents bursts of connections to the GDN
        // for all the missed ticks.
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let mut request = self
                .uplink_client
                .get(self.uplink_url.as_str())
                .bearer_auth(&self.access_token);

            if let Some(id) = self.current_id {
                request = request.header(
                    "If-None-Match",
                    HeaderValue::from_bytes(id.to_string().as_bytes()).expect("must be ascii"),
                );
            }

            let response = request.send().await;

            let response = match response {
                Ok(response) => response,
                Err(e) => {
                    tracing::event!(Level::ERROR, message = "error updating graph", error = e.to_string());
                    continue;
                }
            };

            if response.status() == StatusCode::NOT_MODIFIED {
                tracing::debug!("no updates to the graph");
                continue;
            }

            if let Err(e) = response.error_for_status_ref() {
                match e.status() {
                    Some(StatusCode::NOT_FOUND) => {
                        tracing::warn!("no subgraphs registered, publish at least one subgraph");
                    }
                    _ => {
                        tracing::event!(Level::ERROR, message = "error updating graph", error = e.to_string());
                    }
                }
                continue;
            }

            let response: UplinkResponse = match response.json().await {
                Ok(response) => response,
                Err(e) => {
                    tracing::event!(Level::ERROR, message = "error updating graph", error = e.to_string());
                    continue;
                }
            };

            tracing::event!(
                Level::INFO,
                message = "creating a new gateway",
                graph_ref = self.graph_ref,
                branch = response.branch,
                operation_limits = self.gateway_config.operation_limits.is_some(),
                introspection_enabled = self.gateway_config.enable_introspection,
                authentication = self.gateway_config.authentication.is_some(),
            );

            let gateway = match super::gateway::generate(&response.sdl, self.gateway_config.clone()) {
                Ok(gateway) => gateway,
                Err(e) => {
                    tracing::event!(Level::ERROR, message = "error parsing graph", error = e.to_string());
                    continue;
                }
            };

            self.current_id = Some(response.version_id);

            self.sender
                .send(Some(Arc::new(gateway)))
                .expect("internal error: channel closed");
        }
    }
}
