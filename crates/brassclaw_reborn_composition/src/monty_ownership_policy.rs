//! Revisioned ownership-check admission and captured deadlines. No Tool grants.
use super::OwnershipError;
use brassclaw_host_api::MontyExecutionLimits;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{Notify, oneshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OwnershipLimits {
    pub(crate) check_timeout: Duration,
    pub(crate) heartbeat_interval: Duration,
    pub(crate) max_pending_checks: u32,
}
impl OwnershipLimits {
    pub(crate) fn from_execution(limits: MontyExecutionLimits) -> Self {
        Self {
            check_timeout: Duration::from_millis(limits.ownership_check_timeout_millis),
            heartbeat_interval: Duration::from_millis(limits.ownership_heartbeat_interval_millis),
            max_pending_checks: limits.max_pending_ownership_checks,
        }
    }
    pub(crate) fn valid(self) -> bool {
        let now = Instant::now();
        self.max_pending_checks > 0
            && !self.check_timeout.is_zero()
            && !self.heartbeat_interval.is_zero()
            && now.checked_add(self.check_timeout).is_some()
            && now.checked_add(self.heartbeat_interval).is_some()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OwnershipSnapshot {
    pub(crate) revision: u64,
    pub(crate) limits: OwnershipLimits,
    pub(crate) pending_checks: u64,
}
impl OwnershipSnapshot {
    pub(crate) fn over_capacity(self) -> bool {
        self.pending_checks > u64::from(self.limits.max_pending_checks)
    }
}
#[derive(Clone)]
pub(crate) struct OwnershipControl {
    state: Arc<Mutex<OwnershipSnapshot>>,
    wake: Arc<Notify>,
}
impl OwnershipControl {
    pub(crate) fn new(revision: u64, limits: OwnershipLimits) -> Result<Self, OwnershipError> {
        if revision == 0 || !limits.valid() {
            return Err(OwnershipError::Connection);
        }
        Ok(Self {
            state: Arc::new(Mutex::new(OwnershipSnapshot {
                revision,
                limits,
                pending_checks: 0,
            })),
            wake: Arc::new(Notify::new()),
        })
    }
    pub(crate) fn snapshot(&self) -> Result<OwnershipSnapshot, OwnershipError> {
        self.state
            .lock()
            .map(|state| *state)
            .map_err(|_| OwnershipError::Connection)
    }
    pub(crate) fn publish(
        &self,
        expected: u64,
        revision: u64,
        limits: OwnershipLimits,
    ) -> Result<(), OwnershipError> {
        if revision <= expected || !limits.valid() {
            return Err(OwnershipError::Connection);
        }
        {
            let mut state = self.state.lock().map_err(|_| OwnershipError::Connection)?;
            if state.revision != expected {
                return Err(OwnershipError::Connection);
            }
            // Debt survives reductions. Only real request drop refunds credit.
            state.revision = revision;
            state.limits = limits;
        }
        self.wake.notify_one();
        Ok(())
    }
    pub(crate) async fn changed(&self) {
        self.wake.notified().await;
    }
    pub(crate) fn request(
        &self,
        answer: oneshot::Sender<Result<(), OwnershipError>>,
    ) -> Result<OwnershipRequest, OwnershipError> {
        let deadline;
        {
            let mut state = self.state.lock().map_err(|_| OwnershipError::Connection)?;
            if state.pending_checks >= u64::from(state.limits.max_pending_checks) {
                return Err(OwnershipError::Busy);
            }
            deadline = Instant::now()
                .checked_add(state.limits.check_timeout)
                .ok_or(OwnershipError::Connection)?;
            state.pending_checks = state
                .pending_checks
                .checked_add(1)
                .ok_or(OwnershipError::Connection)?;
        }
        Ok(OwnershipRequest {
            answer,
            deadline,
            _credit: OwnershipCredit(self.clone()),
        })
    }
}

pub(crate) struct OwnershipRequest {
    pub(crate) answer: oneshot::Sender<Result<(), OwnershipError>>,
    pub(crate) deadline: Instant,
    _credit: OwnershipCredit,
}
struct OwnershipCredit(OwnershipControl);
impl Drop for OwnershipCredit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            // Each credit is constructed only after a successful reservation.
            // No saturating subtraction conceals an accounting violation.
            state.pending_checks -= 1;
        }
        // A poisoned ledger remains unavailable, never a fabricated refund.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits(capacity: u32, millis: u64) -> OwnershipLimits {
        OwnershipLimits {
            max_pending_checks: capacity,
            check_timeout: Duration::from_millis(millis),
            heartbeat_interval: Duration::from_millis(2000),
        }
    }
    #[test]
    fn accepted_requests_keep_deadlines_and_credit_after_caller_disappears() {
        let control = OwnershipControl::new(1, limits(8, 2000)).unwrap();
        let (answer, waiter) = oneshot::channel();
        let accepted = control.request(answer).unwrap();
        let captured = accepted.deadline;
        drop(waiter);
        control.publish(1, 2, limits(1, 200)).unwrap();
        assert_eq!(accepted.deadline, captured);
        assert_eq!(control.snapshot().unwrap().pending_checks, 1);
        let (answer, _waiter) = oneshot::channel();
        assert!(control.request(answer).is_err());
        drop(accepted);
        assert_eq!(control.snapshot().unwrap().pending_checks, 0);
        let (answer, _waiter) = oneshot::channel();
        let next = control.request(answer).unwrap();
        assert!(
            next.deadline
                <= Instant::now()
                    .checked_add(Duration::from_millis(200))
                    .unwrap()
        );
    }
    #[test]
    fn live_growth_beyond_eight_and_reduction_keep_actual_debt() {
        let control = OwnershipControl::new(1, limits(8, 2000)).unwrap();
        control.publish(1, 2, limits(1025, 2000)).unwrap();
        let mut requests = Vec::new();
        for _ in 0..1025 {
            let (answer, _waiter) = oneshot::channel();
            requests.push(control.request(answer).unwrap());
        }
        control.publish(2, 3, limits(1, 2000)).unwrap();
        assert!(control.snapshot().unwrap().over_capacity());
        requests.clear();
        assert_eq!(control.snapshot().unwrap().pending_checks, 0);
        assert!(!control.snapshot().unwrap().over_capacity());
        assert!(control.publish(2, 4, limits(8, 2000)).is_err());
        assert!(control.publish(3, 4, limits(0, 2000)).is_err());
        assert_eq!(control.snapshot().unwrap().revision, 3);
    }
}
