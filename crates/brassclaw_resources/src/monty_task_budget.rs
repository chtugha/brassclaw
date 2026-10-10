//! Shared live task limits for Rust supervision and Monty tracking.
//!
//! Composition publishes one validated revision after durable settings commit.
//! Both consumers share this handle. Task usage lives outside the settings, so
//! publishing a shorter duration never resets an already running task's clock.
//! Shared heap accounting and external-call deadlines are separate contracts.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MontyTaskLimits {
    pub max_compute_time: Duration,
    pub token_budgets_enabled: bool,
}

impl MontyTaskLimits {
    fn validate(self) -> Result<(), MontyTaskBudgetError> {
        // Match the positive PostgreSQL INT seconds contract. The old 30..3600
        // operational range prevented valid operator edits at both host and VM.
        if !(Duration::from_secs(1)..=Duration::from_secs(i32::MAX as u64))
            .contains(&self.max_compute_time)
        {
            return Err(MontyTaskBudgetError::InvalidSettings);
        }
        Ok(())
    }

    /// None means no artificial token budget. Model limits are not represented
    /// here and must still be enforced by the provider/context adapter.
    pub fn token_budget(self, configured: usize) -> Option<usize> {
        self.token_budgets_enabled.then_some(configured)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MontyTaskSettingsRevision {
    pub revision: u64,
    pub limits: MontyTaskLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MontyTaskBudgetError {
    #[error("invalid Monty task settings")]
    InvalidSettings,
    #[error("Monty task settings revision conflict")]
    RevisionConflict,
    #[error("Monty task accounting overflow")]
    AccountingOverflow,
    #[error("Monty task compute budget exceeded")]
    ComputeExceeded,
    #[error("Monty task accounting unavailable")]
    AccountingUnavailable,
    #[error("Monty task clock regressed")]
    ClockRegressed,
}

/// Publish capability is held by trusted composition, never sent to Python.
/// Readers receive a watch receiver, not an independent cached duration value.
#[derive(Debug, Clone)]
pub struct LiveMontyTaskSettings {
    sender: watch::Sender<MontyTaskSettingsRevision>,
}

impl LiveMontyTaskSettings {
    pub fn new(initial: MontyTaskSettingsRevision) -> Result<Self, MontyTaskBudgetError> {
        initial.limits.validate()?;
        if initial.revision == 0 {
            return Err(MontyTaskBudgetError::InvalidSettings);
        }
        let (sender, _) = watch::channel(initial);
        Ok(Self { sender })
    }

    pub fn subscribe(&self) -> watch::Receiver<MontyTaskSettingsRevision> {
        self.sender.subscribe()
    }

    pub fn current(&self) -> MontyTaskSettingsRevision {
        *self.sender.borrow()
    }

    pub(crate) fn shares_source(&self, other: &Self) -> bool {
        self.sender.same_channel(&other.sender)
    }

    /// Atomic compare-and-publish: time and token mode cannot be
    /// observed from different revisions. No DB write or runtime await occurs
    /// while the watch value is locked. DB/effective acknowledgement is owned
    /// by composition; this method does not imply settings persistence.
    pub fn publish(
        &self,
        expected_revision: u64,
        next: MontyTaskSettingsRevision,
    ) -> Result<(), MontyTaskBudgetError> {
        next.limits.validate()?;
        if next.revision <= expected_revision {
            return Err(MontyTaskBudgetError::RevisionConflict);
        }
        let mut result = Err(MontyTaskBudgetError::RevisionConflict);
        self.sender.send_if_modified(|current| {
            if current.revision != expected_revision {
                return false;
            }
            *current = next;
            result = Ok(());
            true
        });
        result
    }
}

/// One task's observations, retained over steps and continuations. Only active
/// VM execution debits the duration limit. Preparation and Rust value adaptation
/// remain observable separately; queue/idle/external waits enter none of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MontyTaskUsage {
    pub compute_time: Duration,
    pub preparation_time: Duration,
    pub adaptation_time: Duration,
    overflowed: bool,
}

impl MontyTaskUsage {
    pub fn record_compute_time(&mut self, elapsed: Duration) -> Result<(), MontyTaskBudgetError> {
        self.record_time(TaskClockKind::Execution, elapsed)
    }

    pub fn record_preparation_time(
        &mut self,
        elapsed: Duration,
    ) -> Result<(), MontyTaskBudgetError> {
        self.record_time(TaskClockKind::Preparation, elapsed)
    }

    pub fn record_adaptation_time(
        &mut self,
        elapsed: Duration,
    ) -> Result<(), MontyTaskBudgetError> {
        self.record_time(TaskClockKind::Adaptation, elapsed)
    }

    fn record_time(
        &mut self,
        kind: TaskClockKind,
        elapsed: Duration,
    ) -> Result<(), MontyTaskBudgetError> {
        let observed = match kind {
            TaskClockKind::Execution => &mut self.compute_time,
            TaskClockKind::Preparation => &mut self.preparation_time,
            TaskClockKind::Adaptation => &mut self.adaptation_time,
        };
        let Some(total) = observed.checked_add(elapsed) else {
            self.overflowed = true;
            return Err(MontyTaskBudgetError::AccountingOverflow);
        };
        *observed = total;
        Ok(())
    }

    pub fn check(self, limits: MontyTaskLimits) -> Result<(), MontyTaskBudgetError> {
        limits.validate()?;
        if self.overflowed {
            return Err(MontyTaskBudgetError::AccountingOverflow);
        }
        if self.compute_time > limits.max_compute_time {
            return Err(MontyTaskBudgetError::ComputeExceeded);
        }
        Ok(())
    }
}

/// One shared task account for Rust supervision and the VM hosting adapter.
/// Cloning retains the same consumption and terminal state. Settings publication
/// never creates a new task account. The host records non-overlapping active VM
/// segments (including nested execution) from the interpreter's execution clock;
/// external waits are not segments. This type does not measure or preempt a VM.
#[derive(Debug, Clone)]
pub struct SharedMontyTaskBudget {
    settings: LiveMontyTaskSettings,
    account: Arc<Mutex<TaskAccount>>,
}

#[derive(Debug, Default)]
struct TaskAccount {
    usage: MontyTaskUsage,
    terminal: Option<MontyTaskBudgetError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MontyTaskBudgetSnapshot {
    pub settings: MontyTaskSettingsRevision,
    pub usage: MontyTaskUsage,
}

/// Hosting-owned cursor over one interpreter's cumulative execution or
/// preparation clock, selected by the corresponding account constructor.
///
/// Attach at an explicit task/execution ownership boundary, supplying the
/// current clock as the baseline. Prior service work or another task's work is
/// not charged. Keep the same cursor over resumes and feed/turn limit changes;
/// attach a separate cursor for each nested interpreter to the same task budget.
/// The supplied clock must exclude external waits and nested execution, otherwise
/// its caller would double count. This cursor cannot attribute a clock shared by
/// concurrently executing tasks, measure a VM, or recover its final clock.
///
/// Deliberately not Clone: an execution owner must not duplicate its cursor and
/// debit the same segment twice. Do not also record these segments directly.
#[derive(Debug)]
pub struct MontyTaskClock {
    budget: SharedMontyTaskBudget,
    observed: Duration,
    kind: TaskClockKind,
}

#[derive(Debug, Clone, Copy)]
enum TaskClockKind {
    Execution,
    Preparation,
    Adaptation,
}

impl MontyTaskClock {
    pub fn checkpoint(
        &mut self,
        cumulative: Duration,
    ) -> Result<MontyTaskBudgetSnapshot, MontyTaskBudgetError> {
        let Some(delta) = cumulative.checked_sub(self.observed) else {
            let mut account = self
                .budget
                .account
                .lock()
                .map_err(|_| MontyTaskBudgetError::AccountingUnavailable)?;
            let error = account
                .terminal
                .get_or_insert(MontyTaskBudgetError::ClockRegressed);
            return Err(error.clone());
        };
        self.budget.record_time(self.kind, delta)?;
        self.observed = cumulative;
        self.budget.check()
    }
}

impl SharedMontyTaskBudget {
    pub fn new(settings: LiveMontyTaskSettings) -> Self {
        Self {
            settings,
            account: Arc::new(Mutex::new(TaskAccount::default())),
        }
    }

    /// One cursor per owned interpreter execution, not per feed or checkpoint.
    /// Creating a cursor neither clears task usage nor charges its baseline.
    pub fn execution_clock(&self, baseline: Duration) -> MontyTaskClock {
        MontyTaskClock {
            budget: self.clone(),
            observed: baseline,
            kind: TaskClockKind::Execution,
        }
    }

    /// Separate cumulative preparation cursor, with the same ownership,
    /// regression and terminal-failure rules as the execution cursor. These
    /// observations never debit the executing-VM-time limit.
    pub fn preparation_clock(&self, baseline: Duration) -> MontyTaskClock {
        MontyTaskClock {
            budget: self.clone(),
            observed: baseline,
            kind: TaskClockKind::Preparation,
        }
    }

    /// Record each active execution segment exactly once, through the hosting
    /// owner. Rust and Monty readers use `check`, not separate usage counters.
    pub fn record_compute_time(&self, elapsed: Duration) -> Result<(), MontyTaskBudgetError> {
        self.record_time(TaskClockKind::Execution, elapsed)
    }

    pub fn record_preparation_time(&self, elapsed: Duration) -> Result<(), MontyTaskBudgetError> {
        self.record_time(TaskClockKind::Preparation, elapsed)
    }

    pub fn record_adaptation_time(&self, elapsed: Duration) -> Result<(), MontyTaskBudgetError> {
        self.record_time(TaskClockKind::Adaptation, elapsed)
    }

    fn record_time(
        &self,
        kind: TaskClockKind,
        elapsed: Duration,
    ) -> Result<(), MontyTaskBudgetError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| MontyTaskBudgetError::AccountingUnavailable)?;
        if let Some(error) = &account.terminal {
            return Err(error.clone());
        }
        if let Err(error) = account.usage.record_time(kind, elapsed) {
            account.terminal = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// The watch read guard linearizes this checkpoint against publication.
    /// No settings revision can change between selecting limits and checking
    /// shared consumption. Never hold either lock across I/O or VM execution.
    /// Once exceeded, this task remains failed even if the limit is raised.
    pub fn check(&self) -> Result<MontyTaskBudgetSnapshot, MontyTaskBudgetError> {
        let settings = self.settings.sender.borrow();
        let mut account = self
            .account
            .lock()
            .map_err(|_| MontyTaskBudgetError::AccountingUnavailable)?;
        if let Some(error) = &account.terminal {
            return Err(error.clone());
        }
        if let Err(error) = account.usage.check(settings.limits) {
            account.terminal = Some(error.clone());
            return Err(error);
        }
        Ok(MontyTaskBudgetSnapshot {
            settings: *settings,
            usage: account.usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(revision: u64, seconds: u64) -> MontyTaskSettingsRevision {
        MontyTaskSettingsRevision {
            revision,
            limits: MontyTaskLimits {
                max_compute_time: Duration::from_secs(seconds),
                token_budgets_enabled: false,
            },
        }
    }

    #[test]
    fn preparation_and_adaptation_are_observed_without_debiting_vm_duration() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let task = SharedMontyTaskBudget::new(live.clone());
        let child = task.clone();
        let mut execution = child.execution_clock(Duration::ZERO);
        let mut preparation = child.preparation_clock(Duration::from_secs(5));
        execution.checkpoint(Duration::from_secs(1)).unwrap();
        preparation.checkpoint(Duration::from_secs(1005)).unwrap();
        preparation.checkpoint(Duration::from_secs(1005)).unwrap();
        child
            .record_adaptation_time(Duration::from_secs(2000))
            .unwrap();
        live.publish(1, settings(2, 1)).unwrap();
        let snapshot = task.check().unwrap();
        assert_eq!(snapshot.settings.revision, 2);
        assert_eq!(snapshot.usage.compute_time, Duration::from_secs(1));
        assert_eq!(snapshot.usage.preparation_time, Duration::from_secs(1000));
        assert_eq!(snapshot.usage.adaptation_time, Duration::from_secs(2000));
        assert_eq!(snapshot, child.check().unwrap());
        assert_eq!(
            execution.checkpoint(Duration::from_secs(2)),
            Err(MontyTaskBudgetError::ComputeExceeded)
        );
        live.publish(2, settings(3, 600)).unwrap();
        assert_eq!(
            preparation.checkpoint(Duration::from_secs(1006)),
            Err(MontyTaskBudgetError::ComputeExceeded)
        );
    }

    #[test]
    fn preparation_regression_and_observation_overflow_fail_the_shared_task_closed() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let task = SharedMontyTaskBudget::new(live.clone());
        let mut preparation = task.preparation_clock(Duration::ZERO);
        preparation.checkpoint(Duration::from_secs(1)).unwrap();
        assert_eq!(
            preparation.checkpoint(Duration::ZERO),
            Err(MontyTaskBudgetError::ClockRegressed)
        );
        assert_eq!(
            task.record_compute_time(Duration::from_secs(1)),
            Err(MontyTaskBudgetError::ClockRegressed)
        );
        for kind in [TaskClockKind::Preparation, TaskClockKind::Adaptation] {
            let task = SharedMontyTaskBudget::new(live.clone());
            task.record_time(kind, Duration::MAX).unwrap();
            assert_eq!(task.check().unwrap().usage.compute_time, Duration::ZERO);
            assert_eq!(
                task.record_time(kind, Duration::from_secs(1)),
                Err(MontyTaskBudgetError::AccountingOverflow)
            );
            assert_eq!(
                task.clone().check(),
                Err(MontyTaskBudgetError::AccountingOverflow)
            );
        }
    }

    #[tokio::test]
    async fn rust_and_vm_observe_same_live_revision_and_shared_consumption() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let mut rust_updates = live.subscribe();
        let mut vm_updates = live.subscribe();
        let rust = SharedMontyTaskBudget::new(live.clone());
        let vm = rust.clone();
        vm.record_compute_time(Duration::from_secs(60)).unwrap();
        live.publish(1, settings(2, 120)).unwrap();
        rust_updates.changed().await.unwrap();
        vm_updates.changed().await.unwrap();
        assert_eq!(
            *rust_updates.borrow_and_update(),
            *vm_updates.borrow_and_update()
        );
        assert_eq!(rust.check(), vm.check());
        assert_eq!(
            rust.check().unwrap().usage.compute_time,
            Duration::from_secs(60)
        );
        live.publish(2, settings(3, 30)).unwrap();
        assert_eq!(rust.check(), Err(MontyTaskBudgetError::ComputeExceeded));
        assert_eq!(vm.check(), Err(MontyTaskBudgetError::ComputeExceeded));
        live.publish(3, settings(4, 600)).unwrap();
        // A settings increase cannot revive a terminated task.
        assert_eq!(vm.check(), Err(MontyTaskBudgetError::ComputeExceeded));
        let next_task = SharedMontyTaskBudget::new(live);
        assert_eq!(
            next_task.check().unwrap().usage.compute_time,
            Duration::ZERO
        );
    }

    #[tokio::test(start_paused = true)]
    async fn external_wait_and_settings_publication_do_not_debit_compute() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let rust = SharedMontyTaskBudget::new(live.clone());
        let vm = rust.clone();
        vm.record_compute_time(Duration::from_secs(20)).unwrap();
        tokio::time::sleep(Duration::from_secs(3600)).await;
        live.publish(1, settings(2, 30)).unwrap();
        assert_eq!(rust.check(), vm.check());
        assert_eq!(
            vm.check().unwrap().usage.compute_time,
            Duration::from_secs(20)
        );
        vm.record_compute_time(Duration::from_secs(11)).unwrap();
        assert_eq!(rust.check(), Err(MontyTaskBudgetError::ComputeExceeded));
    }

    #[test]
    fn stale_or_invalid_update_preserves_effective_revision() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        live.publish(1, settings(2, 60)).unwrap();
        assert_eq!(
            live.publish(1, settings(3, 30)),
            Err(MontyTaskBudgetError::RevisionConflict)
        );
        assert_eq!(
            live.publish(2, settings(3, 0)),
            Err(MontyTaskBudgetError::InvalidSettings)
        );
        assert_eq!(live.current(), settings(2, 60));
        assert_eq!(live.current().limits.token_budget(4096), None);
    }

    #[test]
    fn duration_range_preserves_usage_and_only_rejects_unrepresentable_values() {
        let live = LiveMontyTaskSettings::new(settings(1, 7200)).unwrap();
        let task = SharedMontyTaskBudget::new(live.clone());
        task.record_compute_time(Duration::from_millis(250))
            .unwrap();
        live.publish(1, settings(2, 1)).unwrap();
        let snapshot = task.check().unwrap();
        assert_eq!(snapshot.usage.compute_time, Duration::from_millis(250));
        live.publish(2, settings(3, i32::MAX as u64)).unwrap();
        for seconds in [0, i32::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                live.publish(3, settings(4, seconds)),
                Err(MontyTaskBudgetError::InvalidSettings)
            );
        }
        assert_eq!(live.current(), settings(3, i32::MAX as u64));
        assert_eq!(
            task.check().unwrap().usage.compute_time,
            Duration::from_millis(250)
        );
    }

