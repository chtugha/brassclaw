use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use brassclaw_reborn_composition::{
    RebornBuildInput, RebornRuntimeProcessBinding, build_reborn_services,
};

#[tokio::test]
async fn local_dev_composes_injected_tenant_sandbox_process_port() {
    let dir = tempfile::tempdir().expect("tempdir");
    let transport = Arc::new(RecordingSandboxTransport::default());
    let process_port = Arc::new(brassclaw_host_runtime::TenantSandboxProcessPort::new(
        transport.clone(),
    ));
    let services = build_reborn_services(
        RebornBuildInput::local_dev("sandbox-port-owner", dir.path().join("local-dev"))
            .with_runtime_policy(tenant_sandbox_process_policy())
            .with_runtime_process_binding(RebornRuntimeProcessBinding::tenant_sandbox(
                process_port,
            )),
    )
    .await
    .expect("local-dev services build");
    let host_runtime = services.host_runtime.expect("host runtime");

    let output = invoke_shell(
        host_runtime.as_ref(),
        serde_json::json!({"command": "echo composed sandbox", "timeout": 9}),
    )
    .await;

    assert_eq!(output["sandboxed"], serde_json::json!(true));
    assert_eq!(
        output["output"],
        serde_json::json!("sandbox port: echo composed sandbox")
    );
    let requests = transport.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].command, "echo composed sandbox");
    assert_eq!(requests[0].timeout_secs, Some(9));
}

#[derive(Debug, Default)]
struct RecordingSandboxTransport {
    requests: Mutex<Vec<brassclaw_host_runtime::CommandExecutionRequest>>,
}

#[async_trait::async_trait]
impl brassclaw_host_runtime::SandboxCommandTransport for RecordingSandboxTransport {
    async fn run_command(
        &self,
        request: brassclaw_host_runtime::CommandExecutionRequest,
    ) -> Result<
        brassclaw_host_runtime::CommandExecutionOutput,
        brassclaw_host_runtime::RuntimeProcessError,
    > {
        let command = request.command.clone();
        self.requests.lock().unwrap().push(request);
        Ok(brassclaw_host_runtime::CommandExecutionOutput {
            output: format!("sandbox port: {command}"),
            saved_output: None,
            exit_code: 0,
            sandboxed: false,
            duration: Duration::from_millis(5),
        })
    }
}

async fn invoke_shell(
    runtime: &dyn brassclaw_host_runtime::HostRuntime,
    input: serde_json::Value,
) -> serde_json::Value {
    let outcome = runtime
        .invoke_capability(brassclaw_host_runtime::RuntimeCapabilityRequest::new(
            shell_execution_context(),
            brassclaw_host_api::CapabilityId::new(brassclaw_host_runtime::SHELL_CAPABILITY_ID)
                .unwrap(),
            brassclaw_host_api::ResourceEstimate::default(),
            input,
            trust_decision(),
        ))
        .await
        .expect("capability invoke");
    let brassclaw_host_runtime::RuntimeCapabilityOutcome::Completed(completed) = outcome else {
        panic!("expected completed shell invocation, got {outcome:?}");
    };
    completed.output
}

fn tenant_sandbox_process_policy() -> brassclaw_host_api::EffectiveRuntimePolicy {
    brassclaw_host_api::EffectiveRuntimePolicy {
        deployment: brassclaw_host_api::DeploymentMode::LocalSingleUser,
        requested_profile: brassclaw_host_api::RuntimeProfile::LocalDev,
        resolved_profile: brassclaw_host_api::RuntimeProfile::LocalDev,
        filesystem_backend: brassclaw_host_api::FilesystemBackendKind::HostWorkspace,
        process_backend: brassclaw_host_api::ProcessBackendKind::TenantSandbox,
        network_mode: brassclaw_host_api::NetworkMode::DirectLogged,
        secret_mode: brassclaw_host_api::SecretMode::ScrubbedEnv,
        approval_policy: brassclaw_host_api::ApprovalPolicy::AskDestructive,
        audit_mode: brassclaw_host_api::AuditMode::LocalMinimal,
    }
}

fn shell_execution_context() -> brassclaw_host_api::ExecutionContext {
    let grant = brassclaw_host_api::CapabilityGrant {
        id: brassclaw_host_api::CapabilityGrantId::new(),
        capability: brassclaw_host_api::CapabilityId::new(
            brassclaw_host_runtime::SHELL_CAPABILITY_ID,
        )
        .unwrap(),
        grantee: brassclaw_host_api::Principal::Extension(
            brassclaw_host_api::ExtensionId::new("caller").unwrap(),
        ),
        issued_by: brassclaw_host_api::Principal::Extension(
            brassclaw_host_api::ExtensionId::new("issuer").unwrap(),
        ),
        constraints: brassclaw_host_api::GrantConstraints {
            allowed_effects: shell_effects(),
            mounts: brassclaw_host_api::MountView::default(),
            network: shell_test_policy(),
            secrets: Vec::new(),
            resource_ceiling: None,
            expires_at: None,
            max_invocations: None,
        },
    };
    brassclaw_host_api::ExecutionContext::local_default(
        brassclaw_host_api::UserId::new("user").unwrap(),
        brassclaw_host_api::ExtensionId::new("caller").unwrap(),
        brassclaw_host_api::RuntimeKind::FirstParty,
        brassclaw_host_api::TrustClass::FirstParty,
        brassclaw_host_api::CapabilitySet {
            grants: vec![grant],
        },
        brassclaw_host_api::MountView::default(),
    )
    .unwrap()
}

fn shell_effects() -> Vec<brassclaw_host_api::EffectKind> {
    vec![
        brassclaw_host_api::EffectKind::DispatchCapability,
        brassclaw_host_api::EffectKind::ReadFilesystem,
        brassclaw_host_api::EffectKind::WriteFilesystem,
        brassclaw_host_api::EffectKind::Network,
        brassclaw_host_api::EffectKind::SpawnProcess,
        brassclaw_host_api::EffectKind::ExecuteCode,
    ]
}

fn shell_test_policy() -> brassclaw_host_api::NetworkPolicy {
    brassclaw_host_api::NetworkPolicy {
        allowed_targets: vec![brassclaw_host_api::NetworkTargetPattern {
            scheme: None,
            host_pattern: "*".to_string(),
            port: None,
        }],
        deny_private_ip_ranges: false,
        max_egress_bytes: None,
    }
}

fn trust_decision() -> brassclaw_trust::TrustDecision {
    brassclaw_trust::TrustDecision {
        effective_trust: brassclaw_trust::EffectiveTrustClass::user_trusted(),
        authority_ceiling: brassclaw_trust::AuthorityCeiling {
            allowed_effects: shell_effects(),
            max_resource_ceiling: None,
        },
        provenance: brassclaw_trust::TrustProvenance::Default,
        evaluated_at: chrono::Utc::now(),
    }
}
