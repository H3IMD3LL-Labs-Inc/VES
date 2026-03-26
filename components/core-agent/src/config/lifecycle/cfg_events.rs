use crate::config::{
    lifecycle::{
        validation::{ValidationError, ValidationWarning},
        diff::RootConfigDiff,
        version::ConfigVersion,
    },
    schema::root::ConfigProvider
};

use std::time::SystemTime;

#[derive(Clone)]
pub enum ApplyStatus {
    Started,
    Succeeded,
    Failed { error: String },
}

#[derive(Clone)]
pub enum ConfigValidation {
    Success {
        warnings: ValidationWarning,
    },
    Failed {
        errors: ValidationError,
    }
}

#[derive(Clone)]
pub enum ConfigEvent {
    NewSnapshot {
        source: ConfigProvider,
        version: ConfigVersion,
        received_at: SystemTime,
    },
    Validation {
        source: ConfigProvider,
        version: ConfigVersion,
        result: ConfigValidation,
    },
    NoChanges {
        source: ConfigProvider,
        version: ConfigVersion,
        diffs: RootConfigDiff,
    },
    DiffPresent {
        source: ConfigProvider,
        version: ConfigVersion,
        diff: RootConfigDiff,
    },
    SnapshotApplied {
        source: ConfigProvider,
        version: ConfigVersion,
        status: ApplyStatus,
    },
    PersistedSnapshot {
        version: ConfigVersion,
    },
}
