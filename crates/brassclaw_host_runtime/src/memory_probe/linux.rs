use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    time::Instant,
};

use brassclaw_resources::{MontyMemoryPressure, MontyMemorySample};

use super::HostMemoryProbeError;

type ProbeResult<T> = Result<T, HostMemoryProbeError>;

pub(super) fn sample(worker_pid: u32) -> ProbeResult<MontyMemorySample> {
    let measured_at = Instant::now();
    let membership_path = format!("/proc/{worker_pid}/cgroup");
    let membership = read(Path::new(&membership_path))?;
    let group = unified_group(&membership)?;
    let mount = unified_mount(&read(Path::new("/proc/self/mountinfo"))?)?;
    verify_full_memory_hierarchy(&mount)?;
    let meminfo = read(Path::new("/proc/meminfo"))?;
    let (additional_capacity_bytes, pressure) = containing_capacity(
        &mount,
        &group,
        available_memory(&meminfo)?,
        parse_pressure(&read(Path::new("/proc/pressure/memory"))?)?,
    )?;
    if read(Path::new(&membership_path))? != membership {
        // Moving between groups during a sample invalidates the limit view.
        return Err(HostMemoryProbeError::Inconsistent);
    }
    Ok(MontyMemorySample {
        additional_capacity_bytes,
        pressure,
        measured_at,
    })
}

fn containing_capacity(
    mount: &Path,
    group: &Path,
    mut additional_capacity_bytes: u64,
    mut pressure: MontyMemoryPressure,
) -> ProbeResult<(u64, MontyMemoryPressure)> {
    let mut directory = mount.join(group.strip_prefix("/").expect("absolute group"));
    // Include every ancestor's current usage. Never add the cgroup's cache to
    // headroom: it is already counted by MemAvailable and may not be reclaimable
    // inside a constrained ancestor. memory.high also constrains safe growth.
    while directory != mount {
        let current = optional_read(&directory.join("memory.current"))?;
        let max = optional_read(&directory.join("memory.max"))?;
        let high = optional_read(&directory.join("memory.high"))?;
        match (current, max, high) {
            (Some(current), Some(max), Some(high)) => {
                let current = integer(current.trim())?;
                for limit in [max, high] {
                    if let Some(limit) = parse_limit(&limit)? {
                        additional_capacity_bytes =
                            additional_capacity_bytes.min(limit.saturating_sub(current));
                        if current >= limit {
                            pressure = MontyMemoryPressure::Critical;
                        }
                    }
                }
                pressure = worst_pressure(
                    pressure,
                    parse_pressure(&read(&directory.join("memory.pressure"))?)?,
                );
            }
            (None, None, None) => {
                // Memory may be disabled in this branch. Its enabled ancestors
                // are still inspected; partial interface sets are invalid.
                read(&directory.join("cgroup.controllers"))?;
            }
            _ => return Err(HostMemoryProbeError::Inconsistent),
        }
        if !directory.pop() || !directory.starts_with(&mount) {
            return Err(HostMemoryProbeError::ContainingLimitsUnknown);
        }
    }
    Ok((additional_capacity_bytes, pressure))
}

fn read(path: &Path) -> ProbeResult<String> {
    fs::read_to_string(path).map_err(|_| HostMemoryProbeError::Unavailable)
}

fn optional_read(path: &Path) -> ProbeResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(HostMemoryProbeError::Unavailable),
    }
}

fn integer(text: &str) -> ProbeResult<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(HostMemoryProbeError::Inconsistent);
    }
    text.parse().map_err(|_| HostMemoryProbeError::Inconsistent)
}

fn available_memory(text: &str) -> ProbeResult<u64> {
    fn field(text: &str, key: &str) -> ProbeResult<u64> {
        let mut matching = text.lines().filter_map(|line| line.strip_prefix(key));
        let mut parts = matching
            .next()
            .ok_or(HostMemoryProbeError::Inconsistent)?
            .split_whitespace();
        let bytes = integer(parts.next().ok_or(HostMemoryProbeError::Inconsistent)?)?
            .checked_mul(1024)
            .ok_or(HostMemoryProbeError::Inconsistent)?;
        if parts.next() != Some("kB") || parts.next().is_some() || matching.next().is_some() {
            return Err(HostMemoryProbeError::Inconsistent);
        }
        Ok(bytes)
    }
    let total = field(text, "MemTotal:")?;
    let available = field(text, "MemAvailable:")?;
    if total == 0 || available > total {
        return Err(HostMemoryProbeError::Inconsistent);
    }
    Ok(available)
}

