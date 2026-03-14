// =============================================================================
// This configuration schema defines the top-level configuration structure used
// by the Core Agent's runtime. This is a ccmplete configuration snapshot,
// aggregating all configuration groups into one structure, containing agent,
// source and processor configuration.
//
// Every supported configuration provider, static file provider, HEIMDELL Server
// Remote API Provider and Local/Remote API Provider must produce a configuration
// snapshot that conforms to this root structure. The configuration lifecycle
// manager uses this structure when performing validating, diff computing +
// configuration event creation and current_config persistence.
//
// RootConfig describes the Core Agent's desired state not current state
// ============================================================================

use crate::config::schema::{
    agent::CoreAgentConfig,
    sources::SourcesConfig,
    processor::ProcessorConfig,
    security::SecurityConfig,
    storage::StorageConfig,
};

pub enum ConfigProvider {
    StaticFile,
    HEIMDELLServerRemoteAPI,
    LocalRemoteAPI,
}

pub struct RootConfig {
    pub version: u32,
    pub agent: CoreAgentConfig,
    pub sources: SourcesConfig,
    pub processor: ProcessorConfig,
    pub metadata: Option<RootConfigMetadata>,
    pub storage: Option<StorageConfig>,
    pub security: Option<SecurityConfig>,
}

pub struct RootConfigMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_by: Option<ConfigProvider>,
    pub created_at: Option<u64>,
    pub labels: Option<Vec<(String, String)>>,
}
