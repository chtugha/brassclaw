//! Installation boot before turn workers. No lazy VM, thread UUID conversion,
//! task selection, model work or raw Tool dispatch occurs here.
use crate::{
    RebornRuntimeError,
    global_monty_owner::{
        GlobalMontyOwner, GlobalServiceConfig, GlobalServiceExit, OwnershipSettlement,
    },
    installed_monty_catalogue::InstalledMontyCatalogue,
    monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::memory::intent_system::IntentScope;
use brassclaw_host_api::MountView;
use brassclaw_monty_host::{
    VmBounds,
    global::GlobalBounds,
    heap::HeapSettings,
    process::{ProcessLimits, RootBoot, TaskSettings, installed_worker},
    transport_actor::ActorLimits,
};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_resources::LiveMontyTaskSettings;
use std::{sync::Arc, time::Duration};
fn invalid(reason: impl Into<String>) -> RebornRuntimeError {
    RebornRuntimeError::InvalidArgument {
        reason: reason.into(),
    }
}
pub(crate) async fn start(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    scope: &IntentScope,
    mounts: MountView,
) -> Result<
    (
        GlobalMontyOwner,
        Arc<InstalledMontyCatalogue>,
        brassclaw_product_workflow::MontyVmSettings,
    ),
    RebornRuntimeError,
> {
    let worker = installed_worker().map_err(|error| invalid(error.to_string()))?;
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .map_err(|error| invalid(error.to_string()))?;
    let catalogue = InstalledMontyCatalogue::boot(
        pool.clone(),
        kernel.clone(),
        &worker,
        scope,
        mounts,
        root.reference(),
    )
    .await
    .map_err(|error| invalid(error.to_string()))?;
    // Upgrade revision-zero settings without changing any operator values.
    // One instance settings address; conversation IDs are never settings keys.
    let client = pool
        .get()
        .await
        .map_err(|_| invalid("Monty settings unavailable"))?;
    client
        .execute(
            "INSERT INTO reborn_monty_vm_settings(tenant_id,user_id,agent_id,project_id,revision)
        VALUES('default','default','default','default',1)
        ON CONFLICT(tenant_id,user_id,agent_id,project_id) DO UPDATE SET revision=1
        WHERE reborn_monty_vm_settings.revision=0",
            &[],
        )
        .await
        .map_err(|_| invalid("Monty settings initialization failed"))?;
    drop(client);
    let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        pool.clone(),
        "default",
        "default",
    )
    .get("default", "default")
    .await
    .map_err(|error| invalid(error.to_string()))?;
    let task_settings = TaskSettings {
        revision: settings.revision,
        max_compute_time: Duration::from_secs(settings.max_duration_secs),
        token_budgets_enabled: settings.token_budgets_enabled,
    };
    let live = LiveMontyTaskSettings::new(task_settings.into())
        .map_err(|error| invalid(error.to_string()))?;
    kernel
        .bind_task_budget_settings(live.clone())
        .map_err(|error| invalid(error.to_string()))?;
    // Retain the existing explicit heap cap until the adaptive-settings migration.
    // This finite physical backstop is independent of task executing time.
    let soft = usize::try_from(
        settings
            .max_memory_bytes
            .ok_or_else(|| invalid("Monty heap cap missing"))?,
    )
    .map_err(|_| invalid("Monty heap cap out of range"))?;
    let frame = 64 * 1024 * 1024;
    let hard = soft
        .checked_add(2 * frame + 4 * 1024 * 1024)
        .ok_or_else(|| invalid("Monty physical backstop overflow"))?;
    let values = VmBounds {
        max_source_bytes: 1024 * 1024,
        max_compiled_source_bytes: 2 * 1024 * 1024,
        max_feeds: 128,
        max_stdout_bytes: 1024 * 1024,
        execution_slice: Duration::from_millis(5),
        max_value_depth: 48,
        max_value_nodes: 1024 * 1024,
        max_value_bytes: frame,
    };
    let boot = RootBoot {
        heap_settings: Some(HeapSettings {
            revision: 1,
            max_vm_bytes: soft,
        }),
        source: root.source().to_owned(),
        checksum: root.source_checksum(),
        aliases: root.ports().clone(),
        bounds: GlobalBounds {
            values,
            workers: 2,
            max_pending_calls: 128,
        },
        startup_timeout: Duration::from_secs(30),
        task_settings,
        max_recipe_contexts: 64,
    };
    let owner = GlobalMontyOwner::start(
        &pool,
        &worker,
        GlobalServiceConfig {
            boot,
            process: ProcessLimits {
                hard_memory_bytes: hard,
                max_frame_bytes: frame,
                response_timeout: Duration::from_secs(30),
            },
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: frame * 16,
                max_control_unclaimed: 8,
                max_control_reserved_frame_bytes: frame * 16,
            },
            queue_capacity: 64,
            live,
        },
    )
    .await
    .map_err(|error| invalid(error.to_string()))?;
    Ok((owner, catalogue, settings))
}
pub(crate) fn check_shutdown(exit: GlobalServiceExit) -> Result<(), RebornRuntimeError> {
    let released = match exit.ownership {
        OwnershipSettlement::ReleaseAttempt(result) => result.is_ok(),
        OwnershipSettlement::Quarantined(owner) => {
            // The supervisor registry retains this owner until reconciliation.
            tracing::error!(
                owners = Arc::strong_count(&owner),
                "Monty ownership quarantined"
            );
            false
        }
    };
    if !released
        || exit.ownership_failure.is_some()
        || !exit.service.is_ok_and(|service| service.failure.is_none())
    {
        return Err(invalid("global Monty shutdown requires reconciliation"));
    }
    Ok(())
}
