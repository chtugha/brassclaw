//! Native capacity measurement for the adaptive Monty budget controller.
//!
//! No subprocesses, page walks, forced reclamation or background task lives here.
//! The caller supplies the actual supervised worker PID, fences its generation,
//! schedules measurements and applies the separately configured reserve. Host
//! availability excludes existing allocations: do not subtract the VM heap a
//! second time. These are observations, not reservations or allocator limits.

use brassclaw_resources::MontyMemorySample;
use thiserror::Error;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// A failed measurement must not authorize budget growth. No host paths or
/// process contents are included in public diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HostMemoryProbeError {
    #[error("native memory measurement is unavailable")]
    Unavailable,
    #[error("native memory counters are inconsistent")]
    Inconsistent,
    #[error("containing memory limits cannot be established")]
    ContainingLimitsUnknown,
    #[error("native memory measurement is unsupported on this platform")]
    UnsupportedPlatform,
}

/// Measure additional capacity for an existing supervised worker. Linux reads
/// that PID's cgroup membership and all visible ancestors, not the host's own
/// membership. Restricted/hidden hierarchies and Windows jobs whose complete
/// ancestry is unavailable return an explicit error, never host-only capacity.
/// A controller must treat errors as unavailable samples and retain its finite
/// acknowledged budget. This function does not prove process identity; retain
/// the service generation and reject observations after worker replacement.
pub fn sample_process_memory_capacity(
    worker_pid: u32,
) -> Result<MontyMemorySample, HostMemoryProbeError> {
    if worker_pid == 0 {
        return Err(HostMemoryProbeError::Unavailable);
    }
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    let result = platform::sample(worker_pid);
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let result = Err(HostMemoryProbeError::UnsupportedPlatform);
    result
}
