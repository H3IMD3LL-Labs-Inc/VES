// ============================================================================
// This configuration schema defines the configuration structures for source
// drivers, determining what a Core Agent collects and how it collects it.
// Each source driver has multiple configuration entries, where each entry
// corresponds to one driver instance. i.e, a filesystem driver might be
// configured to watch multiple directories, here each configuration entry
// becomes one driver instance. All configured source drivers are grouped
// and aggregated together
//
// Remember, this schema provides the data that the runtime later uses to spawn
// source driver instances
// ============================================================================

enum SocketKind {
    Tcp,
    Udp,
    Unix,
}

enum BindAddress {
    Inet {
        host: String,
        port: u16,
    },
    Unix {
        path: String,
    }
}

pub struct SourcesConfig {
    pub filesystem: Vec<FilesystemSourceConfig>,
    pub journald: Vec<JournaldSourceConfig>,
    pub socket: Vec<SocketSourceConfig>,
}

// Read data from files on the node
pub struct FilesystemSourceConfig {
    pub id: String,
    pub paths: Vec<String>,
    pub recursive: bool,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub file_types: Vec<String>,
}

// Read data from systemd logs on the node
pub struct JournaldSourceConfig {
    pub id: String,
    pub units: Vec<String>,
    pub since: Option<String>,
}

pub struct SocketSourceConfig {
    pub id: String,
    pub kind: SocketKind,
    pub bind_addr: BindAddress,
    pub tls: Option<TlsConfig>,
    pub tcp_options: Option<TcpOptions>,
}

pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
    pub require_client_auth: bool,
}

pub struct TcpOptions {
    pub nodelay: bool,
    pub keep_alive_secs: Option<u64>,
}
