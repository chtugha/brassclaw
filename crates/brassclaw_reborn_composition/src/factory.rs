// arch-exempt: large_file, needs Reborn composition helper extraction, plan #4469
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(feature = "postgres")]
use crate::pg_auth_product_services::PgAuthProductServices;
use crate::product_auth_durable::{FilesystemAuthProductServices, UnavailableAuthProviderClient};
use brassclaw_auth::AuthProviderClient;
#[cfg(feature = "postgres")]
use brassclaw_authorization::FilesystemCapabilityLeaseStore;
use brassclaw_authorization::GrantAuthorizer;
use brassclaw_authorization::InMemoryCapabilityLeaseStore;
#[cfg(feature = "postgres")]
use brassclaw_authorization::PgCapabilityLeaseStore;
use brassclaw_conversations::{
    AdapterInstallationId, AdapterKind, ConversationActorPairingService, ExternalActorRef,
};
use brassclaw_conversations::{InboundTurnError, RebornFilesystemConversationServices};
use brassclaw_events::{
    DurableAuditLog, DurableEventLog, InMemoryDurableAuditLog, InMemoryDurableEventLog,
};
use brassclaw_extensions::{
    ExtensionInstallationStore, ExtensionLifecycleService, ExtensionRegistry,
};
use brassclaw_filesystem::InMemoryBackend;
use brassclaw_filesystem::{
    BackendCapabilities, BackendId, BackendKind, CompositeRootFilesystem, ContentKind, IndexPolicy,
    MountDescriptor, RootFilesystem, StorageClass,
};
use brassclaw_filesystem::{LocalFilesystem, ScopedFilesystem};
#[cfg(feature = "postgres")]
use brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy;
use brassclaw_host_api::runtime_policy::{FilesystemBackendKind, ProcessBackendKind, SecretMode};
use brassclaw_host_api::{
    EffectKind, ExtensionId, HostPath, MountPermissions, MountView, PackageId, RuntimeHttpEgress,
    UserId, VirtualPath,
};
#[cfg(feature = "postgres")]
use brassclaw_host_runtime::{
    BuiltinFirstPartyTools, builtin_first_party_handlers_from_tools_with_trigger,
};
use brassclaw_host_runtime::{
    CapabilitySurfaceVersion, FirstPartyCapabilityRegistry, HostRuntimeHttpEgressPort,
    HostRuntimeServices, LocalHostProcessPort, ProductAuthProviderRuntimePorts, TriggerCreateHook,
    builtin_first_party_handlers_with_trigger_create_hook, builtin_first_party_package,
};
use brassclaw_processes::ProcessServices;
use brassclaw_product_workflow::ProductAuthTurnGateResumeDispatcher;
use brassclaw_resources::InMemoryResourceGovernor;
#[cfg(feature = "postgres")]
use brassclaw_resources::{FilesystemResourceGovernorStore, PersistentResourceGovernor};
use brassclaw_run_state::{InMemoryApprovalRequestStore, InMemoryRunStateStore};
use brassclaw_secrets::FilesystemSecretStore;
use brassclaw_secrets::SecretStore;
#[cfg(feature = "postgres")]
use brassclaw_secrets::{FilesystemCredentialBroker, PgCredentialBroker, PgSecretStore};
use brassclaw_threads::InMemorySessionThreadService;
use brassclaw_threads::SessionThreadService;
use brassclaw_triggers::{
    TRIGGER_TRUSTED_ADAPTER_INSTALLATION_ID, TRIGGER_TRUSTED_ADAPTER_KIND,
    TRIGGER_TRUSTED_EXTERNAL_ACTOR_NAMESPACE, TriggerError, TriggerRecord, TriggerRepository,
};
use brassclaw_trust::{AdminConfig, AdminEntry, HostTrustAssignment, HostTrustPolicy};
#[cfg(feature = "postgres")]
use brassclaw_turns::InMemoryRunProfileResolver;
use brassclaw_turns::{CheckpointStateStore, DefaultTurnCoordinator, LoopCheckpointStore};
use brassclaw_turns::{
    InMemoryCheckpointStateStore, InMemoryLoopCheckpointStore, InMemoryTurnStateStore,
};

use crate::RebornProductAuthServicePorts;
use crate::default_system_prompt::seed_default_system_prompt;
use crate::input::{RebornRuntimeProcessBinding, RebornStorageInput};
use crate::lifecycle::{RebornLocalSkillManagementPort, build_local_skill_management_port};
use crate::local_dev_capability_policy::local_dev_capability_policy;
use crate::local_dev_mounts::{
    ambient_workspace_mount_view, memory_mount_view, skill_context_mount_view,
    skill_management_mount_view, workspace_mount_view,
};
use crate::mcp::hosted_http_mcp_runtime;
use crate::product_auth_providers::{OAuthProviderComposition, compose_provider_client};
use crate::product_auth_runtime_credentials::ProductAuthRuntimeCredentialResolver;
use crate::{
    RebornAuthContinuationDispatcher, RebornBuildError, RebornBuildInput, RebornCompositionProfile,
    RebornFacadeReadiness, RebornProductAuthServices, RebornReadiness, RebornReadinessState,
    RebornWorkerReadiness,
};
use crate::{
    available_extensions::{
        AvailableExtensionCatalog, gmail_manifest_digest, google_calendar_manifest_digest,
        notion_mcp_manifest_digest, web_access_manifest_digest,
    },
    extension_installation_store::FilesystemExtensionInstallationStore,
    extension_lifecycle::{
        ActiveExtensionPublisher, RebornLocalExtensionManagementPort,
        restore_extension_lifecycle_state,
    },
    extension_lifecycle_capabilities::{
        extend_builtin_first_party_package, insert_handlers as insert_extension_lifecycle_handlers,
    },
    gsuite::{
        ProductAuthRuntimeGsuiteCredentialStager, register_bundled_gsuite_first_party_handlers,
    },
    web_access::register_bundled_web_access_first_party_handlers,
};

pub(crate) type LocalDevRootFilesystem = CompositeRootFilesystem;

/// Output of [`build_local_dev_root_filesystem`]: the composed local-dev
/// root filesystem and, when libSQL is the substrate, a clone of the raw
/// libSQL handle. The handle backs both the local-dev trigger repository
/// and the canonical Reborn identity store, so each rides the same
/// `reborn-local-dev.db` rather than opening a second handle to the file
/// (see `RebornRuntime::open_reborn_identity_resolver`).
struct LocalDevRootFilesystemBundle {
    filesystem: Arc<LocalDevRootFilesystem>,
}

type LocalDevWorkspaceFilesystems = (
    Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    MountView,
);

const LOCAL_DEV_DEFAULT_SYSTEM_PROMPT_PATH: &str = "system/prompts/default-system.md";
const LOCAL_DEV_SECRETS_MASTER_KEY_PATH: &str = ".reborn-local-dev-secrets-master-key";

pub(crate) type LocalDevTurnStateStore = InMemoryTurnStateStore;

type LocalDevResourceGovernor = InMemoryResourceGovernor;

type LocalDevRunStateStore = InMemoryRunStateStore;

pub(crate) type LocalDevApprovalRequestStore = InMemoryApprovalRequestStore;

pub(crate) type LocalDevCapabilityLeaseStore = InMemoryCapabilityLeaseStore;

type LocalDevProcessServices = ProcessServices<
    brassclaw_processes::InMemoryProcessStore,
    brassclaw_processes::InMemoryProcessResultStore,
>;

fn apply_runtime_process_binding<F, G, S, R>(
    services: HostRuntimeServices<F, G, S, R>,
    binding: RebornRuntimeProcessBinding,
) -> HostRuntimeServices<F, G, S, R>
where
    F: brassclaw_filesystem::RootFilesystem + 'static,
    G: brassclaw_resources::ResourceGovernor + 'static,
    S: brassclaw_processes::ProcessStore + 'static,
    R: brassclaw_processes::ProcessResultStore + 'static,
{
    match binding {
        RebornRuntimeProcessBinding::None => services,
        RebornRuntimeProcessBinding::TenantSandbox { process_port } => {
            services.with_tenant_sandbox_process_port(process_port)
        }
    }
}

fn local_dev_process_port_for_policy(
    runtime_policy: &Option<brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy>,
    workspace_root: &Path,
    host_home_root: Option<&LocalDevHostHomeRoot>,
) -> Option<LocalHostProcessPort> {
    let runtime_policy = runtime_policy.as_ref()?;
    if runtime_policy.process_backend != ProcessBackendKind::LocalHost {
        return None;
    }
    let mut process_port = if runtime_policy.secret_mode == SecretMode::InheritedEnv {
        LocalHostProcessPort::new_inherited_env()
    } else {
        LocalHostProcessPort::new()
    }
    .with_workdir_alias("/workspace", workspace_root);
    if let Some(host_home_root) = host_home_root {
        process_port =
            process_port.with_workdir_alias("/host", host_home_root.canonical_root.clone());
        for alias in host_home_root.aliases() {
            let alias_str = match alias.to_str() {
                Some(s) => s,
                None => {
                    tracing::debug!(alias = ?alias, "skipping non-UTF-8 host home alias");
                    continue;
                }
            };
            process_port = process_port.with_workdir_alias(alias_str, alias.to_path_buf());
        }
    }
    Some(process_port)
}

fn require_product_auth_runtime_ports<F, G, S, R>(
    services: &HostRuntimeServices<F, G, S, R>,
) -> Result<ProductAuthProviderRuntimePorts, RebornBuildError>
where
    F: brassclaw_filesystem::RootFilesystem + 'static,
    G: brassclaw_resources::ResourceGovernor + 'static,
    S: brassclaw_processes::ProcessStore + 'static,
    R: brassclaw_processes::ProcessResultStore + 'static,
{
    services
        .product_auth_provider_runtime_ports()
        .ok_or_else(|| RebornBuildError::InvalidConfig {
            reason: "product auth runtime ports unavailable; host runtime must be configured with HTTP egress and a secret store".to_string(),
        })
}

fn attach_hosted_mcp_runtime<F, G, S, R>(
    services: HostRuntimeServices<F, G, S, R>,
) -> Result<HostRuntimeServices<F, G, S, R>, RebornBuildError>
where
    F: brassclaw_filesystem::RootFilesystem + 'static,
    G: brassclaw_resources::ResourceGovernor + 'static,
    S: brassclaw_processes::ProcessStore + 'static,
    R: brassclaw_processes::ProcessResultStore + 'static,
{
    // Soft-disable when host runtime HTTP egress is absent. Builds without
    // egress — in-memory test services, minimal compositions — must still
    // succeed; only hosted MCP capabilities go dark.
    let Some(runtime_ports) = services.product_auth_provider_runtime_ports() else {
        tracing::debug!(
            "skipping hosted MCP runtime: host runtime HTTP egress absent \
             (only affects hosted MCP extensions, e.g. Notion, NEAR AI)"
        );
        return Ok(services);
    };
    let runtime_http_egress = runtime_ports.runtime_http_egress();
    let registry = services.shared_extension_registry();

    Ok(services.with_mcp_runtime(Arc::new(hosted_http_mcp_runtime(
        registry,
        runtime_http_egress,
    ))))
}

#[cfg(feature = "postgres")]
pub(crate) fn apply_production_runtime_process_binding<F, G, S, R>(
    services: HostRuntimeServices<F, G, S, R>,
    binding: RebornRuntimeProcessBinding,
) -> HostRuntimeServices<F, G, S, R>
where
    F: brassclaw_filesystem::RootFilesystem + 'static,
    G: brassclaw_resources::ResourceGovernor + 'static,
    S: brassclaw_processes::ProcessStore + 'static,
    R: brassclaw_processes::ProcessResultStore + 'static,
{
    match binding {
        RebornRuntimeProcessBinding::None => services,
        RebornRuntimeProcessBinding::TenantSandbox { process_port } => {
            services.with_production_tenant_sandbox_process_port(process_port)
        }
    }
}

pub struct RebornServices {
    pub host_runtime: Option<Arc<dyn brassclaw_host_runtime::HostRuntime>>,
    pub turn_coordinator: Option<Arc<dyn brassclaw_turns::TurnCoordinator>>,
    pub product_auth: Option<Arc<RebornProductAuthServices>>,
    pub readiness: RebornReadiness,
    pub(crate) local_runtime: Option<Arc<RebornLocalRuntimeServices>>,
    /// Postgres connection pool threaded from the production build path so
    /// `build_reborn_runtime` can pass it to the hooks predicate-state backend
    /// (`PostgresPredicateStateBackend`) instead of the in-memory fallback.
    /// `None` in local-dev until embedded PG is wired (Phase 6).
    #[cfg(feature = "postgres")]
    pub(crate) pg_pool: Option<Arc<deadpool_postgres::Pool>>,
    /// Shared scoped secret store. Exposed so runtime-level features (e.g.
    /// operator LLM-key storage) can reuse the same instance product-auth uses
    /// rather than standing up a second authority.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) secret_store: Arc<dyn SecretStore>,
    /// Postgres-backed safety config store (production path).
    #[cfg(feature = "postgres")]
    pub(crate) pg_safety_config_store: Option<Arc<brassclaw_product_workflow::PgSafetyConfigStore>>,
    /// Postgres-backed token settings store (production path).
    #[cfg(feature = "postgres")]
    pub(crate) pg_token_settings_store:
        Option<Arc<crate::pg_token_settings_store::PgTokenSettingsStore>>,
    /// Postgres-backed engine `Store` for `MemoryDoc` operations (production path).
    #[cfg(feature = "postgres")]
    pub(crate) pg_memory_doc_store: Option<Arc<crate::pg_memory_doc_store::PgMemoryDocStore>>,
}

impl RebornServices {
    /// The shared scoped secret store backing this composition.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn secret_store(&self) -> Arc<dyn SecretStore> {
        Arc::clone(&self.secret_store)
    }

    /// The Postgres connection pool, if this composition uses a Postgres backend.
    #[cfg(feature = "postgres")]
    pub fn pg_pool(&self) -> Option<&Arc<deadpool_postgres::Pool>> {
        self.pg_pool.as_ref()
    }
}

pub(crate) struct RebornLocalRuntimeServices {
    pub(crate) approval_requests: Arc<LocalDevApprovalRequestStore>,
    pub(crate) capability_leases: Arc<LocalDevCapabilityLeaseStore>,
    pub(crate) turn_state: Arc<LocalDevTurnStateStore>,
    pub(crate) trigger_repository: Arc<dyn TriggerRepository>,
    pub(crate) trigger_conversation_services:
        tokio::sync::OnceCell<RebornFilesystemConversationServices>,
    pub(crate) checkpoint_state_store: Arc<dyn CheckpointStateStore>,
    pub(crate) loop_checkpoint_store: Arc<dyn LoopCheckpointStore>,
    pub(crate) thread_service: Arc<dyn SessionThreadService>,
    /// Resource governor handle used by the budget accountant. Kept here
    /// separately from the type-erased `dyn HostRuntime` so the runtime
    /// composer can construct a `GovernorBackedAccountant` without losing
    /// the concrete governor type. Wired through #3841 follow-up "A1: wire
    /// GovernorBackedAccountant into production composition".
    pub(crate) resource_governor: Arc<dyn brassclaw_resources::ResourceGovernor>,
    /// Sink that receives `BudgetEvent`s from the governor. Composition
    /// hands this to downstream consumers (audit log, SSE projection)
    /// without forcing the governor to know about them. Wired through
    /// #3841 follow-up "A2: project BudgetEvent into the gateway event
    /// stream".
    #[allow(dead_code)]
    pub(crate) budget_event_sink: Arc<dyn brassclaw_resources::BudgetEventSink>,
    /// Same sink as `budget_event_sink` but typed as the concrete
    /// `InMemoryBudgetEventSink` so the runtime can expose `drain()` /
    /// `snapshot()` to tests without leaking the concrete type into the
    /// production `BudgetEventSink` boundary.
    #[allow(dead_code)]
    pub(crate) in_memory_budget_event_sink: Arc<brassclaw_resources::InMemoryBudgetEventSink>,
    /// Broadcast sink production callers can subscribe against once a
    /// real projection caller lands (review feedback Thermo-Nuclear
    /// #3: the speculative `src/bridge/budget_events.rs` helper plus
    /// `AppEvent::Budget` variant were removed pending an owner that
    /// actually spawns a projection task with shutdown cancellation).
    /// Composition fans every BudgetEvent through this alongside the
    /// in-memory sink so tests can still inspect history.
    pub(crate) broadcast_budget_event_sink: Arc<brassclaw_resources::BroadcastBudgetEventSink>,
    /// Approval-gate store used to surface `BudgetApprovalRequired` to a
    /// user. Stays in-memory in local-dev; production composition will
    /// swap in the filesystem-backed `FilesystemBudgetGateStore`.
    #[allow(dead_code)]
    pub(crate) budget_gate_store: Arc<dyn brassclaw_resources::BudgetGateStore>,
    pub(crate) skill_management: Arc<RebornLocalSkillManagementPort>,
    // LocalSingleUser-only for now. Production and multi-tenant lifecycle
    // wiring need scoped storage/registry ownership before this is reused
    // outside local-dev composition. Tracked in #4091.
    pub(crate) extension_management: Option<Arc<RebornLocalExtensionManagementPort>>,
    pub(crate) runtime_http_egress: Option<Arc<dyn RuntimeHttpEgress>>,
    pub(crate) host_runtime_http_egress: Option<HostRuntimeHttpEgressPort>,
    pub(crate) skill_mounts: MountView,
    pub(crate) memory_mounts: MountView,
    pub(crate) skill_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    pub(crate) workspace_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    pub(crate) subagent_goal_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    /// Tenant-scoped root filesystem used for third-party extension hook
    /// discovery (`/system/extensions/<tenant>`). The runtime derives the
    /// discovery root from the authenticated tenant id; this is the same
    /// backend the rest of local-dev composition uses.
    pub(crate) extension_filesystem: Arc<LocalDevRootFilesystem>,
    pub(crate) workspace_mounts: MountView,
    pub(crate) local_dev_storage_root: PathBuf,
    pub(crate) default_system_prompt_path: PathBuf,
    pub(crate) event_log: Arc<dyn DurableEventLog>,
    pub(crate) audit_log: Arc<dyn DurableAuditLog>,
    /// Canonical registry shared by capability dispatch and hook activation.
    pub(crate) extension_registry: Arc<ExtensionRegistry>,
    /// Shared content-cache bridge slot updated per turn by the capability port
    /// decorator and read by the `fetch_cached_content` first-party handler.
    pub(crate) content_cache_slot: brassclaw_reborn::content_cache_port::CurrentCacheBridgeSlot,
    /// Shared plan-state slot updated after each completed run by the
    /// plan library post-turn bridge. The `PlanLibraryService` reads it.
    pub(crate) plan_state_slot: crate::plan_library::CurrentPlanStateSlot,
}

