// =============================================================================
// This configuration schema describes how a Core Agent's security settings are
// configurable, TLS, authentication, trust, secret management, etc.
// =============================================================================

pub enum TlsVersion {
    Tls12,
    Tls13,
}

pub enum AuthConfig {
    ApiKey { key: String },
    Token { token: String },
    None,
}

pub enum SecretProvider {
    File { path: String },
    AwsSecretsManager,
    HashiCorpVault,
}

pub struct SecurityConfig {
    pub tls: Option<TlsSecurityConfig>,
    pub authentication: Option<AuthConfig>,
    pub trust: Option<TrustConfig>,
    pub secrets: Option<SecretsConfig>,
}

pub struct TlsSecurityConfig {
    pub cert_path: String,
    pub key_path: String,
    pub verify_server: bool,
    pub min_tls_version: TlsVersion,
}

pub struct TrustConfig {
    pub ca_bundle_path: Option<String>,
    pub use_system_roots: bool,
}

pub struct SecretsConfig {
    pub allow_env: bool,
    pub secrets_provider: Option<SecretProvider>,
}
