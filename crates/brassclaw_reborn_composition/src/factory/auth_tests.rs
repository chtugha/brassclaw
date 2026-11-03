use crate::OAuthClientConfig;
use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthErrorCode, AuthFlowId, AuthFlowKind, AuthGateRef,
    AuthProductScope, AuthProviderId, AuthSessionId, AuthSurface, AuthorizationCodeHash,
    CredentialAccountLabel, InMemoryAuthProductServices, LifecyclePackageRef, NewAuthFlow,
    OAuthAuthorizationCode, OAuthAuthorizationUrl, OAuthClientId, OAuthProviderCallbackRequest,
    OAuthRedirectUri, OpaqueStateHash, PkceVerifierHash, PkceVerifierSecret, ProviderScope,
    TurnRunRef,
};
use brassclaw_capabilities::{CapabilityObligationHandler, CapabilityObligationRequest};
use brassclaw_host_api::{
    AgentId, InvocationId, ProjectId, ResourceScope, RuntimeHttpEgress, RuntimeHttpEgressError,
    RuntimeHttpEgressRequest, RuntimeHttpEgressResponse, TenantId, ThreadId, UserId,
};
use brassclaw_product_workflow::ProductAuthTurnGateResumeDispatcher;
use brassclaw_secrets::InMemorySecretStore;
use brassclaw_turns::{
    AcceptedMessageRef, BlockedReason, CancelRunRequest, CancelRunResponse, EventCursor, GateRef,
    GetRunStateRequest, IdempotencyKey, LoopCheckpointStateRef, ReplyTargetBindingRef,
    RunProfileId, RunProfileRequest, RunProfileVersion, SourceBindingRef, SubmitTurnRequest,
    SubmitTurnResponse, TurnActor, TurnCheckpointId, TurnCoordinator, TurnError, TurnId,
    TurnLeaseToken, TurnRunId, TurnRunState, TurnRunnerId, TurnScope, TurnStatus,
    runner::{BlockRunRequest, ClaimRunRequest, TurnRunTransitionPort},
};
use chrono::{Duration, Utc};
use secrecy::SecretString;
use std::sync::Mutex;

use super::*;
use crate::notion_oauth::{NOTION_PROVIDER_ID, notion_provider_spec};
use crate::oauth_provider_client::HostOAuthProviderClient;

#[derive(Clone)]
struct ErrorTurnCoordinator {
    resume_error: TurnError,
}

#[async_trait::async_trait]
impl TurnCoordinator for ErrorTurnCoordinator {
    async fn prepare_turn(&self, _scope: TurnScope) -> Result<TurnRunId, TurnError> {
        Ok(TurnRunId::new())
    }

    async fn submit_turn(
        &self,
        _request: SubmitTurnRequest,
    ) -> Result<SubmitTurnResponse, TurnError> {
        panic!("submit_turn is not used by auth continuation error mapping tests");
    }

    async fn resume_turn(
        &self,
        _request: brassclaw_turns::ResumeTurnRequest,
    ) -> Result<brassclaw_turns::ResumeTurnResponse, TurnError> {
        Err(self.resume_error.clone())
    }

    async fn cancel_run(&self, _request: CancelRunRequest) -> Result<CancelRunResponse, TurnError> {
        panic!("cancel_run is not used by auth continuation error mapping tests");
    }

    async fn get_run_state(&self, request: GetRunStateRequest) -> Result<TurnRunState, TurnError> {
        Ok(auth_error_mapping_run_state(&request))
    }
}

fn auth_error_mapping_run_state(request: &GetRunStateRequest) -> TurnRunState {
    TurnRunState {
        scope: request.scope.clone(),
        actor: Some(TurnActor::new(UserId::new("alice").unwrap())), // safety: fixed test user id literal is valid.
        turn_id: TurnId::new(),
        run_id: request.run_id,
        status: TurnStatus::BlockedAuth,
        accepted_message_ref: AcceptedMessageRef::new("message-auth-error").unwrap(), // safety: fixed test binding literal is valid.
        source_binding_ref: SourceBindingRef::new("source-auth-error").unwrap(), // safety: fixed test binding literal is valid.
        reply_target_binding_ref: ReplyTargetBindingRef::new("reply-auth-error").unwrap(), // safety: fixed test binding literal is valid.
        resolved_run_profile_id: RunProfileId::default_profile(),
        resolved_run_profile_version: RunProfileVersion::new(1),
        resolved_model_route: None,
        received_at: Utc::now(),
        checkpoint_id: None,
        gate_ref: Some(GateRef::new("gate:auth-error").unwrap()), // safety: fixed test gate literal is valid.
        credential_requirements: Vec::new(),
        failure: None,
        event_cursor: EventCursor::default(),
    }
}

#[derive(Debug, Default)]
struct NoopContinuationDispatcher;

