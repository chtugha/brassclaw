//! OAuth start and callback handlers.

use super::*;
use crate::auth::RebornDcrSetupOriginalLookup;
use crate::oauth_dcr::DcrOAuthCallbackState;
use brassclaw_auth::AuthFlowRecord;

pub(super) async fn oauth_start_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(request): Json<OAuthStartRequest>,
) -> Result<Json<OAuthStartResponse>, ProductAuthRouteFailure> {
    let now = Utc::now();
    if request.expires_at <= now
        || request.expires_at > now + ChronoDuration::seconds(PRODUCT_AUTH_FLOW_MAX_TTL_SECONDS)
    {
        return Err(ProductAuthRouteFailure::invalid_request());
    }

    let scope = scope_from_authenticated_caller(&caller, &request)?;
    let provider = AuthProviderId::new(request.provider).map_err(|_| {
        ProductAuthRouteFailure::new(StatusCode::BAD_REQUEST, AuthErrorCode::InvalidRequest)
    })?;
    if provider.as_str() == GOOGLE_PROVIDER_ID {
        return Err(ProductAuthRouteFailure::invalid_request());
    }
    let authorization_endpoint = authorization_endpoint_url(&request.authorization_url)?;
    let opaque_state = request
        .opaque_state
        .into_validated()
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let pkce_verifier_value = request
        .pkce_verifier
        .into_validated()
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let opaque_state_hash = opaque_state_hash(opaque_state.as_str())?;
    let pkce_verifier_hash = pkce_verifier_hash(pkce_verifier_value.expose_secret())?;
    let pkce_verifier = pkce_verifier_value.clone_secret();

    let flow = run_with_backend_timeout(
        state
            .product_auth
            .start_setup_oauth_flow(RebornOAuthStartFlowRequest {
                requirement_setup: None,
                flow_id: None,
                scope: scope.clone(),
                provider: provider.clone(),
                authorization_url: OAuthAuthorizationUrl::new(authorization_endpoint.to_string())
                    .map_err(ProductAuthRouteFailure::from)?,
                opaque_state_hash,
                pkce_verifier_hash,
                update_binding: None,
                expires_at: request.expires_at,
            }),
    )
    .await?;
    state.store_pkce_verifier(flow.id, pkce_verifier, flow.expires_at)?;
    let authorization_url = compose_authorization_url(authorization_endpoint, flow.id, &scope)?;

    Ok(Json(OAuthStartResponse {
        flow_id: flow.id,
        status: flow.status,
        provider,
        authorization_url,
        expires_at: flow.expires_at,
        continuation: flow.continuation,
        callback_scope: scope_hint(&scope),
    }))
}

pub(super) async fn google_oauth_start_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Json(request): Json<GoogleOAuthStartRequest>,
) -> Result<Json<ProductOAuthStartResponse>, ProductAuthRouteFailure> {
    let flow = start_google_oauth_flow(state, caller, request, None).await?;
    let expired = flow.expires_at <= Utc::now();
    let terminal = brassclaw_auth::is_terminal_status(flow.status) || expired;
    let authorization_url = if terminal {
        None
    } else {
        Some(google_flow_authorization_url(&flow)?)
    };
    Ok(Json(ProductOAuthStartResponse {
        flow_id: flow.id,
        status: flow.status,
        provider: flow.provider,
        authorization_url,
        terminal,
        expired,
        expires_at: flow.expires_at,
        continuation: flow.continuation,
        callback_scope: scope_hint(&flow.scope),
    }))
}

