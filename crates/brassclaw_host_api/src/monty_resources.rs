//! Neutral, instance-wide execution limits. These grant no Tool authority and
//! carry no runtime handles, task values or platform-specific allocator state.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MontyExecutionLimits {
    /// Instance-wide retained child contexts, independent of task worker count.
    #[serde(default = "default_recipe_contexts")]
    pub max_recipe_contexts: u32,
    #[serde(default = "default_queued_tasks")]
    pub max_queued_tasks: u32,
    #[serde(default = "default_queued_bytes")]
    pub max_queued_bytes: u64,
    /// Accepted settings operations, including the currently executing one.
    #[serde(default = "default_pending_settings")]
    pub max_pending_settings: u32,
    /// Shared Rust-owned admissions/attempts retained for acknowledgement/recovery.
    #[serde(default = "default_retained_attempts")]
    pub max_retained_attempts: u32,
    #[serde(default = "default_actor_requests")]
    pub max_actor_requests: u32,
    #[serde(default = "default_actor_bytes")]
    pub max_actor_reserved_bytes: u64,
    #[serde(default = "default_actor_requests")]
    pub max_actor_control_requests: u32,
    #[serde(default = "default_actor_bytes")]
    pub max_actor_control_reserved_bytes: u64,
    /// Native worker startup only; never the lifetime of the global VM.
    #[serde(default = "default_hosting_timeout")]
    pub startup_timeout_millis: u64,
    /// Host IPC/containment time, distinct from task compute and external waits.
    #[serde(default = "default_hosting_timeout")]
    pub response_timeout_millis: u64,
    /// Settings database/probe operations, independent of VM IPC and compute.
    #[serde(default = "default_settings_source_timeout")]
    pub settings_source_timeout_millis: u64,
    /// Settings publication/snapshot waiters; timeout does not cancel accepted work.
    #[serde(default = "default_settings_uptake_timeout")]
    pub settings_uptake_timeout_millis: u64,
    /// Database-instance ownership checks, never a Tool permission lease.
    #[serde(default = "default_settings_source_timeout")]
    pub ownership_check_timeout_millis: u64,
    #[serde(default = "default_settings_source_timeout")]
    pub ownership_heartbeat_interval_millis: u64,
    /// Queued plus executing ownership checks; timeout does not refund a request.
    #[serde(default = "default_actor_requests")]
    pub max_pending_ownership_checks: u32,
    /// Awaiting real cancellation settlement; timeout never discards evidence.
    #[serde(default = "default_settings_uptake_timeout")]
    pub cancellation_ack_timeout_millis: u64,
    pub max_source_bytes: u64,
    pub max_compiled_source_bytes: u64,
    pub max_feeds: u64,
    pub max_stdout_bytes: u64,
    pub execution_slice_millis: u64,
    pub max_value_depth: u32,
    pub max_value_nodes: u64,
    pub max_value_bytes: u64,
}

impl Default for MontyExecutionLimits {
    fn default() -> Self {
        Self {
            max_recipe_contexts: default_recipe_contexts(),
            max_queued_tasks: default_queued_tasks(),
            max_queued_bytes: default_queued_bytes(),
            max_pending_settings: default_pending_settings(),
            max_retained_attempts: default_retained_attempts(),
            max_actor_requests: default_actor_requests(),
            max_actor_reserved_bytes: default_actor_bytes(),
            max_actor_control_requests: default_actor_requests(),
            max_actor_control_reserved_bytes: default_actor_bytes(),
            startup_timeout_millis: default_hosting_timeout(),
            response_timeout_millis: default_hosting_timeout(),
            settings_source_timeout_millis: default_settings_source_timeout(),
            settings_uptake_timeout_millis: default_settings_uptake_timeout(),
            ownership_check_timeout_millis: default_settings_source_timeout(),
            ownership_heartbeat_interval_millis: default_settings_source_timeout(),
            max_pending_ownership_checks: default_actor_requests(),
            cancellation_ack_timeout_millis: default_settings_uptake_timeout(),
            max_source_bytes: 1024 * 1024,
            max_compiled_source_bytes: 2 * 1024 * 1024,
            max_feeds: 128,
            max_stdout_bytes: 1024 * 1024,
            execution_slice_millis: 5,
            max_value_depth: 48,
            max_value_nodes: 1024 * 1024,
            max_value_bytes: 64 * 1024 * 1024,
        }
    }
}

impl MontyExecutionLimits {
    /// Representation-independent validation. Each host also checks its native
    /// integer, transport and control-response constraints before publication.
    pub fn validate(self) -> Result<(), &'static str> {
        if self.max_queued_tasks == 0
            || self.max_pending_settings == 0
            || self.max_retained_attempts == 0
            || self.max_actor_requests == 0
            || self.max_actor_reserved_bytes == 0
            || self.max_actor_control_requests == 0
            || self.max_actor_control_reserved_bytes == 0
            || self.max_queued_bytes == 0
            || self.max_recipe_contexts == 0
            || self.ownership_check_timeout_millis == 0
            || self.ownership_heartbeat_interval_millis == 0
            || self.max_pending_ownership_checks == 0
            || self.cancellation_ack_timeout_millis == 0
            || self.max_source_bytes == 0
            || self.max_compiled_source_bytes < self.max_source_bytes
            || self.max_feeds == 0
            || self.max_stdout_bytes == 0
            || self.startup_timeout_millis == 0
            || self.settings_source_timeout_millis == 0
            || self.settings_uptake_timeout_millis == 0
            || self.response_timeout_millis < self.startup_timeout_millis
            || self.execution_slice_millis >= self.response_timeout_millis
            || self.execution_slice_millis == 0
            || self.execution_slice_millis.checked_mul(1_000_000).is_none()
            || self.max_value_depth == 0
            || self.max_value_nodes == 0
            || self.max_value_bytes == 0
        {
            return Err("invalid Monty execution limits");
        }
        Ok(())
    }
}

fn default_recipe_contexts() -> u32 {
    64
}

fn default_queued_tasks() -> u32 {
    64
}
fn default_queued_bytes() -> u64 {
    64 * 1024 * 1024
}

fn default_pending_settings() -> u32 {
    8
}

fn default_retained_attempts() -> u32 {
    256
}

fn default_actor_requests() -> u32 {
    8
}
fn default_actor_bytes() -> u64 {
    1024 * 1024 * 1024
}

fn default_hosting_timeout() -> u64 {
    30_000
}

fn default_settings_source_timeout() -> u64 {
    2_000
}

fn default_settings_uptake_timeout() -> u64 {
    5_000
}
