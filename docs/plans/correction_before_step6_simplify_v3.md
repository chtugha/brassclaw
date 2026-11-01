# correction_before_step6_simplify_v3.md

> **Scope:** Exact, verified corrections to
> `crates/brassclaw_reborn_composition/src/pg_review_admission_source.rs`
> before the Step 6 work described in [`Step6_simplify_V3.md`](Step6_simplify_V3.md)
> can proceed. All changes are within a single file; no cross-crate edits are
> required. Nothing in this plan activates any class route, constitutes Q1 for
> any component, or permits production cutover — those belong to Steps 7–12.
>
> **Binding ground truth for every decision below (verified by source inspection):**
> - [`crates/brassclaw_engine/src/memory/retained_validator.rs`](../../crates/brassclaw_engine/src/memory/retained_validator.rs) — `RetainedValidatorCoverage`, `observe` signature (line 316)
> - [`crates/brassclaw_engine/src/memory/retained_validator_subject.rs`](../../crates/brassclaw_engine/src/memory/retained_validator_subject.rs) — `observe_subject` signature (line 76); it calls `self.observe(execution, results, limits)` internally (line 142), so both require a full `results` map
> - [`crates/brassclaw_engine/src/memory/retained_instruction.rs`](../../crates/brassclaw_engine/src/memory/retained_instruction.rs) — `RetainedRecipeInstruction::retained_selection()` returns `Result<RetainedRecipeSelection, RetainedSelectionError>`; `RetainedRecipeSelection::checksum()` returns `[u8; 32]`; `RetainedRecipeSelection::exact_bytes()` returns `&str` (lines 72, 131, 134)
> - [`crates/brassclaw_engine/src/executor/retained_recipe.rs`](../../crates/brassclaw_engine/src/executor/retained_recipe.rs) — `RetainedRecipeExecution::task_handle()` (line 539), `observations()` (line 555); `typed_value_checksum_with_limits` (line 328); `RetainedStepObservation::result_checksum() -> Option<[u8; 32]>` (line 308)
> - [`crates/brassclaw_reborn_composition/src/pg_workflow_review.rs`](../../crates/brassclaw_reborn_composition/src/pg_workflow_review.rs) — `WorkflowReview::structural(Arc<InspectedRetainedProgram>) -> Result<Self, StructuralReviewFailure>` (line 57); `persist_workflow_review(&PgPool, Arc<WorkflowReview>) -> Result<(), RetainedReviewPersistenceError<WorkflowReview>>` (line 288)
> - [`crates/brassclaw_reborn_composition/src/pg_retained_q1.rs`](../../crates/brassclaw_reborn_composition/src/pg_retained_q1.rs) — `StructuralReviewFailure` variants: `Preparation(&'static str)`, `Database(tokio_postgres::Error)`, `Connection(deadpool_postgres::PoolError)`, `Integrity`, `Observation` (lines 28–39)
> - [`crates/brassclaw_reborn_composition/src/pg_validator_q1_evidence.rs`](../../crates/brassclaw_reborn_composition/src/pg_validator_q1_evidence.rs) — `persist_validator_q1_evidence(&PgPool, &RetainedValidatorSubject, &RetainedValidatorSelection, &ObservedSubjectValidatorChecks, Uuid) -> Result<PersistedValidatorQ1Evidence, ValidatorQ1EvidenceError>` (line 109)
>
> **Note on the internal contradiction in `Step6_simplify_V3.md`:** §2 Item A
> (lines 113–117) says to keep the `{"format":"validator-structural-receipt/1",…}`
> envelope and call `coverage.observe_subject(…, &results, …)`. §6 (line 365)
> says "without calling `observe_subject`". These two sections contradict each
> other. §2 Item A is the technically correct section: `observe_subject` requires
> all per-step return values (verified at `retained_validator_subject.rs:81`);
> those values are available from the parsed receipt envelope, so the envelope
> is necessary. §6's parenthetical is a leftover from the earlier incorrect version
> of the plan and must be disregarded.

---

## Defects in the current file

Read
[`pg_review_admission_source.rs`](../../crates/brassclaw_reborn_composition/src/pg_review_admission_source.rs).

### D1 — `RetainedValidatorCoverage` is re-derived on every `verify_receipt` call

`verify_receipt` calls `RetainedValidatorCoverage::from_inputs(self.inspected.program().inputs())`
(line 246) on every receipt. Coverage is a deterministic derivation from the
sealed programme that is known at construction time. Failing it at receipt time
means a construction-time programming error is only caught after a task completes,
which is too late.