impl RebornLocalRuntimeServices {
    pub(crate) async fn durable_trigger_conversation_services(
        &self,
    ) -> Result<RebornFilesystemConversationServices, InboundTurnError> {
        let filesystem = Arc::clone(&self.subagent_goal_filesystem);
        self.trigger_conversation_services
            .get_or_try_init(|| async move {
                RebornFilesystemConversationServices::new(filesystem).await
            })
            .await
            .cloned()
    }
}

struct RebornLocalDevStoreGraph {
    run_state: Arc<LocalDevRunStateStore>,
    approval_requests: Arc<LocalDevApprovalRequestStore>,
    capability_leases: Arc<LocalDevCapabilityLeaseStore>,
    turn_state: Arc<LocalDevTurnStateStore>,
    local_runtime: Arc<RebornLocalRuntimeServices>,
    resource_governor: Arc<LocalDevResourceGovernor>,
    process_services: LocalDevProcessServices,
    trigger_repository: Arc<dyn TriggerRepository>,
}

struct RebornLocalDevStoreGraphInput {
    filesystem: Arc<LocalDevRootFilesystem>,
    owner_user_id: UserId,
    skill_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    workspace_filesystem: Arc<ScopedFilesystem<LocalDevRootFilesystem>>,
    workspace_mounts: MountView,
    local_dev_storage_root: PathBuf,
    default_system_prompt_path: PathBuf,
    trigger_repository: Arc<dyn TriggerRepository>,
    extension_registry: Arc<ExtensionRegistry>,
}

impl std::fmt::Debug for RebornServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RebornServices")
            .field("host_runtime", &self.host_runtime.is_some())
            .field("turn_coordinator", &self.turn_coordinator.is_some())
            .field("product_auth", &self.product_auth.is_some())
            .field("readiness", &self.readiness)
            .field("local_runtime", &self.local_runtime.is_some())
            .finish()
    }
}

impl RebornServices {
    pub fn disabled() -> Self {
        Self {
            host_runtime: None,
            turn_coordinator: None,
            product_auth: None,
            readiness: RebornReadiness::disabled(),
            local_runtime: None,
            #[cfg(feature = "postgres")]
            pg_pool: None,
            #[cfg(feature = "root-llm-provider")]
            secret_store: Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            #[cfg(feature = "postgres")]
            pg_safety_config_store: None,
            #[cfg(feature = "postgres")]
            pg_token_settings_store: None,
            #[cfg(feature = "postgres")]
            pg_memory_doc_store: None,
        }
    }
}

pub async fn build_reborn_services(
    input: RebornBuildInput,
) -> Result<RebornServices, RebornBuildError> {
    tracing::debug!(
        profile = %input.profile,
        owner_id = %input.owner_id,
        "building Reborn composition facades"
    );
    match input.profile {
        RebornCompositionProfile::Disabled => Ok(RebornServices::disabled()),
        RebornCompositionProfile::LocalDev | RebornCompositionProfile::LocalDevYolo => {
            // Phase-5 hybrid path: when a Postgres pool is supplied alongside a
            // local-dev profile (the production `brassclaw serve` path), build
            // the local-dev filesystem substrate *and* expose the PG pool.
            // The local-dev filesystem provides the workspace, skills, hooks and
            // extension infrastructure; `build_reborn_runtime` picks up the pool
            // from `services.pg_pool` to upgrade thread service and subagent goal
            // store to PG-backed implementations.  Full turn-state / run-state /
            // approval / lease wiring to PG is tracked in subplan_pg4_runtime_pg_path.md.
            #[cfg(feature = "postgres")]
            if let RebornStorageInput::Postgres {
                pool,
                reborn_home,
                ..
            } = &input.storage
            {
                // Convert the Postgres input into a local-dev input using the
                // reborn_home as the local filesystem root, then add the PG pool.
                let local_root = reborn_home.join("db");
                let local_input = RebornBuildInput::local_dev_with_profile(
                    input.profile,
                    input.owner_id.clone(),
                    local_root,
                );
                // Copy over any extra fields that may have been set on the input.
                let local_input = transfer_build_input_extras(local_input, &input);
                let pg_pool_arc = Arc::new(pool.clone());
                let mut services = build_local_dev(local_input).await?;
                // Inject the PG pool so build_reborn_runtime can use PG-backed stores.
                services.pg_pool = Some(pg_pool_arc);
                // NOTE: `local_runtime.trigger_repository` is still `InMemoryTriggerRepository`
                // because Arc::get_mut would fail (the Arc is aliased by the trigger-create hook
                // stored inside host_runtime).  Fixing this properly requires threading a
                // `trigger_repository_override` through `build_local_dev` — tracked in
                // subplan_pg4_runtime_pg_path.md.
                return Ok(services);
            }
            build_local_dev(input).await
        }
    }
}

/// Copy runtime-policy and other extra fields that cannot be set via
/// `RebornBuildInput::new` from `src` into `dst`.
///
/// Used by the Phase-5 hybrid path in [`build_reborn_services`] to preserve
/// the caller-supplied runtime policy, process binding, and OAuth configs
/// when converting a Postgres storage input to a local-dev storage input.
#[cfg(feature = "postgres")]
fn transfer_build_input_extras(mut dst: RebornBuildInput, src: &RebornBuildInput) -> RebornBuildInput {
    dst.runtime_policy = src.runtime_policy.clone();
    dst.runtime_process_binding = src.runtime_process_binding.clone();
    dst.product_auth_ports = src.product_auth_ports.clone();
    dst.oauth_provider_configs = src.oauth_provider_configs.clone();
    dst.oauth_dcr_provider_configs = src.oauth_dcr_provider_configs.clone();
    dst.required_runtime_backends = src.required_runtime_backends.clone();
    dst.require_runtime_http_egress = src.require_runtime_http_egress;
    dst.production_trust_policy = src.production_trust_policy.clone();
    dst.turn_run_wake_notifier = src.turn_run_wake_notifier.clone();
    dst
}

fn auth_continuation_dispatcher(
    turn_coordinator: Arc<dyn brassclaw_turns::TurnCoordinator>,
) -> Arc<dyn RebornAuthContinuationDispatcher> {
    Arc::new(ProductAuthTurnGateResumeDispatcher::new(turn_coordinator))
}

fn compose_product_auth_services(
    ports: RebornProductAuthServicePorts,
    turn_coordinator: Arc<dyn brassclaw_turns::TurnCoordinator>,
    provider_composition: OAuthProviderComposition,
) -> Arc<RebornProductAuthServices> {
    let ports = match provider_composition.client {
        Some(provider_client) => ports.with_provider_client(provider_client),
        None => ports,
    };
    let mut services = ports.into_services(auth_continuation_dispatcher(turn_coordinator));
    if let Some(registry) = provider_composition.dcr_registry {
        services = services.with_dcr_oauth_registry(registry);
    }
    if let Some(registry) = provider_composition.gate_registry {
        services = services.with_oauth_gate_registry(registry);
    }
    Arc::new(services)
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
fn production_config(
    required_runtime_backends: Vec<brassclaw_host_api::RuntimeKind>,
    require_runtime_http_egress: bool,
) -> brassclaw_host_runtime::ProductionWiringConfig {
    let mut config = brassclaw_host_runtime::ProductionWiringConfig::new(required_runtime_backends);
    if require_runtime_http_egress {
        config = config.require_runtime_http_egress();
    }
    config.require_credential_broker()
}

async fn build_local_dev(input: RebornBuildInput) -> Result<RebornServices, RebornBuildError> {
    let RebornBuildInput {
        profile,
        storage,
        runtime_policy,
        runtime_process_binding,
        product_auth_ports,
        oauth_provider_configs,
        oauth_dcr_provider_configs,
        owner_id,
        ..
    } = input;
    let RebornStorageInput::LocalDev {
        root,
        workspace_root,
        host_home_root,
    } = storage
    else {
        return Err(RebornBuildError::InvalidConfig {
            reason: "local-dev profile requires local-dev storage input".to_string(),
        });
    };
    std::fs::create_dir_all(&root).map_err(|_| RebornBuildError::InvalidConfig {
        reason: "local-dev storage root could not be initialized".to_string(),
    })?;
    std::fs::create_dir_all(root.join("system/extensions")).map_err(|_| {
        RebornBuildError::InvalidConfig {
            reason: "local-dev system extensions root could not be initialized".to_string(),
        }
    })?;
    let workspace_root = workspace_root.unwrap_or_else(|| root.join("workspace"));
    std::fs::create_dir_all(&workspace_root).map_err(|_| RebornBuildError::InvalidConfig {
        reason: "local-dev workspace root could not be initialized".to_string(),
    })?;
    let root = canonicalize_local_dev_path(&root, "storage root")?;
    let workspace_root = canonicalize_local_dev_path(&workspace_root, "workspace root")?;
    let include_host_home = runtime_policy.as_ref().is_some_and(|policy| {
        policy.filesystem_backend == FilesystemBackendKind::HostWorkspaceAndHome
    });
    let host_home_root = match (include_host_home, host_home_root) {
        (true, Some(path)) => Some(LocalDevHostHomeRoot {
            canonical_root: canonicalize_local_dev_host_home_root(&path)?,
            raw_alias: path,
        }),
        (true, None) => {
            return Err(RebornBuildError::InvalidConfig {
                reason: "local-dev-yolo host home access requires a confirmed host home root"
                    .to_string(),
            });
        }
        (false, Some(_)) => {
            return Err(RebornBuildError::InvalidConfig {
                reason:
                    "confirmed host home root was supplied but the resolved runtime policy does not allow host home access"
                        .to_string(),
            });
        }
        (false, None) => None,
    };
    validate_local_dev_workspace_skill_isolation(&root, &workspace_root)?;
    let default_system_prompt_path = local_dev_default_system_prompt_path(&root);
    seed_default_system_prompt(&root, &default_system_prompt_path).map_err(|error| {
        RebornBuildError::InvalidConfig {
            reason: error.to_string(),
        }
    })?;
    crate::bundled_skills::ensure_bundled_reborn_skills_installed(&root).await?;
    let filesystem_bundle =
        build_local_dev_root_filesystem(&root, &workspace_root, host_home_root.as_ref()).await?;
    let filesystem = filesystem_bundle.filesystem;
    let trigger_repository = local_dev_trigger_repository();
    let (skill_filesystem, workspace_filesystem, runtime_workspace_mounts) =
        build_workspace_filesystems(
            Arc::clone(&filesystem),
            &workspace_root,
            host_home_root.as_ref(),
        )?;
    let http_body_filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::clone(&filesystem),
        runtime_workspace_mounts.clone(),
    ));
    let owner_user_id = UserId::new(owner_id).map_err(|error| RebornBuildError::InvalidConfig {
        reason: error.to_string(),
    })?;

    // Create extension_registry BEFORE store_graph so it can be passed in
    let extension_registry = Arc::new(local_dev_builtin_extension_registry()?);
    tracing::debug!(
        count = extension_registry.capabilities().count(),
        "local-dev: built extension_registry"
    );

    let mut store_graph = build_local_dev_store_graph(RebornLocalDevStoreGraphInput {
        filesystem: Arc::clone(&filesystem),
        owner_user_id,
        skill_filesystem,
        workspace_filesystem,
        workspace_mounts: runtime_workspace_mounts,
        local_dev_storage_root: root.clone(),
        default_system_prompt_path,
        trigger_repository,
        extension_registry: Arc::clone(&extension_registry),
    })
    .await?;

    let turn_coordinator: Arc<dyn brassclaw_turns::TurnCoordinator> = Arc::new(
        DefaultTurnCoordinator::new(Arc::clone(&store_graph.turn_state)),
    );
    let local_dev_product_auth_filesystem = local_dev_scoped_filesystem(Arc::clone(&filesystem));
    let local_dev_secret_store =
        build_local_dev_secret_store(&root, Arc::clone(&local_dev_product_auth_filesystem))?;
    let secret_store: Arc<dyn SecretStore> = local_dev_secret_store.clone();
    let local_dev_trust_policy = Arc::new(local_dev_first_party_trust_policy()?);
    let local_dev_trust_invalidation_bus = Arc::new(brassclaw_trust::InvalidationBus::new());
    let mut services = HostRuntimeServices::new(
        Arc::clone(&extension_registry),
        Arc::clone(&filesystem),
        Arc::clone(&store_graph.resource_governor),
        Arc::new(GrantAuthorizer::new()),
        store_graph.process_services.clone(),
        CapabilitySurfaceVersion::new("reborn-app-v1")?,
    )
    .with_trust_policy(Arc::clone(&local_dev_trust_policy))
    .with_secret_store_dyn(Arc::clone(&secret_store))
    .try_with_host_http_egress_with_body_store(
        brassclaw_network::PolicyNetworkHttpEgress::new(
            brassclaw_network::ReqwestNetworkTransport::default(),
        ),
        http_body_filesystem,
    )?
    .with_run_state(Arc::clone(&store_graph.run_state))
    .with_approval_requests(Arc::clone(&store_graph.approval_requests))
    .with_capability_leases(Arc::clone(&store_graph.capability_leases))
    .with_turn_state_and_transition_port(Arc::clone(&store_graph.turn_state));
    let local_dev_process_port = local_dev_process_port_for_policy(
        &runtime_policy,
        &workspace_root,
        host_home_root.as_ref(),
    );
    if let Some(runtime_policy) = runtime_policy {
        services = services.with_runtime_policy(runtime_policy);
    }
    if let Some(process_port) = local_dev_process_port {
        services = services.with_runtime_process_port(Arc::new(process_port));
    }
    services = apply_runtime_process_binding(services, runtime_process_binding);
    services = attach_hosted_mcp_runtime(services)?;
    let product_auth_runtime_ports = require_product_auth_runtime_ports(&services)?;
    let provider_composition = compose_provider_client(
        oauth_provider_configs,
        oauth_dcr_provider_configs,
        Arc::clone(&secret_store),
        product_auth_runtime_ports.clone(),
    )?;
    let product_auth = match product_auth_ports {
        Some(ports) => {
            compose_product_auth_services(ports, turn_coordinator.clone(), provider_composition)
        }
        None => {
            let durable_services = Arc::new(FilesystemAuthProductServices::new(
                local_dev_product_auth_filesystem,
                Arc::clone(&secret_store),
            ));
            let provider_client: Arc<dyn AuthProviderClient> = provider_composition
                .client
                .clone()
                .unwrap_or_else(|| Arc::new(UnavailableAuthProviderClient));
            let services = RebornProductAuthServicePorts::from_shared_with_provider(
                Arc::clone(&durable_services),
                provider_client,
            )
            .into_services(auth_continuation_dispatcher(turn_coordinator.clone()))
            .with_flow_record_source(durable_services);
            let services = match provider_composition.dcr_registry.clone() {
                Some(registry) => services.with_dcr_oauth_registry(registry),
                None => services,
            };
            let services = match provider_composition.gate_registry.clone() {
                Some(registry) => services.with_oauth_gate_registry(registry),
                None => services,
            };
            Arc::new(services)
        }
    };
    services = services.with_runtime_credential_account_resolver(Arc::new(
        ProductAuthRuntimeCredentialResolver::new(
            product_auth.runtime_credential_account_selection_service(),
        ),
    ));
    let mut available_extensions = AvailableExtensionCatalog::from_filesystem_root(
        filesystem.as_ref(),
        &VirtualPath::new("/system/extensions")?,
    )
    .await
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("available extension catalog could not be loaded: {error}"),
    })?;
    available_extensions.extend(
        AvailableExtensionCatalog::from_first_party_assets().map_err(|error| {
            RebornBuildError::InvalidConfig {
                reason: format!("first-party extension catalog could not be loaded: {error}"),
            }
        })?,
    );
    let extension_filesystem: Arc<dyn RootFilesystem> = filesystem.clone();
    let extension_installation_store: Arc<dyn ExtensionInstallationStore> = Arc::new(
        FilesystemExtensionInstallationStore::load(extension_filesystem.clone())
            .await
            .map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("extension installation state could not be loaded: {error}"),
            })?,
    );
    let extension_lifecycle_service = Arc::new(tokio::sync::Mutex::new(
        ExtensionLifecycleService::new(services.shared_extension_registry().snapshot_owned()),
    ));
    let active_registry = services.shared_extension_registry();
    let active_extensions = ActiveExtensionPublisher::new(
        active_registry,
        local_dev_trust_policy,
        local_dev_trust_invalidation_bus,
    );
    restore_extension_lifecycle_state(
        &available_extensions,
        &extension_filesystem,
        &extension_installation_store,
        &extension_lifecycle_service,
        &active_extensions,
    )
    .await
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("extension lifecycle state could not be restored: {error}"),
    })?;
    let extension_management = Arc::new(RebornLocalExtensionManagementPort::new(
        extension_filesystem,
        available_extensions,
        extension_installation_store,
        extension_lifecycle_service,
        active_extensions,
    ));
    if let Some(local_runtime) = Arc::get_mut(&mut store_graph.local_runtime) {
        local_runtime.extension_management = Some(Arc::clone(&extension_management));
        local_runtime.runtime_http_egress = Some(product_auth_runtime_ports.runtime_http_egress());
        // extension_registry is now set during store_graph creation, no need to set it here
        let host_runtime_http_egress = services.host_runtime_http_egress_port();
        local_runtime.host_runtime_http_egress = host_runtime_http_egress;
    } else {
        return Err(RebornBuildError::InvalidConfig {
            reason: "local-dev extension lifecycle facade could not be attached".to_string(),
        });
    }
    let trigger_create_hook = local_dev_trigger_create_hook(&store_graph.local_runtime);
    let mut first_party_registry = builtin_first_party_registry_with_trigger_create_hook(
        Arc::clone(&store_graph.trigger_repository),
        trigger_create_hook,
    )?;
    register_bundled_gsuite_first_party_handlers(
        &mut first_party_registry,
        product_auth.credential_account_service(),
        product_auth.credential_account_record_source(),
        Arc::new(ProductAuthRuntimeGsuiteCredentialStager::new(
            product_auth_runtime_ports.clone(),
        )),
    )
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("GSuite first-party handlers are invalid: {error}"),
    })?;
    register_bundled_web_access_first_party_handlers(&mut first_party_registry).map_err(
        |error| RebornBuildError::InvalidConfig {
            reason: format!("web access first-party handlers are invalid: {error}"),
        },
    )?;
    insert_extension_lifecycle_handlers(&mut first_party_registry, extension_management).map_err(
        |error| RebornBuildError::InvalidConfig {
            reason: format!("local-dev extension lifecycle handlers are invalid: {error}"),
        },
    )?;
    crate::fetch_cached_content::register_fetch_cached_content_handler(
        &mut first_party_registry,
        store_graph.local_runtime.content_cache_slot.clone(),
    )
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("fetch_cached_content handler is invalid: {error}"),
    })?;
    // submit_skill_candidate: internal-only handler, not exposed in any
    // capability schema.  Registered here so the plan library can invoke it
    // via the first-party registry when a skill reaches the Candidate tier.
    // (Currently a no-op stub — actual invocation happens directly in
    // `PlanLibraryService::submit_skill_candidate`.)
    services = services.with_first_party_capabilities(Arc::new(first_party_registry));

    let host_runtime: Arc<dyn brassclaw_host_runtime::HostRuntime> =
        Arc::new(services.host_runtime_for_local_testing());

    Ok(RebornServices {
        host_runtime: Some(host_runtime),
        turn_coordinator: Some(turn_coordinator),
        // Local-dev always composes a safe in-memory product-auth boundary when
        // the caller does not inject one; readiness tracks the assembled facade.
        product_auth: Some(product_auth),
        readiness: readiness_for(profile, true, true, true),
        local_runtime: Some(Arc::clone(&store_graph.local_runtime)),
        // No PG pool in local-dev until embedded PG is wired (Phase 6).
        #[cfg(feature = "postgres")]
        pg_pool: None,
        #[cfg(feature = "root-llm-provider")]
        secret_store,
        #[cfg(feature = "postgres")]
        pg_safety_config_store: None,
        #[cfg(feature = "postgres")]
        pg_token_settings_store: None,
        #[cfg(feature = "postgres")]
        pg_memory_doc_store: None,
    })
}

