use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthFlowKind, AuthGateRef, AuthProductScope,
    AuthProviderId, AuthSessionId, AuthSurface, InMemoryAuthProductServices, NewAuthFlow,
    OAuthAuthorizationUrl, OpaqueStateHash, PkceVerifierHash, TurnRunRef,
};
use brassclaw_host_api::runtime_policy::{
    ApprovalPolicy, AuditMode, DeploymentMode, EffectiveRuntimePolicy, FilesystemBackendKind,
    NetworkMode, ProcessBackendKind, RuntimeProfile, SecretMode,
};
use brassclaw_host_api::{InvocationId, ResourceScope, UserId};
use brassclaw_loop_support::{
    HostManagedModelError, HostManagedModelGateway, HostManagedModelRequest,
    HostManagedModelResponse,
};
use brassclaw_product_workflow::{
    AuthInteractionRejectionKind, ListPendingAuthInteractionsRequest, ProductWorkflowError,
};
use brassclaw_turns::runner::{BlockRunRequest, ClaimRunRequest, TurnRunTransitionPort};
use brassclaw_turns::{
    AcceptedMessageRef, AllowAllTurnAdmissionPolicy, BlockedReason, GateRef, IdempotencyKey,
    InMemoryRunProfileResolver, LoopCheckpointStateRef, ReplyTargetBindingRef, RunProfileRequest,
    SourceBindingRef, SubmitTurnRequest, SubmitTurnResponse, TurnActor, TurnCheckpointId,
    TurnLeaseToken, TurnRunId, TurnRunnerId, TurnScope, TurnStateStore,
};
use chrono::Utc;

use crate::runtime_input::{RebornRuntimeIdentity, RebornRuntimeInput, TurnRunnerSettings};
use crate::{RebornProductAuthServicePorts, RebornRuntimeProcessBinding};

use super::{RebornRuntime, build_reborn_runtime};

#[cfg(feature = "skills-db")]
#[path = "manual_token_browser.rs"]
mod manual_token_browser;

#[derive(Debug)]
struct UnusedModelGateway;

