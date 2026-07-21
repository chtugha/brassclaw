//! Recipe-Skill-Tool facade DTOs and persistence port.
//!
//! These types are the wire-stable shape the WebUI v2 learns against — the
//! engine-side `brassclaw_engine::types::recipe::Recipe` / `ToolSkill`
//! structs are the source of truth on disk, but they round-trip through
//! `serde_json::Value` to avoid a hard product-workflow → engine-types
//! dependency that would force every product workflow file to recompile
//! when an engine field lands. Composition hands the JSON over a thin
//! `RecipeStore` trait, mapping engine errors to a stable wire taxonomy.
//!
//! Three categories of surface:
//!
//! - **Read DTOs** (`RecipeSummary`, `ToolSkillSummary`, full `Recipe` /
//!   `ToolSkill` JSON wrappers) — the WebUI inventory and Recipe Manager
//!   pages render from these.
//! - **Validation queue DTOs** (`ValidationQueueItem`,
//!   `ValidationActionResponse`) — the post-extraction review tab.
//! - **Persistence trait** (`RecipeStore`) + `RecipeStoreError` — the
//!   in-process port composition implements in
//!   `brassclaw_reborn_composition::recipe_store`.

#![forbid(unsafe_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::ProductWorkflowError;

/// What kind of Recipe/ToolSkill row a `ValidationQueueItem` represents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeKind {
    Recipe,
    ToolSkill,
}

/// 4-queue filter for `GET /api/webchat/v2/validation-queue?q=…`.
///
/// Maps to the four lifecycle queues defined in spec §3.5.1.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationQueueFilter {
    /// Q1 — auto-validation failures (Pending + AutoFailed).
    Auto,
    /// Q2 — waiting for manual operator validation (AutoPassed + ReviewRequested + UpgradeQueued).
    #[default]
    Manual,
    /// Q3 — rejected, review_attempts < 3 (awaiting revision mission).
    Revision,
    /// Q4 — rejected, review_attempts >= 3 (pending wipe).
    Rejection,
}

/// Lifecycle status mirrored from
/// `brassclaw_engine::types::recipe::ValidationStatus`. We deliberately
/// re-export strings (not strong enum variants) — the WebUI surfaces
/// `validation_status` as a free-text label today, and keeping the wire
/// type as `String` lets the engine evolve the enum without breaking
/// the WebUI binding.
pub type ValidationStatusValue = String;

/// Lightweight summary card used by `GET /api/webchat/v2/recipes` and
/// the Recipe Manager list tab. Carries everything the operator needs
/// to triage a row without loading the full doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub trigger: serde_json::Value,
    pub step_count: u32,
    pub usage_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub wilson_lower: f64,
    pub tier: String,
    pub tier0_eligible: bool,
    pub validation_status: ValidationStatusValue,
    pub validation_errors: Vec<String>,
    pub review_attempts: u32,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight summary card for ToolSkill rows. Mirrors `RecipeSummary`
/// minus the trigger / step fields that don't apply to skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSkillSummary {
    pub id: String,
    pub name: String,
    pub tool_name: String,
    pub description: String,
    pub category: String,
    pub estimated_tokens: u32,
    pub usage_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub wilson_lower: f64,
    pub tier: String,
    pub validation_status: ValidationStatusValue,
    pub validation_errors: Vec<String>,
    pub review_attempts: u32,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Result shape for `GET /api/webchat/v2/recipes` and
/// `GET /api/webchat/v2/tool-skills`. The WebUI expects `recipes` /
/// `tool_skills` keys — names match the existing stub handlers so the
/// WebUI's existing fetch paths stay unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeListResponse {
    pub recipes: Vec<RecipeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSkillListResponse {
    pub tool_skills: Vec<ToolSkillSummary>,
}

/// Full `Recipe` payload for `GET /api/webchat/v2/recipes/{id}`. Returns
/// the full engine JSON (steps, validation hook, lifecycle fields) as
/// opaque JSON so the WebUI can render the Recipe detail pane without
/// product-workflow recompiles when the engine adds fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeDetail {
    pub id: String,
    pub recipe: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSkillDetail {
    pub id: String,
    pub tool_skill: serde_json::Value,
}