async fn build_local_dev_store_graph(
    input: RebornLocalDevStoreGraphInput,
) -> Result<RebornLocalDevStoreGraph, RebornBuildError> {
    let RebornLocalDevStoreGraphInput {
        filesystem,
        owner_user_id,
        skill_filesystem,
        workspace_filesystem,
        workspace_mounts,
        local_dev_storage_root,
        default_system_prompt_path,
        trigger_repository,
        extension_registry,
    } = input;
    let subagent_goal_filesystem = local_dev_scoped_filesystem(Arc::clone(&filesystem));
    let event_log = local_dev_event_log(Arc::clone(&filesystem))?;
    let audit_log = local_dev_audit_log(Arc::clone(&filesystem))?;
    let run_state = Arc::new(InMemoryRunStateStore::new());
    let approval_requests = Arc::new(InMemoryApprovalRequestStore::new());
    let capability_leases = Arc::new(InMemoryCapabilityLeaseStore::new());
    let turn_state = Arc::new(InMemoryTurnStateStore::default());
    let checkpoint_state_store: Arc<dyn CheckpointStateStore> =
        Arc::new(InMemoryCheckpointStateStore::default());
    let loop_checkpoint_store: Arc<dyn LoopCheckpointStore> =
        Arc::new(InMemoryLoopCheckpointStore::default());
    let thread_service: Arc<dyn SessionThreadService> =
        Arc::new(InMemorySessionThreadService::default());
    let BudgetSinks {
        budget_event_sink,
        in_memory_budget_event_sink,
        broadcast_budget_event_sink,
        budget_gate_store,
    } = build_budget_sinks();
    let resource_governor: Arc<LocalDevResourceGovernor> =
        Arc::new(InMemoryResourceGovernor::new().with_event_sink(Arc::clone(&budget_event_sink)));
    let skill_mounts =
        skill_management_mount_view().map_err(|error| RebornBuildError::InvalidConfig {
            reason: error.to_string(),
        })?;
    let memory_mounts =
        memory_mount_view(MountPermissions::read_write_list_delete()).map_err(|error| {
            RebornBuildError::InvalidConfig {
                reason: error.to_string(),
            }
        })?;
    let skill_management =
        build_local_skill_management_port(owner_user_id, Arc::clone(&filesystem))?;
    let local_runtime = Arc::new(RebornLocalRuntimeServices {
        approval_requests: Arc::clone(&approval_requests),
        capability_leases: Arc::clone(&capability_leases),
        turn_state: Arc::clone(&turn_state),
        trigger_repository: Arc::clone(&trigger_repository),
        trigger_conversation_services: tokio::sync::OnceCell::new(),
        checkpoint_state_store,
        loop_checkpoint_store,
        thread_service,
        resource_governor: Arc::clone(&resource_governor)
            as Arc<dyn brassclaw_resources::ResourceGovernor>,
        budget_event_sink,
        in_memory_budget_event_sink,
        broadcast_budget_event_sink,
        budget_gate_store,
        skill_management,
        extension_management: None,
        runtime_http_egress: None,
        host_runtime_http_egress: None,
        skill_mounts,
        memory_mounts,
        skill_filesystem,
        workspace_filesystem,
        subagent_goal_filesystem,
        extension_filesystem: Arc::clone(&filesystem),
        workspace_mounts,
        local_dev_storage_root,
        default_system_prompt_path,
        event_log,
        audit_log,
        extension_registry,
        content_cache_slot: brassclaw_reborn::content_cache_port::CurrentCacheBridgeSlot::new(),
        plan_state_slot: crate::plan_library::CurrentPlanStateSlot::new(),
    });
    let process_services = ProcessServices::in_memory();

    Ok(RebornLocalDevStoreGraph {
        run_state,
        approval_requests,
        capability_leases,
        turn_state,
        local_runtime,
        resource_governor,
        process_services,
        trigger_repository,
    })
}

fn local_dev_trigger_repository() -> Arc<dyn TriggerRepository> {
    Arc::new(brassclaw_triggers::InMemoryTriggerRepository::default())
}

fn local_dev_trigger_create_hook(
    local_runtime: &Arc<RebornLocalRuntimeServices>,
) -> Arc<dyn TriggerCreateHook> {
    Arc::new(LocalRuntimeTriggerCreatorPairingHook {
        runtime: Arc::clone(local_runtime),
    })
}

struct LocalRuntimeTriggerCreatorPairingHook {
    runtime: Arc<RebornLocalRuntimeServices>,
}

#[async_trait::async_trait]
impl TriggerCreateHook for LocalRuntimeTriggerCreatorPairingHook {
    async fn after_trigger_persisted(&self, record: &TriggerRecord) -> Result<(), TriggerError> {
        let conversations = self
            .runtime
            .durable_trigger_conversation_services()
            .await
            .map_err(|error| {
                trigger_pairing_error(TriggerPairingFailureSource::ConversationInit, error)
            })?;
        pair_trigger_creator(&conversations, record).await
    }
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
struct ScopedFilesystemTriggerCreatorPairingHook<F>
where
    F: RootFilesystem + 'static,
{
    filesystem: Arc<ScopedFilesystem<F>>,
    conversations: tokio::sync::OnceCell<RebornFilesystemConversationServices>,
}

#[cfg(feature = "postgres")]
impl<F> ScopedFilesystemTriggerCreatorPairingHook<F>
where
    F: RootFilesystem + 'static,
{
    fn new(filesystem: Arc<ScopedFilesystem<F>>) -> Self {
        Self {
            filesystem,
            conversations: tokio::sync::OnceCell::new(),
        }
    }
}

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl<F> TriggerCreateHook for ScopedFilesystemTriggerCreatorPairingHook<F>
where
    F: RootFilesystem + 'static,
{
    async fn after_trigger_persisted(&self, record: &TriggerRecord) -> Result<(), TriggerError> {
        let filesystem = Arc::clone(&self.filesystem);
        let conversations = self
            .conversations
            .get_or_try_init(|| async move {
                RebornFilesystemConversationServices::new(filesystem)
                    .await
                    .map_err(|error| {
                        trigger_pairing_error(TriggerPairingFailureSource::ConversationInit, error)
                    })
            })
            .await
            .cloned()?;
        pair_trigger_creator(&conversations, record).await
    }
}

async fn pair_trigger_creator(
    pairing: &dyn ConversationActorPairingService,
    record: &TriggerRecord,
) -> Result<(), TriggerError> {
    let adapter_kind = AdapterKind::new(TRIGGER_TRUSTED_ADAPTER_KIND).map_err(|error| {
        trigger_pairing_error(TriggerPairingFailureSource::TypedIdentity, error)
    })?;
    let adapter_installation_id =
        AdapterInstallationId::new(TRIGGER_TRUSTED_ADAPTER_INSTALLATION_ID).map_err(|error| {
            trigger_pairing_error(TriggerPairingFailureSource::TypedIdentity, error)
        })?;
    let external_actor_ref = ExternalActorRef::new(
        TRIGGER_TRUSTED_EXTERNAL_ACTOR_NAMESPACE,
        record.creator_user_id.as_str(),
    )
    .map_err(|error| trigger_pairing_error(TriggerPairingFailureSource::TypedIdentity, error))?;
    pairing
        .pair_external_actor(
            record.tenant_id.clone(),
            adapter_kind,
            adapter_installation_id,
            external_actor_ref,
            record.creator_user_id.clone(),
        )
        .await
        .map_err(|error| trigger_pairing_error(TriggerPairingFailureSource::ActorPairing, error))
}

enum TriggerPairingFailureSource {
    TypedIdentity,
    ConversationInit,
    ActorPairing,
}

impl TriggerPairingFailureSource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::TypedIdentity => "typed_identity",
            Self::ConversationInit => "conversation_init",
            Self::ActorPairing => "actor_pairing",
        }
    }
}

fn trigger_pairing_error(
    source: TriggerPairingFailureSource,
    _error: impl std::fmt::Display,
) -> TriggerError {
    tracing::debug!(
        error_kind = "pairing_failure",
        error_source = source.as_str(),
        "trigger creator actor pairing failed"
    );
    TriggerError::Backend {
        reason: "trigger creator actor pairing failed".to_string(),
    }
}

struct BudgetSinks {
    budget_event_sink: Arc<dyn brassclaw_resources::BudgetEventSink>,
    in_memory_budget_event_sink: Arc<brassclaw_resources::InMemoryBudgetEventSink>,
    broadcast_budget_event_sink: Arc<brassclaw_resources::BroadcastBudgetEventSink>,
    budget_gate_store: Arc<dyn brassclaw_resources::BudgetGateStore>,
}

fn build_budget_sinks() -> BudgetSinks {
    let in_memory_budget_event_sink = Arc::new(brassclaw_resources::InMemoryBudgetEventSink::new());
    let broadcast_budget_event_sink =
        Arc::new(brassclaw_resources::BroadcastBudgetEventSink::default());
    let budget_event_sink: Arc<dyn brassclaw_resources::BudgetEventSink> =
        Arc::new(brassclaw_resources::CompositeBudgetEventSink::new(vec![
            Arc::clone(&in_memory_budget_event_sink)
                as Arc<dyn brassclaw_resources::BudgetEventSink>,
            Arc::clone(&broadcast_budget_event_sink)
                as Arc<dyn brassclaw_resources::BudgetEventSink>,
        ]));
    let budget_gate_store: Arc<dyn brassclaw_resources::BudgetGateStore> =
        Arc::new(brassclaw_resources::InMemoryBudgetGateStore::new());
    BudgetSinks {
        budget_event_sink,
        in_memory_budget_event_sink,
        broadcast_budget_event_sink,
        budget_gate_store,
    }
}

async fn build_local_dev_root_filesystem(
    root: &Path,
    workspace_root: &Path,
    host_home_root: Option<&LocalDevHostHomeRoot>,
) -> Result<LocalDevRootFilesystemBundle, RebornBuildError> {
    let local = Arc::new(local_dev_project_filesystem(
        root,
        workspace_root,
        host_home_root,
    )?);
    eprintln!(
        "brassclaw: local-dev: /memory is backed by InMemoryBackend; memory documents are ephemeral and will be lost on restart"
    );
    let mut composite = CompositeRootFilesystem::new();
    mount_local_dev_memory_root(&mut composite, Arc::new(InMemoryBackend::new()))?;
    // Mount an in-memory backend at /tenants so that per-tenant structured
    // records (conversations state, auth accounts, secrets, approvals, etc.)
    // have a backend when ScopedFilesystem resolves per-user aliases such as
    // /conversations → /tenants/<t>/users/<u>/conversations.  Without this,
    // local-dev builds that exercise auth flows or the trigger poller fail
    // with "no backend mount found for virtual path /tenants/…".
    mount_local_dev_tenant_root(&mut composite, Arc::new(InMemoryBackend::new()))?;
    mount_local_dev_project_roots(&mut composite, local)?;
    Ok(LocalDevRootFilesystemBundle {
        filesystem: Arc::new(composite),
    })
}

fn local_dev_project_filesystem(
    root: &Path,
    workspace_root: &Path,
    host_home_root: Option<&LocalDevHostHomeRoot>,
) -> Result<LocalFilesystem, RebornBuildError> {
    let mut filesystem = LocalFilesystem::new();
    filesystem.mount_local(
        VirtualPath::new("/projects")?,
        HostPath::from_path_buf(root.to_path_buf()),
    )?;
    filesystem.mount_local(
        VirtualPath::new("/projects/workspace")?,
        HostPath::from_path_buf(workspace_root.to_path_buf()),
    )?;
    filesystem.mount_local(
        VirtualPath::new("/system/extensions")?,
        HostPath::from_path_buf(root.join("system/extensions")),
    )?;
    if let Some(host_home_root) = host_home_root {
        filesystem.mount_local(
            VirtualPath::new("/projects/host")?,
            HostPath::from_path_buf(host_home_root.canonical_root.clone()),
        )?;
    }
    Ok(filesystem)
}

fn mount_local_dev_memory_root<F>(
    root: &mut CompositeRootFilesystem,
    backend: Arc<F>,
) -> Result<(), RebornBuildError>
where
    F: RootFilesystem + 'static,
{
    root.mount(
        local_dev_mount_descriptor(
            "/memory",
            "local-dev-memory",
            BackendKind::MemoryDocuments,
            StorageClass::StructuredRecords,
            ContentKind::MemoryDocument,
            IndexPolicy::FullTextAndVector,
            backend.capabilities(),
        )?,
        backend,
    )?;
    Ok(())
}

fn mount_local_dev_tenant_root<F>(
    root: &mut CompositeRootFilesystem,
    backend: Arc<F>,
) -> Result<(), RebornBuildError>
where
    F: RootFilesystem + 'static,
{
    root.mount(
        local_dev_mount_descriptor(
            "/tenants",
            "local-dev-tenants",
            BackendKind::Custom("in-memory".to_string()),
            StorageClass::StructuredRecords,
            ContentKind::StructuredRecord,
            IndexPolicy::NotIndexed,
            backend.capabilities(),
        )?,
        backend,
    )?;
    Ok(())
}

fn mount_local_dev_project_roots(
    root: &mut CompositeRootFilesystem,
    local: Arc<LocalFilesystem>,
) -> Result<(), RebornBuildError> {
    root.mount(
        local_dev_mount_descriptor(
            "/projects",
            "local-dev-project-files",
            BackendKind::LocalFilesystem,
            StorageClass::FileContent,
            ContentKind::ProjectFile,
            IndexPolicy::NotIndexed,
            BackendCapabilities::bytes_only(),
        )?,
        Arc::clone(&local),
    )?;
    root.mount(
        local_dev_mount_descriptor(
            "/system/extensions",
            "local-dev-system-extensions",
            BackendKind::LocalFilesystem,
            StorageClass::FileContent,
            ContentKind::ExtensionPackage,
            IndexPolicy::NotIndexed,
            BackendCapabilities::bytes_only(),
        )?,
        local,
    )?;
    Ok(())
}

fn build_local_dev_secret_store<F>(
    root: &Path,
    scoped_filesystem: Arc<ScopedFilesystem<F>>,
) -> Result<Arc<FilesystemSecretStore<F>>, RebornBuildError>
where
    F: RootFilesystem + 'static,
{
    let master_key = resolve_local_dev_secret_master_key(root)?;
    let crypto = Arc::new(brassclaw_secrets::SecretsCrypto::new(master_key)?);
    Ok(Arc::new(FilesystemSecretStore::new(
        scoped_filesystem,
        crypto,
    )))
}