pub(super) async fn extension_oauth_start_handler(
    State(state): State<ProductAuthRouteState>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(package_id): Path<String>,
    Json(request): Json<ExtensionOAuthStartRequest>,
) -> Result<(StatusCode, Json<ExtensionOAuthResponse>), ProductAuthRouteFailure> {
    let requester_extension =
        ExtensionId::new(package_id).map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    if request.provider != GOOGLE_PROVIDER_ID {
        return start_dcr_extension_oauth_flow(state, caller, request, requester_extension).await;
    }
    let flow_id = AuthFlowId::from_uuid(
        Uuid::parse_str(
            request
                .flow_id
                .as_deref()
                .ok_or_else(ProductAuthRouteFailure::invalid_request)?,
        )
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?,
    );
    let label = CredentialAccountLabel::new(request.account_label.clone())
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let scopes =
        parse_google_requested_scopes(&request.scopes).map_err(ProductAuthRouteFailure::from)?;
    let scope = scope_from_authenticated_caller_parts_requiring_invocation(
        &caller,
        &ScopeFields {
            session_id: None,
            thread_id: None,
            invocation_id: request.invocation_id.clone(),
        },
    )?;
    // Read the accepted original before config, clock limits or account lookup.
    // Recover/cancel can never create a new flow or generate another verifier.
    let original =
        recover_google_setup_original(&state, &scope, flow_id, &label, &scopes, request.expires_at)
            .await?;
    if let Some(original) = &original {
        extension_oauth_requirement::verify(
            &state,
            &scope,
            original,
            &requester_extension,
            &request,
        )
        .await?;
    }
    let flow = match request.action {
        ExtensionOAuthAction::Recover => original.ok_or(AuthProductError::UnknownOrExpiredFlow)?,
        ExtensionOAuthAction::Cancel => {
            run_with_backend_timeout(state.product_auth.cancel_google_setup_original(
                &scope,
                flow_id,
                &label,
                &scopes,
                request.expires_at,
            ))
            .await?
        }
        ExtensionOAuthAction::Start => match original {
            Some(flow) => flow,
            None => {
                let prepared = extension_oauth_requirement::prepare(
                    &state,
                    &caller,
                    &scope,
                    &requester_extension,
                    &request,
                )
                .await?;

                start_google_oauth_flow(
                    state.clone(),
                    caller,
                    GoogleOAuthStartRequest {
                        flow_id,
                        account_label: request.account_label.clone(),
                        scopes: request.scopes.clone(),
                        expires_at: request.expires_at,
                        session_id: None,
                        thread_id: None,
                        invocation_id: request.invocation_id.clone(),
                    },
                    Some(prepared),
                )
                .await?
            }
        },
    };
    extension_oauth_requirement::verify(&state, &scope, &flow, &requester_extension, &request)
        .await?;
    let terminal = brassclaw_auth::is_terminal_status(flow.status);
    let expired = !terminal && flow.expires_at <= Utc::now();
    if terminal {
        state.remove_pkce_verifier(flow.id);
    }
    let authorization_url = if flow.status == AuthFlowStatus::AwaitingUser && !expired {
        Some(google_flow_authorization_url(&flow)?)
    } else {
        None
    };
    let http = if matches!(
        flow.status,
        AuthFlowStatus::Pending | AuthFlowStatus::CallbackReceived | AuthFlowStatus::Completing
    ) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok((
        http,
        Json(ExtensionOAuthResponse {
            flow_id: flow.id,
            status: flow.status,
            provider: flow.provider,
            account_label: label,
            scopes,
            authorization_url,
            expires_at: flow.expires_at,
            continuation: flow.continuation,
            callback_scope: scope_hint(&scope),
            error: None,
            terminal,
            expired,
            cleanup: None,
        }),
    ))
}

fn google_flow_authorization_url(
    flow: &AuthFlowRecord,
) -> Result<OAuthAuthorizationUrl, ProductAuthRouteFailure> {
    match &flow.challenge {
        Some(AuthChallenge::OAuthUrl {
            authorization_url,
            expires_at,
        }) if *expires_at == flow.expires_at => Ok(authorization_url.clone()),
        _ => Err(AuthProductError::BackendConflict.into()),
    }
}

async fn recover_google_setup_original(
    state: &ProductAuthRouteState,
    scope: &AuthProductScope,
    flow_id: AuthFlowId,
    label: &CredentialAccountLabel,
    scopes: &[ProviderScope],
    expires_at: Timestamp,
) -> Result<Option<AuthFlowRecord>, ProductAuthRouteFailure> {
    run_with_backend_timeout(
        state
            .product_auth
            .recover_google_setup_original(scope, flow_id, label, scopes, expires_at),
    )
    .await
}