fn unified_group(text: &str) -> ProbeResult<PathBuf> {
    // A v1 memory hierarchy is a separate containment contract, not proof of
    // unlimited capacity. Do not silently substitute the host's RAM for it.
    for line in text.lines() {
        let mut fields = line.splitn(3, ':');
        fields.next().ok_or(HostMemoryProbeError::Inconsistent)?;
        let controllers = fields.next().ok_or(HostMemoryProbeError::Inconsistent)?;
        fields.next().ok_or(HostMemoryProbeError::Inconsistent)?;
        if controllers.split(',').any(|name| name == "memory") {
            return Err(HostMemoryProbeError::ContainingLimitsUnknown);
        }
    }
    let mut groups = text.lines().filter_map(|line| line.strip_prefix("0::"));
    let group = groups
        .next()
        .ok_or(HostMemoryProbeError::ContainingLimitsUnknown)?;
    if groups.next().is_some() {
        return Err(HostMemoryProbeError::Inconsistent);
    }
    absolute_path(group)
}

fn absolute_path(text: &str) -> ProbeResult<PathBuf> {
    let path = Path::new(text);
    if !path.is_absolute()
        || text.contains('\0')
        || text.split('/').any(|part| part == "." || part == "..")
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(HostMemoryProbeError::ContainingLimitsUnknown);
    }
    Ok(path.to_owned())
}

fn mount_field(text: &str) -> ProbeResult<PathBuf> {
    // mountinfo escapes space, tab, newline and backslash as octal bytes.
    let mut decoded = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' {
            let escape = [bytes.next(), bytes.next(), bytes.next()];
            let value = match escape {
                [Some(b'0'), Some(b'4'), Some(b'0')] => b' ',
                [Some(b'0'), Some(b'1'), Some(b'1')] => b'\t',
                [Some(b'0'), Some(b'1'), Some(b'2')] => b'\n',
                [Some(b'1'), Some(b'3'), Some(b'4')] => b'\\',
                _ => return Err(HostMemoryProbeError::Inconsistent),
            };
            decoded.push(value);
        } else {
            decoded.push(byte);
        }
    }
    // Kernel paths need not be UTF-8; preserve their bytes exactly.
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(decoded));
    if !path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(HostMemoryProbeError::ContainingLimitsUnknown);
    }
    Ok(path)
}

fn unified_mount(text: &str) -> ProbeResult<PathBuf> {
    let mut selected = None;
    for line in text.lines() {
        let (left, right) = line
            .split_once(" - ")
            .ok_or(HostMemoryProbeError::Inconsistent)?;
        if right.split_whitespace().next() != Some("cgroup2") {
            continue;
        }
        let fields: Vec<_> = left.split_whitespace().collect();
        if fields.len() < 6 {
            return Err(HostMemoryProbeError::Inconsistent);
        }
        // Only an unrestricted root mount can expose all ancestors. Multiple
        // full mounts are equivalent; restricted bind mounts are not selected.
        if mount_field(fields[3])? == Path::new("/") && selected.is_none() {
            selected = Some(mount_field(fields[4])?);
        }
    }
    selected.ok_or(HostMemoryProbeError::ContainingLimitsUnknown)
}

fn verify_full_memory_hierarchy(mount: &Path) -> ProbeResult<()> {
    // Unlike a namespaced non-root cgroup, the actual memory hierarchy root has
    // no memory.current/max/high interfaces, but exposes the memory controller
    // to its children. Both facts are required: absent files alone do not prove
    // there are no hidden enabled ancestors.
    let controllers = read(&mount.join("cgroup.controllers"))?;
    if !controllers.split_whitespace().any(|name| name == "memory") {
        return Err(HostMemoryProbeError::ContainingLimitsUnknown);
    }
    for name in ["memory.current", "memory.max", "memory.high"] {
        if optional_read(&mount.join(name))?.is_some() {
            return Err(HostMemoryProbeError::ContainingLimitsUnknown);
        }
    }
    Ok(())
}

fn parse_limit(text: &str) -> ProbeResult<Option<u64>> {
    if text.trim() == "max" {
        Ok(None)
    } else {
        integer(text.trim()).map(Some)
    }
}

fn parse_pressure(text: &str) -> ProbeResult<MontyMemoryPressure> {
    fn avg(text: &str, kind: &str) -> ProbeResult<f64> {
        let mut lines = text
            .lines()
            .filter(|line| line.split_whitespace().next() == Some(kind));
        let mut fields = lines
            .next()
            .ok_or(HostMemoryProbeError::Inconsistent)?
            .split_whitespace()
            .filter_map(|field| field.strip_prefix("avg10="));
        let value: f64 = fields
            .next()
            .ok_or(HostMemoryProbeError::Inconsistent)?
            .parse()
            .map_err(|_| HostMemoryProbeError::Inconsistent)?;
        if fields.next().is_some()
            || lines.next().is_some()
            || !value.is_finite()
            || !(0.0..=100.0).contains(&value)
        {
            return Err(HostMemoryProbeError::Inconsistent);
        }
        Ok(value)
    }
    let some = avg(text, "some")?;
    let full = avg(text, "full")?;
    if full > some {
        return Err(HostMemoryProbeError::Inconsistent);
    }
    // Conservative stall classification, with no invented RAM percentage.
    // The controller's configurable hysteresis governs the adjustment cadence.
    Ok(if full > 0.0 {
        MontyMemoryPressure::Critical
    } else if some > 0.0 {
        MontyMemoryPressure::Elevated
    } else {
        MontyMemoryPressure::Normal
    })
}