fn resolve_local_dev_secret_master_key(
    root: &Path,
) -> Result<brassclaw_secrets::SecretMaterial, RebornBuildError> {
    let key_path = root.join(LOCAL_DEV_SECRETS_MASTER_KEY_PATH);
    match std::fs::read_to_string(&key_path) {
        Ok(existing) => {
            return Ok(brassclaw_secrets::SecretMaterial::from(
                existing.trim().to_string(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(RebornBuildError::InvalidConfig {
                reason: format!("local-dev secrets master key could not be read: {error}"),
            });
        }
    }

    let key = std::env::var(brassclaw_secrets::keychain::SECRETS_MASTER_KEY_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(brassclaw_secrets::keychain::generate_master_key_hex);
    write_local_dev_secret_master_key(&key_path, &key)?;
    Ok(brassclaw_secrets::SecretMaterial::from(key))
}

fn write_local_dev_secret_master_key(path: &Path, key: &str) -> Result<(), RebornBuildError> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("local-dev secrets master key could not be created: {error}"),
            })?;
        file.write_all(key.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("local-dev secrets master key could not be written: {error}"),
            })
    }
    #[cfg(windows)]
    {
        use std::io::Write as _;

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("local-dev secrets master key could not be created: {error}"),
            })?;
        let account = std::env::var("USERDOMAIN")
            .ok()
            .filter(|domain| !domain.trim().is_empty())
            .zip(
                std::env::var("USERNAME")
                    .ok()
                    .filter(|user| !user.trim().is_empty()),
            )
            .map(|(domain, user)| format!("{domain}\\{user}"))
            .or_else(|| std::env::var("USERNAME").ok())
            .ok_or_else(|| RebornBuildError::InvalidConfig {
                reason: "local-dev secrets master key could not be restricted: USERNAME is unset"
                    .to_string(),
            })?;
        let status = std::process::Command::new("icacls")
            .arg(path)
            .arg("/inheritance:r")
            .arg("/grant:r")
            .arg(format!("{account}:F"))
            .status()
            .map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!(
                    "local-dev secrets master key permissions could not be set: {error}"
                ),
            })?;
        if !status.success() {
            if let Err(rm_err) = std::fs::remove_file(path) {
                tracing::debug!(error = %rm_err, "local-dev: cleanup of partial key file failed after icacls error");
            }
            return Err(RebornBuildError::InvalidConfig {
                reason: format!(
                    "local-dev secrets master key permissions could not be set: icacls exited with {status}"
                ),
            });
        }
        file.write_all(key.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("local-dev secrets master key could not be written: {error}"),
            })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        let _ = key;
        Err(RebornBuildError::InvalidConfig {
            reason:
                "local-dev filesystem secret persistence requires Unix permissions or Windows ACLs"
                    .to_string(),
        })
    }
}

fn local_dev_mount_descriptor(
    virtual_root: &str,
    backend_id: &str,
    backend_kind: BackendKind,
    storage_class: StorageClass,
    content_kind: ContentKind,
    index_policy: IndexPolicy,
    capabilities: BackendCapabilities,
) -> Result<MountDescriptor, RebornBuildError> {
    Ok(MountDescriptor {
        virtual_root: VirtualPath::new(virtual_root)?,
        backend_id: BackendId::new(backend_id)?,
        backend_kind,
        storage_class,
        content_kind,
        index_policy,
        capabilities,
    })
}

fn local_dev_scoped_filesystem(
    filesystem: Arc<LocalDevRootFilesystem>,
) -> Arc<ScopedFilesystem<LocalDevRootFilesystem>> {
    crate::wrap_scoped(filesystem)
}

fn local_dev_event_log(
    _filesystem: Arc<LocalDevRootFilesystem>,
) -> Result<Arc<dyn DurableEventLog>, RebornBuildError> {
    Ok(Arc::new(InMemoryDurableEventLog::new()))
}

fn local_dev_audit_log(
    _filesystem: Arc<LocalDevRootFilesystem>,
) -> Result<Arc<dyn DurableAuditLog>, RebornBuildError> {
    Ok(Arc::new(InMemoryDurableAuditLog::new()))
}

fn canonicalize_local_dev_path(path: &Path, label: &str) -> Result<PathBuf, RebornBuildError> {
    std::fs::canonicalize(path).map_err(|_| RebornBuildError::InvalidConfig {
        reason: format!("local-dev {label} could not be resolved"),
    })
}

struct LocalDevHostHomeRoot {
    canonical_root: PathBuf,
    raw_alias: PathBuf,
}

impl LocalDevHostHomeRoot {
    fn aliases(&self) -> Vec<&Path> {
        vec![self.raw_alias.as_path(), self.canonical_root.as_path()]
    }
}

/// Build the two ScopedFilesystem views used by local-dev: a read-only workspace view
/// for skill context, and a read-write workspace view for runtime operations.
///
/// When `host_home_root` is present, the runtime view is the local-dev-yolo
/// ambient coding-tool view: it grants raw workspace and host-home aliases so
/// real local paths resolve through the same virtual roots as `/workspace` and
/// `/host`.
fn build_workspace_filesystems(
    filesystem: Arc<LocalDevRootFilesystem>,
    workspace_root: &Path,
    host_home_root: Option<&LocalDevHostHomeRoot>,
) -> Result<LocalDevWorkspaceFilesystems, RebornBuildError> {
    let read_only_workspace_mounts = workspace_mount_view(MountPermissions::read_only(), &[])
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: error.to_string(),
        })?;
    let host_home_aliases = host_home_root
        .map(|root| root.aliases())
        .unwrap_or_default();
    let workspace_aliases = if host_home_root.is_some() {
        vec![workspace_root]
    } else {
        Vec::new()
    };
    let runtime_workspace_mounts = ambient_workspace_mount_view(
        MountPermissions::read_write(),
        &workspace_aliases,
        &host_home_aliases,
    )
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: error.to_string(),
    })?;
    let skill_filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::clone(&filesystem),
        skill_context_mount_view().map_err(|error| RebornBuildError::InvalidConfig {
            reason: error.to_string(),
        })?,
    ));
    let workspace_filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
        filesystem,
        read_only_workspace_mounts,
    ));
    Ok((
        skill_filesystem,
        workspace_filesystem,
        runtime_workspace_mounts,
    ))
}

fn canonicalize_local_dev_existing_dir(
    path: &Path,
    label: &str,
) -> Result<PathBuf, RebornBuildError> {
    let path = canonicalize_local_dev_path(path, label)?;
    let metadata = std::fs::metadata(&path).map_err(|_| RebornBuildError::InvalidConfig {
        reason: format!("local-dev {label} could not be inspected"),
    })?;
    if metadata.is_dir() {
        Ok(path)
    } else {
        Err(RebornBuildError::InvalidConfig {
            reason: format!("local-dev {label} must be an existing directory"),
        })
    }
}

fn canonicalize_local_dev_host_home_root(path: &Path) -> Result<PathBuf, RebornBuildError> {
    let path = canonicalize_local_dev_existing_dir(path, "host home root")?;
    if path.parent().is_none() {
        return Err(RebornBuildError::InvalidConfig {
            reason: "local-dev host home root must not be a filesystem root".to_string(),
        });
    }
    Ok(path)
}

fn validate_local_dev_workspace_skill_isolation(
    storage_root: &Path,
    workspace_root: &Path,
) -> Result<(), RebornBuildError> {
    for (label, skill_root) in [
        ("/skills", storage_root.join("skills")),
        (
            "/tenant-shared/skills",
            storage_root.join("tenant-shared/skills"),
        ),
        ("/system/skills", storage_root.join("system/skills")),
        ("/system/extensions", storage_root.join("system/extensions")),
    ] {
        if paths_overlap(workspace_root, &skill_root) {
            return Err(RebornBuildError::InvalidConfig {
                reason: format!(
                    "local-dev workspace root must not overlap default skill root {label}"
                ),
            });
        }
    }
    Ok(())
}

fn local_dev_default_system_prompt_path(storage_root: &Path) -> PathBuf {
    storage_root.join(LOCAL_DEV_DEFAULT_SYSTEM_PROMPT_PATH)
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

pub(crate) fn builtin_extension_registry() -> Result<ExtensionRegistry, RebornBuildError> {
    // Shared by local-dev and production composition so host-owned first-party
    // capabilities expose the same built-in package contract in both profiles.
    let mut registry = ExtensionRegistry::new();
    registry
        .insert(
            builtin_first_party_package().map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("built-in first-party package is invalid: {error}"),
            })?,
        )
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("built-in first-party registry is invalid: {error}"),
        })?;
    Ok(registry)
}

fn builtin_first_party_registry_with_trigger_create_hook(
    trigger_repository: Arc<dyn TriggerRepository>,
    trigger_create_hook: Arc<dyn TriggerCreateHook>,
) -> Result<FirstPartyCapabilityRegistry, RebornBuildError> {
    builtin_first_party_handlers_with_trigger_create_hook(trigger_repository, trigger_create_hook)
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("built-in first-party handlers are invalid: {error}"),
        })
}

fn local_dev_builtin_extension_registry() -> Result<ExtensionRegistry, RebornBuildError> {
    let mut registry = builtin_extension_registry()?;
    let builtin_id =
        ExtensionId::new("builtin").map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("built-in first-party package id is invalid: {error}"),
        })?;
    let package = registry
        .remove(&builtin_id)
        .ok_or_else(|| RebornBuildError::InvalidConfig {
            reason: "built-in first-party package is missing".to_string(),
        })?;
    let package = extend_builtin_first_party_package(package).map_err(|error| {
        RebornBuildError::InvalidConfig {
            reason: format!("local-dev extension lifecycle package is invalid: {error}"),
        }
    })?;
    registry
        .insert(package)
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("local-dev built-in first-party registry is invalid: {error}"),
        })?;
    Ok(registry)
}

fn local_dev_first_party_trust_policy() -> Result<HostTrustPolicy, RebornBuildError> {
    let policy =
        local_dev_capability_policy().map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("local-dev capability policy is invalid: {error}"),
        })?;
    HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            policy.provider.id,
            policy.provider.manifest_path,
            None,
            HostTrustAssignment::first_party(),
            policy.provider.authority_effects,
            None,
        ),
        AdminEntry::for_local_manifest(
            PackageId::new("web-access").map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("Web Access first-party package id is invalid: {error}"),
            })?,
            "/system/extensions/web-access/manifest.toml".to_string(),
            Some(web_access_manifest_digest()),
            HostTrustAssignment::first_party(),
            web_access_allowed_effects(),
            None,
        ),
        AdminEntry::for_local_manifest(
            PackageId::new("google-calendar").map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("Google Calendar first-party package id is invalid: {error}"),
            })?,
            "/system/extensions/google-calendar/manifest.toml".to_string(),
            Some(google_calendar_manifest_digest()),
            HostTrustAssignment::first_party(),
            gsuite_allowed_effects(),
            None,
        ),
        AdminEntry::for_local_manifest(
            PackageId::new("gmail").map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("Gmail first-party package id is invalid: {error}"),
            })?,
            "/system/extensions/gmail/manifest.toml".to_string(),
            Some(gmail_manifest_digest()),
            HostTrustAssignment::first_party(),
            gsuite_allowed_effects(),
            None,
        ),
        AdminEntry::for_local_manifest(
            PackageId::new("notion").map_err(|error| RebornBuildError::InvalidConfig {
                reason: format!("Notion MCP first-party package id is invalid: {error}"),
            })?,
            "/system/extensions/notion/manifest.toml".to_string(),
            Some(notion_mcp_manifest_digest()),
            HostTrustAssignment::first_party(),
            notion_mcp_allowed_effects(),
            None,
        ),
    ]))])
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("built-in first-party trust policy is invalid: {error}"),
    })
}

fn gsuite_allowed_effects() -> Vec<EffectKind> {
    vec![
        EffectKind::DispatchCapability,
        EffectKind::Network,
        EffectKind::UseSecret,
        EffectKind::ExternalWrite,
    ]
}

fn web_access_allowed_effects() -> Vec<EffectKind> {
    vec![EffectKind::DispatchCapability, EffectKind::Network]
}

fn notion_mcp_allowed_effects() -> Vec<EffectKind> {
    vec![
        EffectKind::DispatchCapability,
        EffectKind::Network,
        EffectKind::UseSecret,
        EffectKind::ExternalWrite,
    ]
}

