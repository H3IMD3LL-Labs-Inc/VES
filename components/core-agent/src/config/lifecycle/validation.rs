use crate::config::schema::root::RootConfig;

use std::collections::HashSet;

pub enum ValidationError {
    DuplicateId { id: String },
    MissingTlsMaterial { id: String },
    PathNotFound { path: String },
    UnsupportedFeature { feature: String },
    InvalidConfiguration { reason: String },
    PermissionDenied { path: String },
    InvalidSocketBind {
        id: String,
        reason: String,
    },
}

pub enum ValidationWarning {
    InsecureConfiguration { reason: String },
    ResourceRisk { reason: String },
    DeprecatedField { field: String },
}

pub struct ConfigValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ConfigValidationResult {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn push_error(&mut self, err: ValidationError) {
        self.errors.push(err);
    }

    pub fn push_warning(&mut self, warn: ValidationWarning) {
        self.warnings.push(warn);
    }
}

// ============================================================================
// Determine whether the received root configuration schema received from a
// Configuration provider is valid and should be processed by the configuration
// system. THIS SHOULD BE USED TO DETERMINE WHEN TO RETURN THE CONFIGURATION
// DIFF BETWEEN THE CURRENT STATE AND DESIRED STATE OF THE CORE AGENT
// ============================================================================
pub fn validate_root_config(config: &RootConfig) -> ConfigValidationResult {
    let mut config_result = ConfigValidationResult::new();

    validate_sources_config(config, &mut config_result);
    validate_storage_config(config, &mut config_result);
    validate_security_config(config, &mut config_result);
    validate_cross_component(config, &mut config_result);

    config_result
}

fn validate_sources_config(
    config: &RootConfig,
    result: &mut ConfigValidationResult
) {
    let mut driver_ids = HashSet::new();

    let sources = &config.sources;

    for fs in &sources.filesystem {
        if !driver_ids.insert(&fs.id) {
            result.push_error(ValidationError::DuplicateId {
                id: fs.id.clone(),
            });
        }

        if fs.paths.is_empty() {
            result.push_error(ValidationError::InvalidConfiguration {
                reason: format!("filesystem source '{}' has no paths", fs.id),
            })
        }

        for path in &fs.paths {
            if !std::path::Path::new(path).exists() {
                result.push_error(ValidationError::PathNotFound {
                    path: path.clone(),
                });
            }
        }

        // [TODO]: Journald source driver instance configuration validation
        // [TODO]: Socket source driver instance configuration validation
    }
}

fn validate_storage_config(
    config: &RootConfig,
    result: &mut ConfigValidationResult,
) {
    if let Some(storage) = &config.storage {
        let path = std::path::Path::new(&storage.data_dir);

        if !path.exists() {
            result.push_error(ValidationError::PathNotFound {
                path: storage.data_dir.clone(),
            });
        }

        if path.metadata().map(|m| m.permissions().readonly()).unwrap_or(true) {
            result.push_error(ValidationError::PermissionDenied {
                path: storage.data_dir.clone(),
            });
        }
    }
}

fn validate_security_config(
    config: &RootConfig,
    result: &mut ConfigValidationResult,
) {
    if let Some(sec) = &config.security {
        if let Some(tls) = &sec.tls {
            if tls.cert_path.is_empty() || tls.key_path.is_empty() {
                result.push_error(ValidationError::InvalidConfiguration {
                    reason: "global TLS enabled but cert/key missing".into(),
                });
            }
        }
    }
}

fn validate_cross_component(
    config: &RootConfig,
    result: &mut ConfigValidationResult
) {
    if let Some(sec) = &config.security {
        if sec.authentication.is_some() && sec.tls.is_none() {
            result.push_warning(ValidationWarning::InsecureConfiguration {
                reason: "authentication enabled without TLS".into(),
            });
        }
    }
}
