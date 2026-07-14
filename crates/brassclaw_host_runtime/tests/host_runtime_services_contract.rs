mod support;

use support::legacy_capability_fixture_to_v2;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use async_trait::async_trait;
use brassclaw_approvals::LeaseApproval;
use brassclaw_authorization::{
    CapabilityLeaseStatus, CapabilityLeaseStore, GrantAuthorizer, InMemoryCapabilityLeaseStore,
    TrustAwareCapabilityDispatchAuthorizer,
};
use brassclaw_capabilities::{
    CapabilityHost, CapabilityObligationHandler, CapabilityObligationPhase,
    CapabilityObligationRequest, CapabilitySpawnRequest,
};
use brassclaw_event_projections::{
    AuditProjectionError, AuditProjectionRequest, AuditProjectionService, AuditProjectionStage,
    EventProjectionService, ProjectionCursor, ProjectionError, ProjectionRequest, ProjectionScope,
    ReplayAuditProjectionService, ReplayEventProjectionService, RunProjectionStatus,
    TimelineEntryKind,
};
use brassclaw_events::{
    DurableAuditLog, DurableAuditSink, DurableEventLog, DurableEventSink, EventCursor, EventError,
    EventReplay, EventStreamKey, InMemoryAuditSink, InMemoryDurableAuditLog,
    InMemoryDurableEventLog, InMemoryEventSink, ReadScope, RuntimeEventKind,
};
use brassclaw_extensions::{
    ExtensionManifest, ExtensionPackage, ExtensionRegistry, ManifestSource,
};
#[cfg(feature = "libsql")]
use brassclaw_filesystem::LibSqlRootFilesystem;
#[cfg(feature = "libsql")]
use brassclaw_filesystem::ScopedFilesystem;
use brassclaw_filesystem::{LocalFilesystem, RootFilesystem};
use brassclaw_host_api::*;
use brassclaw_host_runtime::{
    BuiltinObligationHandler, BuiltinObligationServices, CancelReason, CancelRuntimeWorkRequest,
    CapabilitySurfaceVersion, CommandExecutionOutput, CommandExecutionRequest, DefaultHostRuntime,
    HostRuntime, HostRuntimeServices, ProcessObligationLifecycleStore, ProductionWiringComponent,
    ProductionWiringConfig, ProductionWiringIssueKind, RuntimeCapabilityOutcome,
    RuntimeCapabilityRequest, RuntimeCapabilityResumeRequest, RuntimeFailureKind,
    RuntimeProcessError, RuntimeProcessPort, RuntimeStatusRequest, RuntimeWorkId,
    SandboxCommandTransport, TenantSandboxProcessPort, builtin_first_party_handlers,
    builtin_first_party_package,
};
use brassclaw_mcp::{McpError, McpExecutionRequest, McpExecutionResult, McpExecutor};
use brassclaw_network::{
    NetworkHttpEgress, NetworkHttpError, NetworkHttpRequest, NetworkHttpResponse, NetworkUsage,
};
use brassclaw_processes::{
    BackgroundFailureStage, BackgroundProcessManager, InMemoryProcessResultStore,
    InMemoryProcessStore, ProcessError, ProcessExecutionRequest, ProcessExecutionResult,
    ProcessExecutor, ProcessHost, ProcessManager, ProcessResultRecord, ProcessResultStore,
    ProcessServices, ProcessStart, ProcessStatus, ProcessStore,
};
use brassclaw_reborn_event_store::{
    RebornEventStoreConfig, RebornEventStoreError, RebornProfile, build_reborn_event_stores,
};
use brassclaw_resources::{
    InMemoryResourceGovernor, JsonFileResourceGovernorStore, PersistentResourceGovernor,
    ResourceAccount, ResourceError, ResourceGovernor, ResourceLimits, ResourceTally,
};
use brassclaw_run_state::{
    ApprovalRecord, ApprovalRequestStore, InMemoryApprovalRequestStore, InMemoryRunStateStore,
    RunRecord, RunStart, RunStateApprovalStore, RunStateError, RunStateStore, RunStatus,
};
use brassclaw_secrets::{
    InMemoryCredentialBroker, InMemorySecretStore, SecretMaterial, SecretStore,
};
use brassclaw_triggers::InMemoryTriggerRepository;
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
#[cfg(feature = "libsql")]
use brassclaw_turns::FilesystemTurnStateStore;
#[cfg(feature = "libsql")]
use brassclaw_turns::{
    AcceptedMessageRef, IdempotencyKey, InMemoryRunProfileResolver, ReplyTargetBindingRef,
    RunProfileRequest, SourceBindingRef, SubmitTurnRequest, SubmitTurnResponse, TurnActor,
    TurnCoordinator, TurnScope, TurnStateStore,
};
use brassclaw_turns::{NoopTurnRunWakeNotifier, TurnRunWake, TurnRunWakeNotifier};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;

#[tokio::test]
async fn production_wiring_validation_rejects_missing_components_and_local_only_defaults() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    );

    let report = match services.host_runtime_for_production(&ProductionWiringConfig::new([])) {
        Ok(_) => panic!("bare local/test service graph must not pass production validation"),
        Err(report) => report,
    };

    assert!(
        report.contains(
            ProductionWiringComponent::TrustPolicy,
            ProductionWiringIssueKind::Missing
        ),
        "missing explicit trust policy should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::RuntimePolicy,
            ProductionWiringIssueKind::Missing
        ),
        "missing resolved runtime policy should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::RunState,
            ProductionWiringIssueKind::Missing
        ),
        "missing run-state store should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::ApprovalRequests,
            ProductionWiringIssueKind::Missing
        ),
        "missing approval store should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::CapabilityLeases,
            ProductionWiringIssueKind::Missing
        ),
        "missing capability lease store should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::TurnState,
            ProductionWiringIssueKind::Missing
        ),
        "missing turn-state store should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::RunProfileResolver,
            ProductionWiringIssueKind::Missing
        ),
        "missing run-profile resolver should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::TurnRunWakeNotifier,
            ProductionWiringIssueKind::Missing
        ),
        "missing turn wake notifier should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::EventSink,
            ProductionWiringIssueKind::Missing
        ),
        "missing event sink should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::AuditSink,
            ProductionWiringIssueKind::Missing
        ),
        "missing audit sink should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::SecretStore,
            ProductionWiringIssueKind::Missing
        ),
        "missing secret store should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::Filesystem,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "local filesystem should be reported as local-only: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::ResourceGovernor,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory resource governor should be reported as local-only: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::ProcessStore,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory process store should be reported as local-only: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::ProcessResultStore,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory process result store should be reported as local-only: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_rejects_local_only_runtime_policy() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_runtime_policy(local_dev_runtime_policy());

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("local-dev runtime policy must not pass production validation");

    assert!(
        report.contains(
            ProductionWiringComponent::RuntimePolicy,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "local runtime policy should be reported as local-only: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_rejects_each_local_only_runtime_policy_field() {
    let mut host_workspace = hosted_dev_runtime_policy();
    host_workspace.filesystem_backend = FilesystemBackendKind::HostWorkspace;
    assert_local_only_runtime_policy_rejected(host_workspace, "host_workspace_filesystem");

    let mut host_workspace_and_home = hosted_dev_runtime_policy();
    host_workspace_and_home.filesystem_backend = FilesystemBackendKind::HostWorkspaceAndHome;
    assert_local_only_runtime_policy_rejected(host_workspace_and_home, "host_workspace_filesystem");

    let mut local_process = hosted_dev_runtime_policy();
    local_process.process_backend = ProcessBackendKind::LocalHost;
    assert_local_only_runtime_policy_rejected(local_process, "local_host_process");

    let mut direct_network = hosted_dev_runtime_policy();
    direct_network.network_mode = NetworkMode::Direct;
    assert_local_only_runtime_policy_rejected(direct_network, "direct_network");

    let mut scrubbed_secrets = hosted_dev_runtime_policy();
    scrubbed_secrets.secret_mode = SecretMode::ScrubbedEnv;
    assert_local_only_runtime_policy_rejected(scrubbed_secrets, "local_secret_environment");

    let mut inherited_secrets = hosted_dev_runtime_policy();
    inherited_secrets.secret_mode = SecretMode::InheritedEnv;
    assert_local_only_runtime_policy_rejected(inherited_secrets, "local_secret_environment");
}

#[tokio::test]
async fn production_wiring_validation_accepts_production_safe_runtime_policy_shape() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_runtime_policy(hosted_dev_runtime_policy());

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local/test defaults still prevent production validation");

    assert!(
        !report.contains(
            ProductionWiringComponent::RuntimePolicy,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "hosted runtime policy should satisfy runtime-policy guardrail: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_accepts_persistent_resource_governor_component() {
    let dir = tempfile::tempdir().unwrap();
    let governor = Arc::new(PersistentResourceGovernor::new(
        JsonFileResourceGovernorStore::new(dir.path().join("resource-governor.json")),
    ));
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        governor,
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    );

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local/test defaults still prevent production validation");

    assert!(
        !report.contains(
            ProductionWiringComponent::ResourceGovernor,
            ProductionWiringIssueKind::LocalOnlyImplementation,
        ),
        "persistent resource governor should satisfy resource guardrail: {report:?}"
    );
}

/// Filesystem-backed equivalent of the deleted libSQL/Postgres tests.
/// Backend choice is a `RootFilesystem` property; the `with_filesystem_resource_governor`
/// builder drives the same surface that the deleted SQL-specific builders
/// covered.
#[tokio::test]
async fn with_filesystem_resource_governor_persists_reservations_across_handles() {
    use brassclaw_filesystem::{InMemoryBackend, ScopedFilesystem};
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};

    let backend = Arc::new(InMemoryBackend::new());
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/resources").unwrap(),
        VirtualPath::new("/tenants/tenant1/users/user1/resources").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let scoped = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::clone(&backend),
        mounts,
    ));

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_filesystem_resource_governor(Arc::clone(&scoped));

    let governor = services.resource_governor();
    let scope = sample_scope(InvocationId::new());
    let account = ResourceAccount::tenant(scope.tenant_id.clone());
    governor
        .set_limit(
            account.clone(),
            ResourceLimits {
                max_concurrency_slots: Some(1),
                ..ResourceLimits::default()
            },
        )
        .unwrap();
    let reservation = governor
        .reserve(
            scope,
            ResourceEstimate {
                concurrency_slots: Some(1),
                ..ResourceEstimate::default()
            },
        )
        .unwrap();
    governor.release(reservation.id).unwrap();
}

#[tokio::test]
async fn with_filesystem_resource_governor_closes_process_reservations_on_cancel() {
    use brassclaw_filesystem::{InMemoryBackend, ScopedFilesystem};
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};

    let backend = Arc::new(InMemoryBackend::new());
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/resources").unwrap(),
        VirtualPath::new("/tenants/tenant1/users/user1/resources").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let scoped = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::clone(&backend),
        mounts,
    ));
    let process_services = ProcessServices::new(
        Arc::new(InMemoryProcessStore::new()),
        Arc::new(InMemoryProcessResultStore::new()),
    );
    let process_store = process_services.process_store();

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_filesystem_resource_governor(Arc::clone(&scoped));
    let governor = services.resource_governor();
    let scope = sample_scope(InvocationId::new());
    let account = ResourceAccount::tenant(scope.tenant_id.clone());
    let reservation_id = ResourceReservationId::new();
    let estimate = ResourceEstimate {
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    };
    governor
        .set_limit(
            account.clone(),
            ResourceLimits {
                max_concurrency_slots: Some(1),
                ..ResourceLimits::default()
            },
        )
        .unwrap();
    governor
        .reserve_with_id(scope.clone(), estimate.clone(), reservation_id)
        .unwrap();
    let process_id = ProcessId::new();
    let mut start = process_start(process_id, scope.invocation_id, scope.clone());
    start.estimated_resources = estimate;
    start.resource_reservation_id = Some(reservation_id);
    process_store.start(start).await.unwrap();

    let runtime = services.host_runtime_for_local_testing();
    let outcome = runtime
        .cancel_work(CancelRuntimeWorkRequest::new(
            scope.clone(),
            CorrelationId::new(),
            CancelReason::UserRequested,
        ))
        .await
        .unwrap();

    assert_eq!(outcome.cancelled, vec![RuntimeWorkId::Process(process_id)]);
    assert_eq!(
        governor.reserved_for(&account).unwrap(),
        ResourceTally::default()
    );
    assert!(matches!(
        governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

#[tokio::test]
async fn production_wiring_validation_classifies_combined_store_as_run_state_and_approvals() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_local_only_run_state_approval_store(Arc::new(
        InMemoryRecordingCombinedRunStateApprovalStore::new(),
    ));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("local/test combined store must not pass production validation");

    assert!(
        report.contains(
            ProductionWiringComponent::RunState,
            ProductionWiringIssueKind::LocalOnlyImplementation,
        ),
        "combined store should be classified for run-state guardrails: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::ApprovalRequests,
            ProductionWiringIssueKind::LocalOnlyImplementation,
        ),
        "combined store should be classified for approval guardrails: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::RunState,
            ProductionWiringIssueKind::Missing,
        ),
        "combined store should satisfy run-state presence: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::ApprovalRequests,
            ProductionWiringIssueKind::Missing,
        ),
        "combined store should satisfy approval-store presence: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_rejects_unsupported_runtime_requirements() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    );

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([RuntimeKind::System]))
        .expect_err("system runtime requirements are not dispatcher backend requirements");

    assert!(
        report.contains(
            ProductionWiringComponent::RuntimeBackend,
            ProductionWiringIssueKind::UnsupportedRequirement
        ),
        "unsupported runtime backend requirement should be reported: {report:?}"
    );
}

// The legacy `LibSqlRunStateApprovalStore` / `PostgresRunStateApprovalStore`
// per-backend run-state + approval stores were deleted along with their
// `with_libsql_run_state_approval_store` /
// `with_postgres_run_state_approval_store` builder methods (see
// `docs/plans/2026-05-16-scoped-filesystem-tenant-isolation.md`).
// Durability across reopen is now a property of the underlying
// `RootFilesystem` (`LibSqlRootFilesystem`, `PostgresRootFilesystem`, …)
// composed through `with_filesystem_run_state`; the run-state store layer
// no longer owns its own per-SQL persistence. The deleted tests were:
//
//   - `libsql_run_state_store_selection_satisfies_production_run_state_guardrails`
//   - `libsql_run_state_store_selection_persists_runtime_approval_block`
//
// The equivalent guardrail surface for the filesystem-backed wiring is
// exercised by `tests/reborn_durable_restart_integration.rs` (services
// graph restart over `LocalFilesystem`) and the `brassclaw_run_state`
// contract suite.