#[cfg(test)]
fn nearai_allowed_effects() -> Vec<EffectKind> {
    vec![
        EffectKind::DispatchCapability,
        EffectKind::Network,
        EffectKind::UseSecret,
    ]
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
async fn build_production_shaped(
    input: RebornBuildInput,
) -> Result<RebornServices, RebornBuildError> {
    let RebornBuildInput {
        profile,
        owner_id: _,
        storage,
        production_trust_policy,
        runtime_policy,
        turn_run_wake_notifier,
        runtime_process_binding,
        required_runtime_backends,
        require_runtime_http_egress,
        product_auth_ports,
        oauth_provider_configs,
        oauth_dcr_provider_configs,
    } = input;
    let wiring_config = production_config(required_runtime_backends, require_runtime_http_egress);

    match storage {
        RebornStorageInput::Disabled | RebornStorageInput::LocalDev { .. } => {
            Err(RebornBuildError::InvalidConfig {
                reason: format!(
                    "profile={} requires durable database-backed Reborn storage",
                    profile
                ),
            })
        }
        #[cfg(feature = "postgres")]
        RebornStorageInput::Postgres {
            pool,
            url,
            secret_master_key,
            reborn_home,
        } => {
            let production_wiring = production_wiring(
                production_trust_policy,
                runtime_policy,
                turn_run_wake_notifier,
                runtime_process_binding,
            )?;
            let secret_master_key =
                resolve_secret_master_key(secret_master_key, &pool, &reborn_home).await?;
            let context = RebornProductionBuildContext {
                profile,
                wiring_config,
                production_wiring,
                product_auth_ports,
                oauth_provider_configs,
                oauth_dcr_provider_configs,
            };
            build_postgres_production(context, pool, url, secret_master_key, reborn_home).await
        }
    }
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
async fn resolve_secret_master_key(
    explicit: Option<brassclaw_secrets::SecretMaterial>,
    pool: &deadpool_postgres::Pool,
    reborn_home: &std::path::Path,
) -> Result<brassclaw_secrets::SecretMaterial, RebornBuildError> {
    // If an explicit key was pre-resolved (e.g. upgrade path with env var or
    // keychain), use it directly — skip the DB table lookup.
    if let Some(key) = explicit {
        return Ok(key);
    }

    // Per-boot ceremony: read the master key from brassclaw_secrets_master.
    let pg_pool = pool.clone();
    match crate::secrets_master::resolve_pg_master_key(&pg_pool, "default", reborn_home)
        .await
        .map_err(|e| RebornBuildError::InvalidConfig {
            reason: e.to_string(),
        })? {
        crate::secrets_master::ResolvedMasterKey::Key(key) => Ok(key),
        crate::secrets_master::ResolvedMasterKey::NotYetInitialized => {
            // Fresh install — fall back to keychain/env for first-run wizard
            // boot (the wizard will populate brassclaw_secrets_master later).
            resolve_explicit_or_keychain_master_key(None)
                .await?
                .ok_or(RebornBuildError::MissingSecretMasterKey)
        }
    }
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
struct RebornProductionWiring {
    trust_policy: Arc<HostTrustPolicy>,
    runtime_policy: EffectiveRuntimePolicy,
    turn_run_wake_notifier: Arc<brassclaw_host_runtime::SchedulerTurnRunWakeNotifier>,
    runtime_process_binding: RebornRuntimeProcessBinding,
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
struct RebornProductionBuildContext {
    profile: RebornCompositionProfile,
    wiring_config: brassclaw_host_runtime::ProductionWiringConfig,
    production_wiring: RebornProductionWiring,
    product_auth_ports: Option<RebornProductAuthServicePorts>,
    oauth_provider_configs: Vec<crate::input::OAuthProviderBackendConfig>,
    oauth_dcr_provider_configs: Vec<crate::input::OAuthDcrProviderBackendConfig>,
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
fn production_wiring(
    trust_policy: Option<Arc<HostTrustPolicy>>,
    runtime_policy: Option<EffectiveRuntimePolicy>,
    turn_run_wake_notifier: Option<Arc<brassclaw_host_runtime::SchedulerTurnRunWakeNotifier>>,
    runtime_process_binding: RebornRuntimeProcessBinding,
) -> Result<RebornProductionWiring, RebornBuildError> {
    let trust_policy = trust_policy.ok_or(RebornBuildError::MissingProductionTrustPolicy)?;
    if !trust_policy.has_sources() {
        return Err(RebornBuildError::EmptyProductionTrustPolicy);
    }
    let runtime_policy = runtime_policy.ok_or(RebornBuildError::MissingRuntimePolicy)?;
    validate_production_process_binding(&runtime_policy, &runtime_process_binding)?;
    let turn_run_wake_notifier =
        turn_run_wake_notifier.ok_or(RebornBuildError::MissingTurnRunWakeNotifier)?;
    Ok(RebornProductionWiring {
        trust_policy,
        runtime_policy,
        turn_run_wake_notifier,
        runtime_process_binding,
    })
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
fn validate_production_process_binding(
    runtime_policy: &EffectiveRuntimePolicy,
    binding: &RebornRuntimeProcessBinding,
) -> Result<(), RebornBuildError> {
    binding
        .validate_for_production_policy(runtime_policy)
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: error.to_string(),
        })
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
fn planned_run_profile_resolver() -> Result<Arc<InMemoryRunProfileResolver>, RebornBuildError> {
    Ok(Arc::new(
        brassclaw_reborn::planned_driver_factory::default_planned_run_profile_resolver().map_err(
            |error| RebornBuildError::PlannedRunProfileResolver {
                reason: error.to_string(),
            },
        )?,
    ))
}

#[cfg(feature = "postgres")]
type FilesystemProductionHostRuntimeServices<F> = HostRuntimeServices<
    F,
    PersistentResourceGovernor<FilesystemResourceGovernorStore<F>>,
    brassclaw_processes::FilesystemProcessStore<F>,
    brassclaw_processes::FilesystemProcessResultStore<F>,
>;

#[cfg(feature = "postgres")]
pub(crate) async fn build_postgres_production_host_runtime_services<TPolicy, TWake>(
    config: crate::PostgresProductionSubstrateConfig<TPolicy, TWake>,
) -> Result<crate::PostgresProductionHostRuntimeServices, crate::RebornCompositionError>
where
    TPolicy: brassclaw_trust::TrustPolicy + 'static,
    TWake: brassclaw_turns::TurnRunWakeNotifier + 'static,
{
    let filesystem = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        config.pool,
    ));
    filesystem.run_migrations().await?;
    build_filesystem_production_host_runtime_services(
        filesystem,
        config.event_store,
        config.secret_master_key,
        config.trust_policy,
        config.runtime_policy,
        config.turn_run_wake_notifier,
        config.surface_version,
    )
    .await
}

#[cfg(feature = "postgres")]
async fn build_filesystem_production_host_runtime_services<F, TPolicy, TWake>(
    filesystem: Arc<F>,
    event_store: brassclaw_reborn_event_store::RebornEventStoreConfig,
    secret_master_key: Option<brassclaw_secrets::SecretMaterial>,
    trust_policy: Arc<TPolicy>,
    runtime_policy: crate::RebornProductionRuntimePolicy,
    turn_run_wake_notifier: Arc<TWake>,
    surface_version: CapabilitySurfaceVersion,
) -> Result<FilesystemProductionHostRuntimeServices<F>, crate::RebornCompositionError>
where
    F: RootFilesystem + 'static,
    TPolicy: brassclaw_trust::TrustPolicy + 'static,
    TWake: brassclaw_turns::TurnRunWakeNotifier + 'static,
{
    let scoped_filesystem = crate::wrap_scoped(Arc::clone(&filesystem));
    let process_services = ProcessServices::filesystem(Arc::clone(&scoped_filesystem));
    let secret_credentials = build_filesystem_secret_credential_stores(
        Arc::clone(&scoped_filesystem),
        secret_master_key,
    )
    .await?;
    let resource_store = FilesystemResourceGovernorStore::new(Arc::clone(&scoped_filesystem));
    let governor = Arc::new(PersistentResourceGovernor::new(resource_store));
    let capability_leases = Arc::new(FilesystemCapabilityLeaseStore::new(Arc::clone(
        &scoped_filesystem,
    )));
    let (runtime_policy, process_binding) = runtime_policy.into_parts();

    let services = HostRuntimeServices::new(
        Arc::new(ExtensionRegistry::new()),
        filesystem,
        governor,
        Arc::new(GrantAuthorizer::new()),
        process_services,
        surface_version,
    )
    .with_trust_policy(trust_policy)
    .with_runtime_policy(runtime_policy)
    .with_capability_leases(capability_leases)
    .with_security_audit_sink(Arc::new(brassclaw_events::TracingSecurityAuditSink))
    .with_secret_store(Arc::clone(&secret_credentials.secret_store))
    .with_credential_broker(secret_credentials.credential_broker)
    .with_turn_run_wake_notifier(turn_run_wake_notifier)
    .with_filesystem_run_state(Arc::clone(&scoped_filesystem))
    .with_filesystem_turn_state_store(Arc::clone(&scoped_filesystem))
    .with_run_profile_resolver(Arc::new(
        brassclaw_reborn::planned_driver_factory::default_planned_run_profile_resolver()?,
    ))
    .with_reborn_event_store_config(
        brassclaw_reborn_event_store::RebornProfile::Production,
        event_store,
    )
    .await?;
    let services = apply_production_runtime_process_binding(services, process_binding);

    let services = services
        .try_with_host_http_egress_with_body_store(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
            Arc::clone(&scoped_filesystem),
        )
        .map_err(crate::RebornCompositionError::from)?;

    Ok(services)
}

/// Central production secret/credential stores over the shared
/// [`ScopedFilesystem`].
///
/// Backend selection is now a property of the underlying
/// [`RootFilesystem`] (libSQL/Postgres/in-memory), not of each store itself.
/// The secret store and credential broker are deliberately built together from
/// one scoped filesystem and one crypto handle so production composition does
/// not grow parallel ad hoc secret/credential stores.
#[cfg(feature = "postgres")]
struct FilesystemSecretCredentialStores<F>
where
    F: RootFilesystem + 'static,
{
    secret_store: Arc<FilesystemSecretStore<F>>,
    credential_broker: Arc<FilesystemCredentialBroker<F>>,
}

#[cfg(feature = "postgres")]
impl<F> FilesystemSecretCredentialStores<F>
where
    F: RootFilesystem + 'static,
{
    fn new(
        scoped_filesystem: Arc<ScopedFilesystem<F>>,
        crypto: Arc<brassclaw_secrets::SecretsCrypto>,
    ) -> Self {
        Self {
            secret_store: Arc::new(FilesystemSecretStore::new(
                Arc::clone(&scoped_filesystem),
                Arc::clone(&crypto),
            )),
            credential_broker: Arc::new(FilesystemCredentialBroker::new(scoped_filesystem, crypto)),
        }
    }

    fn from_master_key(
        scoped_filesystem: Arc<ScopedFilesystem<F>>,
        master_key: brassclaw_secrets::SecretMaterial,
    ) -> Result<Self, crate::RebornCompositionError> {
        Ok(Self::new(
            scoped_filesystem,
            Arc::new(brassclaw_secrets::SecretsCrypto::new(master_key)?),
        ))
    }
}

/// Postgres-native secret and credential stores bundled for [`ProductionStoreBundle`].
///
/// [`PgSecretStore`] and [`PgCredentialBroker`] write encrypted rows directly
/// to `brassclaw_secrets`, replacing the legacy VFS blob columns.
#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
struct ProductionCredentialBundle {
    secret_store: Arc<PgSecretStore>,
    credential_broker: Arc<PgCredentialBroker>,
}

#[cfg(feature = "postgres")]
async fn build_filesystem_secret_credential_stores<F>(
    scoped_filesystem: Arc<ScopedFilesystem<F>>,
    master_key: Option<brassclaw_secrets::SecretMaterial>,
) -> Result<FilesystemSecretCredentialStores<F>, crate::RebornCompositionError>
where
    F: RootFilesystem + 'static,
{
    let master_key = resolve_explicit_or_keychain_master_key(master_key)
        .await?
        .ok_or(crate::RebornCompositionError::MissingSecretMasterKey)?;
    FilesystemSecretCredentialStores::from_master_key(scoped_filesystem, master_key)
}

#[cfg(feature = "postgres")]
async fn resolve_explicit_or_keychain_master_key(
    explicit: Option<brassclaw_secrets::SecretMaterial>,
) -> Result<Option<brassclaw_secrets::SecretMaterial>, brassclaw_secrets::SecretError> {
    if let Some(master_key) = explicit {
        Ok(Some(master_key))
    } else if let Some(master_key) =
        brassclaw_secrets::keychain::resolve_master_key_material().await?
    {
        Ok(Some(master_key))
    } else {
        Ok(None)
    }
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
struct ProductionStoreBundle<F>
where
    F: RootFilesystem + 'static,
{
    filesystem: Arc<F>,
    scoped_filesystem: Arc<ScopedFilesystem<F>>,
    leases: Arc<FilesystemCapabilityLeaseStore<F>>,
    secret_credentials: ProductionCredentialBundle,
    event_store: brassclaw_reborn_event_store::RebornEventStoreConfig,
}

#[cfg(feature = "postgres")]
impl<F> ProductionStoreBundle<F>
where
    F: RootFilesystem + 'static,
{
    /// Build a bundle backed by the Postgres secret stores.
    ///
    /// `PgSecretStore` and `PgCredentialBroker` write encrypted rows directly
    /// into `brassclaw_secrets`; this replaces the old VFS-backed stores that
    /// used the `PostgresRootFilesystem` blob columns.
    fn new_postgres(
        filesystem: Arc<F>,
        pg_pool: deadpool_postgres::Pool,
        secret_master_key: brassclaw_secrets::SecretMaterial,
        event_store: brassclaw_reborn_event_store::RebornEventStoreConfig,
    ) -> Result<Self, RebornBuildError> {
        let scoped_filesystem = crate::wrap_scoped(Arc::clone(&filesystem));
        // Filesystem capability lease store is kept here for the bundle; the
        // PG-specific build path (build_pg_backend_production_with_tools) will
        // override it with PgCapabilityLeaseStore via with_capability_leases.
        let leases = Arc::new(FilesystemCapabilityLeaseStore::new(Arc::clone(
            &scoped_filesystem,
        )));
        let secret_store = Arc::new(
            PgSecretStore::new(pg_pool.clone(), secret_master_key.clone(), "default").map_err(
                |e| RebornBuildError::InvalidConfig {
                    reason: format!("PgSecretStore init failed: {e}"),
                },
            )?,
        );
        let credential_broker = Arc::new(
            PgCredentialBroker::new(pg_pool, secret_master_key, "default").map_err(|e| {
                RebornBuildError::InvalidConfig {
                    reason: format!("PgCredentialBroker init failed: {e}"),
                }
            })?,
        );
        Ok(Self {
            filesystem,
            scoped_filesystem,
            leases,
            secret_credentials: ProductionCredentialBundle {
                secret_store,
                credential_broker,
            },
            event_store,
        })
    }
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
async fn build_backend_production_with_tools<F>(
    context: RebornProductionBuildContext,
    stores: ProductionStoreBundle<F>,
    trigger_repository: Arc<dyn TriggerRepository>,
    prebuilt_tools: Option<BuiltinFirstPartyTools>,
    pg_pool: Option<Arc<deadpool_postgres::Pool>>,
) -> Result<RebornServices, RebornBuildError>
where
    F: RootFilesystem + 'static,
{
    let RebornProductionBuildContext {
        profile,
        wiring_config,
        production_wiring,
        product_auth_ports,
        oauth_provider_configs,
        oauth_dcr_provider_configs,
    } = context;
    // Destructure stores up front to move fields individually.
    let ProductionStoreBundle {
        filesystem: stores_filesystem,
        scoped_filesystem: stores_scoped_fs,
        leases: stores_leases,
        secret_credentials,
        event_store: stores_event_store,
    } = stores;

    // PgSecretStore implements SecretStore; coerce to trait object for the
    // provider composition and product-auth wiring that needs Arc<dyn SecretStore>.
    let secret_store: Arc<dyn SecretStore> = secret_credentials.secret_store.clone();
    let trigger_create_hook = Arc::new(ScopedFilesystemTriggerCreatorPairingHook::new(Arc::clone(
        &stores_scoped_fs,
    )));
    let mut first_party_registry = match prebuilt_tools {
        Some(tools) => builtin_first_party_handlers_from_tools_with_trigger(
            tools,
            trigger_repository,
            trigger_create_hook,
        )
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("built-in first-party handlers are invalid: {error}"),
        })?,
        None => builtin_first_party_registry_with_trigger_create_hook(
            trigger_repository,
            trigger_create_hook,
        )?,
    };
    let product_auth_filesystem = Arc::clone(&stores_scoped_fs);
    // Wire the Postgres-native secret store and credential broker so all secret
    // and OAuth credential writes go to brassclaw_secrets (§4.4 Issue 3).
    let services = HostRuntimeServices::new(
        Arc::new(builtin_extension_registry()?),
        Arc::clone(&stores_filesystem),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::filesystem(Arc::clone(&stores_scoped_fs)),
        CapabilitySurfaceVersion::new("reborn-app-v1")?,
    )
    .with_trust_policy(production_wiring.trust_policy)
    .with_runtime_policy(production_wiring.runtime_policy)
    .with_capability_leases(stores_leases)
    .with_secret_store_dyn(Arc::clone(&secret_store))
    .with_credential_broker(secret_credentials.credential_broker);
    let services = services
        .with_security_audit_sink(Arc::new(brassclaw_events::TracingSecurityAuditSink))
        .try_with_host_http_egress_with_body_store(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
            Arc::clone(&stores_scoped_fs),
        )?
        .with_filesystem_resource_governor(Arc::clone(&stores_scoped_fs))
        .with_reborn_event_store_config(profile.to_event_store_profile(), stores_event_store)
        .await?
        .with_filesystem_run_state(Arc::clone(&stores_scoped_fs))
        .with_filesystem_turn_state_store(Arc::clone(&stores_scoped_fs))
        .with_run_profile_resolver(planned_run_profile_resolver()?)
        .with_turn_run_wake_notifier(production_wiring.turn_run_wake_notifier);
    let product_auth_runtime_ports = require_product_auth_runtime_ports(&services)?;
    let services = attach_hosted_mcp_runtime(services)?;
    let provider_composition = compose_provider_client(
        oauth_provider_configs,
        oauth_dcr_provider_configs,
        Arc::clone(&secret_store),
        product_auth_runtime_ports.clone(),
    )?;
    let services = apply_production_runtime_process_binding(
        services,
        production_wiring.runtime_process_binding,
    );

    let turn_coordinator: Arc<dyn brassclaw_turns::TurnCoordinator> =
        Arc::new(services.turn_coordinator_for_production()?);
    let product_auth_ports = product_auth_ports.unwrap_or_else(|| {
        #[cfg(feature = "postgres")]
        if let Some(ref pool) = pg_pool {
            let durable = Arc::new(PgAuthProductServices::new(
                Arc::clone(pool),
                Arc::clone(&secret_store),
            ));
            return RebornProductAuthServicePorts::from_shared_with_provider(
                durable,
                provider_composition
                    .client
                    .clone()
                    .unwrap_or_else(|| Arc::new(UnavailableAuthProviderClient)),
            );
        }
        let durable = Arc::new(FilesystemAuthProductServices::new(
            product_auth_filesystem,
            Arc::clone(&secret_store),
        ));
        RebornProductAuthServicePorts::from_shared_with_provider(
            durable,
            provider_composition
                .client
                .clone()
                .unwrap_or_else(|| Arc::new(UnavailableAuthProviderClient)),
        )
    });
    let product_auth_services = compose_product_auth_services(
        product_auth_ports,
        turn_coordinator.clone(),
        provider_composition,
    );
    let product_auth_ready = true;
    // Wire ProductAuthAccount runtime credential resolver before
    // host_runtime_for_production so WASM extensions whose manifest declares a
    // ProductAuthAccount runtime credential source resolve through
    // CredentialAccountService. Unconditional in production: product_auth_services
    // always exists (durable filesystem fallback from #4234).
    let services = services.with_runtime_credential_account_resolver(Arc::new(
        ProductAuthRuntimeCredentialResolver::new(
            product_auth_services.runtime_credential_account_selection_service(),
        ),
    ));
    register_bundled_gsuite_first_party_handlers(
        &mut first_party_registry,
        product_auth_services.credential_account_service(),
        product_auth_services.credential_account_record_source(),
        Arc::new(ProductAuthRuntimeGsuiteCredentialStager::new(
            product_auth_runtime_ports.clone(),
        )),
    )
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("GSuite first-party handlers are invalid: {error}"),
    })?;
    let services = services.with_first_party_capabilities(Arc::new(first_party_registry));

    let host_runtime: Arc<dyn brassclaw_host_runtime::HostRuntime> =
        Arc::new(services.host_runtime_for_production(&wiring_config)?);

    // Build the three Postgres-backed WebUI stores from the shared pool.
    // tenant_id "default" matches the hardcoded tenant used by the embedded-PG
    // production path everywhere else in this composition (see build_postgres_memory_tools).
    #[cfg(feature = "postgres")]
    let (pg_safety_config_store, pg_token_settings_store, pg_memory_doc_store) =
        if let Some(ref pool) = pg_pool {
            (
                Some(Arc::new(
                    brassclaw_product_workflow::PgSafetyConfigStore::new(
                        Arc::clone(pool),
                        "default",
                    ),
                )),
                Some(Arc::new(
                    crate::pg_token_settings_store::PgTokenSettingsStore::new(
                        Arc::clone(pool),
                        "default",
                    ),
                )),
                Some(Arc::new(crate::pg_memory_doc_store::PgMemoryDocStore::new(
                    Arc::clone(pool),
                    "default",
                ))),
            )
        } else {
            (None, None, None)
        };

    Ok(RebornServices {
        host_runtime: Some(host_runtime),
        turn_coordinator: Some(turn_coordinator),
        readiness: readiness_for(profile, true, true, product_auth_ready),
        product_auth: Some(product_auth_services),
        local_runtime: None,
        #[cfg(feature = "postgres")]
        pg_pool,
        #[cfg(feature = "root-llm-provider")]
        secret_store,
        #[cfg(feature = "postgres")]
        pg_safety_config_store,
        #[cfg(feature = "postgres")]
        pg_token_settings_store,
        #[cfg(feature = "postgres")]
        pg_memory_doc_store,
    })
}

#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
async fn build_postgres_production(
    context: RebornProductionBuildContext,
    pool: deadpool_postgres::Pool,
    // `_url` was previously forwarded to `RebornEventStoreConfig::Postgres { url }` which
    // opened a second independent pool.  PG-4 plan requirement: event stores must reuse the
    // shared pool.  The URL is no longer needed here; kept as a parameter to avoid breaking
    // the call site at line ~1683 until the caller is updated in PG-8.
    _url: brassclaw_secrets::SecretMaterial,
    secret_master_key: brassclaw_secrets::SecretMaterial,
    _reborn_home: std::path::PathBuf,
) -> Result<RebornServices, RebornBuildError> {
    use brassclaw_filesystem::PostgresRootFilesystem;

    let filesystem = Arc::new(PostgresRootFilesystem::new(pool.clone()));
    filesystem.run_migrations().await?;
    // brassclaw_pg::run_migrations() (called before this function) already
    // applies V021 (triggers DDL) — no separate trigger-repository migration needed.
    let trigger_repository = Arc::new(brassclaw_triggers::PostgresTriggerRepository::new(
        pool.clone(),
    ));
    // Build stores with PgSecretStore + PgCredentialBroker so that all secret
    // and OAuth credential writes go to the `brassclaw_secrets` table rather
    // than the legacy VFS blob columns (§4.4 Issue 3).
    // PG-4: use SharedPool so event stores reuse the existing pool rather than
    // opening a second independent connection pool.
    let stores = ProductionStoreBundle::new_postgres(
        filesystem,
        pool.clone(),
        secret_master_key,
        brassclaw_reborn_event_store::RebornEventStoreConfig::SharedPool {
            pool: Arc::new(pool.clone()),
            tenant_id: "default".to_string(),
        },
    )?;

    // Clone pool before consuming it in build_postgres_memory_tools so it can
    // be threaded through to auth and hooks wiring. Pool drop must happen
    // before managed_pg.shutdown().await (Phase 6 note §4 item).
    let shared_pool = Arc::new(pool.clone());

    // Run product-auth schema migrations (CREATE TABLE IF NOT EXISTS — idempotent).
    crate::pg_auth_product_services::run_auth_migrations(&shared_pool)
        .await
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("product-auth PostgreSQL migrations failed: {error}"),
        })?;

    // S10: Resolve tenant_id and optional embedding provider; wire
    // PgInterceptorStore and PgChatMemoryRecordStore (Path A).
    let pg_tools = build_postgres_memory_tools(pool).await;

    build_pg_backend_production_with_tools(
        context,
        stores,
        trigger_repository,
        Some(pg_tools),
        Arc::clone(&shared_pool),
    )
    .await
}

