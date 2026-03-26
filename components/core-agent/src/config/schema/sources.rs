// ============================================================================
// This configuration schema defines the configuration structures for source
// drivers, determining what a Core Agent collects and how it collects it.
// Each source driver has multiple configuration entries, where each entry
// corresponds to one driver instance. i.e, a filesystem driver might be
// configured to watch multiple directories, here each configuration entry
// becomes one driver instance. All configured source drivers are grouped
// and aggregated together using SourceConfig
//
// Since multiple source driver instances can be instantiated at the same time,
// keep in mind poor management can regress system performance.
// - Too many FileSystem Source Drivers -> OS limits exhaustion
// - Too many sockets -> Port exhaustion
// - Too many threads spawned -> CPU contention/exhaustion
// - Memory Pressure
// - Backpressure issues
// ============================================================================

use crate::config::schema::security::{
    TlsVersion,
    TrustConfig,
};

#[derive(Clone, PartialEq)]
pub enum SocketKind {
    Tcp,
    Udp,
    Unix,
}

#[derive(Clone, PartialEq)]
pub enum BindAddress {
    Inet {
        host: String,
        port: u16,
    },
    Unix {
        path: String,
    }
}

#[derive(Default, Clone, PartialEq)]
pub struct SourcesConfig {
    pub filesystem: Vec<FilesystemSourceConfig>,
    pub journald: Vec<JournaldSourceConfig>,
    pub socket: Vec<SocketSourceConfig>,
}

#[derive(Clone, PartialEq)]
pub struct FilesystemSourceConfig {
    pub enabled: bool,
    pub id: String,
    pub paths: Vec<String>,
    pub recursive: bool,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub file_types: Vec<String>,
}

#[derive(Clone, PartialEq)]
pub struct JournaldSourceConfig {
    pub enabled: bool,
    pub id: String,
    pub units: Vec<String>,
    pub since: Option<String>,
}

#[derive(Clone, PartialEq)]
pub struct SocketSourceConfig {
    pub enabled: bool,
    pub id: String,
    pub kind: SocketKind,
    pub bind_addr: BindAddress,
    pub tls: Option<SocketTlsConfig>,
    pub tcp_options: Option<TcpOptions>,
    pub max_connections: Option<usize>,
}

// ====================================================================
// Per-Socket TLS configuration, separate from global security configs
// ====================================================================
#[derive(Clone, PartialEq)]
pub struct SocketTlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub require_client_auth: bool,
    pub trust: Option<TrustConfig>,
    pub min_tls_version: Option<TlsVersion>,
}

#[derive(Clone, PartialEq)]
pub struct TcpOptions {
    pub nodelay: bool,
    pub keep_alive_secs: Option<u64>,
}
