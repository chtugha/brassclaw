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
        if !(Duration::from_secs(30)..=Duration::from_secs(3600)).contains(&self.max_compute_time) {
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

/// One task's consumption, retained over steps and continuations. Record only
/// active execution segments; queue/idle/external waits do not debit this clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MontyTaskUsage {
    pub compute_time: Duration,
    overflowed: bool,
}

impl MontyTaskUsage {
    pub fn record_compute_time(&mut self, elapsed: Duration) -> Result<(), MontyTaskBudgetError> {
        let Some(total) = self.compute_time.checked_add(elapsed) else {
            self.overflowed = true;
            return Err(MontyTaskBudgetError::AccountingOverflow);
        };
        self.compute_time = total;
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

impl SharedMontyTaskBudget {
    pub fn new(settings: LiveMontyTaskSettings) -> Self {
        Self {
            settings,
            account: Arc::new(Mutex::new(TaskAccount::default())),
        }
    }

    /// Record each active execution segment exactly once, through the hosting
    /// owner. Rust and Monty readers use `check`, not separate usage counters.
    pub fn record_compute_time(&self, elapsed: Duration) -> Result<(), MontyTaskBudgetError> {
        let mut account = self
            .account
            .lock()
            .map_err(|_| MontyTaskBudgetError::AccountingUnavailable)?;
        if let Some(error) = &account.terminal {
            return Err(error.clone());
        }
        if let Err(error) = account.usage.record_compute_time(elapsed) {
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
}
