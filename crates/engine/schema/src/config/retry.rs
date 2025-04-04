use std::time::Duration;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    /// How many retries are available per second, at a minimum.
    pub min_per_second: Option<u32>,
    /// Each successful request to the subgraph adds to the retry budget. This setting controls for how long the budget remembers successful requests.
    pub ttl: Option<Duration>,
    /// The fraction of the successful requests budget that can be used for retries.
    pub retry_percent: Option<f32>,
    /// Whether mutations should be retried at all. False by default.
    pub retry_mutations: bool,
}

impl From<gateway_config::RetryConfig> for RetryConfig {
    fn from(config: gateway_config::RetryConfig) -> Self {
        RetryConfig {
            min_per_second: config.min_per_second,
            ttl: config.ttl,
            retry_percent: config.retry_percent,
            retry_mutations: config.retry_mutations,
        }
    }
}