#[async_trait::async_trait]
impl RebornAuthContinuationDispatcher for NoopContinuationDispatcher {
    async fn dispatch_auth_continuation(
        &self,
        _event: brassclaw_auth::AuthContinuationEvent,
    ) -> Result<(), brassclaw_auth::AuthProductError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct NoopObligationHandler;

#[async_trait::async_trait]
impl CapabilityObligationHandler for NoopObligationHandler {
    async fn satisfy(
        &self,
        _request: CapabilityObligationRequest<'_>,
    ) -> Result<(), brassclaw_capabilities::CapabilityObligationError> {
        Ok(())
    }
}

#[derive(Debug)]
struct RecordingOAuthEgress {
    response_body: Vec<u8>,
    requests: Mutex<Vec<RuntimeHttpEgressRequest>>,
}

impl RecordingOAuthEgress {
    fn ok(response_body: Vec<u8>) -> Self {
        Self {
            response_body,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn single_request(&self) -> RuntimeHttpEgressRequest {
        let requests = self
            .requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if requests.len() != 1 {
            panic!("expected exactly one OAuth egress request");
        }
        requests[0].clone()
    }
}

#[async_trait::async_trait]
impl RuntimeHttpEgress for RecordingOAuthEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);
        Ok(RuntimeHttpEgressResponse {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: self.response_body.clone(),
            saved_body: None,
            request_bytes: 0,
            response_bytes: 0,
            redaction_applied: true,
        })
    }
}

#[tokio::test]
async fn local_dev_oauth_turn_gate_callback_resumes_default_turn_coordinator() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-auth-owner", dir.path().join("local-dev"))
            .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
            .with_product_auth_ports(in_memory_product_auth_ports()),
    )
    .await
    .expect("local-dev services build");
    let product_auth = services.product_auth.as_ref().expect("product auth");
    let turn_coordinator = services
        .turn_coordinator
        .as_ref()
        .expect("turn coordinator");
    let local_runtime = services.local_runtime.as_ref().expect("local runtime");
    let scope = turn_scope();
    let actor = TurnActor::new(UserId::new("alice").unwrap());
    let submit = turn_coordinator
        .submit_turn(SubmitTurnRequest {
            scope: scope.clone(),
            actor: actor.clone(),
            accepted_message_ref: AcceptedMessageRef::new("message-auth-callback").unwrap(),
            source_binding_ref: SourceBindingRef::new("source-auth-callback").unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("reply-auth-callback").unwrap(),
            requested_run_profile: Some(RunProfileRequest::new("default").unwrap()),
            idempotency_key: IdempotencyKey::new("submit-auth-callback").unwrap(),
            received_at: Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .expect("submit turn");
    let SubmitTurnResponse::Accepted { run_id, .. } = submit;
    let runner_id = TurnRunnerId::new();
    let lease_token = TurnLeaseToken::new();
    local_runtime
        .turn_state
        .claim_next_run(ClaimRunRequest {
            runner_id,
            lease_token,
            scope_filter: Some(scope.clone()),
        })
        .await
        .expect("claim run")
        .expect("queued run exists");
    let gate_ref = brassclaw_turns::GateRef::new("gate:auth-callback").unwrap();
    local_runtime
        .turn_state
        .block_run(BlockRunRequest {
            run_id,
            runner_id,
            lease_token,
            checkpoint_id: TurnCheckpointId::new(),
            state_ref: LoopCheckpointStateRef::new("checkpoint:auth-callback").unwrap(),
            reason: BlockedReason::Auth {
                gate_ref: gate_ref.clone(),
                credential_requirements: Vec::new(),
            },
        })
        .await
        .expect("block auth gate");
    let auth_scope = auth_scope_for_turn(&scope, &actor);
    let flow = product_auth
        .flow_manager()
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: auth_scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::TurnGateResume {
                turn_run_ref: TurnRunRef::new(run_id.to_string()).unwrap(),
                gate_ref: AuthGateRef::new(gate_ref.as_str()).unwrap(),
            },
            update_binding: None,
            opaque_state_hash: Some(state_hash()),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect("auth flow");

    let response = product_auth
        .handle_oauth_callback(crate::RebornOAuthCallbackRequest {
            scope: auth_scope.clone(),
            flow_id: flow.id,
            opaque_state_hash: state_hash(),
            outcome: crate::RebornOAuthCallbackOutcome::Authorized {
                provider_request: OAuthProviderCallbackRequest {
                    provider: provider(),
                    account_label: label(),
                    authorization_code: OAuthAuthorizationCode::new(SecretString::from(
                        "raw-auth-code".to_string(),
                    ))
                    .unwrap(),
                    authorization_code_hash: code_hash("raw-auth-code"),
                    pkce_verifier: PkceVerifierSecret::new(SecretString::from(
                        "raw-pkce-verifier".to_string(),
                    ))
                    .unwrap(),
                    pkce_verifier_hash: pkce_hash("raw-pkce-verifier"),
                    scopes: vec![provider_scope("repo")],
                },
            },
        })
        .await
        .expect("oauth callback succeeds");

    assert_eq!(response.flow_id, flow.id);
    let state = turn_coordinator
        .get_run_state(GetRunStateRequest { scope, run_id })
        .await
        .expect("run state");
    assert_eq!(state.status, TurnStatus::Queued);
    assert_eq!(state.gate_ref, None);
    assert_eq!(state.source_binding_ref.as_str(), "source-auth-callback"); // safety: verifies fixed test binding literal is preserved.
    let reply_binding_ref = state.reply_target_binding_ref.as_str();
    assert_eq!(reply_binding_ref, "reply-auth-callback"); // safety: verifies fixed test binding literal is preserved.
}

#[tokio::test]
async fn local_dev_google_oauth_backend_builds_with_host_provider_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-google-oauth-owner", dir.path().join("local-dev"))
            .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
            .with_google_oauth_backend(OAuthClientConfig {
                client_id: OAuthClientId::new("google-client-123").expect("client id"),
                client_secret: None,
                redirect_uri: OAuthRedirectUri::new("https://app.example/oauth/google/callback")
                    .expect("redirect uri"),
                hosted_domain_hint: None,
            }),
    )
    .await
    .expect("local-dev services build");
    assert!(services.product_auth.is_some());
}