**Fix:** derive coverage once in `new()` and store it as a struct field.

### D2 — `std::ptr::eq` identity guard is fragile

`verify_receipt` line 224 uses `std::ptr::eq(selected, self.inspected.as_ref())`.
This exploits the fact that `Arc::clone` returns the same heap pointer. It is not
the v3 identity contract. The correct guard compares the checksum of the retained
IBS selection (`RetainedRecipeSelection::checksum()`) from the *executed* programme
against the checksum from the *sealed* programme stored in `self.inspected`. This
is exactly the check `observe_subject` itself performs at line 89–95 of
`retained_validator_subject.rs`.

**Fix:** compare `retained_selection().checksum()` from both sides.

### D3 — `WorkflowReview::structural` persist step is missing

`verify_receipt` persists Q1 evidence directly via `persist_validator_q1_evidence`
without first persisting a `WorkflowReview::structural` record. Per
`Step6_simplify_V3.md §2 Item A`, a `WorkflowReview` record must be written
before the Q1 evidence row. The two are separate durable rows — `WorkflowReview`
records the selected-variant structure of the executed programme; the Q1 evidence
records the subject-bound observation result.

**Fix:** call `WorkflowReview::structural(Arc::clone(&self.inspected))` and
`persist_workflow_review(…)` before calling `observe_subject`.

### D4 — Module doc comment describes the wrong execution sequence

The module-level doc comment on lines 16–27 documents the wrong four-step
sequence. It omits the `WorkflowReview::structural` persist step (D3) and
incorrectly describes `observe_subject` as the first action after parsing.

**Fix:** rewrite the receipt-format section to match the corrected sequence.

### D5 — `self::failure(…)` is not the correct path syntax inside `verify_receipt`

The existing code at line 280 (the module-level `fn failure`) is called as
`failure(…)` throughout the file. Using `self::failure(…)` inside a method body
is non-idiomatic Rust and would fail to compile because `self::` refers to the
current module, not `self` the value — but inside an `impl` block, `self::f()`
does resolve to the module-level `f()`, so it compiles. However, every other
call site in the file uses the bare `failure(…)` form. Use `failure(…)` for
consistency.

---

## Step 1 — Add `RetainedValidatorCoverage` as a stored field

**File:** `pg_review_admission_source.rs`

Add the field to `ReviewAdmissionSource` after `inspected` and before `inputs`:

```rust
/// Retained coverage declaration, derived once from the sealed programme at
/// construction. Storing it here ensures construction-time errors surface
/// before any task execution begins.
coverage: brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage,
```

The `#[allow(dead_code)]` annotation on the struct must cover this new field. It
is already on the struct itself (line 85), so new fields inherit it automatically
in Rust — no additional annotation needed.

**Change `new()` to derive coverage once:**

Replace the first call to `RetainedValidatorCoverage::from_inputs` inside `new()`
(currently line 132 — the one inside the `new` body before the
`RetainedProgram::Tools` guard) with the construction-time derivation. In the
current code, `new()` calls `from_inputs` only as a coverage-contract verification
check and discards the result. Replace that discard with a stored value:

```rust
// Verify the inspected programme satisfies the validator coverage contract
// and retain the result — derive once, not on every verify_receipt call.
let coverage =
    RetainedValidatorCoverage::from_inputs(inspected.program().inputs())
        .map_err(|_| "validator programme does not satisfy coverage declaration")?;
```

Then include `coverage` in the `Ok(Self { … })` constructor:

```rust
Ok(Self {
    reference,
    subject,
    selection,
    inspected,
    coverage,
    inputs,
    pool,
    limits,
})
```

**Import:** `RetainedValidatorCoverage` is already imported via:

```rust
use brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage;
```

Confirm this is present in the file's `use` block. It is required both for the
`from_inputs` call and for the `coverage.observe_subject` call in `verify_receipt`.

---

## Step 2 — Replace the `std::ptr::eq` identity guard with a checksum comparison

**File:** `pg_review_admission_source.rs`, `verify_receipt`, current lines 224–226.

**Remove:**

```rust
if !std::ptr::eq(selected, self.inspected.as_ref()) {
    return Err(failure("validator_receipt_programme_mismatch"));
}
```

**Replace with** (using the verified API: `instruction().retained_selection()` is on
`RetainedRecipeInstruction`, accessed via `.program().inputs().instruction()`):

