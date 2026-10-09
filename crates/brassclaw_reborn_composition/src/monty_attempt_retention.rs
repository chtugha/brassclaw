//! Shared private attempt retention. Credits count Rust ownership, not Tool
//! authority; claims and admission addresses never cross the VM/status boundary.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use brassclaw_monty_host::service::{RetainedAttemptCredit, ServiceClient, ServiceFailure};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_turns::run_profile::{AgentLoopDriverError, MontyTaskAttempt};

/// Both driver and factory retain this same lease. Successful durable settlement
/// explicitly refunds the credit; unresolved evidence retains it. A private
/// reconciliation transfer must transfer this lease together with that evidence.
pub(crate) struct AttemptRetentionLease {
    host: Weak<MontyTaskHost>,
    credit: Mutex<Option<RetainedAttemptCredit>>,
}
impl AttemptRetentionLease {
    pub(crate) fn ensure_active(
        &self,
        host: &Arc<MontyTaskHost>,
    ) -> Result<(), AgentLoopDriverError> {
        if self
            .host
            .upgrade()
            .is_none_or(|original| !Arc::ptr_eq(&original, host))
            || self
                .credit
                .lock()
                .map_err(|_| failed("monty_retention_registry_failed"))?
                .is_none()
        {
            return Err(failed("monty_admission_replay_requires_recovery"));
        }
        Ok(())
    }

    /// Only the owning factory calls this after verifying the real completed
    /// service receipt and acknowledging its exact durable admission outcome.
    pub(crate) fn completed(&self) -> Result<(), AgentLoopDriverError> {
        let credit = self
            .credit
            .lock()
            .map_err(|_| failed("monty_retention_registry_failed"))?
            .take();
        drop(credit);
        Ok(())
    }
}

pub(crate) struct AttemptRetentionRegistry {
    service: ServiceClient,
    leases: Mutex<HashMap<MontyTaskAttempt, Weak<AttemptRetentionLease>>>,
}
impl AttemptRetentionRegistry {
    pub(crate) fn new(service: ServiceClient) -> Arc<Self> {
        Arc::new(Self {
            service,
            leases: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn bound_to(&self, service: &ServiceClient) -> bool {
        self.service.root_identity() == service.root_identity()
    }

    pub(crate) fn retain(
        &self,
        host: &Arc<MontyTaskHost>,
    ) -> Result<Arc<AttemptRetentionLease>, AgentLoopDriverError> {
        let mut leases = self
            .leases
            .lock()
            .map_err(|_| failed("monty_retention_registry_failed"))?;
        // Destructors never remove map entries. Cleanup under this lock cannot
        // erase a newly reserved generation when an old weak reference expires.
        leases.retain(|_, lease| lease.strong_count() > 0);
        if let Some(lease) = leases.get(&host.attempt()).and_then(Weak::upgrade) {
            lease.ensure_active(host)?;
            return Ok(lease);
        }
        let credit = self
            .service
            .try_retain_attempt()
            .map_err(|error| match error {
                ServiceFailure::Backpressure => AgentLoopDriverError::Unavailable {
                    reason: "Monty retained attempt capacity exhausted".into(),
                },
                ServiceFailure::Closed => failed("monty_instance_ownership_failed"),
                _ => failed("monty_retention_unavailable"),
            })?;
        let lease = Arc::new(AttemptRetentionLease {
            host: Arc::downgrade(host),
            credit: Mutex::new(Some(credit)),
        });
        leases.insert(host.attempt(), Arc::downgrade(&lease));
        Ok(lease)
    }
}
fn failed(reason: &str) -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: reason.into(),
    }
}
