//! Private scheduling resources for ordinary dependent turns. These reservations
//! bind one actual parked parent continuation to one child run; they neither
//! select/execute a Recipe nor grant Tool authority. The process/service owners
//! must still transport, persist and settle these exact identities.
use super::*;

pub(super) struct DependentWorker {
    pub(super) parent: ContinuationKey,
    parent_token: String,
    child_run: Uuid,
    pub(super) child_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependentWorkerObservation {
    pub worker: u32,
    pub parent: ContinuationKey,
    pub parent_task: String,
    pub child_run: Uuid,
    pub child_task: Option<String>,
}

pub(crate) struct DependentWorkerPreparation {
    pub observation: DependentWorkerObservation,
    update: Option<Value>,
    contexts: usize,
    envelope_workers: u32,
}

impl GlobalVm {
    pub fn dependent_workers(&self) -> Vec<DependentWorkerObservation> {
        self.dependents
            .iter()
            .map(|(worker, reservation)| DependentWorkerObservation {
                worker: *worker,
                parent: reservation.parent,
                parent_task: reservation.parent_token.clone(),
                child_run: reservation.child_run,
                child_task: reservation.child_token.clone(),
            })
            .collect()
    }

    /// Count the union of desired slots, retained actual workers and reserved
    /// dependent slots, plus the coordinator. No array proportional to a target
    /// count is allocated, and a missing/poisoned account fails closed.
    pub(super) fn required_pool_calls(&self, workers: u32) -> Option<u32> {
        if workers == 0 {
            return None;
        }
        let account = self.control.account.lock().ok()?;
        let retained =
            u32::try_from(account.workers.keys().filter(|id| **id >= workers).count()).ok()?;
        let reserved = u32::try_from(
            self.dependents
                .keys()
                .filter(|id| **id >= workers && !account.workers.contains_key(id))
                .count(),
        )
        .ok()?;
        workers
            .checked_add(retained)?
            .checked_add(reserved)?
            .checked_add(1)
    }

    fn parked_parent(&self, key: ContinuationKey, token: Option<&str>) -> Result<String, VmError> {
        let pending = self
            .pending
            .get(&key)
            .filter(|pending| matches!(pending.kind, CallKind::Port))
            .ok_or_else(|| VmError::kind(VmFailure::ForeignContinuation))?;
        let context = pending
            .context
            .ok_or_else(|| VmError::kind(VmFailure::WrongBoundary))?;
        let account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        let task = account
            .tasks
            .get(&context)
            .filter(|task| {
                task.phase == TaskPhase::Running
                    && !task.cancel_requested
                    && token.is_none_or(|expected| task.token == expected)
            })
            .ok_or_else(|| VmError::kind(VmFailure::WrongBoundary))?;
        task.budget
            .check()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        Ok(task.token.clone())
    }

