//! Token settings handlers for WebUI v2.
//!
//! Provides endpoints to manage per-section token limits for LLM context
//! composition (conversation history, skills, identity, inline control,
//! memory, safety, capability surface, total input, and max output).

use axum::Json;
use axum::extract::{Extension, State};
use brassclaw_product_workflow::{
    TokenSettingsResponse, UpdateTokenSettingsRequest, WebUiAuthenticatedCaller,
};

use crate::error::WebUiV2HttpError;
use crate::router::WebUiV2State;

/// `GET /api/webchat/v2/tokens`
///
/// Fetch the current token settings.
pub async fn get_token_settings(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
) -> Result<Json<TokenSettingsResponse>, WebUiV2HttpError> {
    let response = state.services().get_token_settings(caller).await?;
    Ok(Json(response))
}

/// `PUT /api/webchat/v2/tokens`
///
/// Update the token settings.
pub async fn update_token_settings(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(body): Json<UpdateTokenSettingsRequest>,
) -> Result<Json<TokenSettingsResponse>, WebUiV2HttpError> {
    let response = state
        .services()
        .update_token_settings(caller, body)
        .await?;
    Ok(Json(response))
}
