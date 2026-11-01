# Step 6 simplify_V3 — Corrected implementation instructions

> **Scope:** This plan corrects the architecture misunderstandings identified in
> the audited task summary and provides exact, v3-conformant instructions for
> the open work that summary claimed or implied. It does not reopen completed
> work (Step 4 body fix, K1–K2 gates) and does not introduce new owners.
> Binding ground truth: [recipe.md](../../recipe.md), [skills.md](../../skills.md),
> [tools.md](../../tools.md), [toolskills.md](../../toolskills.md),
> [simplified_v3.md](../../simplified_v3.md), [validator_v3.md](../../validator_v3.md),
> [monty.composition.md](monty.composition.md) and the
> [implementation status index](../implementation-status.md).

---

## 1. What a Recipe actually is and does — the mandatory mental model

Before any implementation work begins, read and internalise this. Every error
in the audited summary traces back to not holding this model correctly.

**A Recipe (class 21) is a component-library entry that tells IBS what to
assemble and the orchestrator in which order to execute it.**

It is NOT:
- A Rust function or Rust struct that "runs" something.
- A bespoke per-task Python program authored at implementation time.
- Something that "validates" by calling a special Rust runner.
- Activated by wiring a Rust constant. `ON CONFLICT DO NOTHING` does not make
  a seed active.

It IS:
- A persisted JSON document (`step_descriptions`, `variants`, `input_layouts`,
  `invocations`) stored in `reborn_recipes`.
- Read by IBS (`build_instruction`) at **task-start** to produce an ephemeral
  `BuildInstruction` referencing pinned component UUIDs.
- Resolved by composition into executable Python steps (from class-22 PythonCode
  components) and Tool bindings (from class-13 ToolSkills). Neither are authored
  at runtime.
- Monty executes those composed steps by invoking preloaded exports with typed
  `inputs["name"]` data. Rust never "runs" the validator Python.

**A class-22 PythonCode component** is the executable building block.
Its body declares `def export_name(inputs): ... return result`.
It is preloaded once (definitions only, no effects), then invoked per step with
typed data. Monty captures the return value as the step result.

**`run_validator_python_with_limits()` is legacy.** It is the old single-step
Rust body extractor path that the audited summary treated as the execution path.
Step 6's job is to *replace* it with the retained IBS/Recipe path — not patch it.

---

## 2. The three open implementation items from the audited summary

### Item A — `pg_review_admission_source.rs::verify_receipt` is wrong

**Current state (uncommitted, broken):** `verify_receipt` parses the receipt as
`{"format": "...", "results": {"step_id": value, …}}` and calls
`coverage.observe_subject(…, &results, …)` — but it does so **without first
verifying the aggregate-step fingerprint**, and without calling
`WorkflowReview::structural` or `persist_workflow_review`. The function also
tries to round-trip the `TaskHandle` through a `to_string()`/`parse_str()` chain
when it would be cleaner to hold the `TaskHandle` value directly.

**The receipt envelope is architecturally correct.** The structural validator
Recipe must post all per-step return values in the receipt so that
`observe_subject` can verify every input-fingerprint edge across steps. The
envelope `{"format": "validator-structural-receipt/1", "results": {"step_id":
value, …}}` and the helpers `make_validator_receipt` / `parse_validator_receipt`
are needed and must be kept.

**What actually needs to be fixed in `verify_receipt`:**

1. **Missing fingerprint check.** Before calling `observe_subject`, the
   aggregate-step observation fingerprint must be verified against the parsed
   receipt value. The pattern from `prefix_bootstrap_setup.rs::verify_execution`
   (line 118–122) is exact:
   ```rust
   proof.result_checksum()
       != Some(typed_value_checksum_with_limits(
           &json!(receipt),   // receipt is &str — wraps as Value::String
           proof.data_limits(),
       )?)
   ```
   Here `receipt` is the raw receipt string and `proof` is the aggregate-step
   observation. This check must run before `observe_subject`.