#[cfg(feature = "libsql")]
#[tokio::test]
async fn production_root_filesystem_selection_accepts_libsql_root_filesystem() {
    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join("root-filesystem.db");
    let db = Arc::new(libsql::Builder::new_local(db_path).build().await.unwrap());
    let filesystem = Arc::new(LibSqlRootFilesystem::new(db));
    filesystem.run_migrations().await.unwrap();

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_libsql_root_filesystem(Arc::clone(&filesystem));

    let path = VirtualPath::new("/engine/tenants/t1/users/u1/root-selection.txt").unwrap();
    filesystem.write_file(&path, b"selected").await.unwrap();
    assert_eq!(filesystem.read_file(&path).await.unwrap(), b"selected");

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local services remain intentionally unready");
    assert!(
        !report.contains(
            ProductionWiringComponent::Filesystem,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "LibSqlRootFilesystem must satisfy production filesystem selection: {report:?}"
    );
}

/// Construct an [`Arc<ScopedFilesystem<LibSqlRootFilesystem>>`] that exposes
/// the `/turns` mount alias over a libSQL-backed [`RootFilesystem`]. Mirrors
/// the production composition shape: the `/turns` alias rewrites to a
/// tenant/user-scoped target inside `/engine`, and the filesystem backend
/// supplies durable storage. Used by tests that previously constructed
/// `LibSqlTurnStateStore` directly.
#[cfg(feature = "libsql")]
async fn libsql_scoped_turns_fs(
    db: Arc<libsql::Database>,
) -> Arc<ScopedFilesystem<LibSqlRootFilesystem>> {
    let filesystem = Arc::new(LibSqlRootFilesystem::new(db));
    filesystem.run_migrations().await.unwrap();
    let view = MountView::new(vec![MountGrant::new(
        MountAlias::new("/turns").unwrap(),
        VirtualPath::new("/engine/tenants/tenant1/users/user1/turns").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(ScopedFilesystem::with_fixed_view(filesystem, view))
}

#[cfg(feature = "libsql")]
#[tokio::test]
async fn production_turn_state_selection_accepts_filesystem_turn_state_store() {
    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join("turn-state.db");
    let db = Arc::new(libsql::Builder::new_local(db_path).build().await.unwrap());
    let scoped = libsql_scoped_turns_fs(Arc::clone(&db)).await;

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_filesystem_turn_state_store(scoped);

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local services remain intentionally unready");
    assert!(
        !report.contains(
            ProductionWiringComponent::TurnState,
            ProductionWiringIssueKind::Missing
        ),
        "FilesystemTurnStateStore must satisfy production turn-state presence: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::TurnState,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "FilesystemTurnStateStore over LibSqlRootFilesystem must not be classified local-only: {report:?}"
    );
}

#[derive(Debug, Default)]
struct RecordingTurnRunWakeNotifier {
    wakes: Mutex<Vec<TurnRunWake>>,
}

impl RecordingTurnRunWakeNotifier {
    #[cfg(feature = "libsql")]
    fn wakes(&self) -> Vec<TurnRunWake> {
        self.wakes.lock().unwrap().clone()
    }
}

impl TurnRunWakeNotifier for RecordingTurnRunWakeNotifier {
    fn notify_queued_run(
        &self,
        wake: TurnRunWake,
    ) -> Result<(), brassclaw_turns::TurnRunWakeNotifyError> {
        self.wakes.lock().unwrap().push(wake);
        Ok(())
    }
}

#[cfg(feature = "libsql")]
#[tokio::test]
async fn production_turn_coordinator_uses_configured_store_and_notifier() {
    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join("turn-coordinator.db");
    let db = Arc::new(libsql::Builder::new_local(db_path).build().await.unwrap());
    let notifier = Arc::new(RecordingTurnRunWakeNotifier::default());
    let scoped = libsql_scoped_turns_fs(Arc::clone(&db)).await;

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_filesystem_turn_state_store(Arc::clone(&scoped))
    .with_run_profile_resolver(Arc::new(InMemoryRunProfileResolver::default()))
    .with_turn_run_wake_notifier(Arc::clone(&notifier));

    let coordinator = services
        .turn_coordinator_for_production()
        .expect("production-ready turn wiring should build coordinator");
    let request = submit_turn_request("thread-production-turn-coordinator", "idem-production-turn");
    let response = coordinator.submit_turn(request.clone()).await.unwrap();
    let SubmitTurnResponse::Accepted { run_id, .. } = response;

    let reopened = FilesystemTurnStateStore::new(scoped);
    let state = reopened
        .get_run_state(brassclaw_turns::GetRunStateRequest {
            scope: request.scope,
            run_id,
        })
        .await
        .unwrap();
    assert_eq!(state.run_id, run_id);
    assert_eq!(notifier.wakes().len(), 1);
    assert_eq!(notifier.wakes()[0].run_id, run_id);
}

#[cfg(feature = "libsql")]
#[tokio::test]
async fn production_turn_coordinator_requires_explicit_run_profile_resolver() {
    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join("turn-coordinator-missing-resolver.db");
    let db = Arc::new(libsql::Builder::new_local(db_path).build().await.unwrap());
    let scoped = libsql_scoped_turns_fs(Arc::clone(&db)).await;

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_filesystem_turn_state_store(scoped)
    .with_turn_run_wake_notifier(Arc::new(RecordingTurnRunWakeNotifier::default()));

    let report = match services.turn_coordinator_for_production() {
        Ok(_) => panic!("production turn coordinator must fail closed without a resolver"),
        Err(report) => report,
    };
    assert!(report.contains(
        ProductionWiringComponent::RunProfileResolver,
        ProductionWiringIssueKind::Missing
    ));
}

#[tokio::test]
async fn production_wiring_validation_rejects_noop_turn_wake_notifier() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_turn_run_wake_notifier(Arc::new(NoopTurnRunWakeNotifier));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local services remain intentionally unready");
    assert!(
        report.contains(
            ProductionWiringComponent::TurnRunWakeNotifier,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "NoopTurnRunWakeNotifier must not satisfy production turn wake wiring: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_accepts_configured_turn_wake_notifier() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_turn_run_wake_notifier(Arc::new(RecordingTurnRunWakeNotifier::default()));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local services remain intentionally unready");
    assert!(
        !report.contains(
            ProductionWiringComponent::TurnRunWakeNotifier,
            ProductionWiringIssueKind::Missing
        ),
        "configured turn wake notifier must satisfy production presence: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::TurnRunWakeNotifier,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "configured turn wake notifier must not be classified local-only: {report:?}"
    );
}

#[tokio::test]
async fn production_event_store_config_rejects_jsonl_without_single_node_acceptance() {
    let temp = tempfile::tempdir().unwrap();
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    );

    let result = services
        .with_reborn_event_store_config(
            RebornProfile::Production,
            RebornEventStoreConfig::Jsonl {
                root: temp.path().join("reborn-event-store"),
                accept_single_node_durable: false,
            },
        )
        .await;

    assert!(matches!(
        result,
        Err(RebornEventStoreError::ProductionJsonlRequiresAcceptance)
    ));
}

#[tokio::test]
async fn local_reborn_event_store_config_does_not_satisfy_production_wiring() {
    let temp = tempfile::tempdir().unwrap();
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_reborn_event_store_config(
        RebornProfile::LocalDev,
        RebornEventStoreConfig::Jsonl {
            root: temp.path().join("local-reborn-event-store"),
            accept_single_node_durable: false,
        },
    )
    .await
    .unwrap();

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("LocalDev stores are not production-verified event/audit sinks");

    assert!(
        report.contains(
            ProductionWiringComponent::EventSink,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "LocalDev Reborn event store must not satisfy production event sink guardrail: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::AuditSink,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "LocalDev Reborn audit store must not satisfy production audit sink guardrail: {report:?}"
    );
}

#[tokio::test]
async fn production_event_store_config_installs_verified_event_and_audit_sinks() {
    let temp = tempfile::tempdir().unwrap();
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_reborn_event_store_config(
        RebornProfile::Production,
        RebornEventStoreConfig::Jsonl {
            root: temp.path().join("accepted-reborn-event-store"),
            accept_single_node_durable: true,
        },
    )
    .await
    .unwrap();

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("other local test services are still not production-ready");

    assert!(
        !report.contains(
            ProductionWiringComponent::EventSink,
            ProductionWiringIssueKind::Missing
        ),
        "event sink must be installed from Reborn event store config: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::AuditSink,
            ProductionWiringIssueKind::Missing
        ),
        "audit sink must be installed from Reborn event store config: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::EventSink,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "Reborn durable event store adapter must not be treated as erased unverified sink: {report:?}"
    );
    assert!(
        !report.contains(
            ProductionWiringComponent::AuditSink,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "Reborn durable audit store adapter must not be treated as erased unverified sink: {report:?}"
    );
}


#[tokio::test]
async fn production_wiring_validation_sees_underlying_in_memory_durable_logs() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_durable_event_log(Arc::new(InMemoryDurableEventLog::new()))
    .with_durable_audit_log(Arc::new(InMemoryDurableAuditLog::new()));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("in-memory durable logs must not be hidden behind durable sink wrappers");

    assert!(
        report.contains(
            ProductionWiringComponent::EventSink,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory durable event log should be reported through with_durable_event_log: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::AuditSink,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory durable audit log should be reported through with_durable_audit_log: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_rejects_direct_durable_sink_wrappers_as_unverified() {
    let event_log: Arc<dyn DurableEventLog> = Arc::new(InMemoryDurableEventLog::new());
    let audit_log: Arc<dyn DurableAuditLog> = Arc::new(InMemoryDurableAuditLog::new());
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_event_sink(Arc::new(DurableEventSink::new(event_log)))
    .with_audit_sink(Arc::new(DurableAuditSink::new(audit_log)));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("direct durable sink wrappers must not hide erased underlying log types");

    assert!(
        report.contains(
            ProductionWiringComponent::EventSink,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "direct durable event sink wrapper should require typed with_durable_event_log path: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::AuditSink,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "direct durable audit sink wrapper should require typed with_durable_audit_log path: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_accepts_verified_host_http_egress_shape() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_secret_store(Arc::new(InMemorySecretStore::new()));
    let services = services
        .try_with_host_http_egress(RecordingNetworkHttpEgress::new())
        .unwrap();

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]).require_runtime_http_egress());

    assert!(
        report.as_ref().err().is_none_or(|report| !report.contains(
            ProductionWiringComponent::RuntimeHttpEgress,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        )),
        "verified host HTTP egress should satisfy the runtime egress guardrail: {report:?}"
    );
}

#[tokio::test]
async fn host_http_egress_helper_requires_graph_secret_store() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    );

    let report = match services.try_with_host_http_egress(RecordingNetworkHttpEgress::new()) {
        Ok(_) => panic!("host HTTP egress helper must use configured graph secret store"),
        Err(report) => report,
    };

    assert!(report.contains(
        ProductionWiringComponent::SecretStore,
        ProductionWiringIssueKind::Missing
    ));
}

#[tokio::test]
async fn production_wiring_validation_requires_credential_broker_when_configured() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    );

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]).require_credential_broker())
        .expect_err("production credential broker requirement must fail closed when missing");

    assert!(
        report.contains(
            ProductionWiringComponent::CredentialAccountStore,
            ProductionWiringIssueKind::Missing
        ),
        "missing credential account store should be reported: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::CredentialSessionStore,
            ProductionWiringIssueKind::Missing
        ),
        "missing credential session store should be reported: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_rejects_local_only_credential_broker() {
    let broker = Arc::new(InMemoryCredentialBroker::new());
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_credential_broker(broker);

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]).require_credential_broker())
        .expect_err("in-memory credential broker must not satisfy production guardrail");

    assert!(
        report.contains(
            ProductionWiringComponent::CredentialAccountStore,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory credential account store should be reported as local-only: {report:?}"
    );
    assert!(
        report.contains(
            ProductionWiringComponent::CredentialSessionStore,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "in-memory credential session store should be reported as local-only: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_rejects_unverified_runtime_http_egress() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_runtime_http_egress(Arc::new(RecordingRuntimeHttpEgress::new()));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]).require_runtime_http_egress())
        .expect_err(
            "generic/test runtime HTTP egress must not satisfy production egress guardrail",
        );

    assert!(
        report.contains(
            ProductionWiringComponent::RuntimeHttpEgress,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "runtime HTTP egress should require production verification: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_tracks_process_port_for_builtin_shell() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_builtin_first_party_package()),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(
        builtin_first_party_handlers(Arc::new(InMemoryTriggerRepository::default())).unwrap(),
    ));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([RuntimeKind::FirstParty]))
        .expect_err("default local process port must not satisfy production shell wiring");

    assert!(
        report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "builtin shell should make the local process port visible to production guardrails: {report:?}"
    );

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_builtin_first_party_package()),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(
        builtin_first_party_handlers(Arc::new(InMemoryTriggerRepository::default())).unwrap(),
    ))
    .with_runtime_process_port(Arc::new(ProductionCandidateProcessPort));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([RuntimeKind::FirstParty]))
        .expect_err("other local defaults should still keep this graph non-production");

    assert!(
        !report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::LocalOnlyImplementation
        ),
        "custom process port should clear the process-port local-only issue: {report:?}"
    );
}

#[tokio::test]
async fn production_wiring_validation_tracks_tenant_sandbox_process_port_for_builtin_shell() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_builtin_first_party_package()),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(
        builtin_first_party_handlers(Arc::new(InMemoryTriggerRepository::default())).unwrap(),
    ))
    .with_runtime_policy(hosted_dev_runtime_policy());

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([RuntimeKind::FirstParty]))
        .expect_err("tenant sandbox process policy must require a sandbox process port");

    assert!(
        report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::Missing
        ),
        "tenant sandbox process backend should require the tenant sandbox process port: {report:?}"
    );

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_builtin_first_party_package()),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(
        builtin_first_party_handlers(Arc::new(InMemoryTriggerRepository::default())).unwrap(),
    ))
    .with_runtime_policy(hosted_dev_runtime_policy())
    .with_tenant_sandbox_process_port(Arc::new(TenantSandboxProcessPort::new(Arc::new(
        ProductionCandidateSandboxTransport,
    ))));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([RuntimeKind::FirstParty]))
        .expect_err(
            "sandbox port readiness must remain explicit until a production transport is wired",
        );

    assert!(
        !report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::Missing
        ) && report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "configured tenant sandbox process port should clear missing but remain unverified: {report:?}"
    );

    let services = HostRuntimeServices::new(
        Arc::new(registry_with_builtin_first_party_package()),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(
        builtin_first_party_handlers(Arc::new(InMemoryTriggerRepository::default())).unwrap(),
    ))
    .with_runtime_policy(hosted_dev_runtime_policy())
    .with_production_tenant_sandbox_process_port(Arc::new(TenantSandboxProcessPort::new(
        Arc::new(ProductionCandidateSandboxTransport),
    )));

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([RuntimeKind::FirstParty]))
        .expect_err("test service graph still uses local-only backing stores");

    assert!(
        !report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::Missing
        ) && !report.contains(
            ProductionWiringComponent::RuntimeProcessPort,
            ProductionWiringIssueKind::UnverifiedProductionImplementation
        ),
        "verified tenant sandbox process port should satisfy the process-port gate: {report:?}"
    );
}


