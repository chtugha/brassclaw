//! Historical resource-accounting proof for the pinned Monty v0.0.16.
//!
//! The pinned interpreter accepts a custom ResourceTracker. We do not mutate a
//! LimitedTracker or reset its duration clock on resume. The host-owned account
//! provides task compute/allocation usage and a shared service heap account.
//! This test-only adapter must not be wired into production: Monty 1.0 removed
//! this trait. Its passing tests do not certify the 1.0 resource contract.

use std::{fmt::Debug, sync::Arc};

use monty::{ResourceError, ResourceTracker};

/// Resource accounting only; this port cannot dispatch tools or sequence tasks.
/// All methods are synchronous because the VM calls them inside execution.
/// Implementations must read coherent live settings, fail closed on accounting
/// failure, and preserve counters over parked/resumed and nested execution.
/// Compute time is measured by host-managed active VM execution segments,
/// excluding external waits. Every tracker owns its heap debit; freeing/dropping
/// one context must not release another context's live memory.
trait MontyResourceBudgetPort: Debug + Send + Sync {
    fn allocate(&self, bytes: usize) -> Result<(), ResourceError>;
    fn free(&self, bytes: usize);
    fn grow(&self, bytes: usize) -> Result<(), ResourceError>;
    fn check_time(&self) -> Result<(), ResourceError>;
    fn check_recursion_depth(&self, current_depth: usize) -> Result<(), ResourceError>;
    fn check_large_result(&self, estimated_bytes: usize) -> Result<(), ResourceError>;
}

/// One execution-context tracker. The port's task account can be shared with
/// nested contexts; its heap ownership must remain specific to this context.
#[derive(Debug)]
struct LiveMontyResourceTracker {
    budget: Arc<dyn MontyResourceBudgetPort>,
}

impl LiveMontyResourceTracker {
    fn new(budget: Arc<dyn MontyResourceBudgetPort>) -> Self {
        Self { budget }
    }
}

impl ResourceTracker for LiveMontyResourceTracker {
    fn on_allocate(&self, get_size: impl FnOnce() -> usize) -> Result<(), ResourceError> {
        self.budget.allocate(get_size())
    }
    fn on_free(&self, get_size: impl FnOnce() -> usize) {
        self.budget.free(get_size());
    }
    fn on_grow(&self, additional_bytes: usize) -> Result<(), ResourceError> {
        self.budget.grow(additional_bytes)
    }
    fn check_time(&self) -> Result<(), ResourceError> {
        self.budget.check_time()
    }
    fn check_recursion_depth(&self, current_depth: usize) -> Result<(), ResourceError> {
        self.budget.check_recursion_depth(current_depth)
    }
    fn check_large_result(&self, estimated_bytes: usize) -> Result<(), ResourceError> {
        self.budget.check_large_result(estimated_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monty::{ExtFunctionResult, MontyObject, MontyRun, PrintWriter, RunProgress};
    use std::{sync::Mutex, time::Duration};

    #[derive(Debug)]
    struct Account {
        state: Mutex<(usize, usize)>, // cumulative allocations, live bytes
        allocation_limit: Mutex<usize>,
        compute_limit: Mutex<Duration>,
        consumed_compute: Mutex<Duration>,
    }
    impl MontyResourceBudgetPort for Account {
        fn allocate(&self, bytes: usize) -> Result<(), ResourceError> {
            let limit = *self.allocation_limit.lock().expect("limit lock");
            let mut state = self.state.lock().expect("account lock");
            if state.0 >= limit {
                return Err(ResourceError::Allocation {
                    limit,
                    count: state.0 + 1,
                });
            }
            state.0 += 1;
            state.1 = state.1.checked_add(bytes).expect("test memory overflow");
            Ok(())
        }
        fn free(&self, bytes: usize) {
            let mut state = self.state.lock().expect("account lock");
            state.1 = state.1.checked_sub(bytes).expect("test memory underflow");
        }
        fn grow(&self, bytes: usize) -> Result<(), ResourceError> {
            let mut state = self.state.lock().expect("account lock");
            state.1 = state.1.checked_add(bytes).expect("test memory overflow");
            Ok(())
        }
        fn check_time(&self) -> Result<(), ResourceError> {
            let limit = *self.compute_limit.lock().expect("time limit lock");
            let elapsed = *self.consumed_compute.lock().expect("compute account lock");
            if elapsed > limit {
                Err(ResourceError::Time { limit, elapsed })
            } else {
                Ok(())
            }
        }
        fn check_recursion_depth(&self, _: usize) -> Result<(), ResourceError> {
            Ok(())
        }
        fn check_large_result(&self, _: usize) -> Result<(), ResourceError> {
            Ok(())
        }
    }

    #[test]
    fn live_allocation_change_reaches_parked_vm_without_resetting_counter() {
        let account = Arc::new(Account {
            state: Mutex::new((0, 0)),
            allocation_limit: Mutex::new(100_000),
            compute_limit: Mutex::new(Duration::from_secs(600)),
            consumed_compute: Mutex::new(Duration::ZERO),
        });
        let tracker = LiveMontyResourceTracker::new(account.clone());
        let run = MontyRun::new(
            "values = []\npause()\nresult = 'x' * 10000\nresult".into(),
            "live-budget.py",
            vec![],
        )
        .unwrap();
        let progress = run.start(vec![], tracker, PrintWriter::Disabled).unwrap();
        let RunProgress::FunctionCall(call) = progress else {
            panic!("expected external wait")
        };
        let consumed = account.state.lock().expect("account lock").0;
        assert!(consumed > 0);
        *account.allocation_limit.lock().expect("limit lock") = consumed;
        let error = call
            .resume(
                ExtFunctionResult::Return(MontyObject::None),
                PrintWriter::Disabled,
            )
            .unwrap_err();
        assert!(error.to_string().contains("allocation limit exceeded"));
        assert_eq!(account.state.lock().expect("account lock").0, consumed);
    }
    #[test]
    fn rust_and_parked_vm_enforce_same_duration_without_consumption_reset() {
        let account = Arc::new(Account {
            state: Mutex::new((0, 0)),
            allocation_limit: Mutex::new(100_000),
            compute_limit: Mutex::new(Duration::from_secs(600)),
            consumed_compute: Mutex::new(Duration::ZERO),
        });
        let run = MontyRun::new(
            "pause()\nresult = 'x' * 10000\nresult".into(),
            "live-time.py",
            vec![],
        )
        .unwrap();
        let progress = run
            .start(
                vec![],
                LiveMontyResourceTracker::new(account.clone()),
                PrintWriter::Disabled,
            )
            .unwrap();
        let RunProgress::FunctionCall(call) = progress else {
            panic!("expected external wait")
        };
        // Simulate accumulated active compute segments, not wall-clock wait.
        *account
            .consumed_compute
            .lock()
            .expect("compute account lock") = Duration::from_secs(60);
        *account.compute_limit.lock().expect("time limit lock") = Duration::from_secs(30);
        assert!(matches!(
            account.check_time(),
            Err(ResourceError::Time { .. })
        ));
        let error = call
            .resume(
                ExtFunctionResult::Return(MontyObject::None),
                PrintWriter::Disabled,
            )
            .unwrap_err();
        assert!(error.to_string().contains("time limit exceeded"));
        assert_eq!(
            *account
                .consumed_compute
                .lock()
                .expect("compute account lock"),
            Duration::from_secs(60)
        );
    }
}
