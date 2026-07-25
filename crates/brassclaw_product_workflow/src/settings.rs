//! Settings facade types for the Phase 6 10-tab Settings UI.
//!
//! These types cover the REST surface for `/api/settings/*` routes and
//! `PUT /api/chat/preferences/{key}` (ai_before_user persistence).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ── Shared component list item ────────────────────────────────────────────────

/// Minimal summary row returned by the settings list endpoints.
/// Enough for the list view — name, class, validation status, tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsComponentSummary {
    pub id: String,
    pub name: String,
    pub class_code: u16,
    pub prompt_uid: Option<u64>,
    pub validation_status: String,
    pub tier: Option<String>,
    pub consumer_tags: Vec<String>,
    pub description: Option<String>,
    pub version: Option<String>,
}

/// Response body for list endpoints (`GET /api/settings/*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsListResponse {
    pub items: Vec<SettingsComponentSummary>,
}

// ── Monty VM settings ─────────────────────────────────────────────────────────

/// The Monty VM runtime settings, backed by `reborn_monty_vm_settings`.
/// All fields are immediate-write except `active_orchestrator_id`
/// (gated: only `Validated` orchestrators are accepted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontyVmSettings {
    /// Max duration for a single Monty VM turn in seconds.
    pub max_duration_secs: u64,
    /// Max memory allocation count before the VM is killed.
    pub max_allocations: Option<u64>,
    /// Max resident memory in bytes before the VM is killed.
    pub max_memory_bytes: Option<u64>,
    /// Number of consecutive failures before the VM is auto-rolled back.
    pub failure_rollback_threshold: u32,
    /// Token budget for prior-knowledge injection into each prompt.
    pub prior_knowledge_token_budget: u32,
    /// Retention window for Q4 (rejected, attempts >= 3) components in days.
    pub q4_retention_days: u32,
    /// Retention window for forensic packets in days.
    pub forensic_packet_retention_days: u32,
    /// ID of the currently active orchestrator component (`Validated`).
    pub active_orchestrator_id: Option<String>,
}

/// Request body for `PUT /api/settings/monty-vm`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMontyVmSettingsRequest {
    pub max_duration_secs: Option<u64>,
    pub max_allocations: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub failure_rollback_threshold: Option<u32>,
    pub prior_knowledge_token_budget: Option<u32>,
    pub q4_retention_days: Option<u32>,
    pub forensic_packet_retention_days: Option<u32>,
    /// Only `Validated` orchestrators are accepted; others return 400.
    pub active_orchestrator_id: Option<String>,
}

/// Response for both GET and PUT of Monty VM settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontyVmSettingsResponse {
    pub settings: MontyVmSettings,
}

// ── Monty VM restart ──────────────────────────────────────────────────────────

/// Request body for `POST /api/settings/monty-vm/restart`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MontyVmRestartRequest {
    /// When `true`, abort any in-flight turns before restarting.
    #[serde(default)]
    pub force: bool,
}

/// Response for `POST /api/settings/monty-vm/restart`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontyVmRestartResponse {
    /// New VM state immediately after issuing the restart.
    pub state: MontyVmState,
}

// ── Monty VM status ───────────────────────────────────────────────────────────

/// Live Monty VM state values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MontyVmState {
    Running,
    Draining,
    Restarting,
    Stopped,
    Error,
}

/// Response for `GET /api/settings/monty-vm/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontyVmStatusResponse {
    pub state: MontyVmState,
    /// Version of the currently active orchestrator (e.g. `"1.2.3"`).
    pub orchestrator_version: Option<String>,
    /// SHA-256 hash (hex) of the settings row as applied to the running
    /// instance — lets the operator detect drift vs the DB values.
    pub settings_hash: Option<String>,
}

// ── Chat preference ───────────────────────────────────────────────────────────

/// Request body for `PUT /api/chat/preferences/{key}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChatPreferenceRequest {
    pub value: serde_json::Value,
}

/// Response for `PUT /api/chat/preferences/{key}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChatPreferenceResponse {
    pub key: String,
    pub value: serde_json::Value,
}

// ── MontyVmSettingsStore ─────────────────────────────────────────────────────

/// Storage error for Monty VM settings operations.
#[derive(Debug, thiserror::Error)]
pub enum MontyVmSettingsError {
    #[error("store unavailable: {0}")]
    Unavailable(String),
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Persistence port for `reborn_monty_vm_settings`.
///
/// Backed by `PgMontyVmSettingsStore` in Postgres builds. In DB-less mode
/// the `default_monty_vm_settings()` function provides compiled-in defaults.
#[async_trait]
pub trait MontyVmSettingsStore: Send + Sync {
    /// Load settings for `(user_id, project_id)`.
    /// Returns compiled-in defaults when no DB row exists (first-run).
    async fn get(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<MontyVmSettings, MontyVmSettingsError>;

    /// Upsert settings for `(user_id, project_id)`.
    /// Returns the full updated settings row.
    async fn upsert(
        &self,
        user_id: &str,
        project_id: &str,
        update: &UpdateMontyVmSettingsRequest,
    ) -> Result<MontyVmSettings, MontyVmSettingsError>;
}

/// Compiled-in defaults, used when no DB row exists or in DB-less mode.
pub fn default_monty_vm_settings() -> MontyVmSettings {
    MontyVmSettings {
        max_duration_secs: 300,
        max_allocations: Some(5_000_000),
        max_memory_bytes: Some(128 * 1024 * 1024),
        failure_rollback_threshold: 3,
        prior_knowledge_token_budget: 100_000,
        q4_retention_days: 30,
        forensic_packet_retention_days: 90,
        active_orchestrator_id: None,
    }
}
