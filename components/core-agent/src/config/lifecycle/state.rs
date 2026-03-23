// This is the authoritative model of the agent's live runtime condition used
// in the "Continuous Reconciliation Loop" that reconciles the last received desired_
// state vs current_state. So basically, this is basically logic related to the
// Core Agent's observed runtime state. This completes the "Three-Way" Config
// Lifecycle Management logic
//
// NOTE:...
// This logic obtains the Core Agent's state from memory, not persistence,
// i.e, Active Subsystems(Source Drivers, Processor Pipeline, etc.), Running
// tasks, Enabled Subsystem Components, Resource Handles, System Health, Core
// Agent Version, In-Progress Operations, Last Successfully Applied Config.
//
// THIS IS NOT current_config THAT IS PERSISTED IN LMDB, THIS IS WHAT IS ACTUALLY
// RUNNING IN THE CORE AGENT.
//
// Responsibilities;
// - Track lifecycle of subsystems
// - Store resource handles
// - Track core agent health
// - Provide introspection methods for apply.rs
// - Support observability
// - Idempotency

use crate::config::schema::root::RootConfig;

use std::collections::HashMap;
use std::time::{Instant, Duration};
use tokio::task::JoinHandle;

#[derive(Debug, PartialEq)]
pub enum LifecycleState {
    Starting,
    Running,
    ApplyingConfig,
    Stopping,
    Stopped,
    Failed,
}

pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, PartialEq)]
pub enum ComponentStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

pub enum SourceDriverKind {
    Filesystem,
    Journald,
    Socket,
}

pub enum StorageBackendKind {
    Database,
}

pub enum SecurityMode {
    None,
    TLS,
    MutualTLS,
    Custom,
}

pub struct LiveState {
    pub lifecycle: LifecycleState,
    pub runtime: RuntimeState,
    pub sources: SourcesState,
    pub processor: ProcessorState,
    pub storage: Option<StorageState>,
    pub security: Option<SecurityState>,
    pub last_applied_config: Option<RootConfig>,
    pub apply_progress: Option<ApplyProgress>,
    pub health: HealthStatus,
    pub last_error: Option<String>,
}

impl LiveState {
    pub fn can_apply_config(&self) -> bool {
        matches!(self.lifecycle, LifecycleState::Running)
    }

    pub fn begin_apply(&mut self, total_ops: usize) {
        self.lifecycle = LifecycleState::ApplyingConfig;

        self.apply_progress = Some(ApplyProgress {
            started_at: Instant::now(),
            total_ops,
            completed_ops: 0,
            current_op: None,
        });
    }

    pub fn advance_apply_progress(&mut self, op_name: &str) {
        if let Some(progress) = &mut self.apply_progress {
            progress.completed_ops += 1;
            progress.current_op = Some(op_name.to_string());
        }
    }

    pub fn finish_apply(&mut self) {
        self.lifecycle = LifecycleState::Running;
        self.apply_progress = None;
    }

    pub fn fail_apply(&mut self, error: String) {
        self.lifecycle = LifecycleState::Failed;
        self.last_error = Some(error);
        self.apply_progress = None;
    }

    pub fn refigure_health(&mut self) {
        if self.lifecycle == LifecycleState::Failed {
            self.health = HealthStatus::Unhealthy;
            return;
        }

        if self.sources.instances.values().any(|s| s.status == ComponentStatus::Failed) {
            self.health = HealthStatus::Degraded;
            return;
        }

        self.health = HealthStatus::Healthy;
    }

    pub fn update_uptime(&mut self) {
        self.runtime.uptime = self.runtime.started_at.elapsed();
    }

    pub fn applied_config(&mut self, config: RootConfig) {
        self.last_applied_config = Some(config);
    }

    pub fn mark_failed(&mut self, error: String) {
        self.lifecycle = LifecycleState::Failed;
        self.health = HealthStatus::Unhealthy;
        self.last_error = Some(error);
    }
}

pub struct RuntimeState {
    pub started_at: Instant,
    pub version: String,
    pub worker_threads: usize,
    pub uptime: Duration,
}

pub struct SourcesState {
    pub instances: HashMap<String, SourceInstanceState>,
}

impl SourcesState {
    pub fn insert_new(
        &mut self,
        instance: SourceInstanceState
    ) -> Result<(), String> {
        if self.instances.contains_key(&instance.id) {
            return Err(format!("Source Driver {} already exists", instance.id));
        }

        self.instances.insert(instance.id.clone(), instance);
        Ok(())
    }

    pub fn remove_if_stopped(
        &mut self,
        id: &str
    ) -> Result<SourceInstanceState, String> {
        let instance = self.instances.get(id)
            .ok_or_else(|| format!("Source Driver {} not found", id))?;

        if instance.status != ComponentStatus::Stopped {
            return Err("Cannot remove running source driver".into());
        }

        Ok(self.instances.remove(id).unwrap())
    }

