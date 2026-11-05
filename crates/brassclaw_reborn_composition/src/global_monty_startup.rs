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
    process::{ProcessLimits, RootBoot, TaskSettings},
};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_resources::LiveMontyTaskSettings;
use std::{sync::Arc, time::Duration};

/// Creation policy for a controlled pre-readiness executor. This is not a
/// per-task snapshot of the already running global VM's live shared policy.
pub(crate) struct ControlledExecutorConfiguration {
    process: ProcessLimits,
    actor: brassclaw_monty_host::transport_actor::ActorLimits,
    bounds: GlobalBounds,
    capacity: crate::global_recipe_ports::RecipeCapacity,
    task: TaskSettings,
    admission: brassclaw_monty_host::service::AdmissionLimits,
    max_pending_settings: u32,
    max_retained_attempts: u32,
    heap: HeapSettings,
    adapter: usize,
    startup_timeout: Duration,
    max_recipe_contexts: u32,
    bytes: String,
}
impl ControlledExecutorConfiguration {
    /// Pure conversion of the ONE actual startup record. Parser utility
    /// containment is validated separately and never supplies executor memory.
    pub(crate) fn new(
        settings: &brassclaw_product_workflow::MontyVmSettings,
    ) -> Result<Arc<Self>, crate::RebornBuildError> {
        use crate::bootstrap_reply_validation::invalid;
        let limits = settings.execution_limits;
        if settings.revision == 0
            || !(1..=i32::MAX as u64).contains(&settings.max_duration_secs)
            || settings.max_recipes_per_task == 0
        {
            return Err(invalid("controlled executor settings incomplete"));
        }
        let soft = usize::try_from(
            settings
                .max_memory_bytes
                .ok_or_else(|| invalid("Monty heap cap missing"))?,
        )
        .map_err(invalid)?;
        let frame = crate::live_monty_settings::ipc_frame(limits).map_err(invalid)?;
        let adapter = crate::live_monty_settings::adapter_reserve(limits).map_err(invalid)?;
        let hard = frame
            .checked_mul(2)
            .and_then(|frames| soft.checked_add(frames))
            .and_then(|bytes| bytes.checked_add(adapter))
            .filter(|hard| soft > 0 && *hard < usize::MAX)
            .ok_or_else(|| invalid("Monty physical backstop overflow"))?;
        let deadlines = crate::live_monty_settings::hosting_deadlines(limits).map_err(invalid)?;
        let process = ProcessLimits {
            hard_memory_bytes: hard,
            max_frame_bytes: frame,
            response_timeout: deadlines.response_timeout,
        };
        let actor = crate::live_monty_settings::actor_limits(limits).map_err(invalid)?;
        let admission = crate::live_monty_settings::admission_limits(limits).map_err(invalid)?;
        let bounds = GlobalBounds {
            values: crate::live_monty_settings::execution_bounds(limits).map_err(invalid)?,
            workers: limits.max_workers,
            max_pending_calls: usize::try_from(limits.max_pending_host_calls).map_err(invalid)?,
        };
        let task = crate::live_monty_settings::task_settings(settings);
        // A newly created worker's memory epoch is distinct from task/settings revision.
        let heap = HeapSettings {
            revision: 1,
            max_vm_bytes: soft,
        };
        let capacity = crate::global_recipe_ports::RecipeCapacity {
            revision: settings.revision,
            max_recipes: usize::try_from(settings.max_recipes_per_task).map_err(invalid)?,
            data_limits: brassclaw_skills::value_contract::ContractLimits {
                max_depth: bounds.values.max_value_depth,
                max_nodes: bounds.values.max_value_nodes,
                max_bytes: bounds.values.max_value_bytes,
            },
        };
        let settings_bytes = serde_json::to_string(settings).map_err(invalid)?;
        let bytes = serde_json::json!({"format":"controlled-executor-creation/1", "origin":"controlled_bootstrap",
            "settings_bytes":settings_bytes,"settings_checksum":crate::bootstrap_evidence::checksum(&settings_bytes),
            "bounds":bounds,"task":task,"heap":heap,"adapter_reserve_bytes":adapter,
            "process":{"hard_memory_bytes":hard,"max_frame_bytes":frame,"response_timeout":process.response_timeout},
            "actor":{"max_unclaimed":actor.max_unclaimed,"max_reserved_frame_bytes":actor.max_reserved_frame_bytes,
                "max_control_unclaimed":actor.max_control_unclaimed,"max_control_reserved_frame_bytes":actor.max_control_reserved_frame_bytes},
            "startup_timeout":deadlines.startup_timeout,"max_recipe_contexts":limits.max_recipe_contexts,
            "admission":{"max_tasks":admission.max_tasks,"max_bytes":admission.max_bytes},
            "max_pending_settings":limits.max_pending_settings,"max_retained_attempts":limits.max_retained_attempts,
            "recipe_capacity":{"revision":capacity.revision,"max_recipes":capacity.max_recipes,
                "max_depth":capacity.data_limits.max_depth,"max_nodes":capacity.data_limits.max_nodes,"max_bytes":capacity.data_limits.max_bytes}}).to_string();
        Ok(Arc::new(Self {
            process,
            actor,
            bounds,
            capacity,
            task,
            heap,
            adapter,
            admission,
            max_pending_settings: limits.max_pending_settings,
            max_retained_attempts: limits.max_retained_attempts,
            startup_timeout: deadlines.startup_timeout,
            max_recipe_contexts: limits.max_recipe_contexts,
            bytes,
        }))
    }
    pub(crate) fn restore_original(bytes: &str) -> Result<Arc<Self>, crate::RebornBuildError> {
        use crate::bootstrap_reply_validation::invalid;
        let record = brassclaw_skills::value_contract::strict_json(
            bytes,
            brassclaw_skills::component_revision::REVISION_LIMITS,
        )
        .map_err(invalid)?;
        let settings_bytes = record["settings_bytes"]
            .as_str()
            .ok_or_else(|| invalid("original executor settings missing"))?;
        let settings: brassclaw_product_workflow::MontyVmSettings = serde_json::from_value(
            brassclaw_skills::value_contract::strict_json(
                settings_bytes,
                brassclaw_skills::component_revision::REVISION_LIMITS,
            )
            .map_err(invalid)?,
        )
        .map_err(invalid)?;
        if serde_json::to_string(&settings).map_err(invalid)? != settings_bytes {
            return Err(invalid("original executor settings normalized"));
        }
        let original = Self::new(&settings)?;
        if original.exact_bytes() != bytes {
            return Err(invalid("original executor configuration differs"));
        }
        Ok(original)
    }
    pub(crate) fn process(&self) -> ProcessLimits {
        self.process
    }
    pub(crate) fn actor(&self) -> brassclaw_monty_host::transport_actor::ActorLimits {
        self.actor
    }
    pub(crate) fn bounds(&self) -> GlobalBounds {
        self.bounds
    }
    pub(crate) fn capacity(&self) -> crate::global_recipe_ports::RecipeCapacity {
        self.capacity
    }
    pub(crate) fn task(&self) -> TaskSettings {
        self.task
    }
    pub(crate) fn checksum(&self) -> String {
        crate::bootstrap_evidence::checksum(&self.bytes)
    }
    pub(crate) fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn hosting(&self) -> brassclaw_monty_host::service::ServiceHostingLimits {
        brassclaw_monty_host::service::ServiceHostingLimits {
            max_workers: None,
            max_pending_host_calls: None,
            max_frame_bytes: None,
            adapter_reserve_bytes: None,
            admission: self.admission,
            max_pending_settings: self.max_pending_settings,
            max_retained_attempts: self.max_retained_attempts,
            actor: None,
            deadlines: None,
        }
    }
    pub(crate) fn boot(
        &self,
        root: &brassclaw_skills::orchestrator_contract::GlobalRootDefinition,
    ) -> RootBoot {
        RootBoot {
            source: root.source().to_owned(),
            checksum: root.source_checksum(),
            aliases: root.ports().clone(),
            bounds: self.bounds,
            heap_settings: Some(self.heap),
            adapter_reserve_bytes: self.adapter,
            startup_timeout: self.startup_timeout,
            task_settings: self.task,
            max_recipe_contexts: self.max_recipe_contexts,
        }
    }
}
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
    primitive_settings: &Arc<crate::host_primitive_settings::AcknowledgedPrimitiveSettings>,
    source_inspection: &crate::source_inspection::ContainedSourceInspection,
    resource_store_settings: &Arc<
        crate::resource_store_settings::AcknowledgedResourceStoreSettings,
    >,
) -> Result<
    (
        GlobalMontyOwner,
        Arc<InstalledMontyCatalogue>,
        brassclaw_product_workflow::MontyVmSettings,
        &'static str,
    ),
    RebornRuntimeError,
