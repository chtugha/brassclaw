#[cfg(feature = "postgres")]
use std::sync::Arc;

#[cfg(feature = "postgres")]
use brassclaw_host_api::{
    AuditMode, DeploymentMode, EffectKind, FilesystemBackendKind, NetworkMode, PackageId,
    ProcessBackendKind, RuntimeKind, RuntimeProfile, SecretMode,
    runtime_policy::{ApprovalPolicy, EffectiveRuntimePolicy},
};
#[cfg(feature = "postgres")]
use brassclaw_host_runtime::{
    SchedulerTurnRunWakeNotifier, TurnRunExecutor, TurnRunExecutorError, TurnRunScheduler,
    TurnRunSchedulerConfig, TurnRunSchedulerHandle,
};
#[cfg(feature = "postgres")]
use brassclaw_reborn_composition::RebornRuntimeProcessBinding;
#[cfg(feature = "postgres")]
use brassclaw_reborn_composition::{RebornBuildError, RebornCompositionProfile};
use brassclaw_reborn_composition::{
    RebornBuildInput, RebornManualTokenSetupRequest, RebornManualTokenSubmitRequest,
    RebornReadinessState, build_reborn_services,
};
#[cfg(feature = "postgres")]
use brassclaw_secrets::SecretMaterial;
#[cfg(feature = "postgres")]
use brassclaw_trust::{AdminConfig, AdminEntry, HostTrustAssignment, HostTrustPolicy};
#[cfg(feature = "postgres")]
use brassclaw_turns::{
    InMemoryTurnStateStore,
    runner::{ClaimedTurnRun, TurnRunTransitionPort},
};
#[cfg(feature = "postgres")]
use deadpool_postgres::tokio_postgres;
use secrecy::SecretString;

#[path = "facade_factory/sandbox_process_ports.rs"]
mod sandbox_process_ports;

#[cfg(feature = "postgres")]
fn test_master_key() -> SecretMaterial {
    SecretMaterial::from("01234567890123456789012345678901")
}

#[cfg(feature = "postgres")]
struct NoopTurnRunExecutor;

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl TurnRunExecutor for NoopTurnRunExecutor {
    async fn execute_claimed_run(
        &self,
        _claimed: ClaimedTurnRun,
        _transitions: Arc<dyn TurnRunTransitionPort>,
    ) -> Result<(), TurnRunExecutorError> {
        Ok(())
    }
}

#[cfg(feature = "postgres")]
fn production_trust_policy() -> Arc<HostTrustPolicy> {
    Arc::new(
        HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries([
            AdminEntry::for_admin(
                PackageId::new("reborn-test").unwrap(),
                HostTrustAssignment::first_party(),
                vec![EffectKind::DispatchCapability],
                None,
            ),
        ]))])
        .unwrap(),
    )
}

#[cfg(feature = "postgres")]
fn production_runtime_policy() -> EffectiveRuntimePolicy {
    EffectiveRuntimePolicy {
        deployment: DeploymentMode::HostedMultiTenant,
        requested_profile: RuntimeProfile::HostedDev,
        resolved_profile: RuntimeProfile::HostedDev,
        filesystem_backend: FilesystemBackendKind::TenantWorkspace,
        process_backend: ProcessBackendKind::TenantSandbox,
        network_mode: NetworkMode::Allowlist,
        secret_mode: SecretMode::TenantBroker,
        approval_policy: ApprovalPolicy::AskDestructive,
        audit_mode: AuditMode::Standard,
    }
}

#[cfg(feature = "postgres")]
fn live_wake_notifier() -> (Arc<SchedulerTurnRunWakeNotifier>, TurnRunSchedulerHandle) {
    let transitions: Arc<dyn TurnRunTransitionPort> = Arc::new(InMemoryTurnStateStore::default());
    let executor: Arc<dyn TurnRunExecutor> = Arc::new(NoopTurnRunExecutor);
    let handle =
        TurnRunScheduler::new(transitions, executor, TurnRunSchedulerConfig::default()).start();
    (handle.wake_notifier(), handle)
}

#[cfg(feature = "postgres")]
async fn postgres_pool_or_skip() -> Option<(
    testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    deadpool_postgres::Pool,
    String,
)> {
    let (container, database_url) = start_postgres_container().await?;
    let config: tokio_postgres::Config = database_url
        .parse()
        .expect("testcontainer database URL must parse");
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .expect("Postgres pool must build");
    let _connection = pool
        .get()
        .await
        .expect("Postgres testcontainer must accept connections");
    Some((container, pool, database_url))
}