fn worst_pressure(a: MontyMemoryPressure, b: MontyMemoryPressure) -> MontyMemoryPressure {
    use MontyMemoryPressure::{Critical, Elevated, Normal};
    match (a, b) {
        (Critical, _) | (_, Critical) => Critical,
        (Elevated, _) | (_, Elevated) => Elevated,
        _ => Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_ambiguous_or_hidden_memory_views() {
        assert!(unified_group("0::/../hidden\n").is_err());
        assert!(unified_group("0::/task\n5:memory:/hidden\n").is_err());
        assert!(unified_group("0::/task\n0::/other\n").is_err());
        assert_eq!(
            unified_group("0::/slice/task\n").unwrap(),
            Path::new("/slice/task")
        );
        assert!(unified_mount("7 2 0:1 /hidden /cg rw - cgroup2 cgroup rw").is_err());
        assert_eq!(
            unified_mount("7 2 0:1 / /cg\\040memory rw - cgroup2 cgroup rw").unwrap(),
            Path::new("/cg memory")
        );
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("cgroup.controllers"), "memory cpu\n").unwrap();
        verify_full_memory_hierarchy(root.path()).unwrap();
        fs::write(root.path().join("memory.current"), "10\n").unwrap();
        assert_eq!(
            verify_full_memory_hierarchy(root.path()),
            Err(HostMemoryProbeError::ContainingLimitsUnknown)
        );
    }

    #[test]
    fn reject_capacity_and_pressure_that_could_authorize_unsafe_growth() {
        assert_eq!(
            available_memory("MemTotal: 8 kB\nMemAvailable: 7 kB\n").unwrap(),
            7168
        );
        for text in [
            "MemTotal: 8 kB\nMemAvailable: 9 kB\n",
            "MemTotal: 8 kB\n",
            "MemTotal: 8 kB\nMemAvailable: 1 kB\nMemAvailable: 2 kB\n",
        ] {
            assert!(available_memory(text).is_err());
        }
        assert!(parse_pressure("some avg10=NaN\nfull avg10=0\n").is_err());
        assert!(parse_pressure("some avg10=0\nfull avg10=1\n").is_err());
        assert_eq!(
            parse_pressure("some avg10=0.02\nfull avg10=0.01\n").unwrap(),
            MontyMemoryPressure::Critical
        );
        assert_eq!(parse_limit("max\n").unwrap(), None);
        assert_eq!(parse_limit("0\n").unwrap(), Some(0));
        assert!(parse_limit("-1").is_err());
    }

    #[test]
    fn tightest_ancestor_and_pressure_constrain_available_capacity() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("slice");
        let child = parent.join("worker");
        fs::create_dir_all(&child).unwrap();
        for (path, current, max, high) in [
            (&parent, "700", "1000", "900"),
            (&child, "10", "2000", "max"),
        ] {
            fs::write(path.join("memory.current"), current).unwrap();
            fs::write(path.join("memory.max"), max).unwrap();
            fs::write(path.join("memory.high"), high).unwrap();
            fs::write(path.join("memory.pressure"), "some avg10=0\nfull avg10=0\n").unwrap();
        }
        assert_eq!(
            containing_capacity(
                root.path(),
                Path::new("/slice/worker"),
                5000,
                MontyMemoryPressure::Normal
            )
            .unwrap(),
            (200, MontyMemoryPressure::Normal)
        );
        fs::write(parent.join("memory.current"), "950").unwrap();
        assert_eq!(
            containing_capacity(
                root.path(),
                Path::new("/slice/worker"),
                5000,
                MontyMemoryPressure::Normal
            )
            .unwrap(),
            (0, MontyMemoryPressure::Critical)
        );
        fs::write(parent.join("memory.current"), "700").unwrap();
        fs::write(
            child.join("memory.pressure"),
            "some avg10=1\nfull avg10=0\n",
        )
        .unwrap();
        assert_eq!(
            containing_capacity(
                root.path(),
                Path::new("/slice/worker"),
                5000,
                MontyMemoryPressure::Normal
            )
            .unwrap(),
            (200, MontyMemoryPressure::Elevated)
        );
    }

    #[test]
    fn native_capacity_or_hidden_containment_is_reported_explicitly() {
        available_memory(&read(Path::new("/proc/meminfo")).unwrap())
            .expect("actual Linux counters");
        parse_pressure(&read(Path::new("/proc/pressure/memory")).unwrap())
            .expect("actual Linux pressure");
        match sample(std::process::id()) {
            Ok(sample) => assert!(sample.measured_at <= Instant::now()),
            Err(HostMemoryProbeError::ContainingLimitsUnknown) => {}
            Err(error) => panic!("actual Linux memory probe failed: {error}"),
        }
    }
}
