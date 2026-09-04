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
    /// Global kill switch for all token budgets (§0.21 / Phase O).
    /// When `false`, every token-budget check in the system is bypassed —
    /// the VM runs as if every cap is `usize::MAX`.  Time and USD limits
    /// remain enforced regardless.  Defaults to `true`.
    #[serde(default = "default_true")]
    pub token_budgets_enabled: bool,
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
    /// Toggle the global token-budget kill switch (§0.21).
    pub token_budgets_enabled: Option<bool>,
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

// ── ChatPreferenceStore ──────────────────────────────────────────────────────

/// Persistence port for per-user chat preferences (`reborn_user_preferences`).
#[async_trait]
pub trait ChatPreferenceStore: Send + Sync {
    /// Persist a chat preference for `user_id`.
    /// Returns `(key, stored_value)` on success, or an error if the key is
    /// not allowed or the store is unavailable.
    async fn upsert(
        &self,
        user_id: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
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

fn default_true() -> bool {
    true
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
        token_budgets_enabled: true,
    }
}

// ── SecuritySettingsStore ────────────────────────────────────────────────────

/// Storage error for operator-level security-settings operations.
#[derive(Debug, thiserror::Error)]
pub enum SecuritySettingsError {
    #[error("store unavailable: {0}")]
    Unavailable(String),
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Persistence port for `reborn_security_settings` (V068).
///
/// Operator-level and tenant-scoped: the tenant is captured at construction by
/// the backing `PgSecuritySettingsStore` in composition, so the trait carries no
/// tenant argument. A missing row yields
/// [`brassclaw_turns::run_profile::SecurityModeConfig::default`] (all `Auto`).
/// The C.6 cross-turn driver consumes the same row through the turns-layer
/// [`brassclaw_turns::run_profile::SecurityConfigSource`] port; this trait is
/// the WebUI CRUD surface over it.
#[async_trait]
pub trait SecuritySettingsStore: Send + Sync {
    /// Load the operator-level security config for the store's tenant.
    async fn get(
        &self,
    ) -> Result<brassclaw_turns::run_profile::SecurityModeConfig, SecuritySettingsError>;

    /// Upsert the operator-level security config for the store's tenant.
    /// Returns the full updated config (re-read after write).
    async fn upsert(
        &self,
        config: &brassclaw_turns::run_profile::SecurityModeConfig,
    ) -> Result<brassclaw_turns::run_profile::SecurityModeConfig, SecuritySettingsError>;
}

// ── IntentInputsStore ─────────────────────────────────────────────────────────

/// A single row from `reborn_intent_inputs`, returned by the Settings UI API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentInputRow {
    pub id: String,
    pub input_text: String,
    /// Class code: 1=Word, 2=Partial, 3=Sentence, 4=KeywordFallback.
    pub input_class: i16,
    pub component_id: String,
    pub component_class_code: i16,
    pub score: i32,
    pub source: String,
    pub needs_review: bool,
}

/// Response body for `GET /api/settings/intent-inputs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentInputListResponse {
    pub items: Vec<IntentInputRow>,
}

/// Request body for `PUT /api/settings/intent-inputs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertIntentInputRequest {
    pub project_id: String,
    pub component_id: String,
    pub component_class_code: u16,
    pub input_text: String,
    /// Class code: 1=Word, 2=Partial, 3=Sentence.
    pub input_class: i16,
}

/// Storage port for `reborn_intent_inputs`.
///
/// Backed by `PgIntentInputsStore` when Postgres + skills-db are active.
/// Default trait methods return 501 so DB-less builds fail safe.
#[async_trait]
pub trait IntentInputsStore: Send + Sync {
    /// List intent inputs for a scope, optionally filtered by component.
    async fn list(
        &self,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        component_id: Option<&str>,
    ) -> Result<Vec<IntentInputRow>, Box<dyn std::error::Error + Send + Sync>>;

    /// Upsert an intent input (idempotent by unique index).
    async fn upsert(
        &self,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        req: &UpsertIntentInputRequest,
    ) -> Result<IntentInputRow, Box<dyn std::error::Error + Send + Sync>>;

    /// Delete all intent inputs for a specific component.
    async fn purge_for_component(
        &self,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        component_id: &str,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>;
}
