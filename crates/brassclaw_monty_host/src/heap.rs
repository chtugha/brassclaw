//! Serialized worker resource uptake. Adaptive capacity measurements and
//! operator settings are supplied by composition, never inferred from a task.
use serde::{Deserialize, Serialize};

use crate::{VmError, VmFailure};

fn limit_error(error: monty_alloc::WorkerMemoryLimitError) -> VmError {
    use monty_alloc::WorkerMemoryLimitError;
    VmError::kind(match error {
        WorkerMemoryLimitError::VmReduction | WorkerMemoryLimitError::PhysicalReduction => {
            VmFailure::UnsafeHeapReduction
        }
        WorkerMemoryLimitError::InvalidSettings | WorkerMemoryLimitError::Overflow => {
            VmFailure::InvalidBounds
        }
        WorkerMemoryLimitError::AccountingUnavailable => VmFailure::ResourceLimit,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeapSettings {
    pub revision: u64,
    pub max_vm_bytes: usize,
}

/// One explicit manual successor, coupled to a complete runtime settings edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeapUpdate {
    pub expected_revision: u64,
    pub settings: HeapSettings,
}

/// Policy for new exchanges and capacity required by credited older exchanges.
/// The transport owner supplies the latter from its retained-credit ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameUpdate {
    pub configured_frame_bytes: usize,
    pub required_capacity_bytes: usize,
}
impl FrameUpdate {
    pub(crate) fn valid(self) -> bool {
        crate::process::valid_frame_limit(self.configured_frame_bytes)
            && crate::process::valid_frame_limit(self.required_capacity_bytes)
            && self.required_capacity_bytes >= self.configured_frame_bytes
    }
}

/// Acknowledged worker framing, separate from the accepted bound of each RPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameStatus {
    pub configured_frame_bytes: usize,
    pub required_capacity_bytes: usize,
    pub effective_capacity_bytes: usize,
}
impl FrameStatus {
    pub fn pending_reduction(self) -> bool {
        self.required_capacity_bytes < self.effective_capacity_bytes
    }
    pub(crate) fn valid(self) -> bool {
        FrameUpdate {
            configured_frame_bytes: self.configured_frame_bytes,
            required_capacity_bytes: self.required_capacity_bytes,
        }
        .valid()
            && crate::process::valid_frame_limit(self.effective_capacity_bytes)
            && self.effective_capacity_bytes >= self.required_capacity_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeapStatus {
    pub desired: Option<HeapSettings>,
    pub effective: Option<HeapSettings>,
    pub pending_reduction: bool,
}

/// Acknowledged worker budget, relative to its allocator baseline; not RSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllocatorStatus {
    pub adapter_reserve_bytes: usize,
    pub non_vm_reserve_bytes: usize,
    pub memory_budget_bytes: usize,
}

/// One parent-side publication of coupled worker framing and allocator geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerMemoryStatus {
    pub frame: FrameStatus,
    pub allocator: AllocatorStatus,
}
impl HeapStatus {
    pub fn desired_revision(self) -> u64 {
        self.desired.map_or(0, |settings| settings.revision)
    }
}

pub(crate) struct WorkerHeap {
    status: HeapStatus,
    initial_max_soft_bytes: usize,
    non_vm_reserve_bytes: usize,
    frame: FrameStatus,
    adapter_reserve_bytes: usize,
    memory_budget_bytes: usize,
}
impl WorkerHeap {
    pub(crate) fn new(
        physical: usize,
        frame: usize,
        adapter_reserve: usize,
    ) -> Result<Self, VmError> {
        // Incoming frame, outgoing frame and exception/adapter headroom remain
        // separate from the logical domain. No cap is silently saturated.
        let non_vm_reserve_bytes = frame
            .checked_mul(2)
            .and_then(|frames| frames.checked_add(adapter_reserve))
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        let initial_max_soft_bytes = physical
            .checked_sub(non_vm_reserve_bytes)
            .filter(|capacity| *capacity > 0)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        Ok(Self {
            status: HeapStatus {
                desired: None,
                effective: None,
                pending_reduction: false,
            },
            initial_max_soft_bytes,
            non_vm_reserve_bytes,
            frame: FrameStatus {
                configured_frame_bytes: frame,
                required_capacity_bytes: frame,
                effective_capacity_bytes: frame,
            },
            adapter_reserve_bytes: adapter_reserve,
            memory_budget_bytes: physical,
        })
    }

