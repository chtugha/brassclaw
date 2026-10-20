//! Neutral, instance-wide execution limits. These grant no Tool authority and
//! carry no runtime handles, task values or platform-specific allocator state.
use serde::{Deserialize, Serialize};

pub const DEFAULT_MONTY_ADAPTER_RESERVE_BYTES: u64 = 4 * 1024 * 1024;
pub const DEFAULT_MONTY_IPC_FRAME_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MontyExecutionLimits {
    /// Physical allocator ceiling for each disposable parser/validator worker.
    #[serde(default = "default_utility_memory")]
    pub utility_worker_memory_bytes: u64,
    /// Maximum bytes captured from one retained native executable.
    #[serde(default = "default_artifact_bytes")]
    pub max_native_artifact_bytes: u64,
    /// Admission bounds for a new catalogue generation; retained generations
    /// keep the preparation policy that qualified their exact selection.
    #[serde(default = "default_catalogue_recipes")]
    pub max_catalogue_recipes: u32,
    #[serde(default = "default_catalogue_components")]
    pub max_catalogue_components_per_recipe: u32,
    #[serde(default = "default_catalogue_recipes")]
    pub max_catalogue_variants_per_recipe: u32,
    #[serde(default = "default_ipc_frame")]
    pub max_catalogue_bytes: u64,
    #[serde(default = "default_catalogue_actor_bytes")]
    pub max_catalogue_actor_bytes: u32,
    /// Retained host HTTP clients. Zero disables caching; active request
    /// handles survive eviction. This is not a model-response cache.
    #[serde(default = "default_http_client_cache_entries")]
    pub max_http_client_cache_entries: u32,
    /// Distinct active MCP request IDs per provider exchange. Completed calls
    /// are released; duplicate waiters share one entry. Lowering preserves
    /// accepted requests and stops new identities until occupancy permits them.
    #[serde(default = "default_mcp_inflight_calls")]
    pub max_mcp_inflight_calls: u32,
    /// Encoded MCP HTTP request body, captured before body decoding. Existing
    /// accepted reads keep their captured ceiling across live settings edits.
    #[serde(default = "default_mcp_message_bytes")]
    pub max_mcp_request_bytes: u32,
    /// Encoded MCP HTTP response consumed by Kohai, captured before sending
    /// each operation. Overflow is an error, never a truncated answer or retry.
    #[serde(default = "default_mcp_message_bytes")]
    pub max_mcp_response_bytes: u32,
    /// Reservation-only output estimate when neither the work request nor the
    /// selected cost row supplies an output estimate. Never a response cap.
    #[serde(default = "default_model_quote_output_tokens")]
    pub model_quote_output_tokens: u32,
    /// Deadline per PostgreSQL resource-account or budget-gate exchange.
    /// A read/CAS-write operation captures one revision for both exchanges.
    #[serde(default = "default_resource_store_timeout")]
    pub resource_store_timeout_millis: u64,
    /// Defaults for host HTTP requests that omit explicit bounds.
    #[serde(default = "default_http_request_timeout")]
    pub default_http_request_timeout_millis: u64,
    #[serde(default = "default_http_response_body_bytes")]
    pub default_http_response_body_bytes: u64,
    /// Memory tree depth used when the caller omits it, and the exact largest
    /// permitted request. The resulting value still obeys typed-value limits.
    #[serde(default = "default_memory_tree_depth")]
    pub default_memory_tree_depth: u32,
    #[serde(default = "default_memory_tree_max_depth")]
    pub max_memory_tree_depth: u32,
    /// Bound for newly accepted exchanges; older credits retain their bounds.
    #[serde(default = "default_ipc_frame")]
    pub max_ipc_frame_bytes: u64,
    /// Worker adapter/exception headroom, separate from VM heap and IPC frames.
    #[serde(default = "default_adapter_reserve")]
    pub worker_adapter_reserve_bytes: u64,
    /// Desired independent task slots in the one global VM. A dependent turn
    /// can borrow its parked parent's slot; retained state still obeys the
    /// shared memory and host-call limits. No per-worker VM or reserved heap.
    #[serde(default = "default_workers")]
    pub max_workers: u32,
    /// Instance-wide retained child contexts, independent of task worker count.
    #[serde(default = "default_recipe_contexts")]
    pub max_recipe_contexts: u32,
    /// Global outstanding host calls or withheld late answers. The worker
    /// checks its current occupancy before accepting a live reduction.
    #[serde(default = "default_pending_host_calls")]
    pub max_pending_host_calls: u32,
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
    /// Cancellation or pre-VM rejection acknowledgement; timeout retains evidence.
    #[serde(default = "default_settings_uptake_timeout")]
    pub cancellation_ack_timeout_millis: u64,
    /// Idle durable-settings reconciliation; operator saves wake it immediately.
    #[serde(default = "default_settings_reconcile_interval")]
    pub settings_reconcile_interval_millis: u64,
    /// Browser status refresh while uptake/restart is pending; no idle polling.
    #[serde(default = "default_status_poll_interval")]
    pub status_poll_interval_millis: u32,
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
            utility_worker_memory_bytes: default_utility_memory(),
            max_native_artifact_bytes: default_artifact_bytes(),
            max_catalogue_recipes: default_catalogue_recipes(),
            max_catalogue_components_per_recipe: default_catalogue_components(),
            max_catalogue_variants_per_recipe: default_catalogue_recipes(),
            max_catalogue_bytes: default_ipc_frame(),
            max_catalogue_actor_bytes: default_catalogue_actor_bytes(),
            max_http_client_cache_entries: default_http_client_cache_entries(),
            max_mcp_inflight_calls: default_mcp_inflight_calls(),
            max_mcp_request_bytes: default_mcp_message_bytes(),
            max_mcp_response_bytes: default_mcp_message_bytes(),
            model_quote_output_tokens: default_model_quote_output_tokens(),
            resource_store_timeout_millis: default_resource_store_timeout(),
            default_http_request_timeout_millis: default_http_request_timeout(),
            default_http_response_body_bytes: default_http_response_body_bytes(),
            default_memory_tree_depth: default_memory_tree_depth(),
            max_memory_tree_depth: default_memory_tree_max_depth(),
            max_ipc_frame_bytes: default_ipc_frame(),
            worker_adapter_reserve_bytes: default_adapter_reserve(),
            max_workers: default_workers(),
            max_recipe_contexts: default_recipe_contexts(),
            max_pending_host_calls: default_pending_host_calls(),
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
            settings_reconcile_interval_millis: default_settings_reconcile_interval(),
            status_poll_interval_millis: default_status_poll_interval(),
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
        if self.utility_worker_memory_bytes <= self.max_ipc_frame_bytes
            || self.utility_worker_memory_bytes == u64::MAX
            || self.max_native_artifact_bytes == 0
            || self.max_native_artifact_bytes == u64::MAX
            || self.model_quote_output_tokens == 0
            || self.resource_store_timeout_millis == 0
            || self.default_http_request_timeout_millis == 0
            || self.default_memory_tree_depth == 0
            || self.default_memory_tree_depth > self.max_memory_tree_depth
            || self.default_http_response_body_bytes == 0
            || self.default_http_response_body_bytes > isize::MAX as u64
            || self.max_catalogue_recipes == 0
            || self.max_mcp_inflight_calls == 0
            || self.max_mcp_request_bytes == 0
            || self.max_mcp_response_bytes == 0
            || u64::from(self.max_mcp_request_bytes) > isize::MAX as u64
            || u64::from(self.max_mcp_response_bytes) > isize::MAX as u64
            || self.max_catalogue_components_per_recipe == 0
            || self.max_catalogue_variants_per_recipe == 0
            || self.max_catalogue_actor_bytes == 0
            || self.max_catalogue_bytes == 0
            || self.max_catalogue_bytes == u64::MAX
            || usize::try_from(self.max_catalogue_bytes).is_err()
            || self.max_ipc_frame_bytes == 0
            || u32::try_from(self.max_ipc_frame_bytes).is_err()
            || self.max_source_bytes > self.max_ipc_frame_bytes
            || self.max_stdout_bytes > self.max_ipc_frame_bytes
            || self.max_value_bytes > self.max_ipc_frame_bytes
            || self.max_actor_reserved_bytes <= self.max_ipc_frame_bytes
            || self.max_ipc_frame_bytes.checked_mul(2).is_none_or(|bytes|
                self.max_actor_control_reserved_bytes < bytes)
            || self.max_queued_tasks == 0
            || self.max_pending_settings == 0
            || self.max_retained_attempts == 0
            || self.max_actor_requests == 0
            || self.max_actor_reserved_bytes == 0
            || self.max_actor_control_requests == 0
            || self.max_actor_control_reserved_bytes == 0
            || self.max_queued_bytes == 0
            || self.max_recipe_contexts == 0
            || self.max_workers == 0
            || self.max_workers.checked_add(1).is_none_or(|minimum| self.max_pending_host_calls < minimum)
            || self.ownership_check_timeout_millis == 0
            || self.ownership_heartbeat_interval_millis == 0
            || self.max_pending_ownership_checks == 0
            || self.cancellation_ack_timeout_millis == 0
            || self.settings_reconcile_interval_millis == 0
            || self.status_poll_interval_millis == 0
            // Browser timers accept a signed 32-bit millisecond delay. Larger
            // values can wrap into immediate polling; reject rather than clamp.
            || self.status_poll_interval_millis > i32::MAX as u32
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

fn default_memory_tree_depth() -> u32 {
    1
}

fn default_memory_tree_max_depth() -> u32 {
    10
}

fn default_recipe_contexts() -> u32 {
    64
}

fn default_workers() -> u32 {
    2
}

fn default_pending_host_calls() -> u32 {
    128
}

fn default_model_quote_output_tokens() -> u32 {
    8192
}

fn default_resource_store_timeout() -> u64 {
    2000
}

fn default_http_request_timeout() -> u64 {
    30_000
}
fn default_http_response_body_bytes() -> u64 {
    10 * 1024 * 1024
}

fn default_http_client_cache_entries() -> u32 {
    128
}

fn default_mcp_inflight_calls() -> u32 {
    4096
}

fn default_mcp_message_bytes() -> u32 {
    1024 * 1024
}

fn default_catalogue_recipes() -> u32 {
    512
}
fn default_catalogue_components() -> u32 {
    4096
}
fn default_catalogue_actor_bytes() -> u32 {
    2048
}

fn default_utility_memory() -> u64 {
    132 * 1024 * 1024
}

fn default_artifact_bytes() -> u64 {
    1024 * 1024 * 1024
}

fn default_ipc_frame() -> u64 {
    DEFAULT_MONTY_IPC_FRAME_BYTES
}

fn default_adapter_reserve() -> u64 {
    DEFAULT_MONTY_ADAPTER_RESERVE_BYTES
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

fn default_settings_reconcile_interval() -> u64 {
    1_000
}

fn default_status_poll_interval() -> u32 {
    3_000
}