#[tokio::test]
async fn process_lifecycle_projects_through_durable_replay_without_output_leaks() {
    let event_log = Arc::new(InMemoryDurableEventLog::new());
    let inner_process_store = Arc::new(InMemoryProcessStore::new());
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(InMemorySecretStore::new()),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let process_store =
        Arc::new(obligation_services.process_obligation_lifecycle_store(inner_process_store));
    let durable_event_log: Arc<dyn DurableEventLog> = event_log.clone();
    process_store.set_event_sink(Arc::new(DurableEventSink::new(durable_event_log)));
    let result_store = Arc::new(InMemoryProcessResultStore::new());
    let manager = BackgroundProcessManager::new(
        Arc::clone(&process_store),
        Arc::new(BackgroundExecutor::success_with_output(json!({
            "result": "PROCESS_OUTPUT_SENTINEL_3022 /tmp/process-output-private"
        }))),
    )
    .with_result_store(Arc::clone(&result_store));
    let process_id = ProcessId::new();
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);

    let process = manager
        .spawn(process_start(process_id, invocation_id, scope.clone()))
        .await
        .unwrap();
    wait_for_status(
        process_store.as_ref(),
        &scope,
        process.process_id,
        ProcessStatus::Completed,
    )
    .await;

    let host =
        ProcessHost::new(process_store.as_ref()).with_result_store(Arc::clone(&result_store));
    let output = host
        .output(&scope, process.process_id)
        .await
        .unwrap()
        .expect("process output should be available through ProcessHost");
    assert_eq!(
        output,
        json!({"result": "PROCESS_OUTPUT_SENTINEL_3022 /tmp/process-output-private"})
    );

    let projection = ReplayEventProjectionService::new(Arc::clone(&event_log));
    let snapshot = projection
        .snapshot(ProjectionRequest {
            scope: ProjectionScope::for_process(&scope, process.process_id),
            after: None,
            limit: 10,
        })
        .await
        .unwrap();

    assert_eq!(
        snapshot
            .timeline
            .entries
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        vec![
            TimelineEntryKind::ProcessStarted,
            TimelineEntryKind::ProcessCompleted,
        ]
    );
    assert_eq!(snapshot.runs.len(), 1);
    assert_eq!(snapshot.runs[0].status, RunProjectionStatus::Completed);
    assert_eq!(snapshot.runs[0].process_id, Some(process.process_id));

    let foreign_scope = ResourceScope {
        project_id: Some(ProjectId::new("foreign-project").unwrap()),
        ..scope.clone()
    };
    let foreign_snapshot = projection
        .snapshot(ProjectionRequest {
            scope: ProjectionScope::for_process(&foreign_scope, process.process_id),
            after: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert!(foreign_snapshot.timeline.entries.is_empty());

    let projection_json = serde_json::to_string(&snapshot).unwrap();
    let replay_json = serde_json::to_string(
        &event_log
            .read_after_cursor(
                &EventStreamKey::from_scope(&scope),
                &ReadScope::any(),
                None,
                10,
            )
            .await
            .unwrap(),
    )
    .unwrap();
    for forbidden in [
        "PROCESS_OUTPUT_SENTINEL_3022",
        "/tmp/process-output-private",
    ] {
        assert!(
            !projection_json.contains(forbidden),
            "process projection leaked {forbidden}: {projection_json}"
        );
        assert!(
            !replay_json.contains(forbidden),
            "process durable replay leaked {forbidden}: {replay_json}"
        );
    }
}

#[tokio::test]
async fn host_runtime_services_cancel_projects_kill_event_from_configured_event_sink() {
    let event_log = Arc::new(InMemoryDurableEventLog::new());
    let process_services = ProcessServices::new(
        Arc::new(InMemoryProcessStore::new()),
        Arc::new(InMemoryProcessResultStore::new()),
    );
    let process_store = process_services.process_store();
    let result_store = process_services.result_store();
    let runtime = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_durable_event_log(Arc::clone(&event_log))
    .host_runtime_for_local_testing();
    let process_id = ProcessId::new();
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    let mut start = process_start(process_id, invocation_id, scope.clone());
    start.input = json!({
        "message": "KILL_PROCESS_INPUT_SENTINEL_3022 /tmp/process-kill-private"
    });
    process_store.start(start).await.unwrap();

    let outcome = runtime
        .cancel_work(CancelRuntimeWorkRequest::new(
            scope.clone(),
            CorrelationId::new(),
            CancelReason::UserRequested,
        ))
        .await
        .unwrap();
    assert_eq!(outcome.cancelled, vec![RuntimeWorkId::Process(process_id)]);
    assert_eq!(
        result_store
            .get(&scope, process_id)
            .await
            .unwrap()
            .expect("cancel should persist killed process result")
            .status,
        ProcessStatus::Killed
    );

    let projection = ReplayEventProjectionService::new(Arc::clone(&event_log));
    let snapshot = projection
        .snapshot(ProjectionRequest {
            scope: ProjectionScope::for_process(&scope, process_id),
            after: None,
            limit: 10,
        })
        .await
        .unwrap();

    assert_eq!(snapshot.timeline.entries.len(), 1);
    assert_eq!(
        snapshot.timeline.entries[0].kind,
        TimelineEntryKind::ProcessKilled
    );
    assert_eq!(snapshot.runs.len(), 1);
    assert_eq!(snapshot.runs[0].status, RunProjectionStatus::Killed);

    let projection_json = serde_json::to_string(&snapshot).unwrap();
    let replay_json = serde_json::to_string(
        &event_log
            .read_after_cursor(
                &EventStreamKey::from_scope(&scope),
                &ReadScope::any(),
                None,
                10,
            )
            .await
            .unwrap(),
    )
    .unwrap();
    for forbidden in [
        "KILL_PROCESS_INPUT_SENTINEL_3022",
        "/tmp/process-kill-private",
    ] {
        assert!(
            !projection_json.contains(forbidden),
            "kill projection leaked {forbidden}: {projection_json}"
        );
        assert!(
            !replay_json.contains(forbidden),
            "kill durable replay leaked {forbidden}: {replay_json}"
        );
    }
}

#[tokio::test]
async fn host_runtime_services_resumes_approved_capability_and_consumes_lease_once() {
    let fixture = approval_resume_fixture();
    let runtime = fixture.services.host_runtime_for_local_testing();
    let context = execution_context_without_grants();
    let scope = context.resource_scope.clone();
    let estimate = ResourceEstimate::default();
    let input = json!({"message": "approval resume"});

    let gate = block_for_approval(&runtime, context.clone(), estimate.clone(), input.clone()).await;
    let lease =
        approve_dispatch_for_services(&fixture.services, &scope, gate.approval_request_id, None)
            .await;

    let resumed = runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            context.clone(),
            gate.approval_request_id,
            script_capability_id(),
            estimate.clone(),
            input.clone(),
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    match resumed {
        RuntimeCapabilityOutcome::Completed(completed) => {
            assert_eq!(completed.capability_id, script_capability_id());
            assert_eq!(completed.output, input);
        }
        other => panic!("expected completed resume outcome, got {other:?}"),
    }
    assert_eq!(
        fixture
            .capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Consumed
    );
    let kinds = fixture
        .events
        .events()
        .into_iter()
        .map(|event| event.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            RuntimeEventKind::DispatchRequested,
            RuntimeEventKind::RuntimeSelected,
            RuntimeEventKind::DispatchSucceeded,
        ]
    );

    let second = runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            context,
            gate.approval_request_id,
            script_capability_id(),
            estimate,
            input,
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(second, RuntimeFailureKind::Authorization);
    assert_eq!(
        fixture.events.events().len(),
        3,
        "second resume must fail before a second dispatch"
    );
}


#[tokio::test]
async fn host_runtime_services_resume_changed_input_fails_before_lease_claim_or_dispatch() {
    let fixture = approval_resume_fixture();
    let runtime = fixture.services.host_runtime_for_local_testing();
    let context = execution_context_without_grants();
    let scope = context.resource_scope.clone();
    let estimate = ResourceEstimate::default();
    let original_input = json!({"message": "original"});

    let gate =
        block_for_approval(&runtime, context.clone(), estimate.clone(), original_input).await;
    let lease =
        approve_dispatch_for_services(&fixture.services, &scope, gate.approval_request_id, None)
            .await;

    let outcome = runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            context,
            gate.approval_request_id,
            script_capability_id(),
            estimate,
            json!({"message": "changed"}),
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Authorization);
    assert!(fixture.events.events().is_empty());
    // The approval request stores the original invocation fingerprint; changed input
    // computes a different resume fingerprint, so no matching lease is claimable.
    assert_eq!(
        fixture
            .capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active,
        "fingerprint mismatch must fail before lease claim/consume"
    );
}

#[tokio::test]
async fn host_runtime_services_resume_wrong_user_scope_is_hidden_before_dispatch() {
    let fixture = approval_resume_fixture();
    let runtime = fixture.services.host_runtime_for_local_testing();
    let context = execution_context_without_grants();
    let scope = context.resource_scope.clone();
    let estimate = ResourceEstimate::default();
    let input = json!({"message": "wrong user"});

    let gate = block_for_approval(&runtime, context.clone(), estimate.clone(), input.clone()).await;
    let lease =
        approve_dispatch_for_services(&fixture.services, &scope, gate.approval_request_id, None)
            .await;
    let wrong_scope = ResourceScope {
        user_id: UserId::new("other-user").unwrap(),
        ..scope.clone()
    };
    let wrong_context =
        execution_context_with_dispatch_grant_for_scope(script_capability_id(), wrong_scope);

    let outcome = runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            wrong_context,
            gate.approval_request_id,
            script_capability_id(),
            estimate,
            input,
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Backend);
    assert!(fixture.events.events().is_empty());
    let original_run = fixture
        .run_state
        .get(&scope, context.invocation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original_run.status, RunStatus::BlockedApproval);
    assert_eq!(
        original_run.approval_request_id,
        Some(gate.approval_request_id)
    );
    assert_eq!(
        fixture
            .capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active
    );
}

#[tokio::test]
async fn host_runtime_services_resume_expired_lease_fails_before_dispatch() {
    let fixture = approval_resume_fixture();
    let runtime = fixture.services.host_runtime_for_local_testing();
    let context = execution_context_without_grants();
    let scope = context.resource_scope.clone();
    let estimate = ResourceEstimate::default();
    let input = json!({"message": "expired"});

    let gate = block_for_approval(&runtime, context.clone(), estimate.clone(), input.clone()).await;
    let lease = approve_dispatch_for_services(
        &fixture.services,
        &scope,
        gate.approval_request_id,
        Some(Utc::now() - ChronoDuration::seconds(1)),
    )
    .await;

    let outcome = runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            context,
            gate.approval_request_id,
            script_capability_id(),
            estimate,
            input,
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Authorization);
    assert!(fixture.events.events().is_empty());
    assert_eq!(
        fixture
            .capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active
    );
}

#[tokio::test]
async fn host_runtime_services_resume_trust_preflight_failure_fails_only_matching_blocked_run() {
    let fixture = approval_resume_fixture();
    let runtime = fixture.services.host_runtime_for_local_testing();
    let context = execution_context_without_grants();
    let scope = context.resource_scope.clone();
    let estimate = ResourceEstimate::default();
    let input = json!({"message": "stale trust metadata"});

    let gate = block_for_approval(&runtime, context.clone(), estimate.clone(), input.clone()).await;
    let lease =
        approve_dispatch_for_services(&fixture.services, &scope, gate.approval_request_id, None)
            .await;
    let broken_runtime = resume_runtime_with_empty_registry(&fixture);

    let wrong_scope = ResourceScope {
        user_id: UserId::new("other-user").unwrap(),
        ..scope.clone()
    };
    let wrong_context = execution_context_without_grants_for_scope(wrong_scope);
    let wrong_scope_outcome = broken_runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            wrong_context,
            gate.approval_request_id,
            script_capability_id(),
            estimate.clone(),
            input.clone(),
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();
    assert_failed_outcome(wrong_scope_outcome, RuntimeFailureKind::MissingRuntime);
    assert_blocked_approval_run(
        &fixture,
        &scope,
        context.invocation_id,
        gate.approval_request_id,
    )
    .await;

    let mut invalid_context = context.clone();
    invalid_context.user_id = UserId::new("tampered-user").unwrap();
    let invalid_context_outcome = broken_runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            invalid_context,
            gate.approval_request_id,
            script_capability_id(),
            estimate.clone(),
            input.clone(),
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();
    assert_failed_outcome(invalid_context_outcome, RuntimeFailureKind::MissingRuntime);
    assert_blocked_approval_run(
        &fixture,
        &scope,
        context.invocation_id,
        gate.approval_request_id,
    )
    .await;

    let matching_outcome = broken_runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            context.clone(),
            gate.approval_request_id,
            script_capability_id(),
            estimate,
            input,
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();
    assert_failed_outcome(matching_outcome, RuntimeFailureKind::MissingRuntime);

    let failed_run = fixture
        .run_state
        .get(&scope, context.invocation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed_run.status, RunStatus::Failed);
    assert_eq!(failed_run.approval_request_id, None);
    assert_eq!(failed_run.error_kind.as_deref(), Some("unknown_capability"));
    assert_eq!(
        fixture
            .capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active,
        "trust preflight failure must not claim or consume the approval lease"
    );
    assert!(fixture.events.events().is_empty());
}

