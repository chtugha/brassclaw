//!
//! Reduction-rule endpoints bound to [`brassclaw_product_workflow::RebornServices`].
//!
//! These handlers back the soft-budget pipeline in
//! `crates/brassclaw_engine/orchestrator/`. The orchestrator Python reads the
//! rules every time the assembled prompt overflows `prompt_budget_tokens`;
//! the WebUI exposes CRUD plus an authoring endpoint so operators can tune
//! the pipeline without restarting the runtime.
//!
//! Three routes, all project-scoped via the caller's authenticated
//! `WebUiAuthenticatedCaller.project_id`:
//!
//! | Method | Path                                                          | Body                                |
//! |--------|---------------------------------------------------------------|-------------------------------------|
//! | GET    | `/api/webchat/v2/tokens/reduction-rules`                      | —                                   |
//! | PUT    | `/api/webchat/v2/tokens/reduction-rules`                      | [`ReductionRulesRequest`]           |
//! | POST   | `/api/webchat/v2/tokens/reduction-rules/author`               | [`AuthorReductionRuleRequest`]      |
//!
//! All three reject with `400 InvalidRequest` when the caller carries no
//! `project_id` — silent fall-through to an empty-string bucket would land
//! rules in the wrong scope and the orchestrator's `(project_id, user_id)`
//! cache would never see them on subsequent turns.

use axum::Json;
use axum::extract::{Extension, State};
use brassclaw_product_workflow::{
    AuthorReductionRuleRequest, AuthorReductionRuleResponse, RebornServicesError,
    ReductionRulesRequest, ReductionRulesResponse, WebUiAuthenticatedCaller,
};

use crate::error::WebUiV2HttpError;
use crate::router::WebUiV2State;

/// Resolve the authenticated caller's project id into the `&str` form the
/// `RebornServices` trait expects. Returns `Err(_)` (mapped through
/// [`WebUiV2HttpError`] via `?`) when no project is bound to the caller.
fn require_project_id(caller: &WebUiAuthenticatedCaller) -> Result<&str, WebUiV2HttpError> {
    match caller.project_id.as_ref() {
        Some(id) => Ok(id.as_str()),
        None => Err(RebornServicesError::invalid_request().into()),
    }
}

/// `GET /api/webchat/v2/tokens/reduction-rules`
///
/// List the saved reduction rules for the caller's project. The response
/// is sorted with ascending `priority` so a subsequent `PUT` round-trips
/// byte-for-byte. Empty arrays are returned (not `404`) when no rules
/// are configured — operators seeing an empty list is a normal state.
pub async fn list_reduction_rules(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
) -> Result<Json<ReductionRulesResponse>, WebUiV2HttpError> {
    let project_id = require_project_id(&caller)?.to_string();
    let response = state
        .services()
        .list_reduction_rules(caller, &project_id)
        .await?;
    Ok(Json(response))
}

/// `PUT /api/webchat/v2/tokens/reduction-rules`
///
/// Replace the entire rule set atomically. Validation runs before any
/// write — duplicate ids, oversize lists, malformed fields, and unknown
/// `rule_type` values are all rejected with `400`. The replacement
/// invalidates the orchestrator's in-process reduction-rule cache only
/// after the storage write succeeds; if the invalidation hook fails
/// (e.g. closed channel), the storage row is still authoritative and the
/// next cache miss will reread it.
pub async fn replace_reduction_rules(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<ReductionRulesRequest>,
) -> Result<Json<ReductionRulesResponse>, WebUiV2HttpError> {
    let project_id = require_project_id(&caller)?.to_string();
    let response = state
        .services()
        .replace_reduction_rules(caller, &project_id, body)
        .await?;
    Ok(Json(response))
}

/// `POST /api/webchat/v2/tokens/reduction-rules/author`
///
/// Author a single new rule from a structured request and persist it
/// through the same validation path that gates `PUT`. The author's project
/// id comes from the caller's bound `project_id` rather than the request
/// body — keeping the surface minimal and avoiding an ambiguity where the
/// body could otherwise claim a different scope from the caller.
///
/// The caller-bound `project_id` is required at the handler boundary even
/// though the trait impl also gates missing-project callers: defending at
/// the boundary keeps the no-project guard symmetric with the list and
/// replace endpoints so a wildcard caller never reaches the facade,
/// preserving the orchestrator cache's `(project_id, user_id)` invariant.
pub async fn author_reduction_rule(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<AuthorReductionRuleRequest>,
) -> Result<Json<AuthorReductionRuleResponse>, WebUiV2HttpError> {
    // Resolve and discard — `require_project_id` returns immediately on
    // a missing project, so the facade is never reached with a wildcard
    // caller; the trait impl also gates this internally as a defence
    // in depth.
    let _ = require_project_id(&caller)?.to_string();
    let response = state.services().author_reduction_rule(caller, body).await?;
    Ok(Json(response))
}