async fn start_dcr_extension_oauth_flow(
    state: ProductAuthRouteState,
    caller: WebUiAuthenticatedCaller,
    request: ExtensionOAuthStartRequest,
    requester_extension: ExtensionId,
) -> Result<(StatusCode, Json<ExtensionOAuthResponse>), ProductAuthRouteFailure> {
    let flow_id = AuthFlowId::from_uuid(
        Uuid::parse_str(
            request
                .flow_id
                .as_deref()
                .ok_or_else(ProductAuthRouteFailure::invalid_request)?,
        )
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?,
    );
    let provider = AuthProviderId::new(request.provider.as_str())
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let account_label = CredentialAccountLabel::new(request.account_label.as_str())
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let scopes = request
        .scopes
        .iter()
        .cloned()
        .map(ProviderScope::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let fields = ScopeFields {
        session_id: None,
        thread_id: None,
        invocation_id: request.invocation_id.clone(),
    };
    let scope = scope_from_authenticated_caller_parts_requiring_invocation(&caller, &fields)?;
    let original_lookup = RebornDcrSetupOriginalLookup {
        scope: &scope,
        flow_id,
        provider: &provider,
        account_label: &account_label,
        provider_scopes: &scopes,
        expires_at: request.expires_at,
        requester_extension: &requester_extension,
        requirement_name: request.requirement.as_deref(),
        target: request.target.as_ref(),
        cancel_preparation: request.action == ExtensionOAuthAction::Cancel,
    };
    let original = run_with_backend_timeout(
        state
            .product_auth
            .recover_dcr_setup_original(original_lookup.clone()),
    )
    .await?;
    let mut error = None;
    let flow = match request.action {
        ExtensionOAuthAction::Recover => original.ok_or(AuthProductError::UnknownOrExpiredFlow)?,
        ExtensionOAuthAction::Cancel => {
            original.ok_or(AuthProductError::UnknownOrExpiredFlow)?;
            match run_with_backend_timeout(
                state
                    .product_auth
                    .cancel_dcr_setup_original(&scope, flow_id),
            )
            .await
            {
                Ok(flow) => flow,
                Err(failure) => {
                    // Cancellation may already be committed while provider or
                    // local cleanup remains unknown. Retain that actual original
                    // disposition and the full safe error instead of reverting
                    // the browser to its earlier authorization state.
                    let safe_failure = failure.body;
                    let actual = run_with_backend_timeout(
                        state
                            .product_auth
                            .recover_dcr_setup_original(original_lookup.clone()),
                    )
                    .await?
                    .ok_or(failure)?;
                    error = Some(safe_failure);
                    actual
                }
            }
        }
        ExtensionOAuthAction::Start => {
            let (requirement_setup, binding) = match original {
                Some(original) => (
                    run_with_backend_timeout(
                        state
                            .product_auth
                            .flow_manager()
                            .requirement_setup(&scope, flow_id),
                    )
                    .await?
                    .map(Box::new),
                    original.update_binding,
                ),
                None => {
                    let now = Utc::now();
                    if request.expires_at <= now
                        || request.expires_at
                            > now + ChronoDuration::seconds(PRODUCT_AUTH_FLOW_MAX_TTL_SECONDS)
                    {
                        return Err(ProductAuthRouteFailure::invalid_request());
                    }
                    let (setup, binding) = extension_oauth_requirement::prepare(
                        &state,
                        &caller,
                        &scope,
                        &requester_extension,
                        &request,
                    )
                    .await?;
                    (Some(Box::new(setup)), binding)
                }
            };
            match run_with_backend_timeout(state.product_auth.start_dcr_setup_oauth_flow(
                RebornDcrOAuthStartFlowRequest {
                    requirement_setup,
                    flow_id,
                    scope: scope.clone(),
                    provider: provider.clone(),
                    account_label: account_label.clone(),
                    provider_scopes: scopes.clone(),
                    update_binding: binding,
                    expires_at: request.expires_at,
                },
            ))
            .await
            {
                Ok(Some(flow)) => flow,
                Ok(None) => return Err(ProductAuthRouteFailure::malformed_config()),
                Err(failure) => {
                    // Preserve and expose the actual original disposition alongside
                    // the failure. A 202 receipt is not successful authorization.
                    let safe_failure = failure.body;
                    let actual = run_with_backend_timeout(
                        state
                            .product_auth
                            .recover_dcr_setup_original(original_lookup.clone()),
                    )
                    .await?
                    .ok_or(failure)?;
                    error = Some(safe_failure);
                    actual
                }
            }
        }
    };
    extension_oauth_requirement::verify(&state, &scope, &flow, &requester_extension, &request)
        .await?;
    let cleanup = Some(
        run_with_backend_timeout(state.product_auth.dcr_cleanup_status(&scope, flow.id)).await?,
    );
    let status = flow.status;
    let terminal = brassclaw_auth::is_terminal_status(status);
    let expired = !terminal && flow.expires_at <= Utc::now();
    let authorization_url = if status == AuthFlowStatus::AwaitingUser && error.is_none() && !expired
    {
        match &flow.challenge {
            Some(AuthChallenge::OAuthUrl {
                authorization_url, ..
            }) => Some(authorization_url.clone()),
            _ => return Err(ProductAuthRouteFailure::backend_unavailable()),
        }
    } else {
        None
    };
    let http = if error.is_some()
        || matches!(
            status,
            AuthFlowStatus::Pending | AuthFlowStatus::CallbackReceived | AuthFlowStatus::Completing
        ) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok((
        http,
        Json(ExtensionOAuthResponse {
            flow_id: flow.id,
            status,
            provider,
            account_label,
            scopes,
            authorization_url,
            expires_at: flow.expires_at,
            continuation: flow.continuation,
            callback_scope: scope_hint(&scope),
            error,
            terminal,
            expired,
            cleanup,
        }),
    ))
}

async fn start_google_oauth_flow(
    state: ProductAuthRouteState,
    caller: WebUiAuthenticatedCaller,
    request: GoogleOAuthStartRequest,
    prepared: Option<extension_oauth_requirement::PreparedOAuthRequirement>,
) -> Result<AuthFlowRecord, ProductAuthRouteFailure> {
    let provider = AuthProviderId::new(GOOGLE_PROVIDER_ID)
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let account_label = CredentialAccountLabel::new(request.account_label)
        .map_err(|_| ProductAuthRouteFailure::invalid_request())?;
    let requested_scopes =
        parse_google_requested_scopes(&request.scopes).map_err(ProductAuthRouteFailure::from)?;
    let fields = ScopeFields {
        session_id: request.session_id,
        thread_id: request.thread_id,
        invocation_id: request.invocation_id,
    };
    let scope = scope_from_authenticated_caller_parts_requiring_invocation(&caller, &fields)?;
    let flow_id = request.flow_id;
    if flow_id.as_uuid().is_nil() || scope.resource.invocation_id.as_uuid().is_nil() {
        return Err(ProductAuthRouteFailure::invalid_request());
    }
    if let Some(flow) = recover_google_setup_original(
        &state,
        &scope,
        flow_id,
        &account_label,
        &requested_scopes,
        request.expires_at,
    )
    .await?
    {
        return Ok(flow);
    }
    let now = Utc::now();
    if request.expires_at <= now
        || request.expires_at > now + ChronoDuration::seconds(PRODUCT_AUTH_FLOW_MAX_TTL_SECONDS)
    {
        return Err(ProductAuthRouteFailure::invalid_request());
    }

    let config = state.google_oauth_config()?;
    let (requirement_setup, update_binding) = prepared.map_or((None, None), |(setup, binding)| {
        (Some(Box::new(setup)), binding)
    });
    if let Some(setup) = &requirement_setup {
        setup.verify_oauth(&scope, &provider, &requested_scopes)?;
    }
    let created = run_with_backend_timeout(state.product_auth.start_captured_google_setup(
        crate::oauth_gate::GoogleSetupRequest {
            scope: scope.clone(),
            flow_id,
            account_label: account_label.clone(),
            requested_scopes: requested_scopes.clone(),
            continuation: AuthContinuationRef::SetupOnly,
            requirement_setup,
            update_binding,
            update_account_snapshot: None,
            deadline: crate::oauth_gate::GoogleSetupDeadline::Exact(request.expires_at),
        },
        config,
    ))
    .await;
    let flow = match created {
        Ok(flow) => flow,
        Err(failure) => match recover_google_setup_original(
            &state,
            &scope,
            flow_id,
            &account_label,
            &requested_scopes,
            request.expires_at,
        )
        .await?
        {
            Some(flow) => flow,
            None => return Err(failure),
        },
    };
    Ok(flow)
}

pub(super) async fn oauth_callback_handler(
    State(state): State<ProductAuthRouteState>,
    Path(flow_id): Path<String>,
    RawQuery(raw_query): RawQuery,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ProductAuthRouteFailure> {
    validate_callback_raw_query(raw_query.as_deref())?;
    let query = axum::extract::Query::<OAuthCallbackQuery>::try_from_uri(&uri)
        .map_err(|_| ProductAuthRouteFailure::malformed_callback())?
        .0;
    validate_callback_query_fields(&query)?;

    let flow_id = AuthFlowId::from_uuid(
        Uuid::parse_str(&flow_id).map_err(|_| ProductAuthRouteFailure::malformed_callback())?,
    );
    let state_value = query
        .state
        .as_ref()
        .ok_or_else(ProductAuthRouteFailure::malformed_callback)?;
    let decoded_state = dcr_callback_state_from_oauth_state(state_value.as_str())?;
    if let Some(decoded) = &decoded_state
        && decoded.flow_id() != flow_id
    {
        return Err(ProductAuthRouteFailure::malformed_callback());
    }
    let scope = decoded_state
        .as_ref()
        .map(|decoded| decoded.scope().clone())
        .map(Ok)
        .unwrap_or_else(|| scope_from_callback_query(&state, &query))?;
    let state_hash = opaque_state_hash(state_value.as_str())?;

    let result = async {
        if is_authorized_callback_candidate(&query, decoded_state.as_ref()) {
            let reference =
                callback_reference_from_query(flow_id, &scope, decoded_state.as_ref(), &query)?;
            if let Some(response) = run_with_backend_timeout(
                state
                    .product_auth
                    .resume_oauth_callback_original(&reference),
            )
            .await?
            {
                state.remove_pkce_verifier(flow_id);
                return Ok(oauth_callback_response(&headers, response));
            }
        }

        let flow_provider = if is_authorized_callback_candidate(&query, decoded_state.as_ref()) {
            Some(
                run_with_backend_timeout(
                    state
                        .product_auth
                        .ensure_oauth_callback_flow_known(&scope, flow_id),
                )
                .await?,
            )
        } else {
            None
        };
        let outcome = callback_outcome_from_query(
            &state,
            flow_id,
            &scope,
            flow_provider.as_ref(),
            decoded_state.as_ref(),
            &query,
        )
        .await?;

        let response = run_with_backend_timeout(state.product_auth.handle_oauth_callback(
            RebornOAuthCallbackRequest {
                scope: scope.clone(),
                flow_id,
                opaque_state_hash: state_hash.clone(),
                outcome,
            },
        ))
        .await?;
        state.remove_pkce_verifier(flow_id);

        Ok(oauth_callback_response(&headers, response))
    }
    .await;
    if result.is_err() {
        forget_pkce_for_terminal_callback(&state, &scope, flow_id, &state_hash).await?;
    }
    result
}

pub(super) async fn google_oauth_callback_handler(
    State(state): State<ProductAuthRouteState>,
    RawQuery(raw_query): RawQuery,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response, ProductAuthRouteFailure> {
    validate_callback_raw_query(raw_query.as_deref())?;
    let query = axum::extract::Query::<GoogleOAuthCallbackQuery>::try_from_uri(&uri)
        .map_err(|_| ProductAuthRouteFailure::malformed_callback())?
        .0;
    validate_google_callback_query_fields(&query)?;
    let state_value = query
        .state
        .as_ref()
        .ok_or_else(ProductAuthRouteFailure::malformed_callback)?;
    let state_hash = opaque_state_hash(state_value.as_str())?;
    let callback_state = GoogleOAuthCallbackState::decode(state_value.as_str())
        .map_err(ProductAuthRouteFailure::from)?;
    let flow_id = callback_state.flow_id();
    let callback_scope = callback_state.scope();

    let result = async {
        if query
            .error
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            let response = run_with_backend_timeout(state.product_auth.handle_oauth_callback(
                RebornOAuthCallbackRequest {
                    scope: callback_scope.clone(),
                    flow_id,
                    opaque_state_hash: state_hash.clone(),
                    outcome: RebornOAuthCallbackOutcome::ProviderDenied,
                },
            ))
            .await;
            return response.map(|response| oauth_callback_response(&headers, response));
        }

        let Some(code) = query.code.as_ref() else {
            return Err(ProductAuthRouteFailure::malformed_callback());
        };
        let requested_scopes = callback_state.requested_scopes();
        if let Some(callback_scopes) = parse_google_callback_scopes(query.scopes.as_deref())
            .map_err(ProductAuthRouteFailure::from)?
        {
            validate_google_callback_includes_requested_scopes(&callback_scopes, requested_scopes)?;
        }
        let callback_scopes = requested_scopes.to_vec();
        let authorization_code_hash = authorization_code_hash(code.expose_secret())?;
        let reference = brassclaw_auth::OAuthCallbackReference {
            scope: callback_scope.clone(),
            flow_id,
            provider: AuthProviderId::new(GOOGLE_PROVIDER_ID)
                .map_err(|_| ProductAuthRouteFailure::malformed_callback())?,
            opaque_state_hash: state_hash.clone(),
            authorization_code_hash: authorization_code_hash.clone(),
            account_label: callback_state.account_label().clone(),
            requested_scopes: callback_scopes.clone(),
        };
        if let Some(response) = run_with_backend_timeout(
            state
                .product_auth
                .resume_oauth_callback_original(&reference),
        )
        .await?
        {
            state.remove_pkce_verifier(flow_id);
            return Ok(oauth_callback_response(&headers, response));
        }
        let provider = run_with_backend_timeout(
            state
                .product_auth
                .ensure_oauth_callback_flow_known(callback_scope, flow_id),
        )
        .await?;
        let pkce_verifier =
            pkce_verifier_for_known_callback_flow(&state, callback_scope, &provider, flow_id)
                .await?;
        let pkce_verifier_hash = pkce_verifier_hash(pkce_verifier.expose_secret())?;

        let response = run_with_backend_timeout(
            state
                .product_auth
                .handle_oauth_callback(RebornOAuthCallbackRequest {
                    scope: callback_scope.clone(),
                    flow_id,
                    opaque_state_hash: state_hash.clone(),
                    outcome: RebornOAuthCallbackOutcome::Authorized {
                        provider_request: OAuthProviderCallbackRequest {
                            provider: AuthProviderId::new(GOOGLE_PROVIDER_ID)
                                .map_err(|_| ProductAuthRouteFailure::malformed_callback())?,
                            account_label: callback_state.account_label().clone(),
                            authorization_code: OAuthAuthorizationCode::new(code.clone_secret())
                                .map_err(ProductAuthRouteFailure::from)?,
                            authorization_code_hash,
                            pkce_verifier: PkceVerifierSecret::new(pkce_verifier)
                                .map_err(ProductAuthRouteFailure::from)?,
                            pkce_verifier_hash,
                            scopes: callback_scopes,
                        },
                    },
                }),
        )
        .await?;
        state.remove_pkce_verifier(flow_id);

        Ok(oauth_callback_response(&headers, response))
    }
    .await;
    if result.is_err() {
        forget_pkce_for_terminal_callback(&state, callback_scope, flow_id, &state_hash).await?;
    }
    result
}

fn validate_google_callback_includes_requested_scopes(
    callback_scopes: &[ProviderScope],
    requested_scopes: &[ProviderScope],
) -> Result<(), ProductAuthRouteFailure> {
    if callback_scopes.is_empty()
        || !requested_scopes
            .iter()
            .all(|requested| callback_scopes.iter().any(|scope| scope == requested))
    {
        return Err(ProductAuthRouteFailure::malformed_callback());
    }
    Ok(())
}

fn oauth_callback_response(headers: &HeaderMap, response: RebornOAuthCallbackResponse) -> Response {
    if response.status != AuthFlowStatus::Completed {
        return ProductAuthRouteFailure::backend_unavailable().into_response();
    }
    if wants_oauth_callback_html(headers) {
        return oauth_callback_completion_html(&response);
    }
    Json(response).into_response()
}

fn wants_oauth_callback_html(headers: &HeaderMap) -> bool {
    let Some(accept) = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let accepts_html = accept
        .split(',')
        .any(|part| part.trim_start().starts_with("text/html"));
    let accepts_json = accept
        .split(',')
        .any(|part| part.trim_start().starts_with("application/json"));
    accepts_html && !accepts_json
}

fn oauth_callback_completion_html(response: &RebornOAuthCallbackResponse) -> Response {
    const CHANNEL: &str = "brassclaw-product-auth";
    const STORAGE_KEY: &str = "brassclaw:product-auth:oauth-complete";
    const MESSAGE_TYPE: &str = "brassclaw:product-auth:oauth-complete";

    let payload = json!({
        "type": MESSAGE_TYPE,
        "flowId": response.flow_id,
        "status": response.status,
        "continuation": response.continuation,
    })
    .to_string();
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Authorization complete</title>
</head>
<body>
  <p>Authorization complete. You can close this window.</p>
  <script>
    (() => {{
      const payload = {payload};
      try {{
        new BroadcastChannel("{CHANNEL}").postMessage(payload);
      }} catch (_err) {{}}
      try {{
        localStorage.setItem(
          "{STORAGE_KEY}",
          JSON.stringify({{ ...payload, completedAt: Date.now() }})
        );
      }} catch (_err) {{}}
      window.close();
    }})();
  </script>
</body>
</html>"#
    );
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

pub(super) async fn callback_outcome_from_query(
    state: &ProductAuthRouteState,
    flow_id: AuthFlowId,
    scope: &AuthProductScope,
    flow_provider: Option<&AuthProviderId>,
    callback_state: Option<&DcrOAuthCallbackState>,
    query: &OAuthCallbackQuery,
) -> Result<RebornOAuthCallbackOutcome, ProductAuthRouteFailure> {
    if query
        .error
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        return Ok(RebornOAuthCallbackOutcome::ProviderDenied);
    }

    let reference = callback_reference_from_query(flow_id, scope, callback_state, query)?;
    if flow_provider.is_some_and(|known_provider| known_provider != &reference.provider) {
        return Err(ProductAuthRouteFailure::malformed_callback());
    }
    let pkce_verifier =
        pkce_verifier_for_known_callback_flow(state, scope, &reference.provider, flow_id).await?;
    let code = query
        .code
        .as_ref()
        .ok_or_else(ProductAuthRouteFailure::malformed_callback)?;
    let pkce_verifier_hash = pkce_verifier_hash(pkce_verifier.expose_secret())?;
    Ok(RebornOAuthCallbackOutcome::Authorized {
        provider_request: OAuthProviderCallbackRequest {
            provider: reference.provider,
            account_label: reference.account_label,
            authorization_code: OAuthAuthorizationCode::new(code.clone_secret())
                .map_err(ProductAuthRouteFailure::from)?,
            authorization_code_hash: reference.authorization_code_hash,
            pkce_verifier: PkceVerifierSecret::new(pkce_verifier)
                .map_err(ProductAuthRouteFailure::from)?,
            pkce_verifier_hash,
            scopes: reference.requested_scopes,
        },
    })
}

/// Parse only explicit payload metadata. This never loads PKCE/current provider
/// config or rejects an already committed original because its first-call TTL ended.
fn callback_reference_from_query(
    flow_id: AuthFlowId,
    scope: &AuthProductScope,
    callback_state: Option<&DcrOAuthCallbackState>,
    query: &OAuthCallbackQuery,
) -> Result<brassclaw_auth::OAuthCallbackReference, ProductAuthRouteFailure> {
    let provider = match query.provider.as_deref() {
        Some(provider) => AuthProviderId::new(provider.to_string())
            .map_err(|_| ProductAuthRouteFailure::malformed_callback())?,
        None => callback_state
            .map(|state| state.provider().clone())
            .ok_or_else(ProductAuthRouteFailure::malformed_callback)?,
    };
    let account_label = match query.account_label.as_deref() {
        Some(label) => CredentialAccountLabel::new(label.to_string())
            .map_err(|_| ProductAuthRouteFailure::malformed_callback())?,
        None => callback_state
            .map(|state| state.account_label().clone())
            .ok_or_else(ProductAuthRouteFailure::malformed_callback)?,
    };
    let code = query
        .code
        .as_ref()
        .ok_or_else(ProductAuthRouteFailure::malformed_callback)?;
    let state = query
        .state
        .as_ref()
        .ok_or_else(ProductAuthRouteFailure::malformed_callback)?;
    let requested_scopes = match query.scopes.as_deref() {
        Some(raw) => parse_provider_scopes(Some(raw))?,
        None => callback_state
            .map(|state| state.requested_scopes().to_vec())
            .unwrap_or_default(),
    };
    Ok(brassclaw_auth::OAuthCallbackReference {
        scope: scope.clone(),
        flow_id,
        provider,
        account_label,
        opaque_state_hash: opaque_state_hash(state.as_str())?,
        authorization_code_hash: authorization_code_hash(code.expose_secret())?,
        requested_scopes,
    })
}

async fn pkce_verifier_for_known_callback_flow(
    state: &ProductAuthRouteState,
    scope: &AuthProductScope,
    provider: &AuthProviderId,
    flow_id: AuthFlowId,
) -> Result<SecretString, ProductAuthRouteFailure> {
    if provider.as_str() == GOOGLE_PROVIDER_ID {
        return run_with_backend_timeout(
            state
                .product_auth
                .oauth_pkce_verifier_for_flow(scope, provider, flow_id),
        )
        .await?
        .ok_or_else(ProductAuthRouteFailure::unknown_or_expired_flow);
    }
    let cache_error = match state.pkce_verifier_for_callback(flow_id) {
        Ok(verifier) => return Ok(verifier),
        Err(error) => error,
    };
    run_with_backend_timeout(
        state
            .product_auth
            .oauth_pkce_verifier_for_flow(scope, provider, flow_id),
    )
    .await?
    .ok_or(cache_error)
}

fn validate_google_callback_query_fields(
    query: &GoogleOAuthCallbackQuery,
) -> Result<(), ProductAuthRouteFailure> {
    validate_optional_callback_field(
        query.error.as_deref(),
        OAUTH_CALLBACK_FIELD_MAX_BYTES,
        false,
    )?;
    validate_optional_callback_field(
        query.scopes.as_deref(),
        OAUTH_CALLBACK_SCOPES_MAX_BYTES,
        true,
    )?;
    Ok(())
}

pub(super) fn is_authorized_callback_candidate(
    query: &OAuthCallbackQuery,
    callback_state: Option<&DcrOAuthCallbackState>,
) -> bool {
    query.error.as_deref().is_none_or(|value| value.is_empty())
        && (query.provider.is_some() || callback_state.is_some())
        && (query.account_label.is_some() || callback_state.is_some())
        && query.code.is_some()
}

async fn forget_pkce_for_terminal_callback(
    state: &ProductAuthRouteState,
    scope: &AuthProductScope,
    flow_id: AuthFlowId,
    state_hash: &OpaqueStateHash,
) -> Result<(), ProductAuthRouteFailure> {
    if run_with_backend_timeout(
        state
            .product_auth
            .oauth_callback_flow_is_terminal(scope, flow_id, state_hash),
    )
    .await?
    {
        state.remove_pkce_verifier(flow_id);
    }
    Ok(())
}

fn dcr_callback_state_from_oauth_state(
    state: &str,
) -> Result<Option<DcrOAuthCallbackState>, ProductAuthRouteFailure> {
    if !DcrOAuthCallbackState::has_prefix(state) {
        return Ok(None);
    }
    DcrOAuthCallbackState::decode(state)
        .map(Some)
        .map_err(ProductAuthRouteFailure::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RebornAuthContinuationDispatcher;
    use crate::auth_prompt::AuthChallengeProvider;
    use crate::input::OAuthClientConfig;
    use crate::oauth_gate::{GoogleOAuthGateProvider, GoogleOAuthGateProviderRegistry};
    use async_trait::async_trait;
    use brassclaw_auth::{GOOGLE_CALENDAR_READONLY_SCOPE, InMemoryAuthProductServices};
    use brassclaw_host_api::{
        RuntimeCredentialAccountProviderId, RuntimeCredentialAuthRequirement,
    };
    use brassclaw_secrets::{InMemorySecretStore, SecretStore};
    use brassclaw_turns::{TurnRunId, TurnScope};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn pkce_cleanup_requires_owned_terminal_oauth_flow() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let product_auth = Arc::new(RebornProductAuthServices::from_shared(
            shared,
            Arc::new(RecordingDispatcher::default()),
        ));
        let state = ProductAuthRouteState::new(
            product_auth.clone(),
            TenantId::new("tenant-alpha").unwrap(),
            None,
            None,
        );
        let scope = AuthProductScope::new(
            ResourceScope::local_default(UserId::new("user-alpha").unwrap(), InvocationId::new())
                .unwrap(),
            AuthSurface::Callback,
        );
        let hash = opaque_state_hash("owned-state").unwrap();
        let flow = product_auth
            .start_setup_oauth_flow(RebornOAuthStartFlowRequest {
                requirement_setup: None,
                flow_id: None,
                scope: scope.clone(),
                provider: AuthProviderId::new("google").unwrap(),
                authorization_url: OAuthAuthorizationUrl::new(
                    "https://accounts.google.com/authorize",
                )
                .unwrap(),
                opaque_state_hash: hash.clone(),
                pkce_verifier_hash: pkce_verifier_hash("owned-verifier").unwrap(),
                update_binding: None,
                expires_at: Utc::now() + ChronoDuration::minutes(5),
            })
            .await
            .unwrap();
        state
            .store_pkce_verifier(
                flow.id,
                SecretString::from("owned-verifier"),
                flow.expires_at,
            )
            .unwrap();
        forget_pkce_for_terminal_callback(&state, &scope, flow.id, &hash)
            .await
            .unwrap();
        assert!(state.pkce_verifier_for_callback(flow.id).is_ok());
        product_auth
            .flow_manager()
            .cancel_flow(&scope, flow.id)
            .await
            .unwrap();
        let error = forget_pkce_for_terminal_callback(
            &state,
            &scope,
            flow.id,
            &opaque_state_hash("foreign-state").unwrap(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.body.code, AuthErrorCode::CrossScopeDenied);
        assert!(state.pkce_verifier_for_callback(flow.id).is_ok());
        forget_pkce_for_terminal_callback(&state, &scope, flow.id, &hash)
            .await
            .unwrap();
        assert!(state.pkce_verifier_for_callback(flow.id).is_err());
    }

    #[tokio::test]
    async fn google_oauth_callback_uses_gate_pkce_store_when_route_cache_misses() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let secret_store = Arc::new(InMemorySecretStore::new());
        let secret_store_for_provider: Arc<dyn SecretStore> = secret_store.clone();
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let google_gate = Arc::new(GoogleOAuthGateProvider::new(
            OAuthClientConfig::new(
                "google-client.apps.googleusercontent.com",
                "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
                None,
            )
            .expect("google oauth client"),
            secret_store_for_provider,
        ));
        let product_auth = Arc::new(
            RebornProductAuthServices::from_shared(shared.clone(), dispatcher.clone())
                .with_flow_record_source(shared)
                .with_oauth_gate_registry(Arc::new(GoogleOAuthGateProviderRegistry::new(vec![
                    google_gate,
                ]))),
        );
        let state = ProductAuthRouteState::new(
            product_auth.clone(),
            TenantId::new("tenant-alpha").expect("tenant"),
            None,
            None,
        );
        let turn_scope = TurnScope::new(
            TenantId::new("tenant-alpha").expect("tenant"),
            None,
            None,
            ThreadId::new("thread-alpha").expect("thread"),
        );
        let owner_user_id = UserId::new("user-alpha").expect("user");
        let run_id = TurnRunId::new();
        let gate_ref = "gate:google-auth";
        let requirements = vec![RuntimeCredentialAuthRequirement {
            requirement: None,
            provider: RuntimeCredentialAccountProviderId::new("google").expect("provider"),
            requester_extension: ExtensionId::new("google-calendar").expect("extension"),
            provider_scopes: vec![GOOGLE_CALENDAR_READONLY_SCOPE.to_string()],
        }];

        let challenge = product_auth
            .challenge_for_gate(&turn_scope, &owner_user_id, run_id, gate_ref, &requirements)
            .await
            .expect("challenge lookup")
            .expect("google oauth challenge");
        let authorization_url = challenge.authorization_url.expect("authorization url");
        let state_value = Url::parse(authorization_url.as_str())
            .expect("authorization url")
            .query_pairs()
            .find_map(|(name, value)| (name == "state").then(|| value.into_owned()))
            .expect("oauth state");
        let encoded_state =
            url::form_urlencoded::byte_serialize(state_value.as_bytes()).collect::<String>();
        let encoded_scope =
            url::form_urlencoded::byte_serialize(GOOGLE_CALENDAR_READONLY_SCOPE.as_bytes())
                .collect::<String>();
        let uri = format!(
            "{GOOGLE_OAUTH_CALLBACK_PATH}?state={encoded_state}&code=google-auth-code&scope={encoded_scope}"
        )
        .parse::<Uri>()
        .expect("callback uri");

        let response = google_oauth_callback_handler(
            State(state),
            RawQuery(uri.query().map(str::to_string)),
            uri,
            HeaderMap::new(),
        )
        .await
        .expect("google callback");

        assert_eq!(response.status(), StatusCode::OK);
        let events = dispatcher.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].continuation,
            AuthContinuationRef::TurnGateResume {
                turn_run_ref: TurnRunRef::new(run_id.to_string()).expect("run ref"),
                gate_ref: AuthGateRef::new(gate_ref).expect("gate ref"),
            }
        );
    }

    #[tokio::test]
    async fn oauth_callback_rejects_dcr_state_with_mismatched_path_flow_id() {
        let shared = Arc::new(InMemoryAuthProductServices::new());
        let product_auth = Arc::new(RebornProductAuthServices::from_shared(
            shared,
            Arc::new(RecordingDispatcher::default()),
        ));
        let state = ProductAuthRouteState::new(
            product_auth,
            TenantId::new("tenant-alpha").expect("tenant"),
            None,
            None,
        );
        let state_flow_id = AuthFlowId::new();
        let path_flow_id = AuthFlowId::new();
        let scope = AuthProductScope::new(
            ResourceScope::local_default(
                UserId::new("user-alpha").expect("user"),
                InvocationId::new(),
            )
            .expect("scope"),
            AuthSurface::Callback,
        );
        let dcr_state = DcrOAuthCallbackState::new(
            state_flow_id,
            scope,
            AuthProviderId::new("notion").expect("provider"),
            CredentialAccountLabel::new("work notion").expect("label"),
            Vec::new(),
        )
        .encode()
        .expect("encoded DCR state");
        let encoded_state =
            url::form_urlencoded::byte_serialize(dcr_state.as_str().as_bytes()).collect::<String>();
        let uri = format!(
            "/api/reborn/product-auth/oauth/callback/{path_flow_id}?state={encoded_state}&code=notion-code"
        )
        .parse::<Uri>()
        .expect("callback uri");

        let error = oauth_callback_handler(
            State(state),
            Path(path_flow_id.to_string()),
            RawQuery(uri.query().map(str::to_string)),
            uri,
            HeaderMap::new(),
        )
        .await
        .expect_err("DCR state bound to another flow must be rejected");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.body.code, AuthErrorCode::MalformedCallback);
    }

    #[derive(Default)]
    struct RecordingDispatcher {
        events: Mutex<Vec<brassclaw_auth::AuthContinuationEvent>>,
    }

    impl RecordingDispatcher {
        fn events(&self) -> Vec<brassclaw_auth::AuthContinuationEvent> {
            self.events
                .lock()
                .expect("recording dispatcher lock")
                .clone()
        }
    }

    #[async_trait]
    impl RebornAuthContinuationDispatcher for RecordingDispatcher {
        async fn dispatch_auth_continuation(
            &self,
            event: brassclaw_auth::AuthContinuationEvent,
        ) -> Result<(), AuthProductError> {
            self.events
                .lock()
                .expect("recording dispatcher lock")
                .push(event);
            Ok(())
        }
    }
}