#[tokio::test]
async fn host_runtime_services_resume_runtime_policy_denial_fails_matching_blocked_run() {
    let fixture = approval_resume_fixture_with_manifest(
        SCRIPT_NETWORK_MANIFEST,
        vec![EffectKind::DispatchCapability, EffectKind::Network],
    );
    let runtime = fixture.services.host_runtime_for_local_testing();
    let context = execution_context_without_grants();
    let scope = context.resource_scope.clone();
    let estimate = ResourceEstimate::default();
    let input = json!({"message": "policy reduced before resume"});

    let gate = block_for_approval(&runtime, context.clone(), estimate.clone(), input.clone()).await;
    let lease =
        approve_dispatch_for_services(&fixture.services, &scope, gate.approval_request_id, None)
            .await;
    let denied_runtime = resume_runtime_with_policy(&fixture, network_denied_runtime_policy());

    let outcome = denied_runtime
        .resume_capability(RuntimeCapabilityResumeRequest::new(
            context.clone(),
            gate.approval_request_id,
            script_capability_id(),
            estimate,
            input,
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Authorization);
    let failed_run = fixture
        .run_state
        .get(&scope, context.invocation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed_run.status, RunStatus::Failed);
    assert_eq!(failed_run.approval_request_id, None);
    assert_eq!(
        failed_run.error_kind.as_deref(),
        Some("process_backend_none")
    );
    assert_eq!(
        fixture
            .capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active,
        "runtime-policy preflight failure must not claim or consume the approval lease"
    );
    assert!(fixture.events.events().is_empty());
}




#[tokio::test]
async fn host_runtime_routes_system_process_sandbox_to_configured_executor() {
    let process_services = ProcessServices::in_memory();
    let result_store = process_services.result_store();
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let runtime = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor))
    .host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());
    let process_id = ProcessId::new();

    let handle = runtime
        .spawn_process(process_sandbox_start(process_id, scope.clone()))
        .await
        .unwrap();

    assert_eq!(handle.process_id, process_id);
    assert_eq!(handle.capability_id, process_sandbox_capability_id());
    wait_for_sandbox_process_result(&sandbox_executor, &scope, process_id, result_store.as_ref())
        .await;
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_routes_approved_request_to_configured_executor() {
    let process_services = ProcessServices::in_memory();
    let result_store = process_services.result_store();
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let runtime = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor))
    .host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());

    let outcome = runtime
        .spawn_capability(process_sandbox_runtime_request_for_scope(scope.clone()))
        .await
        .unwrap();

    let process_id = match outcome {
        RuntimeCapabilityOutcome::SpawnedProcess(handle) => {
            assert_eq!(handle.capability_id, process_sandbox_capability_id());
            handle.process_id
        }
        other => panic!("expected spawned process, got {other:?}"),
    };
    wait_for_sandbox_process_result(&sandbox_executor, &scope, process_id, result_store.as_ref())
        .await;
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_rejects_invalid_plan_before_executor() {
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let runtime = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor))
    .host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());
    let mut request = process_sandbox_runtime_request_for_scope(scope);
    request.input = invalid_process_sandbox_input();

    let error = runtime
        .spawn_capability(request)
        .await
        .expect_err("invalid sandbox plans must fail at the host runtime boundary");

    match error {
        brassclaw_host_runtime::HostRuntimeError::InvalidRequest { reason } => {
            assert!(reason.contains("SandboxProcessPlan"));
        }
        other => panic!("expected invalid request, got {other:?}"),
    }
    assert!(
        sandbox_executor.requests().is_empty(),
        "invalid sandbox plan must not reach process spawn"
    );
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_runtime_policy_denial_fails_before_executor() {
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let runtime = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor))
    .with_runtime_policy(network_denied_runtime_policy())
    .host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());

    let outcome = runtime
        .spawn_capability(process_sandbox_runtime_request_for_scope(scope))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Authorization);
    assert!(
        sandbox_executor.requests().is_empty(),
        "runtime policy denial must fail before process spawn"
    );
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_host_failure_fails_after_preflight() {
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let runtime = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor))
    .host_runtime_for_local_testing()
    .with_process_manager(Arc::new(FailingSpawnManager));
    let scope = sample_scope(InvocationId::new());

    let outcome = runtime
        .spawn_capability(process_sandbox_runtime_request_for_scope(scope))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Backend);
    assert!(
        sandbox_executor.requests().is_empty(),
        "host spawn failure must not reach the process sandbox executor"
    );
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_blocks_for_approval_before_executor() {
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let approval_requests = Arc::new(InMemoryApprovalRequestStore::new());
    let capability_leases = Arc::new(InMemoryCapabilityLeaseStore::new());
    let process_services = ProcessServices::in_memory();
    let result_store = process_services.result_store();
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_run_state(Arc::clone(&run_state))
    .with_approval_requests(Arc::clone(&approval_requests))
    .with_capability_leases(Arc::clone(&capability_leases))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor));
    let runtime = services.host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());
    let context = execution_context_without_grants_for_scope(scope.clone());
    let input = process_sandbox_input();
    let estimate = process_sandbox_estimate();

    let blocked = runtime
        .spawn_capability(RuntimeCapabilityRequest::new(
            context.clone(),
            process_sandbox_capability_id(),
            estimate.clone(),
            input.clone(),
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    let approval_request_id = match blocked {
        RuntimeCapabilityOutcome::ApprovalRequired(gate) => {
            assert_eq!(gate.capability_id, process_sandbox_capability_id());
            gate.approval_request_id
        }
        other => panic!("expected approval gate, got {other:?}"),
    };
    assert!(
        sandbox_executor.requests().is_empty(),
        "process sandbox executor must not run before approval"
    );

    approve_spawn_for_services(&services, &scope, approval_request_id, None).await;
    let resumed = runtime
        .resume_spawn_capability(RuntimeCapabilityResumeRequest::new(
            context,
            approval_request_id,
            process_sandbox_capability_id(),
            estimate,
            input,
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    let process_id = match resumed {
        RuntimeCapabilityOutcome::SpawnedProcess(handle) => handle.process_id,
        other => panic!("expected spawned process after approval, got {other:?}"),
    };
    wait_for_sandbox_process_result(&sandbox_executor, &scope, process_id, result_store.as_ref())
        .await;
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_resume_changed_input_fails_before_executor() {
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let approval_requests = Arc::new(InMemoryApprovalRequestStore::new());
    let capability_leases = Arc::new(InMemoryCapabilityLeaseStore::new());
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_run_state(Arc::clone(&run_state))
    .with_approval_requests(Arc::clone(&approval_requests))
    .with_capability_leases(Arc::clone(&capability_leases))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor));
    let runtime = services.host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());
    let context = execution_context_without_grants_for_scope(scope.clone());
    let input = process_sandbox_input();
    let estimate = process_sandbox_estimate();

    let blocked = runtime
        .spawn_capability(RuntimeCapabilityRequest::new(
            context.clone(),
            process_sandbox_capability_id(),
            estimate.clone(),
            input,
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    let approval_request_id = match blocked {
        RuntimeCapabilityOutcome::ApprovalRequired(gate) => gate.approval_request_id,
        other => panic!("expected approval gate, got {other:?}"),
    };
    let lease = approve_spawn_for_services(&services, &scope, approval_request_id, None).await;

    let outcome = runtime
        .resume_spawn_capability(RuntimeCapabilityResumeRequest::new(
            context,
            approval_request_id,
            process_sandbox_capability_id(),
            estimate,
            json!({"run": {"command": "echo", "args": ["changed"]}}),
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Authorization);
    assert!(
        sandbox_executor.requests().is_empty(),
        "changed resume input must fail before process spawn"
    );
    assert_eq!(
        capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active,
        "fingerprint mismatch must fail before lease claim/consume"
    );
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_resume_invalid_plan_fails_before_executor() {
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let approval_requests = Arc::new(InMemoryApprovalRequestStore::new());
    let capability_leases = Arc::new(InMemoryCapabilityLeaseStore::new());
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_run_state(Arc::clone(&run_state))
    .with_approval_requests(Arc::clone(&approval_requests))
    .with_capability_leases(Arc::clone(&capability_leases))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor));
    let runtime = services.host_runtime_for_local_testing();
    let scope = sample_scope(InvocationId::new());
    let context = execution_context_without_grants_for_scope(scope.clone());
    let input = process_sandbox_input();
    let estimate = process_sandbox_estimate();

    let blocked = runtime
        .spawn_capability(RuntimeCapabilityRequest::new(
            context.clone(),
            process_sandbox_capability_id(),
            estimate.clone(),
            input,
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    let approval_request_id = match blocked {
        RuntimeCapabilityOutcome::ApprovalRequired(gate) => gate.approval_request_id,
        other => panic!("expected approval gate, got {other:?}"),
    };
    let lease = approve_spawn_for_services(&services, &scope, approval_request_id, None).await;

    let error = runtime
        .resume_spawn_capability(RuntimeCapabilityResumeRequest::new(
            context,
            approval_request_id,
            process_sandbox_capability_id(),
            estimate,
            invalid_process_sandbox_input(),
            process_sandbox_trust_decision(),
        ))
        .await
        .expect_err("invalid sandbox resume input must fail at the host runtime boundary");

    match error {
        brassclaw_host_runtime::HostRuntimeError::InvalidRequest { reason } => {
            assert!(reason.contains("SandboxProcessPlan"));
        }
        other => panic!("expected invalid request, got {other:?}"),
    }
    assert!(
        sandbox_executor.requests().is_empty(),
        "invalid resume plan must not reach process spawn"
    );
    assert_eq!(
        capability_leases
            .get(&scope, lease.grant.id)
            .await
            .unwrap()
            .status,
        CapabilityLeaseStatus::Active,
        "invalid resume input must fail before lease claim/consume"
    );
}

#[tokio::test]
async fn host_runtime_spawn_process_sandbox_resume_host_failure_fails_after_approval() {
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let approval_requests = Arc::new(InMemoryApprovalRequestStore::new());
    let capability_leases = Arc::new(InMemoryCapabilityLeaseStore::new());
    let sandbox_executor = Arc::new(RecordingSandboxProcessExecutor::default());
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_host_bundled_manifest(
            PROCESS_SANDBOX_MANIFEST,
        )),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "system.process_sandbox",
        process_sandbox_authority_effects(),
    )))
    .with_run_state(Arc::clone(&run_state))
    .with_approval_requests(Arc::clone(&approval_requests))
    .with_capability_leases(Arc::clone(&capability_leases))
    .with_process_sandbox_executor(Arc::clone(&sandbox_executor));
    let runtime = services
        .host_runtime_for_local_testing()
        .with_process_manager(Arc::new(FailingSpawnManager));
    let scope = sample_scope(InvocationId::new());
    let context = execution_context_without_grants_for_scope(scope.clone());
    let input = process_sandbox_input();
    let estimate = process_sandbox_estimate();

    let blocked = runtime
        .spawn_capability(RuntimeCapabilityRequest::new(
            context.clone(),
            process_sandbox_capability_id(),
            estimate.clone(),
            input.clone(),
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    let approval_request_id = match blocked {
        RuntimeCapabilityOutcome::ApprovalRequired(gate) => gate.approval_request_id,
        other => panic!("expected approval gate, got {other:?}"),
    };
    approve_spawn_for_services(&services, &scope, approval_request_id, None).await;

    let outcome = runtime
        .resume_spawn_capability(RuntimeCapabilityResumeRequest::new(
            context,
            approval_request_id,
            process_sandbox_capability_id(),
            estimate,
            input,
            process_sandbox_trust_decision(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Backend);
    assert!(
        sandbox_executor.requests().is_empty(),
        "host resume-spawn failure must not reach the process sandbox executor"
    );
}



#[tokio::test]
async fn host_runtime_services_maps_mcp_client_failure_through_private_adapter() {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(MCP_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ObligatingAuthorizer::new(Vec::new())),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_runtime_http_egress(Arc::new(RecordingRuntimeHttpEgress::new()))
    .with_mcp_runtime(Arc::new(ClientErrorMcpExecutor));

    let outcome = services
        .host_runtime_for_local_testing()
        .invoke_capability(RuntimeCapabilityRequest::new(
            execution_context_with_dispatch_grant(mcp_capability_id()),
            mcp_capability_id(),
            ResourceEstimate::default(),
            json!({"query": "fail through services"}),
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::Backend);
}






#[tokio::test]
async fn host_runtime_services_releases_reservation_when_dispatch_preflight_fails_after_obligations()
 {
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let scope = sample_scope(InvocationId::new());
    let account = ResourceAccount::tenant(scope.tenant_id.clone());
    governor
        .set_limit(
            account.clone(),
            ResourceLimits {
                max_concurrency_slots: Some(1),
                ..ResourceLimits::default()
            },
        )
        .unwrap();
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let reservation_id = ResourceReservationId::new();
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::clone(&governor),
        Arc::new(ObligatingAuthorizer::new(vec![
            Obligation::ReserveResources { reservation_id },
        ])),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_run_state(Arc::clone(&run_state))
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "script",
        vec![EffectKind::DispatchCapability],
    )));

    let outcome = services
        .host_runtime_for_local_testing()
        .invoke_capability(RuntimeCapabilityRequest::new(
            execution_context_with_dispatch_grant_for_scope(script_capability_id(), scope.clone()),
            script_capability_id(),
            ResourceEstimate {
                concurrency_slots: Some(1),
                ..ResourceEstimate::default()
            },
            json!({"message": "missing runtime after reservation"}),
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    assert_failed_outcome(outcome, RuntimeFailureKind::MissingRuntime);
    assert_eq!(governor.reserved_for(&account), Default::default());
    assert!(matches!(
        governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
    let run = run_state
        .get(&scope, scope.invocation_id)
        .await
        .unwrap()
        .expect("run state should record the failed invocation");
    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.error_kind.as_deref(), Some("Dispatch"));
}


#[tokio::test]
async fn host_runtime_services_cancel_and_status_share_process_result_and_cancellation_graph() {
    let process_services = ProcessServices::in_memory();
    let process_store = process_services.process_store();
    let result_store = process_services.result_store();
    let cancellation_registry = process_services.cancellation_registry();
    let registry = Arc::new(registry_with_manifest(SCRIPT_MANIFEST));
    let runtime = HostRuntimeServices::new(
        registry,
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .host_runtime_for_local_testing();
    let invocation_id = InvocationId::new();
    let process_id = ProcessId::new();
    let scope = sample_scope(invocation_id);
    let token = cancellation_registry.register(&scope, process_id);
    process_store
        .start(process_start(process_id, invocation_id, scope.clone()))
        .await
        .unwrap();

    let status = runtime
        .runtime_status(RuntimeStatusRequest::new(
            scope.clone(),
            CorrelationId::new(),
        ))
        .await
        .unwrap();
    assert_eq!(status.active_work.len(), 1);
    assert_eq!(
        status.active_work[0].work_id,
        RuntimeWorkId::Process(process_id)
    );

    let outcome = runtime
        .cancel_work(CancelRuntimeWorkRequest::new(
            scope.clone(),
            CorrelationId::new(),
            CancelReason::UserRequested,
        ))
        .await
        .unwrap();

    assert_eq!(outcome.cancelled, vec![RuntimeWorkId::Process(process_id)]);
    assert!(token.is_cancelled());
    let result = result_store.get(&scope, process_id).await.unwrap().unwrap();
    assert_eq!(result.status, ProcessStatus::Killed);
}

#[tokio::test]
async fn host_runtime_services_cancel_writes_killed_result_when_reservation_is_stale() {
    let process_services = ProcessServices::in_memory();
    let process_store = process_services.process_store();
    let result_store = process_services.result_store();
    let cancellation_registry = process_services.cancellation_registry();
    let registry = Arc::new(registry_with_manifest(SCRIPT_MANIFEST));
    let runtime = HostRuntimeServices::new(
        registry,
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .host_runtime_for_local_testing();
    let invocation_id = InvocationId::new();
    let process_id = ProcessId::new();
    let stale_reservation_id = ResourceReservationId::new();
    let scope = sample_scope(invocation_id);
    let token = cancellation_registry.register(&scope, process_id);
    let mut start = process_start(process_id, invocation_id, scope.clone());
    start.resource_reservation_id = Some(stale_reservation_id);
    process_store.start(start).await.unwrap();

    let outcome = runtime
        .cancel_work(CancelRuntimeWorkRequest::new(
            scope.clone(),
            CorrelationId::new(),
            CancelReason::UserRequested,
        ))
        .await
        .unwrap();

    assert_eq!(outcome.cancelled, vec![RuntimeWorkId::Process(process_id)]);
    assert!(token.is_cancelled());
    let record = process_store
        .get(&scope, process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Killed);
    let result = result_store.get(&scope, process_id).await.unwrap().unwrap();
    assert_eq!(result.status, ProcessStatus::Killed);
}

#[tokio::test]
async fn host_runtime_services_cancel_records_kill_side_effects_when_cleanup_fails() {
    let process_services = ProcessServices::new(
        Arc::new(InMemoryProcessStore::new()),
        Arc::new(InMemoryProcessResultStore::new()),
    );
    let process_store = process_services.process_store();
    let result_store = process_services.result_store();
    let cancellation_registry = process_services.cancellation_registry();
    let registry = Arc::new(registry_with_manifest(SCRIPT_MANIFEST));
    let runtime = HostRuntimeServices::new(
        registry,
        Arc::new(LocalFilesystem::new()),
        Arc::new(FailingCleanupResourceGovernor),
        Arc::new(GrantAuthorizer::new()),
        process_services,
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .host_runtime_for_local_testing();
    let invocation_id = InvocationId::new();
    let process_id = ProcessId::new();
    let scope = sample_scope(invocation_id);
    let token = cancellation_registry.register(&scope, process_id);
    let mut start = process_start(process_id, invocation_id, scope.clone());
    start.resource_reservation_id = Some(ResourceReservationId::new());
    process_store.start(start).await.unwrap();

    let _error = runtime
        .cancel_work(CancelRuntimeWorkRequest::new(
            scope.clone(),
            CorrelationId::new(),
            CancelReason::UserRequested,
        ))
        .await
        .expect_err("cleanup failure should remain visible to callers");

    assert!(
        token.is_cancelled(),
        "cleanup errors after terminalization must not skip cooperative cancellation"
    );
    let record = process_store
        .get(&scope, process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Killed);
    let result = result_store
        .get(&scope, process_id)
        .await
        .unwrap()
        .expect("cleanup errors after terminalization must still write a killed result");
    assert_eq!(result.status, ProcessStatus::Killed);
}

#[tokio::test]
async fn spawned_obligation_lifecycle_reconciles_resources_and_discards_handoffs_on_success() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let fixture = spawn_obligation_fixture(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::success(),
    )
    .await;

    let process = fixture.spawn().await;
    wait_for_status(
        fixture.process_store.as_ref(),
        &fixture.scope,
        process.process_id,
        ProcessStatus::Completed,
    )
    .await;

    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Reconciled,
            ..
        }
    ));
}

#[tokio::test]
async fn spawned_obligation_lifecycle_releases_resources_and_discards_handoffs_on_runtime_failure()
{
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let fixture = spawn_obligation_fixture(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::failure("runtime_dispatch"),
    )
    .await;

    let process = fixture.spawn().await;
    wait_for_status(
        fixture.process_store.as_ref(),
        &fixture.scope,
        process.process_id,
        ProcessStatus::Failed,
    )
    .await;

    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

#[tokio::test]
async fn spawned_obligation_lifecycle_releases_resources_and_discards_handoffs_on_kill() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let fixture = spawn_obligation_fixture(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::delayed_success(Duration::from_millis(50)),
    )
    .await;

    let process = fixture.spawn().await;
    let host = ProcessHost::new(fixture.process_store.as_ref());
    host.kill(&fixture.scope, process.process_id).await.unwrap();

    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

#[tokio::test]
async fn process_obligation_lifecycle_cleans_record_started_before_wrapper_exists() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let inner_store = Arc::new(InMemoryProcessStore::new());
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(InMemorySecretStore::new()),
        governor.clone(),
    );
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    let estimate = ResourceEstimate {
        process_count: Some(1),
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    };
    governor
        .reserve_with_id(scope.clone(), estimate.clone(), reservation_id)
        .unwrap();
    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "runtime-secret",
    )
    .await;
    let process_id = ProcessId::new();
    let mut start = process_start(process_id, invocation_id, scope.clone());
    start.estimated_resources = estimate;
    start.resource_reservation_id = Some(reservation_id);
    inner_store.start(start).await.unwrap();

    let lifecycle_store = obligation_services.process_obligation_lifecycle_store(inner_store);
    lifecycle_store.kill(&scope, process_id).await.unwrap();

    assert!(matches!(
        governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

#[tokio::test]
async fn process_obligation_lifecycle_cleans_legacy_handoffs_without_resource_reservation() {
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let inner_store = Arc::new(InMemoryProcessStore::new());
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(InMemorySecretStore::new()),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "runtime-secret",
    )
    .await;
    let process_id = ProcessId::new();
    inner_store
        .start(process_start(process_id, invocation_id, scope.clone()))
        .await
        .unwrap();

    let lifecycle_store = obligation_services.process_obligation_lifecycle_store(inner_store);
    lifecycle_store.kill(&scope, process_id).await.unwrap();
}

#[tokio::test]
async fn process_obligation_lifecycle_rejects_second_active_handoff_for_same_scope_capability() {
    let inner_store = Arc::new(InMemoryProcessStore::new());
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(InMemorySecretStore::new()),
        Arc::new(InMemoryResourceGovernor::new()),
    );
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    let first_process_id = ProcessId::new();
    let second_process_id = ProcessId::new();
    let lifecycle_store = obligation_services.process_obligation_lifecycle_store(inner_store);
    let secret_handle = SecretHandle::new("api_token").unwrap();

    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "runtime-secret",
    )
    .await;
    lifecycle_store
        .start(process_start(
            first_process_id,
            invocation_id,
            scope.clone(),
        ))
        .await
        .unwrap();

    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "runtime-secret",
    )
    .await;
    let error = lifecycle_store
        .start(process_start(
            second_process_id,
            invocation_id,
            scope.clone(),
        ))
        .await
        .expect_err("a scoped capability may only have one active process handoff");

    assert!(matches!(error, ProcessError::InvalidStoredRecord { .. }));
    assert!(
        lifecycle_store
            .get(&scope, second_process_id)
            .await
            .unwrap()
            .is_none(),
        "the rejected second process must not be persisted as running"
    );

    lifecycle_store
        .complete(&scope, first_process_id)
        .await
        .unwrap();
    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "runtime-secret",
    )
    .await;
    lifecycle_store
        .start(process_start(
            second_process_id,
            invocation_id,
            scope.clone(),
        ))
        .await
        .expect("a new handoff can start after the prior handoff reaches terminal cleanup");
}

#[tokio::test]
async fn process_obligation_lifecycle_does_not_clean_handoffs_twice_after_background_cleanup() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let inner_store = Arc::new(InMemoryProcessStore::new());
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(InMemorySecretStore::new()),
        governor.clone(),
    );
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    let process_id = ProcessId::new();
    let estimate = ResourceEstimate {
        process_count: Some(1),
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    };
    governor
        .reserve_with_id(scope.clone(), estimate.clone(), reservation_id)
        .unwrap();
    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "first-runtime-secret",
    )
    .await;
    let lifecycle_store = obligation_services.process_obligation_lifecycle_store(inner_store);
    let mut start = process_start(process_id, invocation_id, scope.clone());
    start.estimated_resources = estimate;
    start.resource_reservation_id = Some(reservation_id);
    lifecycle_store.start(start).await.unwrap();

    lifecycle_store
        .cleanup_process_obligations(&scope, process_id, false)
        .await
        .unwrap();
    stage_process_handoffs(
        &obligation_services,
        &scope,
        &script_capability_id(),
        &secret_handle,
        sample_network_policy(),
        "second-runtime-secret",
    )
    .await;

    lifecycle_store.kill(&scope, process_id).await.unwrap();
}

