//! Durable instance task-budget publication through the existing global service.
//! The owner survives HTTP cancellation; desired edits never masquerade as uptake.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use brassclaw_host_api::MontyExecutionLimits;
use brassclaw_monty_host::{
    VmBounds,
    heap::HeapSettings,
    process::TaskSettings,
    service::{AdmissionLimits, HeapCommitOutcome, ServiceClient, ServiceFailure},
    transport_actor::{ActorLimits, HostingDeadlines},
};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    MontyBudgetUptake, MontyExecutionLimitsStatus, MontyMemoryBudgetStatus, MontyMemoryMode,
    MontyMemoryPolicy, MontyRecipeBudgetStatus, MontyTaskBudgetStatus, MontyVmSettings,
    MontyVmSettingsError, MontyVmSettingsStore, MontyVmState, MontyVmStatusResponse,
    UpdateMontyVmSettingsRequest,
};
use brassclaw_resources::{AdaptiveMontyHeapBudget, MontyHeapBudgetConfig};
use tokio::{
    sync::{Mutex, Notify, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{global_monty_owner::GlobalOwnerCheck, pg_monty_vm_settings::PgMontyVmSettingsStore};

#[derive(Clone, Copy)]
struct SettingsDeadlines {
    source: Duration,
    uptake: Duration,
    cancellation: Duration,
    reconcile_interval: Duration,
    status_poll_interval_millis: u32,
}
impl SettingsDeadlines {
    fn from_limits(limits: MontyExecutionLimits) -> Self {
        Self {
            source: Duration::from_millis(limits.settings_source_timeout_millis),
            uptake: Duration::from_millis(limits.settings_uptake_timeout_millis),
            cancellation: Duration::from_millis(limits.cancellation_ack_timeout_millis),
            reconcile_interval: Duration::from_millis(limits.settings_reconcile_interval_millis),
            status_poll_interval_millis: limits.status_poll_interval_millis,
        }
    }
    fn validate(self) -> Result<(), MontyVmSettingsError> {
        let now = Instant::now();
        if self.source.is_zero()
            || self.uptake.is_zero()
            || now.checked_add(self.source).is_none()
            || now.checked_add(self.uptake).is_none()
            || self.cancellation.is_zero()
            || now.checked_add(self.cancellation).is_none()
            || self.reconcile_interval.is_zero()
            || now.checked_add(self.reconcile_interval).is_none()
        {
            return Err(MontyVmSettingsError::Invalid(
                "settings deadlines exceed the host clock representation".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Observation {
    effective: MontyVmSettings,
    // Only classified host-owned diagnostics; no DB/provider error strings.
    failure: Option<(Option<u64>, &'static str)>,
    memory_revision: u64,
    measurement_status: &'static str,
    memory_sample_count: u64,
    last_memory_sample: Option<Instant>,
}

struct MemoryControl {
    configured: Option<u64>,
    policy: MontyMemoryPolicy,
    settings_revision: u64,
    last_critical: bool,
    last_sample: Option<Instant>,
    measurement_status: &'static str,
    memory_sample_count: u64,
    inflight_probe: Option<
        JoinHandle<
            Result<
                brassclaw_resources::MontyMemorySample,
                brassclaw_host_runtime::HostMemoryProbeError,
            >,
        >,
    >,
}

pub(crate) struct LiveMontySettingsStore {
    desired: PgMontyVmSettingsStore,
    service: ServiceClient,
    ownership: GlobalOwnerCheck,
    wake: Arc<Notify>,
    edit: Arc<Mutex<MemoryControl>>,
    observed: watch::Sender<Observation>,
    closed: AtomicBool,
}

pub(crate) struct MontySettingsOwner {
    store: Arc<LiveMontySettingsStore>,
    cancel: CancellationToken,
    join: Option<JoinHandle<()>>,
}
impl MontySettingsOwner {
    pub(crate) fn start(
        pool: Arc<PgPool>,
        service: ServiceClient,
        ownership: GlobalOwnerCheck,
        initial: MontyVmSettings,
        measurement_status: &'static str,
    ) -> Result<Self, MontyVmSettingsError> {
        let owner_policy = ownership.snapshot().map_err(|_| {
            MontyVmSettingsError::Internal("ownership accounting unavailable".into())
        })?;
        if owner_policy.revision != initial.revision
            || owner_policy.limits
                != crate::global_monty_owner::OwnershipLimits::from_execution(
                    initial.execution_limits,
                )
            || task_settings(&initial) != TaskSettings::from(service.live_task_settings().current())
            || execution_bounds(initial.execution_limits)? != service.vm_bounds()
            || adapter_reserve(initial.execution_limits)?
                != service.allocator_status().adapter_reserve_bytes
            || initial.execution_limits.max_recipe_contexts
                != service.recipe_context_capacity().limit
            || admission_limits(initial.execution_limits)? != service.admission_observation().limits
            || initial.execution_limits.max_pending_settings != service.settings_capacity().limit
            || initial.execution_limits.max_retained_attempts != service.retained_attempts().limit
            || hosting_deadlines(initial.execution_limits)?
                != service.hosting_deadlines().map_err(|_| {
                    MontyVmSettingsError::Internal("hosting accounting unavailable".into())
                })?
            || actor_limits(initial.execution_limits)?
                != service
                    .actor_capacity()
                    .map_err(|_| {
                        MontyVmSettingsError::Internal("actor accounting unavailable".into())
                    })?
                    .limits
        {
            return Err(MontyVmSettingsError::Internal(
                "initial task revision mismatch".into(),
            ));
        }
        let last_sample =
            (initial.memory_policy.mode == MontyMemoryMode::Automatic).then(Instant::now);
        let edit = Arc::new(Mutex::new(MemoryControl {
            configured: initial.max_memory_bytes,
            policy: initial.memory_policy,
            settings_revision: initial.revision,
            last_critical: measurement_status == "startup_critical",
            last_sample,
            measurement_status,
            memory_sample_count: u64::from(initial.memory_policy.mode != MontyMemoryMode::Manual),
            inflight_probe: None,
        }));
        let (observed, _) = watch::channel(Observation {
            last_memory_sample: last_sample,
            memory_revision: initial.revision,
            memory_sample_count: u64::from(initial.memory_policy.mode != MontyMemoryMode::Manual),
            measurement_status,
            effective: initial,
            failure: None,
        });
        let store = Arc::new(LiveMontySettingsStore {
            desired: PgMontyVmSettingsStore::new(pool, "default", "default"),
            service,
            ownership,
            wake: Arc::new(Notify::new()),
            edit,
            observed,
            closed: AtomicBool::new(false),
        });
        let cancel = CancellationToken::new();
        let join = tokio::spawn(reconcile_loop(store.clone(), cancel.clone()));
        Ok(Self {
            store,
            cancel,
            join: Some(join),
        })
    }
    pub(crate) fn store(&self) -> Arc<LiveMontySettingsStore> {
        self.store.clone()
    }
    pub(crate) async fn shutdown(mut self) -> Result<(), MontyVmSettingsError> {
        self.store.closed.store(true, Ordering::Release);
        self.cancel.cancel();
        if let Some(join) = self.join.take() {
            join.await
                .map_err(|_| MontyVmSettingsError::Internal("settings owner failed".into()))?;
        }
        // Accepted edits already own the mutex before being spawned. Wait for
        // their bounded persistence/publication before the VM owner shuts down.
        drop(self.store.edit.lock().await);
        Ok(())
    }
}
impl Drop for MontySettingsOwner {
    fn drop(&mut self) {
        self.store.closed.store(true, Ordering::Release);
        self.cancel.cancel();
    }
}

/// One measurement at startup or an explicit startup-policy edit. No timer.
pub(crate) async fn startup_heap_selection(
    service: &ServiceClient,
    settings: &MontyVmSettings,
) -> Result<(usize, &'static str), MontyVmSettingsError> {
    let deadlines = SettingsDeadlines::from_limits(settings.execution_limits);
    deadlines.validate()?;
    startup_heap_selection_with_deadline(service, settings, deadlines.source).await
}

async fn startup_heap_selection_with_deadline(
    service: &ServiceClient,
    settings: &MontyVmSettings,
    source_deadline: Duration,
) -> Result<(usize, &'static str), MontyVmSettingsError> {
    let fallback = settings
        .max_memory_bytes
        .and_then(|bytes| usize::try_from(bytes).ok())
        .filter(|bytes| *bytes >= brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES as usize)
        .ok_or_else(|| {
            MontyVmSettingsError::Invalid(
                "startup heap must be at least the 512 MiB default".into(),
            )
        })?;
    let pid = service
        .worker_process_id()
        .ok_or_else(|| MontyVmSettingsError::Unavailable("worker identity unavailable".into()))?;
    let sample = tokio::time::timeout(
        source_deadline,
        tokio::task::spawn_blocking(move || {
            brassclaw_host_runtime::sample_process_memory_capacity(pid)
        }),
    )
    .await;
    let Ok(Ok(Ok(sample))) = sample else {
        tracing::warn!("Monty startup capacity unavailable; retaining configured finite budget");
        return Ok((fallback, "startup_fallback"));
    };
    let observed = service.heap_observation();
    let capacity = (observed.vm_live_bytes as u64)
        .checked_add(sample.additional_capacity_bytes)
        .ok_or_else(|| MontyVmSettingsError::Unavailable("startup capacity overflow".into()))?
        .saturating_sub(settings.memory_policy.reserve_bytes);
    let sized = (fallback as u64).min(capacity);
    let selected = if settings.memory_policy.mode == MontyMemoryMode::Automatic {
        sized
    } else {
        sized.max(brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES)
    };
    if selected == 0 || selected < observed.vm_live_bytes as u64 {
        return Err(MontyVmSettingsError::Invalid(
            "insufficient memory for startup reserve".into(),
        ));
    }
    Ok((
        usize::try_from(selected)
            .map_err(|_| MontyVmSettingsError::Invalid("startup heap out of range".into()))?,
        if settings.memory_policy.mode == MontyMemoryMode::Automatic
            && sample.pressure == brassclaw_resources::MontyMemoryPressure::Critical
        {
            "startup_critical"
        } else if sized < selected {
            "startup_floor"
        } else {
            "startup_sized"
        },
    ))
}

pub(crate) fn execution_bounds(
    limits: MontyExecutionLimits,
) -> Result<VmBounds, MontyVmSettingsError> {
    limits
        .validate()
        .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
    SettingsDeadlines::from_limits(limits).validate()?;
    if !crate::global_monty_owner::OwnershipLimits::from_execution(limits).valid() {
        return Err(MontyVmSettingsError::Invalid(
            "ownership controls exceed the host clock representation".into(),
        ));
    }
    let native = |value: u64| {
        usize::try_from(value).map_err(|_| {
            MontyVmSettingsError::Invalid("execution limit exceeds native representation".into())
        })
    };
    Ok(VmBounds {
        max_source_bytes: native(limits.max_source_bytes)?,
        max_compiled_source_bytes: native(limits.max_compiled_source_bytes)?,
        max_feeds: native(limits.max_feeds)?,
        max_stdout_bytes: native(limits.max_stdout_bytes)?,
        execution_slice: Duration::from_millis(limits.execution_slice_millis),
        max_value_depth: native(u64::from(limits.max_value_depth))?,
        max_value_nodes: native(limits.max_value_nodes)?,
        max_value_bytes: native(limits.max_value_bytes)?,
    })
}

pub(crate) fn adapter_reserve(limits: MontyExecutionLimits) -> Result<usize, MontyVmSettingsError> {
    usize::try_from(limits.worker_adapter_reserve_bytes).map_err(|_| {
        MontyVmSettingsError::Invalid("adapter reserve exceeds native representation".into())
    })
}
pub(crate) fn admission_limits(
    limits: MontyExecutionLimits,
) -> Result<AdmissionLimits, MontyVmSettingsError> {
    limits
        .validate()
        .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
    Ok(AdmissionLimits {
        max_tasks: limits.max_queued_tasks,
        max_bytes: usize::try_from(limits.max_queued_bytes).map_err(|_| {
            MontyVmSettingsError::Invalid("queued bytes exceed native representation".into())
        })?,
    })
}

pub(crate) fn actor_limits(
    limits: MontyExecutionLimits,
) -> Result<ActorLimits, MontyVmSettingsError> {
    limits
        .validate()
        .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
    let native = |value| {
        usize::try_from(value).map_err(|_| {
            MontyVmSettingsError::Invalid("actor capacity exceeds native representation".into())
        })
    };
    Ok(ActorLimits {
        max_unclaimed: native(u64::from(limits.max_actor_requests))?,
        max_reserved_frame_bytes: native(limits.max_actor_reserved_bytes)?,
        max_control_unclaimed: native(u64::from(limits.max_actor_control_requests))?,
        max_control_reserved_frame_bytes: native(limits.max_actor_control_reserved_bytes)?,
    })
}

pub(crate) fn hosting_deadlines(
    limits: MontyExecutionLimits,
) -> Result<HostingDeadlines, MontyVmSettingsError> {
    limits
        .validate()
        .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
    let deadlines = HostingDeadlines {
        startup_timeout: Duration::from_millis(limits.startup_timeout_millis),
        response_timeout: Duration::from_millis(limits.response_timeout_millis),
    };
    if !deadlines.valid() {
        return Err(MontyVmSettingsError::Invalid(
            "hosting deadlines exceed the host clock representation".into(),
        ));
    }
    Ok(deadlines)
}

fn observed_limits(
    resources: (VmBounds, brassclaw_monty_host::heap::AllocatorStatus),
    max_recipe_contexts: u32,
    admission: AdmissionLimits,
    max_pending_settings: u32,
    max_retained_attempts: u32,
    actor: ActorLimits,
    deadlines: (
        HostingDeadlines,
        SettingsDeadlines,
        crate::global_monty_owner::OwnershipLimits,
    ),
) -> MontyExecutionLimits {
    let (bounds, allocator) = resources;
    let (deadlines, settings_deadlines, ownership) = deadlines;
    MontyExecutionLimits {
        worker_adapter_reserve_bytes: allocator.adapter_reserve_bytes as u64,
        max_recipe_contexts,
        max_queued_tasks: admission.max_tasks,
        max_queued_bytes: admission.max_bytes as u64,
        max_pending_settings,
        max_retained_attempts,
        max_actor_requests: actor.max_unclaimed as u32,
        max_actor_reserved_bytes: actor.max_reserved_frame_bytes as u64,
        max_actor_control_requests: actor.max_control_unclaimed as u32,
        max_actor_control_reserved_bytes: actor.max_control_reserved_frame_bytes as u64,
        startup_timeout_millis: deadlines.startup_timeout.as_millis() as u64,
        response_timeout_millis: deadlines.response_timeout.as_millis() as u64,
        settings_source_timeout_millis: settings_deadlines.source.as_millis() as u64,
        settings_uptake_timeout_millis: settings_deadlines.uptake.as_millis() as u64,
        cancellation_ack_timeout_millis: settings_deadlines.cancellation.as_millis() as u64,
        settings_reconcile_interval_millis: settings_deadlines.reconcile_interval.as_millis()
            as u64,
        status_poll_interval_millis: settings_deadlines.status_poll_interval_millis,
        ownership_check_timeout_millis: ownership.check_timeout.as_millis() as u64,
        ownership_heartbeat_interval_millis: ownership.heartbeat_interval.as_millis() as u64,
        max_pending_ownership_checks: ownership.max_pending_checks,
        max_source_bytes: bounds.max_source_bytes as u64,
        max_compiled_source_bytes: bounds.max_compiled_source_bytes as u64,
        max_feeds: bounds.max_feeds as u64,
        max_stdout_bytes: bounds.max_stdout_bytes as u64,
        execution_slice_millis: bounds.execution_slice.as_millis() as u64,
        max_value_depth: bounds.max_value_depth as u32,
        max_value_nodes: bounds.max_value_nodes as u64,
        max_value_bytes: bounds.max_value_bytes as u64,
    }
}

fn task_settings(settings: &MontyVmSettings) -> TaskSettings {
    TaskSettings {
        revision: settings.revision,
        max_compute_time: Duration::from_secs(settings.max_duration_secs),
        token_budgets_enabled: settings.token_budgets_enabled,
    }
}

impl LiveMontySettingsStore {
    fn deadlines(&self) -> SettingsDeadlines {
        SettingsDeadlines::from_limits(self.observed.borrow().effective.execution_limits)
    }
    /// Internal read port for retrieval. It returns the complete acknowledged
    /// snapshot, not a newer desired token mode or prior-knowledge ceiling.
    pub(crate) fn effective_view(self: &Arc<Self>) -> Arc<dyn MontyVmSettingsStore> {
        Arc::new(EffectiveSettings(self.clone()))
    }

    pub(crate) fn recipe_capacity_source(
        self: &Arc<Self>,
    ) -> Arc<dyn crate::global_recipe_ports::RecipeCapacitySource> {
        // A retained task must not own the service that retains its ports.
        // These observations/accounts have no service/transport ownership.
        Arc::new(EffectiveRecipeCapacity {
            observed: self.observed.subscribe(),
            live: self.service.live_task_settings(),
            ownership: self.ownership.clone(),
        })
    }

    pub(crate) fn cancellation_policy_source(
        &self,
    ) -> Arc<dyn crate::global_monty_driver::CancellationAckSource> {
        // This receiver does not own the service retaining the task factory.
        // Last acknowledged policy remains usable to settle a failed service.
        Arc::new(EffectiveCancellationPolicy {
            observed: self.observed.subscribe(),
        })
    }

    async fn read_desired(&self) -> Result<MontyVmSettings, MontyVmSettingsError> {
        self.read_desired_with_deadline(self.deadlines().source)
            .await
    }

    async fn read_desired_with_deadline(
        &self,
        deadline: Duration,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        let settings = tokio::time::timeout(deadline, self.desired.get("default", "default"))
            .await
            .map_err(|_| MontyVmSettingsError::Unavailable("settings source deadline".into()))??;
        if settings.revision == 0 {
            return Err(MontyVmSettingsError::Unavailable(
                "instance settings row missing".into(),
            ));
        }
        Ok(settings)
    }

    async fn reconcile(&self) -> Result<(), (Option<u64>, &'static str)> {
        // Capture the effective policy before any await. A subsequent edit
        // cannot retime this accepted reconciliation or its readback/ACK waits.
        let acknowledged = self.observed.borrow().effective.clone();
        let deadlines = SettingsDeadlines::from_limits(acknowledged.execution_limits);
        let mut memory = tokio::time::timeout(deadlines.source, self.edit.lock())
            .await
            .map_err(|_| (None, "settings_edit_lock_deadline"))?;
        let desired = self
            .read_desired_with_deadline(deadlines.source)
            .await
            .map_err(|_| (None, "settings_source_unavailable"))?;
        let revision = Some(desired.revision);
        if desired.revision < acknowledged.revision {
            return Err((revision, "settings_revision_regressed"));
        }
        if desired.revision == acknowledged.revision && !snapshots_equal(&desired, &acknowledged) {
            return Err((revision, "settings_revision_conflict"));
        }
        let effective = self.service.live_task_settings().current();
        let owner_policy = self
            .ownership
            .snapshot()
            .map_err(|_| (revision, "ownership_accounting_unavailable"))?;
        let owner_limits =
            crate::global_monty_owner::OwnershipLimits::from_execution(desired.execution_limits);
        if owner_policy.revision < acknowledged.revision
            || owner_policy.revision > effective.revision
            || owner_policy.revision > desired.revision
            || (owner_policy.revision == desired.revision && owner_policy.limits != owner_limits)
        {
            return Err((revision, "ownership_revision_conflict"));
        }
        let values = execution_bounds(desired.execution_limits)
            .map_err(|_| (revision, "invalid_execution_limits"))?;
        let reserve = adapter_reserve(desired.execution_limits)
            .map_err(|_| (revision, "invalid_adapter_reserve"))?;
        self.service
            .validate_adapter_reserve(reserve)
            .map_err(|_| (revision, "unsupported_adapter_reserve"))?;
        let hosting = hosting_deadlines(desired.execution_limits)
            .map_err(|_| (revision, "invalid_hosting_deadlines"))?;
        self.service
            .validate_runtime_bounds(values, hosting.response_timeout)
            .map_err(|_| (revision, "unsupported_execution_limits"))?;
        let actor = actor_limits(desired.execution_limits)
            .map_err(|_| (revision, "invalid_actor_limits"))?;
        self.service
            .validate_actor_limits(actor)
            .map_err(|_| (revision, "unsupported_actor_limits"))?;
        if desired.revision < effective.revision {
            return Err((revision, "settings_revision_regressed"));
        }
        if desired.revision == effective.revision
            && (hosting
                != self
                    .service
                    .hosting_deadlines()
                    .map_err(|_| (revision, "hosting_accounting_unavailable"))?
                || task_settings(&desired) != TaskSettings::from(effective)
                || values != self.service.vm_bounds()
                || reserve != self.service.allocator_status().adapter_reserve_bytes
                || desired.execution_limits.max_recipe_contexts
                    != self.service.recipe_context_capacity().limit
                || admission_limits(desired.execution_limits)
                    .map_err(|_| (revision, "invalid_admission_limits"))?
                    != self.service.admission_observation().limits
                || desired.execution_limits.max_pending_settings
                    != self.service.settings_capacity().limit
                || desired.execution_limits.max_retained_attempts
                    != self.service.retained_attempts().limit
                || actor
                    != self
                        .service
                        .actor_capacity()
                        .map_err(|_| (revision, "actor_accounting_unavailable"))?
                        .limits)
        {
            return Err((revision, "settings_revision_conflict"));
        }
        // Validate the complete desired revision before any heap/worker effect.
        self.ownership
            .check()
            .await
            .map_err(|_| (revision, "runtime_unavailable"))?;
        self.reconcile_memory(&desired, &mut memory, deadlines)
            .await
            .map_err(|reason| (revision, reason))?;
        if desired.revision != effective.revision {
            tokio::time::timeout(
                deadlines.uptake,
                self.service.publish_control_settings(
                    effective.revision,
                    task_settings(&desired),
                    values,
                    desired.execution_limits.max_recipe_contexts,
                    brassclaw_monty_host::service::ServiceHostingLimits {
                        adapter_reserve_bytes: Some(reserve),
                        admission: admission_limits(desired.execution_limits)
                            .map_err(|_| (revision, "invalid_admission_limits"))?,
                        max_pending_settings: desired.execution_limits.max_pending_settings,
                        max_retained_attempts: desired.execution_limits.max_retained_attempts,
                        actor: Some(actor),
                        deadlines: Some(hosting),
                    },
                ),
            )
            .await
            .map_err(|_| (revision, "settings_publication_deadline"))?
            .map_err(|_| (revision, "settings_publication_failed"))?;
        }
        // The service has acknowledged the VM/Rust controls. Publish the exact
        // ownership successor before exposing the full effective snapshot. A
        // timed-out earlier worker ACK can settle this same desired revision.
        if owner_policy.revision != desired.revision {
            self.ownership
                .publish(owner_policy.revision, desired.revision, owner_limits)
                .map_err(|_| (revision, "ownership_publication_failed"))?;
        }
        // The service publishes the shared Rust revision only after its real
        // worker acknowledges the same limits. Keep retrieval's whole snapshot.
        self.observed.send_replace(Observation {
            last_memory_sample: memory.last_sample,
            memory_revision: memory.settings_revision,
            measurement_status: memory.measurement_status,
            memory_sample_count: memory.memory_sample_count,
            effective: desired,
            failure: None,
        });
        Ok(())
    }

    async fn reconcile_memory(
        &self,
        desired: &MontyVmSettings,
        memory: &mut MemoryControl,
        deadlines: SettingsDeadlines,
    ) -> Result<(), &'static str> {
        let changed = memory.configured != desired.max_memory_bytes;
        let observed_heap = self.service.heap_observation();
        let restore_manual = desired.memory_policy.mode == MontyMemoryMode::Manual
            && (memory.policy.mode != MontyMemoryMode::Manual
                || observed_heap
                    .status
                    .effective
                    .map(|heap| heap.max_vm_bytes as u64)
                    != desired.max_memory_bytes);
        let restore_floor = desired.memory_policy.mode == MontyMemoryMode::Startup
            && observed_heap.status.effective.is_some_and(|heap| {
                (heap.max_vm_bytes as u64) < brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES
            });
        if changed || restore_manual || restore_floor {
            let bytes = desired.max_memory_bytes.ok_or("heap_limit_missing")?;
            let heap = self.service.heap_observation();
            let expected = heap.status.desired_revision();
            let next = HeapSettings {
                revision: expected.checked_add(1).ok_or("heap_revision_exhausted")?,
                max_vm_bytes: usize::try_from(bytes).map_err(|_| "heap_limit_out_of_range")?,
            };
            tokio::time::timeout(
                deadlines.uptake,
                self.service.publish_heap(expected, next, false),
            )
            .await
            .map_err(|_| "heap_publication_deadline")?
            .map_err(|_| "heap_publication_failed")?;
        }
        if memory.policy != desired.memory_policy {
            memory.last_sample = None;
        }
        memory.configured = desired.max_memory_bytes;
        memory.policy = desired.memory_policy;
        memory.settings_revision = desired.revision;
        if desired.memory_policy.mode != MontyMemoryMode::Automatic {
            // No OS reads in startup/manual mode after startup. Clear only the
            // optional controller's pause; pending worker reductions stay fenced.
            memory.last_critical = false;
            let admission = self.service.memory_admission_policy();
            if admission.backpressure {
                tokio::time::timeout(
                    deadlines.uptake,
                    self.service
                        .publish_memory_backpressure(admission.revision, false),
                )
                .await
                .map_err(|_| "memory_admission_publication_deadline")?
                .map_err(|_| "memory_admission_publication_failed")?;
            }
            if desired.memory_policy.mode == MontyMemoryMode::Manual {
                memory.measurement_status = "disabled";
            }
            return Ok(());
        }
        let interval = Duration::from_secs(desired.memory_policy.sample_interval_secs);
        let now = Instant::now();
        if memory
            .last_sample
            .is_some_and(|last| now.duration_since(last) < interval)
        {
            return Ok(());
        }
        memory.last_sample = Some(now);
        let pid = self
            .service
            .worker_process_id()
            .ok_or("worker_identity_unavailable")?;
        if memory.inflight_probe.is_none() {
            memory.memory_sample_count = memory
                .memory_sample_count
                .checked_add(1)
                .ok_or("memory_sample_counter_exhausted")?;
            memory.inflight_probe = Some(tokio::task::spawn_blocking(move || {
                brassclaw_host_runtime::sample_process_memory_capacity(pid)
            }));
        }
        // A timed-out read stays owned; never accumulate blocking probes.
        let sample = tokio::time::timeout(
            deadlines.source,
            memory
                .inflight_probe
                .as_mut()
                .ok_or("memory_probe_owner_missing")?,
        )
        .await;
        if sample.is_ok() {
            memory.inflight_probe.take();
        }
        let sample = match sample {
            Ok(Ok(Ok(sample)))
                if Instant::now()
                    .checked_duration_since(sample.measured_at)
                    .is_some_and(|age| age <= interval.saturating_mul(2)) =>
            {
                memory.measurement_status = "available";
                Some(sample)
            }
            _ => {
                memory.measurement_status = "unavailable";
                None
            }
        };
        self.ownership
            .check()
            .await
            .map_err(|_| "runtime_unavailable")?;
        let heap = self.service.heap_observation();
        let effective = heap
            .status
            .effective
            .ok_or("heap_observation_unavailable")?;
        let config = MontyHeapBudgetConfig {
            reserve_bytes: desired.memory_policy.reserve_bytes,
            manual_ceiling_bytes: desired.max_memory_bytes,
            growth_step_bytes: desired.memory_policy.growth_step_bytes,
            growth_headroom_bytes: desired.memory_policy.growth_headroom_bytes,
            max_sample_age: interval.saturating_mul(2),
            fallback_bytes: effective.max_vm_bytes as u64,
        };
        let mut budget =
            AdaptiveMontyHeapBudget::new(config).map_err(|_| "invalid_memory_policy")?;
        budget
            .acknowledge(effective.max_vm_bytes as u64, heap.vm_live_bytes as u64)
            .map_err(|_| "heap_accounting_failed")?;
        if let Some(sample) = sample {
            memory.last_critical =
                sample.pressure == brassclaw_resources::MontyMemoryPressure::Critical;
        }
        let decision = budget
            .evaluate(heap.vm_live_bytes as u64, sample, Instant::now())
            .map_err(|_| "heap_accounting_failed")?;
        let admission = self.service.memory_admission_policy();
        let paused = decision.backpressure || memory.last_critical;
        if paused != admission.backpressure {
            tokio::time::timeout(
                deadlines.uptake,
                self.service
                    .publish_memory_backpressure(admission.revision, paused),
            )
            .await
            .map_err(|_| "memory_admission_publication_deadline")?
            .map_err(|_| "memory_admission_publication_failed")?;
        }
        let target = if decision.pending_reduction {
            decision.target_bytes.max(1)
        } else {
            decision.proposed_bytes
        };
        if heap
            .status
            .desired
            .is_none_or(|previous| previous.max_vm_bytes as u64 != target)
        {
            let expected = heap.status.desired_revision();
            tokio::time::timeout(
                deadlines.uptake,
                self.service.publish_heap(
                    expected,
                    HeapSettings {
                        revision: expected.checked_add(1).ok_or("heap_revision_exhausted")?,
                        max_vm_bytes: usize::try_from(target)
                            .map_err(|_| "heap_limit_out_of_range")?,
                    },
                    true,
                ),
            )
            .await
            .map_err(|_| "heap_publication_deadline")?
            .map_err(|_| "heap_publication_failed")?;
        }
        Ok(())
    }

    fn observation(&self, desired: &MontyVmSettings) -> MontyVmStatusResponse {
        let observation = self.observed.borrow().clone();
        let settings_deadlines =
            SettingsDeadlines::from_limits(observation.effective.execution_limits);
        let effective = self.service.live_task_settings().current();
        let failed = self.observed.borrow().failure;
        let actor = self.service.actor_capacity().ok();
        let deadlines = self.service.hosting_deadlines().ok();
        let ownership = self.ownership.snapshot().ok();
        let closed = self.ownership.is_closed()
            || actor.is_none()
            || deadlines.is_none()
            || ownership.is_none();
        let failure = if closed {
            Some("runtime_unavailable")
        } else {
            failed
                .filter(|(revision, _)| revision.is_none_or(|r| r == desired.revision))
                .map(|(_, reason)| reason)
        };
        let applied = task_settings(desired) == TaskSettings::from(effective);
        let uptake = if closed {
            MontyBudgetUptake::Failed
        } else if applied {
            MontyBudgetUptake::Applied
        } else if failure.is_some() {
            MontyBudgetUptake::Failed
        } else {
            MontyBudgetUptake::Pending
        };
        let contexts = self.service.recipe_context_capacity();
        let admission = self.service.admission_observation();
        let settings_capacity = self.service.settings_capacity();
        let retained_attempts = self.service.retained_attempts();
        let allocator = self.service.allocator_status();
        let hosting = actor.zip(deadlines).zip(ownership);
        let limits = hosting.map(|((actor, deadlines), ownership)| {
            observed_limits(
                (self.service.vm_bounds(), allocator),
                contexts.limit,
                admission.limits,
                settings_capacity.limit,
                retained_attempts.limit,
                actor.limits,
                (deadlines, settings_deadlines, ownership.limits),
            )
        });
        let limits_uptake = if closed {
            MontyBudgetUptake::Failed
        } else if desired.revision == effective.revision
            && ownership.is_some_and(|ownership| ownership.revision == desired.revision)
            && Some(desired.execution_limits) == limits
        {
            MontyBudgetUptake::Applied
        } else if failure.is_some() {
            MontyBudgetUptake::Failed
        } else {
            MontyBudgetUptake::Pending
        };
        let memory = self.service.heap_observation();
        let effective_heap = memory.status.effective;
        let memory_uptake = if closed {
            MontyBudgetUptake::Failed
        } else if observation.memory_revision == desired.revision
            && !memory.status.pending_reduction
        {
            MontyBudgetUptake::Applied
        } else if failure.is_some() {
            MontyBudgetUptake::Failed
        } else {
            MontyBudgetUptake::Pending
        };
        MontyVmStatusResponse {
            recipe_budget: Some(MontyRecipeBudgetStatus {
                desired_revision: desired.revision,
                effective_revision: observation.effective.revision,
                max_recipes_per_task: observation.effective.max_recipes_per_task,
                uptake: if closed {
                    MontyBudgetUptake::Failed
                } else if observation.effective.revision == desired.revision
                    && observation.effective.max_recipes_per_task == desired.max_recipes_per_task
                    && task_settings(&observation.effective) == TaskSettings::from(effective)
                {
                    MontyBudgetUptake::Applied
                } else if failure.is_some() {
                    MontyBudgetUptake::Failed
                } else {
                    MontyBudgetUptake::Pending
                },
                failure_reason: failure.map(str::to_owned),
            }),
            memory_budget: Some(MontyMemoryBudgetStatus {
                mode: observation.effective.memory_policy.mode,
                desired_settings_revision: desired.revision,
                effective_settings_revision: observation.memory_revision,
                effective_heap_revision: effective_heap.map_or(0, |heap| heap.revision),
                max_memory_bytes: effective_heap.map_or(0, |heap| heap.max_vm_bytes as u64),
                live_heap_bytes: memory.vm_live_bytes as u64,
                pending_reduction: memory.status.pending_reduction,
                admission_paused: self.service.memory_admission_policy().backpressure
                    || memory.status.pending_reduction,
                measurement_status: observation.measurement_status.into(),
                memory_sample_count: observation.memory_sample_count,
                uptake: memory_uptake,
                failure_reason: if memory_uptake == MontyBudgetUptake::Failed {
                    failure.map(str::to_owned)
                } else {
                    None
                },
            }),
            execution_limits: hosting.map(|((actor, _deadlines), ownership)| {
                MontyExecutionLimitsStatus {
                    worker_memory_budget_bytes: allocator.memory_budget_bytes as u64,
                    worker_non_vm_reserve_bytes: allocator.non_vm_reserve_bytes as u64,
                    pending_ownership_checks: ownership.pending_checks,
                    ownership_over_capacity: ownership.over_capacity(),
                    ownership_effective_revision: ownership.revision,
                    actor_requests: actor.ordinary_requests as u64,
                    actor_reserved_bytes: actor.ordinary_reserved_bytes as u64,
                    actor_over_capacity: actor.ordinary_over_capacity(),
                    actor_control_requests: actor.control_requests as u64,
                    actor_control_reserved_bytes: actor.control_reserved_bytes as u64,
                    actor_control_over_capacity: actor.control_over_capacity(),
                    retained_attempts: retained_attempts.retained,
                    retention_over_capacity: retained_attempts.over_capacity(),
                    pending_settings: settings_capacity.pending,
                    settings_over_capacity: settings_capacity.over_capacity(),
                    queued_tasks: admission.tasks,
                    queued_bytes: admission.bytes as u64,
                    queue_over_capacity: admission.over_capacity(),
                    active_recipe_contexts: contexts.active as u64,
                    recipe_contexts_over_capacity: contexts.active > contexts.limit as usize,
                    desired_revision: desired.revision,
                    effective_revision: effective.revision,
                    limits: limits.expect("same hosting observation"),
                    uptake: limits_uptake,
                    failure_reason: if limits_uptake == MontyBudgetUptake::Failed {
                        failure.map(str::to_owned)
                    } else {
                        None
                    },
                }
            }),
            state: if closed {
                MontyVmState::Error
            } else {
                MontyVmState::Running
            },
            orchestrator_version: None,
            // No claim that the entire desired row (including heap/catalogue)
            // has been applied. Task budgets have their explicit observation.
            settings_hash: None,
            restart_supported: false,
            task_budget: Some(MontyTaskBudgetStatus {
                desired_revision: desired.revision,
                effective_revision: effective.revision,
                max_duration_secs: effective.limits.max_compute_time.as_secs(),
                token_budgets_enabled: effective.limits.token_budgets_enabled,
                uptake,
                failure_reason: if uptake == MontyBudgetUptake::Failed {
                    failure.map(str::to_owned)
                } else {
                    None
                },
            }),
        }
    }
}

#[async_trait]
impl MontyVmSettingsStore for LiveMontySettingsStore {
    fn runtime_observation(&self, desired: &MontyVmSettings) -> Option<MontyVmStatusResponse> {
        Some(self.observation(desired))
    }
    async fn runtime_status(&self) -> Result<Option<MontyVmStatusResponse>, MontyVmSettingsError> {
        let desired = self.read_desired().await?;
        // Refresh actual instance ownership before making a liveness claim.
        self.ownership
            .check()
            .await
            .map_err(|_| MontyVmSettingsError::Unavailable("runtime unavailable".into()))?;
        Ok(Some(self.observation(&desired)))
    }
    async fn get(
        &self,
        _user: &str,
        _project: &str,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        self.read_desired().await
    }
    async fn upsert(
        &self,
        _user: &str,
        _project: &str,
        update: &UpdateMontyVmSettingsRequest,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        // Reject intrinsic invalidity before contending with the controller.
        // A periodic reconciliation must not turn a below-floor edit into 503.
        if update.max_memory_bytes.is_some_and(|bytes| {
            bytes < brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES
                || bytes > i64::MAX as u64
                || usize::try_from(bytes).is_err()
        }) {
            return Err(MontyVmSettingsError::Invalid(
                "max_memory_bytes must be at least the 512 MiB default and fit the host".into(),
            ));
        }
        if let Some(limits) = update.execution_limits {
            self.service
                .validate_adapter_reserve(adapter_reserve(limits)?)
                .map_err(|_| {
                    MontyVmSettingsError::Invalid(
                        "adapter reserve and heap exceed native capacity".into(),
                    )
                })?;
            admission_limits(limits)?;
            self.service.validate_actor_limits(actor_limits(limits)?)
                .map_err(|_| MontyVmSettingsError::Invalid("ordinary actor bytes must exceed one frame; control bytes must reserve a full request and response".into()))?;
            self.service
                .validate_runtime_bounds(
                    execution_bounds(limits)?,
                    hosting_deadlines(limits)?.response_timeout,
                )
                .map_err(|error| MontyVmSettingsError::Invalid(match error {
                    ServiceFailure::Backpressure => "VM slice must fit current and pending IPC deadlines; increase the response deadline first and let earlier exchanges finish".into(),
                    _ => "execution limits exceed current transport or response settings".into(),
                }))?;
        }
        if update.active_orchestrator_id.is_some() {
            return Err(MontyVmSettingsError::Invalid(
                "orchestrator edits require their separate live publication path".into(),
            ));
        }
        if update
            .expected_revision
            .is_some_and(|revision| revision < self.service.live_task_settings().current().revision)
        {
            return Err(MontyVmSettingsError::RevisionConflict);
        }
        if let Some(policy) = update.memory_policy {
            policy
                .validate()
                .map_err(|reason| MontyVmSettingsError::Invalid(reason.into()))?;
        }
        let deadlines = self.deadlines();
        let mut observed = self.observed.subscribe();
        let store = self.desired.clone();
        let service = self.service.clone();
        let memory = tokio::time::timeout(deadlines.source, self.edit.clone().lock_owned())
            .await
            .map_err(|_| MontyVmSettingsError::Unavailable("settings edit lock deadline".into()))?;
        if self.closed.load(Ordering::Acquire) {
            return Err(MontyVmSettingsError::Unavailable(
                "settings owner stopped".into(),
            ));
        }
        let wake = self.wake.clone();
        let mut update = update.clone();
        // The owned operation survives an HTTP waiter disappearing. Serialize
        // memory feasibility, durable mutation and optional-controller updates.
        let operation = tokio::spawn(async move {
            let mut memory = memory;
            let before = tokio::time::timeout(deadlines.source, store.settled_get())
                .await
                .map_err(|_| {
                    MontyVmSettingsError::Unavailable("settings lock deadline".into())
                })??;
            if update.expected_revision != Some(before.revision) {
                return Err(MontyVmSettingsError::RevisionConflict);
            }
            if update.max_memory_bytes.is_some() && update.memory_policy.is_none() {
                let mut policy = before.memory_policy;
                policy.mode = MontyMemoryMode::Manual;
                update.memory_policy = Some(policy);
            }
            let expected = patched_snapshot(&before, &update)?;
            if let Some(bytes) = expected.max_memory_bytes {
                let heap = usize::try_from(bytes).map_err(|_| {
                    MontyVmSettingsError::Invalid("heap budget exceeds native capacity".into())
                })?;
                service
                    .validate_memory_layout(heap, adapter_reserve(expected.execution_limits)?)
                    .map_err(|_| {
                        MontyVmSettingsError::Invalid(
                            "heap, frames and adapter reserve exceed native capacity".into(),
                        )
                    })?;
            }
            let manual_selection = update.memory_policy.is_some_and(|policy| {
                policy.mode == MontyMemoryMode::Manual && policy.mode != before.memory_policy.mode
            });
            let startup_edit = expected.memory_policy.mode == MontyMemoryMode::Startup
                && (update.max_memory_bytes.is_some()
                    || update
                        .memory_policy
                        .is_some_and(|policy| policy != before.memory_policy));
            let desired = if update.max_memory_bytes.is_some() || manual_selection || startup_edit {
                let bytes = expected
                    .max_memory_bytes
                    .ok_or_else(|| MontyVmSettingsError::Invalid("heap limit missing".into()))?;
                let max_vm_bytes = if startup_edit {
                    let (bytes, status) =
                        startup_heap_selection_with_deadline(&service, &expected, deadlines.source)
                            .await?;
                    memory.memory_sample_count =
                        memory.memory_sample_count.checked_add(1).ok_or_else(|| {
                            MontyVmSettingsError::Internal("memory sample counter exhausted".into())
                        })?;
                    memory.measurement_status = status;
                    bytes
                } else {
                    usize::try_from(bytes)
                        .ok()
                        .filter(|bytes| *bytes > 0)
                        .ok_or_else(|| {
                            MontyVmSettingsError::Invalid(
                                "heap limit must be a positive native integer".into(),
                            )
                        })?
                };
                let heap = service.heap_observation();
                let heap_revision = heap.status.desired_revision();
                let next = HeapSettings {
                    revision: heap_revision.checked_add(1).ok_or_else(|| {
                        MontyVmSettingsError::Invalid("heap revision exhausted".into())
                    })?,
                    max_vm_bytes,
                };
                let (persisted, mut result) = watch::channel(None);
                let committed = expected.clone();
                let commit = Box::pin(async move {
                    let write = tokio::time::timeout(
                        deadlines.source,
                        store.upsert("default", "default", &update),
                    )
                    .await;
                    let settled = match write {
                        Ok(Ok(snapshot)) => Ok(snapshot),
                        Ok(Err(
                            error @ (MontyVmSettingsError::RevisionConflict
                            | MontyVmSettingsError::Invalid(_)),
                        )) => {
                            persisted.send_replace(Some(Err(error)));
                            return HeapCommitOutcome::Rejected;
                        }
                        failure => {
                            let row =
                                tokio::time::timeout(deadlines.source, store.settled_get()).await;
                            match row {
                                Ok(Ok(snapshot))
                                    if snapshot.revision == committed.revision
                                        && snapshots_equal(&snapshot, &committed) =>
                                {
                                    Ok(snapshot)
                                }
                                Ok(Ok(snapshot))
                                    if snapshot.revision == before.revision
                                        && snapshots_equal(&snapshot, &before) =>
                                {
                                    let error = match failure {
                                        Ok(Err(error)) => error,
                                        _ => MontyVmSettingsError::Unavailable(
                                            "settings write did not commit".into(),
                                        ),
                                    };
                                    persisted.send_replace(Some(Err(error)));
                                    return HeapCommitOutcome::Rejected;
                                }
                                _ => {
                                    persisted.send_replace(Some(Err(
                                        MontyVmSettingsError::Unavailable(
                                            "settings commit outcome requires reconciliation"
                                                .into(),
                                        ),
                                    )));
                                    return HeapCommitOutcome::Unknown;
                                }
                            }
                        }
                    };
                    persisted.send_replace(Some(settled));
                    HeapCommitOutcome::Committed
                });
                let receipt = service
                    .publish_heap_transaction(heap_revision, next, commit)
                    .await;
                let persisted = result.borrow_and_update().clone();
                if let Err(failure) = receipt {
                    if failure == ServiceFailure::SettingsConflict
                        && let Some(Err(error)) = persisted
                    {
                        return Err(error);
                    }
                    return Err(match failure {
                        ServiceFailure::UnsafeHeapReduction => MontyVmSettingsError::Invalid(
                            "heap reduction cannot be applied to current live allocations".into(),
                        ),
                        ServiceFailure::InvalidLimits => {
                            MontyVmSettingsError::Invalid("heap limit is out of range".into())
                        }
                        _ => MontyVmSettingsError::Unavailable(
                            "heap transaction failed; read current settings before retry".into(),
                        ),
                    });
                }
                persisted.ok_or_else(|| {
                    MontyVmSettingsError::Internal("heap commit receipt missing".into())
                })??
            } else {
                tokio::time::timeout(
                    deadlines.source,
                    store.upsert("default", "default", &update),
                )
                .await
                .map_err(|_| {
                    MontyVmSettingsError::Unavailable(
                        "settings write outcome requires readback".into(),
                    )
                })??
            };
            memory.configured = desired.max_memory_bytes;
            memory.policy = desired.memory_policy;
            memory.settings_revision = desired.revision;
            memory.last_sample = None;
            wake.notify_one();
            Ok::<_, MontyVmSettingsError>(desired)
        });
        let desired = operation
            .await
            .map_err(|_| MontyVmSettingsError::Internal("settings edit owner failed".into()))??;
        // A persisted edit is returned even if uptake is pending/failed. Its
        // revision remains recoverable; this waiter never owns reconciliation.
        let wait = async {
            loop {
                let status = self.observation(&desired);
                let pending = status
                    .task_budget
                    .is_some_and(|budget| budget.uptake == MontyBudgetUptake::Pending)
                    || status
                        .execution_limits
                        .is_some_and(|budget| budget.uptake == MontyBudgetUptake::Pending)
                    || status
                        .recipe_budget
                        .is_some_and(|budget| budget.uptake == MontyBudgetUptake::Pending)
                    || status
                        .memory_budget
                        .is_some_and(|budget| budget.uptake == MontyBudgetUptake::Pending);
                if !pending {
                    break;
                }
                if observed.changed().await.is_err() {
                    break;
                }
            }
        };
        if tokio::time::timeout(deadlines.uptake, wait).await.is_err() {
            tracing::warn!(
                revision = desired.revision,
                "Monty task settings persisted; uptake acknowledgement remains pending"
            );
        }
        Ok(desired)
    }
}

fn snapshots_equal(left: &MontyVmSettings, right: &MontyVmSettings) -> bool {
    // DTO contains only validated public settings; comparison includes all fields.
    serde_json::to_value(left)
        .ok()
        .zip(serde_json::to_value(right).ok())
        .is_some_and(|(left, right)| left == right)
}
fn patched_snapshot(
    before: &MontyVmSettings,
    update: &UpdateMontyVmSettingsRequest,
) -> Result<MontyVmSettings, MontyVmSettingsError> {
    let mut fields = serde_json::to_value(before)
        .map_err(|_| MontyVmSettingsError::Internal("settings encoding failed".into()))?;
    let patch = serde_json::to_value(update)
        .map_err(|_| MontyVmSettingsError::Internal("settings patch encoding failed".into()))?;
    let object = fields
        .as_object_mut()
        .ok_or_else(|| MontyVmSettingsError::Internal("settings object missing".into()))?;
    for (key, value) in patch
        .as_object()
        .ok_or_else(|| MontyVmSettingsError::Internal("settings patch missing".into()))?
    {
        if key != "expected_revision" && !value.is_null() {
            object.insert(key.clone(), value.clone());
        }
    }
    let revision = before
        .revision
        .checked_add(1)
        .filter(|revision| *revision <= i64::MAX as u64)
        .ok_or_else(|| MontyVmSettingsError::Invalid("settings revision exhausted".into()))?;
    object.insert("revision".into(), revision.into());
    let candidate: MontyVmSettings = serde_json::from_value(fields)
        .map_err(|_| MontyVmSettingsError::Invalid("invalid settings patch".into()))?;
    if !candidate.max_memory_bytes.is_some_and(|bytes| {
        bytes >= brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES && bytes <= i64::MAX as u64
    }) {
        return Err(MontyVmSettingsError::Invalid(
            "max_memory_bytes must be at least the 512 MiB default".into(),
        ));
    }
    if candidate.max_recipes_per_task == 0 || candidate.max_recipes_per_task > i32::MAX as u32 {
        return Err(MontyVmSettingsError::Invalid(
            "invalid task Recipe capacity".into(),
        ));
    }
    Ok(candidate)
}

struct EffectiveRecipeCapacity {
    observed: watch::Receiver<Observation>,
    live: brassclaw_resources::LiveMontyTaskSettings,
    ownership: GlobalOwnerCheck,
}
#[async_trait]
impl crate::global_recipe_ports::RecipeCapacitySource for EffectiveRecipeCapacity {
    async fn current(
        &self,
    ) -> Result<
        crate::global_recipe_ports::RecipeCapacity,
        brassclaw_monty_host::service::PortFailure,
    > {
        let settings = effective_snapshot(self.observed.clone(), &self.live, &self.ownership)
            .await
            .map_err(|_| {
                brassclaw_monty_host::service::PortFailure::new("recipe_capacity_unavailable")
                    .expect("static reason")
            })?;
        Ok(crate::global_recipe_ports::RecipeCapacity {
            revision: settings.revision,
            max_recipes: settings.max_recipes_per_task as usize,
        })
    }
}

struct EffectiveSettings(Arc<LiveMontySettingsStore>);
struct EffectiveCancellationPolicy {
    observed: watch::Receiver<Observation>,
}
impl crate::global_monty_driver::CancellationAckSource for EffectiveCancellationPolicy {
    fn current(
        &self,
    ) -> Result<
        crate::global_monty_driver::CancellationAckSettings,
        brassclaw_turns::run_profile::AgentLoopDriverError,
    > {
        let snapshot = self.observed.borrow();
        crate::global_monty_driver::CancellationAckSettings {
            revision: snapshot.effective.revision,
            timeout: SettingsDeadlines::from_limits(snapshot.effective.execution_limits)
                .cancellation,
        }
        .validate()
    }
}
async fn effective_snapshot(
    mut observed: watch::Receiver<Observation>,
    live: &brassclaw_resources::LiveMontyTaskSettings,
    ownership: &GlobalOwnerCheck,
) -> Result<MontyVmSettings, MontyVmSettingsError> {
    let deadlines = SettingsDeadlines::from_limits(observed.borrow().effective.execution_limits);
    tokio::time::timeout(deadlines.uptake, async {
        loop {
            if ownership.is_closed() {
                return Err(MontyVmSettingsError::Unavailable(
                    "runtime unavailable".into(),
                ));
            }
            let snapshot = observed.borrow_and_update().effective.clone();
            if task_settings(&snapshot) == TaskSettings::from(live.current()) {
                return Ok(snapshot);
            }
            observed.changed().await.map_err(|_| {
                MontyVmSettingsError::Unavailable("effective settings unavailable".into())
            })?;
        }
    })
    .await
    .map_err(|_| {
        MontyVmSettingsError::Unavailable("effective settings acknowledgement pending".into())
    })?
}
#[async_trait]
impl MontyVmSettingsStore for EffectiveSettings {
    async fn get(
        &self,
        _user: &str,
        _project: &str,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        effective_snapshot(
            self.0.observed.subscribe(),
            &self.0.service.live_task_settings(),
            &self.0.ownership,
        )
        .await
    }
    async fn upsert(
        &self,
        _user: &str,
        _project: &str,
        _update: &UpdateMontyVmSettingsRequest,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        Err(MontyVmSettingsError::Invalid(
            "effective settings are read-only".into(),
        ))
    }
}

fn reconciliation_deadline(
    limits: MontyExecutionLimits,
    policy: MontyMemoryPolicy,
    last_sample: Option<Instant>,
    failed: bool,
    now: Instant,
) -> Result<Instant, &'static str> {
    let cadence = Duration::from_millis(limits.settings_reconcile_interval_millis);
    if cadence.is_zero() {
        return Err("settings_reconcile_clock_unsupported");
    }
    let poll = now
        .checked_add(cadence)
        .ok_or("settings_reconcile_clock_unsupported")?;
    if policy.mode != MontyMemoryMode::Automatic {
        return Ok(poll);
    }
    let interval = Duration::from_secs(policy.sample_interval_secs);
    // On failure, back off instead of repeatedly waking on an already due
    // sample. Keep retries within the separately configured sampling cadence.
    let sample = if failed {
        now.checked_add(interval)
    } else {
        last_sample.map_or(Some(now), |last| last.checked_add(interval))
    }
    .ok_or("settings_reconcile_clock_unsupported")?;
    Ok(poll.min(sample.max(now)))
}

async fn wait_reconciliation_deadline(at: Instant) {
    // Tokio's native timer driver saturates very distant absolute ticks.
    // Preserve the operator's logical deadline with representable timer chunks;
    // intermediate wakes perform no reconciliation, OS read or policy effect.
    while let Some(remaining) = at.checked_duration_since(Instant::now()) {
        if remaining.is_zero() {
            return;
        }
        tokio::time::sleep(remaining.min(Duration::from_millis(u32::MAX as u64))).await;
    }
}

async fn reconcile_loop(store: Arc<LiveMontySettingsStore>, cancel: CancellationToken) {
    loop {
        // Capture only the complete acknowledged policy. One accepted timer
        // retains its deadline; a save has an independent, lossless Notify wake.
        let deadline = {
            let observed = store.observed.borrow();
            reconciliation_deadline(
                observed.effective.execution_limits,
                observed.effective.memory_policy,
                observed.last_memory_sample,
                observed.failure.is_some(),
                Instant::now(),
            )
        };
        if let Err(reason) = deadline {
            let mut next = store.observed.borrow().clone();
            let failure = (Some(next.effective.revision), reason);
            if next.failure != Some(failure) {
                tracing::error!(
                    reason,
                    revision = failure.0,
                    "Monty settings timer unavailable"
                );
            }
            next.failure = Some(failure);
            store.observed.send_replace(next);
        }
        let periodic = async {
            match deadline {
                Ok(at) => wait_reconciliation_deadline(at).await,
                // Preserve a visible technical error, but allow a valid save
                // or shutdown to recover without spinning or killing Monty.
                Err(_) => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            _ = store.wake.notified() => {},
            _ = periodic => {},
        }
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            result = store.reconcile() => {
                if let Err(failure) = result {
                    let mut next = store.observed.borrow().clone();
                    if next.failure != Some(failure) {
                        tracing::error!(reason = failure.1, revision = failure.0, "Monty settings reconciliation failed");
                    }
                    next.failure = Some(failure);
                    store.observed.send_replace(next);
                }
            }
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_schedule_preserves_sampling_and_failure_backoff() {
        let now = Instant::now();
        let limits = MontyExecutionLimits {
            settings_reconcile_interval_millis: 3_600_000,
            ..Default::default()
        };
        let mut policy = MontyMemoryPolicy::default();
        for mode in [MontyMemoryMode::Manual, MontyMemoryMode::Startup] {
            policy.mode = mode;
            assert_eq!(
                reconciliation_deadline(limits, policy, None, false, now).unwrap(),
                now + Duration::from_secs(3600)
            );
        }
        policy.mode = MontyMemoryMode::Automatic;
        let last = now.checked_sub(Duration::from_secs(20)).unwrap();
        assert_eq!(
            reconciliation_deadline(limits, policy, Some(last), false, now).unwrap(),
            now + Duration::from_secs(40)
        );
        assert_eq!(
            reconciliation_deadline(limits, policy, Some(last), true, now).unwrap(),
            now + Duration::from_secs(60)
        );
        for last in [None, now.checked_sub(Duration::from_secs(70))] {
            assert_eq!(
                reconciliation_deadline(limits, policy, last, false, now).unwrap(),
                now
            );
        }
        let invalid = MontyExecutionLimits {
            settings_reconcile_interval_millis: 0,
            ..limits
        };
        assert!(reconciliation_deadline(invalid, policy, None, false, now).is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_settings_edit_retains_source_deadline_and_rejects_same_revision_mutation() {
        let rig = crate::runtime::test_pg::pg_rig().await;
        rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
        let home = tempfile::tempdir().expect("runtime home");
        let gateway = Arc::new(crate::test_support::BudgetTestGateway::new());
        let input = crate::RebornRuntimeInput::from_services(
            rig.build_input("coordination-owner", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().expect("local policy")),
        )
        .with_model_gateway_override(gateway.clone());
        let runtime = crate::build_reborn_runtime(input)
            .await
            .expect("actual global runtime");
        let settings = runtime.webui_monty_settings_store();
        let before = settings.get("default", "default").await.unwrap();
        assert_eq!(before.execution_limits.settings_source_timeout_millis, 2000);
        let mut limits = before.execution_limits;
        limits.settings_source_timeout_millis = 200;
        limits.settings_uptake_timeout_millis = 9000;
        let update = serde_json::from_value(serde_json::json!({
            "expected_revision":before.revision,"execution_limits":limits,
        }))
        .unwrap();
        let mut client = rig.pool.get().await.unwrap();
        let transaction = client.transaction().await.unwrap();
        transaction
            .query_one(
                "SELECT revision FROM reborn_monty_vm_settings
            WHERE tenant_id='default' AND user_id='default' AND agent_id='default'
            AND project_id='default' FOR UPDATE",
                &[],
            )
            .await
            .unwrap();
        let entered = Arc::new(Notify::new());
        let editing = {
            let settings = settings.clone();
            let entered = entered.clone();
            tokio::spawn(async move {
                entered.notify_one();
                settings.upsert("default", "default", &update).await
            })
        };
        entered.notified().await;
        // An actual row lock makes the accepted source operation exceed the
        // requested successor's 200 ms. It must retain the old 2 s deadline.
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(
            !editing.is_finished(),
            "edit must be waiting for the real database lock"
        );
        transaction.commit().await.unwrap();
        let changed = editing
            .await
            .unwrap()
            .expect("accepted old deadline survives reduction");
        assert_eq!(changed.revision, before.revision + 1);
        let status = settings.runtime_observation(&changed).unwrap();
        let execution = status.execution_limits.unwrap();
        assert_eq!(execution.uptake, MontyBudgetUptake::Applied);
        assert_eq!(execution.limits, limits);
        assert_eq!(execution.effective_revision, changed.revision);
        let heap = status.memory_budget.unwrap().max_memory_bytes;

        // Restore normal deadlines via the real operator port before exercising
        // catalogue-independent corruption detection. No VM restart is used.
        limits.settings_source_timeout_millis = 2000;
        limits.settings_uptake_timeout_millis = 5000;
        let update = serde_json::from_value(serde_json::json!({
            "expected_revision":changed.revision,"execution_limits":limits,
        }))
        .unwrap();
        let restored = settings
            .upsert("default", "default", &update)
            .await
            .unwrap();
        let mut corrupt_limits = limits;
        corrupt_limits.settings_source_timeout_millis = 7000;
        let error = client
            .execute(
                "UPDATE reborn_monty_vm_settings SET execution_limits=$1,
            max_memory_bytes=$2 WHERE tenant_id='default' AND user_id='default'
            AND agent_id='default' AND project_id='default' AND revision=$3",
                &[
                    &serde_json::to_value(corrupt_limits).unwrap(),
                    &((heap + brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES) as i64),
                    &(restored.revision as i64),
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let persisted = settings.get("default", "default").await.unwrap();
        assert!(snapshots_equal(&persisted, &restored));
        let status = settings.runtime_status().await.unwrap().unwrap();
        assert_eq!(status.state, MontyVmState::Running);
        let execution = status.execution_limits.unwrap();
        assert_eq!(execution.uptake, MontyBudgetUptake::Applied);
        assert_eq!(execution.limits, limits);
        assert_eq!(execution.effective_revision, restored.revision);
        assert_eq!(
            status.memory_budget.unwrap().max_memory_bytes,
            heap,
            "rejected unversioned edits must not change the actual heap"
        );
        assert_eq!(
            gateway.call_count(),
            0,
            "settings work must not call a model"
        );
        drop(settings);
        drop(client);
        runtime
            .shutdown()
            .await
            .expect("settled global owner shutdown");
    }
}
