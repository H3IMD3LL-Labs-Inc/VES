// =============================================================================
// This configuration schema describes how a Core Agent's security settings are
// configurable, TLS, authentication, trust, secret management, etc.
// =============================================================================

#[derive(Clone, PartialEq)]
pub enum TlsVersion {
    Tls12,
    Tls13,
}

#[derive(Clone, PartialEq)]
pub enum AuthConfig {
    ApiKey { key: String },
    Token { token: String },
    None,
}

#[derive(Clone, PartialEq)]
pub enum SecretProvider {
    File { path: String },
    AwsSecretsManager,
    HashiCorpVault,
}

#[derive(Clone, PartialEq)]
pub struct SecurityConfig {
    pub tls: Option<TlsSecurityConfig>,
    pub authentication: Option<AuthConfig>,
    pub trust: Option<TrustConfig>,
    pub secrets: Option<SecretsConfig>,
}

#[derive(Clone, PartialEq)]
pub struct TlsSecurityConfig {
    pub cert_path: String,
    pub key_path: String,
    pub verify_server: bool,
    pub min_tls_version: TlsVersion,
}

#[derive(Clone, PartialEq)]
pub struct TrustConfig {
    pub ca_bundle_path: Option<String>,
    pub use_system_roots: bool,
}

#[derive(Clone, PartialEq)]
pub struct SecretsConfig {
    pub allow_env: bool,
    pub secrets_provider: Option<SecretProvider>,
}