#[tokio::test]
async fn local_dev_notion_oauth_backend_builds_with_host_provider_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-notion-oauth-owner", dir.path().join("local-dev"))
            .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
            .with_google_oauth_backend(OAuthClientConfig {
                client_id: OAuthClientId::new("google-client-123").expect("client id"),
                client_secret: None,
                redirect_uri: OAuthRedirectUri::new("https://app.example/oauth/google/callback")
                    .expect("redirect uri"),
                hosted_domain_hint: None,
            })
            .with_notion_oauth_backend(OAuthClientConfig {
                client_id: OAuthClientId::new("notion-client-123").expect("client id"),
                client_secret: None,
                redirect_uri: OAuthRedirectUri::new("https://app.example/oauth/notion/callback")
                    .expect("redirect uri"),
                hosted_domain_hint: None,
            }),
    )
    .await
    .expect("local-dev services build");
    assert!(services.product_auth.is_some());
}

#[tokio::test]
async fn local_dev_notion_dcr_oauth_backend_builds_and_wires_registry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev(
            "local-dev-notion-dcr-oauth-owner",
            dir.path().join("local-dev"),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
        .with_notion_dcr_oauth_backend("http://127.0.0.1:3000", "Brassclaw")
        .expect("notion dcr config"),
    )
    .await
    .expect("local-dev services build");

    assert!(services.product_auth.is_some());
    assert!(
        services
            .product_auth
            .as_ref()
            .and_then(|product_auth| product_auth.as_auth_challenge_provider())
            .is_some(),
        "DCR-backed product auth must expose the challenge provider projection path"
    );
}

#[tokio::test]
async fn oauth_callback_exchanges_notion_through_reborn_product_auth_boundary() {
    let egress = Arc::new(RecordingOAuthEgress::ok(
        br#"{"access_token":"notion-access","refresh_token":"notion-refresh","expires_in":3600,"token_type":"Bearer"}"#.to_vec(),
    ));
    let provider_client = HostOAuthProviderClient::new(
        notion_provider_spec(),
        egress.clone(),
        Arc::new(InMemorySecretStore::new()),
        Arc::new(NoopObligationHandler),
        OAuthClientId::new("notion-client-123").expect("client id"),
        OAuthRedirectUri::new("https://app.example/oauth/notion/callback").expect("redirect uri"),
    )
    .expect("notion provider client");
    let services = RebornProductAuthServices::from_shared(
        Arc::new(InMemoryAuthProductServices::new()),
        Arc::new(NoopContinuationDispatcher),
    )
    .with_provider_client(Arc::new(provider_client));
    let auth_scope = auth_scope_for_turn(
        &turn_scope(),
        &TurnActor::new(UserId::new("alice").unwrap()),
    );
    let flow_id = create_notion_flow(
        &services,
        auth_scope.clone(),
        AuthContinuationRef::SetupOnly,
    )
    .await;

    let response = services
        .handle_oauth_callback(notion_authorized_request(auth_scope, flow_id))
        .await
        .expect("notion callback succeeds through product auth");

    assert_eq!(response.flow_id, flow_id);
    assert!(response.credential_account_id.is_some());
    let request = egress.single_request();
    assert_eq!(request.url, "https://mcp.notion.com/token");
    let body = form_params(&request.body);
    assert_eq!(
        body.get("grant_type").map(String::as_str),
        Some("authorization_code")
    );
    assert_eq!(
        body.get("resource").map(String::as_str),
        Some("https://mcp.notion.com/mcp")
    );
}

#[tokio::test]
async fn local_dev_google_oauth_backend_accepts_optional_client_secret_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev(
            "local-dev-google-oauth-secret-owner",
            dir.path().join("local-dev"),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
        .with_google_oauth_backend(OAuthClientConfig {
            client_id: OAuthClientId::new("google-client-123").expect("client id"),
            client_secret: Some(SecretString::from("raw-client-secret".to_string())),
            redirect_uri: OAuthRedirectUri::new("https://app.example/oauth/google/callback")
                .expect("redirect uri"),
            hosted_domain_hint: None,
        }),
    )
    .await
    .expect("local-dev services build");
    assert!(services.product_auth.is_some());
}

