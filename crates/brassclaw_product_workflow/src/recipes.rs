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
    Component,
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
    /// Q3 — state-3 rejection below the current owner threshold (awaiting revision).
    Revision,
    /// Q4 — persisted deletion candidates (state 4) plus state-3 rejections
    /// at/above the current owner threshold. A higher threshold never revives state 4.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OriginalReviewBindingState {
    Bound,
    Unresolved,
    LegacyUnresolved,
    MissingQueue,
    JointUnsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalReviewBindingView {
    pub state: OriginalReviewBindingState,
    pub queue_id: Option<String>,
    pub submission_id: Option<String>,
}

/// One row in the validation queue — surfaced by
/// `GET /api/webchat/v2/validation-queue` (the post-extraction review
/// tab) and the per-kind counters.
///
/// Extended in Phase 3 (Step 3.5) to carry `class_code`, `class_label`,
/// `queue_code`, `validator_tag_present`, `consumer_tags`,
/// `llm_audit_status`, and `llm_audit_findings` for the 4-queue UI.
/// Phase P.0: extended with `q2_actor` audit field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationQueueItem {
    /// Original-candidate diagnosis, separate from queue lifecycle and Work status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_review: Option<OriginalReviewBindingView>,
    /// Stable list ordering time: queue submission, or creation without a queue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_at: Option<String>,
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
    /// Derived queue label: "q1_auto" | "q2_manual" | "q3_revision" |
    /// "q4_rejection" (state 3 at/above threshold) | "garbage" (persisted state 4).
    pub queue_code: String,
    /// True when the `05:validator` workflow tag is present.
    pub validator_tag_present: bool,
    /// All consumer tags on this component row (e.g. `["01:monty", "05:validator"]`).
    pub consumer_tags: Vec<String>,
    /// LLM code-audit status for Orchestrator (class 10) and Scaffold (class 50).
    /// `"pending"` | `"clean"` | `"flagged"` | `"not_applicable"`.
    pub llm_audit_status: String,
    /// LLM code-audit findings when `llm_audit_status == "flagged"`.
    pub llm_audit_findings: Vec<String>,
    /// Phase P.0: who performed the Q2 graduation. `None` = not yet approved.
    /// `Some("human")` = operator via WebUI. `Some("builtin")` = bootstrap seeder
    /// (builtin-exempt audit label only — not a generic automated graduation path).
    pub q2_actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationQueueListResponse {
    pub items: Vec<ValidationQueueItem>,
    /// Existing list bound is a visible page size, never a silent total ceiling.
    #[serde(default)]
    pub page_size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
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

/// Exact evidence selection for one authored Skill usage. No client reports,
/// source documents or success flags are accepted by the human-review API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssociationReviewSelection {
    pub q1_ref: String,
    pub behavioral_refs: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AssociationReviewView {
    pub review_checksum: String,
    pub review: serde_json::Value,
}

/// Explicit human decision on the displayed exact view. Reuse approval_id and
/// identical fields after a transport failure; a retry cannot replace bytes.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApproveAssociationRequest {
    pub approval_id: String,
    pub selection: AssociationReviewSelection,
    pub review_checksum: String,
    pub semantic_review: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociationApprovalResponse {
    pub approval_id: String,
    pub q2_ref: String,
    pub review_checksum: String,
    pub catalogue_activated: bool,
}

/// Exact selection, never an approval or request to look up latest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentRevisionSelection {
    pub uuid: String,
    pub class_code: i32,
    pub version: u64,
    pub checksum: String,
}

/// Stable ID and identical fields are required after an unknown commit.
/// Candidate bytes remain data, including source, schemas and associations.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitComponentReviewRequest {
    /// Explicit new queue captures. Empty retains the standalone immutable path.
    /// IDs are selectors only; the host verifies the complete candidate and scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queue_ids: Vec<String>,
    /// Create the supported legacy projection in this explicitly selected project.
    /// Tenant/actor/agent remain host-derived; candidate bytes remain original.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_queue_project: Option<String>,
    pub submission_id: String,
    pub candidate_bytes: String,
    /// Observed allocation head, independent of the exact lineage base. Missing
    /// retains historical adjacent-version authoring for compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_previous_version: Option<u64>,
    pub base: Option<ComponentRevisionSelection>,
    pub dependencies: Vec<ComponentRevisionSelection>,
    /// Optional Skill/implementing-code partner in the same function review.
    /// Context is retained exactly, never approved again by this submission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_partner: Option<ComponentFunctionReviewPartner>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "role", deny_unknown_fields)]