#[tokio::test]
async fn process_obligation_lifecycle_surfaces_resource_cleanup_errors_after_terminal_transition() {
    let reservation_id = ResourceReservationId::new();
    let inner_store = Arc::new(InMemoryProcessStore::new());
    let governor = Arc::new(FailingCleanupResourceGovernor);
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(InMemorySecretStore::new()),
        governor.clone(),
    );
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    let process_id = ProcessId::new();
    let mut start = process_start(process_id, invocation_id, scope.clone());
    start.resource_reservation_id = Some(reservation_id);
    let lifecycle_store = obligation_services.process_obligation_lifecycle_store(inner_store);
    lifecycle_store.start(start).await.unwrap();

    let error = lifecycle_store
        .kill(&scope, process_id)
        .await
        .expect_err("terminal cleanup failures should be visible to callers");

    assert!(matches!(
        error,
        ProcessError::Resource(ResourceError::ReservationMismatch { id }) if id == reservation_id
    ));
    let record = lifecycle_store
        .get(&scope, process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Killed);
}

#[tokio::test]
async fn spawned_obligation_lifecycle_cleans_handoffs_when_result_store_complete_fails() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let result_store = Arc::new(FailingProcessResultStore::default());
    let fixture = spawn_obligation_fixture_with_result_store(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::success(),
        Arc::clone(&result_store),
    )
    .await;

    let process = fixture.spawn().await;
    wait_for_result_store_attempt(&result_store, "complete").await;
    wait_for_no_reserved_processes(&fixture.governor).await;

    let record = fixture
        .process_store
        .get(&fixture.scope, process.process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Running);
    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Reconciled,
            ..
        }
    ));
}

#[tokio::test]
async fn spawned_obligation_lifecycle_cleans_handoffs_when_result_store_fail_fails() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let result_store = Arc::new(FailingProcessResultStore::default());
    let fixture = spawn_obligation_fixture_with_result_store(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::failure("runtime_dispatch"),
        Arc::clone(&result_store),
    )
    .await;

    let process = fixture.spawn().await;
    wait_for_result_store_attempt(&result_store, "fail").await;
    wait_for_no_reserved_processes(&fixture.governor).await;

    let record = fixture
        .process_store
        .get(&fixture.scope, process.process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Running);
    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

#[tokio::test]
async fn spawned_obligation_lifecycle_reconciles_when_store_complete_fails_after_result_write() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let inner_process_store = Arc::new(FailingTerminalProcessStore::fail_complete());
    let fixture = spawn_obligation_fixture_with_process_store_and_result_store(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::success(),
        Arc::clone(&inner_process_store),
        Arc::new(InMemoryProcessResultStore::new()),
    )
    .await;

    let process = fixture.spawn().await;
    wait_for_process_store_attempt(&inner_process_store, "complete").await;
    wait_for_no_reserved_processes(&fixture.governor).await;

    let record = fixture
        .process_store
        .get(&fixture.scope, process.process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Running);
    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Reconciled,
            ..
        }
    ));
}

#[tokio::test]
async fn spawned_obligation_lifecycle_releases_when_store_fail_fails_after_result_write() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let inner_process_store = Arc::new(FailingTerminalProcessStore::fail_fail());
    let fixture = spawn_obligation_fixture_with_process_store_and_result_store(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::failure("runtime_dispatch"),
        Arc::clone(&inner_process_store),
        Arc::new(InMemoryProcessResultStore::new()),
    )
    .await;

    let process = fixture.spawn().await;
    wait_for_process_store_attempt(&inner_process_store, "fail").await;
    wait_for_no_reserved_processes(&fixture.governor).await;

    let record = fixture
        .process_store
        .get(&fixture.scope, process.process_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, ProcessStatus::Running);
    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

#[tokio::test]
async fn spawned_obligation_lifecycle_abort_cleans_up_when_process_start_fails() {
    let reservation_id = ResourceReservationId::new();
    let secret_handle = SecretHandle::new("api_token").unwrap();
    let fixture = spawn_obligation_fixture(
        reservation_id,
        secret_handle.clone(),
        BackgroundExecutor::success(),
    )
    .await;
    let failing_manager = FailingSpawnManager;
    let host = CapabilityHost::new(
        fixture.registry.as_ref(),
        fixture.dispatcher.as_ref(),
        fixture.authorizer.as_ref(),
    )
    .with_obligation_handler(fixture.handler.as_ref())
    .with_process_manager(&failing_manager);

    let err = host
        .spawn_json(CapabilitySpawnRequest {
            context: fixture.context.clone(),
            capability_id: script_capability_id(),
            estimate: fixture.estimate.clone(),
            input: json!({"message": "spawn fails"}),
            trust_decision: trust_decision_with_dispatch_authority(),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        brassclaw_capabilities::CapabilityInvocationError::Process { .. }
    ));
    assert!(matches!(
        fixture.governor.release(reservation_id).unwrap_err(),
        ResourceError::ReservationClosed {
            status: ReservationStatus::Released,
            ..
        }
    ));
}

fn assert_failed_outcome(outcome: RuntimeCapabilityOutcome, expected_kind: RuntimeFailureKind) {
    match outcome {
        RuntimeCapabilityOutcome::Failed(failure) => assert_eq!(failure.kind, expected_kind),
        other => panic!("expected failed outcome, got {other:?}"),
    }
}

type InMemoryHostRuntimeServices = HostRuntimeServices<
    LocalFilesystem,
    InMemoryResourceGovernor,
    InMemoryProcessStore,
    InMemoryProcessResultStore,
>;

struct InMemoryRecordingCombinedRunStateApprovalStore {
    runs: InMemoryRunStateStore,
    approvals: InMemoryApprovalRequestStore,
    combined_calls: AtomicUsize,
    separate_save_calls: AtomicUsize,
}

impl InMemoryRecordingCombinedRunStateApprovalStore {
    fn new() -> Self {
        Self {
            runs: InMemoryRunStateStore::new(),
            approvals: InMemoryApprovalRequestStore::new(),
            combined_calls: AtomicUsize::new(0),
            separate_save_calls: AtomicUsize::new(0),
        }
    }

    fn combined_calls(&self) -> usize {
        self.combined_calls.load(Ordering::SeqCst)
    }

