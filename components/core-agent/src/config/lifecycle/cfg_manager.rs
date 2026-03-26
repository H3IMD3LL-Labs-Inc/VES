use crate::config::{
    schema::root::{RootConfig, ConfigProvider},
    lifecycle::{
        validation::validate_root_config,
        version::ConfigVersion,
        diff::{diff_root, RootConfigDiff},
        state::LiveState,
        apply::apply_config,
        cfg_events::{
            ConfigEvent,
            ConfigValidation,
            ApplyStatus
        },
    }
};

use std::time::SystemTime;

pub struct ConfigManager {
    pub state: LiveState,
    pub last_applied_version: Option<ConfigVersion>,
}

impl ConfigManager {
    pub fn new(state: LiveState) -> Self {
        Self {
            state,
            last_applied_version: None,
        }
    }

    pub fn process_config(
        &mut self,
        source: ConfigProvider,
        config: RootConfig,
        raw_bytes: &[u8],
    ) -> Vec<ConfigEvent> {
        let mut events = Vec::new();

        let version = ConfigVersion::from_root_config_bytes(raw_bytes);

        if self.last_applied_version.as_ref() == Some(&version) {
            return events;
        }

        events.push(ConfigEvent::NewSnapshot {
            source: source.clone(),
            version: version.clone(),
            received_at: SystemTime::now(),
        });

        let validation = validate_root_config(&config);

        if !validation.is_valid() {
            for err in validation.errors {
                events.push(ConfigEvent::Validation {
                    source: source.clone(),
                    version: version.clone(),
                    result: ConfigValidation::Failed { errors: err },
                });
            }

            return events;
        }

        for warn in validation.warnings {
            events.push(ConfigEvent::Validation {
                source: source.clone(),
                version: version.clone(),
                result: ConfigValidation::Success { warnings: warn },
            });
        }

        let diff = match &self.state.last_applied_config {
            Some(old) => diff_root(old, &config),
            None => diff_empty_root_config(&config),
        };

        if !diff.has_changes {
            events.push(ConfigEvent::NoChanges {
                source,
                version,
                diffs: diff,
            });

            return events;
        }

        events.push(ConfigEvent::DiffPresent {
            source: source.clone(),
            version: version.clone(),
            diff: diff.clone(),
        });

        events.push(ConfigEvent::SnapshotApplied {
            source: source.clone(),
            version: version.clone(),
            status: ApplyStatus::Started,
        });

        match apply_config(&mut self.state, diff, config.clone()) {
            Ok(_) => {
                self.last_applied_version = Some(version.clone());

                events.push(ConfigEvent::SnapshotApplied {
                    source: source.clone(),
                    version: version.clone(),
                    status: ApplyStatus::Succeeded,
                });
            }
            Err(e) => {
                events.push(ConfigEvent::SnapshotApplied {
                    source,
                    version,
                    status: ApplyStatus::Failed { error: e },
                });

                return events;
            }
        }

        // [TODO]: Perform actual persistence of the successfully applied
        //         RootConfig as the "latest" state/config
        events.push(ConfigEvent::PersistedSnapshot { version });

        events
    }
}

fn diff_empty_root_config(new: &RootConfig) -> RootConfigDiff {
    diff_root(&RootConfig::empty(), new)
}