#[async_trait]
impl HostManagedModelGateway for UnusedModelGateway {
    async fn stream_model(
        &self,
        _request: HostManagedModelRequest,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        Ok(HostManagedModelResponse::assistant_reply(
            "unused auth interaction test reply".to_string(),
        ))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_dev_runtime_auth_interactions_use_flow_record_source() {
    let root = tempfile::tempdir().expect("tempdir");
    let rig = super::test_pg::pg_rig().await;
    let _db_guard = rig.lock_db().await;
    let runtime = build_runtime(&rig, "auth-read-model-present", root.path(), None)
        .await
        .expect("runtime builds");
    super::test_pg::stop_worker_for_state_fixture(&runtime).await;
    let conversation = runtime.new_conversation().await.expect("conversation");
    let subject_user_id = UserId::new("team-agent-user").expect("subject user id");
    let scope = TurnScope::new_with_owner(
        runtime.thread_scope.tenant_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
        conversation.0,
        Some(subject_user_id.clone()),
    );
    let actor = TurnActor::new(runtime.actor_user_id.clone());
    let gate_ref = GateRef::new("gate:runtime-auth-read-model").expect("gate");
    let run_id = submit_and_block_auth_run(
        &runtime,
        scope.clone(),
        actor.clone(),
        &gate_ref,
        Vec::new(),
    )
    .await;
    create_auth_flow(&runtime, &scope, &actor, run_id, &gate_ref).await;

    let pending = runtime
        .webui_auth_interaction_service()
        .list_pending(ListPendingAuthInteractionsRequest {
            scope: scope.clone(),
            actor: actor.clone(),
        })
        .await
        .expect("pending auth interactions");

    assert_eq!(pending.auth_interactions.len(), 1);
    let view = &pending.auth_interactions[0];
    assert_eq!(view.scope.tenant_id, scope.tenant_id);
    assert_eq!(view.scope.user_id, subject_user_id);
    assert_eq!(view.scope.thread_id, scope.thread_id);
    assert_eq!(view.run_id, run_id);
    assert_eq!(view.auth_request_ref, gate_ref);

    #[cfg(feature = "skills-db")]
    let runtime = {
        let runtime = Arc::new(runtime);
        manual_token_browser::run_if_requested(runtime.clone()).await;
        Arc::try_unwrap(runtime).unwrap_or_else(|_| panic!("browser runtime owners released"))
    };
    runtime.shutdown().await.expect("runtime shutdown");
}

#[cfg(feature = "root-llm-provider")]
#[tokio::test]
async fn pg_factory_explicit_tenant_reuses_native_auth_owner() {
    let root = tempfile::tempdir().expect("tempdir");
    let rig = super::test_pg::pg_rig().await;
    let services = crate::build_reborn_services(
        rig.build_input("auth-explicit-tenant", root.path())
            .with_tenant_id("auth-explicit-tenant")
            .with_runtime_policy(local_dev_runtime_policy())
            .with_runtime_process_binding(RebornRuntimeProcessBinding::None),
    )
    .await
    .expect("explicit-tenant factory builds");
    let auth = services.product_auth.as_ref().expect("product auth");
    let native = auth
        .native_codex_owner()
        .expect("explicit-tenant factory attaches native owner");
    let original: Arc<dyn brassclaw_auth::AuthFlowManager> = native;
    assert!(Arc::ptr_eq(&auth.flow_manager(), &original));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_dev_runtime_auth_interactions_are_unavailable_without_flow_record_source() {
    let auth = Arc::new(InMemoryAuthProductServices::new());
    let ports = RebornProductAuthServicePorts::from_shared(auth);
    let root = tempfile::tempdir().expect("tempdir");
    let rig = super::test_pg::pg_rig().await;
    let _db_guard = rig.lock_db().await;
    let runtime = build_runtime(&rig, "auth-read-model-absent", root.path(), Some(ports))
        .await
        .expect("runtime builds");
    #[cfg(feature = "root-llm-provider")]
    assert!(matches!(
        runtime
            .services
            .product_auth
            .as_ref()
            .expect("product auth")
            .native_codex_owner(),
        Err(brassclaw_auth::AuthProductError::BackendUnavailable)
    ));
    assert!(
        runtime
            .services
            .product_auth
            .as_ref()
            .expect("product auth")
            .flow_record_source()
            .is_none(),
        "custom product-auth ports intentionally do not imply a WebUI read projection"
    );
    let conversation = runtime.new_conversation().await.expect("conversation");
    let scope = TurnScope::new(
        runtime.thread_scope.tenant_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
        conversation.0,
    );

    let error = runtime
        .webui_auth_interaction_service()
        .list_pending(ListPendingAuthInteractionsRequest {
            scope,
            actor: TurnActor::new(runtime.actor_user_id.clone()),
        })
        .await
        .expect_err("auth interaction read model is unavailable");

    assert!(matches!(
        error,
        ProductWorkflowError::AuthInteractionRejected {
            kind: AuthInteractionRejectionKind::FlowUnavailable
        }
    ));

    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_completion_resumes_the_runtime_coordinator() {
    use crate::{RebornManualTokenSetupRequest, RebornManualTokenSubmitRequest};
    use brassclaw_auth::{CredentialAccountLabel, ManualTokenSetupIdentity};
    use brassclaw_turns::{GetRunStateRequest, TurnStatus};

    let root = tempfile::tempdir().expect("tempdir");
    let rig = super::test_pg::pg_rig().await;
    let _db_guard = rig.lock_db().await;
    let runtime = build_runtime(&rig, "auth-native-runtime-resume", root.path(), None)
        .await
        .expect("runtime builds");
    super::test_pg::stop_worker_for_state_fixture(&runtime).await;
    let conversation = runtime.new_conversation().await.expect("conversation");
    let scope = TurnScope::new_with_owner(
        runtime.thread_scope.tenant_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
        conversation.0,
        Some(runtime.actor_user_id.clone()),
    );
    let actor = TurnActor::new(runtime.actor_user_id.clone());
    let gate = GateRef::new("gate:native-runtime-auth-resume").unwrap();
    let run =
        submit_and_block_auth_run(&runtime, scope.clone(), actor.clone(), &gate, Vec::new()).await;
    let auth_scope = auth_scope_for_turn(&scope, &actor);
    let auth = runtime.services.product_auth.as_ref().unwrap();
    let identity = ManualTokenSetupIdentity::default();
    auth.request_original_manual_token_setup(
        identity,
        RebornManualTokenSetupRequest::new(
            auth_scope.clone(),
            AuthProviderId::new("native-runtime-provider").unwrap(),
            CredentialAccountLabel::new("Native runtime continuation").unwrap(),
            AuthContinuationRef::TurnGateResume {
                turn_run_ref: TurnRunRef::new(run.to_string()).unwrap(),
                gate_ref: AuthGateRef::new(gate.as_str()).unwrap(),
            },
            Utc::now() + chrono::Duration::minutes(5),
        ),
    )
    .await
    .unwrap();
    let submitted = auth
        .submit_original_manual_token(
            identity,
            RebornManualTokenSubmitRequest::new(
                auth_scope.clone(),
                identity.interaction_id,
                secrecy::SecretString::from("native-runtime-test-token-sentinel".to_owned()),
            ),
        )
        .await
        .expect("completed credentials resume through the actual runtime coordinator");
    let actual = runtime
        .webui_turn_coordinator()
        .get_run_state(GetRunStateRequest {
            scope: scope.clone(),
            run_id: run,
        })
        .await
        .unwrap();
    assert_eq!(actual.status, TurnStatus::Queued);
    assert!(actual.gate_ref.is_none());
    let flow = auth
        .flow_manager()
        .get_flow(&auth_scope, identity.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(flow.credential_account_id, Some(submitted.account_id));
    assert!(flow.continuation_emitted_at.is_some());
    // Reference replay reads the original result and does not create another run.
    let replay = auth
        .continue_original_manual_token(&auth_scope, identity)
        .await
        .unwrap();
    assert_eq!(replay.account_id, submitted.account_id);
    let after = runtime
        .webui_turn_coordinator()
        .get_run_state(GetRunStateRequest { scope, run_id: run })
        .await
        .unwrap();
    assert_eq!(after, actual);
    runtime.shutdown().await.expect("runtime shutdown");
}

async fn build_runtime(
    rig: &super::test_pg::PgRig,
    owner: &str,
    reborn_home: &std::path::Path,
    product_auth_ports: Option<RebornProductAuthServicePorts>,
) -> Result<RebornRuntime, super::RebornRuntimeError> {
    let mut services = rig
        .build_input(owner, reborn_home)
        .with_runtime_policy(local_dev_runtime_policy())
        .with_runtime_process_binding(RebornRuntimeProcessBinding::None);
    if let Some(ports) = product_auth_ports {
        services = services.with_product_auth_ports(ports);
    }
    build_reborn_runtime(
        RebornRuntimeInput::from_services(services)
            .with_identity(RebornRuntimeIdentity {
                tenant_id: format!("{owner}-tenant"),
                agent_id: format!("{owner}-agent"),
                source_binding_id: format!("{owner}-source"),
                reply_target_binding_id: format!("{owner}-reply"),
            })
            .with_runner_settings(TurnRunnerSettings {
                heartbeat_interval: Duration::from_secs(60),
                poll_interval: Duration::from_secs(60),
            })
            .with_model_gateway_override(Arc::new(UnusedModelGateway)),
    )
    .await
}

async fn submit_and_block_auth_run(
    runtime: &RebornRuntime,
    scope: TurnScope,
    actor: TurnActor,
    gate_ref: &GateRef,
    credential_requirements: Vec<brassclaw_host_api::RuntimeCredentialAuthRequirement>,
) -> TurnRunId {
    let turn_state = brassclaw_turns::PgTurnStateStore::new(
        Arc::clone(
            runtime
                .services
                .pg_pool
                .as_ref()
                .expect("runtime PostgreSQL pool"),
        ),
        scope.tenant_id.as_str(),
    );
    let admission = AllowAllTurnAdmissionPolicy;
    let profiles = InMemoryRunProfileResolver::default();
    let submit = turn_state
        .submit_turn(
            SubmitTurnRequest {
                scope: scope.clone(),
                actor,
                accepted_message_ref: AcceptedMessageRef::new(format!(
                    "message-{}",
                    gate_ref.as_str()
                ))
                .expect("message ref"),
                source_binding_ref: SourceBindingRef::new("source-runtime-auth-read-model")
                    .expect("source ref"),
                reply_target_binding_ref: ReplyTargetBindingRef::new(
                    "reply-runtime-auth-read-model",
                )
                .expect("reply ref"),
                requested_run_profile: Some(RunProfileRequest::new("default").expect("profile")),
                idempotency_key: IdempotencyKey::new(format!("submit-{}", gate_ref.as_str()))
                    .expect("idempotency key"),
                received_at: Utc::now(),
                requested_run_id: None,
                parent_run_id: None,
                subagent_depth: 0,
                spawn_tree_root_run_id: None,
            },
            &admission,
            &profiles,
        )
        .await
        .expect("submit turn through the runtime durable turn state");
    let SubmitTurnResponse::Accepted { run_id, .. } = submit;
    let runner_id = TurnRunnerId::new();
    let lease_token = TurnLeaseToken::new();
    turn_state
        .claim_next_run(ClaimRunRequest {
            runner_id,
            lease_token,
            scope_filter: Some(scope),
        })
        .await
        .expect("claim run")
        .expect("queued run exists");
    turn_state
        .block_run(BlockRunRequest {
            run_id,
            runner_id,
            lease_token,
            checkpoint_id: TurnCheckpointId::new(),
            state_ref: LoopCheckpointStateRef::new("checkpoint:runtime-auth-read-model")
                .expect("checkpoint ref"),
            reason: BlockedReason::Auth {
                gate_ref: gate_ref.clone(),
                credential_requirements,
            },
        })
        .await
        .expect("block auth run");
    run_id
}

async fn create_auth_flow(
    runtime: &RebornRuntime,
    scope: &TurnScope,
    actor: &TurnActor,
    run_id: TurnRunId,
    gate_ref: &GateRef,
) {
    runtime
        .services
        .product_auth
        .as_ref()
        .expect("product auth")
        .flow_manager()
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: auth_scope_for_turn(scope, actor),
            kind: AuthFlowKind::IntegrationCredential,
            provider: AuthProviderId::new("github").expect("provider"),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .expect("authorization url"),
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            },
            continuation: AuthContinuationRef::TurnGateResume {
                turn_run_ref: TurnRunRef::new(run_id.to_string()).expect("turn run ref"),
                gate_ref: AuthGateRef::new(gate_ref.as_str()).expect("auth gate ref"),
            },
            update_binding: None,
            opaque_state_hash: Some(state_hash()),
            pkce_verifier_hash: Some(pkce_hash()),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
        })
        .await
        .expect("auth flow");
}

fn auth_scope_for_turn(scope: &TurnScope, actor: &TurnActor) -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope
                .explicit_owner_user_id()
                .cloned()
                .unwrap_or_else(|| actor.user_id.clone()),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            thread_id: Some(scope.thread_id.clone()),
            invocation_id: InvocationId::new(),
        },
        AuthSurface::Web,
    )
    .with_session_id(AuthSessionId::new("session-runtime-auth-read-model").expect("session id"))
}

fn state_hash() -> OpaqueStateHash {
    OpaqueStateHash::new(fake_digest("state-hash")).expect("state hash")
}

fn pkce_hash() -> PkceVerifierHash {
    PkceVerifierHash::new(fake_digest("pkce-hash")).expect("pkce hash")
}

fn fake_digest(value: &str) -> String {
    format!(
        "{:064x}",
        value.bytes().fold(0_u64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u64::from(byte))
        })
    )
}

fn local_dev_runtime_policy() -> EffectiveRuntimePolicy {
    EffectiveRuntimePolicy {
        deployment: DeploymentMode::LocalSingleUser,
        requested_profile: RuntimeProfile::LocalDev,
        resolved_profile: RuntimeProfile::LocalDev,
        filesystem_backend: FilesystemBackendKind::HostWorkspace,
        process_backend: ProcessBackendKind::LocalHost,
        network_mode: NetworkMode::DirectLogged,
        secret_mode: SecretMode::ScrubbedEnv,
        approval_policy: ApprovalPolicy::AskDestructive,
        audit_mode: AuditMode::LocalMinimal,
    }
}
