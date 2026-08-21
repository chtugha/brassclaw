---
paths:
  - "crates/brassclaw_safety/**"
  - "crates/brassclaw_process_sandbox/**"
  - "crates/brassclaw_host_runtime/**"
  - "crates/brassclaw_secrets/**"
  - "crates/brassclaw_engine/**"
  - "crates/brassclaw_turns/**"
  - "crates/brassclaw_reborn_composition/**"
---
# Safety Layer & Sandbox Rules

## Safety Layer

All external data passes through `crates/brassclaw_safety/`:
1. **Sanitizer** (`sanitizer.rs`) — Detects injection patterns, escapes dangerous content
2. **Validator** (`validator.rs`) — Checks length, encoding, forbidden patterns
3. **Policy** (`policy.rs`) — Rules with severity (Critical/High/Medium/Low) and actions (Block/Warn/Review/Sanitize)
4. **Leak Detector** (`leak_detector.rs`) — Scans for secret patterns at two points: tool output before LLM, and LLM responses before user
5. **Prompt Injection Validation** (`prompt_validation.rs`) — Scans inbound content for prompt-injection payloads before engine dispatch

## Shell and Process Environment Scrubbing

Shell commands and process-executor invocations (`crates/brassclaw_host_runtime/src/services/`, `sandbox_process/`) scrub sensitive env vars before executing. The sanitizer detects command injection patterns (chained commands, subshells, path traversal). See `sensitive_paths.rs` and `redaction.rs` for the exact redaction pipeline.

## Sandbox Policies (`crates/brassclaw_runtime_policy/`)

The `RuntimeProfile` enum (not `RebornCompositionProfile` — that is deleted) controls per-invocation capability policy. `BRASSCLAW_RUNTIME_PROFILE` sets it at startup. Valid values: `local_dev`, `local_safe`, `local_yolo`, `hosted_safe`, etc. — see `brassclaw runtime-profile list`.

| Profile | Filesystem | Network |
|---------|-----------|---------|
| `local_safe` | Read-only workspace | Allowlisted domains |
| `local_dev` | Read-write workspace | Allowlisted domains |
| `local_yolo` | Full filesystem | Unrestricted |

## Zero-Exposure Credential Model

Secrets are stored encrypted in `brassclaw_secrets` (Postgres, AES-GCM, master key from `$BRASSCLAW_REBORN_HOME/.secrets-master-key`). Container processes never see raw credential values. The `SecretStore` trait is implemented by `PgSecretStore`; no filesystem-backed secrets store exists in production.

## Every New Ingress Scans Before Storage or LLM

Every new surface that accepts external data — user messages, webhook payloads, memory writes, URL fetches, file ingestion — must run the matching safety scan on the **pre-transform, pre-injection** payload before the data reaches the LLM or the database.

Recurring bug shape: a new code path is added, and the safety scan is skipped, applied post-injection (too late), or applied to the wrong stage.

Rules:

- **Inbound user text** → `prompt_validation.rs` injection scan before engine dispatch.
- **Tool output** → sanitize + leak detector before LLM, wrapped in `<tool_output>` XML.
- **LLM response** → leak detector before user delivery.
- **Memory / workspace writes** → injection scan on the pre-storage value. Never on the transformed/rendered value.
- **URL fetches** → leak-pattern scan on the resolved URL **before** credential injection; not on the post-injection URL.

A newly added ingress handler (HTTP route, webhook receiver, turn submission) that reaches an LLM call or DB write without calling a safety function on the payload is a review-blocker.

## Bounded Resources

User-controlled inputs must not grow unbounded. Apply caps at the boundary:

- **Interners, caches, accumulators** — hard size limit (entries + total bytes), eviction policy documented.
- **File reads in HTTP handlers** — stream; never read entire file into memory at once.
- **Fan-out scans** — position cap + O(n) algorithm required. Tool-specific fuel limits.
- **Tokio task fan-out** — in-flight dedup or bounded semaphore on spawns driven by user input.

## Cache Keys Must Be Complete

A cache whose stored value depends on input X must include a stable representation of X in its key. If `get_or_create(a, b)` inserts using only `a` but `b` affects the stored value, that is a bug.