pub enum ComponentFunctionReviewPartner {
    #[serde(rename = "changed")]
    Changed {
        candidate_bytes: String,
        base: Option<ComponentRevisionSelection>,
        expected_previous_version: u64,
    },
    #[serde(rename = "context")]
    Context {
        reference: ComponentRevisionSelection,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentFunctionReviewPartnerReceipt {
    pub reference: ComponentRevisionSelection,
    pub changed: bool,
    pub base: Option<ComponentRevisionSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentRevisionHeadView {
    pub component_id: String,
    pub class_code: Option<i32>,
    pub last_allocated_version: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReviewQueueBindingStatus {
    #[default]
    Standalone,
    Bound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentReviewSubmissionReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation_revision: Option<u64>,
    /// Original immutable queue selectors, also readable after Q2 removes rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queue_ids: Vec<String>,
    /// Standalone means an original immutable subject, without a legacy queue seal.
    /// Neither state establishes Work admission or validator readiness.
    #[serde(default)]
    pub queue_binding_status: ReviewQueueBindingStatus,
    pub submission_id: String,
    pub subject_checksum: String,
    pub candidate: ComponentRevisionSelection,
    pub base: Option<ComponentRevisionSelection>,
    pub dependencies: Vec<ComponentRevisionSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_partner: Option<ComponentFunctionReviewPartnerReceipt>,
    pub review_status: String,
    pub catalogue_activated: bool,
}

/// One original Pure invocation request. Data only: no code, expected outcome,
/// approval, Tool permission or live task/context selector.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitOriginalPureCaseRequest {
    pub case_id: String,
    pub submission_id: String,
    pub export: String,
    pub inputs: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OriginalPureCaseStatus {
    AwaitingPreparation,
    Ready,
    Running,
    ReconciliationRequired,
    Cancelled,
    ObservationPendingDelivery,
    TechnicalFailure,
    Settled,
}
/// Original custody metadata; observation data is exposed only after genuine
/// private harness delivery and case-owner settlement, never as a pass verdict.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalPureCaseView {
    pub case_id: String,
    pub submission_id: String,
    pub case_checksum: String,
    pub status: OriginalPureCaseStatus,
    pub observation_bytes: Option<String>,
    pub settlement_checksum: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ComponentReviewSubmissionView {
    pub receipt: ComponentReviewSubmissionReceipt,
    pub subject_bytes: String,
    pub component_bytes: Vec<String>,
    /// Original preparation metadata only; not Q1, approval or task completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_selection_bytes: Option<String>,
    /// Original custody only. Structural completion does not qualify full Q1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<ReviewWorkView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewWorkStatus {
    OriginalQueueChanged,
    OriginalPreparationMissing,
    LegacyCustodyUnresolved,
    JointSelectionUnsupported,
    AwaitingPreparation,
    Ready,
    Running,
    ReconciliationRequired,
    Cancelled,
    NativeTerminalUnacknowledged,
    FullQ1QualificationMissing,
    StructuralObservationsConfirmed,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ReviewWorkView {
    pub status: ReviewWorkStatus,
    pub work_id: Option<String>,
    pub run_id: Option<String>,
    pub delivery_checksum: Option<String>,
    pub confirmation_checksum: Option<String>,
    pub selection_preparation_present: bool,
    pub q1_qualified: bool,
}

/// Bounded, cursor-paginated operator projection of internal Sempai attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostTurnReviewSummary {
    pub source_run_id: String,
    pub attempt_id: String,
    pub phase: String,
    pub model_dispatch_count: i32,
    pub settlement_recorded: bool,
    pub reconciliation_recorded: bool,
    pub disposition_count: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostTurnReviewList {
    pub items: Vec<PostTurnReviewSummary>,
    pub next_after: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PostTurnReviewInspection {
    pub evidence_bytes: String,
    pub evidence_checksum: String,
    pub dispositions: Vec<ReviewDispositionView>,
    pub disposition_count: i64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ReviewDispositionView {
    pub receipt: ReviewDispositionReceipt,
    pub actor: String,
    pub note: String,
}
/// An immutable operator observation, never permission to retry or release state.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDispositionRequest {
    pub disposition_id: String,
    pub evidence_checksum: String,
    pub note: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDispositionReceipt {
    pub disposition_id: String,
    pub attempt_id: String,
    pub evidence_checksum: String,
    pub retention_released: bool,
    pub work_replayed: bool,
}

/// Native reconciliation accepts no effect outcomes or accounting amounts
/// from the caller. Exact-ID retries read the immutable native proof.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewReconciliationRequest {
    pub evidence_checksum: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ReviewReconciliationReceipt {
    pub attempt_id: String,
    pub retention_released: bool,
    pub reconciliation_recorded: bool,
    pub work_replayed: bool,
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

/// Existing catalogue publisher's durable target and process-local uptake.
/// These are projections, never approval or generated-prefix activation.
#[derive(Clone, Serialize, Deserialize)]
pub struct CataloguePublicationStatus {
    pub desired_generation: String,
    pub effective_generation: String,
    pub discovery_effective_generation: Option<String>,
    pub pending: bool,
    pub prefix_invalidation_id: Option<String>,
    pub invalidated_profiles: Vec<String>,
    /// The generated-prefix consumer has not acknowledged these sources.
    /// A qualified retained inline context can independently remain usable.
    pub prefix_rebuild_pending: bool,
    pub inline_prefix_ready: bool,
    pub inline_prefix_reference: Option<String>,
    pub model_context_support: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CatalogueActivationStatus {
    /// Optional private single-case support, not ordinary catalogue activation
    /// or behavior/Q1/Q2 approval. Raw internal causes are never serialized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_pure_case: Option<OriginalPureCaseReadiness>,
    pub publication: CataloguePublicationStatus,
    pub generation: String,
    pub recipes: Vec<String>,
    /// Exact complete authored selections, suitable for a generation-CAS edit.
    pub selections: Vec<CatalogueRecipeSelection>,
    /// Stable routing disables, including temporarily withdrawn Recipes.
    pub disabled_recipe_ids: Vec<String>,
    /// Packaged ordinary matching Recipes; internal reply/history remain usable.
    pub packaged_recipe_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginalPureCaseReadiness {
    pub ready: bool,
    pub evidence_pending: bool,
    pub phase: Option<String>,
    pub reason_kind: Option<String>,
}
/// Complete retained source projection for one active ordinary-workflow generation.
/// Root retention and checked usage selections are distinct; this export is not
/// activation authority or proof of full-library coverage/cache qualification.
#[derive(Clone, Serialize, Deserialize)]
pub struct CatalogueSourceExport {
    pub format: String,
    pub generation: String,
    pub coverage: String,
    pub catalogue_bytes: String,
    pub catalogue_checksum: String,
    pub root: ComponentRevisionSelection,
    pub components: Vec<CatalogueSourceComponent>,
    /// Exact retained selection bytes include variant/input/export/approval pins.
    pub selections: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CatalogueSourceComponent {
    pub reference: ComponentRevisionSelection,
    pub revision_bytes: String,
}
/// Operator-selected coherent catalogue replacement. References select existing
/// immutable artifacts; this request cannot manufacture approval evidence.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivateCatalogueRequest {
    pub activation_id: String,
    pub expected_generation: String,
    pub recipes: Vec<CatalogueRecipeSelection>,
    /// Optional immutable proposal provenance; validated again on publication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployment_subjects: Vec<DeploymentSubjectPin>,
    /// None preserves existing operator routing choices; Some replaces them.
    /// These choices affect matching/discovery, never Tool policy or approvals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_recipe_ids: Option<Vec<String>>,
}
/// Preparation selects retained proposal bytes, never accepts approval evidence.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentSubjectPin {
    pub submission_id: String,
    pub subject_checksum: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeDeploymentSubject {
    pub subject: DeploymentSubjectPin,
    pub workflow_class: CatalogueWorkflowClass,
    pub association_approvals: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareComponentDeploymentRequest {
    pub activation_id: String,
    pub expected_generation: String,
    pub recipes: Vec<RecipeDeploymentSubject>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ComponentDeploymentPlan {
    pub activation: ActivateCatalogueRequest,
    pub preserved_recipe_ids: Vec<String>,
    pub replaced_recipe_ids: Vec<String>,
    pub catalogue_activated: bool,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogueWorkflowClass {
    Deterministic,
    RequiresModel,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogueRecipeSelection {
    pub recipe_id: String,
    pub workflow_class: CatalogueWorkflowClass,
    pub components: Vec<ComponentRevisionSelection>,
    /// Existing exact usage approvals, keyed by stable Skill UUID.
    pub association_approvals: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CatalogueActivationReceipt {
    pub activation_id: String,
    pub generation: String,
    pub catalogue_activated: bool,
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
    async fn prepare_component_deployment(
        &self,
        _request: PrepareComponentDeploymentRequest,
        _preparation: Option<brassclaw_host_api::task_preparation::RetainedPreparation>,
    ) -> Result<ComponentDeploymentPlan, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "component deployment unavailable".into(),
        ))
    }

    async fn export_catalogue_sources(
        &self,
        _generation: &str,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "catalogue source export unavailable".into(),
        ))
    }

    async fn catalogue_status(&self) -> Result<CatalogueActivationStatus, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "authored catalogue activation unavailable".into(),
        ))
    }

    /// Reconcile an already committed activation using its original identity.
    /// This cannot author, approve or activate a new catalogue.
    async fn reconcile_catalogue(
        &self,
        _activation_id: &str,
    ) -> Result<CatalogueActivationReceipt, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "catalogue reconciliation unavailable".into(),
        ))
    }

    /// Host ingress preparation only: returns the existing owner's acknowledged
    /// capture, never caller JSON, desired settings, admission or Tool authority.
    async fn capture_component_preparation(
        &self,
    ) -> Result<Option<brassclaw_host_api::task_preparation::RetainedPreparation>, RecipeStoreError>
    {
        Err(RecipeStoreError::Unavailable(
            "acknowledged component preparation unavailable".into(),
        ))
    }

    async fn activate_catalogue(
        &self,
        _actor: &str,
        _request: ActivateCatalogueRequest,
        _preparation: Option<brassclaw_host_api::task_preparation::RetainedPreparation>,
    ) -> Result<CatalogueActivationReceipt, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "authored catalogue activation unavailable".into(),
        ))
    }

    async fn list_post_turn_reviews(
        &self,
        _after: Option<&str>,
    ) -> Result<PostTurnReviewList, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "post-turn review inspection unavailable".into(),
        ))
    }
    async fn inspect_post_turn_review(
        &self,
        _attempt: &str,
    ) -> Result<PostTurnReviewInspection, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "post-turn review inspection unavailable".into(),
        ))
    }
    async fn reconcile_post_turn_review(
        &self,
        _attempt: &str,
        _request: ReviewReconciliationRequest,
    ) -> Result<ReviewReconciliationReceipt, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "native review reconciliation unavailable".into(),
        ))
    }

    async fn record_review_disposition(
        &self,
        _actor: &str,
        _attempt: &str,
        _request: ReviewDispositionRequest,
    ) -> Result<ReviewDispositionReceipt, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "post-turn review inspection unavailable".into(),
        ))
    }

    async fn submit_component_review(
        &self,
        _actor: &str,
        _request: SubmitComponentReviewRequest,
        _preparation: Option<brassclaw_host_api::task_preparation::RetainedPreparation>,
    ) -> Result<ComponentReviewSubmissionReceipt, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "immutable review submissions are unavailable".into(),
        ))
    }

    async fn submit_original_pure_case(
        &self,
        _request: SubmitOriginalPureCaseRequest,
        _preparation: Option<brassclaw_host_api::task_preparation::RetainedPreparation>,
    ) -> Result<OriginalPureCaseView, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "original Pure case owner unavailable".into(),
        ))
    }

    async fn get_original_pure_case(
        &self,
        _case_id: &str,
    ) -> Result<OriginalPureCaseView, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "original Pure case owner unavailable".into(),
        ))
    }

    async fn get_component_revision_head(
        &self,
        _component_id: &str,
    ) -> Result<ComponentRevisionHeadView, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "immutable allocation metadata unavailable".into(),
        ))
    }

    async fn get_component_review_submission(
        &self,
        _submission_id: &str,
    ) -> Result<ComponentReviewSubmissionView, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "immutable review submissions are unavailable".into(),
        ))
    }

    async fn prepare_association_review(
        &self,
        _skill_id: &str,
        _selection: AssociationReviewSelection,
    ) -> Result<AssociationReviewView, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "exact association review is unavailable".into(),
        ))
    }

    /// Actor is supplied by the authenticated human ingress, never the body.
    async fn approve_association(
        &self,
        _actor: &str,
        _skill_id: &str,
        _request: ApproveAssociationRequest,
    ) -> Result<AssociationApprovalResponse, RecipeStoreError> {
        Err(RecipeStoreError::Unavailable(
            "exact association review is unavailable".into(),
        ))
    }

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
    /// Live keyset pages across all classes, oldest queue/creation time first.
    /// Follow next_cursor; concurrent lifecycle changes may change membership.
    async fn list_validation_queue(
        &self,
        user_id: &str,
        project_id: &str,
        filter: ValidationQueueFilter,
        after: Option<&str>,
    ) -> Result<ValidationQueueListResponse, RecipeStoreError>;

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

    /// Manual re-review metadata. The PostgreSQL Recipe implementation requires
    /// `rejected` and atomically sets `pending` plus the validator tag, without a
    /// Q4-only condition. Invalid state/class returns `RecipeStoreError::Invalid`;
    /// an absent scoped row returns `NotFound`. This action does not reset a queue,
    /// establish Q1/Q2 approval or activate the component.
    async fn re_review_component(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
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
    // Phase N: auto_validate_pending removed — the legacy sweep used the
    // now-dropped queue_code column. Q1 is driven per-component via
    // ValidationQueueStore + run_q1_validation.
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
    #[error("component authoring conflict: {0}")]
    Conflict(String),
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
            RecipeStoreError::NotFound(reason) | RecipeStoreError::Conflict(reason) => {
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