#[tokio::test]
async fn oauth_callback_with_stale_gate_maps_to_terminal_invalid_request() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-auth-stale-owner", dir.path().join("local-dev"))
            .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
            .with_product_auth_ports(in_memory_product_auth_ports()),
    )
    .await
    .expect("local-dev services build");
    let product_auth = services.product_auth.as_ref().expect("product auth");
    let turn_coordinator = services
        .turn_coordinator
        .as_ref()
        .expect("turn coordinator");
    let local_runtime = services.local_runtime.as_ref().expect("local runtime");
    let scope = turn_scope();
    let actor = TurnActor::new(UserId::new("alice").unwrap());
    let run_id = submit_and_block_auth_run(
        turn_coordinator.as_ref(),
        local_runtime.as_ref(),
        scope.clone(),
        actor.clone(),
        "gate:current-auth",
    )
    .await;
    let auth_scope = auth_scope_for_turn(&scope, &actor);
    let flow_id = create_flow(
        product_auth,
        auth_scope.clone(),
        AuthContinuationRef::TurnGateResume {
            turn_run_ref: TurnRunRef::new(run_id.to_string()).unwrap(),
            gate_ref: AuthGateRef::new("gate:stale-auth").unwrap(),
        },
    )
    .await;

    let error = product_auth
        .handle_oauth_callback(authorized_request(auth_scope, flow_id))
        .await
        .expect_err("stale auth gate should not resume");

    assert_eq!(error.code, AuthErrorCode::InvalidRequest);
    assert!(!error.retryable);
}

#[tokio::test]
async fn oauth_callback_with_lifecycle_activation_returns_ok_without_resume() {
    let dir = tempfile::tempdir().expect("tempdir");
    let services = build_reborn_services(
        RebornBuildInput::local_dev(
            "local-dev-auth-lifecycle-owner",
            dir.path().join("local-dev"),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().expect("local-dev policy"))
        .with_product_auth_ports(in_memory_product_auth_ports()),
    )
    .await
    .expect("local-dev services build");
    let product_auth = services.product_auth.as_ref().expect("product auth");
    let auth_scope = auth_scope_for_turn(
        &turn_scope(),
        &TurnActor::new(UserId::new("alice").unwrap()),
    );
    let continuation = AuthContinuationRef::LifecycleActivation {
        package_ref: LifecyclePackageRef::new("github-extension").unwrap(),
    };
    let flow_id = create_flow(product_auth, auth_scope.clone(), continuation.clone()).await;

    let response = product_auth
        .handle_oauth_callback(authorized_request(auth_scope, flow_id))
        .await
        .expect("lifecycle continuation is deferred");

    assert_eq!(response.flow_id, flow_id);
    assert_eq!(response.continuation, continuation);
}

#[tokio::test]
async fn oauth_callback_continuation_dispatch_maps_turn_error_categories() {
    for (turn_error, expected_code, expected_retryable) in [
        (
            TurnError::Unavailable {
                reason: "turn coordinator offline".to_string(),
            },
            AuthErrorCode::BackendUnavailable,
            true,
        ),
        (
            TurnError::Unauthorized,
            AuthErrorCode::CrossScopeDenied,
            false,
        ),
        (
            TurnError::ScopeNotFound,
            AuthErrorCode::UnknownOrExpiredFlow,
            false,
        ),
    ] {
        let coordinator = Arc::new(ErrorTurnCoordinator {
            resume_error: turn_error,
        });
        let services = RebornProductAuthServices::from_shared(
            Arc::new(InMemoryAuthProductServices::new()),
            Arc::new(ProductAuthTurnGateResumeDispatcher::new(coordinator)),
        );
        let scope = turn_scope();
        let actor = TurnActor::new(UserId::new("alice").unwrap());
        let auth_scope = auth_scope_for_turn(&scope, &actor);
        let flow_id = create_flow(
            &services,
            auth_scope.clone(),
            AuthContinuationRef::TurnGateResume {
                turn_run_ref: TurnRunRef::new(TurnRunId::new().to_string()).unwrap(),
                gate_ref: AuthGateRef::new("gate:auth-error").unwrap(),
            },
        )
        .await;

        let error = services
            .handle_oauth_callback(authorized_request(auth_scope, flow_id))
            .await
            .expect_err("continuation dispatch error should surface");

        assert_eq!(error.code, expected_code);
        assert_eq!(error.retryable, expected_retryable);
    }
}

#[cfg(test)]
fn turn_scope() -> TurnScope {
    TurnScope::new_with_owner(
        TenantId::new("tenant-auth").unwrap(),
        Some(AgentId::new("agent-auth").unwrap()),
        Some(ProjectId::new("project-auth").unwrap()),
        ThreadId::new("thread-auth").unwrap(),
        Some(UserId::new("alice").unwrap()), // safety: fixed test user id literal is valid.
    )
}

