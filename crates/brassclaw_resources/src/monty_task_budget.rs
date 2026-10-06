//! Shared live task limits for Rust supervision and Monty tracking.
//!
//! Composition publishes one validated revision after durable settings commit.
//! Both consumers share this handle. Task usage lives outside the settings, so
//! publishing a shorter duration never resets an already running task's clock.
//! Shared heap accounting and external-call deadlines are separate contracts.

use std::time::Duration;

use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MontyTaskLimits {
    pub max_compute_time: Duration,
    pub max_allocations: u64,
    pub token_budgets_enabled: bool,
}

impl MontyTaskLimits {
    fn validate(self) -> Result<(), MontyTaskBudgetError> {
        if !(Duration::from_secs(30)..=Duration::from_secs(3600)).contains(&self.max_compute_time)
            || self.max_allocations == 0
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
    #[error("Monty task allocation budget exceeded")]
    AllocationsExceeded,
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

    /// Atomic compare-and-publish: time, allocations and token mode cannot be
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
    pub allocations: u64,
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

    pub fn record_allocations(&mut self, count: u64) -> Result<(), MontyTaskBudgetError> {
        let Some(total) = self.allocations.checked_add(count) else {
            self.overflowed = true;
            return Err(MontyTaskBudgetError::AccountingOverflow);
        };
        self.allocations = total;
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
        if self.allocations > limits.max_allocations {
            return Err(MontyTaskBudgetError::AllocationsExceeded);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(revision: u64, seconds: u64, allocations: u64) -> MontyTaskSettingsRevision {
        MontyTaskSettingsRevision {
            revision,
            limits: MontyTaskLimits {
                max_compute_time: Duration::from_secs(seconds),
                max_allocations: allocations,
                token_budgets_enabled: false,
            },
        }
    }

    #[tokio::test]
    async fn rust_and_vm_observe_same_live_revision_without_resetting_consumption() {
        let live = LiveMontyTaskSettings::new(settings(1, 600, 100)).unwrap();
        let mut rust = live.subscribe();
        let mut vm = live.subscribe();
        let mut usage = MontyTaskUsage::default();
        usage.record_compute_time(Duration::from_secs(60)).unwrap();
        usage.record_allocations(50).unwrap();
        live.publish(1, settings(2, 30, 40)).unwrap();
        rust.changed().await.unwrap();
        vm.changed().await.unwrap();
        assert_eq!(*rust.borrow_and_update(), *vm.borrow_and_update());
        assert_eq!(
            usage.check(live.current().limits),
            Err(MontyTaskBudgetError::ComputeExceeded)
        );
        live.publish(2, settings(3, 120, 40)).unwrap();
        assert_eq!(
            usage.check(live.current().limits),
            Err(MontyTaskBudgetError::AllocationsExceeded)
        );
        live.publish(3, settings(4, 120, 100)).unwrap();
        assert!(usage.check(live.current().limits).is_ok());
        assert_eq!(usage.compute_time, Duration::from_secs(60));
        assert_eq!(usage.allocations, 50);
    }

    #[test]
    fn stale_or_invalid_update_preserves_effective_revision() {
        let live = LiveMontyTaskSettings::new(settings(1, 600, 100)).unwrap();
        live.publish(1, settings(2, 60, 50)).unwrap();
        assert_eq!(
            live.publish(1, settings(3, 30, 25)),
            Err(MontyTaskBudgetError::RevisionConflict)
        );
        assert_eq!(
            live.publish(2, settings(3, 0, 25)),
            Err(MontyTaskBudgetError::InvalidSettings)
        );
        assert_eq!(live.current(), settings(2, 60, 50));
        assert_eq!(live.current().limits.token_budget(4096), None);
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;

    #[test]
    fn accounting_overflow_remains_fail_closed_after_limits_change() {
        let mut usage = MontyTaskUsage::default();
        usage.record_allocations(u64::MAX).unwrap();
        assert_eq!(
            usage.record_allocations(1),
            Err(MontyTaskBudgetError::AccountingOverflow)
        );
        let limits = MontyTaskLimits {
            max_compute_time: Duration::from_secs(600),
            max_allocations: u64::MAX,
            token_budgets_enabled: false,
        };
        assert_eq!(
            usage.check(limits),
            Err(MontyTaskBudgetError::AccountingOverflow)
        );
    }
}