#[cfg(feature = "postgres")]
async fn start_postgres_container() -> Option<(
    testcontainers_modules::testcontainers::ContainerAsync<
        testcontainers_modules::postgres::Postgres,
    >,
    String,
)> {
    use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

    let image = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("brassclaw_test")
        .with_user("postgres")
        .with_password("postgres")
        .with_tag("16-alpine");

    let container = match image.start().await {
        Ok(container) => container,
        Err(error) => {
            eprintln!(
                "skipping Postgres composition tests: docker/testcontainers unavailable ({error})"
            );
            return None;
        }
    };
    let host = match container.get_host().await {
        Ok(host) => host,
        Err(error) => {
            eprintln!(
                "skipping Postgres composition tests: could not resolve container host ({error})"
            );
            return None;
        }
    };
    let port = match container.get_host_port_ipv4(5432).await {
        Ok(port) => port,
        Err(error) => {
            eprintln!(
                "skipping Postgres composition tests: could not resolve container port ({error})"
            );
            return None;
        }
    };
    Some((
        container,
        format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test"),
    ))
}

#[tokio::test]
async fn disabled_returns_empty_services() {
    let services = build_reborn_services(RebornBuildInput::disabled("test-owner"))
        .await
        .unwrap();

    assert!(services.host_runtime.is_none());
    assert!(services.turn_coordinator.is_none());
    assert_eq!(services.readiness.state, RebornReadinessState::Disabled);
}

#[tokio::test]
async fn local_dev_builds_facades_without_production_claim() {
    let dir = tempfile::tempdir().unwrap();
    let services = build_reborn_services(RebornBuildInput::local_dev(
        "test-owner",
        dir.path().to_path_buf(),
    ))
    .await
    .unwrap();

    assert!(services.host_runtime.is_some());
    assert!(services.turn_coordinator.is_some());
    assert_eq!(services.readiness.state, RebornReadinessState::DevOnly);
    assert!(services.readiness.facades.host_runtime);
    assert!(services.readiness.facades.turn_coordinator);
    assert!(services.readiness.facades.product_auth);
    assert!(services.product_auth.is_some());
}

#[cfg(feature = "postgres")]
fn test_sandbox_process_binding() -> RebornRuntimeProcessBinding {
    let process_port = Arc::new(brassclaw_host_runtime::TenantSandboxProcessPort::new(
        Arc::new(ProductionReadySandboxTransport),
    ));
    RebornRuntimeProcessBinding::tenant_sandbox(process_port)
}

#[cfg(feature = "postgres")]
#[derive(Debug)]
struct ProductionReadySandboxTransport;

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl brassclaw_host_runtime::SandboxCommandTransport for ProductionReadySandboxTransport {
    async fn run_command(
        &self,
        _request: brassclaw_host_runtime::CommandExecutionRequest,
    ) -> Result<
        brassclaw_host_runtime::CommandExecutionOutput,
        brassclaw_host_runtime::RuntimeProcessError,
    > {
        Ok(brassclaw_host_runtime::CommandExecutionOutput {
            output: String::new(),
            saved_output: None,
            exit_code: 0,
            sandboxed: true,
            duration: std::time::Duration::ZERO,
        })
    }
}

#[tokio::test]
async fn local_dev_product_auth_entrypoint_redacts_manual_token_submit() {
    let dir = tempfile::tempdir().unwrap();
    let services = build_reborn_services(RebornBuildInput::local_dev(
        "test-owner",
        dir.path().to_path_buf(),
    ))
    .await
    .unwrap();
    let product_auth = services
        .product_auth
        .as_ref()
        .expect("local-dev composes product auth");
    let scope = auth_scope("alice");
    let provider = brassclaw_auth::AuthProviderId::new("github").unwrap();
    let label = brassclaw_auth::CredentialAccountLabel::new("work github").unwrap();

    let challenge = product_auth
        .request_manual_token_setup(RebornManualTokenSetupRequest {
            scope: scope.clone(),
            provider: provider.clone(),
            label: label.clone(),
            continuation: brassclaw_auth::AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        })
        .await
        .unwrap();
    assert_eq!(challenge.provider, provider);
    assert_eq!(challenge.label, label);

    let submit = RebornManualTokenSubmitRequest::new(
        scope.clone(),
        challenge.interaction_id,
        SecretString::from("super-secret-token".to_string()),
    );
    let debug = format!("{submit:?}");
    assert!(!debug.contains("super-secret-token"));

    let result = product_auth.submit_manual_token(submit).await.unwrap();
    assert_eq!(
        result.status,
        brassclaw_auth::CredentialAccountStatus::Configured
    );

    let accounts = product_auth
        .credential_account_service()
        .list_accounts(brassclaw_auth::CredentialAccountListRequest::new(
            scope.clone(),
            provider,
        ))
        .await
        .unwrap();
    assert_eq!(accounts.accounts.len(), 1);
    let serialized = serde_json::to_string(&accounts).unwrap();
    assert!(!serialized.contains("super-secret-token"));
    assert!(!serialized.contains("manual-access-"));
}