    pub(crate) fn status(&self) -> HeapStatus {
        self.status
    }

    pub(crate) fn frame_status(&self) -> FrameStatus {
        self.frame
    }

    pub(crate) fn allocator_status(&self) -> AllocatorStatus {
        AllocatorStatus {
            adapter_reserve_bytes: self.adapter_reserve_bytes,
            non_vm_reserve_bytes: self.non_vm_reserve_bytes,
            memory_budget_bytes: self.memory_budget_bytes,
        }
    }

    /// Nonmutating feasibility at a serialized worker boundary. The observation
    /// does not reserve capacity or authorize a later, unfenced durable edit.
    pub(crate) fn validate_layout(
        &self,
        expected_revision: u64,
        vm_bytes: usize,
        frame_bytes: usize,
        adapter_reserve_bytes: usize,
    ) -> Result<(), VmError> {
        if expected_revision != self.status.desired_revision() {
            return Err(VmError::kind(VmFailure::SettingsRevisionConflict));
        }
        let reserve = frame_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(adapter_reserve_bytes))
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        if self.status.desired.is_some_and(|desired| {
            desired
                .max_vm_bytes
                .checked_add(reserve)
                .is_none_or(|bytes| bytes == usize::MAX)
        }) {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        monty_alloc::validate_worker_limits(vm_bytes, reserve).map_err(limit_error)
    }

    pub(crate) fn validate_runtime_update(
        &self,
        update: Option<HeapUpdate>,
        reserve: Option<usize>,
        frame: Option<FrameUpdate>,
    ) -> Result<(), VmError> {
        if update.is_none() && reserve.is_none() && frame.is_none() {
            return Ok(());
        }
        if frame.is_some_and(|frame| !frame.valid()) {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        let frame_bytes = frame.map_or(self.frame.effective_capacity_bytes, |frame| {
            frame.required_capacity_bytes
        });
        let reserve = reserve.unwrap_or(self.adapter_reserve_bytes);
        if let Some(update) = update {
            if update.expected_revision != self.status.desired_revision()
                || update.settings.revision <= update.expected_revision
            {
                return Err(VmError::kind(VmFailure::SettingsRevisionConflict));
            }
            let non_vm = frame_bytes
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(reserve))
                .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
            // This manual successor replaces a pending automatic target; the
            // discarded target cannot constrain its new reserve geometry.
            monty_alloc::validate_worker_limits(update.settings.max_vm_bytes, non_vm)
                .map_err(limit_error)
        } else {
            let current = self
                .status
                .effective
                .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
            self.validate_layout(
                self.status.desired_revision(),
                current.max_vm_bytes,
                frame_bytes,
                reserve,
            )
        }
    }

    /// Publish heap and reserve in one allocator operation. Validation and
    /// metadata assignment execute synchronously, with no intervening VM work.
    pub(crate) fn publish_runtime_update(
        &mut self,
        update: Option<HeapUpdate>,
        reserve: Option<usize>,
        frame: Option<FrameUpdate>,
    ) -> Result<(), VmError> {
        if update.is_none() && reserve.is_none() && frame.is_none() {
            return Ok(());
        }
        self.validate_runtime_update(update, reserve, frame)?;
        let frame_bytes = frame.map_or(self.frame.effective_capacity_bytes, |frame| {
            frame.required_capacity_bytes
        });
        let reserve = reserve.unwrap_or(self.adapter_reserve_bytes);
        let non_vm = frame_bytes
            .checked_mul(2)
            .and_then(|frames| frames.checked_add(reserve))
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        let current = update
            .map(|update| update.settings)
            .or(self.status.effective)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        let capacity = current
            .max_vm_bytes
            .checked_add(non_vm)
            .filter(|bytes| *bytes != usize::MAX)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        monty_alloc::set_worker_limits(current.max_vm_bytes, non_vm).map_err(limit_error)?;
        if update.is_some() {
            self.status = HeapStatus {
                desired: Some(current),
                effective: Some(current),
                pending_reduction: false,
            };
        }
        self.non_vm_reserve_bytes = non_vm;
        self.adapter_reserve_bytes = reserve;
        self.memory_budget_bytes = capacity;
        if let Some(frame) = frame {
            self.frame = FrameStatus {
                configured_frame_bytes: frame.configured_frame_bytes,
                required_capacity_bytes: frame.required_capacity_bytes,
                effective_capacity_bytes: frame.required_capacity_bytes,
            };
        }
        Ok(())
    }

