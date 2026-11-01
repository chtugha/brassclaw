# Q1 Validation Retained — Implementation Plan

> **Revision 5 — 2026-11-01 (post-implementation audit)**
>
> This revision reflects the actual state of all source files after external
> edits. The core architecture is now implemented. The remaining work is
> three targeted fixes plus the integration test.
>
> All instructions are grounded in direct source inspection of the modified files.

---

## Current implementation status

The following have been implemented correctly and need no further changes:

| Component | File | Status |
|-----------|------|--------|
| `AdmissionMode::ValidatorInternal` | `bootstrap_validation.rs:176` | ✅ Correct — sets `trusted_internal_turn`, uses `TRUSTED_INTERNAL_SOURCE_BINDING` |
| `admit_validator_handoff` | `bootstrap_validation.rs:139` | ✅ Correct — returns `Admitted` with trusted host |
| `ValidatorTaskDriver` | `validator_task_driver.rs` | ✅ Correct — calls `drive_admitted(admitted.host)` |
| `GlobalMontyDriver::drive_admitted` | `global_monty_driver.rs:575` | ✅ Correct — `build` mutates `input.entry` to `InternalRecipe` in the `Internal` arm |
| `ValidatorServiceBridge` | `installed_monty_catalogue.rs:2153` | ⚠️ Visibility bug (see Fix 1) |
| `bind_validator_service` | `installed_monty_catalogue.rs:201` | ✅ Correct |
| `validator_bridge` | `installed_monty_catalogue.rs:215` | ✅ Correct |
| `fire_q1_kick_if_wired` | `validation_queue.rs:219` | ✅ Correct — uses `bridge.as_ref()` |
| `run_q1_validation_retained` | `q1_orchestrator.rs:487` | ✅ Correct — uses `bridge.driver.submit(...)` |
| `runtime.rs` wiring | `runtime.rs:3127–3130` | ⚠️ Misleading `_pool_for_validator` name (see Fix 2) |
| `AdmissionMode::ValidatorInternal` dead_code comment | `bootstrap_validation.rs:175` | ⚠️ Stale comment (see Fix 3) |

---

## Remaining issues

### Fix 1 — `ValidatorServiceBridge` visibility: `pub` should be `pub(crate)`

**File:** `crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs`
**Line:** 2153

**Current:**
```rust
pub struct ValidatorServiceBridge {
    pub(crate) driver: std::sync::Arc<crate::validator_task_driver::ValidatorTaskDriver>,
    pub(crate) pool: std::sync::Arc<brassclaw_pg::PgPool>,
}
```

**Problem:** The struct is `pub` but all its fields are `pub(crate)`. Every other
bridge/internal struct in this file uses `pub(crate)`. Making `ValidatorServiceBridge`
`pub` leaks it to downstream consumers of the crate when it should be strictly
crate-internal. `ValidationQueueStore` holds `Option<Arc<ValidatorServiceBridge>>`
which is a `#[cfg(feature = "skills-db")]` field — all its call sites are within
the crate.

**Fix:** Change `pub struct` to `pub(crate) struct`.

```rust
pub(crate) struct ValidatorServiceBridge {
    pub(crate) driver: std::sync::Arc<crate::validator_task_driver::ValidatorTaskDriver>,
    pub(crate) pool: std::sync::Arc<brassclaw_pg::PgPool>,
}
```

---

### Fix 2 — `_pool_for_validator` naming in `runtime.rs`

**File:** `crates/brassclaw_reborn_composition/src/runtime.rs`
**Lines:** 3095, 3130

**Current:**
```rust
let _pool_for_validator = Arc::clone(&pool);
// ...
let _ = catalogue.bind_validator_service(validator_driver, _pool_for_validator);
```

**Problem:** The `_` prefix conventionally signals "intentionally unused" to both
the compiler and readers. This variable is actually used at line 3130. The leading
underscore was added to suppress unused warnings while `bind_validator_service` was
not yet wired, but now that it is used, the prefix is misleading.

**Fix:** Rename to `pool_for_validator`.

```rust
let pool_for_validator = Arc::clone(&pool);
// ... (other code unchanged)
let _ = catalogue.bind_validator_service(validator_driver, pool_for_validator);
```

---

### Fix 3 — Stale `#[allow(dead_code)]` comment on `AdmissionMode::ValidatorInternal`

