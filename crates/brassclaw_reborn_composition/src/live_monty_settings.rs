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
    service::{HeapCommitOutcome, ServiceClient, ServiceFailure},
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

const SOURCE_BOUND: Duration = Duration::from_secs(2);
const UPTAKE_BOUND: Duration = Duration::from_secs(5);
const RECONCILE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
struct Observation {
    effective: MontyVmSettings,
    // Only classified host-owned diagnostics; no DB/provider error strings.
    failure: Option<(Option<u64>, &'static str)>,
    memory_revision: u64,
    measurement_status: &'static str,
    memory_sample_count: u64,
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
        if task_settings(&initial) != TaskSettings::from(service.live_task_settings().current())
            || execution_bounds(initial.execution_limits)? != service.vm_bounds()
        {
            return Err(MontyVmSettingsError::Internal(
                "initial task revision mismatch".into(),
            ));
        }
        let edit = Arc::new(Mutex::new(MemoryControl {
            configured: initial.max_memory_bytes,
            policy: initial.memory_policy,
            settings_revision: initial.revision,
            last_critical: measurement_status == "startup_critical",
            last_sample: (initial.memory_policy.mode == MontyMemoryMode::Automatic)
                .then(Instant::now),
            measurement_status,
            memory_sample_count: u64::from(initial.memory_policy.mode != MontyMemoryMode::Manual),
            inflight_probe: None,
        }));
        let (observed, _) = watch::channel(Observation {
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
        SOURCE_BOUND,
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
fn observed_limits(bounds: VmBounds) -> MontyExecutionLimits {
    MontyExecutionLimits {
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

    async fn read_desired(&self) -> Result<MontyVmSettings, MontyVmSettingsError> {
        let settings = tokio::time::timeout(SOURCE_BOUND, self.desired.get("default", "default"))
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
        let mut memory = self.edit.lock().await;
        let desired = self
            .read_desired()
            .await
            .map_err(|_| (None, "settings_source_unavailable"))?;
        let revision = Some(desired.revision);
        self.ownership
            .check()
            .await
            .map_err(|_| (revision, "runtime_unavailable"))?;
        self.reconcile_memory(&desired, &mut memory)
            .await
            .map_err(|reason| (revision, reason))?;
        let effective = self.service.live_task_settings().current();
        let values = execution_bounds(desired.execution_limits)
            .map_err(|_| (revision, "invalid_execution_limits"))?;
        self.service
            .validate_vm_bounds(values)
            .map_err(|_| (revision, "unsupported_execution_limits"))?;
        if desired.revision < effective.revision {
            return Err((revision, "settings_revision_regressed"));
        }
        if desired.revision == effective.revision {
            if task_settings(&desired) != TaskSettings::from(effective)
                || values != self.service.vm_bounds()
            {
                return Err((revision, "settings_revision_conflict"));
            }
        } else {
            tokio::time::timeout(
                UPTAKE_BOUND,
                self.service.publish_runtime_settings(
                    effective.revision,
                    task_settings(&desired),
                    values,
                ),
            )
            .await
            .map_err(|_| (revision, "settings_publication_deadline"))?
            .map_err(|_| (revision, "settings_publication_failed"))?;
        }
        // The service publishes the shared Rust revision only after its real
        // worker acknowledges the same limits. Keep retrieval's whole snapshot.
        self.observed.send_replace(Observation {
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
                UPTAKE_BOUND,
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
                self.service
                    .publish_memory_backpressure(admission.revision, false)
                    .await
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
            SOURCE_BOUND,
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
            self.service
                .publish_memory_backpressure(admission.revision, paused)
                .await
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
            self.service
                .publish_heap(
                    expected,
                    HeapSettings {
                        revision: expected.checked_add(1).ok_or("heap_revision_exhausted")?,
                        max_vm_bytes: usize::try_from(target)
                            .map_err(|_| "heap_limit_out_of_range")?,
                    },
                    true,
                )
                .await
                .map_err(|_| "heap_publication_failed")?;
        }
        Ok(())
    }

    fn observation(&self, desired: &MontyVmSettings) -> MontyVmStatusResponse {
        let effective = self.service.live_task_settings().current();
        let failed = self.observed.borrow().failure;
        let closed = self.ownership.is_closed();
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
        let limits = observed_limits(self.service.vm_bounds());
        let limits_uptake = if closed {
            MontyBudgetUptake::Failed
        } else if desired.revision == effective.revision && desired.execution_limits == limits {
            MontyBudgetUptake::Applied
        } else if failure.is_some() {
            MontyBudgetUptake::Failed
        } else {
            MontyBudgetUptake::Pending
        };
        let memory = self.service.heap_observation();
        let effective_heap = memory.status.effective;
        let observation = self.observed.borrow().clone();
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
            execution_limits: Some(MontyExecutionLimitsStatus {
                desired_revision: desired.revision,
                effective_revision: effective.revision,
                limits,
                uptake: limits_uptake,
                failure_reason: if limits_uptake == MontyBudgetUptake::Failed {
                    failure.map(str::to_owned)
                } else {
                    None
                },
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
        if let Some(limits) = update.execution_limits {
            self.service
                .validate_vm_bounds(execution_bounds(limits)?)
                .map_err(|_| {
                    MontyVmSettingsError::Invalid(
                        "execution limits exceed current transport or response settings".into(),
                    )
                })?;
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
        let mut observed = self.observed.subscribe();
        let store = self.desired.clone();
        let service = self.service.clone();
        let memory = self.edit.clone().try_lock_owned().map_err(|_| {
            MontyVmSettingsError::Unavailable("settings edit already in progress".into())
        })?;
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
            let before = tokio::time::timeout(SOURCE_BOUND, store.settled_get())
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
                    let (bytes, status) = startup_heap_selection(&service, &expected).await?;
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
                        SOURCE_BOUND,
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
                            let row = tokio::time::timeout(SOURCE_BOUND, store.settled_get()).await;
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
                tokio::time::timeout(SOURCE_BOUND, store.upsert("default", "default", &update))
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
                if self
                    .observation(&desired)
                    .task_budget
                    .is_some_and(|budget| budget.uptake != MontyBudgetUptake::Pending)
                {
                    break;
                }
                if observed.changed().await.is_err() {
                    break;
                }
            }
        };
        if tokio::time::timeout(UPTAKE_BOUND, wait).await.is_err() {
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
async fn effective_snapshot(
    mut observed: watch::Receiver<Observation>,
    live: &brassclaw_resources::LiveMontyTaskSettings,
    ownership: &GlobalOwnerCheck,
) -> Result<MontyVmSettings, MontyVmSettingsError> {
    tokio::time::timeout(UPTAKE_BOUND, async {
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

async fn reconcile_loop(store: Arc<LiveMontySettingsStore>, cancel: CancellationToken) {
    let mut interval = tokio::time::interval(RECONCILE_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            _ = store.wake.notified() => {},
            _ = interval.tick() => {},
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
