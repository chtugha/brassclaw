//! Actual model-call adapter, native JSON primitive, kernel and durable result
//! path. This does not certify ordinary factory enrollment of every Tool.
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
    ToolExecutionRules,
};
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_filesystem::LocalFilesystem;
use brassclaw_host_api::runtime_policy::*;
use brassclaw_host_api::{
    CapabilityGrant, CapabilityGrantId, CapabilityId, CapabilitySet, DispatchAuthorityModel,
    EffectKind, ExecutionContext, ExtensionId, GrantConstraints, MountView, NetworkPolicy,
    PackageId, Principal, RuntimeKind, ThreadId, TrustClass, UserId,
};
use brassclaw_host_runtime::{
    CapabilitySurfacePolicy, CapabilitySurfaceVersion, HostRuntime, HostRuntimeServices,
    JSON_CAPABILITY_ID, SurfaceKind, VisibleCapabilityRequest as HostVisibleRequest,
    builtin_first_party_handlers, builtin_first_party_package,
};
use brassclaw_loop_support::{HostRuntimeLoopCapabilityPortFactory, PgCapabilityIo};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_triggers::InMemoryTriggerRepository;
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use brassclaw_turns::{
    TurnId, TurnRunId, TurnScope,
    run_profile::{
        CapabilityInvocation, CapabilityOutcome, InMemoryLoopHostMilestoneSink,
        InMemoryRunProfileResolver, LoopRunContext, ProviderToolCall, RunProfileResolutionRequest,
        RunProfileResolver, VisibleCapabilityRequest,
    },
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn native_model_tool_adapter_uses_instance_policy_without_scoped_grants() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let id = CapabilityId::new(JSON_CAPABILITY_ID).unwrap();
    let tool = Uuid::new_v4();
    let snapshot = |revision, enabled, effects| StableToolPolicySnapshot {
        revision,
        tools: HashMap::from([(
            tool,
            InstanceToolRule {
                enabled,
                revision,
                execution: ToolExecutionRules {
                    allowed_effects: effects,
                    mounts: MountView::default(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: None,
                },
            },
        )]),
        capabilities: HashMap::from([(id.clone(), tool)]),
    };
    let effects = vec![EffectKind::DispatchCapability];
    let policy = Arc::new(LiveStableToolPolicy::new(snapshot(1, true, effects.clone())).unwrap());
    let mut registry = ExtensionRegistry::new();
    registry
        .insert(builtin_first_party_package().unwrap())
        .unwrap();
    let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new("builtin").unwrap(),
            "/system/extensions/builtin/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            effects.clone(),
            None,
        ),
    ]))])
    .unwrap();
    let runtime = Arc::new(
        HostRuntimeServices::new(
            Arc::new(registry),
            Arc::new(LocalFilesystem::new()),
            Arc::new(InMemoryResourceGovernor::new()),
            Arc::new(InstanceToolAuthorizer::new(policy.clone())),
            brassclaw_processes::ProcessServices::in_memory(),
            CapabilitySurfaceVersion::new("instance-model-call").unwrap(),
        )
        .with_first_party_capabilities(Arc::new(
            builtin_first_party_handlers(Arc::new(InMemoryTriggerRepository::default())).unwrap(),
        ))
        .with_trust_policy(Arc::new(trust_policy))
        .with_runtime_policy(EffectiveRuntimePolicy {
            deployment: DeploymentMode::LocalSingleUser,
            requested_profile: RuntimeProfile::LocalDev,
            resolved_profile: RuntimeProfile::LocalDev,
            filesystem_backend: FilesystemBackendKind::HostWorkspace,
            process_backend: ProcessBackendKind::LocalHost,
            network_mode: NetworkMode::DirectLogged,
            secret_mode: SecretMode::ScrubbedEnv,
            approval_policy: ApprovalPolicy::AskDestructive,
            audit_mode: AuditMode::LocalMinimal,
        })
        .host_runtime_for_local_testing(),
    );
    assert_eq!(
        runtime.dispatch_authority_model(),
        DispatchAuthorityModel::InstancePolicy
    );
    let thread = ThreadId::new("opaque-model-tool-chat").unwrap();
    let mut context = ExecutionContext::local_default(
        UserId::new("operator").unwrap(),
        ExtensionId::new("model-call").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::UserTrusted,
        CapabilitySet::default(),
        MountView::default(),
    )
    .unwrap();
    context.thread_id = Some(thread.clone());
    context.resource_scope.thread_id = Some(thread.clone());
    let run = LoopRunContext::new(
        TurnScope::new(
            context.tenant_id.clone(),
            context.agent_id.clone(),
            context.project_id.clone(),
            thread,
        ),
        TurnId::new(),
        TurnRunId::new(),
        InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .unwrap(),
    );
    assert!(context.grants.grants.is_empty());
    // A leftover scoped grant from another operator must neither authorize nor
    // prevent an instance-policy call. The adapter must discard it entirely.
    context.grants.grants.push(CapabilityGrant {
        id: CapabilityGrantId::new(),
        capability: id.clone(),
        grantee: Principal::User(UserId::new("other-operator").unwrap()),
        issued_by: Principal::HostRuntime,
        constraints: GrantConstraints {
            allowed_effects: vec![EffectKind::WriteFilesystem],
            mounts: MountView::default(),
            network: NetworkPolicy::default(),
            secrets: vec![],
            resource_ceiling: None,
            expires_at: None,
            max_invocations: None,
        },
    });
    let mut visible = HostVisibleRequest::new(context, SurfaceKind::new("agent_loop").unwrap())
        .with_policy(CapabilitySurfacePolicy::allow_all());
    visible.provider_trust = BTreeMap::from([(
        ExtensionId::new("builtin").unwrap(),
        TrustDecision {
            effective_trust: EffectiveTrustClass::user_trusted(),
            authority_ceiling: AuthorityCeiling {
                allowed_effects: effects.clone(),
                max_resource_ceiling: None,
            },
            provenance: TrustProvenance::Default,
            evaluated_at: chrono::Utc::now(),
        },
    )]);
    let io = Arc::new(PgCapabilityIo::new(rig.pool.clone()));
    let port = HostRuntimeLoopCapabilityPortFactory::new(
        runtime,
        visible,
        io.clone(),
        io.clone(),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    )
    .for_run_context(run.clone());
    let surface = port
        .visible_capabilities(VisibleCapabilityRequest)
        .await
        .unwrap();
    assert!(
        surface
            .descriptors
            .iter()
            .any(|descriptor| descriptor.capability_id == id)
    );
    let definition = port
        .tool_definitions()
        .unwrap()
        .into_iter()
        .find(|definition| definition.capability_id == id)
        .unwrap();
    // Keep this same advertised surface across revisions: live kernel policy,
    // not a new advertisement or an operation approval, decides each call.
    for (revision, enabled, allowed) in [
        (1, true, true),
        (2, false, false),
        (3, true, false),
        (4, true, true),
    ] {
        if revision > 1 {
            policy
                .publish(
                    revision - 1,
                    snapshot(
                        revision,
                        enabled,
                        if allowed { effects.clone() } else { vec![] },
                    ),
                )
                .unwrap();
        }
        let candidate = port
            .register_provider_tool_call(ProviderToolCall {
                provider_id: "local-provider".into(),
                provider_model_id: "local-model".into(),
                turn_id: Some("model-turn".into()),
                id: format!("call-{revision}"),
                name: definition.name.clone(),
                arguments: json!({"operation":"parse","data":"{\"value\":7}"}),
                response_reasoning: None,
                reasoning: None,
                signature: None,
            })
            .await
            .unwrap();
        let outcome = port
            .invoke_capability(CapabilityInvocation {
                surface_version: candidate.surface_version,
                capability_id: candidate.capability_id,
                input_ref: candidate.input_ref,
            })
            .await
            .unwrap();
        if allowed {
            let CapabilityOutcome::Completed(result) = outcome else {
                panic!("native completion required: {outcome:?}");
            };
            assert_eq!(
                io.result_output(&run.scope, run.run_id, &result.result_ref)
                    .await
                    .unwrap(),
                json!({"value":7})
            );
        } else {
            assert!(
                matches!(outcome, CapabilityOutcome::Denied(ref denied)
                    if denied.reason_kind == brassclaw_turns::run_profile::CapabilityDeniedReasonKind::Authorization),
                "live policy must deny: {outcome:?}"
            );
        }
    }
    let count: i64 = rig.pool.get().await.unwrap().query_one(
        "SELECT count(*) FROM brassclaw_loop_capability_values WHERE run_id=$1 AND kind='result'",
        &[&run.run_id.as_uuid()]).await.unwrap().get(0);
    assert_eq!(
        count, 2,
        "denied dispatches cannot store synthetic success results"
    );
}
