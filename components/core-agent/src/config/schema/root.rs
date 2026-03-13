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
    // Schema version for compatibility and migrations
    pub version: u32,
    // Core Agent runtime behavior configuration
    pub agent: CoreAgentConfig,
    // Source Drivers configuration
    pub sources: SourcesConfig,
    // Processor pipeline configuration
    pub processor: ProcessorConfig,
    // For-context root configuration metadata
    pub metadata: Option<RootConfigMetadata>,
    // Persistent storage configuration
    pub storage: Option<StorageConfig>,
    // Config security configuration
    pub security: Option<SecurityConfig>,
}

pub struct RootConfigMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    // Configuration provider that created this configuration
    pub created_by: Option<ConfigProvider>,
    // Unix timestamp of creation
    pub created_at: Option<u64>,
    // Key/Value pairs for additional context
    pub labels: Vec<(String, String)>,
}
