pub mod logs;
pub mod metrics;
pub mod otlp;
pub mod response_extension;
pub mod stdout;
pub mod tracing;

pub use logs::LogsConfig;
pub use metrics::MetricsConfig;
pub use otlp::*;
pub use response_extension::*;
pub use tracing::{DEFAULT_SAMPLING, PropagationConfig, TracingCollectConfig, TracingConfig};

use serde::{Deserialize, Deserializer};
pub use stdout::StdoutExporterConfig;

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalExporterConfig {
    pub stdout: Option<StdoutExporterConfig>,
    pub otlp: Option<OtlpExporterConfig>,
    pub response_extension: Option<ResponseExtensionExporterConfig>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenTelemetryExportersConfig {
    pub stdout: Option<StdoutExporterConfig>,
    pub otlp: Option<OtlpExporterConfig>,
}

/// Configuration for batched exports
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BatchExportConfig {
    /// The delay, in seconds, between two consecutive processing of batches.
    /// The default value is 5 seconds.
    #[serde(deserialize_with = "deserialize_duration")]
    pub scheduled_delay: chrono::Duration,

    /// The maximum queue size to buffer spans for delayed processing. If the
    /// queue gets full it drops the spans.
    /// The default value of is 2048.
    pub max_queue_size: usize,

    /// The maximum number of spans to process in a single batch. If there are
    /// more than one batch worth of spans then it processes multiple batches
    /// of spans one batch after the other without any delay.
    /// The default value is 512.
    pub max_export_batch_size: usize,

    /// Maximum number of concurrent exports
    ///
    /// Limits the number of spawned tasks for exports and thus resources consumed
    /// by an exporter. A value of 1 will cause exports to be performed
    /// synchronously on the [`BatchSpanProcessor`] task.
    /// The default is 1.
    pub max_concurrent_exports: usize,
}

impl BatchExportConfig {
    pub(crate) fn default_scheduled_delay() -> chrono::Duration {
        chrono::Duration::try_seconds(5).expect("must be fine")
    }

    pub(crate) fn default_max_queue_size() -> usize {
        2048
    }

    pub(crate) fn default_max_export_batch_size() -> usize {
        512
    }

    pub(crate) fn default_max_concurrent_exports() -> usize {
        1
    }
}

impl Default for BatchExportConfig {
    fn default() -> Self {
        Self {
            scheduled_delay: BatchExportConfig::default_scheduled_delay(),
            max_queue_size: BatchExportConfig::default_max_queue_size(),
            max_export_batch_size: BatchExportConfig::default_max_export_batch_size(),
            max_concurrent_exports: BatchExportConfig::default_max_concurrent_exports(),
        }
    }
}

pub(crate) fn default_export_timeout() -> chrono::Duration {
    chrono::Duration::try_seconds(60).expect("must be fine")
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<chrono::Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let input = i64::deserialize(deserializer)?;

    Ok(chrono::Duration::try_seconds(input).expect("must be fine"))
}

fn deserialize_duration_opt<'de, D>(deserializer: D) -> Result<Option<chrono::Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let input = Option::<i64>::deserialize(deserializer)?;

    Ok(input.map(|input| chrono::Duration::try_seconds(input).expect("must be fine")))
}