    fn separate_save_calls(&self) -> usize {
        self.separate_save_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RunStateStore for InMemoryRecordingCombinedRunStateApprovalStore {
    async fn start(&self, start: RunStart) -> Result<RunRecord, RunStateError> {
        self.runs.start(start).await
    }

    async fn block_approval(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        approval: ApprovalRequest,
    ) -> Result<RunRecord, RunStateError> {
        self.runs
            .block_approval(scope, invocation_id, approval)
            .await
    }

    async fn block_auth(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        error_kind: String,
    ) -> Result<RunRecord, RunStateError> {
        self.runs.block_auth(scope, invocation_id, error_kind).await
    }

    async fn complete(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
    ) -> Result<RunRecord, RunStateError> {
        self.runs.complete(scope, invocation_id).await
    }

    async fn fail(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        error_kind: String,
    ) -> Result<RunRecord, RunStateError> {
        self.runs.fail(scope, invocation_id, error_kind).await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
    ) -> Result<Option<RunRecord>, RunStateError> {
        self.runs.get(scope, invocation_id).await
    }

    async fn records_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<RunRecord>, RunStateError> {
        self.runs.records_for_scope(scope).await
    }
}

#[async_trait]
impl ApprovalRequestStore for InMemoryRecordingCombinedRunStateApprovalStore {
    async fn save_pending(
        &self,
        scope: ResourceScope,
        request: ApprovalRequest,
    ) -> Result<ApprovalRecord, RunStateError> {
        self.separate_save_calls.fetch_add(1, Ordering::SeqCst);
        self.approvals.save_pending(scope, request).await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<Option<ApprovalRecord>, RunStateError> {
        self.approvals.get(scope, request_id).await
    }

    async fn approve(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<ApprovalRecord, RunStateError> {
        self.approvals.approve(scope, request_id).await
    }

    async fn deny(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<ApprovalRecord, RunStateError> {
        self.approvals.deny(scope, request_id).await
    }

    async fn discard_pending(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<ApprovalRecord, RunStateError> {
        self.approvals.discard_pending(scope, request_id).await
    }

    async fn records_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<ApprovalRecord>, RunStateError> {
        self.approvals.records_for_scope(scope).await
    }
}

#[async_trait]
impl RunStateApprovalStore for InMemoryRecordingCombinedRunStateApprovalStore {
    async fn save_pending_and_block_approval(
        &self,
        scope: ResourceScope,
        invocation_id: InvocationId,
        approval: ApprovalRequest,
    ) -> Result<RunRecord, RunStateError> {
        self.combined_calls.fetch_add(1, Ordering::SeqCst);
        self.approvals
            .save_pending(scope.clone(), approval.clone())
            .await?;
        self.runs
            .block_approval(&scope, invocation_id, approval)
            .await
    }
}

struct ApprovalResumeFixture {
    services: InMemoryHostRuntimeServices,
    run_state: Arc<InMemoryRunStateStore>,
    approval_requests: Arc<InMemoryApprovalRequestStore>,
    capability_leases: Arc<InMemoryCapabilityLeaseStore>,
    events: InMemoryEventSink,
}

fn approval_resume_fixture() -> ApprovalResumeFixture {
    approval_resume_fixture_with_manifest(SCRIPT_MANIFEST, vec![EffectKind::DispatchCapability])
}

fn approval_resume_fixture_with_manifest(
    manifest: &str,
    trust_effects: Vec<EffectKind>,
) -> ApprovalResumeFixture {
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let approval_requests = Arc::new(InMemoryApprovalRequestStore::new());
    let capability_leases = Arc::new(InMemoryCapabilityLeaseStore::new());
    let events = InMemoryEventSink::new();
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(manifest)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "script",
        trust_effects,
    )))
    .with_run_state(Arc::clone(&run_state))
    .with_approval_requests(Arc::clone(&approval_requests))
    .with_capability_leases(Arc::clone(&capability_leases))
    .with_event_sink(Arc::new(events.clone()));

    ApprovalResumeFixture {
        services,
        run_state,
        approval_requests,
        capability_leases,
        events,
    }
}

fn resume_runtime_with_empty_registry(fixture: &ApprovalResumeFixture) -> DefaultHostRuntime {
    HostRuntimeServices::new(
        Arc::new(ExtensionRegistry::new()),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "script",
        vec![EffectKind::DispatchCapability],
    )))
    .with_run_state(Arc::clone(&fixture.run_state))
    .with_approval_requests(Arc::clone(&fixture.approval_requests))
    .with_capability_leases(Arc::clone(&fixture.capability_leases))
    .host_runtime_for_local_testing()
}

fn resume_runtime_with_policy(
    fixture: &ApprovalResumeFixture,
    policy: EffectiveRuntimePolicy,
) -> DefaultHostRuntime {
    HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_NETWORK_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(ApprovalThenGrantAuthorizer),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_trust_policy(Arc::new(local_manifest_trust_policy(
        "script",
        vec![EffectKind::DispatchCapability, EffectKind::Network],
    )))
    .with_run_state(Arc::clone(&fixture.run_state))
    .with_approval_requests(Arc::clone(&fixture.approval_requests))
    .with_capability_leases(Arc::clone(&fixture.capability_leases))
    .with_event_sink(Arc::new(fixture.events.clone()))
    .with_runtime_policy(policy)
    .host_runtime_for_local_testing()
}

async fn assert_blocked_approval_run(
    fixture: &ApprovalResumeFixture,
    scope: &ResourceScope,
    invocation_id: InvocationId,
    approval_request_id: ApprovalRequestId,
) {
    let run = fixture
        .run_state
        .get(scope, invocation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.status, RunStatus::BlockedApproval);
    assert_eq!(run.approval_request_id, Some(approval_request_id));
    assert_eq!(run.error_kind, None);
}

async fn block_for_approval(
    runtime: &impl HostRuntime,
    context: ExecutionContext,
    estimate: ResourceEstimate,
    input: serde_json::Value,
) -> brassclaw_host_runtime::RuntimeApprovalGate {
    let outcome = runtime
        .invoke_capability(RuntimeCapabilityRequest::new(
            context,
            script_capability_id(),
            estimate,
            input,
            trust_decision_with_dispatch_authority(),
        ))
        .await
        .unwrap();

    match outcome {
        RuntimeCapabilityOutcome::ApprovalRequired(gate) => gate,
        other => panic!("expected approval gate, got {other:?}"),
    }
}

async fn approve_dispatch_for_services(
    services: &InMemoryHostRuntimeServices,
    scope: &ResourceScope,
    approval_request_id: ApprovalRequestId,
    expires_at: Option<Timestamp>,
) -> brassclaw_authorization::CapabilityLease {
    services
        .approval_resolver()
        .expect("approval resolver should be configured")
        .approve_dispatch(
            scope,
            approval_request_id,
            LeaseApproval {
                issued_by: Principal::HostRuntime,
                allowed_effects: vec![EffectKind::DispatchCapability],
                mounts: MountView::default(),
                network: NetworkPolicy::default(),
                secrets: Vec::new(),
                resource_ceiling: None,
                expires_at,
                max_invocations: Some(1),
            },
        )
        .await
        .unwrap()
}

async fn approve_spawn_for_services(
    services: &InMemoryHostRuntimeServices,
    scope: &ResourceScope,
    approval_request_id: ApprovalRequestId,
    expires_at: Option<Timestamp>,
) -> brassclaw_authorization::CapabilityLease {
    services
        .approval_resolver()
        .expect("approval resolver should be configured")
        .approve_spawn(
            scope,
            approval_request_id,
            LeaseApproval {
                issued_by: Principal::HostRuntime,
                allowed_effects: process_sandbox_authority_effects(),
                mounts: MountView::default(),
                network: NetworkPolicy::default(),
                secrets: Vec::new(),
                resource_ceiling: None,
                expires_at,
                max_invocations: Some(1),
            },
        )
        .await
        .unwrap()
}

struct SentinelApprovalAuthorizer;

#[async_trait]
impl TrustAwareCapabilityDispatchAuthorizer for SentinelApprovalAuthorizer {
    async fn authorize_dispatch_with_trust(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust_decision: &TrustDecision,
    ) -> Decision {
        if context.grants.grants.is_empty() {
            Decision::RequireApproval {
                request: ApprovalRequest {
                    id: ApprovalRequestId::new(),
                    correlation_id: context.correlation_id,
                    requested_by: Principal::Extension(context.extension_id.clone()),
                    action: Box::new(Action::Dispatch {
                        capability: descriptor.id.clone(),
                        estimated_resources: estimate.clone(),
                    }),
                    invocation_fingerprint: None,
                    reason: "APPROVAL_REASON_SENTINEL_3022 /tmp/private-approval-reason"
                        .to_string(),
                    reusable_scope: None,
                },
            }
        } else {
            GrantAuthorizer::new()
                .authorize_dispatch_with_trust(context, descriptor, estimate, trust_decision)
                .await
        }
    }
}

struct ApprovalThenGrantAuthorizer;

#[async_trait]
impl TrustAwareCapabilityDispatchAuthorizer for ApprovalThenGrantAuthorizer {
    async fn authorize_dispatch_with_trust(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust_decision: &TrustDecision,
    ) -> Decision {
        if context.grants.grants.is_empty() {
            Decision::RequireApproval {
                request: ApprovalRequest {
                    id: ApprovalRequestId::new(),
                    correlation_id: context.correlation_id,
                    requested_by: Principal::Extension(context.extension_id.clone()),
                    action: Box::new(Action::Dispatch {
                        capability: descriptor.id.clone(),
                        estimated_resources: estimate.clone(),
                    }),
                    invocation_fingerprint: None,
                    reason: "approval required".to_string(),
                    reusable_scope: None,
                },
            }
        } else {
            GrantAuthorizer::new()
                .authorize_dispatch_with_trust(context, descriptor, estimate, trust_decision)
                .await
        }
    }

    async fn authorize_spawn_with_trust(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        trust_decision: &TrustDecision,
    ) -> Decision {
        if context.grants.grants.is_empty() {
            Decision::RequireApproval {
                request: ApprovalRequest {
                    id: ApprovalRequestId::new(),
                    correlation_id: context.correlation_id,
                    requested_by: Principal::Extension(context.extension_id.clone()),
                    action: Box::new(Action::SpawnCapability {
                        capability: descriptor.id.clone(),
                        estimated_resources: estimate.clone(),
                    }),
                    invocation_fingerprint: None,
                    reason: "spawn approval required".to_string(),
                    reusable_scope: None,
                },
            }
        } else {
            GrantAuthorizer::new()
                .authorize_spawn_with_trust(context, descriptor, estimate, trust_decision)
                .await
        }
    }
}

struct ApprovalThenSecretObligationAuthorizer {
    handle: SecretHandle,
}

#[async_trait]
impl TrustAwareCapabilityDispatchAuthorizer for ApprovalThenSecretObligationAuthorizer {
    async fn authorize_dispatch_with_trust(
        &self,
        context: &ExecutionContext,
        descriptor: &CapabilityDescriptor,
        estimate: &ResourceEstimate,
        _trust_decision: &TrustDecision,
    ) -> Decision {
        if context.grants.grants.is_empty() {
            Decision::RequireApproval {
                request: ApprovalRequest {
                    id: ApprovalRequestId::new(),
                    correlation_id: context.correlation_id,
                    requested_by: Principal::Extension(context.extension_id.clone()),
                    action: Box::new(Action::Dispatch {
                        capability: descriptor.id.clone(),
                        estimated_resources: estimate.clone(),
                    }),
                    invocation_fingerprint: None,
                    reason: "approval required".to_string(),
                    reusable_scope: None,
                },
            }
        } else {
            Decision::Allow {
                obligations: Obligations::new(vec![Obligation::InjectSecretOnce {
                    handle: self.handle.clone(),
                }])
                .unwrap(),
            }
        }
    }
}




struct FailingDurableAuditLog;

#[async_trait]
impl DurableAuditLog for FailingDurableAuditLog {
    async fn append(
        &self,
        _record: AuditEnvelope,
    ) -> Result<brassclaw_events::EventLogEntry<AuditEnvelope>, EventError> {
        Err(EventError::DurableLog {
            reason: "simulated audit backend failure at /tmp/audit-backend-secret".to_string(),
        })
    }

    async fn read_after_cursor(
        &self,
        _stream: &EventStreamKey,
        _filter: &ReadScope,
        _after: Option<EventCursor>,
        _limit: usize,
    ) -> Result<EventReplay<AuditEnvelope>, EventError> {
        Err(EventError::DurableLog {
            reason: "simulated audit replay failure".to_string(),
        })
    }
}

struct ObligatingAuthorizer {
    obligations: Vec<Obligation>,
}

impl ObligatingAuthorizer {
    fn new(obligations: Vec<Obligation>) -> Self {
        Self { obligations }
    }
}

#[derive(Debug)]
struct ProductionCandidateProcessPort;

#[async_trait]
impl RuntimeProcessPort for ProductionCandidateProcessPort {
    async fn run_command(
        &self,
        _request: CommandExecutionRequest,
    ) -> Result<CommandExecutionOutput, RuntimeProcessError> {
        Ok(CommandExecutionOutput {
            output: String::new(),
            saved_output: None,
            exit_code: 0,
            sandboxed: true,
            duration: Duration::ZERO,
        })
    }
}

#[derive(Debug)]
struct ProductionCandidateSandboxTransport;

#[async_trait]
impl SandboxCommandTransport for ProductionCandidateSandboxTransport {
    async fn run_command(
        &self,
        _request: CommandExecutionRequest,
    ) -> Result<CommandExecutionOutput, RuntimeProcessError> {
        Ok(CommandExecutionOutput {
            output: String::new(),
            saved_output: None,
            exit_code: 0,
            sandboxed: false,
            duration: Duration::ZERO,
        })
    }
}

#[derive(Debug, Clone, Default)]
struct RecordingNetworkHttpEgress {
    requests: Arc<std::sync::Mutex<Vec<NetworkHttpRequest>>>,
}

impl RecordingNetworkHttpEgress {
    fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl NetworkHttpEgress for RecordingNetworkHttpEgress {
    async fn execute(
        &self,
        request: NetworkHttpRequest,
    ) -> Result<NetworkHttpResponse, NetworkHttpError> {
        let request_bytes = request.body.len() as u64;
        self.requests.lock().unwrap().push(request);
        Ok(NetworkHttpResponse {
            status: 200,
            headers: Vec::new(),
            body: Vec::new(),
            usage: NetworkUsage {
                request_bytes,
                response_bytes: 0,
                resolved_ip: None,
            },
        })
    }
}

#[derive(Debug, Clone)]
struct RecordingRuntimeHttpEgress {
    requests: Arc<std::sync::Mutex<Vec<RuntimeHttpEgressRequest>>>,
    delay: Duration,
    response_status: u16,
}

impl Default for RecordingRuntimeHttpEgress {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordingRuntimeHttpEgress {
    fn new() -> Self {
        Self {
            requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            delay: Duration::ZERO,
            response_status: 200,
        }
    }
}

#[async_trait::async_trait]
impl RuntimeHttpEgress for RecordingRuntimeHttpEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        if !self.delay.is_zero() {
            thread::sleep(self.delay);
        }
        self.requests.lock().unwrap().push(request.clone());
        Ok(RuntimeHttpEgressResponse {
            status: self.response_status,
            headers: Vec::new(),
            body: Vec::new(),
            saved_body: None,
            request_bytes: request.body.len() as u64,
            response_bytes: 0,
            redaction_applied: false,
        })
    }
}

async fn stage_process_handoffs(
    services: &BuiltinObligationServices,
    scope: &ResourceScope,
    capability_id: &CapabilityId,
    secret_handle: &SecretHandle,
    policy: NetworkPolicy,
    material: &str,
) {
    services
        .secret_store()
        .put(
            scope.clone(),
            secret_handle.clone(),
            SecretMaterial::from(material),
        )
        .await
        .unwrap();
    let context =
        execution_context_with_dispatch_grant_for_scope(capability_id.clone(), scope.clone());
    services
        .obligation_handler()
        .satisfy(CapabilityObligationRequest {
            phase: CapabilityObligationPhase::Invoke,
            context: &context,
            capability_id,
            estimate: &ResourceEstimate::default(),
            obligations: &[
                Obligation::ApplyNetworkPolicy { policy },
                Obligation::InjectSecretOnce {
                    handle: secret_handle.clone(),
                },
            ],
        })
        .await
        .unwrap();
}

struct SpawnObligationFixture {
    registry: Arc<ExtensionRegistry>,
    dispatcher: Arc<NoopDispatcher>,
    authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer>,
    handler: Arc<BuiltinObligationHandler>,
    process_manager: Arc<BackgroundProcessManager>,
    process_store: Arc<ProcessObligationLifecycleStore>,
    governor: Arc<InMemoryResourceGovernor>,
    context: ExecutionContext,
    scope: ResourceScope,
    estimate: ResourceEstimate,
}

impl SpawnObligationFixture {
    async fn spawn(&self) -> brassclaw_processes::ProcessRecord {
        let host = CapabilityHost::new(
            self.registry.as_ref(),
            self.dispatcher.as_ref(),
            self.authorizer.as_ref(),
        )
        .with_obligation_handler(self.handler.as_ref())
        .with_process_manager(self.process_manager.as_ref());

        host.spawn_json(CapabilitySpawnRequest {
            context: self.context.clone(),
            capability_id: script_capability_id(),
            estimate: self.estimate.clone(),
            input: json!({"message": "background"}),
            trust_decision: trust_decision_with_dispatch_authority(),
        })
        .await
        .unwrap()
        .process
    }
}

async fn spawn_obligation_fixture(
    reservation_id: ResourceReservationId,
    secret_handle: SecretHandle,
    executor: BackgroundExecutor,
) -> SpawnObligationFixture {
    spawn_obligation_fixture_with_result_store(
        reservation_id,
        secret_handle,
        executor,
        Arc::new(InMemoryProcessResultStore::new()),
    )
    .await
}

async fn spawn_obligation_fixture_with_result_store<R>(
    reservation_id: ResourceReservationId,
    secret_handle: SecretHandle,
    executor: BackgroundExecutor,
    result_store: Arc<R>,
) -> SpawnObligationFixture
where
    R: ProcessResultStore + 'static,
{
    spawn_obligation_fixture_with_process_store_and_result_store(
        reservation_id,
        secret_handle,
        executor,
        Arc::new(InMemoryProcessStore::new()),
        result_store,
    )
    .await
}

async fn spawn_obligation_fixture_with_process_store_and_result_store<P, R>(
    reservation_id: ResourceReservationId,
    secret_handle: SecretHandle,
    executor: BackgroundExecutor,
    inner_process_store: Arc<P>,
    result_store: Arc<R>,
) -> SpawnObligationFixture
where
    P: ProcessStore + 'static,
    R: ProcessResultStore + 'static,
{
    let registry = Arc::new(registry_with_manifest(SCRIPT_MANIFEST));
    let dispatcher = Arc::new(NoopDispatcher);
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let secret_store = Arc::new(InMemorySecretStore::new());
    let obligation_services = BuiltinObligationServices::new(
        Arc::new(InMemoryAuditSink::new()),
        secret_store.clone(),
        governor.clone(),
    );
    let invocation_id = InvocationId::new();
    let scope = sample_scope(invocation_id);
    let context =
        execution_context_with_dispatch_grant_for_scope(script_capability_id(), scope.clone());
    let estimate = ResourceEstimate {
        process_count: Some(1),
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    };
    secret_store
        .put(
            scope.clone(),
            secret_handle.clone(),
            SecretMaterial::from("runtime-secret"),
        )
        .await
        .unwrap();
    let handler = Arc::new(obligation_services.obligation_handler());
    let authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer> =
        Arc::new(ObligatingAuthorizer::new(vec![
            Obligation::ReserveResources { reservation_id },
            Obligation::ApplyNetworkPolicy {
                policy: sample_network_policy(),
            },
            Obligation::InjectSecretOnce {
                handle: secret_handle,
            },
        ]));
    let process_store =
        Arc::new(obligation_services.process_obligation_lifecycle_store(inner_process_store));
    let cleanup_process_store = Arc::clone(&process_store);
    let process_manager = Arc::new(
        BackgroundProcessManager::new(Arc::clone(&process_store), Arc::new(executor))
            .with_result_store(result_store)
            .with_error_handler(move |failure| {
                let reconcile = match failure.stage {
                    BackgroundFailureStage::StoreComplete => true,
                    BackgroundFailureStage::StoreFail => false,
                    BackgroundFailureStage::ResultStoreComplete => true,
                    BackgroundFailureStage::ResultStoreFail => false,
                    _ => return,
                };
                let cleanup_process_store = Arc::clone(&cleanup_process_store);
                tokio::spawn(async move {
                    let _ = cleanup_process_store
                        .cleanup_process_obligations(&failure.scope, failure.process_id, reconcile)
                        .await;
                });
            }),
    );

    SpawnObligationFixture {
        registry,
        dispatcher,
        authorizer,
        handler,
        process_manager,
        process_store,
        governor,
        context,
        scope,
        estimate,
    }
}

#[derive(Default)]
struct FailingProcessResultStore {
    attempts: std::sync::Mutex<Vec<&'static str>>,
}

#[derive(Debug)]
struct FailingCleanupResourceGovernor;

impl ResourceGovernor for FailingCleanupResourceGovernor {
    fn set_limit(
        &self,
        _account: ResourceAccount,
        _limits: ResourceLimits,
    ) -> Result<(), ResourceError> {
        Ok(())
    }