/// Postgres-specific production backend builder.
///
/// Unlike [`build_backend_production_with_tools`] this function uses the Pg
/// store implementations for every store that has one: run-state, approvals,
/// turn-state, resource governor. It accepts a concrete (non-Option) pool
/// because the postgres production path always has a live pool.
#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
async fn build_pg_backend_production_with_tools<F>(
    context: RebornProductionBuildContext,
    stores: ProductionStoreBundle<F>,
    trigger_repository: Arc<dyn TriggerRepository>,
    prebuilt_tools: Option<BuiltinFirstPartyTools>,
    pg_pool: Arc<deadpool_postgres::Pool>,
) -> Result<RebornServices, RebornBuildError>
where
    F: RootFilesystem + 'static,
{
    let RebornProductionBuildContext {
        profile,
        wiring_config,
        production_wiring,
        product_auth_ports,
        oauth_provider_configs,
        oauth_dcr_provider_configs,
    } = context;
    // Destructure stores up front to move fields individually.
    // `stores_leases` (filesystem-backed) is intentionally dropped here — the
    // PG path constructs PgCapabilityLeaseStore directly from the shared pool.
    let ProductionStoreBundle {
        filesystem: stores_filesystem,
        scoped_filesystem: stores_scoped_fs,
        leases: _stores_leases,
        secret_credentials,
        event_store: stores_event_store,
    } = stores;
    // PG-4: Postgres-backed capability lease store — leases survive process
    // restart and are visible across concurrent processes sharing the DB.
    let pg_lease_store = Arc::new(PgCapabilityLeaseStore::new(
        Arc::clone(&pg_pool),
        "default",
    ));

    // PgSecretStore implements SecretStore; coerce to trait object for the
    // provider composition and product-auth wiring that needs Arc<dyn SecretStore>.
    let secret_store: Arc<dyn SecretStore> = secret_credentials.secret_store.clone();
    let trigger_create_hook = Arc::new(ScopedFilesystemTriggerCreatorPairingHook::new(Arc::clone(
        &stores_scoped_fs,
    )));
    let mut first_party_registry = match prebuilt_tools {
        Some(tools) => builtin_first_party_handlers_from_tools_with_trigger(
            tools,
            trigger_repository,
            trigger_create_hook,
        )
        .map_err(|error| RebornBuildError::InvalidConfig {
            reason: format!("built-in first-party handlers are invalid: {error}"),
        })?,
        None => builtin_first_party_registry_with_trigger_create_hook(
            trigger_repository,
            trigger_create_hook,
        )?,
    };
    // Wire the Postgres-native secret store and credential broker so all secret
    // and OAuth credential writes go to brassclaw_secrets (§4.4 Issue 3).
    // PG-4: resource governor is Postgres-backed for durable resource accounting.
    // PG-4: use Postgres-backed process store so process records survive restart.
    // Tenant "default" matches the scope used throughout the postgres production path.
    let services = HostRuntimeServices::new(
        Arc::new(builtin_extension_registry()?),
        Arc::clone(&stores_filesystem),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(GrantAuthorizer::new()),
        ProcessServices::postgres(Arc::clone(&pg_pool), "default"),
        CapabilitySurfaceVersion::new("reborn-app-v1")?,
    )
    .with_trust_policy(production_wiring.trust_policy)
    .with_runtime_policy(production_wiring.runtime_policy)
    // PG-4: Postgres-backed capability lease store.
    .with_capability_leases(pg_lease_store)
    .with_secret_store_dyn(Arc::clone(&secret_store))
    .with_credential_broker(secret_credentials.credential_broker);
    let services = services
        .with_security_audit_sink(Arc::new(brassclaw_events::TracingSecurityAuditSink))
        .try_with_host_http_egress_with_body_store(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
            Arc::clone(&stores_scoped_fs),
        )?
        // PG-4: replace InMemoryResourceGovernor with PgResourceGovernorStore.
        .with_pg_resource_governor(Arc::clone(&pg_pool))
        .with_reborn_event_store_config(profile.to_event_store_profile(), stores_event_store)
        .await?
        // PG-4: replace filesystem-backed run-state + approvals with PgRunStateStore
        //       and PgApprovalRequestStore.
        .with_pg_run_state(Arc::clone(&pg_pool))
        // PG-4: replace filesystem-backed turn-state with PgTurnStateStore.
        .with_pg_turn_state_store(Arc::clone(&pg_pool))
        .with_run_profile_resolver(planned_run_profile_resolver()?)
        .with_turn_run_wake_notifier(production_wiring.turn_run_wake_notifier);
    let product_auth_runtime_ports = require_product_auth_runtime_ports(&services)?;
    let services = attach_hosted_mcp_runtime(services)?;
    let provider_composition = compose_provider_client(
        oauth_provider_configs,
        oauth_dcr_provider_configs,
        Arc::clone(&secret_store),
        product_auth_runtime_ports.clone(),
    )?;
    let services = apply_production_runtime_process_binding(
        services,
        production_wiring.runtime_process_binding,
    );

    let turn_coordinator: Arc<dyn brassclaw_turns::TurnCoordinator> =
        Arc::new(services.turn_coordinator_for_production()?);
    let durable = Arc::new(PgAuthProductServices::new(
        Arc::clone(&pg_pool),
        Arc::clone(&secret_store),
    ));
    let product_auth_ports = product_auth_ports.unwrap_or_else(|| {
        RebornProductAuthServicePorts::from_shared_with_provider(
            durable,
            provider_composition
                .client
                .clone()
                .unwrap_or_else(|| Arc::new(UnavailableAuthProviderClient)),
        )
    });
    let product_auth_services = compose_product_auth_services(
        product_auth_ports,
        turn_coordinator.clone(),
        provider_composition,
    );
    let product_auth_ready = true;
    // Wire ProductAuthAccount runtime credential resolver before
    // host_runtime_for_production so WASM extensions whose manifest declares a
    // ProductAuthAccount runtime credential source resolve through
    // CredentialAccountService.
    let services = services.with_runtime_credential_account_resolver(Arc::new(
        ProductAuthRuntimeCredentialResolver::new(
            product_auth_services.runtime_credential_account_selection_service(),
        ),
    ));
    register_bundled_gsuite_first_party_handlers(
        &mut first_party_registry,
        product_auth_services.credential_account_service(),
        product_auth_services.credential_account_record_source(),
        Arc::new(ProductAuthRuntimeGsuiteCredentialStager::new(
            product_auth_runtime_ports.clone(),
        )),
    )
    .map_err(|error| RebornBuildError::InvalidConfig {
        reason: format!("GSuite first-party handlers are invalid: {error}"),
    })?;
    let services = services.with_first_party_capabilities(Arc::new(first_party_registry));

    let host_runtime: Arc<dyn brassclaw_host_runtime::HostRuntime> =
        Arc::new(services.host_runtime_for_production(&wiring_config)?);

    // Build the three Postgres-backed WebUI stores from the shared pool.
    let pg_safety_config_store = Some(Arc::new(
        brassclaw_product_workflow::PgSafetyConfigStore::new(Arc::clone(&pg_pool), "default"),
    ));
    let pg_token_settings_store = Some(Arc::new(
        crate::pg_token_settings_store::PgTokenSettingsStore::new(Arc::clone(&pg_pool), "default"),
    ));
    let pg_memory_doc_store = Some(Arc::new(crate::pg_memory_doc_store::PgMemoryDocStore::new(
        Arc::clone(&pg_pool),
        "default",
    )));

    Ok(RebornServices {
        host_runtime: Some(host_runtime),
        turn_coordinator: Some(turn_coordinator),
        readiness: readiness_for(profile, true, true, product_auth_ready),
        product_auth: Some(product_auth_services),
        local_runtime: None,
        pg_pool: Some(pg_pool),
        #[cfg(feature = "root-llm-provider")]
        secret_store,
        pg_safety_config_store,
        pg_token_settings_store,
        pg_memory_doc_store,
    })
}

/// Build a `BuiltinFirstPartyTools` with PostgreSQL-backed memory stores and
/// optionally an embedding provider resolved from `brassclaw_config`.
///
/// Runs best-effort: errors in config load or embedding resolution degrade
/// silently rather than aborting boot (the stores / embedding are optional).
#[cfg(feature = "postgres")]
#[allow(dead_code)] // Phase-5 factory wiring
async fn build_postgres_memory_tools(pool: deadpool_postgres::Pool) -> BuiltinFirstPartyTools {
    use std::sync::Arc;

    use crate::pg_chat_memory_record_store::PgChatMemoryRecordStore;

    // Default tenant — embeddings config is a process-global setting.
    let tenant_id = "default";

    // Resolve embedding.provider_id from brassclaw_config (best-effort).
    // Only available when the root-llm-provider feature is also active.
    #[cfg(feature = "root-llm-provider")]
    let embedding_provider: Option<Arc<dyn brassclaw_memory::EmbeddingProvider>> =
        resolve_pg_embedding_provider(&pool, tenant_id).await;
    #[cfg(not(feature = "root-llm-provider"))]
    let embedding_provider: Option<Arc<dyn brassclaw_memory::EmbeddingProvider>> = None;

    // Build PgInterceptorStore and PgChatMemoryRecordStore.
    let interceptor_store = Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
        Arc::new(pool.clone()),
        tenant_id,
    ));
    let chat_memory_store = Arc::new(PgChatMemoryRecordStore::new(
        Arc::new(pool),
        interceptor_store,
    ));

    let mut tools = BuiltinFirstPartyTools::default().with_chat_memory_writer(chat_memory_store);
    if let Some(provider) = embedding_provider {
        tools = tools.with_memory_embedding_provider(provider);
    }
    tools
}

/// Resolve the embedding provider from `brassclaw_config` for the given tenant.
///
/// Returns `None` when no `embedding.provider_id` is configured or the provider
/// cannot be resolved.  All errors are logged at debug level and swallowed.
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
async fn resolve_pg_embedding_provider(
    pool: &deadpool_postgres::Pool,
    tenant_id: &str,
) -> Option<Arc<dyn brassclaw_memory::EmbeddingProvider>> {
    use crate::embedding_providers::{ProviderDeps, create_provider};
    use crate::embedding_role_adapter::EmbeddingRoleAdapter;
    use crate::pg_provider_repo::PgProviderRepo;
    use brassclaw_embeddings::EmbeddingCacheConfig;
    use brassclaw_llm::{SessionConfig, SessionManager};

    // Load config snapshot to get embedding.provider_id.
    let config = match crate::db_config::load_config_snapshot(pool, tenant_id).await {
        Ok(c) => c,
        Err(err) => {
            tracing::debug!(error = %err, "failed to load config for embedding resolution");
            return None;
        }
    };
    let embedding = config.embedding?;
    let provider_id = embedding.provider_id?;
    if provider_id.is_empty() {
        return None;
    }
    let model = embedding.model.clone();

    // Look up the provider definition from the DB.
    let provider_repo = PgProviderRepo::new(pool.clone(), tenant_id.to_string());
    let providers = match provider_repo.load().await {
        Ok(p) => p,
        Err(err) => {
            tracing::debug!(error = %err, "failed to load providers for embedding resolution");
            return None;
        }
    };
    let provider_def = providers.into_iter().find(|p| p.id == provider_id);

    // Build the embeddings config from the provider definition.
    let embeddings_config = build_embeddings_config_from_provider(provider_def.as_ref(), &model)?;

    // Build the provider.
    let session = Arc::new(SessionManager::new(SessionConfig::default()));
    let deps = ProviderDeps {
        session,
        #[cfg(feature = "bedrock")]
        bedrock_setup: None,
    };
    let raw_provider = create_provider(&embeddings_config, deps).await?;

    // Wrap in EmbeddingRoleAdapter with default cache config.
    Some(EmbeddingRoleAdapter::new_cached(
        raw_provider,
        EmbeddingCacheConfig::default(),
    ))
}

/// Build `EmbeddingsConfig` from a provider definition (optional) and a model override.
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
fn build_embeddings_config_from_provider(
    provider_def: Option<&brassclaw_llm::ProviderDefinition>,
    model_override: &Option<String>,
) -> Option<crate::embedding_providers::EmbeddingsConfig> {
    use crate::embedding_providers::{EmbeddingsConfig, default_dimension_for_model};
    use brassclaw_llm::registry::ProviderProtocol;

    let Some(def) = provider_def else {
        tracing::debug!("embedding.provider_id not found in provider catalog — embedding disabled");
        return None;
    };

    // Map ProviderProtocol to the EmbeddingsConfig provider string.
    let provider_str = match def.protocol {
        ProviderProtocol::Ollama => "ollama",
        ProviderProtocol::NearAi => "nearai",
        ProviderProtocol::Bedrock => "bedrock",
        // OpenAI-compatible variants all use the openai embeddings backend.
        _ => "openai",
    };

    let model = model_override
        .clone()
        .unwrap_or_else(|| def.default_model.clone());
    let dimension = default_dimension_for_model(&model);
    let base_url = def.default_base_url.clone();

    let config = EmbeddingsConfig {
        enabled: true,
        provider: provider_str.to_string(),
        model,
        dimension,
        openai_base_url: base_url,
        // API key resolved from env via api_key_env at composition startup.
        openai_api_key: def
            .api_key_env
            .as_deref()
            .and_then(|var| std::env::var(var).ok().map(secrecy::SecretString::from)),
        ..EmbeddingsConfig::default()
    };
    Some(config)
}

/// Public alias for `resolve_pg_embedding_provider` — used by the
/// `backfill-embeddings` CLI command in `retention_sweep.rs`.
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
pub(crate) async fn resolve_pg_embedding_provider_pub(
    pool: &deadpool_postgres::Pool,
    tenant_id: &str,
) -> Option<Arc<dyn brassclaw_memory::EmbeddingProvider>> {
    resolve_pg_embedding_provider(pool, tenant_id).await
}

