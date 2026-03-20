// =============================================================================
// This configuration schema defines how root configuration is persisted to disk
// using LMDB via the Core Agent's recovery/ module
// =============================================================================

#[derive(Clone, PartialEq)]
pub enum StorageEngine {
    Lmdb,
}

#[derive(Clone, PartialEq)]
pub struct StorageConfig {
    pub data_dir: String,
    pub storage_engine: StorageEngine,
    pub max_disk_bytes: Option<u64>,
    pub cleanup: Option<CleanupPolicy>,
}

#[derive(Clone, PartialEq)]
pub struct CleanupPolicy {
    pub max_period_secs: Option<u64>,
    pub interval_secs: u64,
}
