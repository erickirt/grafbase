use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use clap::Parser;
use federated_server::GraphLoader;
use gateway_config::Config;
use graph_ref::GraphRef;

use super::{LogLevel, log::LogStyle};

#[derive(Debug, Parser)]
#[command(name = "Grafbase Lambda Gateway", version)]
/// Grafbase Lambda Gateway
pub struct Args {
    /// Path to the TOML configuration file
    #[arg(env = "GRAFBASE_CONFIG_PATH", default_value = "./grafbase.toml")]
    pub config: PathBuf,
    /// Path to the schema SDL. If provided, the graph will be static and no connection is made
    /// to the Grafbase API.
    #[arg(env = "GRAFBASE_SCHEMA_PATH", default_value = "./federated.graphql")]
    pub schema: PathBuf,
    #[arg(short, long, help = GraphRef::ARG_DESCRIPTION, env = "GRAFBASE_GRAPH_REF")]
    pub graph_ref: Option<GraphRef>,
    /// Set the logging level, this applies to all spans, logs and trace events.
    ///
    /// Beware that *only* 'off', 'error', 'warn' and 'info' can be used safely in production. More
    /// verbose levels, such as 'debug', will include sensitive information like request variables, responses, etc.
    ///
    /// Possible values are: 'off', 'error', 'warn', 'info', 'debug', 'trace' or a custom string.
    /// In the last case, the string is passed on to [`tracing_subscriber::EnvFilter`] as is and is
    /// only meant for debugging purposes. No stability guarantee is made on the format.
    #[arg(long = "log", env = "GRAFBASE_LOG", default_value = "info")]
    pub log_level: String,
    /// Set the style of log output
    #[arg(env = "GRAFBASE_LOG_STYLE", default_value_t = LogStyle::Text)]
    log_style: LogStyle,
}

impl super::Args for Args {
    /// The method of fetching a graph
    fn fetch_method(&self) -> anyhow::Result<GraphLoader> {
        Ok(GraphLoader::FromSchemaFile {
            path: self.schema.clone(),
        })
    }

    /// The gateway configuration
    fn config(&self) -> anyhow::Result<Config> {
        let mut config = match fs::read_to_string(&self.config) {
            Ok(config) => toml::from_str(&config)?,
            Err(e) => match e.kind() {
                ErrorKind::NotFound => Config::default(),
                _ => return Err(anyhow::anyhow!("error loading config file: {e}")),
            },
        };

        if let Some(otel_config) = self.grafbase_otel_config()? {
            config.telemetry.grafbase = Some(otel_config);
        }

        Ok(config)
    }

    fn config_path(&self) -> Option<&Path> {
        Some(&self.config)
    }

    fn log_style(&self) -> LogStyle {
        self.log_style
    }

    fn hot_reload(&self) -> bool {
        false
    }

    fn listen_address(&self) -> Option<std::net::SocketAddr> {
        None
    }

    fn log_level(&self) -> LogLevel<'_> {
        LogLevel(&self.log_level)
    }

    fn graph_ref(&self) -> Option<&GraphRef> {
        self.graph_ref.as_ref()
    }
}