**File:** `crates/brassclaw_reborn_composition/src/bootstrap_validation.rs`
**Lines:** 175–176

**Current:**
```rust
#[allow(dead_code)] // constructed by submit_validator_review dispatch (PKG-C E2E gate)
ValidatorInternal,
```

**Problem:** `submit_validator_review` is the old path that was replaced.
`ValidatorInternal` is now constructed by `admit_validator_handoff`, which is
called by `ValidatorTaskDriver::submit`. The `#[allow(dead_code)]` itself is
still needed — `admit_validator_handoff` carries `#[allow(dead_code)]` at the
call site because the PKG-C E2E integration test that calls the full production
path does not yet exist. But the comment text must be corrected.

Also confirm whether `admit_validator_handoff` itself needs `#[allow(dead_code)]`
or whether it is already live (called by `ValidatorTaskDriver::submit` which is
called by `run_q1_validation_retained` which is called by
`fire_q1_kick_if_wired`). The call chain exists in production code, so
`admit_validator_handoff` is live. The `#[allow(dead_code)]` on `ValidatorInternal`
is only needed because the variant itself has no direct match arm outside the
already-compiled path — verify with `cargo clippy` whether it actually produces
a warning before deciding to keep or remove `#[allow(dead_code)]`.

**Fix:** Update the comment to accurately describe the current wiring:

```rust
#[allow(dead_code)] // constructed by admit_validator_handoff (ValidatorTaskDriver path)
ValidatorInternal,
```

If `cargo clippy` confirms no dead_code warning (because `AdmissionMode` is
private and the variant is used via `prepare_admission`), remove `#[allow(dead_code)]`
entirely.

---

### Fix 4 — Remove stale `#[allow(dead_code)]` from `WorkflowReview::structural` path items

**File:** `crates/brassclaw_reborn_composition/src/pg_workflow_review.rs`

`WorkflowReview`, `WorkflowReview::structural`, and `persist_workflow_review` are
now live — they are called from `pg_review_admission_source.rs::verify_receipt`
(lines 327–341). The `#[allow(dead_code)]` markers that were protecting them
from compiler warnings no longer apply because the call chain now exists.

**What to check:** The `WorkflowReview` struct itself (line 45) and
`WorkflowReview::structural` (line 55) and `persist_workflow_review` (line 288)
currently have **no** `#[allow(dead_code)]` attributes — the external edits
already removed them. This is correct.

The items that remain with `#[allow(dead_code)]` are the behavior-path items
(`WorkflowStepExpectation` line 28, `ExpectedWorkflowTermination` line 34) which
are not yet called from the structural path — their markers are correct and must
remain.

**No change needed for `pg_workflow_review.rs`** — the external edits correctly
removed the structural-path dead_code markers and left the behavior-path ones.

---

### Fix 5 — Remove stale `#[allow(dead_code)]` from `pg_validator_q1_evidence.rs`

**File:** `crates/brassclaw_reborn_composition/src/pg_validator_q1_evidence.rs`

`persist_validator_q1_evidence` is now called from `verify_receipt` (line 348
of `pg_review_admission_source.rs`). The call chain:
`run_q1_validation_retained` → `ValidatorTaskDriver::submit` →
`drive_admitted` → `build` → `GlobalRecipePorts::for_internal` → on
`InternalCompleted`, `verify_receipt` → `persist_validator_q1_evidence`.

The external edits to `pg_validator_q1_evidence.rs` must be inspected to confirm
whether `#[allow(dead_code)]` was removed from `persist_validator_q1_evidence`
and its supporting types.

**Action:** Run `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db -- -D warnings`
and check for dead_code warnings on:
- `MAX_EVIDENCE_BYTES`
- `ValidatorQ1EvidenceError`
- `PersistedValidatorQ1Evidence`
- `persist_validator_q1_evidence`

If any remain with `#[allow(dead_code)]` but are now live, remove the attributes.
If clippy produces warnings without the attributes, the call chain has a gap that
must be identified.

**Note:** `pg_validator_q1_evidence.rs` does not show `#[allow(dead_code)]` on
`MAX_EVIDENCE_BYTES` or `ValidatorQ1EvidenceError` in the content read above (lines 1–50).
This suggests the attributes were already removed. Verify with clippy.

---