/// One row in the validation queue — surfaced by
/// `GET /api/webchat/v2/validation-queue` (the post-extraction review
/// tab) and the per-kind counters.
///
/// Extended in Phase 3 (Step 3.5) to carry `class_code`, `class_label`,
/// `queue_code`, `validator_tag_present`, `consumer_tags`,
/// `llm_audit_status`, and `llm_audit_findings` for the 4-queue UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationQueueItem {
    pub id: String,
    pub name: String,
    pub item_type: RecipeKind,
    pub category: String,
    pub description: String,
    pub trigger_summary: String,
    pub estimated_tokens: Option<u32>,
    pub validation_status: ValidationStatusValue,
    pub validation_errors: Vec<String>,
    pub review_feedback: Option<String>,
    pub review_attempts: u32,
    pub similarity_parent_id: Option<String>,
    pub created_at: String,
    pub source: String,
    /// Integer class code from spec §4 (e.g. 0 = Tool, 1 = Skill/Rusty, 21 = Recipe).
    pub class_code: u16,
    /// Human-readable label derived from `class_code`.
    pub class_label: String,
    /// Derived queue bucket: "q1_auto" | "q2_manual" | "q3_revision" | "q4_rejection".
    pub queue_code: String,
    /// True when the `05:validator` consumer tag is present — greys out delivery.
    pub validator_tag_present: bool,
    /// All consumer tags on this component row (e.g. `["01:monty", "05:validator"]`).
    pub consumer_tags: Vec<String>,
    /// LLM code-audit status for Orchestrator (class 10) and Scaffold (class 50).
    /// `"pending"` | `"clean"` | `"flagged"` | `"not_applicable"`.
    pub llm_audit_status: String,
    /// LLM code-audit findings when `llm_audit_status == "flagged"`.
    pub llm_audit_findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationQueueListResponse {
    pub items: Vec<ValidationQueueItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationQueueCountResponse {
    pub count: u32,
    pub status: ValidationStatusValue,
}

/// Body for PUT validation endpoints. The `feedback` field is optional —
/// it's required for `request_review` (rejection-of-review), and ignored
/// for plain `validate` / `reject`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateValidationStatusRequest {
    pub feedback: Option<String>,
}

/// Response for `PUT/validate`, `PUT/reject`, `PUT/review-request`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateValidationStatusResponse {
    pub id: String,
    pub item_type: RecipeKind,
    pub previous_status: ValidationStatusValue,
    pub new_status: ValidationStatusValue,
    pub review_attempts: u32,
}