    /// Release only transport capacity, retaining logical heap policy/revisions.
    /// A physical reduction below actual live allocation stays pending. This
    /// operation cannot pause admission or reset any task account.
    pub(crate) fn reconcile_frame_capacity(
        &mut self,
        configured: usize,
        required: usize,
    ) -> Result<(), VmError> {
        let target = FrameUpdate {
            configured_frame_bytes: configured,
            required_capacity_bytes: required,
        };
        if !target.valid()
            || configured != self.frame.configured_frame_bytes
            || required > self.frame.effective_capacity_bytes
        {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        match self.publish_runtime_update(None, None, Some(target)) {
            Ok(()) => Ok(()),
            Err(error) if error.failure == VmFailure::UnsafeHeapReduction => {
                self.frame.required_capacity_bytes = required;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn update(
        &mut self,
        expected: u64,
        next: HeapSettings,
        automatic: bool,
    ) -> Result<(), VmError> {
        if expected != self.status.desired_revision() || next.revision <= expected {
            return Err(VmError::kind(VmFailure::SettingsRevisionConflict));
        }
        if next.max_vm_bytes == 0
            || next
                .max_vm_bytes
                .checked_add(self.non_vm_reserve_bytes)
                .is_none_or(|bytes| bytes == usize::MAX)
            || (self.status.effective.is_none() && next.max_vm_bytes > self.initial_max_soft_bytes)
        {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        if next.max_vm_bytes < monty_alloc::vm_live_bytes() {
            if !automatic || self.status.effective.is_none() {
                return Err(VmError::kind(VmFailure::UnsafeHeapReduction));
            }
            self.status.desired = Some(next);
            self.status.pending_reduction = true;
        } else {
            match monty_alloc::set_worker_limits(next.max_vm_bytes, self.non_vm_reserve_bytes) {
                Ok(()) => {
                    self.memory_budget_bytes = next.max_vm_bytes + self.non_vm_reserve_bytes;
                    self.status = HeapStatus {
                        desired: Some(next),
                        effective: Some(next),
                        pending_reduction: false,
                    };
                }
                Err(monty_alloc::WorkerMemoryLimitError::PhysicalReduction)
                    if automatic && self.status.effective.is_some() =>
                {
                    // Bounded transport data may still be alive at this command.
                    // Defer the reduction rather than killing the worker or
                    // claiming a limit the allocator did not accept.
                    self.status.desired = Some(next);
                    self.status.pending_reduction = true;
                }
                Err(error) => return Err(limit_error(error)),
            }
        }
        Ok(())
    }

    pub(crate) fn reconcile(&mut self, retained_frame: usize) -> Result<(), VmError> {
        if self.status.pending_reduction {
            let desired = self
                .status
                .desired
                .ok_or_else(|| VmError::kind(VmFailure::ResourceLimit))?;
            if monty_alloc::vm_live_bytes() <= desired.max_vm_bytes {
                match monty_alloc::set_worker_limits(
                    desired.max_vm_bytes,
                    self.non_vm_reserve_bytes,
                ) {
                    Ok(()) => {
                        self.memory_budget_bytes = desired
                            .max_vm_bytes
                            .checked_add(self.non_vm_reserve_bytes)
                            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
                        self.status.effective = Some(desired);
                        self.status.pending_reduction = false;
                    }
                    Err(monty_alloc::WorkerMemoryLimitError::PhysicalReduction) => {}
                    Err(error) => return Err(limit_error(error)),
                }
            }
        }
        if self.frame.pending_reduction() && retained_frame <= self.frame.required_capacity_bytes {
            self.reconcile_frame_capacity(
                self.frame.configured_frame_bytes,
                self.frame.required_capacity_bytes,
            )?;
        }
        Ok(())
    }
}
