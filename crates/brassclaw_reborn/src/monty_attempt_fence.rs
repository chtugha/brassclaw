//! Rust-owned admission and completion barrier for one Monty task attempt.
//!
//! This is a host-call barrier, not proof that an external provider or tool has
//! stopped. Kernel effect records remain authoritative for external outcomes.
//! Dropping an admitted call without observing its result requires reconciliation;
//! it must never be counted as a confirmed completion.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use brassclaw_turns::run_profile::{AgentLoopHostError, AgentLoopHostErrorKind, MontyTaskAttempt};
use parking_lot::Mutex;
use tokio::{sync::Notify, time::Instant};

/// Technical bound on retained concurrent host calls, separate from token budgets.
const MAX_IN_FLIGHT_CALLS: usize = 64;

/// Rust-only local address; scope is the owning fence's exact attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MontyHostCallId(u64);

/// Acknowledges the state of Rust host calls, never external-effect quiescence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MontyFenceReceipt {
    /// Calls whose futures have not returned or been dropped yet.
    pub pending_calls: usize,
    /// Futures dropped without a result. Retain the task's existing effect/audit
    /// references for reconciliation; these IDs do not authorize replay.
    pub abandoned_calls: Vec<MontyHostCallId>,
}

impl MontyFenceReceipt {
    pub fn calls_settled(&self) -> bool {
        self.pending_calls == 0 && self.abandoned_calls.is_empty()
    }
}

#[derive(Default)]
struct CallState {
    fenced: bool,
    next_id: u64,
    active: BTreeSet<MontyHostCallId>,
    abandoned: BTreeSet<MontyHostCallId>,
}

#[derive(Default)]
struct FenceState {
    calls: Mutex<CallState>,
    changed: Notify,
}

/// Retained by the trusted supervisor. Never expose this handle or its attempt
/// address to Python/model payloads. A fence cannot be reopened or reused.
#[derive(Clone)]
pub struct MontyTaskFence {
    attempt: MontyTaskAttempt,
    state: Arc<FenceState>,
}

impl MontyTaskFence {
    pub(crate) fn new(attempt: MontyTaskAttempt) -> Self {
        Self {
            attempt,
            state: Arc::default(),
        }
    }

    pub(crate) fn close(&self) {
        self.state.calls.lock().fenced = true;
        self.state.changed.notify_waiters();
    }

    pub(crate) fn begin_call(&self) -> Result<MontyHostCall, AgentLoopHostError> {
        let mut calls = self.state.calls.lock();
        if calls.fenced {
            return Err(cancelled());
        }
        if calls.active.len() >= MAX_IN_FLIGHT_CALLS {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::Unavailable,
                "Monty task host-call capacity exhausted",
            ));
        }
        calls.next_id = calls.next_id.checked_add(1).ok_or_else(|| {
            AgentLoopHostError::new(
                AgentLoopHostErrorKind::Unavailable,
                "Monty task host-call address space exhausted",
            )
        })?;
        let id = MontyHostCallId(calls.next_id);
        calls.active.insert(id);
        Ok(MontyHostCall {
            id,
            state: Arc::clone(&self.state),
            finished: false,
        })
    }

    fn receipt(&self) -> MontyFenceReceipt {
        let calls = self.state.calls.lock();
        MontyFenceReceipt {
            pending_calls: calls.active.len(),
            abandoned_calls: calls.abandoned.iter().copied().collect(),
        }
    }

    /// Fence only this exact attempt, then wait for its admitted Rust host calls
    /// up to the caller's explicit acknowledgement deadline. A timeout returns
    /// pending evidence rather than pretending cancellation completed. An old
    /// attempt cannot fence a replacement attempt with the same durable run ID.
    pub async fn fence_and_wait(
        &self,
        attempt: MontyTaskAttempt,
        acknowledgement_timeout: Duration,
    ) -> Result<MontyFenceReceipt, AgentLoopHostError> {
        if attempt != self.attempt {
            return Err(AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "Monty cancellation does not address this task attempt",
            ));
        }
        let deadline = Instant::now()
            .checked_add(acknowledgement_timeout)
            .ok_or_else(|| {
                AgentLoopHostError::new(
                    AgentLoopHostErrorKind::InvalidInvocation,
                    "Monty cancellation acknowledgement deadline is out of range",
                )
            })?;
        self.close();
        loop {
            // Register before inspecting state: completion between the snapshot
            // and await must not lose its wakeup, including concurrent waiters.
            let changed = self.state.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let receipt = self.receipt();
            if receipt.pending_calls == 0 {
                return Ok(receipt);
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Ok(self.receipt());
            }
        }
    }
}

pub(crate) struct MontyHostCall {
    id: MontyHostCallId,
    state: Arc<FenceState>,
    finished: bool,
}