fn readiness_for(
    profile: RebornCompositionProfile,
    host_runtime: bool,
    turn_coordinator: bool,
    product_auth: bool,
) -> RebornReadiness {
    let state = match profile {
        RebornCompositionProfile::Disabled => RebornReadinessState::Disabled,
        RebornCompositionProfile::LocalDev | RebornCompositionProfile::LocalDevYolo => {
            RebornReadinessState::DevOnly
        }
    };
    RebornReadiness {
        profile,
        state,
        facades: RebornFacadeReadiness {
            host_runtime,
            turn_coordinator,
            product_auth,
        },
        workers: RebornWorkerReadiness {
            turn_runner: false,
            trigger_poller: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_auth::{
        AuthProductScope, AuthSurface, CredentialAccountLabel, CredentialAccountStatus,
        CredentialOwnership, GOOGLE_CALENDAR_EVENTS_SCOPE, GOOGLE_GMAIL_SEND_SCOPE,
        NewCredentialAccount, ProviderScope,
    };
    use brassclaw_filesystem::FilesystemError;

    use crate::{
        extension_lifecycle::ExtensionActivationMode, runtime::SKILL_ACTIVATE_CAPABILITY_ID,
    };
    use brassclaw_filesystem::{
        DirEntry, FileStat, FilesystemOperation, RootFilesystem, VersionedEntry,
    };
    use brassclaw_host_api::{
        CapabilityGrant, CapabilityGrantId, CapabilityId, CapabilitySet, EffectKind,
        ExecutionContext, ExtensionId, GrantConstraints, InvocationId, MountAlias, MountGrant,
        MountPermissions, NetworkPolicy, NetworkScheme, NetworkTargetPattern, Principal,
        ResourceEstimate, ResourceScope, RuntimeCredentialAccountProviderId,
        RuntimeCredentialRequirementSource, RuntimeKind, ScopedPath, SecretHandle, TenantId,
        TrustClass, UserId, VirtualPath,
    };
    use brassclaw_host_runtime::{
        MEMORY_SEARCH_CAPABILITY_ID, MEMORY_TREE_CAPABILITY_ID, MEMORY_WRITE_CAPABILITY_ID,
        RuntimeCapabilityOutcome, RuntimeCapabilityRequest, RuntimeFailureKind,
        SKILL_INSTALL_CAPABILITY_ID, SKILL_LIST_CAPABILITY_ID, SKILL_REMOVE_CAPABILITY_ID,
        TRIGGER_CREATE_CAPABILITY_ID, TRIGGER_LIST_CAPABILITY_ID, TRIGGER_REMOVE_CAPABILITY_ID,
    };
    use brassclaw_product_workflow::{LifecyclePackageKind, LifecyclePackageRef};
    use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};

    struct FailingConversationActorPairingService;

    #[async_trait::async_trait]
    impl ConversationActorPairingService for FailingConversationActorPairingService {
        async fn pair_external_actor(
            &self,
            _tenant_id: TenantId,
            _adapter_kind: AdapterKind,
            _adapter_installation_id: AdapterInstallationId,
            _external_actor_ref: ExternalActorRef,
            _user_id: UserId,
        ) -> Result<(), brassclaw_conversations::InboundTurnError> {
            Err(brassclaw_conversations::InboundTurnError::DurableState {
                reason: "raw durable store error".to_string(),
            })
        }
    }

    struct FailingConversationStateFilesystem;

    #[async_trait::async_trait]
    impl RootFilesystem for FailingConversationStateFilesystem {
        async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
            Err(FilesystemError::Backend {
                path: path.clone(),
                operation: FilesystemOperation::ReadFile,
                reason: "conversation state load failed".to_string(),
            })
        }

        async fn list_dir(&self, _path: &VirtualPath) -> Result<Vec<DirEntry>, FilesystemError> {
            Ok(Vec::new())
        }

        async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
            Err(FilesystemError::NotFound {
                path: path.clone(),
                operation: FilesystemOperation::ReadFile,
            })
        }
    }

    fn trigger_record_for_pairing_test() -> TriggerRecord {
        TriggerRecord {
            trigger_id: brassclaw_triggers::TriggerId::new(),
            tenant_id: TenantId::new("pairing-test-tenant").expect("tenant id"),
            creator_user_id: UserId::new("pairing-test-user").expect("user id"),
            agent_id: None,
            project_id: None,
            name: "pairing test".to_string(),
            source: brassclaw_triggers::TriggerSourceKind::Schedule,
            schedule: brassclaw_triggers::TriggerSchedule::cron("* * * * *")
                .expect("valid cron expression"),
            completion_policy: brassclaw_triggers::TriggerCompletionPolicy::Recurring,
            prompt: "pairing test prompt".to_string(),
            state: brassclaw_triggers::TriggerState::Scheduled,
            next_run_at: chrono::Utc::now(),
            last_run_at: None,
            last_fired_slot: None,
            last_status: None,
            active_fire_slot: None,
            active_run_ref: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn pair_trigger_creator_maps_pairing_failure_to_sanitized_backend_error() {
        let record = trigger_record_for_pairing_test();

        let error = pair_trigger_creator(&FailingConversationActorPairingService, &record)
            .await
            .expect_err("pairing failure should surface");

        let TriggerError::Backend { reason } = error else {
            panic!("expected backend trigger error");
        };
        assert_eq!(reason, "trigger creator actor pairing failed");
    }

    async fn local_runtime_with_failing_trigger_conversations() -> Arc<RebornLocalRuntimeServices> {
        let local_dev_root = tempfile::tempdir().expect("tempdir");
        let owner_user_id = "pairing-owner";
        let services = build_reborn_services(RebornBuildInput::local_dev(
            owner_user_id,
            local_dev_root.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");

        let base_runtime = services.local_runtime.expect("local runtime");
        let mut failing_root = CompositeRootFilesystem::new();
        failing_root
            .mount(
                local_dev_mount_descriptor(
                    "/conversations",
                    "failing-conversation-state",
                    BackendKind::Custom("test".to_string()),
                    StorageClass::StructuredRecords,
                    ContentKind::StructuredRecord,
                    IndexPolicy::NotIndexed,
                    BackendCapabilities::default(),
                )
                .expect("mount descriptor"),
                Arc::new(FailingConversationStateFilesystem),
            )
            .expect("mount failing backend");
        Arc::new(RebornLocalRuntimeServices {
            approval_requests: Arc::clone(&base_runtime.approval_requests),
            capability_leases: Arc::clone(&base_runtime.capability_leases),
            turn_state: Arc::clone(&base_runtime.turn_state),
            trigger_repository: Arc::clone(&base_runtime.trigger_repository),
            trigger_conversation_services: tokio::sync::OnceCell::new(),
            checkpoint_state_store: Arc::clone(&base_runtime.checkpoint_state_store),
            loop_checkpoint_store: Arc::clone(&base_runtime.loop_checkpoint_store),
            thread_service: Arc::clone(&base_runtime.thread_service),
            resource_governor: Arc::clone(&base_runtime.resource_governor),
            budget_event_sink: Arc::clone(&base_runtime.budget_event_sink),
            in_memory_budget_event_sink: Arc::clone(&base_runtime.in_memory_budget_event_sink),
            broadcast_budget_event_sink: Arc::clone(&base_runtime.broadcast_budget_event_sink),
            budget_gate_store: Arc::clone(&base_runtime.budget_gate_store),
            skill_management: Arc::clone(&base_runtime.skill_management),
            extension_management: base_runtime.extension_management.clone(),
            runtime_http_egress: base_runtime.runtime_http_egress.clone(),
            host_runtime_http_egress: base_runtime.host_runtime_http_egress.clone(),
            skill_mounts: base_runtime.skill_mounts.clone(),
            memory_mounts: base_runtime.memory_mounts.clone(),
            skill_filesystem: Arc::clone(&base_runtime.skill_filesystem),
            workspace_filesystem: Arc::clone(&base_runtime.workspace_filesystem),
            subagent_goal_filesystem: Arc::new(ScopedFilesystem::with_fixed_view(
                Arc::new(failing_root),
                MountView::new(vec![MountGrant::new(
                    MountAlias::new("/conversations").expect("mount alias"),
                    VirtualPath::new("/conversations").expect("virtual path"),
                    MountPermissions::read_write_list_delete(),
                )])
                .expect("mount view"),
            )),
            extension_filesystem: Arc::clone(&base_runtime.extension_filesystem),
            workspace_mounts: base_runtime.workspace_mounts.clone(),
            local_dev_storage_root: base_runtime.local_dev_storage_root.clone(),
            default_system_prompt_path: base_runtime.default_system_prompt_path.clone(),
            event_log: Arc::clone(&base_runtime.event_log),
            audit_log: Arc::clone(&base_runtime.audit_log),
            extension_registry: Arc::clone(&base_runtime.extension_registry),
            content_cache_slot: brassclaw_reborn::content_cache_port::CurrentCacheBridgeSlot::new(),
            plan_state_slot: crate::plan_library::CurrentPlanStateSlot::new(),
        })
    }

    #[tokio::test]
    async fn durable_trigger_conversation_services_propagates_init_error() {
        let runtime = local_runtime_with_failing_trigger_conversations().await;

        let error = match runtime.durable_trigger_conversation_services().await {
            Ok(_) => panic!("conversation service init should fail"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            brassclaw_conversations::InboundTurnError::DurableState { .. }
        ));
    }

    #[tokio::test]
    async fn local_runtime_trigger_create_hook_maps_conversation_init_error_to_backend() {
        let hook = LocalRuntimeTriggerCreatorPairingHook {
            runtime: local_runtime_with_failing_trigger_conversations().await,
        };
        let record = trigger_record_for_pairing_test();

        let error = hook
            .after_trigger_persisted(&record)
            .await
            .expect_err("conversation init failure should surface as trigger backend error");

        let TriggerError::Backend { reason } = error else {
            panic!("expected backend trigger error");
        };
        assert_eq!(reason, "trigger creator actor pairing failed");
    }

    #[tokio::test]
    async fn local_dev_services_include_repl_runtime_substrate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-substrate-owner",
            dir.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");

        assert!(services.host_runtime.is_some());
        assert!(services.turn_coordinator.is_some());
        assert!(services.product_auth.is_some());
        assert!(services.local_runtime.is_some());
        assert!(
            services
                .local_runtime
                .as_ref()
                .expect("local runtime")
                .extension_management
                .is_some()
        );
        assert_eq!(services.readiness.state, RebornReadinessState::DevOnly);
    }

    #[tokio::test]
    async fn local_dev_memory_first_party_tools_use_mounted_memory_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-memory-owner",
            dir.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");
        let runtime = services.host_runtime.expect("host runtime composed");

        invoke_json(
            runtime.as_ref(),
            MEMORY_WRITE_CAPABILITY_ID,
            memory_context(MEMORY_WRITE_CAPABILITY_ID),
            serde_json::json!({
                "target": "projects/alpha/notes.md",
                "content": "local dev mounted memory root search marker",
                "append": false
            }),
        )
        .await
        .expect("memory_write should use the mounted /memory root");

        let tree = invoke_json(
            runtime.as_ref(),
            MEMORY_TREE_CAPABILITY_ID,
            memory_context(MEMORY_TREE_CAPABILITY_ID),
            serde_json::json!({"path": "", "depth": 3}),
        )
        .await
        .expect("memory_tree should list the mounted /memory root");
        assert!(
            tree.to_string().contains("alpha/"),
            "memory_tree should include the written memory document: {tree}"
        );

        let search = invoke_json(
            runtime.as_ref(),
            MEMORY_SEARCH_CAPABILITY_ID,
            memory_context(MEMORY_SEARCH_CAPABILITY_ID),
            serde_json::json!({"query": "mounted memory root search marker", "limit": 5}),
        )
        .await
        .expect("memory_search should query the mounted /memory root");
        assert_eq!(search["result_count"], serde_json::json!(1));
        assert_eq!(
            search["results"][0]["path"],
            serde_json::json!("projects/alpha/notes.md")
        );
    }

    /// Verify that `attach_hosted_mcp_runtime` is soft-disabled when the host
    /// runtime has no HTTP egress (e.g. in-memory-only test services). The
    /// function must not panic or return an error; it simply skips the MCP
    /// runtime attachment so the rest of the composition continues.
    #[test]
    fn attach_hosted_mcp_runtime_skips_services_without_http_egress() {
        let services = HostRuntimeServices::new(
            Arc::new(ExtensionRegistry::new()),
            Arc::new(LocalFilesystem::new()),
            Arc::new(InMemoryResourceGovernor::new()),
            Arc::new(GrantAuthorizer::new()),
            ProcessServices::in_memory(),
            CapabilitySurfaceVersion::new("surface-v1").unwrap(),
        );
        // product_auth_provider_runtime_ports() is None without HTTP egress.
        assert!(services.product_auth_provider_runtime_ports().is_none());

        // attach_hosted_mcp_runtime must succeed (soft-skip) rather than error.
        let services = attach_hosted_mcp_runtime(services).expect("soft-disable must not error");

        // Runtime ports still absent — no egress was added by the attachment.
        assert!(services.product_auth_provider_runtime_ports().is_none());
    }

    #[tokio::test]
    async fn local_dev_gsuite_installs_activates_and_dispatches_through_host_runtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-gsuite-owner",
            dir.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");
        let local_runtime = services.local_runtime.as_ref().expect("local runtime");
        let extension_management = local_runtime
            .extension_management
            .as_ref()
            .expect("extension management");
        let gmail_ref =
            LifecyclePackageRef::new(LifecyclePackageKind::Extension, "gmail").expect("valid ref");
        let calendar_ref =
            LifecyclePackageRef::new(LifecyclePackageKind::Extension, "google-calendar")
                .expect("valid ref");

        extension_management
            .install(gmail_ref.clone())
            .await
            .expect("install Gmail");
        extension_management
            .activate(gmail_ref, ExtensionActivationMode::Static)
            .await
            .expect("activate Gmail");
        extension_management
            .install(calendar_ref.clone())
            .await
            .expect("install Google Calendar");
        extension_management
            .activate(calendar_ref, ExtensionActivationMode::Static)
            .await
            .expect("activate Google Calendar");

        let gmail_context = gsuite_context("gmail.send_message");
        let auth_scope =
            AuthProductScope::new(gmail_context.resource_scope.clone(), AuthSurface::Api);
        services
            .product_auth
            .as_ref()
            .expect("product auth")
            .credential_account_service()
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: brassclaw_first_party_extensions::google_provider_id()
                    .expect("Google provider id"),
                label: CredentialAccountLabel::new("work google").expect("valid label"),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("missing-google-access-token").unwrap()),
                refresh_secret: None,
                scopes: vec![
                    ProviderScope::new(GOOGLE_GMAIL_SEND_SCOPE).unwrap(),
                    ProviderScope::new(GOOGLE_CALENDAR_EVENTS_SCOPE).unwrap(),
                ],
            })
            .await
            .expect("create Google account");

        let outcome = services
            .host_runtime
            .as_ref()
            .expect("host runtime")
            .invoke_capability(RuntimeCapabilityRequest::new(
                gmail_context,
                CapabilityId::new("gmail.send_message").unwrap(),
                ResourceEstimate::default(),
                serde_json::json!({ "message": { "raw": "base64url-rfc822" } }),
                trust_decision(),
            ))
            .await
            .expect("runtime invocation completes");

        let RuntimeCapabilityOutcome::Failed(failure) = outcome else {
            panic!("expected fail-closed handler outcome, got {outcome:?}");
        };
        assert_eq!(failure.capability_id.as_str(), "gmail.send_message");
        assert_ne!(failure.kind, RuntimeFailureKind::Authorization);
        assert_ne!(failure.kind, RuntimeFailureKind::MissingRuntime);

        let calendar_context = gsuite_context("google-calendar.create_event");
        let outcome = services
            .host_runtime
            .as_ref()
            .expect("host runtime")
            .invoke_capability(RuntimeCapabilityRequest::new(
                calendar_context,
                CapabilityId::new("google-calendar.create_event").unwrap(),
                ResourceEstimate::default(),
                serde_json::json!({
                    "calendar_id": "primary",
                    "event": { "summary": "Review" }
                }),
                trust_decision(),
            ))
            .await
            .expect("runtime invocation completes");

        let RuntimeCapabilityOutcome::Failed(failure) = outcome else {
            panic!("expected fail-closed handler outcome, got {outcome:?}");
        };
        assert_eq!(
            failure.capability_id.as_str(),
            "google-calendar.create_event"
        );
        assert_ne!(failure.kind, RuntimeFailureKind::Authorization);
        assert_ne!(failure.kind, RuntimeFailureKind::MissingRuntime);
    }

    #[tokio::test]
    async fn local_dev_notion_mcp_installs_activates_and_reaches_auth_gate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-notion-mcp-owner",
            dir.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");
        let local_runtime = services.local_runtime.as_ref().expect("local runtime");
        let extension_management = local_runtime
            .extension_management
            .as_ref()
            .expect("extension management");
        let notion_ref =
            LifecyclePackageRef::new(LifecyclePackageKind::Extension, "notion").expect("valid ref");
        let catalog = AvailableExtensionCatalog::from_first_party_assets()
            .expect("first-party extensions load");
        let notion_package = catalog.resolve(&notion_ref).expect("Notion MCP is bundled");
        let capability_ids = notion_package
            .package
            .manifest
            .capabilities
            .iter()
            .map(|capability| capability.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(capability_ids.len(), 18);
        assert!(capability_ids.contains(&"notion.notion-create-pages"));
        assert!(capability_ids.contains(&"notion.notion-query-data-sources"));
        assert!(capability_ids.contains(&"notion.notion-create-comment"));
        assert!(capability_ids.contains(&"notion.notion-get-self"));

        extension_management
            .install(notion_ref.clone())
            .await
            .expect("install Notion MCP");
        extension_management
            .activate(notion_ref, ExtensionActivationMode::Static)
            .await
            .expect("activate Notion MCP");

        let outcome = services
            .host_runtime
            .as_ref()
            .expect("host runtime")
            .invoke_capability(RuntimeCapabilityRequest::new(
                notion_mcp_context("notion.notion-search"),
                CapabilityId::new("notion.notion-search").unwrap(),
                ResourceEstimate::default(),
                serde_json::json!({ "query": "project notes" }),
                notion_mcp_trust_decision(),
            ))
            .await
            .expect("runtime invocation completes");

        let RuntimeCapabilityOutcome::AuthRequired(gate) = outcome else {
            panic!("expected missing Notion token to open auth gate, got {outcome:?}");
        };
        assert_eq!(gate.capability_id.as_str(), "notion.notion-search");
    }

    #[tokio::test]
    async fn local_dev_web_access_installs_activates_and_dispatches_through_host_runtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-web-access-owner",
            dir.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");
        let local_runtime = services.local_runtime.as_ref().expect("local runtime");
        let extension_management = local_runtime
            .extension_management
            .as_ref()
            .expect("extension management");
        let web_access_ref =
            LifecyclePackageRef::new(LifecyclePackageKind::Extension, "web-access")
                .expect("valid ref");

        extension_management
            .install(web_access_ref.clone())
            .await
            .expect("install Web Access");
        extension_management
            .activate(web_access_ref, ExtensionActivationMode::Static)
            .await
            .expect("activate Web Access");

        let outcome = services
            .host_runtime
            .as_ref()
            .expect("host runtime")
            .invoke_capability(RuntimeCapabilityRequest::new(
                web_access_context("web-access.search"),
                CapabilityId::new("web-access.search").unwrap(),
                ResourceEstimate::default(),
                serde_json::json!({
                    "provider": "brave",
                    "query": "brassclaw reborn"
                }),
                trust_decision(),
            ))
            .await
            .expect("runtime invocation completes");

        let RuntimeCapabilityOutcome::Failed(failure) = outcome else {
            panic!("expected fail-closed handler outcome, got {outcome:?}");
        };
        assert_eq!(failure.capability_id.as_str(), "web-access.search");
        assert_eq!(failure.kind, RuntimeFailureKind::Backend);
    }

    #[tokio::test]
    async fn local_dev_nearai_mcp_installs_and_activates_model_visible_capability() {
        let dir = tempfile::tempdir().expect("tempdir");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-nearai-mcp-owner",
            dir.path().join("local-dev"),
        ))
        .await
        .expect("local-dev services build");
        let local_runtime = services.local_runtime.as_ref().expect("local runtime");
        let extension_management = local_runtime
            .extension_management
            .as_ref()
            .expect("extension management");
        let nearai_ref =
            LifecyclePackageRef::new(LifecyclePackageKind::Extension, "nearai").expect("valid ref");

        extension_management
            .install(nearai_ref.clone())
            .await
            .expect("install NEAR AI MCP");
        extension_management
            .activate(nearai_ref, ExtensionActivationMode::Static)
            .await
            .expect("activate NEAR AI MCP");

        let capabilities = extension_management
            .active_model_visible_capabilities()
            .await
            .expect("active capabilities");
        let search = capabilities
            .iter()
            .find(|capability| capability.id.as_str() == "nearai.search")
            .expect("nearai.search active");

        assert_eq!(search.provider.as_str(), "nearai");
        assert_eq!(search.effects, nearai_allowed_effects());
        assert_eq!(search.runtime_credentials.len(), 1);
        assert_eq!(
            search.runtime_credentials[0].handle,
            SecretHandle::new("llm_nearai_api_key").unwrap()
        );
        // NEAR AI MCP credential is sourced from a product-auth account so that the
        // user-facing setup flow is the manual-token product-auth surface (shared
        // with GitHub WASM), not an out-of-band SecretStore handle drop.
        // The 'handle' field remains the staging slot name the MCP egress planner
        // reads from RuntimeSecretInjectionStore after the obligation handler resolves
        // the access secret via RuntimeCredentialAccountResolver.
        assert_eq!(
            search.runtime_credentials[0].source,
            RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("nearai").unwrap(),
                setup: Default::default(),
            }
        );
        assert_eq!(
            search.runtime_credentials[0].audience.host_pattern,
            "private.near.ai"
        );
    }

    #[test]
    fn attach_hosted_mcp_runtime_skips_services_without_runtime_http_egress() {
        let services = HostRuntimeServices::new(
            Arc::new(ExtensionRegistry::new()),
            Arc::new(LocalFilesystem::new()),
            Arc::new(InMemoryResourceGovernor::new()),
            Arc::new(GrantAuthorizer::new()),
            ProcessServices::in_memory(),
            CapabilitySurfaceVersion::new("surface-v1").unwrap(),
        );

        let services = attach_hosted_mcp_runtime(services).expect("attach is optional");

        assert!(services.product_auth_provider_runtime_ports().is_none());
    }

    #[tokio::test]
    async fn local_dev_setup_marker_workspace_filesystem_is_read_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_root = dir.path().join("local-dev");
        let marker_path = storage_root.join("workspace/markers/setup.done");
        std::fs::create_dir_all(marker_path.parent().expect("marker parent"))
            .expect("marker directory");
        std::fs::write(&marker_path, "done").expect("marker file");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-marker-workspace-owner",
            storage_root,
        ))
        .await
        .expect("local-dev services build");
        let local_runtime = services
            .local_runtime
            .as_ref()
            .expect("local-dev runtime substrate");
        let scope = ResourceScope::local_default(
            UserId::new("local-dev-marker-user").expect("valid user"),
            InvocationId::new(),
        )
        .expect("valid resource scope");

        let stat = local_runtime
            .workspace_filesystem
            .stat(
                &scope,
                &ScopedPath::new("/workspace/markers/setup.done").expect("valid marker path"),
            )
            .await
            .expect("marker stat succeeds");
        assert_eq!(stat.len, 4);

        let error = local_runtime
            .workspace_filesystem
            .write_file(
                &scope,
                &ScopedPath::new("/workspace/markers/new.done").expect("valid marker path"),
                b"done",
            )
            .await
            .expect_err("setup marker workspace filesystem should be read-only");
        assert!(matches!(error, FilesystemError::PermissionDenied { .. }));
    }

    #[tokio::test]
    async fn local_dev_skill_management_invokes_through_first_party_runtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_root = dir.path().join("local-dev");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-skill-tools-owner",
            storage_root.clone(),
        ))
        .await
        .expect("local-dev services build");
        let runtime = services.host_runtime.expect("host runtime composed");

        let install_output = invoke_json(
            runtime.as_ref(),
            SKILL_INSTALL_CAPABILITY_ID,
            skill_context(SKILL_INSTALL_CAPABILITY_ID),
            serde_json::json!({
                "content": skill_md("runtime-sentinel", "runtime skill", "RUNTIME_SENTINEL")
            }),
        )
        .await
        .expect("skill install succeeds");
        assert_eq!(install_output["installed"], true);
        assert_eq!(install_output["name"], "runtime-sentinel");
        assert!(
            storage_root
                .join("skills/runtime-sentinel/SKILL.md")
                .exists()
        );

        let list_output = invoke_json(
            runtime.as_ref(),
            SKILL_LIST_CAPABILITY_ID,
            skill_context(SKILL_LIST_CAPABILITY_ID),
            serde_json::json!({}),
        )
        .await
        .expect("skill list succeeds");
        assert!(
            list_output["skills"]
                .as_array()
                .unwrap()
                .iter()
                .any(|skill| { skill["name"] == "runtime-sentinel" && skill["source"] == "user" })
        );

        let remove_output = invoke_json(
            runtime.as_ref(),
            SKILL_REMOVE_CAPABILITY_ID,
            skill_context(SKILL_REMOVE_CAPABILITY_ID),
            serde_json::json!({"name": "runtime-sentinel"}),
        )
        .await
        .expect("skill remove succeeds");
        assert_eq!(remove_output["removed"], true);
        assert!(
            !storage_root
                .join("skills/runtime-sentinel/SKILL.md")
                .exists()
        );
    }

    #[tokio::test]
    async fn local_dev_workspace_mounts_do_not_authorize_skill_writes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_root = dir.path().join("local-dev");
        let services = build_reborn_services(RebornBuildInput::local_dev(
            "local-dev-workspace-skill-boundary-owner",
            storage_root.clone(),
        ))
        .await
        .expect("local-dev services build");
        let runtime = services.host_runtime.expect("host runtime composed");

        let failure = invoke_json(
            runtime.as_ref(),
            "builtin.write_file",
            workspace_context("builtin.write_file"),
            serde_json::json!({
                "path": "/skills/blocked/SKILL.md",
                "content": skill_md("blocked", "blocked skill", "BLOCKED")
            }),
        )
        .await
        .expect_err("workspace tool cannot write skill root");

        assert_eq!(failure, RuntimeFailureKind::Authorization);
        assert!(!storage_root.join("skills/blocked/SKILL.md").exists());
    }

    #[test]
    fn local_dev_workspace_root_overlapping_skill_root_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage_root = dir.path().join("local-dev");

        for skill_root in [
            storage_root.join("skills"),
            storage_root.join("tenant-shared/skills"),
            storage_root.join("system/skills"),
        ] {
            for workspace_root in [
                skill_root.clone(),
                skill_root
                    .parent()
                    .expect("skill root parent")
                    .to_path_buf(),
                skill_root.join("nested-workspace"),
            ] {
                let error =
                    validate_local_dev_workspace_skill_isolation(&storage_root, &workspace_root)
                        .expect_err("workspace root overlapping skill root should be rejected");
                assert!(
                    matches!(error, RebornBuildError::InvalidConfig { .. }),
                    "unexpected error: {error:?}"
                );
            }
        }
    }

    #[test]
    fn builtin_first_party_package_declares_skill_management_tools() {
        let package = builtin_first_party_package().expect("built-in package builds");
        let ids = package
            .capabilities
            .iter()
            .map(|capability| capability.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&SKILL_LIST_CAPABILITY_ID));
        assert!(!ids.contains(&SKILL_ACTIVATE_CAPABILITY_ID));
        assert!(ids.contains(&SKILL_INSTALL_CAPABILITY_ID));
        assert!(ids.contains(&SKILL_REMOVE_CAPABILITY_ID));
        assert!(ids.contains(&TRIGGER_CREATE_CAPABILITY_ID));
        assert!(ids.contains(&TRIGGER_LIST_CAPABILITY_ID));
        assert!(ids.contains(&TRIGGER_REMOVE_CAPABILITY_ID));

        let registry = brassclaw_host_runtime::builtin_first_party_handlers(Arc::new(
            brassclaw_triggers::InMemoryTriggerRepository::default(),
        ))
        .expect("built-in handlers build");
        for id in [
            SKILL_LIST_CAPABILITY_ID,
            SKILL_INSTALL_CAPABILITY_ID,
            SKILL_REMOVE_CAPABILITY_ID,
            TRIGGER_CREATE_CAPABILITY_ID,
            TRIGGER_LIST_CAPABILITY_ID,
            TRIGGER_REMOVE_CAPABILITY_ID,
        ] {
            assert!(registry.contains_handler(&brassclaw_host_api::CapabilityId::new(id).unwrap()));
        }
        assert!(!registry.contains_handler(
            &brassclaw_host_api::CapabilityId::new(SKILL_ACTIVATE_CAPABILITY_ID).unwrap()
        ));
    }

    #[test]
    fn disabled_services_do_not_include_repl_runtime_substrate() {
        let services = RebornServices::disabled();

        assert!(services.host_runtime.is_none());
        assert!(services.turn_coordinator.is_none());
        assert!(services.product_auth.is_none());
        assert!(services.local_runtime.is_none());
        assert_eq!(services.readiness.state, RebornReadinessState::Disabled);
    }

    #[test]
    fn local_dev_readiness_reflects_product_auth_presence() {
        let without_auth = readiness_for(RebornCompositionProfile::LocalDev, true, true, false);
        assert_eq!(without_auth.state, RebornReadinessState::DevOnly);
        assert!(!without_auth.facades.product_auth);

        let with_auth = readiness_for(RebornCompositionProfile::LocalDev, true, true, true);
        assert_eq!(with_auth.state, RebornReadinessState::DevOnly);
        assert!(with_auth.facades.product_auth);
    }

    async fn invoke_json(
        runtime: &dyn brassclaw_host_runtime::HostRuntime,
        capability_id: &str,
        context: ExecutionContext,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeFailureKind> {
        let outcome = runtime
            .invoke_capability(RuntimeCapabilityRequest::new(
                context,
                CapabilityId::new(capability_id).expect("valid capability id"),
                ResourceEstimate::default(),
                input,
                trust_decision(),
            ))
            .await
            .expect("runtime invocation completes");
        match outcome {
            RuntimeCapabilityOutcome::Completed(completed) => Ok(completed.output),
            RuntimeCapabilityOutcome::Failed(failure) => Err(failure.kind),
            other => panic!("unexpected runtime outcome: {other:?}"),
        }
    }

    fn skill_context(capability_id: &str) -> ExecutionContext {
        execution_context(capability_id, skill_mounts())
    }

    fn workspace_context(capability_id: &str) -> ExecutionContext {
        execution_context(capability_id, workspace_mounts())
    }

    fn memory_context(capability_id: &str) -> ExecutionContext {
        execution_context(
            capability_id,
            memory_mount_view(MountPermissions::read_write_list_delete())
                .expect("valid memory mounts"),
        )
    }

    fn gsuite_context(capability_id: &str) -> ExecutionContext {
        let extension_id = ExtensionId::new("caller").expect("valid extension id");
        ExecutionContext::local_default(
            UserId::new("local-dev-test-user").expect("valid user id"),
            extension_id.clone(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet {
                grants: vec![CapabilityGrant {
                    id: CapabilityGrantId::new(),
                    capability: CapabilityId::new(capability_id).expect("valid capability id"),
                    grantee: Principal::Extension(extension_id),
                    issued_by: Principal::HostRuntime,
                    constraints: GrantConstraints {
                        allowed_effects: gsuite_allowed_effects(),
                        mounts: MountView::new(Vec::new()).expect("valid empty mount view"),
                        network: NetworkPolicy::default(),
                        secrets: vec![SecretHandle::new("missing-google-access-token").unwrap()],
                        resource_ceiling: None,
                        expires_at: None,
                        max_invocations: None,
                    },
                }],
            },
            MountView::new(Vec::new()).expect("valid empty mount view"),
        )
        .expect("valid execution context")
    }

    fn notion_mcp_context(capability_id: &str) -> ExecutionContext {
        let extension_id = ExtensionId::new("caller").expect("valid extension id");
        ExecutionContext::local_default(
            UserId::new("local-dev-test-user").expect("valid user id"),
            extension_id.clone(),
            RuntimeKind::Mcp,
            TrustClass::Sandbox,
            CapabilitySet {
                grants: vec![CapabilityGrant {
                    id: CapabilityGrantId::new(),
                    capability: CapabilityId::new(capability_id).expect("valid capability id"),
                    grantee: Principal::Extension(extension_id),
                    issued_by: Principal::HostRuntime,
                    constraints: GrantConstraints {
                        allowed_effects: notion_mcp_allowed_effects(),
                        mounts: MountView::new(Vec::new()).expect("valid empty mount view"),
                        network: notion_mcp_network_policy(),
                        secrets: vec![SecretHandle::new("mcp_notion_access_token").unwrap()],
                        resource_ceiling: None,
                        expires_at: None,
                        max_invocations: None,
                    },
                }],
            },
            MountView::new(Vec::new()).expect("valid empty mount view"),
        )
        .expect("valid execution context")
    }

    fn web_access_context(capability_id: &str) -> ExecutionContext {
        let extension_id = ExtensionId::new("caller").expect("valid extension id");
        ExecutionContext::local_default(
            UserId::new("local-dev-test-user").expect("valid user id"),
            extension_id.clone(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet {
                grants: vec![CapabilityGrant {
                    id: CapabilityGrantId::new(),
                    capability: CapabilityId::new(capability_id).expect("valid capability id"),
                    grantee: Principal::Extension(extension_id),
                    issued_by: Principal::HostRuntime,
                    constraints: GrantConstraints {
                        allowed_effects: web_access_allowed_effects(),
                        mounts: MountView::new(Vec::new()).expect("valid empty mount view"),
                        network: web_access_network_policy(),
                        secrets: Vec::new(),
                        resource_ceiling: None,
                        expires_at: None,
                        max_invocations: None,
                    },
                }],
            },
            MountView::new(Vec::new()).expect("valid empty mount view"),
        )
        .expect("valid execution context")
    }

    fn web_access_network_policy() -> NetworkPolicy {
        NetworkPolicy {
            allowed_targets: vec![NetworkTargetPattern {
                scheme: Some(brassclaw_host_api::NetworkScheme::Https),
                host_pattern: "mcp.exa.ai".to_string(),
                port: None,
            }],
            deny_private_ip_ranges: true,
            max_egress_bytes: None,
        }
    }

    fn execution_context(capability_id: &str, mounts: MountView) -> ExecutionContext {
        let extension_id = ExtensionId::new("caller").expect("valid extension id");
        ExecutionContext::local_default(
            UserId::new("local-dev-test-user").expect("valid user id"),
            extension_id.clone(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet {
                grants: vec![capability_grant(
                    capability_id,
                    extension_id,
                    mounts.clone(),
                )],
            },
            mounts,
        )
        .expect("valid execution context")
    }

    fn capability_grant(
        capability_id: &str,
        grantee: ExtensionId,
        mounts: MountView,
    ) -> CapabilityGrant {
        CapabilityGrant {
            id: CapabilityGrantId::new(),
            capability: CapabilityId::new(capability_id).expect("valid capability id"),
            grantee: Principal::Extension(grantee),
            issued_by: Principal::HostRuntime,
            constraints: GrantConstraints {
                allowed_effects: allowed_effects(),
                mounts,
                network: network_policy(),
                secrets: Vec::new(),
                resource_ceiling: None,
                expires_at: None,
                max_invocations: None,
            },
        }
    }

    fn skill_mounts() -> MountView {
        MountView::new(vec![
            MountGrant::new(
                MountAlias::new("/skills").expect("valid mount alias"),
                VirtualPath::new("/projects/skills").expect("valid virtual path"),
                MountPermissions::read_write_list_delete(),
            ),
            MountGrant::new(
                MountAlias::new("/system/skills").expect("valid mount alias"),
                VirtualPath::new("/projects/system/skills").expect("valid virtual path"),
                MountPermissions::read_only(),
            ),
        ])
        .expect("valid mount view")
    }

    fn workspace_mounts() -> MountView {
        MountView::new(vec![MountGrant::new(
            MountAlias::new("/workspace").expect("valid mount alias"),
            VirtualPath::new("/projects/workspace").expect("valid virtual path"),
            MountPermissions::read_write(),
        )])
        .expect("valid mount view")
    }

    fn allowed_effects() -> Vec<EffectKind> {
        vec![
            EffectKind::DispatchCapability,
            EffectKind::ReadFilesystem,
            EffectKind::WriteFilesystem,
            EffectKind::DeleteFilesystem,
            EffectKind::Network,
        ]
    }

    fn network_policy() -> NetworkPolicy {
        NetworkPolicy {
            allowed_targets: vec![NetworkTargetPattern {
                scheme: None,
                host_pattern: "*".to_string(),
                port: None,
            }],
            deny_private_ip_ranges: true,
            max_egress_bytes: None,
        }
    }

    fn notion_mcp_network_policy() -> NetworkPolicy {
        NetworkPolicy {
            allowed_targets: vec![NetworkTargetPattern {
                scheme: Some(NetworkScheme::Https),
                host_pattern: "mcp.notion.com".to_string(),
                port: None,
            }],
            deny_private_ip_ranges: true,
            max_egress_bytes: None,
        }
    }

    fn notion_mcp_allowed_effects() -> Vec<EffectKind> {
        vec![
            EffectKind::DispatchCapability,
            EffectKind::Network,
            EffectKind::UseSecret,
        ]
    }

    fn trust_decision() -> TrustDecision {
        TrustDecision {
            effective_trust: EffectiveTrustClass::user_trusted(),
            authority_ceiling: AuthorityCeiling {
                allowed_effects: allowed_effects(),
                max_resource_ceiling: None,
            },
            provenance: TrustProvenance::Default,
            evaluated_at: chrono::Utc::now(),
        }
    }

    fn notion_mcp_trust_decision() -> TrustDecision {
        TrustDecision {
            effective_trust: EffectiveTrustClass::user_trusted(),
            authority_ceiling: AuthorityCeiling {
                allowed_effects: notion_mcp_allowed_effects(),
                max_resource_ceiling: None,
            },
            provenance: TrustProvenance::Default,
            evaluated_at: chrono::Utc::now(),
        }
    }

    fn skill_md(name: &str, description: &str, prompt: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n{prompt}\n")
    }
}

#[cfg(test)]
mod auth_tests;
#[cfg(test)]
mod local_dev_host_tests;