```rust
// Verify the executed programme matches the one sealed at admission by
// comparing their retained IBS selection checksums. This is the same
// identity check observe_subject performs internally; ptr equality is
// an implementation accident that does not survive refactoring.
let executed_checksum = selected
    .program()
    .inputs()
    .instruction()
    .retained_selection()
    .map_err(|_| failure("validator_receipt_selection_invalid"))?
    .checksum();
let sealed_checksum = self
    .inspected
    .program()
    .inputs()
    .instruction()
    .retained_selection()
    .map_err(|_| failure("validator_receipt_sealed_selection_invalid"))?
    .checksum();
if executed_checksum != sealed_checksum {
    return Err(failure("validator_receipt_programme_mismatch"));
}
```

**No new imports required.** `selected: &InspectedRetainedProgram` is already
in scope as the parameter. `InspectedRetainedProgram` is imported at line 49.
`RetainedSelectionError` is not needed here — the `?` operator on
`map_err(|_| failure(…))` converts it inline.

**Note on cost:** `retained_selection()` serialises the selection JSON to a
`String` and SHA-256 hashes it on every call. This happens twice (once per
side) per `verify_receipt` invocation. That is one receipt-time cost for the
sealed programme — not a hot path, acceptable. Caching the sealed checksum as
a struct field is an optional micro-optimisation outside this plan's scope.

---

## Step 3 — Add `WorkflowReview::structural` + `persist_workflow_review`

**File:** `pg_review_admission_source.rs`

**New imports** — add to the existing `use crate::…` block at the top of the file.
Do not duplicate imports already present:

```rust
use crate::pg_retained_q1::StructuralReviewFailure;
use crate::pg_workflow_review::{WorkflowReview, persist_workflow_review};
```

`Arc` is already imported: line 42 has `use std::{collections::BTreeMap, sync::Arc};`.

**Where to insert in `verify_receipt`:** after the `results` map is constructed
(i.e. after the `let results: BTreeMap<String, Value> = …` expression) and before
the `observe_subject` call.

```rust
// Persist the structural workflow-review record before the subject-bound
// Q1 evidence. WorkflowReview::structural derives its report from the
// sealed inspected programme alone — no receipt values are needed.
// A failed persist leaves evidence incomplete without synthesising a pass.
let structural_review =
    WorkflowReview::structural(Arc::clone(&self.inspected))
        .map_err(|e| match e {
            StructuralReviewFailure::Preparation(reason) => {
                tracing::warn!(reason, "validator structural review preparation failed");
                failure("validator_receipt_structural_review_failed")
            }
            StructuralReviewFailure::Integrity
            | StructuralReviewFailure::Observation => {
                failure("validator_receipt_structural_review_integrity")
            }
            StructuralReviewFailure::Connection(_)
            | StructuralReviewFailure::Database(_) => {
                failure("validator_receipt_structural_review_failed")
            }
        })?;
persist_workflow_review(&self.pool, Arc::new(structural_review))
    .await
    .map_err(|_| failure("validator_receipt_structural_review_persist_failed"))?;
```

**Explanation of the `tracing::warn!` call:**

`StructuralReviewFailure::Preparation` carries a `&'static str` field. The
correct structured-logging invocation for a bare `&'static str` binding named
`reason` uses the field-value syntax:

```rust
tracing::warn!(reason, "validator structural review preparation failed");
```

This emits `reason` as a string field. The `tracing` macro interprets a bare
identifier (not a `%` or `?` prefix) in field position as a local variable being
captured by value. Because `reason: &'static str` implements `tracing::Value`
directly, this compiles and produces the correct structured field. Do **not** use
`tracing::warn!(%reason, …)` — `Display` formatting is correct here too, but the
bare form is idiomatic for `&str`.

**`StructuralReviewFailure` match arm grouping:**

The enum has five variants (confirmed from `pg_retained_q1.rs` lines 28–39):
- `Preparation(&'static str)` — authoring error; log the reason
- `Database(tokio_postgres::Error)` — transient; map to the same opaque failure
- `Connection(deadpool_postgres::PoolError)` — transient; same
- `Integrity` — immutable data mismatch; distinct failure code
- `Observation` — incomplete execution; distinct failure code

The `Observation` variant should not occur from `WorkflowReview::structural` (it is only emitted by `WorkflowReview::behavior`), but must be included in the match to satisfy exhaustiveness. It is correctly grouped with `Integrity` as an internal error, not a transient one.

---

## Step 4 — Use `self.coverage` in `observe_subject`

After Step 1, the local `coverage` variable inside `verify_receipt` (lines 245–248)
is gone. Replace the `coverage.observe_subject(…)` call at lines 251–258 to use
`self.coverage`:

