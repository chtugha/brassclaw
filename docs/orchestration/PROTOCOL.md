# Orchestration Protocol

## Roles

- **Root/Auth (Coordinator):** Single owner of all Cargo invocations and the
  NVMe target (`/Users/ollama/brassclaw-target`). Reads INBOX.md, writes OUTBOX.md,
  owns TASKS.md.  
- **Sub-task (Worker):** Polls OUTBOX.md every 10 s. Implements code only.
  Writes BUILD_REQUEST to INBOX.md when ready. Writes DONE when complete.

## Message formats

### ASSIGNMENT (coordinator → OUTBOX.md)

```
## TASK-<NNN> — <title>

**Status:** ASSIGNED  
**Package:** <PKG-id>  
**Assigned:** <ISO datetime>

### Goal
<one paragraph>

### Exact steps
1. …

### Files to touch
- `path/to/file.rs`

### Acceptance
- cargo fmt --all -- --check clean
- cargo clippy -p <crate> --features <f> -- -D warnings: 0 warnings
- <specific test filter>: all pass

### Dependencies
- Blocked by: TASK-NNN (or "none")
```

### BUILD_REQUEST (worker → INBOX.md)

```
## BUILD_REQUEST — TASK-<NNN>

**From:** TASK-<NNN>  
**Time:** <ISO datetime>  
**Crate:** <crate_name>  
**Features:** <features>  
**Test filter:** <filter or "none">  
**Notes:** <any issues or special instructions>
```

### DONE (worker → INBOX.md)

```
## DONE — TASK-<NNN>

**From:** TASK-<NNN>  
**Time:** <ISO datetime>  
**Commits:** <hash list>  
**Result:** PASS | FAIL  
**Notes:** <findings>
```

### BUILD_RESULT (coordinator → OUTBOX.md after compile)

```
## BUILD_RESULT — TASK-<NNN>

**For:** TASK-<NNN>  
**Time:** <ISO datetime>  
**Screen:** build-<slug>  
**Result:** PASS | FAIL  
**Warnings:** <count>  
**Notes:** <excerpt if failed>
```

## Disk space rule

Coordinator checks `df -h /Users/ollama/brassclaw-target` before every `cargo`
invocation. Abort if available < 15 GiB. Workers do not run cargo.

## Cargo lock rule

All cargo invocations are serial under `/private/tmp/brassclaw-target-cargo.lock`.
`flock /private/tmp/brassclaw-target-cargo.lock cargo …`

## Screen convention

`screen -dmS build-<slug> bash -c '…'`  
Coordinator checks logs with `screen -S build-<slug> -X hardcopy /tmp/screen-<slug>.txt`.
