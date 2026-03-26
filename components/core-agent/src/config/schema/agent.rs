// ===================================================================================
// This is global Core Agent configuration. This configuration affects the
// runtime environment of a Core Agent itself. The settings allow the Core Agent
// to affect how its runtime behaves, before sources and processors are affected
//
// Think of this as effects on how the Core Agent behaves as a system; work
// scheduling, data handling, communication with HEIMDELL Server, failures/retries,
// resource management, etc.
//
// This configuration is usually only loaded once on startup, and reloads are
// usually entire restarts of the whole Core Agent
// ===================================================================================

use std::time::Duration;

#[derive(Clone, PartialEq)]
pub enum EnvironmentProvider {
    Static,
    Host,
    AwsEc2 {
        use_instance_id_in_node_id: bool,
        use_instance_type: bool,
        use_ami_id: bool,
        use_availability_zone: bool,
        use_region: bool,
        use_hostname: bool,
    },
    Docker,
    Custom,
}

#[derive(Clone, PartialEq)]
pub enum HEIMDELLServer {
    // [TODO]: Information about the HEIMDELL Server tied to the Core Agent
}

#[derive(Clone, PartialEq)]
pub enum NetworkProtocol {
    Grpc,
    Http,
}

#[derive(Clone, PartialEq)]
pub enum LogLevel {
    Trace,
    Error,
    Warn,
    Debug,
    Info,
}

#[derive(Clone, PartialEq)]
pub enum Compression {
    Gzip,
    Zstd,
}

#[derive(Default, Clone, PartialEq)]
pub struct CoreAgentConfig {
    pub identity: IdentityConfig,
    pub data_batching: BatchingConfig,
    pub retries: RetryConfig,
    pub network: Option<NetworkConfig>,
    pub runtime: RuntimeConfig,
    pub data_buffering: BufferConfig,
    pub telemetry: Option<TelemetryConfig>,
}

#[derive(Default, Clone, PartialEq)]
pub struct IdentityConfig {
    pub heimdell_server: Option<HEIMDELLServer>,
    pub node_id: String,
    pub hostname: Option<String>,
    pub environment: Option<String>,
    pub environment_provider: Option<EnvironmentProvider>,
}

#[derive(Default, Clone, PartialEq)]
pub struct BatchingConfig {
    pub max_events: usize,
    pub max_batch_bytes: usize,
    pub flush_interval: Duration,
}

#[derive(Default, Clone, PartialEq)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

#[derive(Clone, PartialEq)]
pub struct NetworkConfig {
    pub heimdell_server_endpoint: String,
    pub protocol: NetworkProtocol,
    pub request_timeout: Duration,
    pub compression: Compression,
}

#[derive(Default, Clone, PartialEq)]
pub struct RuntimeConfig {
    pub processor_worker_threads: usize,
    pub max_parallel_sources: usize,
}

#[derive(Default, Clone, PartialEq)]
pub struct BufferConfig {
    pub max_memory_bytes: usize,
    pub backpressure_threshold: usize,
}

#[derive(Clone, PartialEq)]
pub struct TelemetryConfig {
    pub metrics_enabled: bool,
    pub metrics_port: Option<u16>,
    pub log_level: LogLevel,
}