### Fix 6 — `Q1AdmissionError` stale dead_code comment

**File:** `crates/brassclaw_reborn_composition/src/pg_validator_q1_admission.rs`
**Lines:** 52–53

**Current:**
```rust
#[allow(dead_code)] // Coverage and Submission used only by submit_validator_review (retained for testing)
pub(crate) enum Q1AdmissionError {
```

**Problem:** `submit_validator_review` function still exists in this file (it is
not deleted — the external changes kept it but it is now unused by
`run_q1_validation_retained`). `retain_validator_selection` is the function still
used by the production path (called at line 729 of `q1_orchestrator.rs`).

`Q1AdmissionError` variants `Coverage` and `Submission` exist only for
`submit_validator_review`'s old API; `retain_validator_selection` uses
`Q1AdmissionError::SelectionBytes`, `Retention`, and `Pool`.

**Action:** Verify whether `submit_validator_review` should be kept or removed:
- If it is still used (e.g. by tests), keep it with an accurate comment.
- If it is dead code, remove both the function and the `Submission`/`Coverage`
  variants of `Q1AdmissionError`, then remove `#[allow(dead_code)]`.

Read the current state of `pg_validator_q1_admission.rs` in full before deciding.

---

## Architectural verification — `drive_admitted` TaskEntry flow

**Critical check:** `drive_admitted` constructs `TaskInput` with
`entry: TaskEntry::Chat` (line 604). `internal_entry` (line 645) is evaluated
**after** `build` returns the modified `input`. In the `Internal` arm of `build`
(line 664 of `global_task_factory.rs`):

```rust
input.entry = brassclaw_monty_host::service::TaskEntry::InternalRecipe {
    name: source.name().to_owned(),
};
```

The `PreparedGlobalTask { input, ports }` returned by `build` carries the
corrected `input.entry`. The check at line 645:

```rust
let internal_entry = matches!(
    &input.entry,
    brassclaw_monty_host::service::TaskEntry::InternalRecipe { .. }
);
```

evaluates to `true` for the validator path. This is correct — `build` canonically
owns the `entry` field assignment for internal tasks. **No change needed.**

---

## Integration test (still required)

**File:** New `crates/brassclaw_reborn_composition/src/runtime/tests/validator_retained_q1_tests.rs`

### Why this is needed

The entire `run_q1_validation_retained` production call chain through
`ValidatorTaskDriver::submit` → `drive_admitted` → `capture_internal` →
`ReviewAdmissionSource` → `verify_receipt` → `persist_validator_q1_evidence` has
not been exercised by any test. All the `#[allow(dead_code)]` markers at the
evidence-persistence end remain because no test calls through the full path.

### Constraints

- No stubs, no mocked outcomes.
- Real `InstalledMontyCatalogue`, real `GlobalMontyOwner`, real PostgreSQL.
- Asserts a real row in `reborn_validator_q1_evidence` after the call.
- Asserts a real row in `reborn_component_workflow_reviews` after the call.

### Test wiring for `bind_validator_service` in test builds

`runtime.rs:3124` wraps the `bind_validator_service` call in `#[cfg(not(test))]`.
In test builds, the bridge is never wired automatically.

**Resolution:** Add a `#[cfg(test)]` counterpart to `bind_validator_service`:

```rust
// In InstalledMontyCatalogue impl, gated on #[cfg(test)]
#[cfg(test)]
pub(crate) fn bind_validator_service_for_test(
    &self,
    driver: std::sync::Arc<crate::validator_task_driver::ValidatorTaskDriver>,
    pool: std::sync::Arc<brassclaw_pg::PgPool>,
) -> Result<(), AgentLoopDriverError> {
    self.bind_validator_service(driver, pool)
}
```

Or, expose `monty_test_controls` (from `runtime.rs:3122`) to the integration test
and construct the bridge manually:

```rust
let (driver, _ports) = &monty_test_controls;
let validator_driver = Arc::new(ValidatorTaskDriver::new(driver.clone()));
catalogue.bind_validator_service_for_test(validator_driver, Arc::clone(&pool)).unwrap();
```

Then call `run_q1_validation_retained` directly with
`catalogue.validator_bridge().as_deref()`.

### Test cases

**T1 — bridge absent → Deferred:**
Call `run_q1_validation_retained(..., bridge: None)`.
Assert `Ok(Q1Outcome::Deferred("retained path not yet wired"))`.
No Monty service required; can use a pool-only fixture.