2. **Missing `WorkflowReview::structural` + `persist_workflow_review` call.**
   After the fingerprint check and before returning `Ok(())`, the structural
   evidence must be written via the existing path:
   ```rust
   let review = WorkflowReview::structural(Arc::clone(&self.inspected))
       .map_err(|_| failure("validator_receipt_structural_review_failed"))?;
   persist_workflow_review(&self.pool, Arc::new(review))
       .await
       .map_err(|_| failure("validator_receipt_structural_review_persist_failed"))?;
   ```

3. **`TaskHandle` round-trip.** The existing code does
   `Uuid::parse_str(&execution.task_handle().to_string())`. `TaskHandle(Uuid)`
   exposes only `Display`, so this round-trip is the only available path. It is
   correct but brittle. Retain it and add a comment explaining the constraint:
   ```rust
   // TaskHandle exposes only Display; parse back to Uuid for the evidence writer.
   let task_handle = Uuid::parse_str(&execution.task_handle().to_string())
       .map_err(|_| failure("validator_receipt_task_handle_invalid"))?;
   ```

4. **`observe_subject` signature mismatch.** The current call passes
   `&self.selection` as `saved`, but `observe_subject` expects
   `&RetainedValidatorSelection` (not `Arc<RetainedValidatorSelection>`).
   Dereference: `&*self.selection`.

**Corrected `verify_receipt` body — exact sequence:**

```rust
async fn verify_receipt(
    &self,
    receipt: &str,
    selected: &InspectedRetainedProgram,
    execution: &RetainedRecipeExecution,
) -> Result<(), PortFailure> {
    use brassclaw_engine::executor::retained_recipe::typed_value_checksum_with_limits;
    use serde_json::json;

    // 1. Identity guard: only the exact programme admitted for this review.
    if !std::ptr::eq(selected, self.inspected.as_ref()) {
        return Err(failure("validator_receipt_programme_mismatch"));
    }

    // 2. Parse the receipt envelope. The structural validator Recipe posts
    //    {"format":"validator-structural-receipt/1","results":{step_id:value,…}}.
    let receipt_value: serde_json::Value = serde_json::from_str(receipt)
        .map_err(|_| failure("validator_receipt_parse_failed"))?;
    if receipt_value.get("format").and_then(|v| v.as_str())
        != Some("validator-structural-receipt/1")
    {
        return Err(failure("validator_receipt_format_invalid"));
    }
    let results_obj = receipt_value
        .get("results")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| failure("validator_receipt_results_missing"))?;
    let results: std::collections::BTreeMap<String, serde_json::Value> =
        results_obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    // 3. Verify the aggregate-step fingerprint matches the raw receipt string.
    //    The receipt string is the complete string-typed value posted by Python;
    //    typed_value_checksum_with_limits wraps it as Value::String before hashing.
    let coverage = RetainedValidatorCoverage::from_inputs(self.inspected.program().inputs())
        .map_err(|_| failure("validator_receipt_coverage_invalid"))?;
    let aggregate_step = coverage.aggregate_step();
    let proof = execution
        .observations()
        .get(aggregate_step)
        .ok_or_else(|| failure("validator_receipt_aggregate_observation_missing"))?;
    let expected_checksum =
        typed_value_checksum_with_limits(&json!(receipt), proof.data_limits())
            .map_err(|_| failure("validator_receipt_checksum_overflow"))?;
    if proof.result_checksum() != Some(expected_checksum) {
        return Err(failure("validator_receipt_fingerprint_mismatch"));
    }

    // 4. Verify subject identity and every input-edge fingerprint, then build
    //    the sealed ObservedSubjectValidatorChecks from the parsed results map.
    let checks = coverage
        .observe_subject(
            &self.subject,
            &*self.selection,
            execution,
            &results,
            self.limits,
        )
        .map_err(|_| failure("validator_receipt_observation_failed"))?;

    // 5. Write structural evidence (fingerprint-only source-inspection record).
    let review =
        crate::pg_workflow_review::WorkflowReview::structural(Arc::clone(&self.inspected))
            .map_err(|_| failure("validator_receipt_structural_review_failed"))?;
    crate::pg_workflow_review::persist_workflow_review(&self.pool, Arc::new(review))
        .await
        .map_err(|_| failure("validator_receipt_structural_review_persist_failed"))?;

    // 6. Write the durable Q1 evidence row.
    //    TaskHandle exposes only Display; parse back to Uuid for the writer.
    let task_handle = Uuid::parse_str(&execution.task_handle().to_string())
        .map_err(|_| failure("validator_receipt_task_handle_invalid"))?;
    crate::pg_validator_q1_evidence::persist_validator_q1_evidence(
        &self.pool,
        &self.subject,
        &*self.selection,
        &checks,
        task_handle,
    )
    .await
    .map(|_| ())
    .map_err(|_| failure("validator_receipt_evidence_persist_failed"))
}
```

