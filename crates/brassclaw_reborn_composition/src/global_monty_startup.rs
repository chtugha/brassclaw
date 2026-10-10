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
    global::GlobalBounds,
    heap::HeapSettings,
    process::{ProcessLimits, RootBoot, TaskSettings, installed_worker},
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
        &'static str,
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
    // Start with the configured finite value. Startup sizing is applied to the
    // actual worker before ingress; manual startup retains the upgraded value.
    // This finite physical backstop is independent of task executing time.
    let soft = usize::try_from(
        settings
            .max_memory_bytes
            .ok_or_else(|| invalid("Monty heap cap missing"))?,
    )
    .map_err(|_| invalid("Monty heap cap out of range"))?;
    let frame = crate::live_monty_settings::ipc_frame(settings.execution_limits)
        .map_err(|error| invalid(error.to_string()))?;
    let adapter_reserve_bytes =
        usize::try_from(settings.execution_limits.worker_adapter_reserve_bytes)
            .map_err(|_| invalid("Monty adapter reserve out of range"))?;
    let hard = soft
        .checked_add(
            frame
                .checked_mul(2)
                .ok_or_else(|| invalid("Monty frames overflow"))?,
        )
        .and_then(|bytes| bytes.checked_add(adapter_reserve_bytes))
        .ok_or_else(|| invalid("Monty physical backstop overflow"))?;
    let values = crate::live_monty_settings::execution_bounds(settings.execution_limits)
        .map_err(|error| invalid(error.to_string()))?;
    let deadlines = crate::live_monty_settings::hosting_deadlines(settings.execution_limits)
        .map_err(|error| invalid(error.to_string()))?;
    let boot = RootBoot {
        adapter_reserve_bytes,
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
        startup_timeout: deadlines.startup_timeout,
        task_settings,
        max_recipe_contexts: settings.execution_limits.max_recipe_contexts,
    };
    let mut owner = GlobalMontyOwner::start(
        &pool,
        &worker,
        GlobalServiceConfig {
            boot,
            process: ProcessLimits {
                hard_memory_bytes: hard,
                max_frame_bytes: frame,
                response_timeout: deadlines.response_timeout,
            },
            actor: crate::live_monty_settings::actor_limits(settings.execution_limits)
                .map_err(|error| invalid(error.to_string()))?,
            queue_capacity: settings.execution_limits.max_queued_tasks,
            max_pending_settings: settings.execution_limits.max_pending_settings,
            max_retained_attempts: settings.execution_limits.max_retained_attempts,
            queue_bytes: crate::live_monty_settings::admission_limits(settings.execution_limits)
                .map_err(|error| invalid(error.to_string()))?
                .max_bytes,
            ownership: crate::global_monty_owner::OwnershipLimits::from_execution(
                settings.execution_limits,
            ),
            live,
        },
    )
    .await
    .map_err(|error| invalid(error.to_string()))?;
    let measurement_status =
        if settings.memory_policy.mode == brassclaw_product_workflow::MontyMemoryMode::Manual {
            "disabled"
        } else {
            let service = owner.client();
            let selection =
                crate::live_monty_settings::startup_heap_selection(&service, &settings).await;
            let (selected, status) = match selection {
                Ok(selection) => selection,
                Err(error) => {
                    owner.request_shutdown();
                    check_shutdown(
                        owner
                            .join()
                            .await
                            .map_err(|error| invalid(error.to_string()))?,
                    )?;
                    return Err(invalid(error.to_string()));
                }
            };
            if selected != soft {
                let publication = service
                    .publish_heap(
                        1,
                        HeapSettings {
                            revision: 2,
                            max_vm_bytes: selected,
                        },
                        false,
                    )
                    .await;
                if let Err(error) = publication {
                    owner.request_shutdown();
                    check_shutdown(
                        owner
                            .join()
                            .await
                            .map_err(|error| invalid(error.to_string()))?,
                    )?;
                    return Err(invalid(format!(
                        "startup heap publication failed: {error:?}"
                    )));
                }
            }
            if status == "startup_critical"
                && service.publish_memory_backpressure(0, true).await.is_err()
            {
                owner.request_shutdown();
                check_shutdown(
                    owner
                        .join()
                        .await
                        .map_err(|error| invalid(error.to_string()))?,
                )?;
                return Err(invalid("initial memory admission publication failed"));
            }
            status
        };
    Ok((owner, catalogue, settings, measurement_status))
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