    pub(crate) fn prepare_dependent_worker(
        &self,
        parent: ContinuationKey,
        child_run: Uuid,
    ) -> Result<DependentWorkerPreparation, VmError> {
        if self.lifecycle != Lifecycle::Ready || child_run.is_nil() {
            return Err(VmError::kind(VmFailure::InvalidInputs));
        }
        let pool = self.worker_pool_capacity()?;
        if !pool.coordinator_waiting || !pool.target_present() {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        let parent_token = self.parked_parent(parent, None)?;
        {
            let account = self
                .control
                .account
                .lock()
                .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
            if account
                .tasks
                .values()
                .any(|task| task.run_id == child_run.to_string())
            {
                return Err(VmError::kind(VmFailure::InvalidInputs));
            }
        }
        if let Some((worker, reservation)) = self.dependents.iter().find(|(_, reservation)| {
            reservation.parent_token == parent_token || reservation.child_run == child_run
        }) {
            if reservation.parent == parent
                && reservation.parent_token == parent_token
                && reservation.child_run == child_run
            {
                return Ok(DependentWorkerPreparation {
                    observation: self
                        .dependent_workers()
                        .into_iter()
                        .find(|item| item.worker == *worker)
                        .expect("existing reservation"),
                    update: None,
                    contexts: 0,
                    envelope_workers: self.bounds.workers,
                });
            }
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        // Reuse an idle draining slot or the lowest vacant gather sibling.
        // A parked parent/unfinished child is never overwritten.
        let active: BTreeSet<_> = pool.active_ids.iter().copied().collect();
        let idle: BTreeSet<_> = self.work_waits().into_iter().map(|(id, _)| id).collect();
        let mut worker = self.desired_workers;
        while self.dependents.contains_key(&worker)
            || (active.contains(&worker) && !idle.contains(&worker))
        {
            worker = worker
                .checked_add(1)
                .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        }
        let newly_reserved = u32::from(!active.contains(&worker));
        let minimum = self
            .required_pool_calls(self.desired_workers)
            .and_then(|minimum| minimum.checked_add(newly_reserved))
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        if minimum as usize > self.bounds.max_pending_calls {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        let envelope_workers = worker
            .checked_add(1)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        let contexts = usize::try_from(minimum)
            .ok()
            .and_then(|minimum| minimum.checked_add(2))
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        let mut borrowed: Vec<_> = self.dependents.keys().copied().collect();
        borrowed.push(worker);
        borrowed.sort_unstable();
        let update = serde_json::json!({"workers": self.desired_workers,
            "active_ids": pool.active_ids, "borrowed_ids": borrowed});
        json_input(&update, self.bounds.values)?;
        Ok(DependentWorkerPreparation {
            observation: DependentWorkerObservation {
                worker,
                parent,
                parent_task: parent_token,
                child_run,
                child_task: None,
            },
            update: Some(update),
            contexts,
            envelope_workers,
        })
    }

    /// Reserve hosting resources, never select or execute a workflow. The owner
    /// must verify durable lineage separately and drive the returned boundary.
    pub fn reserve_dependent_worker(
        &mut self,
        parent: ContinuationKey,
        child_run: Uuid,
    ) -> Result<(u32, GlobalBoundary), VmError> {
        let prepared = self.prepare_dependent_worker(parent, child_run)?;
        let worker = prepared.observation.worker;
        let Some(update) = prepared.update else {
            return Ok((worker, GlobalBoundary::Waiting(self.outstanding())));
        };
        let coordinator = self
            .pool_wait()
            .ok_or_else(|| VmError::kind(VmFailure::WrongBoundary))?;
        let parent_token = prepared.observation.parent_task;
        self.dependents.insert(
            worker,
            DependentWorker {
                parent,
                parent_token,
                child_run,
                child_token: None,
            },
        );
        self.bounds.workers = self.bounds.workers.max(prepared.envelope_workers);
        self.control
            .max_contexts
            .store(prepared.contexts, Ordering::Release);
        // Execution errors after this boundary retain the reservation and every
        // real continuation for containment; they are never a clean rejection.
        self.resolve_inner(coordinator, HostAnswer::Return(update.into()), None)
            .map(|boundary| (worker, boundary))
    }

    pub(crate) fn dependent_admission(&self, key: ContinuationKey, input: &Value) -> bool {
        self.pending
            .get(&key)
            .is_some_and(|pending| match pending.kind {
                CallKind::WorkWait(worker) => {
                    self.dependents.contains_key(&worker)
                        && self.admission_slot_allowed(worker, input)
                }
                _ => false,
            })
    }
    pub(crate) fn active_extra_dependents(&self) -> Result<usize, VmError> {
        let account = self
            .control
            .account
            .lock()
            .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
        Ok(self
            .dependents
            .iter()
            .filter(|(worker, reservation)| {
                **worker >= self.desired_workers
                    && account
                        .workers
                        .get(worker)
                        .and_then(|context| account.tasks.get(context))
                        .is_some_and(|task| {
                            reservation.child_token.as_deref() == Some(task.token.as_str())
                        })
            })
            .count())
    }
    pub(crate) fn publish_dependent_worker(
        &mut self,
        parent: ContinuationKey,
        child_run: Uuid,
    ) -> Result<(), VmError> {
        let (worker, mut boundary) = self.reserve_dependent_worker(parent, child_run)?;
        loop {
            boundary = match boundary {
                GlobalBoundary::HostCall(call)
                    if matches!(call.name.as_str(), "await_next_task" | "await_pool_update") =>
                {
                    self.defer(call.continuation)?
                }
                GlobalBoundary::ControlYield(key) => self.resume_control(key)?,
                GlobalBoundary::Waiting(_) => {
                    let actual = self.worker_pool_capacity()?;
                    if actual.coordinator_waiting
                        && actual.target_present()
                        && actual.active_ids.binary_search(&worker).is_ok()
                        && (self
                            .dependents
                            .get(&worker)
                            .is_some_and(|loan| loan.child_token.is_some())
                            || self.dependent_worker_wait(worker).is_some())
                    {
                        return Ok(());
                    }
                    return Err(self.fail(VmError::kind(VmFailure::WrongBoundary)));
                }
                _ => return Err(self.fail(VmError::kind(VmFailure::WrongBoundary))),
            };
        }
    }

    /// Expose actual work waits only; a reservation or desired count does not
    /// claim that Python has created the corresponding coroutine.
    pub fn dependent_worker_wait(&self, worker: u32) -> Option<ContinuationKey> {
        self.dependents
            .contains_key(&worker)
            .then(|| {
                self.work_waits()
                    .into_iter()
                    .find_map(|(id, key)| (id == worker).then_some(key))
            })
            .flatten()
    }

    pub(super) fn admission_slot_allowed(&self, worker: u32, task: &Value) -> bool {
        let run = task.get("run_id").and_then(Value::as_str);
        if let Some(reservation) = self.dependents.get(&worker) {
            return reservation.child_token.is_none()
                && run == Some(reservation.child_run.to_string().as_str())
                && self
                    .parked_parent(reservation.parent, Some(&reservation.parent_token))
                    .is_ok();
        }
        worker < self.desired_workers
            && !self
                .dependents
                .values()
                .any(|reservation| run == Some(reservation.child_run.to_string().as_str()))
    }

    // Called with the real root account already locked, at the final insertion
    // boundary. Do not recursively acquire that lock through parked_parent.
    pub(super) fn admission_binding_valid(
        &self,
        key: ContinuationKey,
        binding: &RootTaskAccount,
        account: &RootAccount,
    ) -> bool {
        let Some(worker) = self
            .pending
            .get(&key)
            .and_then(|pending| match pending.kind {
                CallKind::WorkWait(worker) => Some(worker),
                _ => None,
            })
        else {
            return false;
        };
        if let Some(reservation) = self.dependents.get(&worker) {
            let parent = self
                .pending
                .get(&reservation.parent)
                .filter(|pending| matches!(pending.kind, CallKind::Port))
                .and_then(|pending| pending.context)
                .and_then(|context| account.tasks.get(&context));
            return reservation.child_token.as_deref() == Some(binding.token.as_str())
                && reservation.child_run.to_string() == binding.run_id
                && !account
                    .tasks
                    .values()
                    .any(|task| task.run_id == binding.run_id)
                && parent.is_some_and(|parent| {
                    parent.token == reservation.parent_token
                        && parent.phase == TaskPhase::Running
                        && !parent.cancel_requested
                        && parent.budget.check().is_ok()
                });
        }
        let active_dependent = self
            .dependents
            .iter()
            .filter(|(worker, reservation)| {
                **worker >= self.desired_workers
                    && account
                        .workers
                        .get(worker)
                        .and_then(|context| account.tasks.get(context))
                        .is_some_and(|task| {
                            reservation.child_token.as_deref() == Some(task.token.as_str())
                        })
            })
            .count();
        (self.desired_workers as usize)
            .checked_add(active_dependent)
            .is_some_and(|capacity| account.tasks.len() < capacity)
    }

    /// Release only after actual root completion AND the owner's child-context
    /// cleanup. `None` abandons an unadmitted reservation; it cannot free a task.
    /// The idle coroutine becomes drainable, or stays if now inside the target.
    pub fn release_dependent_worker(
        &mut self,
        worker: u32,
        child_token: Option<&str>,
    ) -> Result<(), VmError> {
        let pool = self.worker_pool_capacity()?;
        if self.lifecycle != Lifecycle::Ready
            || !pool.coordinator_waiting
            || self.dependent_worker_wait(worker).is_none()
        {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        let reservation = self
            .dependents
            .get(&worker)
            .expect("actual reserved work wait");
        if reservation.child_token.as_deref() != child_token
            || child_token.is_some_and(|token| !matches!(self.task_active(token), Ok(false)))
        {
            return Err(VmError::kind(VmFailure::WrongBoundary));
        }
        self.dependents.remove(&worker);
        Ok(())
    }
}
