//! Durable instance task-budget publication through the existing global service.
//! The owner survives HTTP cancellation; desired edits never masquerade as uptake.
use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use brassclaw_host_api::MontyExecutionLimits;
use brassclaw_monty_host::{VmBounds, process::TaskSettings, service::ServiceClient};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    MontyBudgetUptake, MontyExecutionLimitsStatus, MontyTaskBudgetStatus, MontyVmSettings,
    MontyVmSettingsError, MontyVmSettingsStore, MontyVmState, MontyVmStatusResponse,
    UpdateMontyVmSettingsRequest,
};
use tokio::{
    sync::{Notify, watch},
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
}

pub(crate) struct LiveMontySettingsStore {
    desired: PgMontyVmSettingsStore,
    service: ServiceClient,
    ownership: GlobalOwnerCheck,
    wake: Notify,
    observed: watch::Sender<Observation>,
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
    ) -> Result<Self, MontyVmSettingsError> {
        if task_settings(&initial) != TaskSettings::from(service.live_task_settings().current())
            || execution_bounds(initial.execution_limits)? != service.vm_bounds()
        {
            return Err(MontyVmSettingsError::Internal(
                "initial task revision mismatch".into(),
            ));
        }
        let (observed, _) = watch::channel(Observation {
            effective: initial,
            failure: None,
        });
        let store = Arc::new(LiveMontySettingsStore {
            desired: PgMontyVmSettingsStore::new(pool, "default", "default"),
            service,
            ownership,
            wake: Notify::new(),
            observed,
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
        self.cancel.cancel();
        if let Some(join) = self.join.take() {
            join.await
                .map_err(|_| MontyVmSettingsError::Internal("settings owner failed".into()))?;
        }
        Ok(())
    }
}
impl Drop for MontySettingsOwner {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
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
        let desired = self
            .read_desired()
            .await
            .map_err(|_| (None, "settings_source_unavailable"))?;
        let revision = Some(desired.revision);
        self.ownership
            .check()
            .await
            .map_err(|_| (revision, "runtime_unavailable"))?;
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
            effective: desired,
            failure: None,
        });
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
        MontyVmStatusResponse {
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
        if update.max_memory_bytes.is_some() || update.active_orchestrator_id.is_some() {
            return Err(MontyVmSettingsError::Invalid(
                "heap and orchestrator edits require their separate live publication paths".into(),
            ));
        }
        if update
            .expected_revision
            .is_some_and(|revision| revision < self.service.live_task_settings().current().revision)
        {
            return Err(MontyVmSettingsError::RevisionConflict);
        }
        // Subscribe before persistence to avoid a lost wake/fast acknowledgement.
        let mut observed = self.observed.subscribe();
        let desired = tokio::time::timeout(
            SOURCE_BOUND,
            self.desired.upsert("default", "default", update),
        )
        .await
        .map_err(|_| {
            MontyVmSettingsError::Unavailable("settings write outcome requires readback".into())
        })??;
        self.wake.notify_one();
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

struct EffectiveSettings(Arc<LiveMontySettingsStore>);
#[async_trait]
impl MontyVmSettingsStore for EffectiveSettings {
    async fn get(
        &self,
        _user: &str,
        _project: &str,
    ) -> Result<MontyVmSettings, MontyVmSettingsError> {
        let mut observed = self.0.observed.subscribe();
        tokio::time::timeout(UPTAKE_BOUND, async {
            loop {
                if self.0.ownership.is_closed() {
                    return Err(MontyVmSettingsError::Unavailable(
                        "runtime unavailable".into(),
                    ));
                }
                let snapshot = observed.borrow_and_update().effective.clone();
                if task_settings(&snapshot)
                    == TaskSettings::from(self.0.service.live_task_settings().current())
                {
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