    fn reserve_with_outcome(
        &self,
        scope: ResourceScope,
        estimate: ResourceEstimate,
    ) -> Result<brassclaw_resources::ReservationOutcome, ResourceError> {
        Ok(brassclaw_resources::ReservationOutcome {
            reservation: ResourceReservation {
                id: ResourceReservationId::new(),
                scope,
                estimate,
            },
            warnings: Vec::new(),
        })
    }

    fn reserve_with_id_and_outcome(
        &self,
        scope: ResourceScope,
        estimate: ResourceEstimate,
        reservation_id: ResourceReservationId,
    ) -> Result<brassclaw_resources::ReservationOutcome, ResourceError> {
        Ok(brassclaw_resources::ReservationOutcome {
            reservation: ResourceReservation {
                id: reservation_id,
                scope,
                estimate,
            },
            warnings: Vec::new(),
        })
    }

    fn reconcile(
        &self,
        reservation_id: ResourceReservationId,
        _actual: ResourceUsage,
    ) -> Result<ResourceReceipt, ResourceError> {
        Err(ResourceError::ReservationMismatch { id: reservation_id })
    }

    fn release(
        &self,
        reservation_id: ResourceReservationId,
    ) -> Result<ResourceReceipt, ResourceError> {
        Err(ResourceError::ReservationMismatch { id: reservation_id })
    }

    fn account_snapshot(
        &self,
        _account: &ResourceAccount,
    ) -> Result<Option<brassclaw_resources::AccountSnapshot>, ResourceError> {
        Ok(None)
    }
}

impl FailingProcessResultStore {
    fn attempts(&self) -> Vec<&'static str> {
        self.attempts.lock().unwrap().clone()
    }
}

#[async_trait]
impl ProcessResultStore for FailingProcessResultStore {
    async fn complete(
        &self,
        _scope: &ResourceScope,
        _process_id: ProcessId,
        _output: serde_json::Value,
    ) -> Result<ProcessResultRecord, ProcessError> {
        self.attempts.lock().unwrap().push("complete");
        Err(ProcessError::InvalidStoredRecord {
            reason: "result complete failed".to_string(),
        })
    }

    async fn fail(
        &self,
        _scope: &ResourceScope,
        _process_id: ProcessId,
        _error_kind: String,
    ) -> Result<ProcessResultRecord, ProcessError> {
        self.attempts.lock().unwrap().push("fail");
        Err(ProcessError::InvalidStoredRecord {
            reason: "result fail failed".to_string(),
        })
    }

    async fn kill(
        &self,
        _scope: &ResourceScope,
        _process_id: ProcessId,
    ) -> Result<ProcessResultRecord, ProcessError> {
        self.attempts.lock().unwrap().push("kill");
        Err(ProcessError::InvalidStoredRecord {
            reason: "result kill failed".to_string(),
        })
    }

    async fn get(
        &self,
        _scope: &ResourceScope,
        _process_id: ProcessId,
    ) -> Result<Option<ProcessResultRecord>, ProcessError> {
        Ok(None)
    }
}

struct FailingTerminalProcessStore {
    inner: InMemoryProcessStore,
    fail_complete: bool,
    fail_fail: bool,
    attempts: std::sync::Mutex<Vec<&'static str>>,
}

