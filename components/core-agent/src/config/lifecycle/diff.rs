use crate::config::schema::{
    agent::CoreAgentConfig,
    sources::{
        SourcesConfig,
        FilesystemSourceConfig,
        JournaldSourceConfig,
        SocketSourceConfig,
    },
    processor::ProcessorConfig,
    security::SecurityConfig,
    storage::StorageConfig,
    root::RootConfig,
};

use std::collections::HashMap;
use std::mem::discriminant;

#[derive(Clone)]
pub struct RootConfigDiff {
    pub has_changes: bool,
    pub operations: Vec<Operation>,
    pub validation_errors: Vec<String>,
}

#[derive(Clone)]
pub enum Operation {
    UpdateCoreAgentRuntime(CoreAgentConfig),

    EnableSource { id: String },
    DisableSource { id: String },
    AddSource(SourceSpec),
    RemoveSource { id: String },
    UpdateSource { id: String, spec: SourceSpec },

    UpdateProcessor(ProcessorConfig),

    EnableStorage(StorageConfig),
    DisableStorage,
    UpdateStorage(StorageConfig),

    EnableSecurity(SecurityConfig),
    DisableSecurity,
    UpdateSecurity(SecurityConfig),
}

#[derive(Clone, PartialEq)]
pub enum SourceSpec {
    FileSystem(FilesystemSourceConfig),
    Journald(JournaldSourceConfig),
    Socket(SocketSourceConfig),
}

impl SourceSpec {
    pub fn id(&self) -> &str {
        match self {
            SourceSpec::FileSystem(s) => &s.id,
            SourceSpec::Journald(s) => &s.id,
            SourceSpec::Socket(s) => &s.id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            SourceSpec::FileSystem(s) => s.enabled,
            SourceSpec::Journald(s) => s.enabled,
            SourceSpec::Socket(s) => s.enabled,
        }
    }
}

pub fn diff_root(old: &RootConfig, new: &RootConfig) -> RootConfigDiff {
    let mut diff_ops = Vec::new();
    let mut errors = Vec::new();

    if old.agent != new.agent {
        diff_ops.push(Operation::UpdateCoreAgentRuntime(new.agent.clone()));
    }

    diff_sources(
        &old.sources,
        &new.sources,
        &mut diff_ops,
        &mut errors,
    );

    if old.processor != new.processor {
        diff_ops.push(Operation::UpdateProcessor(new.processor.clone()));
    }

    match (&old.storage, &new.storage) {
        (None, Some(n)) => {
            diff_ops.push(Operation::EnableStorage(n.clone()));
        }
        (Some(_), None) => {
            diff_ops.push(Operation::DisableStorage);
        }
        (Some(o), Some(n)) if o != n => {
            diff_ops.push(Operation::UpdateStorage(n.clone()));
        }
        _ => {}
    }

    match (&old.security, &new.security) {
        (None, Some(n)) => {
            diff_ops.push(Operation::EnableSecurity(n.clone()));
        }
        (Some(_), None) => {
            diff_ops.push(Operation::DisableSecurity);
        }
        (Some(o), Some(n)) if o != n => {
            diff_ops.push(Operation::UpdateSecurity(n.clone()));
        }
        _ => {}
    }

    RootConfigDiff {
        has_changes: !diff_ops.is_empty(),
        operations: diff_ops,
        validation_errors: errors,
    }
}

fn diff_sources(
    old: &SourcesConfig,
    new: &SourcesConfig,
    diff_ops: &mut Vec<Operation>,
    errors: &mut Vec<String>,
) {
    let old_map = collect_sources(old, errors);
    let new_map = collect_sources(new, errors);

    for (id, new_source) in &new_map {
        if !old_map.contains_key(id) {
            diff_ops.push(Operation::AddSource(new_source.clone()));
        }
    }

    for id in old_map.keys() {
        if !new_map.contains_key(id) {
            diff_ops.push(Operation::RemoveSource { id: id.clone() });
        }
    }

    for (id, new_source) in &new_map {
        if let Some(old_source) = old_map.get(id) {
            if discriminant(old_source) != discriminant(new_source) {
                diff_ops.push(Operation::RemoveSource { id: id.clone() });
                diff_ops.push(Operation::AddSource(new_source.clone()));
                continue;
            }

            if old_source.enabled() != new_source.enabled() {
                if new_source.enabled() {
                    diff_ops.push(Operation::EnableSource { id: id.clone() });
                } else {
                    diff_ops.push(Operation::DisableSource { id: id.clone() });
                }
            }

            else if old_source != new_source {
                diff_ops.push(Operation::UpdateSource {
                    id: id.clone(),
                    spec: new_source.clone(),
                });
            }
        }
    }
}

fn collect_sources(
    cfg: &SourcesConfig,
    errors: &mut Vec<String>,
) -> HashMap<String, SourceSpec> {
    let mut sd_instances = HashMap::new();

    for s in &cfg.filesystem {
        insert_source(&mut sd_instances, SourceSpec::FileSystem(s.clone()), errors);
    }

    for s in &cfg.journald {
        insert_source(&mut sd_instances, SourceSpec::Journald(s.clone()), errors);
    }

    for s in &cfg.socket {
        insert_source(&mut sd_instances, SourceSpec::Socket(s.clone()), errors);
    }

    sd_instances
}

fn insert_source(
    instances: &mut HashMap<String, SourceSpec>,
    spec: SourceSpec,
    errors: &mut Vec<String>,
) {
    let sd_id = spec.id().to_string();

    if instances.contains_key(&sd_id) {
        errors.push(format!("Duplicate source id across types: {}", sd_id));
    } else {
        instances.insert(sd_id, spec);
    }
}