fn auth_scope(user: &str) -> brassclaw_auth::AuthProductScope {
    brassclaw_auth::AuthProductScope::new(
        brassclaw_host_api::ResourceScope::local_default(
            brassclaw_host_api::UserId::new(user).unwrap(),
            brassclaw_host_api::InvocationId::new(),
        )
        .unwrap(),
        brassclaw_auth::AuthSurface::Web,
    )
    .with_session_id(brassclaw_auth::AuthSessionId::new(format!("session-{user}")).unwrap())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn production_postgres_services_migrate_trigger_repository_before_runtime_injection() {
    let Some((_container, pool, database_url)) = postgres_pool_or_skip().await else {
        return;
    };
    let (notifier, handle) = live_wake_notifier();

    let services = build_reborn_services(
        RebornBuildInput::postgres(
            RebornCompositionProfile::Production,
            "test-owner",
            pool.clone(),
            SecretMaterial::from(database_url),
            test_master_key(),
        )
        .with_production_trust_policy(production_trust_policy())
        .with_runtime_policy(production_runtime_policy())
        .with_turn_run_wake_notifier(notifier)
        .with_runtime_process_binding(test_sandbox_process_binding()),
    )
    .await
    .expect("production postgres services should build with trigger repository migrations");

    handle.shutdown().await;

    assert!(services.host_runtime.is_some());

    let client = pool.get().await.expect("connect postgres state db");
    let row = client
        .query_one("SELECT COUNT(*) FROM trigger_records", &[])
        .await
        .expect("trigger table exists after production build");
    let count: i64 = row.get(0);
    assert_eq!(count, 0);
}

#[cfg(feature = "postgres")]
#[tokio::test]
#[ignore = "TODO(#3856): restore when tenant sandbox process-port wiring exists"]
async fn production_postgres_services_wire_first_party_runtime_http_egress() {
    // Restore the ProductionValidated readiness and host_runtime.health()
    // happy-path assertions that are temporarily fail-closed below.
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn production_postgres_services_require_process_port_for_first_party_runtime() {
    let Some((_container, pool, database_url)) = postgres_pool_or_skip().await else {
        return;
    };
    let (notifier, handle) = live_wake_notifier();

    let result = build_reborn_services(
        RebornBuildInput::postgres(
            RebornCompositionProfile::Production,
            "test-owner",
            pool,
            SecretMaterial::from(database_url),
            test_master_key(),
        )
        .with_production_trust_policy(production_trust_policy())
        .with_runtime_policy(production_runtime_policy())
        .with_turn_run_wake_notifier(notifier)
        .with_required_runtime_backends([RuntimeKind::FirstParty])
        .require_runtime_http_egress(),
    )
    .await;

    handle.shutdown().await;

    let Err(RebornBuildError::InvalidConfig { reason }) = result else {
        panic!(
            "expected postgres production first-party runtime to require a process port, got {result:?}"
        );
    };
    assert!(
        reason.contains("tenant sandbox process binding"),
        "postgres first-party shell capability should keep production wiring fail-closed until a tenant sandbox process port is configured: {reason}"
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
#[ignore = "TODO(#3856): restore when tenant sandbox process-port wiring exists"]
async fn migration_dry_run_validates_postgres_planned_turn_profile() {
    // Restore the MigrationDryRunValidated readiness and planned-profile
    // submit_turn assertions that are temporarily fail-closed below.
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn migration_dry_run_requires_postgres_process_port_for_first_party_runtime() {
    let Some((_container, pool, database_url)) = postgres_pool_or_skip().await else {
        return;
    };
    let (notifier, handle) = live_wake_notifier();

    let result = build_reborn_services(
        RebornBuildInput::postgres(
            RebornCompositionProfile::LocalDev,
            "test-owner",
            pool,
            SecretMaterial::from(database_url),
            test_master_key(),
        )
        .with_production_trust_policy(production_trust_policy())
        .with_runtime_policy(production_runtime_policy())
        .with_turn_run_wake_notifier(notifier),
    )
    .await;

    handle.shutdown().await;

    let Err(RebornBuildError::InvalidConfig { reason }) = result else {
        panic!("expected postgres migration dry-run to require a process port, got {result:?}");
    };
    assert!(
        reason.contains("tenant sandbox process binding"),
        "postgres migration dry-run should keep production-shaped first-party wiring fail-closed until a tenant sandbox process port is configured: {reason}"
    );
}