impl FailingTerminalProcessStore {
    fn fail_complete() -> Self {
        Self {
            inner: InMemoryProcessStore::new(),
            fail_complete: true,
            fail_fail: false,
            attempts: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn fail_fail() -> Self {
        Self {
            inner: InMemoryProcessStore::new(),
            fail_complete: false,
            fail_fail: true,
            attempts: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn attempts(&self) -> Vec<&'static str> {
        self.attempts.lock().unwrap().clone()
    }
}

#[async_trait]
impl ProcessStore for FailingTerminalProcessStore {
    async fn start(
        &self,
        start: ProcessStart,
    ) -> Result<brassclaw_processes::ProcessRecord, ProcessError> {
        self.inner.start(start).await
    }

    async fn complete(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<brassclaw_processes::ProcessRecord, ProcessError> {
        self.attempts.lock().unwrap().push("complete");
        if self.fail_complete {
            return Err(ProcessError::InvalidStoredRecord {
                reason: "status complete failed".to_string(),
            });
        }
        self.inner.complete(scope, process_id).await
    }

    async fn fail(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
        error_kind: String,
    ) -> Result<brassclaw_processes::ProcessRecord, ProcessError> {
        self.attempts.lock().unwrap().push("fail");
        if self.fail_fail {
            return Err(ProcessError::InvalidStoredRecord {
                reason: "status fail failed".to_string(),
            });
        }
        self.inner.fail(scope, process_id, error_kind).await
    }

    async fn kill(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<brassclaw_processes::ProcessRecord, ProcessError> {
        self.inner.kill(scope, process_id).await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<Option<brassclaw_processes::ProcessRecord>, ProcessError> {
        self.inner.get(scope, process_id).await
    }

    async fn records_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<brassclaw_processes::ProcessRecord>, ProcessError> {
        self.inner.records_for_scope(scope).await
    }
}

struct BackgroundExecutor {
    outcome: BackgroundExecutorOutcome,
}

impl BackgroundExecutor {
    fn success() -> Self {
        Self {
            outcome: BackgroundExecutorOutcome::Success(json!({"ok": true})),
        }
    }

    fn success_with_output(output: serde_json::Value) -> Self {
        Self {
            outcome: BackgroundExecutorOutcome::Success(output),
        }
    }

    fn failure(kind: impl Into<String>) -> Self {
        Self {
            outcome: BackgroundExecutorOutcome::Failure(kind.into()),
        }
    }

    fn delayed_success(delay: Duration) -> Self {
        Self {
            outcome: BackgroundExecutorOutcome::DelayedSuccess(delay),
        }
    }
}

enum BackgroundExecutorOutcome {
    Success(serde_json::Value),
    Failure(String),
    DelayedSuccess(Duration),
}

#[async_trait]
impl ProcessExecutor for BackgroundExecutor {
    async fn execute(
        &self,
        _request: ProcessExecutionRequest,
    ) -> Result<ProcessExecutionResult, brassclaw_processes::ProcessExecutionError> {
        match &self.outcome {
            BackgroundExecutorOutcome::Success(output) => Ok(ProcessExecutionResult {
                output: output.clone(),
            }),
            BackgroundExecutorOutcome::Failure(kind) => Err(
                brassclaw_processes::ProcessExecutionError::new(kind.clone()),
            ),
            BackgroundExecutorOutcome::DelayedSuccess(delay) => {
                tokio::time::sleep(*delay).await;
                Ok(ProcessExecutionResult {
                    output: json!({"ok": true}),
                })
            }
        }
    }
}

#[derive(Default)]
struct RecordingSandboxProcessExecutor {
    requests: std::sync::Mutex<Vec<ProcessExecutionRequest>>,
}

impl RecordingSandboxProcessExecutor {
    fn requests(&self) -> Vec<ProcessExecutionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl ProcessExecutor for RecordingSandboxProcessExecutor {
    async fn execute(
        &self,
        request: ProcessExecutionRequest,
    ) -> Result<ProcessExecutionResult, brassclaw_processes::ProcessExecutionError> {
        self.requests.lock().unwrap().push(request);
        Ok(ProcessExecutionResult {
            output: json!({"executor": "process_sandbox"}),
        })
    }
}

struct FailingSpawnManager;

#[async_trait]
impl brassclaw_processes::ProcessManager for FailingSpawnManager {
    async fn spawn(
        &self,
        _start: ProcessStart,
    ) -> Result<brassclaw_processes::ProcessRecord, ProcessError> {
        Err(ProcessError::InvalidStoredRecord {
            reason: "start failed".to_string(),
        })
    }
}

struct NoopDispatcher;

#[async_trait]
impl CapabilityDispatcher for NoopDispatcher {
    async fn dispatch_json(
        &self,
        _request: CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchResult, DispatchError> {
        panic!("spawn tests must not invoke the foreground dispatcher")
    }
}

async fn wait_for_status(
    store: &dyn ProcessStore,
    scope: &ResourceScope,
    process_id: ProcessId,
    status: ProcessStatus,
) {
    for _ in 0..100 {
        if let Some(record) = store.get(scope, process_id).await.unwrap()
            && record.status == status
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("process {process_id} did not reach {status:?}");
}

async fn wait_for_sandbox_process_result(
    executor: &RecordingSandboxProcessExecutor,
    scope: &ResourceScope,
    process_id: ProcessId,
    result_store: &dyn ProcessResultStore,
) {
    for _ in 0..100 {
        let requests = executor.requests();
        if let Some(request) = requests.first()
            && request.process_id == process_id
            && request.capability_id == process_sandbox_capability_id()
            && request.runtime == RuntimeKind::System
            && let Some(result) = result_store.get(scope, process_id).await.unwrap()
        {
            assert_eq!(result.status, ProcessStatus::Completed);
            assert_eq!(result.output, Some(json!({"executor": "process_sandbox"})));
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("process sandbox executor did not complete process {process_id}");
}

async fn wait_for_result_store_attempt(store: &FailingProcessResultStore, attempt: &'static str) {
    for _ in 0..100 {
        if store.attempts().contains(&attempt) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("result store did not record {attempt} attempt");
}

async fn wait_for_process_store_attempt(
    store: &FailingTerminalProcessStore,
    attempt: &'static str,
) {
    for _ in 0..100 {
        if store.attempts().contains(&attempt) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("process store did not record {attempt} attempt");
}

async fn wait_for_no_reserved_processes(governor: &InMemoryResourceGovernor) {
    for _ in 0..100 {
        if governor.reserved_for(&sample_account()).process_count == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("process reservation was not cleaned up");
}

#[async_trait]
impl TrustAwareCapabilityDispatchAuthorizer for ObligatingAuthorizer {
    async fn authorize_dispatch_with_trust(
        &self,
        _context: &ExecutionContext,
        _descriptor: &CapabilityDescriptor,
        _estimate: &ResourceEstimate,
        _trust_decision: &TrustDecision,
    ) -> Decision {
        Decision::Allow {
            obligations: Obligations::new(self.obligations.clone()).unwrap(),
        }
    }

    async fn authorize_spawn_with_trust(
        &self,
        _context: &ExecutionContext,
        _descriptor: &CapabilityDescriptor,
        _estimate: &ResourceEstimate,
        _trust_decision: &TrustDecision,
    ) -> Decision {
        Decision::Allow {
            obligations: Obligations::new(self.obligations.clone()).unwrap(),
        }
    }
}

struct ClientErrorMcpExecutor;

#[async_trait]
impl McpExecutor for ClientErrorMcpExecutor {
    async fn execute_extension_json(
        &self,
        _governor: &dyn ResourceGovernor,
        _request: McpExecutionRequest<'_>,
    ) -> Result<McpExecutionResult, McpError> {
        Err(McpError::Client {
            reason: "simulated MCP client failure".to_string(),
        })
    }
}

struct PanicMcpExecutor;

#[async_trait]
impl McpExecutor for PanicMcpExecutor {
    async fn execute_extension_json(
        &self,
        _governor: &dyn ResourceGovernor,
        _request: McpExecutionRequest<'_>,
    ) -> Result<McpExecutionResult, McpError> {
        panic!("health-only test must not execute MCP runtime")
    }
}

fn registry_with_manifest(manifest: &str) -> ExtensionRegistry {
    registry_with_manifests(&[manifest])
}

fn registry_with_host_bundled_manifest(manifest: &str) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    let manifest = parse_manifest_from_source(manifest, ManifestSource::HostBundled);
    let root = VirtualPath::new(format!("/system/extensions/{}", manifest.id.as_str())).unwrap();
    let package = ExtensionPackage::from_manifest(manifest, root).unwrap();
    registry.insert(package).unwrap();
    registry
}

fn registry_with_builtin_first_party_package() -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry
        .insert(builtin_first_party_package().unwrap())
        .unwrap();
    registry
}

fn registry_with_manifests(manifests: &[&str]) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    for manifest in manifests {
        let manifest = parse_manifest(manifest);
        let root =
            VirtualPath::new(format!("/system/extensions/{}", manifest.id.as_str())).unwrap();
        let package = ExtensionPackage::from_manifest(manifest, root).unwrap();
        registry.insert(package).unwrap();
    }
    registry
}

fn parse_manifest(manifest: &str) -> ExtensionManifest {
    parse_manifest_from_source(manifest, ManifestSource::InstalledLocal)
}

fn parse_manifest_from_source(manifest: &str, source: ManifestSource) -> ExtensionManifest {
    let manifest = legacy_capability_fixture_to_v2(manifest);
    ExtensionManifest::parse(&manifest, source, &HostPortCatalog::empty()).unwrap()
}

fn execution_context_without_grants() -> ExecutionContext {
    ExecutionContext::local_default(
        UserId::new("user").unwrap(),
        ExtensionId::new("caller").unwrap(),
        RuntimeKind::Script,
        TrustClass::UserTrusted,
        CapabilitySet::default(),
        MountView::default(),
    )
    .unwrap()
}

fn execution_context_without_grants_for_scope(scope: ResourceScope) -> ExecutionContext {
    let context = ExecutionContext {
        invocation_id: scope.invocation_id,
        correlation_id: CorrelationId::new(),
        process_id: None,
        parent_process_id: None,
        tenant_id: scope.tenant_id.clone(),
        user_id: scope.user_id.clone(),
        agent_id: scope.agent_id.clone(),
        project_id: scope.project_id.clone(),
        mission_id: scope.mission_id.clone(),
        thread_id: scope.thread_id.clone(),
        extension_id: ExtensionId::new("caller").unwrap(),
        runtime: RuntimeKind::Script,
        trust: TrustClass::UserTrusted,
        grants: CapabilitySet::default(),
        mounts: MountView::default(),
        resource_scope: scope,
    };
    context.validate().unwrap();
    context
}

fn execution_context_with_dispatch_grant(capability: CapabilityId) -> ExecutionContext {
    let grants = capability_grants(capability);
    ExecutionContext::local_default(
        UserId::new("user").unwrap(),
        ExtensionId::new("caller").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::UserTrusted,
        grants,
        MountView::default(),
    )
    .unwrap()
}

fn execution_context_with_dispatch_grant_for_scope(
    capability: CapabilityId,
    scope: ResourceScope,
) -> ExecutionContext {
    execution_context_with_effect_grants_for_scope(
        capability,
        scope,
        vec![EffectKind::DispatchCapability, EffectKind::Network],
    )
}

fn execution_context_with_effect_grants_for_scope(
    capability: CapabilityId,
    scope: ResourceScope,
    allowed_effects: Vec<EffectKind>,
) -> ExecutionContext {
    let context = ExecutionContext {
        invocation_id: scope.invocation_id,
        correlation_id: CorrelationId::new(),
        process_id: None,
        parent_process_id: None,
        tenant_id: scope.tenant_id.clone(),
        user_id: scope.user_id.clone(),
        agent_id: scope.agent_id.clone(),
        project_id: scope.project_id.clone(),
        mission_id: scope.mission_id.clone(),
        thread_id: scope.thread_id.clone(),
        extension_id: ExtensionId::new("caller").unwrap(),
        runtime: RuntimeKind::FirstParty,
        trust: TrustClass::UserTrusted,
        grants: capability_grants_with_effects(capability, allowed_effects),
        mounts: MountView::default(),
        resource_scope: scope,
    };
    context.validate().unwrap();
    context
}

fn capability_grants(capability: CapabilityId) -> CapabilitySet {
    capability_grants_with_effects(
        capability,
        vec![EffectKind::DispatchCapability, EffectKind::Network],
    )
}

fn capability_grants_with_effects(
    capability: CapabilityId,
    allowed_effects: Vec<EffectKind>,
) -> CapabilitySet {
    let mut grants = CapabilitySet::default();
    grants.grants.push(CapabilityGrant {
        id: CapabilityGrantId::new(),
        capability,
        grantee: Principal::Extension(ExtensionId::new("caller").unwrap()),
        issued_by: Principal::HostRuntime,
        constraints: GrantConstraints {
            allowed_effects,
            mounts: MountView::default(),
            network: NetworkPolicy::default(),
            secrets: Vec::new(),
            resource_ceiling: None,
            expires_at: None,
            max_invocations: None,
        },
    });
    grants
}

fn mount_view(alias: &str, target: &str, permissions: MountPermissions) -> MountView {
    MountView::new(vec![MountGrant::new(
        MountAlias::new(alias).unwrap(),
        VirtualPath::new(target).unwrap(),
        permissions,
    )])
    .unwrap()
}

fn local_manifest_trust_policy(
    extension_id: &str,
    allowed_effects: Vec<EffectKind>,
) -> HostTrustPolicy {
    HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new(extension_id).unwrap(),
            format!("/system/extensions/{extension_id}/manifest.toml"),
            None,
            HostTrustAssignment::user_trusted(),
            allowed_effects,
            None,
        ),
    ]))])
    .unwrap()
}

fn trust_decision_with_dispatch_authority() -> TrustDecision {
    trust_decision_with_authority(vec![EffectKind::DispatchCapability, EffectKind::Network])
}

fn trust_decision_with_authority(allowed_effects: Vec<EffectKind>) -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::user_trusted(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects,
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::Default,
        evaluated_at: Utc::now(),
    }
}

fn network_denied_runtime_policy() -> EffectiveRuntimePolicy {
    EffectiveRuntimePolicy {
        deployment: DeploymentMode::LocalSingleUser,
        requested_profile: RuntimeProfile::SecureDefault,
        resolved_profile: RuntimeProfile::SecureDefault,
        filesystem_backend: FilesystemBackendKind::ScopedVirtual,
        process_backend: ProcessBackendKind::None,
        network_mode: NetworkMode::Deny,
        secret_mode: SecretMode::BrokeredHandles,
        approval_policy: ApprovalPolicy::AskAlways,
        audit_mode: AuditMode::LocalMinimal,
    }
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

fn hosted_dev_runtime_policy() -> EffectiveRuntimePolicy {
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

fn assert_local_only_runtime_policy_rejected(
    runtime_policy: EffectiveRuntimePolicy,
    expected_implementation: &'static str,
) {
    let services = HostRuntimeServices::new(
        Arc::new(registry_with_manifest(SCRIPT_MANIFEST)),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("surface-v1").unwrap(),
    )
    .with_runtime_policy(runtime_policy);

    let report = services
        .validate_production_wiring(&ProductionWiringConfig::new([]))
        .expect_err("local-only runtime-policy field must not pass production validation");

    assert!(
        report.issues().iter().any(|issue| {
            issue.component() == ProductionWiringComponent::RuntimePolicy
                && issue.kind() == ProductionWiringIssueKind::LocalOnlyImplementation
                && issue.implementation() == Some(expected_implementation)
        }),
        "runtime policy should report {expected_implementation}: {report:?}"
    );
}

fn read_directory_text(root: &std::path::Path) -> String {
    let mut output = String::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = std::fs::read_dir(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                output.push_str(&std::fs::read_to_string(&path).unwrap_or_else(|err| {
                    panic!("failed to read {} as utf-8 text: {err}", path.display())
                }));
            }
        }
    }
    output
}

fn sample_scope(invocation_id: InvocationId) -> ResourceScope {
    ResourceScope {
        tenant_id: TenantId::new("tenant-a").unwrap(),
        user_id: UserId::new("user-a").unwrap(),
        agent_id: Some(AgentId::new("agent-a").unwrap()),
        project_id: Some(ProjectId::new("project-a").unwrap()),
        mission_id: Some(MissionId::new("mission-a").unwrap()),
        thread_id: Some(ThreadId::new("thread-a").unwrap()),
        invocation_id,
    }
}

fn process_start(
    process_id: ProcessId,
    invocation_id: InvocationId,
    scope: ResourceScope,
) -> ProcessStart {
    ProcessStart {
        process_id,
        parent_process_id: None,
        invocation_id,
        scope,
        extension_id: script_extension_id(),
        capability_id: script_capability_id(),
        runtime: RuntimeKind::Script,
        grants: CapabilitySet::default(),
        mounts: MountView::default(),
        estimated_resources: ResourceEstimate::default(),
        resource_reservation_id: None,
        input: json!({"message": "running"}),
    }
}

fn process_sandbox_start(process_id: ProcessId, scope: ResourceScope) -> ProcessStart {
    let invocation_id = scope.invocation_id;
    ProcessStart {
        process_id,
        parent_process_id: None,
        invocation_id,
        scope,
        extension_id: ExtensionId::new("system.process_sandbox").unwrap(),
        capability_id: process_sandbox_capability_id(),
        runtime: RuntimeKind::System,
        grants: CapabilitySet::default(),
        mounts: MountView::default(),
        estimated_resources: ResourceEstimate::default(),
        resource_reservation_id: None,
        input: process_sandbox_input(),
    }
}

fn process_sandbox_runtime_request_for_scope(scope: ResourceScope) -> RuntimeCapabilityRequest {
    RuntimeCapabilityRequest::new(
        execution_context_with_effect_grants_for_scope(
            process_sandbox_capability_id(),
            scope,
            process_sandbox_authority_effects(),
        ),
        process_sandbox_capability_id(),
        process_sandbox_estimate(),
        process_sandbox_input(),
        process_sandbox_trust_decision(),
    )
}

fn process_sandbox_estimate() -> ResourceEstimate {
    ResourceEstimate {
        process_count: Some(1),
        concurrency_slots: Some(1),
        ..ResourceEstimate::default()
    }
}

fn process_sandbox_input() -> serde_json::Value {
    json!({"run": {"command": "echo", "args": ["ok"]}})
}

fn invalid_process_sandbox_input() -> serde_json::Value {
    json!({"run": {"command": ""}})
}

fn process_sandbox_authority_effects() -> Vec<EffectKind> {
    vec![EffectKind::ExecuteCode, EffectKind::SpawnProcess]
}

fn process_sandbox_trust_decision() -> TrustDecision {
    trust_decision_with_authority(process_sandbox_authority_effects())
}

fn script_extension_id() -> ExtensionId {
    ExtensionId::new("script").unwrap()
}

fn script_capability_id() -> CapabilityId {
    CapabilityId::new("script.echo").unwrap()
}

fn mcp_capability_id() -> CapabilityId {
    CapabilityId::new("mcp.search").unwrap()
}

fn process_sandbox_capability_id() -> CapabilityId {
    CapabilityId::new("system.process_sandbox.run").unwrap()
}

fn governor_with_default_limit(account: ResourceAccount) -> InMemoryResourceGovernor {
    let governor = InMemoryResourceGovernor::new();
    governor
        .set_limit(
            account,
            ResourceLimits {
                max_concurrency_slots: Some(10),
                max_network_egress_bytes: Some(10_000),
                max_output_bytes: Some(100_000),
                ..ResourceLimits::default()
            },
        )
        .unwrap();
    governor
}

fn sample_account() -> ResourceAccount {
    ResourceAccount::tenant(TenantId::new("tenant-a").unwrap())
}

fn sample_network_policy() -> NetworkPolicy {
    NetworkPolicy {
        allowed_targets: vec![NetworkTargetPattern {
            scheme: Some(NetworkScheme::Https),
            host_pattern: "example.test".to_string(),
            port: None,
        }],
        deny_private_ip_ranges: true,
        max_egress_bytes: Some(10_000),
    }
}

#[cfg(feature = "libsql")]
fn submit_turn_request(thread: &str, idempotency_key: &str) -> SubmitTurnRequest {
    SubmitTurnRequest {
        scope: TurnScope::new(
            TenantId::new("tenant1").unwrap(),
            Some(AgentId::new("agent1").unwrap()),
            Some(ProjectId::new("project1").unwrap()),
            ThreadId::new(thread).unwrap(),
        ),
        actor: TurnActor::new(UserId::new("user1").unwrap()),
        accepted_message_ref: AcceptedMessageRef::new(format!("message-{thread}")).unwrap(),
        source_binding_ref: SourceBindingRef::new("source-web").unwrap(),
        reply_target_binding_ref: ReplyTargetBindingRef::new("reply-web").unwrap(),
        requested_run_profile: Some(RunProfileRequest::new("default").unwrap()),
        idempotency_key: IdempotencyKey::new(idempotency_key).unwrap(),
        received_at: Utc::now(),
        requested_run_id: None,
        parent_run_id: None,
        subagent_depth: 0,
        spawn_tree_root_run_id: None,
    }
}

const SCRIPT_MANIFEST: &str = r#"
id = "script"
name = "Script Echo"
version = "0.1.0"
description = "Script integration extension"
trust = "untrusted"

[runtime]
kind = "script"
runner = "sandboxed_process"
command = "echo"
args = []

[[capabilities]]
id = "script.echo"
description = "Echo through Script"
effects = ["dispatch_capability"]
default_permission = "allow"
parameters_schema = { type = "object" }
"#;

const PROCESS_SANDBOX_MANIFEST: &str = r#"
id = "system.process_sandbox"
name = "Process Sandbox"
version = "0.1.0"
description = "System process sandbox runtime"
trust = "system_requested"

[runtime]
kind = "system"
service = "process_sandbox"

[[capabilities]]
id = "system.process_sandbox.run"
description = "Run a process inside the system sandbox backend"
effects = ["execute_code", "spawn_process"]
default_permission = "ask"
parameters_schema = { type = "object" }
"#;

const SCRIPT_NETWORK_MANIFEST: &str = r#"
id = "script"
name = "Script Echo"
version = "0.1.0"
description = "Script integration extension"
trust = "untrusted"

[runtime]
kind = "script"
runner = "sandboxed_process"
command = "echo"
args = []

[[capabilities]]
id = "script.echo"
description = "Echo through Script"
effects = ["dispatch_capability", "network"]
default_permission = "allow"
parameters_schema = { type = "object" }
"#;

const MCP_MANIFEST: &str = r#"
id = "mcp"
name = "MCP Search"
version = "0.1.0"
description = "MCP integration extension"
trust = "third_party"

[runtime]
kind = "mcp"
transport = "http"
url = "https://mcp.example.test/rpc"

[[capabilities]]
id = "mcp.search"
description = "Search through MCP"
effects = ["dispatch_capability", "network"]
default_permission = "ask"
parameters_schema = { type = "object" }
"#;
