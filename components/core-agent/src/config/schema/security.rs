// =============================================================================
// This configuration schema describes how a Core Agent's security settings are
// configurable, TLS, authentication, trust, secret management, etc.
// =============================================================================

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum TlsVersion {
    Tls12,
    Tls13,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum AuthConfig {
    ApiKey { key: String },
    Token { token: String },
    None,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum SecretProvider {
    File { path: String },
    AwsSecretsManager,
    HashiCorpVault,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SecurityConfig {
    pub tls: Option<TlsSecurityConfig>,
    pub authentication: Option<AuthConfig>,
    pub trust: Option<TrustConfig>,
    pub secrets: Option<SecretsConfig>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct TlsSecurityConfig {
    pub cert_path: String,
    pub key_path: String,
    pub verify_server: bool,
    pub min_tls_version: TlsVersion,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct TrustConfig {
    pub ca_bundle_path: Option<String>,
    pub use_system_roots: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SecretsConfig {
    pub allow_env: bool,
    pub secrets_provider: Option<SecretProvider>,
}