```rust
let checks = self
    .coverage
    .observe_subject(
        &self.subject,
        &self.selection,
        execution,
        &results,
        self.limits,
    )
    .map_err(|_| failure("validator_receipt_observation_failed"))?;
```

No other change to this call is required. The argument order matches
`retained_validator_subject.rs:76–83` exactly:
`observe_subject(&self, subject, saved, execution, results, limits)`.

---

## Step 5 — Remove the dead local `coverage` variable

After Steps 1 and 4, the lines:

```rust
let coverage = RetainedValidatorCoverage::from_inputs(self.inspected.program().inputs())
    .map_err(|_| failure("validator_receipt_coverage_invalid"))?;
```

are dead. Remove them entirely. Removing them will also make the
`RetainedValidatorCoverage` import (if only used there) necessary at the struct
field level — which is covered by Step 1's import check.

---

## Step 6 — Fix the module doc comment

Replace the `# Receipt format` section (lines 16–27) with:

```rust
//! # Receipt format
//!
//! The Python orchestrator for the structural validator must post its internal
//! receipt as a JSON string of the form:
//! ```json
//! {"format": "validator-structural-receipt/1", "results": {"step_id": <value>, …}}
//! ```
//! where each `step_id` key maps to the actual return value produced by that
//! step. `verify_receipt` then performs the following sequence, in order:
//!
//! 1. Verifies that the executed programme matches the one sealed at admission
//!    by comparing their `RetainedRecipeSelection` checksums.
//! 2. Parses the receipt JSON and extracts the per-step `results` map.
//! 3. Persists a `WorkflowReview::structural` record via `persist_workflow_review`,
//!    recording the selected-variant structure of the executed programme.
//! 4. Calls `RetainedValidatorCoverage::observe_subject` with the parsed
//!    per-step results map, the sealed subject, the sealed selection and the
//!    completed execution to produce `ObservedSubjectValidatorChecks`.
//! 5. Calls `persist_validator_q1_evidence` to write the durable Q1 evidence row.
//!
//! Steps 3 and 5 must both succeed before `Ok(())` is returned. A failed persist
//! leaves the task in an incomplete-evidence state; neither Q1 success nor a
//! fabricated pass is synthesised from a persistence error.
```

---

## Complete corrected body of `verify_receipt`

Use this as the authoritative reference when applying the diff. Every token has
been verified against the actual source signatures listed in the preamble.

```rust
/// Persist the sealed Q1 evidence after verifying the receipt.
///
/// The Python orchestrator posts a receipt string of the form:
/// `{"format": "validator-structural-receipt/1", "results": {"step_id": <value>, …}}`
/// This method verifies programme identity, persists structural workflow evidence,
/// produces sealed observations, and persists durable Q1 evidence — in that order.
async fn verify_receipt(
    &self,
    receipt: &str,
    selected: &InspectedRetainedProgram,
    execution: &RetainedRecipeExecution,
) -> Result<(), PortFailure> {
    // Step 2: verify the executed programme matches the admission-time selection
    // by comparing RetainedRecipeSelection checksums — the v3 identity contract.
    let executed_checksum = selected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(|_| failure("validator_receipt_selection_invalid"))?
        .checksum();
    let sealed_checksum = self
        .inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(|_| failure("validator_receipt_sealed_selection_invalid"))?
        .checksum();
    if executed_checksum != sealed_checksum {
        return Err(failure("validator_receipt_programme_mismatch"));
    }

    // Parse the receipt JSON. The orchestrator must post the exact envelope.
    let receipt_value: Value = serde_json::from_str(receipt)
        .map_err(|_| failure("validator_receipt_parse_failed"))?;
    if receipt_value["format"] != "validator-structural-receipt/1" {
        return Err(failure("validator_receipt_format_invalid"));
    }
    let results_value = receipt_value
        .get("results")
        .and_then(Value::as_object)
        .ok_or_else(|| failure("validator_receipt_results_missing"))?;
    let results: BTreeMap<String, Value> = results_value
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // Step 3: persist structural workflow-review evidence before Q1 evidence.
    // WorkflowReview::structural derives its report from the sealed inspected
    // programme — no receipt values are needed. Both persists must succeed;
    // neither a partial nor a fabricated pass is acceptable on failure.
    let structural_review =
        WorkflowReview::structural(Arc::clone(&self.inspected))
            .map_err(|e| match e {
                StructuralReviewFailure::Preparation(reason) => {
                    tracing::warn!(reason, "validator structural review preparation failed");
                    failure("validator_receipt_structural_review_failed")
                }
                StructuralReviewFailure::Integrity
                | StructuralReviewFailure::Observation => {
                    failure("validator_receipt_structural_review_integrity")
                }
                StructuralReviewFailure::Connection(_)
                | StructuralReviewFailure::Database(_) => {
                    failure("validator_receipt_structural_review_failed")
                }
            })?;
    persist_workflow_review(&self.pool, Arc::new(structural_review))
        .await
        .map_err(|_| failure("validator_receipt_structural_review_persist_failed"))?;

    // Step 4: produce sealed subject-bound observations using the stored
    // coverage declaration (derived once at construction in new()).
    let checks = self
        .coverage
        .observe_subject(
            &self.subject,
            &self.selection,
            execution,
            &results,
            self.limits,
        )
        .map_err(|_| failure("validator_receipt_observation_failed"))?;

    // Extract the task handle UUID. TaskHandle wraps Uuid and only exposes
    // Display; the hyphenated form produced by Display always round-trips
    // through Uuid::parse_str without error.
    let task_handle = Uuid::parse_str(&execution.task_handle().to_string())
        .map_err(|_| failure("validator_receipt_task_handle_invalid"))?;

    // Step 5: persist the durable Q1 evidence row.
    crate::pg_validator_q1_evidence::persist_validator_q1_evidence(
        &self.pool,
        &self.subject,
        &self.selection,
        &checks,
        task_handle,
    )
    .await
    .map(|_| ())
    .map_err(|_| failure("validator_receipt_evidence_persist_failed"))
}
```

---

## Complete import additions for `pg_review_admission_source.rs`

The existing `use` block starts at line 42. Add only the items that are not
already present. Verified against the current file content:

```rust
// Already present — confirm, do not re-add:
use std::{collections::BTreeMap, sync::Arc};
use brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage;
// (RetainedValidatorCoverage is already imported; used in new() at line 132)

