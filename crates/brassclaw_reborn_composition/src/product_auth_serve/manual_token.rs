//! Manual token values use only the dedicated secret bodies. Original metadata
//! and confirmed-reference continuation use separate explicit contracts.
use super::*;
use axum::extract::rejection::JsonRejection;
use brassclaw_auth::ManualTokenSetupIdentity;

fn manual_scope(
    caller: &WebUiAuthenticatedCaller,
    session: Option<String>,
    thread: Option<String>,
    invocation: String,
) -> Result<AuthProductScope, ProductAuthRouteFailure> {
    scope_from_authenticated_caller_parts_requiring_invocation(
        caller,
        &ScopeFields {
            session_id: session,
            thread_id: thread,
            invocation_id: Some(invocation),
        },
    )
}
fn ready_for_raw(status: AuthFlowStatus) -> Result<(), ProductAuthRouteFailure> {
    match status {
        AuthFlowStatus::AwaitingUser | AuthFlowStatus::Completed => Ok(()),
        AuthFlowStatus::Canceled => Err(AuthProductError::Canceled.into()),
        AuthFlowStatus::Expired => Err(AuthProductError::UnknownOrExpiredFlow.into()),
        AuthFlowStatus::Failed => Err(AuthProductError::FlowAlreadyTerminal.into()),
        _ => Err(AuthProductError::BackendConflict.into()),
    }
}
fn submit_response(
    identity: ManualTokenSetupIdentity,
    invocation_id: InvocationId,
    submitted: RebornManualTokenSubmitResponse,
) -> ManualTokenSubmitResponse {
    ManualTokenSubmitResponse {
        identity,
        invocation_id,
        credential_ref: submitted.account_id,
        status: submitted.status,
        continuation: submitted.continuation,
    }
}

pub(super) async fn manual_token_submit_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    payload: Result<Json<ManualTokenSubmitRequest>, JsonRejection>,
) -> Result<Json<ManualTokenSubmitResponse>, ProductAuthRouteFailure> {
    let Json(request) = payload.map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let identity = request.identity.parse()?;
    let scope = manual_scope(
        &caller,
        request.session_id,
        request.thread_id,
        request.invocation_id,
    )?;
    let provider = AuthProviderId::new(request.provider)
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let label = CredentialAccountLabel::new(request.account_label)
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let token = request
        .token
        .into_validated()
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let continuation = manual_token_continuation(Some(&request.run_id), Some(&request.gate_ref))?;
    let expires_at = Utc::now() + ChronoDuration::seconds(PRODUCT_AUTH_FLOW_MAX_TTL_SECONDS);
    let original =
        run_with_backend_timeout(state.product_auth.request_original_manual_token_setup(
            identity,
            RebornManualTokenSetupRequest::new(
                scope.clone(),
                provider,
                label,
                continuation,
                expires_at,
            ),
        ))
        .await?;
    ready_for_raw(original.status)?;
    let submitted =
        submit_original_manual_token_request(&state, &scope, identity, token.into_secret()).await?;
    Ok(Json(submit_response(
        identity,
        scope.resource.invocation_id,
        submitted,
    )))
}

/// Timeouts and errors preserve the original pending/completed state. They are
/// never automatic abandonment or replacement-setup requests.
pub(super) async fn submit_original_manual_token_request(
    state: &ProductAuthRouteState,
    scope: &AuthProductScope,
    identity: ManualTokenSetupIdentity,
    secret: SecretString,
) -> Result<RebornManualTokenSubmitResponse, ProductAuthRouteFailure> {
    run_with_backend_timeout(state.product_auth.submit_original_manual_token(
        identity,
        RebornManualTokenSubmitRequest::new(scope.clone(), identity.interaction_id, secret),
    ))
    .await
}

pub(super) async fn manual_token_setup_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    payload: Result<Json<ManualTokenSetupRequest>, JsonRejection>,
) -> Result<Json<ManualTokenSetupResponse>, ProductAuthRouteFailure> {
    let Json(request) = payload.map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let identity = request.identity.parse()?;
    let scope = manual_scope(
        &caller,
        request.session_id,
        request.thread_id,
        request.invocation_id,
    )?;
    let invocation_id = scope.resource.invocation_id;
    let provider = AuthProviderId::new(request.provider)
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let label = CredentialAccountLabel::new(request.account_label)
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let continuation =
        manual_token_continuation(request.run_id.as_deref(), request.gate_ref.as_deref())?;
    let expires_at = Utc::now() + ChronoDuration::seconds(PRODUCT_AUTH_FLOW_MAX_TTL_SECONDS);
    let receipt = run_with_backend_timeout(state.product_auth.request_original_manual_token_setup(
        identity,
        RebornManualTokenSetupRequest::new(scope, provider, label, continuation, expires_at),
    ))
    .await?;
    Ok(Json(ManualTokenSetupResponse {
        identity: receipt.identity,
        status: receipt.status,
        interaction_id: receipt.challenge.interaction_id,
        provider: receipt.challenge.provider,
        label: receipt.challenge.label,
        expires_at: receipt.challenge.expires_at,
        invocation_id,
    }))
}

pub(super) async fn manual_token_secret_submit_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    payload: Result<Json<ManualTokenSecretSubmitRequest>, JsonRejection>,
) -> Result<Json<ManualTokenSubmitResponse>, ProductAuthRouteFailure> {
    let Json(request) = payload.map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let identity = request.identity.parse()?;
    let scope = manual_scope(
        &caller,
        request.session_id,
        request.thread_id,
        request.invocation_id,
    )?;
    let token = request
        .token
        .into_validated()
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let submitted =
        submit_original_manual_token_request(&state, &scope, identity, token.into_secret()).await?;
    Ok(Json(submit_response(
        identity,
        scope.resource.invocation_id,
        submitted,
    )))
}

pub(super) async fn manual_token_continue_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    payload: Result<Json<ManualTokenContinueRequest>, JsonRejection>,
) -> Result<Json<ManualTokenSubmitResponse>, ProductAuthRouteFailure> {
    let Json(request) = payload.map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let identity = request.identity.parse()?;
    let scope = manual_scope(
        &caller,
        request.session_id,
        request.thread_id,
        request.invocation_id,
    )?;
    let submitted = run_with_backend_timeout(
        state
            .product_auth
            .continue_original_manual_token(&scope, identity),
    )
    .await?;
    Ok(Json(submit_response(
        identity,
        scope.resource.invocation_id,
        submitted,
    )))
}

pub(super) fn manual_token_continuation(
    run_id: Option<&str>,
    gate_ref: Option<&str>,
) -> Result<AuthContinuationRef, ProductAuthRouteFailure> {
    match (run_id, gate_ref) {
        (Some(run_id), Some(gate_ref)) => Ok(AuthContinuationRef::TurnGateResume {
            turn_run_ref: TurnRunRef::new(run_id.to_string())
                .map_err(|_| ProductAuthRouteFailure::invalid_request())?,
            gate_ref: AuthGateRef::new(gate_ref.to_string())
                .map_err(|_| ProductAuthRouteFailure::invalid_request())?,
        }),
        (None, None) => Ok(AuthContinuationRef::SetupOnly),
        // run_id without gate_ref (or vice versa) is rejected so we never
        // resume the wrong gate or fabricate a partial turn-resume.
        _ => Err(ProductAuthRouteFailure::invalid_request()),
    }
}