> {
    let root = Arc::new(
        crate::global_root_seed::retain_packaged_global_root(
            pool.clone(),
            crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
        )
        .await
        .map_err(|error| invalid(error.to_string()))?,
    );
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
    crate::live_monty_settings::validate_host_defaults(settings.execution_limits)
        .map_err(|error| invalid(error.to_string()))?;
    resource_store_settings
        .prepare_bootstrap(
            settings.revision,
            settings.execution_limits.resource_store_timeout_millis,
        )
        .map_err(invalid)?;
    // Bootstrap qualification itself uses JSON. It must see exactly the same
    // selected revision as the catalogue/VM, before the live owner exists.
    // Ingress remains disabled until runtime binds the acknowledged live view.
    primitive_settings
        .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
            revision: settings.revision,
            limits: settings.execution_limits,
        })
        .map_err(|error| invalid(error.to_string()))?;
    source_inspection
        .bind(primitive_settings.clone())
        .await
        .map_err(|error| invalid(error.to_string()))?;
    // Source facts, catalogue inspection and the ordinary global process share
    // the already retained installation image. Never reopen the mutable installed
    // pathname after qualification or lose the copy during abandoned startup.
    let worker_image = source_inspection
        .prepare()
        .await
        .map_err(|_| invalid("Monty retained worker preparation unavailable"))?
        .worker;
    let executor = ControlledExecutorConfiguration::new(&settings)
        .map_err(|error| invalid(error.to_string()))?;
    let catalogue = InstalledMontyCatalogue::boot(
        pool.clone(),
        kernel.clone(),
        worker_image,
        scope,
        mounts,
        crate::installed_monty_catalogue::InstalledCatalogueStartup {
            root: root.clone(),
            settings: &settings,
            executor: executor.clone(),
            resource_store: resource_store_settings.clone(),
        },
    )
    .await
    .map_err(|error| invalid(error.to_string()))?;
    let task_settings = executor.task;
    let live = LiveMontyTaskSettings::new(task_settings.into())
        .map_err(|error| invalid(error.to_string()))?;
    kernel
        .bind_task_budget_settings(live.clone())
        .map_err(|error| invalid(error.to_string()))?;
    // Start with the configured finite value. Startup sizing is applied to the
    // actual worker before ingress; manual startup retains the upgraded value.
    // This finite physical backstop is independent of task executing time.
    let soft = executor.heap.max_vm_bytes;
    let boot = executor.boot(&root);
    let mut owner = GlobalMontyOwner::start(
        &pool,
        catalogue.worker_image.clone(),
        GlobalServiceConfig {
            boot,
            process: executor.process,
            actor: executor.actor,
            queue_capacity: executor.admission.max_tasks,
            max_pending_settings: executor.max_pending_settings,
            max_retained_attempts: executor.max_retained_attempts,
            queue_bytes: executor.admission.max_bytes,
            ownership: crate::global_monty_owner::OwnershipLimits::from_execution(
                settings.execution_limits,
            ),
            live,
        },
    )
    .await
    .map_err(|error| invalid(error.to_string()))?;
    tracing::info!(worker_checksum=%hex::encode(owner.worker_image().checksum()),
        "global Monty retained worker started");
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
