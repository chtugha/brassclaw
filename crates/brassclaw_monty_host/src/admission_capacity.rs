//! Logical ownership of admitted backlog, independent of channel/FIFO location.
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::Notify;

use crate::service::ServiceFailure;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionLimits {
    pub max_tasks: u32,
    /// Validated serialized Admit envelopes, not resident-memory estimates.
    pub max_bytes: usize,
}
impl AdmissionLimits {
    pub fn valid(self) -> bool {
        self.max_tasks > 0 && self.max_bytes > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionObservation {
    pub limits: AdmissionLimits,
    pub tasks: u32,
    pub bytes: usize,
}
impl AdmissionObservation {
    pub fn over_capacity(self) -> bool {
        self.tasks > self.limits.max_tasks || self.bytes > self.limits.max_bytes
    }
}

struct State {
    observation: AdmissionObservation,
    closed: bool,
}
pub(crate) struct AdmissionCapacity {
    state: Mutex<State>,
    changed: Notify,
}
impl AdmissionCapacity {
    pub(crate) fn new(limits: AdmissionLimits) -> Result<Arc<Self>, ServiceFailure> {
        if !limits.valid() {
            return Err(ServiceFailure::InvalidLimits);
        }
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                observation: AdmissionObservation {
                    limits,
                    tasks: 0,
                    bytes: 0,
                },
                closed: false,
            }),
            changed: Notify::new(),
        }))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| {
            let mut state = poisoned.into_inner();
            state.closed = true;
            self.changed.notify_waiters();
            state
        })
    }

    pub(crate) fn observation(&self) -> AdmissionObservation {
        self.lock().observation
    }

    /// Called only in the service's acknowledged settings-publication lane.
    pub(crate) fn publish(&self, limits: AdmissionLimits) -> Result<(), ServiceFailure> {
        if !limits.valid() {
            return Err(ServiceFailure::InvalidLimits);
        }
        let mut state = self.lock();
        if state.closed {
            return Err(ServiceFailure::Closed);
        }
        // Owned credits survive reductions. No underflow/debt erasure or replay.
        state.observation.limits = limits;
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_waiters();
    }

    /// Cheap preflight only; callers must still reserve the complete envelope.
    pub(crate) fn available(&self) -> Result<(), ServiceFailure> {
        let state = self.lock();
        if state.closed {
            return Err(ServiceFailure::Closed);
        }
        let observed = state.observation;
        if observed.tasks >= observed.limits.max_tasks
            || observed.bytes >= observed.limits.max_bytes
        {
            return Err(ServiceFailure::Backpressure);
        }
        Ok(())
    }

    pub(crate) async fn wait_available(&self) -> Result<(), ServiceFailure> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match self.available() {
                Err(ServiceFailure::Backpressure) => changed.await,
                result => return result,
            }
        }
    }

    pub(crate) fn try_reserve(
        self: &Arc<Self>,
        bytes: usize,
    ) -> Result<AdmissionCredit, ServiceFailure> {
        let mut state = self.lock();
        if state.closed {
            return Err(ServiceFailure::Closed);
        }
        let observed = state.observation;
        let tasks = observed
            .tasks
            .checked_add(1)
            .ok_or(ServiceFailure::Backpressure)?;
        let total = observed
            .bytes
            .checked_add(bytes)
            .ok_or(ServiceFailure::Backpressure)?;
        if tasks > observed.limits.max_tasks || total > observed.limits.max_bytes {
            return Err(ServiceFailure::Backpressure);
        }
        state.observation.tasks = tasks;
        state.observation.bytes = total;
        Ok(AdmissionCredit {
            owner: self.clone(),
            bytes,
        })
    }

    pub(crate) async fn reserve(
        self: &Arc<Self>,
        bytes: usize,
    ) -> Result<AdmissionCredit, ServiceFailure> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match self.try_reserve(bytes) {
                Err(ServiceFailure::Backpressure) => changed.await,
                result => return result,
            }
        }
    }
}

pub(crate) struct AdmissionCredit {
    owner: Arc<AdmissionCapacity>,
    bytes: usize,
}
impl Drop for AdmissionCredit {
    fn drop(&mut self) {
        let mut state = self.owner.lock();
        match (
            state.observation.tasks.checked_sub(1),
            state.observation.bytes.checked_sub(self.bytes),
        ) {
            (Some(tasks), Some(bytes)) => {
                state.observation.tasks = tasks;
                state.observation.bytes = bytes;
            }
            _ => state.closed = true,
        }
        drop(state);
        self.owner.changed.notify_waiters();
    }
}
