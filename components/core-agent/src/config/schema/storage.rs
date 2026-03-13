// =============================================================================
// This configuration schema defines how root configuration is persisted to disk
// using LMDB via the Core Agent's recovery/ module
// =============================================================================

pub enum StorageEngine {
    Lmdb,
    // [TODO]: Support other storage engines
}

pub struct StorageConfig {
    pub data_dir: String,
    pub storage_engine: StorageEngine,
    pub max_disk_bytes: Option<u64>,
    pub cleanup: Option<CleanupPolicy>,
}

pub struct CleanupPolicy {
    pub max_period_secs: Option<u64>,
    pub interval_secs: u64,
}