Remove the now-redundant `make_validator_receipt`, `parse_validator_receipt` and
`subject_inputs_with_default_limits` free functions from `pg_review_admission_source.rs`
only if they are unused after this fix — inspect call sites first with grep.
The `subject_validator_task_inputs` function is still needed by the `capture_internal`
wiring in Item B; retain it.

**Acceptance gate for Item A:**
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db -- -D warnings`
  passes with zero new warnings.
- `verify_receipt` returns `Err` when the aggregate-step fingerprint does not
  match `typed_value_checksum_with_limits(&json!(receipt), …)`.
- `verify_receipt` returns `Err` when `persist_workflow_review` or
  `persist_validator_q1_evidence` fails, without fabricating a success value.
- `verify_receipt` returns `Err` when `observe_subject` detects an input-edge
  fingerprint mismatch (sealed subject dataflow violation).
- No evidence row is written if any earlier guard fails.

---

### Item B — `capture_internal` routing for the validator prefix

**What v3 architecture requires:**

The prefix-based `capture_internal` pattern already exists for prefix jobs
(see `installed_monty_catalogue.rs` lines 1103–1230). The validator routing
reuses exactly that pattern. It is **admission routing**, not Recipe selection.
The Recipe is selected by `ReviewAdmissionSource::select()` — already correctly
implemented — returning the pre-built `SelectedMontyRecipe` containing the pinned
`InspectedRetainedProgram`.

**The wiring required in `InstalledMontyCatalogue::capture_internal`:**

```rust
if input.user_input().starts_with(
    crate::pg_review_admission_source::VALIDATOR_REVIEW_PREFIX,
) {
    // 1. Strip the known-length prefix and parse the submission UUID.
    //    VALIDATOR_REVIEW_PREFIX is a &'static str constant; its length is
    //    known at compile time. Reject malformed suffixes before any DB read.
    let suffix = input
        .user_input()
        .strip_prefix(crate::pg_review_admission_source::VALIDATOR_REVIEW_PREFIX)
        .ok_or_else(|| /* appropriate PortFailure */ ...)?;
    let submission_id = Uuid::parse_str(suffix)
        .map_err(|_| /* appropriate PortFailure */ ...)?;

    // 2. Read the sealed subject from durable storage.
    //    Use the existing RetainedValidatorSubject reader — do not accept
    //    submission_id from request data without this sealed read.
    let subject = /* read via the same pattern used in bootstrap_setup_approval */;

    // 3. Read the sealed validator selection.
    let selection =
        RetainedValidatorSelection::read_preparation_in_transaction(pool, &subject)
            .await
            .map_err(|_| ...)?;

    // 4. Restore the retained programme from the sealed selection.
    let program = RetainedValidatorCoverage::restore_preparation(&selection)
        .map_err(|_| ...)?;
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(Arc::new(program)),
            &worker_image.path(),
            limits,
        )
        .await
        .map_err(|_| ...)?,
    );

    // 5. Derive typed inputs from the sealed subject (no source substitution).
    let coverage = RetainedValidatorCoverage::from_inputs(inspected.program().inputs())
        .map_err(|_| ...)?;
    let inputs = crate::pg_review_admission_source::subject_validator_task_inputs(
        &coverage,
        &subject,
        &inspected,
        limits,
    )
    .map_err(|_| ...)?;

    // 6. Construct the sealed ReviewAdmissionSource.
    let source = Arc::new(
        crate::pg_review_admission_source::ReviewAdmissionSource::new(
            Arc::new(subject),
            Arc::new(selection),
            inspected,
            inputs,
            pool.clone(),
            limits,
        )
        .map_err(|_| ...)?,
    );

    // 7. Delegate to the existing for_internal path — no new runner.
    return GlobalRecipePorts::for_internal(
        host,
        admission,
        source,
        ownership,
        /* observe_behavior = */ false,
        capacity,
    );
}
```

**Key invariants:**
- Rust admits and transports; Monty executes. Step 7 calls
  `GlobalRecipePorts::for_internal` — this is not a new recipe runner.
- The `submission_id` comes from stripping the sealed `VALIDATOR_REVIEW_PREFIX`
  constant. It is not taken from request JSON or a caller-supplied field.
- The `RetainedValidatorSubject` is read from durable storage keyed on the parsed
  UUID, not constructed from the input string.
- `ReviewAdmissionSource::new` enforces subject/selection agreement and the
  zero-Tool-binding invariant at construction; the returned `Err` must be mapped
  to an appropriate `PortFailure` before returning.
- No public intent example, Tier-2 match, `run_validator_python_with_limits` call,
  or `compose_orchestrator` call is permitted in this branch.

**What NOT to do:**
- Do not construct a `BuildInstruction` manually. IBS does that inside
  `GlobalRecipePorts::for_internal`.
- Do not require a user-visible intent example. This is a private trusted
  internal admission path with no public matching entry.
- Do not call `compose_orchestrator` from Rust. The Recipe orchestrator
  (`compose_orchestrator` in `global_recipe_ports.rs`) is called by Monty, not
  by the admission path.

**Acceptance gate for Item B:**
- Compiles with zero warnings under `--features postgres,skills-db`.
- A focused integration test (real PostgreSQL, `--features integration`) verifies
  that a `user_input` matching `VALIDATOR_REVIEW_PREFIX + <valid_uuid>` is routed
  through `ReviewAdmissionSource` and that a `user_input` without the prefix is
  not. The test does not require a fully running Monty VM — it tests the admission
  branch selection only.
- No new Rust workflow loop, no second execution path, no `PortFailure` that
  silently discards a real error as `Ok(())`.

---

### Item C — L001–L101 legacy migrations are NOT text replacements

**What the audited summary said:**

> L001–L101 PythonCode body conversions (`{{vars.slotN}}` → `inputs["name"]`)

This framing is wrong. It implies a find-and-replace exercise.

**What v3 architecture actually requires** (per [monty.composition.md §1–2](monty.composition.md)):

Each of the 101 candidate PythonCode components must be migrated to the
**preloadable function interface**. The before/after pattern is:

```python
# BEFORE — legacy top-level body, source-substitution, no function boundary:
result = host.read_file(path="{{vars.slot0}}")