    pub fn running_source_drivers(&self) -> Vec<&SourceInstanceState> {
        self.instances
            .values()
            .filter(|s| s.status == ComponentStatus::Running)
            .collect()
    }

    pub fn any_failing(&self) -> bool {
        self.instances
            .values()
            .any(|s| s.status == ComponentStatus::Failed)
    }

    pub fn get(&self, id: &str) -> Option<&SourceInstanceState> {
        self.instances.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut SourceInstanceState> {
        self.instances.get_mut(id)
    }

    pub fn exists(&self, id: &str) -> bool {
        self.instances.contains_key(id)
    }
}

pub struct SourceInstanceState {
    pub id: String,
    pub kind: SourceDriverKind,
    pub enabled: bool,
    pub status: ComponentStatus,
    pub restart_count: u32,
    pub last_error: Option<String>,
    pub started_at: Option<Instant>,
    pub last_stopped_at: Option<Instant>,
}

impl SourceInstanceState {
    pub fn is_active(&self) -> bool {
        matches!(self.status,
           ComponentStatus::Starting | ComponentStatus::Running
        )
    }

    pub fn is_starting(&self) -> bool {
        self.status == ComponentStatus::Starting
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            ComponentStatus::Stopped | ComponentStatus::Failed
        )
    }

    pub fn is_stopping(&self) -> bool {
        self.status == ComponentStatus::Stopping
    }

    pub fn is_stopped(&self) -> bool {
        self.status == ComponentStatus::Stopped
    }

    pub fn is_failed(&self) -> bool {
        self.status == ComponentStatus::Failed
    }

    pub fn source_driver_uptime(&self) -> Option<Duration> {
        self.started_at.map(|t| t.elapsed())
    }

    pub fn should_be_active(&self) -> bool {
        self.enabled
    }

    pub fn needs_start(&self) -> bool {
        self.should_be_active() && self.is_stopped()
    }

    pub fn needs_restart(&self) -> bool {
        self.should_be_active() && self.is_failed()
    }

    pub fn needs_stop(&self) -> bool {
        !self.should_be_active() && self.is_active()
    }
}

pub struct ProcessorState {
    pub status: ComponentStatus,
    pub pipeline_version: u64,
    pub last_rebuild: Option<Instant>,
    pub last_error: Option<String>,
}

impl ProcessorState {
    // [TODO]: Core Agent Processor Pipeline is still unimplemented
}

pub struct StorageState {
    pub status: ComponentStatus,
    pub backend: StorageBackendKind,
    pub connection_alive: bool,
    pub last_error: Option<String>,
    pub connected_at: Option<Instant>,
}

impl StorageState {
    pub fn is_running(&self) -> bool {
        self.status == ComponentStatus::Running
    }

    pub fn is_starting(&self) -> bool {
        self.status == ComponentStatus::Starting
    }

    pub fn is_stopping(&self) -> bool {
        self.status == ComponentStatus::Stopping
    }

    pub fn is_failed(&self) -> bool {
        self.status == ComponentStatus::Failed
    }

    pub fn is_connected(&self) -> bool {
        self.connection_alive && self.status == ComponentStatus::Running
    }

    pub fn is_unhealthy(&self) -> bool {
        !self.is_connected() || self.is_failed()
    }

    pub fn uptime(&self) -> Option<Duration> {
        self.connected_at.map(|t| t.elapsed())
    }
}

pub struct SecurityState {
    pub status: ComponentStatus,
    pub mode: SecurityMode,
    pub credentials_loaded: bool,
    pub last_validation: Option<Instant>,
    pub last_error: Option<String>,
}

impl SecurityState {
    pub fn is_running(&self) -> bool {
        self.status == ComponentStatus::Running
    }

    pub fn starting(&self) -> bool {
        self.status == ComponentStatus::Starting
    }

    pub fn is_failed(&self) -> bool {
        self.status == ComponentStatus::Failed
    }

    pub fn are_credentials_loaded(&self) -> bool {
        self.credentials_loaded
    }

    pub fn is_validated(&self) -> bool {
        self.last_validation.is_some() && self.is_running()
    }

    pub fn is_unhealthy(&self) -> bool {
        self.is_failed() || !self.are_credentials_loaded()
    }

    pub fn time_since_last_valid(&self) -> Option<Duration> {
        self.last_validation.map(|t| t.elapsed())
    }
}

pub struct ApplyProgress {
    pub started_at: Instant,
    pub total_ops: usize,
    pub completed_ops: usize,
    pub current_op: Option<String>,
}
