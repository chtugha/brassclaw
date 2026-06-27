//! Safety configuration handlers for WebUI v2.
//!
//! Provides endpoints to manage filesystem safety rules:
//! - Sensitive path patterns (credentials, keys, etc.)
//! - Workspace-protected files (MEMORY.md, IDENTITY.md, etc.)
//! - Blocked device/process paths (/dev/zero, /proc/kcore, etc.)

use axum::Json;
use axum::extract::{Extension, State};
use brassclaw_product_workflow::{
    SafetyConfigResponse, UpdateSafetyConfigRequest, WebUiAuthenticatedCaller,
};

use crate::error::WebUiV2HttpError;
use crate::router::WebUiV2State;

/// `GET /api/webchat/v2/safety/sensitive-paths`
///
/// Fetch the current sensitive path patterns configuration.
pub async fn get_sensitive_paths(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
) -> Result<Json<SafetyConfigResponse>, WebUiV2HttpError> {
    let response = state.services().get_safety_sensitive_paths(caller).await?;
    Ok(Json(response))
}

/// `PUT /api/webchat/v2/safety/sensitive-paths`
///
/// Update the sensitive path patterns configuration.
pub async fn update_sensitive_paths(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<UpdateSafetyConfigRequest>,
) -> Result<Json<SafetyConfigResponse>, WebUiV2HttpError> {
    let response = state
        .services()
        .update_safety_sensitive_paths(caller, body)
        .await?;
    Ok(Json(response))
}

/// `GET /api/webchat/v2/safety/workspace-rules`
///
/// Fetch the current workspace file protection rules.
pub async fn get_workspace_rules(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
) -> Result<Json<SafetyConfigResponse>, WebUiV2HttpError> {
    let response = state.services().get_safety_workspace_rules(caller).await?;
    Ok(Json(response))
}

/// `PUT /api/webchat/v2/safety/workspace-rules`
///
/// Update the workspace file protection rules.
pub async fn update_workspace_rules(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<UpdateSafetyConfigRequest>,
) -> Result<Json<SafetyConfigResponse>, WebUiV2HttpError> {
    let response = state
        .services()
        .update_safety_workspace_rules(caller, body)
        .await?;
    Ok(Json(response))
}

/// `GET /api/webchat/v2/safety/blocked-paths`
///
/// Fetch the current blocked device/process paths.
pub async fn get_blocked_paths(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
) -> Result<Json<SafetyConfigResponse>, WebUiV2HttpError> {
    let response = state.services().get_safety_blocked_paths(caller).await?;
    Ok(Json(response))
}

/// `PUT /api/webchat/v2/safety/blocked-paths`
///
/// Update the blocked device/process paths.
pub async fn update_blocked_paths(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<UpdateSafetyConfigRequest>,
) -> Result<Json<SafetyConfigResponse>, WebUiV2HttpError> {
    let response = state
        .services()
        .update_safety_blocked_paths(caller, body)
        .await?;
    Ok(Json(response))
}