**T2 — no submission row → Deferred:**
Seed validator drafts. Create a component. Do NOT submit to review store.
Call with real bridge.
Assert `Ok(Q1Outcome::Deferred("no active submission for component ..."))`.

**T3 — submission exists but not in queue → NotFound error:**
Seed drafts. Create component. Create review submission.
Do NOT call `ValidationQueueStore::submit` (no queue row).
Call with real bridge.
Assert `Err(Q1Error::Queue(NotFound { ... }))` — this is NOT a Deferred.

**T4 — end-to-end pass:**
1. `build_reborn_runtime` (real DB + real Monty).
2. Seed validator drafts with `retain_validator_check_drafts`.
3. Create minimal class-22 component (non-empty name/description/content).
4. Create review submission via `PgReviewSubmissionStore`.
5. `ValidationQueueStore::submit` to create queue row.
6. Wire bridge in test (via `bind_validator_service_for_test`).
7. Call `run_q1_validation_retained` directly.
8. Assert `Ok(Q1Outcome::Passed)`.
9. Query `reborn_validator_q1_evidence` — assert row exists with the submission_id.
10. Query `reborn_component_workflow_reviews` — assert row exists.

**T5 — end-to-end fail (malformed component):**
Create a component with empty `content` (or content that the structural validator
rejects). Assert `Ok(Q1Outcome::Failed { errors: [...] })` and confirm the queue
row is at state 3 (Q1_failed).

---

## Execution order for remaining work

1. **Fix 1** — change `pub struct ValidatorServiceBridge` to `pub(crate) struct`. (1 line)
2. **Fix 2** — rename `_pool_for_validator` to `pool_for_validator`. (2 lines)
3. **Fix 3** — update `#[allow(dead_code)]` comment on `ValidatorInternal`; verify
   whether to keep or remove after running clippy.
4. **Fix 6** — read `pg_validator_q1_admission.rs` in full; decide whether to
   remove `submit_validator_review` and update `Q1AdmissionError`.
5. **Fix 5** — run clippy, check `pg_validator_q1_evidence.rs` for stale markers.
6. **Integration test** — add `validator_retained_q1_tests.rs` and
   `bind_validator_service_for_test` to `InstalledMontyCatalogue`.
7. **Lint and validate** — see commands below.

---

## Lint and validation commands

```bash
# Mandatory disk space check (AGENTS.md)
df -h /Users/ollama/brassclaw-target

# Format
cargo fmt --all -- --check

# Lint — fix all newly introduced warnings
cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings

# Unit tests
cargo test -p brassclaw_reborn_composition bootstrap_validation
cargo test -p brassclaw_reborn_composition validator_task_driver
cargo test -p brassclaw_reborn_composition q1_orchestrator

# Integration test (requires postgres + skills-db + a live Monty worker)
cargo test -p brassclaw_reborn_composition --features postgres,skills-db,integration validator_retained_q1
```

---

## Contract invariants (must hold throughout)

- `Q1Outcome::Deferred` is the only valid return when any precondition is absent.
- `Q1Outcome::Passed` requires a confirmed `persist_validator_q1_evidence` write
  inside `verify_receipt`. `LoopExit::Completed(ResultOnly)` from `drive_admitted`
  means `verify_receipt` ran and returned `Ok(())` — which means the write succeeded.
  `verify_receipt` maps every persistence error to a `PortFailure`, which causes
  the service to emit `TaskOutcome::Failed` → `LoopExit::Cancelled` or
  `AgentLoopDriverError::Failed` — neither of which reaches the `Passed` branch.
- `Err(Q1Error::Queue(NotFound))` is the correct return when a submission exists
  in `reborn_component_review_submissions` but no queue row exists. This is a
  caller error, not a deferred state.
- `bootstrap_validation::settle_closed` must NOT be called after `drive_admitted`
  returns. The `OwnedGlobalTaskFactory::settle` path owns the lifecycle.
- `ValidatorDeferredPorts` and `make_validator_ports_factory` are deleted and must
  not be reintroduced.
- `AdmissionMode::ValidatorInternal` must only be used for trusted internal
  validator tasks. It must not be exposed or reused for arbitrary trusted-internal
  admission outside the validator path.