/// Outcome-recording DTO — the agent loop's Tier 0/1/2 outcome path
/// funnels through [`RecipeStore::record_outcome`] (one shared call
/// for both kinds). The metric persistence lives in the engine
/// (`MetricRecorder`); the composition layer is a thin
/// sync→async bridge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    Recipe,
    ToolSkill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordOutcomeRequest {
    pub id: String,
    pub kind: OutcomeKind,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordOutcomeResponse {
    pub id: String,
    pub kind: OutcomeKind,
    pub recorded: bool,
}

/// Persistence port used by [`crate::RebornServicesApi`]. Composition
/// wires this in from `brassclaw_reborn_composition::recipe_store`;
/// the WebUI's facade impl forwards each call here.
///
/// All read-side methods return `serde_json::Value` for the full
/// payload so the engine schema can evolve without a product-workflow
/// release. Summary fields are projected into typed
/// `RecipeSummary` / `ToolSkillSummary` for the WebUI list tab.
#[async_trait]
pub trait RecipeStore: Send + Sync {
    /// List recipes owned by `(user_id, project_id)`, sorted by `updated_at`
    /// descending so freshly-promoted rows surface first.
    async fn list_recipes(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<RecipeSummary>, RecipeStoreError>;

    /// List tool skills owned by `(user_id, project_id)`, sorted same way.
    async fn list_tool_skills(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ToolSkillSummary>, RecipeStoreError>;

    /// Fetch the full recipe payload (steps, validation hook, lifecycle).
    /// Returns `None` if the id is unknown — the WebUI renders this as a
    /// 404 row.
    async fn get_recipe(
        &self,
        user_id: &str,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<Option<RecipeDetail>, RecipeStoreError>;

    /// Fetch the full tool skill payload.
    async fn get_tool_skill(
        &self,
        user_id: &str,
        project_id: &str,
        skill_id: &str,
    ) -> Result<Option<ToolSkillDetail>, RecipeStoreError>;

    /// List rows in the validation queue filtered by the given queue bucket.
    /// Sorted by `created_at` ascending so the operator reviews oldest-first.
    async fn list_validation_queue(
        &self,
        user_id: &str,
        project_id: &str,
        filter: ValidationQueueFilter,
    ) -> Result<Vec<ValidationQueueItem>, RecipeStoreError>;

    /// Count rows by validation status. Used for tab badges.
    async fn count_by_status(
        &self,
        user_id: &str,
        project_id: &str,
        status: &str,
    ) -> Result<u32, RecipeStoreError>;

    /// Promote or reject a Recipe — `new_status` is one of `validated`,
    /// `rejected`, or `review_requested`. Implementations update the
    /// `MemoryDoc` metadata (validation_status + review_feedback +
    /// review_attempts) and persist.
    async fn update_recipe_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        recipe_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<UpdateValidationStatusResponse, RecipeStoreError>;

    /// Same, but for a ToolSkill row.
    async fn update_skill_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        skill_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<UpdateValidationStatusResponse, RecipeStoreError>;

    /// Generalized component status update for any class code.
    ///
    /// `class_code` is used to identify the DocType/component table.
    /// Routes through `update_recipe_validation_status` or
    /// `update_skill_validation_status` for known legacy classes; other
    /// class codes are dispatched to the DB-backed component store once
    /// available.
    async fn update_component_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<UpdateValidationStatusResponse, RecipeStoreError>;

    /// Wipe a component row (Q4 terminal delete). Removes the MemoryDoc
    /// and its creation-process provenance fields but never deletes
    /// thread messages, steps, or events.
    async fn delete_component(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
    ) -> Result<(), RecipeStoreError>;

    /// Return the LLM code-audit status for Orchestrator (10) / Scaffold (50)
    /// components. For other class codes returns `"not_applicable"`.
    async fn get_component_audit_status(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
    ) -> Result<ComponentAuditStatus, RecipeStoreError>;

    /// Record a success/failure outcome — wires through to the engine's
    /// `MetricRecorder` so Wilson/tier counters update atomically.
    async fn record_outcome(
        &self,
        user_id: &str,
        project_id: &str,
        request: RecordOutcomeRequest,
    ) -> Result<RecordOutcomeResponse, RecipeStoreError>;
}

/// LLM code-audit status returned by `RecipeStore::get_component_audit_status`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ComponentAuditStatus {
    /// `"pending"` | `"clean"` | `"flagged"` | `"not_applicable"`.
    pub status: String,
    pub findings: Vec<String>,
}

impl ComponentAuditStatus {
    pub fn not_applicable() -> Self {
        Self {
            status: "not_applicable".to_string(),
            findings: vec![],
        }
    }
}

/// Storage-side error taxonomy. Mirrors [`crate::reduction_rules::ReductionRuleStoreError`]:
/// the WebUI surface maps these to HTTP status codes.
#[derive(Debug, thiserror::Error)]
pub enum RecipeStoreError {
    #[error("invalid recipe/skill payload: {0}")]
    Invalid(String),
    #[error("recipe/skill not found: {0}")]
    NotFound(String),
    #[error("recipe/skill store unavailable: {0}")]
    Unavailable(String),
    #[error("recipe/skill store internal error: {0}")]
    Internal(String),
}

impl From<RecipeStoreError> for ProductWorkflowError {
    fn from(error: RecipeStoreError) -> Self {
        match error {
            RecipeStoreError::Invalid(reason) => {
                ProductWorkflowError::InvalidBindingRequest { reason }
            }
            RecipeStoreError::NotFound(reason) => {
                ProductWorkflowError::InvalidBindingRequest { reason }
            }
            RecipeStoreError::Unavailable(reason) => ProductWorkflowError::Transient { reason },
            RecipeStoreError::Internal(reason) => ProductWorkflowError::BeforeInboundPolicyFailed {
                reason,
                permanent: false,
            },
        }
    }
}

/// Configuration knobs the WebUI uses to filter list endpoints. Kept on
/// the wire shape (not the trait) so future filters don't force a trait
/// revision — the trait takes `Option<&RecipeListRequest>` style filters
/// through this struct.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecipeListRequest {
    pub status_filter: Option<String>,
    pub category_filter: Option<String>,
    pub max_results: Option<u32>,
}
