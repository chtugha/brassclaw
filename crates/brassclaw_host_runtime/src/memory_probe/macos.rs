use std::{ffi::CStr, time::Instant};

use brassclaw_resources::{MontyMemoryPressure, MontyMemorySample};

use super::HostMemoryProbeError;

pub(super) fn sample(_worker_pid: u32) -> Result<MontyMemorySample, HostMemoryProbeError> {
    let measured_at = Instant::now();
    let total = counter(c"hw.memsize")?;
    let page_size = counter(c"hw.pagesize")?;
    let free = counter(c"vm.page_free_count")?;
    let purgeable = counter(c"vm.page_purgeable_count")?;
    let additional_capacity_bytes = free
        .checked_add(purgeable)
        .and_then(|pages| pages.checked_mul(page_size))
        .ok_or(HostMemoryProbeError::Inconsistent)?;
    if total == 0 || page_size == 0 || additional_capacity_bytes > total {
        return Err(HostMemoryProbeError::Inconsistent);
    }
    // XNU's sysctl exports dispatch levels, not the internal pressure enum.
    // free_count already includes speculative pages; never add them again.
    // Purgeable pages are discardable. Inactive/compressed pages are not
    // assumed available, and querying counters does not force a purge.
    let pressure = pressure(counter(c"kern.memorystatus_vm_pressure_level")?)?;
    Ok(MontyMemorySample {
        additional_capacity_bytes,
        pressure,
        measured_at,
    })
}

fn counter(name: &CStr) -> Result<u64, HostMemoryProbeError> {
    let mut bytes = [0u8; 8];
    let mut len = bytes.len();
    // SAFETY: name is NUL-terminated; the writable buffer has exactly len
    // bytes. Null newp/zero newlen make this a read-only sysctl operation.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return Err(HostMemoryProbeError::Unavailable);
    }
    match len {
        4 => Ok(u32::from_ne_bytes(bytes[..4].try_into().expect("four bytes")) as u64),
        8 => Ok(u64::from_ne_bytes(bytes)),
        _ => Err(HostMemoryProbeError::Inconsistent),
    }
}

fn pressure(value: u64) -> Result<MontyMemoryPressure, HostMemoryProbeError> {
    match value {
        1 => Ok(MontyMemoryPressure::Normal),
        2 => Ok(MontyMemoryPressure::Elevated),
        4 => Ok(MontyMemoryPressure::Critical),
        _ => Err(HostMemoryProbeError::Inconsistent),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_capacity_is_finite_and_fresh() {
        let before = std::time::Instant::now();
        let sample = super::sample(std::process::id()).expect("actual macOS counters");
        assert!(sample.measured_at >= before);
        assert!(sample.measured_at <= std::time::Instant::now());
        assert!(sample.additional_capacity_bytes <= super::counter(c"hw.memsize").unwrap());
    }
}
