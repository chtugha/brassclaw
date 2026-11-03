# missing3_v3_at_k3 — Step 6 completion: legacy path removal and validator_bridge wiring

**Date:** 2026-11-02.
**Status:** Plan only. No code, runtime, seed or activation change results from
creating this file. Implementation belongs to its named owning layer. Existing
approvals, running tasks and current deployed state remain unchanged until the
steps below are completed and accepted.

---

## 1. Problem statement

This document records the original gap tracked as "Step 6 completion —
remove `run_q1_validation` legacy path; wire `PgCompositionPort`
`validator_bridge()`" and its current resolution state.

The gap comprised two originally-open items at the time of the
[PKG-C session](docs/implementation-status.md#2026-11-01--pkg-c-validatortaskdriver--drive_admitted-end-to-end-q1-retained-path):

| Item | Original description | Current status |
|------|----------------------|----------------|
| **ITEM-A** | Remove `run_q1_validation` legacy path (`load_validator_program`, `run_validator_python_with_limits`, `ComponentContentFields`, `reviewed_content_fields`, `Q1Error::AmbiguousValidator`, `Q1Error::InvalidValidator`) from `q1_orchestrator.rs` | **CLOSED** — removed in commit `942ff5c69` (PKG-H, 2026-11-01) |
| **ITEM-B** | Wire `PgCompositionPort` `validator_bridge()` to trigger Q1 on composition | **CLOSED** — TASK-006/007/008 closed by wave-2 audit (2026-11-02): `compose` is a read-only composition path; Q1 kicks already fire from `create_and_submit` callers; no bridge needed in `PgCompositionPort` |
| **ITEM-C** | Live execution of integration tests T2–T5 for `run_q1_validation_retained` | **CLOSED** — all 6 tests (T1–T5b) pass as of 2026-11-03 session-7; see `docs/implementation-status.md` dated entry |

The five tests T1–T5 cover the complete `run_q1_validation_retained` contract.
T1 passes under `--lib` without Monty. T2–T5 require a live `monty_worker`
binary and embedded Postgres (both supplied by the test infrastructure when the
binary is built).

---

## 2. What was done (ITEM-A and ITEM-B)

### ITEM-A — Legacy path removal (PKG-H, commit `942ff5c69`)

The legacy mutable-recipe Q1 path was removed from
[`q1_orchestrator.rs`](crates/brassclaw_reborn_composition/src/q1_orchestrator.rs).

**Removed entirely:**
- `run_q1_validation` — the old entry point that queried mutable `reborn_recipes`
  rows directly and ran Python in a standalone subprocess.
- `load_validator_program` — fetched the validator body by class code from mutable rows.
- `run_validator_python_with_limits` — executed that body through a
  `PackagedExecutableImage` subprocess without the IBS/Recipe path.
- `ComponentContentFields` struct and `reviewed_content_fields` helper — projected
  only name/description/content, discarding schemas, bindings and dependency facts.
- `Q1Error::AmbiguousValidator` and `Q1Error::InvalidValidator` variants — no longer
  needed once the single-recipe retained path is the only entry point.
- The stale `use brassclaw_monty_host::utility::UtilityLimits` top-level import
  that was only used by `run_validator_python_with_limits`.
- Unused `pub(crate) use review::Q1Candidate` re-export in `validation_queue.rs`.
- Legacy unit test `run_q1_validation_defers_when_no_recipe_seeded` in
  `validation_queue.rs`.
- Stale comment in `retention_sweep.rs` that referenced the old sweep loop.

**Retained:**
- `q1_db` helper — used by `run_q1_validation_retained` for three pool-error
  conversions (submission lookup, transaction start, and transaction commit).
- [`run_q1_validation_retained`](crates/brassclaw_reborn_composition/src/q1_orchestrator.rs) —
  the sole production Q1 entry point, unchanged.
- Module doc updated to describe only the retained IBS/Recipe path.
- All four Q1 outcome unit tests continue to pass.

**Validation evidence (PKG-H):**
- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.
- `cargo test -p brassclaw_reborn_composition --features postgres,skills-db -- q1_orchestrator`: 4/4 pass.

### ITEM-B — `PgCompositionPort` investigation (TASK-006/007/008, wave-2 audit 2026-11-02)

Source inspection confirmed that
[`PgCompositionPort::compose`](crates/brassclaw_reborn_composition/src/pg_composition_port.rs)
is a **read-only composition path**. Its sole responsibilities are:

1. Fetch the Recipe row from `reborn_recipes`.
2. Build a `BuildInstruction` via `build_instruction`.
3. Batch-resolve component UUIDs.
4. Return a `ComposedProgram` to Monty.

`compose` never creates or modifies a component and never inserts into
`reborn_component_review_submissions` or `reborn_validation_queue`. It executes
only when a component has already been admitted and selected by the retained
catalogue. Q1 kicks fire at the point of **component creation and submission**,
not at composition time. Adding a `validator_bridge` field to `PgCompositionPort`
would fire Q1 on every composition of an already-approved component — architecturally
incorrect and contrary to the v3 contract.

**The actual Q1 kick path (already wired, no change needed):**

- [`PgPythonCodeStore::create_and_submit`](crates/brassclaw_reborn_composition/src/pg_python_code_store.rs)
  calls [`fire_q1_kick_if_wired`](crates/brassclaw_reborn_composition/src/validation_queue.rs)
  after a successful commit (class 22).
- [`PgRecipeStore::create_and_submit`](crates/brassclaw_reborn_composition/src/pg_recipe_store.rs)
  calls `fire_q1_kick_if_wired` after commit (class 21).
- [`PgSempaiProposalSink`](crates/brassclaw_reborn_composition/src/sempai_proposal_sink.rs)
  uses `with_validator_service` to attach the bridge so all three proposal paths
  (recipe / python_code / intent) trigger Q1.

`PgSempaiProposalSink` is wired to `installed_catalogue.validator_bridge()` in
[`runtime.rs`](crates/brassclaw_reborn_composition/src/runtime.rs) in the
`#[cfg(all(feature = "postgres", feature = "root-llm-provider", feature = "skills-db"))]`
block. `bind_validator_service` is called in the `#[cfg(not(test))]` block
immediately after `GlobalMontyDriver::new`.

The `#[cfg(not(test))]` gate is intentional: in test builds `build_reborn_runtime`
is used as a fixture, not as a production runtime. Tests that need the bridge
construct a `ValidatorServiceBridge` directly from the test-path
`ValidatorTaskDriver`, bypassing the `OnceLock` on `InstalledMontyCatalogue`.
This is the correct pattern; tests must not call `bind_validator_service` because
`OnceLock` permits exactly one successful set and a second call returns `Err`.

---

## 3. Remaining open item (ITEM-C): live execution of T2–T5

### Test inventory and numbering

The five tests covering `run_q1_validation_retained` span two source files.
T1 is a unit test that does not require Monty; T2–T5 all call `build_reborn_runtime`
and require the `monty_worker` binary beside the test executable.

| ID | Test function | File | Requires Monty | Outcome |
|----|---------------|------|----------------|---------|
| T1 | `run_q1_validation_retained_defers_when_no_service_wired` | `validation_queue.rs` inner test module | No (bridge=None) | Already passes under `--lib` |
| T2 | `run_q1_validation_retained_defers_when_no_revision_draft_seeded` | `validation_queue_q1_retained_tests.rs` | Yes | Needs live execution |
| T3 | `run_q1_validation_retained_defers_when_no_submission` | `validation_queue_q1_retained_tests.rs` | Yes | Needs live execution |
| T4 | `run_q1_validation_retained_errors_not_defers_when_no_queue_row` | `validation_queue_q1_retained_tests.rs` | Yes | Needs live execution |
| T5 | `run_q1_validation_retained_passes_and_persists_evidence` | `validator_retained_q1_tests.rs` | Yes | Needs live execution |
| T5b | `run_q1_validation_retained_fails_for_empty_content` | `validator_retained_q1_tests.rs` | Yes | Needs live execution |

T5b is labeled `/// T5: end-to-end fail` in its source comment — the test file
predates the settled numbering. For clarity this document uses T5 for the pass
case and T5b for the fail case.

All T2–T5/T5b tests use `#[cfg(all(feature = "postgres", feature = "skills-db"))]`
and are registered unconditionally in that feature configuration. They self-contain
their Postgres infrastructure via `pg_rig()`, which starts an embedded PostgreSQL
instance, runs all migrations, and isolates each test in its own private database.
No external service is required beyond the `monty_worker` binary.

### Why `monty_worker` is required

`run_q1_validation_retained` calls
[`brassclaw_monty_host::process::installed_worker()`](crates/brassclaw_monty_host/src/process.rs)
to locate the worker binary beside the test executable. The function walks from
`current_exe()` up one level if inside a `deps` subdirectory, then looks for a
file named `monty_worker` (or `monty_worker.exe` on Windows). If the file is
absent the function returns `Err` and `run_q1_validation_retained` defers with
`"worker image not available"` instead of running.

`build_reborn_runtime` (called by all T2–T5 tests) also calls `installed_worker()`
internally through `InstalledMontyCatalogue::boot` → `NativeExecutableImage::capture_with_verification`,
so the binary must exist before the runtime is constructed.

---

## 4. Required work for ITEM-C

### Step 1 — Build the full workspace to produce `monty_worker`

The `monty_worker` binary is produced by a workspace member. Build it alongside
the test binary before running the tests:

```bash
df -h /Users/ollama/brassclaw-target   # confirm ≥ 15 GB free

cargo build -p brassclaw_reborn_composition \
  --features postgres,skills-db \
  --tests
```

This places the test binary and the `monty_worker` binary in the same Cargo
profile directory, satisfying `installed_worker()`. Run `ls` to confirm:

```bash
ls $(cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db --no-run 2>&1 \
  | grep -oE 'target/[^ ]+/brassclaw_reborn_composition[^ ]+' | head -1 | xargs dirname)/ \
  | grep monty_worker
```

### Step 2 — Run T2–T5 and T5b

```bash
cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db \
  run_q1_validation_retained
```

This matches all six test function names via substring. Expected: T1 + T2 + T3 +
T4 + T5 + T5b all pass.

If `installed_worker()` returns `Err`, all T2–T5 calls to `build_reborn_runtime`
will fail at boot with an error about the worker executable. The `monty_worker`
binary must be present before the test binary runs.

If any test fails, record the exact failure, root-cause the gap (draft seeding
precondition, `verify_receipt` contract, `ReviewAdmissionSource` precondition, or
Monty execution path defect) and repair in the owning layer before re-running.

### Step 3 — Verify T5 evidence rows

After T5 (`passes_and_persists_evidence`) passes, its own DB assertions confirm:

1. `reborn_component_review_evidence` has at least one row linked via
   `reborn_validator_q1_runs` to the test's `submission_id` with `succeeded = true`.
2. The validation queue row for the test component is at `STATE_Q1_PASSED`
   (`state = 2`), confirming `gate1_pass_reviewed` was called by the retained path.

These assertions are in the test body. A passing run with the assertions intact
constitutes acceptance.

### Step 4 — Verify T5b failure behaviour

After T5b (`fails_for_empty_content`) passes, its own assertions confirm:

1. `Q1Outcome::Failed` is returned with a non-empty error list.
2. At least one error string contains `"content"`.
3. The validation queue row remains at `STATE_Q1_PENDING` (`state = 1`) —
   `gate1_fail_reviewed` was called, not `gate1_pass_reviewed`.

### Step 5 — Update `docs/implementation-status.md`

Add a dated entry once T2–T5/T5b have live execution receipts. The entry must:

- Record the exact `cargo test` invocation and exit status.
- Confirm that T5 evidence rows were written (submission_id, succeeded=true).
- Confirm queue state transitions match the contract for both T5 and T5b.
- Supersede the "Integration tests T3–T5 for `run_q1_validation_retained`
  require live Postgres + Monty" bullet in the remaining-open-items list.
- Note that Step 6 of `validator_v3.md` is **fully closed** once all T2–T5/T5b pass.

---

## 5. What this plan does NOT change

- `q1_orchestrator.rs` — legacy path already removed; no further structural
  change needed.
- `PgCompositionPort` — no `validator_bridge` field is added. `compose` remains
  a pure read-only path.
- `runtime.rs` wiring — `bind_validator_service` and `PgSempaiProposalSink`
  wiring are already correct.
- `ValidationQueueStore::fire_q1_kick_if_wired` — already wired in both
  `create_and_submit` callers.
- All four Q1 outcome unit tests in `q1_orchestrator.rs` — pass today and must
  continue to pass.
- T1 (`defers_when_no_service_wired`) in `validation_queue.rs` — passes today
  under `--lib`; this plan does not modify it.
- Validator Steps 7–12 — out of scope. They require behavioral evidence,
  human Q2 and production activation via the eight class routes.

---

## 6. Constraints inherited from the owning architecture

- **`run_q1_validation_retained` is the sole production Q1 entry point.**
  The legacy path is gone; no resurrection or compatibility alias is permitted.
  Any future attempt to add a second Q1 entry point must own its evidence
  contract separately; it cannot claim coverage from the retained path's tests.

- **Q1 kicks fire at `create_and_submit` time, not at composition time.**
  This is intentional: `PgCompositionPort::compose` operates on already-admitted,
  already-approved components. Submission triggers review of candidates; composition
  assembles approved programs. These are separate lifecycle stages.

- **`ValidatorServiceBridge` is set once via `OnceLock`; a second call returns `Err`.**
  The `#[cfg(not(test))]` gate on `bind_validator_service` in `runtime.rs` is the
  explicit production-only contract. Tests construct `ValidatorServiceBridge`
  directly via `ValidatorTaskDriver::new(runtime.monty_test_controls.0.clone())` —
  this is the only supported test pattern.

- **`verify_receipt` (via `ReviewAdmissionSource` → `drive_admitted`) is the only
  legitimate writer of `STATE_Q1_PASSED` (`state = 2`) on the validation queue.**
  The `gate1_pass_reviewed` and `gate1_fail_reviewed` methods are `pub(crate)` on
  `ValidationQueueStore`, enforced by the access-control doc at
  `q1_orchestrator.rs` §State-2 write invariant (FIND-P9-01 / FIND-P9-08).

- **The test infrastructure is self-contained.** `pg_rig()` starts a private
  embedded PostgreSQL instance per test, runs all migrations, and isolates data.
  No external Postgres service, no Docker container, no `integration` feature flag
  — only the `monty_worker` binary is needed from outside the test process.