// Add these two — not present in the current file:
use crate::pg_retained_q1::StructuralReviewFailure;
use crate::pg_workflow_review::{WorkflowReview, persist_workflow_review};
```

`tracing` is a workspace dependency (`tracing = "0.1"` in `Cargo.toml` line 137).
No explicit `use tracing;` is needed — the macros are used as `tracing::warn!(…)`
throughout the crate.

---

## Validation before committing

```bash
# 1. Check disk space first — mandatory on this host.
df -h /Users/ollama/brassclaw-target

# 2. Format check — must be clean before any commit.
cargo fmt --all -- --check

# 3. Lint the affected package only under the relevant features.
#    Fix every warning introduced by this change; do not suppress new warnings.
cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db -- -D warnings
```

The existing `#[allow(dead_code)]` annotations on `VALIDATOR_REVIEW_PREFIX`,
`validator_review_reference`, `ReviewAdmissionSource`, and `ReviewAdmissionSource::new`
must remain — they are correctly annotated as pre-wired for the Step 6
`capture_internal` routing. No new `#[allow(dead_code)]` suppressions should be
needed by this correction.

---

## Acceptance criteria

- [ ] `ReviewAdmissionSource` has a `coverage: RetainedValidatorCoverage` field.
- [ ] `ReviewAdmissionSource::new()` derives `coverage` from
  `RetainedValidatorCoverage::from_inputs(inspected.program().inputs())` and
  stores it. The `?` propagation means a malformed programme fails at `new()`.
- [ ] `verify_receipt` contains no call to `RetainedValidatorCoverage::from_inputs`.
- [ ] The identity guard in `verify_receipt` compares `retained_selection().checksum()`
  from `selected` and from `self.inspected`, returning a distinct `PortFailure`
  for each failure mode (`validator_receipt_selection_invalid`,
  `validator_receipt_sealed_selection_invalid`, `validator_receipt_programme_mismatch`).
- [ ] `WorkflowReview::structural(Arc::clone(&self.inspected))` is called and its
  `Ok` result is passed wrapped in `Arc::new(…)` to `persist_workflow_review`
  before `observe_subject` is called.
- [ ] The `StructuralReviewFailure` match arm is exhaustive (all five variants
  covered) and uses `tracing::warn!(reason, …)` for the `Preparation` arm.
- [ ] `self.coverage.observe_subject(…)` is called with `&self.subject`,
  `&self.selection`, `execution`, `&results`, `self.limits` — in that order.
- [ ] `make_validator_receipt` and `parse_validator_receipt` remain in the file
  unchanged.
- [ ] The module doc comment describes the five-step sequence in `verify_receipt`.
- [ ] `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db -- -D warnings`
  passes with zero new warnings.
- [ ] No new Rust workflow loop. No call to `run_validator_python_with_limits`.
  No call to `compose_orchestrator`.
