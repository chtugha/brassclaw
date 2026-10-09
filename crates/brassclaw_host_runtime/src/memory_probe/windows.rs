use std::{mem::size_of, ptr, time::Instant};

use brassclaw_resources::{MontyMemoryPressure, MontyMemorySample};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        JobObjects::IsProcessInJob,
        Memory::{
            CreateMemoryResourceNotification, LowMemoryResourceNotification,
            QueryMemoryResourceNotification,
        },
        ProcessStatus::{K32GetPerformanceInfo, PERFORMANCE_INFORMATION},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

use super::HostMemoryProbeError;

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: construction checks non-null; this is the sole owning handle.
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn sample(worker_pid: u32) -> Result<MontyMemorySample, HostMemoryProbeError> {
    let measured_at = Instant::now();
    // SAFETY: read-only query access; the supplied PID is a scalar.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, worker_pid) };
    if process.is_null() {
        return Err(HostMemoryProbeError::Unavailable);
    }
    let process = OwnedHandle(process);
    let mut in_job = 0;
    // SAFETY: live process handle and writable BOOL; null job queries membership
    // in any job. No job or process state is modified.
    if unsafe { IsProcessInJob(process.0, ptr::null_mut(), &mut in_job) } == 0 {
        return Err(HostMemoryProbeError::Unavailable);
    }
    if in_job != 0 {
        // QueryInformationJobObject(NULL) exposes only the immediate job.
        // PeakJobMemoryUsed is not current usage and cannot reveal nested parent
        // limits. An authoritative job-aware adapter is required in this case.
        return Err(HostMemoryProbeError::ContainingLimitsUnknown);
    }
    host_capacity(measured_at)
}

fn host_capacity(measured_at: Instant) -> Result<MontyMemorySample, HostMemoryProbeError> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    let mut performance = PERFORMANCE_INFORMATION {
        cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
        ..Default::default()
    };
    let performance_size = performance.cb;
    // SAFETY: both OS structures are initialised with their exact ABI sizes.
    if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0
        || unsafe { K32GetPerformanceInfo(&mut performance, performance_size) } == 0
    {
        return Err(HostMemoryProbeError::Unavailable);
    }
    if memory.ullTotalPhys == 0
        || memory.ullAvailPhys > memory.ullTotalPhys
        || performance.PageSize == 0
        || performance.CommitTotal > performance.CommitLimit
    {
        return Err(HostMemoryProbeError::Inconsistent);
    }
    let commit_headroom = (performance.CommitLimit - performance.CommitTotal)
        .checked_mul(performance.PageSize)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(HostMemoryProbeError::Inconsistent)?;
    // SAFETY: returns an owned event handle for a defined notification type.
    let notification = unsafe { CreateMemoryResourceNotification(LowMemoryResourceNotification) };
    if notification.is_null() {
        return Err(HostMemoryProbeError::Unavailable);
    }
    let notification = OwnedHandle(notification);
    let mut low = 0;
    // SAFETY: valid notification handle, writable BOOL. This queries state
    // without waiting, changing the event or allocating/reclaiming VM pages.
    if unsafe { QueryMemoryResourceNotification(notification.0, &mut low) } == 0 {
        return Err(HostMemoryProbeError::Unavailable);
    }
    Ok(MontyMemorySample {
        additional_capacity_bytes: memory.ullAvailPhys.min(commit_headroom),
        pressure: if low != 0 {
            MontyMemoryPressure::Critical
        } else {
            MontyMemoryPressure::Normal
        },
        measured_at,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_capacity_or_containment_is_reported_explicitly() {
        // Exercise the actual RAM/commit/pressure APIs even when the CI worker
        // is contained in a job and cannot supply an authoritative budget.
        let host =
            super::host_capacity(std::time::Instant::now()).expect("actual Windows counters");
        assert!(host.measured_at <= std::time::Instant::now());
        match super::sample(std::process::id()) {
            Ok(sample) => assert!(sample.measured_at <= std::time::Instant::now()),
            Err(super::HostMemoryProbeError::ContainingLimitsUnknown) => {
                // Hosted CI runners commonly execute inside a Windows job.
            }
            Err(error) => panic!("actual Windows memory probe failed: {error}"),
        }
    }
}