impl MontyHostCall {
    /// Preserve an actual success before withholding it from a fenced task.
    /// The callback owns the value; recording does not authorize continuation.
    pub(crate) fn finish_retaining<T>(
        mut self,
        result: Result<T, AgentLoopHostError>,
        retain: impl FnOnce(MontyHostCallId, T),
    ) -> Result<T, AgentLoopHostError> {
        let result = {
            let mut calls = self.state.calls.lock();
            // Recording must finish before removing the active call or waking
            // acknowledgement waiters. This callback is private synchronous
            // storage only; it must not reenter the fence or call host ports.
            let result = match result {
                Ok(value) if calls.fenced => {
                    retain(self.id, value);
                    Err(cancelled())
                }
                other => other,
            };
            calls.active.remove(&self.id);
            result
        };
        self.finished = true;
        self.state.changed.notify_waiters();
        result
    }
}

impl Drop for MontyHostCall {
    fn drop(&mut self) {
        if !self.finished {
            let mut calls = self.state.calls.lock();
            calls.active.remove(&self.id);
            calls.abandoned.insert(self.id);
            calls.fenced = true;
            drop(calls);
            self.state.changed.notify_waiters();
        }
    }
}

fn cancelled() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::Cancelled,
        "Monty task attempt is fenced",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_turns::{TurnLeaseToken, TurnRunId, TurnRunnerId};

    fn attempt() -> MontyTaskAttempt {
        MontyTaskAttempt {
            run_id: TurnRunId::new(),
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
        }
    }

    #[tokio::test]
    async fn fence_bounds_wait_and_rejects_late_success_without_losing_host_errors() {
        let address = attempt();
        let fence = MontyTaskFence::new(address);
        let first = fence.begin_call().unwrap();
        let second = fence.begin_call().unwrap();
        let pending = fence.fence_and_wait(address, Duration::ZERO).await.unwrap();
        assert_eq!(pending.pending_calls, 2);
        assert!(!pending.calls_settled());
        assert_eq!(
            fence.begin_call().err().unwrap().kind,
            AgentLoopHostErrorKind::Cancelled
        );
        let first_id = first.id;
        let mut retained = None;
        assert_eq!(
            first
                .finish_retaining(Ok("late value"), |id, value| retained = Some((id, value)))
                .unwrap_err()
                .kind,
            AgentLoopHostErrorKind::Cancelled
        );
        assert_eq!(retained, Some((first_id, "late value")));
        let error =
            AgentLoopHostError::new(AgentLoopHostErrorKind::Unavailable, "original host error");
        assert_eq!(
            second
                .finish_retaining::<()>(Err(error.clone()), |_, _| panic!(
                    "errors must remain errors"
                ))
                .unwrap_err(),
            error
        );
        assert!(
            fence
                .fence_and_wait(address, Duration::ZERO)
                .await
                .unwrap()
                .calls_settled()
        );
    }

    #[tokio::test]
    async fn dropped_call_requires_reconciliation_and_cannot_reopen_the_attempt() {
        let address = attempt();
        let fence = MontyTaskFence::new(address);
        let call = fence.begin_call().unwrap();
        let id = call.id;
        drop(call);
        let receipt = fence.fence_and_wait(address, Duration::ZERO).await.unwrap();
        assert_eq!(receipt.pending_calls, 0);
        assert_eq!(receipt.abandoned_calls, vec![id]);
        assert!(!receipt.calls_settled());
        assert_eq!(
            fence.begin_call().err().unwrap().kind,
            AgentLoopHostErrorKind::Cancelled
        );
    }

    #[tokio::test]
    async fn wrong_attempt_cannot_fence_a_reclaimed_run_and_waiters_do_not_lose_completion() {
        let address = attempt();
        let fence = MontyTaskFence::new(address);
        let wrong = MontyTaskAttempt {
            lease_token: TurnLeaseToken::new(),
            ..address
        };
        assert_eq!(
            fence
                .fence_and_wait(wrong, Duration::ZERO)
                .await
                .unwrap_err()
                .kind,
            AgentLoopHostErrorKind::InvalidInvocation
        );
        let call = fence.begin_call().unwrap();
        // Biased join polls both waits before completing the call. This exercises
        // registered waiters rather than only a completion-before-wait snapshot.
        let recorded = std::sync::atomic::AtomicBool::new(false);
        let (first, second, ()) = tokio::join!(
            biased;
            fence.fence_and_wait(address, Duration::from_secs(1)),
            fence.fence_and_wait(address, Duration::from_secs(1)),
            async { assert!(call.finish_retaining(Ok(()), |_, _| recorded.store(true, std::sync::atomic::Ordering::Release)).is_err()); }
        );
        assert!(recorded.load(std::sync::atomic::Ordering::Acquire));
        assert!(first.unwrap().calls_settled());
        assert!(second.unwrap().calls_settled());
    }
}