# AFTER — preload interface, typed inputs, function boundary, no substitution:
def read_file_usage(inputs):
    return host.read_file(path=inputs["path"])
```

The migration for each component requires all four of the following, not just
a body edit:

1. **Author the class-22 PythonCode revision** with:
   - A named `def export_name(inputs):` function using `inputs["name"]` for
     every argument. No top-level `result = …` assignment.
   - A `python-preload/2` declaration specifying `exports` (with `symbol`,
     `parameters: ["inputs"]`, `mapping: true`), `private_functions`, `constants`,
     `imports`, `dependencies` and `default_export`.
   - An `input_contract` with fully declared recursive schemas (`type`, `required`,
     `checks`, and for objects: `fields`, `allow_extra_fields`).
   - A `result_contract` with the same recursive schema precision.
   - Zero `{{vars.` occurrences anywhere in `content`.

2. **Update the consuming Recipe's `input_layouts`** to bind each local input
   name (`inputs["name"]`) to one of:
   - A task-level captured variable from the matched template
     (`{"kind": "task_input", "name": "slot_name"}`), or
   - A typed constant (`{"kind": "constant", "value": …}`), or
   - A named earlier step result
     (`{"kind": "step_result", "step_id": "…", "path": ["field"]}`).
   The Recipe's `input_layouts` owns this mapping; the PythonCode component
   must not contain the source-substitution grammar.

3. **Create the explicit `skill-association/1` record** for any component that
   is a Tool-calling usage (not pure logic). Pure PythonCode components
   (zero `host.<tool>(…)` calls) need no artificial Skill, ToolSkill or
   association record — they are consumed directly as class-22 steps.
   Do not create a zero-Tool Skill to force a pure component through the Skill
   review path.

4. **Stage through supported revision/upgrade contracts.** The existing seeder
   with `ON CONFLICT DO NOTHING` will not update live DB rows. Changes to an
   installed instance create a new immutable revision via `PgComponentRevisionStore`
   and the existing authoring owner — they do not overwrite the constant in
   `builtin_bootstrap.rs` and call `brassclaw repair`. Resolve actual installed
   UUIDs/checksums from the live store before authoring migrations; do not infer
   UUIDs from seed constant names.

**Priority order** (from [monty.composition.md §1](monty.composition.md#remaining-implementation--consolidated-2026-10-22)):

1. Close the identity/consumer ledger first — enumerate actual installed
   UUID/version/checksum from the live `reborn_component_revisions` table, not
   from constant names in Rust source. Each entry in the 101-candidate list needs
   its actual store identity resolved before any migration is authored.
2. Migrate component families in dependency order (helpers before their callers).
   Verify the complete dependency graph is acyclic; cycles are a hard error.
3. Test actual typed inputs/null/overflow/injection data through real adapters,
   not only source parsing.
4. Do not activate a migrated component until its own Q1/behavior evidence and
   human Q2 (authored path) or controlled seed qualification (system_seed path)
   are complete. A migrated body in a seed constant is not an active component.

**Acceptance gate for Item C:**
- Each migrated component has a `python-preload/2` block with a declared export.
- Zero `{{vars.` occurrences in the migrated `content`.
- The consuming Recipe's `input_layouts` maps every local input name to an
  explicit kind/value binding — no implicit positional mapping.
- The migrated component's `input_contract` and `result_contract` declare
  explicit recursive schemas (not `{}` or null).
- The legacy top-level body executor (`run_validator_python_with_limits` or
  `compose_orchestrator`) is NOT invoked for migrated components.
- Clippy passes for all affected crates with no new warnings.

---

## 3. What the summary correctly identified as open (unchanged requirements)

The following items were correctly listed as open in the audited summary and
remain at their existing owning plans. No implementation is prescribed here
beyond what those plans already specify:

| Open item | Owner | Plan reference |
|-----------|-------|---------------|
| Validator Steps 7–12: class routes, behavioral evidence, human Q2, production cutover | validator_v3.md | [validator_v3.md Steps 7–12](../../validator_v3.md#step-7--author-all-eight-class-specific-validator-recipes) |
| Durable ordinary Q1 scheduling and operator handoff | simplified Phase 3a / shared review-runtime contract | [validator_v3.md Step 9](../../validator_v3.md#step-9--wire-durable-q1-work-human-q2-and-approval-writers) |
| K3 formal closure (provider change acknowledgement) | simplified_v3.md K3 | [simplified_v3.md §K3](../../simplified_v3.md#k3--bestehende-provideränderungen-bis-zu-ihren-verbrauchern-quittieren) |
| Prefix Phase C2/C3/D implementation | prefix_v3_upgrade.md | [prefix plan remaining sequence](prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion) |
| R09 parent usages and five subagent Recipes | monty.composition.md | [monty.composition.md remaining §3](monty.composition.md#remaining-implementation--consolidated-2026-10-22) |
| Dynamic occurrences (V35-13/14) | path_to_v3.5.md | V35-13, V35-14 |
| General pure approval (V35-03/04) | validator_v3.md Steps 3/8–10 | [pure-component-review.md](../reborn/pure-component-review.md#exact-remaining-cutover-contract) |

---

## 4. v3.5 awareness — what must not be pre-empted

The following v3.5 work IDs from [`path_to_v3.5.md`](../../path_to_v3.5.md)
are in flight or upcoming. Implementations under this plan must not pre-empt,
duplicate, or conflict with them:

| Work ID | Boundary | Do not implement here |
|---------|----------|----------------------|
| V35-03 | Standalone pure approval schema + trusted writer | Use `WorkflowReview::structural` + `persist_workflow_review` from the existing owner; do not add a new approval writer in `verify_receipt` |
| V35-04 | Activation/restoration readers for standalone pure | Do not add a new activation flag; the existing store and CAS own this |
| V35-13 | Dynamic occurrences / qualified flow | Do not introduce looping or conditional step execution in Recipe JSON; unsupported graphs stay draft |
| V35-14 | Durable per-item occurrences | Do not flatten conditional effects or invent Recipe fields to work around absent qualified flow |
| V35-09–16 | Recipe-owned Sempai semantics | Do not implement a second completed-turn consumer or review workflow in Rust |

When a required control or occurrence profile is not yet supported by the
runtime, record the dependency explicitly and keep the affected validator
workflow as a draft. Do not paper over missing support with bespoke Rust code.

---

## 5. Ordering and commit discipline

1. **Fix `verify_receipt` first** (Item A). This is uncommitted and broken.
   It is a bounded, self-contained fix requiring no cross-crate changes.
   Before committing: `cargo fmt --all -- --check && cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db -- -D warnings`.

2. **Wire `capture_internal`** (Item B) after Item A compiles cleanly. The two
   together form one logical Step-6 unit. Commit them together or as two adjacent
   commits with a clear boundary between the receipt fix and the routing wire.

3. **L001–L101 migrations** (Item C) proceed component-by-component per the
   composition plan's ledger, after the identity/UUID ledger is closed. Each
   component family is its own bounded work package with its own commit and
   evidence record. Do not batch-edit seed constants as a substitute for proper
   revision/upgrade contracts.

4. **Before each compile:** `df -h /Users/ollama/brassclaw-target` — require ≥15 GB
   free. Serial Cargo execution under `/private/tmp/brassclaw-target-cargo.lock`.

5. **Dead-code annotation pattern** for pre-wired items:
   `#[allow(dead_code)] // wired by validator Step 6 capture_internal routing`

---

## 6. What a "completed" Step 6 actually means

Step 6 is complete when:

- A **real multi-step validator Recipe** (not a Rust runner, not a legacy body
  extractor) selects its class-22 PythonCode steps through IBS, executes them
  via Monty with typed `inputs["name"]` data, and posts a receipt of the form
  `{"format":"validator-structural-receipt/1","results":{step_id:value,…}}`.
- `verify_receipt` verifies the aggregate-step fingerprint against
  `typed_value_checksum_with_limits(&json!(receipt), proof.data_limits())`,
  then calls `observe_subject` with the parsed results map, then writes durable
  `WorkflowReview` + Q1 evidence rows — all in that order, with no success
  synthesised on any partial failure.
- The `capture_internal` branch routes `"validator-review:<uuid>"` inputs
  through `ReviewAdmissionSource` to `GlobalRecipePorts::for_internal`.
- The legacy `load_validator_program` body extraction path is no longer in the
  active code path for the structural validator.
- All of the above compile and pass `clippy -D warnings` under
  `--features postgres,skills-db` with zero new warnings.
- The gate from [validator_v3.md Step 6](../../validator_v3.md#step-6--assemble-and-execute-multi-step-validation-recipes):
  *"a real multi-step validator transports typed results across an external
  wait and interleaved task, preserves its snapshot, and cannot replay a
  completed check's external effect after output/evidence persistence failure"*
  is exercised through real retained callers.

Steps 7–12 remain open after Step 6 is closed. Step 6 alone does not activate
any class route, does not constitute Q1 for any component, and does not permit
production cutover. Those gates belong to their named owners in §3 above.
