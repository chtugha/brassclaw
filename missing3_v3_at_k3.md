# missing3_v3_at_k3 — Step 6 completion: legacy path removal and validator_bridge wiring

**Date:** 2026-11-02.
**Status:** Plan only. No code, runtime, seed or activation change results from
creating this file. Implementation belongs to its named owning layer. Existing
approvals, running tasks and current deployed state remain unchanged until the
steps below are completed and accepted.

---

## 1. Problem statement

This document records the original gap that was tracked as "Step 6 completion —
remove `run_q1_validation` legacy path; wire `PgCompositionPort`
`validator_bridge()`" and its current resolution state.

The Step 6 completion had two originally-open items at the time of the
[PKG-C session](docs/implementation-status.md#2026-11-01--pkg-c-validatortaskdriver--drive_admitted-end-to-end-q1-retained-path):

| Item | Original description | Current status |
|------|----------------------|----------------|
| **ITEM-A** | Remove `run_q1_validation` legacy path (`load_validator_program`, `run_validator_python_with_limits`, `ComponentContentFields`, `reviewed_content_fields`, `Q1Error::AmbiguousValidator`, `Q1Error::InvalidValidator`) from `q1_orchestrator.rs` | **CLOSED** — removed in commit `942ff5c69` (PKG-H, 2026-11-01) |
| **ITEM-B** | Wire `PgCompositionPort::validator_bridge()` to trigger Q1 on composition | **CLOSED** — TASK-006/007/008 closed by wave-2 audit (2026-11-02): `compose` is a read-only composition path; Q1 kicks already fire from `create_and_submit` callers; no bridge needed in `PgCompositionPort` |
| **ITEM-C** | Integration tests T3–T5 for `run_q1_validation_retained` requiring live Postgres + Monty | **OPEN** — tests exist and compile; live execution requires `--features integration` with a live environment |

---

## 2. What was done (ITEM-A and ITEM-B)

### ITEM-A — Legacy path removal (PKG-H, commit `942ff5c69`)

The legacy mutable-recipe Q1 path was removed from
[`q1_orchestrator.rs`](crates/brassclaw_reborn_composition/src/q1_orchestrator.rs):

**Removed:**
- `run_q1_validation` function and its entire doc block.
- `load_validator_program` — queried mutable `reborn_recipes` rows directly.
- `run_validator_python_with_limits` async function.
- `ComponentContentFields` struct and `reviewed_content_fields` helper.
- `Q1Error::AmbiguousValidator` and `Q1Error::InvalidValidator` variants.
- Unused `brassclaw_monty_host::utility::UtilityLimits` import.
- Unused `pub(crate) use review::Q1Candidate` re-export in `validation_queue.rs`.
- Legacy test `run_q1_validation_defers_when_no_recipe_seeded` in `validation_queue.rs`.
- Stale comment in `retention_sweep.rs`.

**Retained:**
- `q1_db` helper — still used by the retained path at lines 292, 331, 341.
- [`run_q1_validation_retained`](crates/brassclaw_reborn_composition/src/q1_orchestrator.rs) —
  the sole production Q1 entry point, unchanged.
- Module doc updated to describe only the retained IBS/Recipe path.
- All four Q1 outcome tests (pass, fail, deferred, invalid-class-code) continue
  to pass.

**Validation evidence (PKG-H):**
- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.
- `cargo test -p brassclaw_reborn_composition --features postgres,skills-db -- q1_orchestrator`: 4/4 pass.

### ITEM-B — PgCompositionPort investigation (TASK-006/007/008, wave-2 audit 2026-11-02)

Source inspection confirmed that `PgCompositionPort::compose` is a **read-only
composition path**. Its responsibilities are:

1. Fetch the Recipe row from `reborn_recipes`.
2. Build a `BuildInstruction` via `build_instruction`.
3. Batch-resolve component UUIDs.
4. Return a `ComposedProgram` to Monty.

`compose` never creates or modifies a component, and never inserts into
`reborn_component_review_submissions` or `reborn_validation_queue`. It is called
_after_ a component has already been admitted and selected by the retained
catalogue. Q1 kicks fire at the point of **component creation and submission**,
not at composition time.

The actual Q1 kick path is:
- [`PgPythonCodeStore::create_and_submit`](crates/brassclaw_reborn_composition/src/pg_python_code_store.rs)
  → calls `fire_q1_kick_if_wired` after commit (class 22).
- [`PgRecipeStore::create_and_submit`](crates/brassclaw_reborn_composition/src/pg_recipe_store.rs)
  → calls `fire_q1_kick_if_wired` after commit (class 21).
- [`PgSempaiProposalSink`](crates/brassclaw_reborn_composition/src/sempai_proposal_sink.rs)
  → `with_validator_service` attaches the bridge so all three proposal paths
  (recipe / python_code / intent) trigger Q1 via the retained IBS/Recipe path.

`PgSempaiProposalSink` is wired to `installed_catalogue.validator_bridge()` in
`runtime.rs` at the `#[cfg(all(feature = "postgres", feature = "root-llm-provider", feature = "skills-db"))]`
block (lines 3394–3411). `bind_validator_service` itself is called at lines
3126–3130 of `runtime.rs` in the `#[cfg(not(test))]` block, immediately after
`GlobalMontyDriver::new`.

**Conclusion:** No change needed in `PgCompositionPort`. Adding a `validator_bridge`
field there would be incorrect architecture — it would fire Q1 on every composition
of an already-approved component, not just on new submissions.

---

## 3. Remaining open item (ITEM-C)

### Integration tests T3–T5

Two integration tests exist in
[`crates/brassclaw_reborn_composition/src/runtime/tests/validator_retained_q1_tests.rs`](crates/brassclaw_reborn_composition/src/runtime/tests/validator_retained_q1_tests.rs):

| Test | Description |
|------|-------------|
| **T3** (`run_q1_validation_retained_passes_and_persists_evidence`) | Full end-to-end pass: seeds structural validator drafts, creates a valid class-22 component, submits it for review and to the queue, calls `run_q1_validation_retained` with a live `ValidatorServiceBridge`, asserts `Q1Outcome::Passed`, and verifies that `reborn_component_review_evidence` + `reborn_validator_q1_runs` rows were written by `verify_receipt`. |
| **T5** (`run_q1_validation_retained_fails_for_empty_content`) | End-to-end fail: same setup but with an empty-content component; asserts `Q1Outcome::Failed` with a non-empty error list mentioning `"content"`, and confirms the queue row stays at state 1. |

Both tests compile and are registered in the test module. They require:

- A live Postgres instance (the `pg_rig()` fixture).
- A live Monty worker binary (`brassclaw_monty_host::process::installed_worker()`).
- The `integration` feature flag on the `brassclaw_reborn_composition` package.

**The tests are not yet run in CI** because the environment requires the full
`skills-db + integration` feature combination with a running Monty service.

---

## 4. Required work for ITEM-C

### Step 1 — Run T3 and T5 in a live integration environment

```bash
cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db,integration \
  run_q1_validation_retained
```

Expected: both `run_q1_validation_retained_passes_and_persists_evidence` and
`run_q1_validation_retained_fails_for_empty_content` pass.

**Prerequisites:**
- Live Postgres with migrations applied through at least V202.
- `brassclaw_monty_host::process::installed_worker()` returns a valid path.
- Disk space ≥ 15 GB on the NVMe target before compilation.
- The `pg_rig()` fixture connects to the test database successfully.

If either test fails, record the exact failure, root-cause the gap (missing
migration, store precondition, `verify_receipt` precondition, or Monty path
defect) and fix in the owning layer before re-running.

### Step 2 — Confirm T3 evidence rows match the contract

After T3 passes, verify the DB assertions match the contract:

1. `reborn_component_review_evidence` has at least one row with
   `submission_id` matching the test's submission and `succeeded = true` in
   the linked `reborn_validator_q1_runs` row.
2. The validation queue row for the test component is at `state = 2`
   (gate1-passed).

These are already asserted by the test body. A passing run with the assertions
not overridden constitutes acceptance.

### Step 3 — Confirm T5 failure behaviour

After T5 passes, verify:

1. `Q1Outcome::Failed` is returned with a non-empty error list.
2. At least one error mentions `"content"`.
3. The validation queue row remains at `state = 1`.

These are asserted by the T5 body.

### Step 4 — Update `docs/implementation-status.md`

Add a dated entry once T3 and T5 have live execution receipts. The entry must:

- Record the exact `cargo test` invocation and its exit status.
- Confirm that evidence rows were written for T3.
- Confirm that the queue state transitions match the contract.
- Supersede the "Integration tests T3–T5 for `run_q1_validation_retained`
  require live Postgres + Monty" bullet in the remaining-open-items list.
- Note that Step 6 completion is **fully closed** once T3 and T5 pass.

---

## 5. What this plan does NOT change

- `q1_orchestrator.rs` — the legacy path is already removed; no further
  structural change is needed.
- `PgCompositionPort` — no `validator_bridge` field is added. `compose` remains
  a read-only path.
- `runtime.rs` wiring — `bind_validator_service` and `PgSempaiProposalSink`
  wiring are already correct and wired.
- `ValidationQueueStore::fire_q1_kick_if_wired` — already wired in both
  `create_and_submit` callers.
- All four Q1 outcome unit tests in `q1_orchestrator.rs` — already pass.
- PKG-D deferred-path unit tests (B/C/D in
  `validation_queue_q1_retained_tests.rs`) — already pass under `--lib`.
- Validator Steps 7–12 — not in scope here. They require behavioral evidence,
  human Q2, and production activation via the eight class routes.

---

## 6. Constraints inherited from the owning architecture

- `run_q1_validation_retained` is the **sole** production Q1 entry point.
  The legacy path is removed; no resurrection or compatibility alias is permitted.
- Q1 kicks fire at `create_and_submit` time, not at `compose` time. This
  ordering is intentional: composition uses already-admitted, already-approved
  components; submission triggers review of new candidates.
- `ValidatorServiceBridge` is set exactly once via `OnceLock`; a second call
  returns `Err`. Tests that wire their own bridge must use the test-path
  `ValidatorTaskDriver` constructor directly (as T3/T5 already do).
- The `integration` feature must gate any test that requires a live Postgres
  pool or Monty worker. Non-integration builds compile the test bodies but
  do not run them; this is the intended behaviour per the development policy.
- `verify_receipt` (called inside `drive_admitted` via `ReviewAdmissionSource`)
  is the only legitimate writer of `state = 2` on the validation queue.
  No other path may promote a component to gate1-passed.