    #[test]
    fn accounting_overflow_remains_fail_closed_after_limits_change() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let rust = SharedMontyTaskBudget::new(live.clone());
        let vm = rust.clone();
        vm.record_compute_time(Duration::MAX).unwrap();
        assert_eq!(
            vm.record_compute_time(Duration::from_secs(1)),
            Err(MontyTaskBudgetError::AccountingOverflow)
        );
        live.publish(1, settings(2, 3600)).unwrap();
        assert_eq!(rust.check(), Err(MontyTaskBudgetError::AccountingOverflow));
        assert_eq!(vm.check(), Err(MontyTaskBudgetError::AccountingOverflow));
    }

    #[test]
    fn cumulative_checkpoints_and_nested_clocks_preserve_task_consumption() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let task = SharedMontyTaskBudget::new(live.clone());
        let mut root = task.execution_clock(Duration::from_secs(100));
        root.checkpoint(Duration::from_secs(110)).unwrap();
        root.checkpoint(Duration::from_secs(110)).unwrap();
        let mut nested = task.execution_clock(Duration::ZERO);
        nested.checkpoint(Duration::from_secs(15)).unwrap();
        live.publish(1, settings(2, 30)).unwrap();
        let snapshot = root.checkpoint(Duration::from_secs(114)).unwrap();
        assert_eq!(snapshot.settings.revision, 2);
        assert_eq!(snapshot.usage.compute_time, Duration::from_secs(29));
        assert_eq!(
            nested.checkpoint(Duration::from_secs(17)),
            Err(MontyTaskBudgetError::ComputeExceeded)
        );
        live.publish(2, settings(3, 600)).unwrap();
        assert_eq!(task.check(), Err(MontyTaskBudgetError::ComputeExceeded));
    }

    #[test]
    fn resetting_an_execution_clock_fails_the_shared_task_closed() {
        let live = LiveMontyTaskSettings::new(settings(1, 600)).unwrap();
        let task = SharedMontyTaskBudget::new(live.clone());
        let mut clock = task.execution_clock(Duration::ZERO);
        clock.checkpoint(Duration::from_secs(20)).unwrap();
        assert_eq!(
            clock.checkpoint(Duration::ZERO),
            Err(MontyTaskBudgetError::ClockRegressed)
        );
        live.publish(1, settings(2, 120)).unwrap();
        assert_eq!(task.check(), Err(MontyTaskBudgetError::ClockRegressed));
        let mut replacement = task.execution_clock(Duration::ZERO);
        assert_eq!(
            replacement.checkpoint(Duration::from_secs(1)),
            Err(MontyTaskBudgetError::ClockRegressed)
        );
    }
}
