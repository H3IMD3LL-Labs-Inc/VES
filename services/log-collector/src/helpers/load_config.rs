use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

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
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&config_str)?;
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