#[cfg(test)]
fn in_memory_product_auth_ports() -> RebornProductAuthServicePorts {
    RebornProductAuthServicePorts::from_shared(Arc::new(InMemoryAuthProductServices::new()))
}

#[cfg(test)]
fn auth_scope_for_turn(scope: &TurnScope, actor: &TurnActor) -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope {
            tenant_id: scope.tenant_id.clone(),
            user_id: actor.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            thread_id: Some(scope.thread_id.clone()),
            invocation_id: InvocationId::new(),
        },
        AuthSurface::Callback,
    )
    .with_session_id(AuthSessionId::new("session-auth-callback").unwrap())
}

#[cfg(test)]
fn provider() -> AuthProviderId {
    AuthProviderId::new("github").unwrap()
}

#[cfg(test)]
fn notion_provider() -> AuthProviderId {
    AuthProviderId::new(NOTION_PROVIDER_ID).unwrap()
}

#[cfg(test)]
fn label() -> CredentialAccountLabel {
    CredentialAccountLabel::new("work github").unwrap()
}

#[cfg(test)]
fn notion_label() -> CredentialAccountLabel {
    CredentialAccountLabel::new("work notion").unwrap()
}

#[cfg(test)]
fn authorization_url(value: &str) -> OAuthAuthorizationUrl {
    OAuthAuthorizationUrl::new(value).unwrap()
}

#[cfg(test)]
fn provider_scope(value: &str) -> ProviderScope {
    ProviderScope::new(value).unwrap()
}

