// External crates
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use tracing::instrument;

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    pub enable_local_mode: bool,
    pub enable_network_mode: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub buffer: BufferConfig,
    pub shipper: ShipperConfig,
    pub parser: ParserConfig,
    pub watcher: Option<WatcherConfig>,
}

impl Config {
    /// Load and parse the configuration file
    #[instrument(
        name = "config_loader",
        target = "helpers::load_config",
        level = "trace",
        skip_all
    )]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();

        tracing::trace!(
            configuration_file_path = %path_ref.display(),
            "Loading VES configuration file"
        );

        let config_str = match fs::read_to_string(path_ref) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Failed to read configuration file");
                return Err(e)
                    .with_context(|| format!("Failed to read config file at {:?}", path_ref))?;
            }
        };
        let config: Config = match toml::from_str(&config_str) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::error!(error = %e, "Failed to parse TOML configuration");
                return Err(e)
                    .with_context(|| format!("Failed to parse TOML from {:?}", path_ref))?;
            }
        };

        tracing::trace!(configuration_file_path = %path_ref.display(), "VES configuration file loaded successfully");
        Ok(config)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BufferConfig {
    pub capacity_option: String,
    pub buffer_capacity: u64,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub durability: DurabilityConfig,
    pub overflow_policy: String,
    pub drain_policy: String,
    pub flush_policy: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", content = "path", rename_all = "kebab-case")]
pub enum DurabilityConfig {
    InMemory,
    SQLite(String),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ShipperConfig {
    pub embedder_target_addr: String,
    pub connection_timeout_ms: u64,
    pub max_reconnect_attempts: Option<u64>,
    pub initial_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
    pub backoff_factor: f64,
    pub retry_jitter: f64,
    pub send_timeout_ms: u64,
    pub response_timeout_ms: u64,
    pub metrics_enabled: bool,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WatcherConfig {
    pub enabled: bool,
    pub log_dir: String,
    pub checkpoint_path: String,
    pub poll_interval_ms: Option<u64>,
    pub recursive: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ParserConfig {
    // TODO: Add parser configuration fields
}
