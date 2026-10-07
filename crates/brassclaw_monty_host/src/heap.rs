//! Serialized worker resource uptake. Adaptive capacity measurements and
//! operator settings are supplied by composition, never inferred from a task.
use serde::{Deserialize, Serialize};

use crate::{VmError, VmFailure};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeapSettings {
    pub revision: u64,
    pub max_vm_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeapStatus {
    pub desired: Option<HeapSettings>,
    pub effective: Option<HeapSettings>,
    pub pending_reduction: bool,
}
impl HeapStatus {
    pub fn desired_revision(self) -> u64 {
        self.desired.map_or(0, |settings| settings.revision)
    }
}

pub(crate) struct WorkerHeap {
    status: HeapStatus,
    max_soft_bytes: usize,
}
impl WorkerHeap {
    pub(crate) fn new(physical: usize, frame: usize) -> Result<Self, VmError> {
        // Incoming frame, outgoing frame and exception/adapter headroom remain
        // separate from the logical domain. No cap is silently saturated.
        let max_soft_bytes = frame
            .checked_mul(2)
            .and_then(|frames| frames.checked_add(4 * 1024 * 1024))
            .and_then(|reserve| physical.checked_sub(reserve))
            .filter(|capacity| *capacity > 0)
            .ok_or_else(|| VmError::kind(VmFailure::InvalidBounds))?;
        Ok(Self {
            status: HeapStatus {
                desired: None,
                effective: None,
                pending_reduction: false,
            },
            max_soft_bytes,
        })
    }

    pub(crate) fn status(&self) -> HeapStatus {
        self.status
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
        if next.max_vm_bytes == 0 || next.max_vm_bytes > self.max_soft_bytes {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        if next.max_vm_bytes < monty_alloc::vm_live_bytes() {
            if !automatic || self.status.effective.is_none() {
                return Err(VmError::kind(VmFailure::UnsafeHeapReduction));
            }
            self.status.desired = Some(next);
            self.status.pending_reduction = true;
        } else {
            monty_alloc::set_vm_limit(next.max_vm_bytes)
                .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
            self.status = HeapStatus {
                desired: Some(next),
                effective: Some(next),
                pending_reduction: false,
            };
        }
        Ok(())
    }

    pub(crate) fn reconcile(&mut self) -> Result<(), VmError> {
        if self.status.pending_reduction {
            let desired = self
                .status
                .desired
                .ok_or_else(|| VmError::kind(VmFailure::ResourceLimit))?;
            if monty_alloc::vm_live_bytes() <= desired.max_vm_bytes {
                monty_alloc::set_vm_limit(desired.max_vm_bytes)
                    .map_err(|_| VmError::kind(VmFailure::ResourceLimit))?;
                self.status.effective = Some(desired);
                self.status.pending_reduction = false;
            }
        }
        Ok(())
    }
}