#[cfg(test)]
async fn submit_and_block_auth_run(
    turn_coordinator: &dyn brassclaw_turns::TurnCoordinator,
    local_runtime: &RebornLocalRuntimeServices,
    scope: TurnScope,
    actor: TurnActor,
    gate_ref: &str,
) -> brassclaw_turns::TurnRunId {
    let submit = turn_coordinator
        .submit_turn(SubmitTurnRequest {
            scope: scope.clone(),
            actor,
            accepted_message_ref: AcceptedMessageRef::new("message-auth-callback-2").unwrap(),
            source_binding_ref: SourceBindingRef::new("source-auth-callback-2").unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("reply-auth-callback-2").unwrap(),
            requested_run_profile: Some(RunProfileRequest::new("default").unwrap()),
            idempotency_key: IdempotencyKey::new("submit-auth-callback-2").unwrap(),
            received_at: Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .expect("submit turn");
    let SubmitTurnResponse::Accepted { run_id, .. } = submit;
    let runner_id = TurnRunnerId::new();
    let lease_token = TurnLeaseToken::new();
    local_runtime
        .turn_state
        .claim_next_run(ClaimRunRequest {
            runner_id,
            lease_token,
            scope_filter: Some(scope),
        })
        .await
        .expect("claim run")
        .expect("queued run exists");
    local_runtime
        .turn_state
        .block_run(BlockRunRequest {
            run_id,
            runner_id,
            lease_token,
            checkpoint_id: TurnCheckpointId::new(),
            state_ref: LoopCheckpointStateRef::new("checkpoint:auth-callback-2").unwrap(),
            reason: BlockedReason::Auth {
                gate_ref: brassclaw_turns::GateRef::new(gate_ref).unwrap(),
                credential_requirements: Vec::new(),
            },
        })
        .await
        .expect("block auth gate");
    run_id
}

#[cfg(test)]
async fn create_flow(
    product_auth: &RebornProductAuthServices,
    scope: AuthProductScope,
    continuation: AuthContinuationRef,
) -> AuthFlowId {
    product_auth
        .flow_manager()
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope,
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation,
            update_binding: None,
            opaque_state_hash: Some(state_hash()),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect("auth flow")
        .id
}

async fn create_notion_flow(
    product_auth: &RebornProductAuthServices,
    scope: AuthProductScope,
    continuation: AuthContinuationRef,
) -> AuthFlowId {
    match product_auth
        .flow_manager()
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope,
            kind: AuthFlowKind::IntegrationCredential,
            provider: notion_provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://mcp.notion.com/oauth/authorize"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation,
            update_binding: None,
            opaque_state_hash: Some(state_hash()),
            pkce_verifier_hash: Some(pkce_hash("raw-notion-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
    {
        Ok(flow) => flow.id,
        Err(error) => panic!("notion auth flow failed: {error:?}"),
    }
}

#[cfg(test)]
fn authorized_request(
    scope: AuthProductScope,
    flow_id: AuthFlowId,
) -> crate::RebornOAuthCallbackRequest {
    crate::RebornOAuthCallbackRequest {
        scope,
        flow_id,
        opaque_state_hash: state_hash(),
        outcome: crate::RebornOAuthCallbackOutcome::Authorized {
            provider_request: OAuthProviderCallbackRequest {
                provider: provider(),
                account_label: label(),
                authorization_code: OAuthAuthorizationCode::new(SecretString::from(
                    "raw-auth-code".to_string(),
                ))
                .unwrap(),
                authorization_code_hash: code_hash("raw-auth-code"),
                pkce_verifier: PkceVerifierSecret::new(SecretString::from(
                    "raw-pkce-verifier".to_string(),
                ))
                .unwrap(),
                pkce_verifier_hash: pkce_hash("raw-pkce-verifier"),
                scopes: vec![provider_scope("repo")],
            },
        },
    }
}

fn notion_authorized_request(
    scope: AuthProductScope,
    flow_id: AuthFlowId,
) -> crate::RebornOAuthCallbackRequest {
    crate::RebornOAuthCallbackRequest {
        scope,
        flow_id,
        opaque_state_hash: state_hash(),
        outcome: crate::RebornOAuthCallbackOutcome::Authorized {
            provider_request: OAuthProviderCallbackRequest {
                provider: notion_provider(),
                account_label: notion_label(),
                authorization_code: OAuthAuthorizationCode::new(SecretString::from(
                    "raw-notion-auth-code".to_string(),
                ))
                .unwrap(), // safety: test-only fixture literal is valid by construction.
                authorization_code_hash: code_hash("raw-notion-auth-code"),
                pkce_verifier: PkceVerifierSecret::new(SecretString::from(
                    "raw-notion-pkce-verifier".to_string(),
                ))
                .unwrap(), // safety: test-only fixture literal is valid by construction.
                pkce_verifier_hash: pkce_hash("raw-notion-pkce-verifier"),
                scopes: vec![provider_scope("workspace")],
            },
        },
    }
}

#[cfg(test)]
fn state_hash() -> OpaqueStateHash {
    brassclaw_auth::opaque_state_hash("factory-auth-state-fixture").unwrap()
}

#[cfg(test)]
fn pkce_hash(raw: &str) -> PkceVerifierHash {
    let verifier = PkceVerifierSecret::new(SecretString::from(raw.to_owned())).unwrap();
    brassclaw_auth::pkce_verifier_hash(&verifier).unwrap()
}

#[cfg(test)]
fn code_hash(raw: &str) -> AuthorizationCodeHash {
    let code = OAuthAuthorizationCode::new(SecretString::from(raw.to_owned())).unwrap();
    brassclaw_auth::authorization_code_hash(&code).unwrap()
}

fn form_params(body: &[u8]) -> std::collections::BTreeMap<String, String> {
    url::form_urlencoded::parse(body).into_owned().collect()
}

#[cfg(all(
    feature = "postgres",
    feature = "skills-db",
    feature = "root-llm-provider"
))]
#[tokio::test]
async fn pg_factory_pending_auth_custody_exposes_original_recovery_then_retries_native_cutover() {
    use axum::{
        Extension,
        body::{Body, to_bytes},
        http::Request,
    };
    use brassclaw_auth::{
        AuthFlowManager, AuthFlowStatus, CredentialAccountService, CredentialAccountStatus,
        CredentialOwnership, NewCredentialAccount, OAuthCallbackAdmission, OAuthCallbackOriginal,
        OAuthCallbackPrepared, OAuthTokenDestinations,
    };
    use brassclaw_host_api::SecretHandle;
    use tower::ServiceExt;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let home = tempfile::tempdir().unwrap();
    let storage = home.path().join("db");
    std::fs::create_dir_all(&storage).unwrap();
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    crate::pg_auth_product_services::run_auth_migrations(&rig.pool)
        .await
        .unwrap();
    let secrets = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            resolve_local_dev_secret_master_key(&storage).unwrap(),
            "reborn-cli",
        )
        .unwrap(),
    );
    let filesystem = crate::wrap_scoped(root);
    let old = Arc::new(FilesystemAuthProductServices::new(
        filesystem.clone(),
        secrets.clone(),
    ));
    let native_candidate =
        crate::pg_auth_product_services::PgAuthProductServices::new(rig.pool.clone(), secrets);
    let scope = AuthProductScope::new(
        ResourceScope {
            tenant_id: TenantId::new("reborn-cli").unwrap(),
            user_id: UserId::new("recovery-installer").unwrap(),
            agent_id: None,
            project_id: None,
            thread_id: None,
            invocation_id: InvocationId::new(),
        },
        AuthSurface::SetupAdmin,
    );
    let flow = old
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash()),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let crate::RebornOAuthCallbackOutcome::Authorized { provider_request } =
        authorized_request(scope.clone(), flow.id).outcome
    else {
        panic!("callback input")
    };
    let original = OAuthCallbackOriginal::from_request(
        scope.clone(),
        flow.id,
        state_hash(),
        &provider_request,
    )
    .unwrap();
    let reference = original.reference();
    let admission = old
        .claim_oauth_callback(
            &scope,
            OAuthCallbackPrepared {
                original,
                destinations: OAuthTokenDestinations {
                    access_secret: SecretHandle::new("recovery-original-access").unwrap(),
                    refresh_secret: SecretHandle::new("recovery-original-refresh").unwrap(),
                },
            },
        )
        .await
        .unwrap();
    let OAuthCallbackAdmission::Dispatch(original) = admission else {
        panic!("actual original FS claim")
    };
    assert_eq!(original.flow.status, AuthFlowStatus::Completing);
    // No provider is contacted: this is a durable unknown/pending original,
    // never a synthetic successful exchange or permission to submit it again.
    let input = || {
        RebornBuildInput::postgres_with_reborn_home(
            "recovery-owner",
            rig.pool.as_ref().clone(),
            rig.url.clone(),
            home.path().to_path_buf(),
        )
        .with_runtime_policy(crate::local_dev_runtime_policy().unwrap())
        .with_runtime_process_binding(crate::RebornRuntimeProcessBinding::None)
    };
    // This source account sorts AFTER the open original's user/path. Pending
    // must never short-circuit inspection of its native identity/index conflict.
    let mut later_scope = scope.clone();
    later_scope.resource.user_id = UserId::new("zz-later-owner").unwrap();
    let new_account = || NewCredentialAccount {
        scope: later_scope.clone(),
        provider: provider(),
        label: label(),
        status: CredentialAccountStatus::PendingSetup,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: None,
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![],
    };
    let later = old.create_account(new_account()).await.unwrap();
    let json = serde_json::to_value(&later).unwrap();
    rig.pool.get().await.unwrap().execute(
        "INSERT INTO brassclaw_product_auth_accounts (tenant_id,id,user_id,data) VALUES ($1,$2,'wrong-native-owner',$3)",
        &[&"reborn-cli", &later.id.to_string(), &json],
    ).await.unwrap();
    assert!(matches!(
        build_reborn_services(input().with_tenant_id("reborn-cli")).await,
        Err(RebornBuildError::InvalidConfig { .. })
    ));
    let conflicting: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT user_id FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
            &[&"reborn-cli", &later.id.to_string()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(conflicting, "wrong-native-owner");
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
            &[&"reborn-cli", &later.id.to_string()],
        )
        .await
        .unwrap();
    // A genuine native-only account cannot disappear behind a global FS owner.
    let native_only = native_candidate
        .create_account(new_account())
        .await
        .unwrap();
    assert!(matches!(
        build_reborn_services(input().with_tenant_id("reborn-cli")).await,
        Err(RebornBuildError::InvalidConfig { .. })
    ));
    assert!(
        native_candidate
            .get_account(brassclaw_auth::CredentialAccountLookupRequest::new(
                later_scope.clone(),
                native_only.id
            ))
            .await
            .unwrap()
            .is_some()
    );
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
            &[&"reborn-cli", &native_only.id.to_string()],
        )
        .await
        .unwrap();
    // Nor can a native-only flow be hidden, even without any native account.
    let native_flow = native_candidate
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: later_scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash()),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    assert!(matches!(
        build_reborn_services(input().with_tenant_id("reborn-cli")).await,
        Err(RebornBuildError::InvalidConfig { .. })
    ));
    assert!(
        native_candidate
            .get_flow(&later_scope, native_flow.id)
            .await
            .unwrap()
            .is_some()
    );
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2",
            &[&"reborn-cli", &native_flow.id.to_string()],
        )
        .await
        .unwrap();
    assert!(rig.pool.get().await.unwrap().query_opt(
        "SELECT tenant_id FROM brassclaw_product_auth_filesystem_recoveries WHERE tenant_id=$1", &[&"reborn-cli"],
    ).await.unwrap().is_none(), "integrity/custody errors never grant recovery ownership");
    // With no native rows at all, a malformed later original must still fail;
    // the early open flow is not permission to stop the source inventory.
    let later_path = crate::product_auth_durable::paths::account_path(&later_scope, later.id);
    let source_account = filesystem
        .get(&later_scope.resource, &later_path)
        .await
        .unwrap()
        .unwrap();
    filesystem
        .put(
            &later_scope.resource,
            &later_path,
            brassclaw_filesystem::Entry::bytes(b"malformed later original".to_vec()),
            brassclaw_filesystem::CasExpectation::Any,
        )
        .await
        .unwrap();
    assert!(matches!(
        build_reborn_services(input().with_tenant_id("reborn-cli")).await,
        Err(RebornBuildError::InvalidConfig { .. })
    ));
    assert_eq!(
        filesystem
            .get(&later_scope.resource, &later_path)
            .await
            .unwrap()
            .unwrap()
            .entry
            .body,
        b"malformed later original"
    );
    assert!(rig.pool.get().await.unwrap().query_opt(
        "SELECT tenant_id FROM brassclaw_product_auth_filesystem_recoveries WHERE tenant_id=$1", &[&"reborn-cli"],
    ).await.unwrap().is_none());
    filesystem
        .put(
            &later_scope.resource,
            &later_path,
            brassclaw_filesystem::Entry::bytes(source_account.entry.body),
            brassclaw_filesystem::CasExpectation::Any,
        )
        .await
        .unwrap();
    drop(old);
    let services = build_reborn_services(input())
        .await
        .expect("open original must not strand startup");
    let auth = services.product_auth.as_ref().unwrap().clone();
    assert_eq!(
        auth.custody_status(),
        crate::RebornAuthCustodyStatus {
            state: crate::RebornAuthCustodyState::FilesystemRecoveryPending,
            native_codex_available: false,
            retry_cutover_on_next_start: true
        }
    );
    assert!(matches!(
        auth.native_codex_owner(),
        Err(AuthProductError::BackendUnavailable)
    ));
    // A stale/lower native facade cannot race a fresh native-only record into
    // the namespace after the exclusive FS owner is published.
    assert!(
        native_candidate
            .create_account(new_account())
            .await
            .is_err()
    );
    for table in [
        "brassclaw_product_auth_accounts",
        "brassclaw_product_auth_flows",
        "brassclaw_product_auth_interactions",
        "brassclaw_product_auth_requirement_selections",
    ] {
        assert!(rig.pool.get().await.unwrap().execute(
            &format!("INSERT INTO {table} (tenant_id,id,user_id,data) VALUES ($1,'blocked-native-write','blocked','{{}}'::jsonb)"),
            &[&"reborn-cli"],
        ).await.is_err());
    }
    assert!(rig.pool.get().await.unwrap().execute(
        "INSERT INTO brassclaw_product_auth_refreshes (tenant_id,scope_ref,id,account_id,blocks_refresh,data) VALUES ($1,'blocked','blocked','blocked',false,'{}'::jsonb)",
        &[&"reborn-cli"],
    ).await.is_err());
    let native_count: i64 = rig.pool.get().await.unwrap().query_one(
        "SELECT (SELECT count(*) FROM brassclaw_product_auth_accounts WHERE tenant_id=$1) + (SELECT count(*) FROM brassclaw_product_auth_flows WHERE tenant_id=$1) + (SELECT count(*) FROM brassclaw_product_auth_interactions WHERE tenant_id=$1) + (SELECT count(*) FROM brassclaw_product_auth_requirement_selections WHERE tenant_id=$1) + (SELECT count(*) FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1)",
        &[&"reborn-cli"],
    ).await.unwrap().get(0);
    assert_eq!(
        native_count, 0,
        "pending commits no transient native imports"
    );
    assert!(
        crate::PgProviderRepo::new(rig.pool.as_ref().clone(), "reborn-cli")
            .get_revision("openai", false)
            .await
            .unwrap()
            .is_some(),
        "unrelated configured provider catalogue remains available"
    );
    let returned = auth
        .flow_manager()
        .reconcile_oauth_callback(&scope, &reference)
        .await
        .unwrap();
    assert_eq!(
        returned.action, original.action,
        "existing original read/reconciliation must remain available without a new issuer claim"
    );
    assert_eq!(
        returned.action.receipt().unwrap().outcome,
        brassclaw_auth::OAuthCallbackOutcomeKind::Unconfirmed
    );
    assert!(rig.pool.get().await.unwrap().query_opt("SELECT tenant_id FROM brassclaw_product_auth_filesystem_cutovers WHERE tenant_id=$1",&[&"reborn-cli"]).await.unwrap().is_none());
    // Actual Auth route projection, using an already authenticated caller. The
    // separate route contract test covers bearer and NoBody middleware.
    let mounted = crate::product_auth_serve::product_auth_route_mount(
        crate::product_auth_serve::ProductAuthRouteState::new(
            auth.clone(),
            scope.resource.tenant_id.clone(),
            None,
            None,
        ),
    );
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        scope.resource.tenant_id.clone(),
        scope.resource.user_id.clone(),
        None,
        None,
    );
    let response = mounted
        .protected
        .layer(Extension(caller))
        .oneshot(
            Request::builder()
                .uri(crate::product_auth_serve::AUTH_CUSTODY_PATH)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let visible: crate::RebornAuthCustodyStatus =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(visible, auth.custody_status());
    // Explicit cancellation is a real terminal disposition, not successful
    // authorization and not a claim that the unknown exchange had no effect.
    auth.flow_manager()
        .cancel_flow(&scope, flow.id)
        .await
        .unwrap();
    drop(auth);
    drop(services);
    let restarted = build_reborn_services(input().with_tenant_id("reborn-cli"))
        .await
        .unwrap();
    let native = restarted.product_auth.as_ref().unwrap();
    assert_eq!(
        native.custody_status().state,
        crate::RebornAuthCustodyState::Native
    );
    assert!(native.native_codex_owner().is_ok());
    assert!(rig.pool.get().await.unwrap().query_opt(
        "SELECT tenant_id FROM brassclaw_product_auth_filesystem_recoveries WHERE tenant_id=$1", &[&"reborn-cli"],
    ).await.unwrap().is_none());
    assert_eq!(
        native_candidate
            .get_account(brassclaw_auth::CredentialAccountLookupRequest::new(
                later_scope.clone(),
                later.id
            ))
            .await
            .unwrap(),
        Some(later)
    );
    native_candidate
        .create_account(new_account())
        .await
        .expect("native writer fence released only by committed cutover");
    let retained = native
        .flow_manager()
        .read_oauth_callback_original(&scope, &reference)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.flow.status, AuthFlowStatus::Canceled);
    assert_eq!(
        retained.action, original.action,
        "unknown original custody is retained after cancellation and cutover"
    );
}
