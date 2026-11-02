# OUTBOX — Coordinator Assignment Queue

_Coordinator writes ASSIGNMENT blocks here. Workers poll every 10 s._

---

## TASK-023 — bootstrap_validator_approval.rs: Step 8 pure-component-approval/1 for validator PCs

**Status:** ASSIGNED (inline coordinator)
**Package:** PKG-VALIDATOR-STEP8
**Assigned:** 2026-11-01T18:30:00Z

### Goal

Implement `bootstrap_validator_approval.rs` to produce `pure-component-approval/1`
records for the eight structural validator PythonCode drafts (one per class route).
This closes `missing4_v3_at_k3.md §Step 8` for the system-seed path and
establishes behavioral evidence rows in `reborn_component_review_evidence`.

### Prerequisites

- TASK-021 / TASK-022: `bootstrap_l013_approval.rs` bugfix + retest (in progress).
- All eight `ComponentRevisionDraft` envelopes already retained by
  `retain_validator_check_drafts` via `structural_validator_recipe_drafts`.

### Plan

1. Create `crates/brassclaw_reborn_composition/src/bootstrap_validator_approval.rs`
   modelled after `qualify_pure_component` in `bootstrap_l013_approval.rs`.
   For each of the eight class codes, call the pattern:
   - Look up `pc_id` via `structural_validator_pc_id(class_code)` (no DB lookup
     needed — UUID is deterministic from `packaged_component_id`).
   - Build `pc_draft = pc_revision(pc_id, pc_body)` using the class-specific body.
   - Retain: `revision_store.retain_packaged_draft(&pc_draft)` → `pc_ref`.
   - Deterministic approval UUIDs (VALIDATOR_QUAL_NAMESPACE, seed=`"validator:class-{N}:{pc_id}"`).
   - Check idempotent guard: skip if `reborn_pure_component_approvals` row exists.
   - Write Q1 + behavioral evidence in one RepeatableRead transaction, then
     call `pg_pure_component_approval::record_pure_component_approval`.
   - Q1 report: `["no-host-calls","effect-free-load","declared-export-matches-body"]`
   - Behavior report: `["no-prohibited-identifiers","no-effectful-top-level","json-loads-used-for-candidate-decode"]`

2. Export `pub(crate) async fn qualify_all_validators(pool, tenant_id)`.

3. Register `mod bootstrap_validator_approval` in `lib.rs` under
   `#[cfg(all(feature = "postgres", feature = "skills-db"))]`.

4. Wire `qualify_all_validators` in `component_boot.rs` immediately after the
   `qualify_all` call, also under `#[cfg(all(feature = "postgres", feature = "skills-db"))]`.

5. Add acceptance test in `component_boot.rs` test module:
   `validator_approval_writes_eight_pure_approvals_at_boot` — after
   `initialize_runtime_components`, count rows in `reborn_pure_component_approvals`
   WHERE `root_id IN (<8 validator pc IDs>)` and assert = 8.
   Also assert idempotent: second boot produces no duplicates (still 8).

6. `cargo fmt`, `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.

### Acceptance

- `SELECT count(*) FROM reborn_pure_component_approvals WHERE root_id IN (<8 uuids>)` = 8 after boot.
- Acceptance test passes in isolation.
- `run_content_integrity_check` does not fail for any validator PC row.

---


## TASK-002 — Wire validator_bridge into PgCompositionPort

**Status:** ASSIGNED  
**Package:** PKG-G  
**Assigned:** 2026-11-01T20:55:00Z

### Goal

`PgCompositionPort` currently has no knowledge of the validator bridge
(`ValidatorServiceBridge`). The `PgCompositionPort::new` constructor takes only
`pool`, `store`, and `basic_prompt_store`. The `ValidationQueueStore` that fires
Q1 kicks needs the bridge wired through the composition port path so that
components submitted via `PgCompositionPort::compose` (the engine-side trait impl)
also trigger Q1.

Concretely: `PgCompositionPort` calls `PgPythonCodeStore::create_and_submit` and
`PgRecipeStore::create_and_submit`, which call `fire_q1_kick_if_wired`. Those
stores currently receive a `ValidationQueueStore` with `validator_bridge: None`
because there is no mechanism to pass the bridge to `PgCompositionPort` at
construction time. This task wires it.

### Exact steps

1. **Read** `pg_composition_port.rs` lines 148–175 (struct + constructor) and the
   `PgCompositionPort::compose` implementation to see exactly where
   `PgPythonCodeStore` and `PgRecipeStore` are instantiated with a
   `ValidationQueueStore`.

2. **Add** an optional `validator_bridge` field to `PgCompositionPort`:
   ```rust
   #[cfg(feature = "skills-db")]
   validator_bridge: Option<Arc<crate::installed_monty_catalogue::ValidatorServiceBridge>>,
   ```

3. **Update** `PgCompositionPort::new` to accept:
   ```rust
   #[cfg(feature = "skills-db")]
   validator_bridge: Option<Arc<crate::installed_monty_catalogue::ValidatorServiceBridge>>,
   ```
   and store it in the struct.

4. **Thread** the bridge into `ValidationQueueStore` wherever `PgCompositionPort`
   constructs one — find all `ValidationQueueStore::new(...)` calls within
   `pg_composition_port.rs` and call `.with_validator_bridge(bridge.clone())` (or
   whatever the existing setter is named — check `validation_queue.rs:205–215`).

5. **Update** all call sites of `PgCompositionPort::new` in `runtime.rs` and any
   tests to pass `None` for the new parameter (preserving existing behavior). Search:
   `grep -n "PgCompositionPort::new" crates/brassclaw_reborn_composition/src/`

6. **Verify** with:
   ```
   cargo fmt --all -- --check
   cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
   cargo test -p brassclaw_reborn_composition --features postgres,skills-db pg_composition_port
   ```

### Files to touch

- `crates/brassclaw_reborn_composition/src/pg_composition_port.rs`
- `crates/brassclaw_reborn_composition/src/runtime.rs` (call sites only)
- Any test files that construct `PgCompositionPort::new` directly

### Acceptance

- `cargo fmt --all -- --check`: clean
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings
- All existing `pg_composition_port` tests still pass

### Dependencies

- Blocked by: none

---

## TASK-003 — Remove run_q1_validation legacy path

**Status:** ASSIGNED  
**Package:** PKG-H  
**Assigned:** 2026-11-01T20:55:00Z

### Goal

`run_q1_validation` in `q1_orchestrator.rs` is the legacy mutable-Recipe Q1 path
(uses `load_validator_program` + mutable `reborn_recipes` rows). The production
path now uses `run_q1_validation_retained` exclusively. `run_q1_validation` has
zero production callers — only tests call it, and those tests exist solely to verify
the legacy path's own behavior. Removing it and its dependencies simplifies the
module and completes Step 6 of `validator_v3.md`.

### Exact steps

1. **Confirm no production callers** before touching anything:
   ```bash
   grep -rn "run_q1_validation\b" crates/ --include="*.rs" | grep -v "#\[cfg(test)\]" | grep -v "^.*//.*run_q1_validation"
   ```
   Expected: only `q1_orchestrator.rs` (definition + module doc) and test code.
   If any production caller is found, STOP and post a BUILD_REQUEST noting the blocker.

2. **Read** `q1_orchestrator.rs` to identify what `run_q1_validation` depends on
   that is NOT also used by `run_q1_validation_retained`. Key suspect: `load_validator_program`.
   Check if `load_validator_program` is used by `run_q1_validation_retained` or anything else.

3. **Remove** `run_q1_validation` function and its doc comment block.

4. **Remove** `load_validator_program` if it has no other callers (check with grep).
   Also check `Q1Error` variants used only by `run_q1_validation` and remove them if orphaned.

5. **Remove** or update tests in `q1_orchestrator.rs` that solely test `run_q1_validation`:
   - The tests in `validation_queue.rs::run_q1_validation_defers_when_no_recipe_seeded` — 
     this test calls the legacy function; delete it or replace with a note that the retained
     path's deferral is covered by `run_q1_validation_retained_defers_when_no_service_wired`.

6. **Update module doc** at the top of `q1_orchestrator.rs` to remove references to the
   legacy path; point to `run_q1_validation_retained` as the active path.

7. **Verify**:
   ```
   cargo fmt --all -- --check
   cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
   cargo test -p brassclaw_reborn_composition --features postgres,skills-db q1_orchestrator
   ```

### Files to touch

- `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`
- `crates/brassclaw_reborn_composition/src/validation_queue.rs` (remove legacy test)
- Possibly `crates/brassclaw_reborn_composition/src/retention_sweep.rs` (comment cleanup)

### Acceptance

- `cargo fmt --all -- --check`: clean
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings
- `cargo test -p brassclaw_reborn_composition --features postgres,skills-db q1_orchestrator`: all pass
- No remaining `run_q1_validation\b` in non-comment, non-test production code

### Dependencies

- Blocked by: none (independent of PKG-G)

---

## TASK-004 — Integration test T1–T5 for run_q1_validation_retained

**Status:** ASSIGNED  
**Package:** PKG-I  
**Assigned:** 2026-11-01T20:55:00Z

### Goal

Add `bind_validator_service_for_test` to `InstalledMontyCatalogue` and implement
the five integration test cases (T1–T5) specified in `docs/plans/q1_validation_v3.md`
§ Integration test. This provides the full end-to-end receipt for the retained Q1
path.

These are `#[cfg(feature = "integration")]` tests requiring a live PostgreSQL
instance. They do NOT need to be run by workers — the worker writes the code and
posts a BUILD_REQUEST; the coordinator runs the actual integration test.

### Exact steps

1. **Read** `docs/plans/q1_validation_v3.md` §§ "Integration test" and
   "Test wiring for `bind_validator_service` in test builds" (lines 245–330).
   These specify the exact test file, helper, and five test cases.

2. **Add** to `installed_monty_catalogue.rs`:
   ```rust
   #[cfg(test)]
   pub(crate) fn bind_validator_service_for_test(
       &self,
       driver: std::sync::Arc<crate::validator_task_driver::ValidatorTaskDriver>,
       pool: std::sync::Arc<brassclaw_pg::PgPool>,
   ) -> Result<(), brassclaw_agent_loop::AgentLoopDriverError> {
       self.bind_validator_service(driver, pool)
   }
   ```

3. **Create** `crates/brassclaw_reborn_composition/src/runtime/tests/validator_retained_q1_tests.rs`
   with tests T1–T5 exactly as specified in the plan. Key constraints:
   - T1 and T2: no live Monty needed, pool-only fixtures
   - T4/T5: require `#[cfg(feature = "integration")]` guard and `build_reborn_runtime`
   - Use `retain_validator_check_drafts` to seed validator recipes before T4/T5
   - Assert real DB rows in `reborn_validator_q1_evidence` after T4 passes

4. **Register** the new test module in `runtime.rs`:
   ```rust
   #[cfg(test)]
   mod validator_retained_q1_tests;
   ```
   (Check if this is already registered from the prior PKG-C work — `git log` to confirm.)

5. **Verify** (non-integration):
   ```
   cargo fmt --all -- --check
   cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
   cargo test -p brassclaw_reborn_composition --features postgres,skills-db validator_retained_q1
   ```
   Post BUILD_REQUEST with `features: postgres,skills-db,integration` and
   `test_filter: validator_retained_q1` for the coordinator to run the full suite.

### Files to touch

- `crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs`
- `crates/brassclaw_reborn_composition/src/runtime/tests/validator_retained_q1_tests.rs` (new)
- `crates/brassclaw_reborn_composition/src/runtime.rs` (module registration if needed)

### Acceptance

- Non-integration clippy: 0 warnings
- `cargo test ... validator_retained_q1` (non-integration): T1–T2 pass
- BUILD_REQUEST posted for integration run of T3–T5

### Dependencies

- Blocked by: TASK-002 (PKG-G) — bridge must be accessible in the composition path
  before the full T4/T5 end-to-end can be fully exercised. T1/T2 can be written
  and verified immediately; T3–T5 need PKG-G wired to confirm the full chain.

---

## TASK-005 — Qualify L013–L025 shell/git component exports and associations

**Status:** ASSIGNED  
**Package:** PKG-L-A (wave 1 of component migration)  
**Assigned:** 2026-11-01T20:55:00Z

### Goal

Components L013–L025 in `builtin_bootstrap.rs` have been migrated to the
preloadable `def usage_lNNN(inputs):` format but have NOT yet been qualified with:
- Declared `python-preload/2` dependency header
- Explicit exported function name in the `pc_export` field
- Reviewed association to their companion Skill prose rows

This task qualifies the L013–L025 component seeds: verifies the export names are
correct, adds missing `python-preload/2` declarations where absent, and confirms
the seed function names in `builtin_bootstrap.rs` are consistent.

### Exact steps

1. **Read** `docs/plans/monty.composition.implementation.md` §L013–L025 entries
   to get the canonical export names and input contracts for each component.

2. **Read** each component's current seed in `builtin_bootstrap.rs` — use:
   ```bash
   grep -n "L013\|L014\|L015\|L016\|L017\|L018\|L019\|L020\|L021\|L022\|L023\|L024\|L025" \
     crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs | head -60
   ```

3. For each L013–L025 component, verify:
   a. The body starts with `def usage_lNNN(inputs):` (or the canonical export name)
   b. The body contains a `# python-preload/2` comment or equivalent declaration
   c. The function returns the correct typed result (not a bare `host.*` top-level call)
   d. If a companion Skill exists (check `monty.composition.md` §1 table), that
      Skill's seed has a `pc_export` or `associated_pc` field pointing to this component

4. For each component with a missing or wrong `python-preload/2` declaration: add it.
   For each component with a wrong export name: correct it.

5. **Verify**:
   ```
   cargo fmt --all -- --check
   cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
   cargo test -p brassclaw_reborn_composition --features postgres,skills-db bootstrap
   ```

### Files to touch

- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` (L013–L025 seeds)

### Acceptance

- All L013–L025 bodies begin with a correct `def` export
- All have a `python-preload/2` declaration in the seed
- `cargo clippy`: 0 new warnings
- Bootstrap-related tests pass

### Dependencies

- Blocked by: none (fully independent)

---

<!-- Coordinator posts BUILD_RESULT blocks below after running each compile -->

---

## TASK-009 — Audit and qualify L026–L050 component body exports

**Status:** DONE (inline audit 2026-11-02)
**Package:** PKG-L-B
**Assigned:** 2026-11-02

### Result

All L026–L050 components confirmed already in correct preloadable `def` export format:
- L026 `extract_web_search_results(inputs)`, L027 `url_encode(inputs)` — ✅
- L028–L032 memory group: `usage_memory_search/write/patch/read/tree(inputs)` — ✅
- L033 `extract_section(inputs)`, L034 `format_memory_entry(inputs)` — ✅
- L035 `prepare_append_content(inputs)` — ✅ (pure prepare helper for 2-Tool monolith)
- L036–L041 git shell: `usage_l036–l041(inputs)` — ✅

No code changes required. TASK-009 closed.

---

## TASK-010 — Identify and document remaining open items for wave-3

**Status:** ASSIGNED
**Package:** PKG-AUDIT-3
**Assigned:** 2026-11-02

### Goal

Produce a bounded, prioritized list of the next 3–5 concrete implementation packages
for wave-3, drawn from the open items in `docs/implementation-status.md` and
`docs/plans/`. Focus on items that:
1. Have a clear bounded scope (< 500 lines of new code)
2. Do not depend on live Postgres/Monty for compilation evidence
3. Advance a plan gate (validator Step 7 class routes, K3 closure, prefix C1)

### Candidates to evaluate

1. **K3 closure prerequisite**: Check what BUILD-036 native/browser/worker/lint
   receipts are actually needed and whether any bounded code change can advance them.
   Read `simplified_v3.md` §K3 and `docs/plans/prefix_v3_upgrade.md` steps 2–5.

2. **Validator Step 7 class-1 route**: The structural presence check for class-1
   (Skill) is already seeded. What's needed to activate it? Check
   `validator_v3.md` §Step 7 class-1 route requirements vs current seeder state.

3. **R09 subagent Recipes**: `monty.composition.md` §3 lists 5 subagent Recipes.
   How many are seeded? What's missing? Read `builtin_bootstrap.rs` spawn/subagent
   section and compare to the plan.

4. **L010 `not_found` contract**: `missing1_v3_at_k3.md` documents the gap.
   Is there a concrete fix or does it require a new migration?

5. **prefix Phase C1**: What's the smallest deliverable in Phase C1?
   Read `docs/plans/prefix_v3_upgrade.md` §Phase C1.

### Acceptance

Produce a WAVE-3 ASSIGNMENTS block in this OUTBOX with specific bounded packages,
each with < 20-line step-by-step instructions.

### Dependencies

Blocked by: none (read-only audit)

---


---

## TASK-013 — Add content_checksum to remaining inline ToolSkill struct literals

**Status:** DONE (inline implementation 2026-11-02)
**Package:** PKG-TS-CHECKSUM
**Commit:** `5d186ce05`

### Result: PASS
- Modified `upsert_tool_skill()` in `builtin_bootstrap.rs` to auto-compute
  `sha256_hex(content)` when `source = "system"` and `content_checksum.is_none()`.
- Covers all 29 remaining inline ToolSkill literals without modifying each one.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings.

---

## TASK-014 — Add L010 file_exists to bootstrap_l013_approval TOOL_CALLING_PAIRS

**Status:** DONE (inline implementation 2026-11-02)
**Package:** PKG-L010-APPROVAL
**Commit:** `71c17a5fa`

### Result: PASS
- `PairSpec` for `pc-exec-file-exists` / `skill-file-exists` / `ts-file-exists`
  added to `TOOL_CALLING_PAIRS` array with `host.file_exists`, `use_file_exists` export,
  `{path: req_string()}` input and typed `{kind, path}` result.
- Module doc updated to mention L010.
- `cargo clippy`: 0 warnings.

---

## WAVE-4 ASSIGNMENTS

### TASK-015 — Verify and document validator Step 7 class-1 (Skill) PC body coverage

**Status:** ASSIGNED
**Package:** PKG-VALIDATOR-STEP7
**Assigned:** 2026-11-02

### Goal

`validator_v3.md` Step 7 lists 8 class-specific validator Recipes. The structural
scaffolding in `brassclaw_skills::structural_validator_recipe_drafts` produces
`ComponentRevisionDraft` JSON for all 8 classes (1–3, 0, 13, 21, 22, 23) but the
actual per-class PythonCode body content (the structural check logic for each class)
needs to be verified and documented.

### Exact steps

1. Read `validator_v3.md` §Step 7 lines covering class-1 (Skill), class-22
   (PythonCode) and class-21 (Recipe) route specifications. Understand what
   checks are required.

2. Read `crates/brassclaw_skills/src/validator_components.rs`:
   - Find `structural_validator_recipe_drafts(class_code, label, pc_body)` — note
     what `pc_body` is expected to contain.
   - Find where actual class-specific `pc_body` constants are defined for each of
     the 8 classes (search: `MINIMAL_PC_BODY` in tests, and real constants elsewhere).

3. Run `cargo test -p brassclaw_skills structural_validator_recipe` to confirm
   existing tests pass.

4. Document findings in a brief audit note: which class routes have a substantive
   `pc_body` vs a placeholder, and what Step 7 says each must check.

5. If any class route has a placeholder body where a specific structural check is
   specified: write the bounded implementation for that class route's PythonCode body
   (the body is stored in the `ComponentRevisionDraft`, not in `reborn_python_code`).

### Files to read

- `validator_v3.md` §Step 7 (especially class-1/Skill and class-22/PythonCode rows)
- `crates/brassclaw_skills/src/validator_components.rs`

### Acceptance

- `cargo test -p brassclaw_skills structural_validator_recipe`: all pass
- Brief audit note in INBOX confirming coverage vs placeholder state for each class
- If bodies updated: `cargo clippy -p brassclaw_skills --all-targets -- -D warnings`: 0 warnings

### Dependencies

- Blocked by: none

---

### TASK-016 — Add L001–L009 filesystem group to bootstrap_l013_approval

**Status:** ASSIGNED
**Package:** PKG-L001-APPROVAL
**Assigned:** 2026-11-02

### Goal

The filesystem Tool-calling pairs L001–L006 (read_file, write_file, list_dir,
glob, grep, apply_patch) are seeded through `seed_filesystem_group` but are
absent from `bootstrap_l013_approval.rs` `TOOL_CALLING_PAIRS`. They need
`skill-association-approval/1` records just like L010 and L013–L101.

The pure-logic helpers L007–L009, L011–L012 (filter helpers) should go in
`PURE_SPECS`.

### Exact steps

1. Read `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` to confirm
   the exact `pc_name`, `skill_name`, `ts_name`, `callable`, and `export` for
   L001–L006. Search:
   ```bash
   grep -n "pc-exec-read-file\|pc-exec-write-file\|pc-exec-list-dir\|pc-exec-glob\|pc-exec-grep\|pc-exec-apply-patch\|skill-read-file\|skill-write-file\|skill-list-dir\|skill-glob\|skill-grep\|skill-apply-patch" builtin_bootstrap.rs | head -40
   ```

2. For each of L001–L006, add a `PairSpec` entry to `TOOL_CALLING_PAIRS` in
   `bootstrap_l013_approval.rs` with the correct:
   - `pc_name`, `skill_name`, `toolskill_name`, `tool_name`, `callable`, `export`
   - `inputs` (at minimum `path: req_string()`)
   - `result: any_object_result`

3. For L007–L009, L011–L012 (filter/path helpers), add `PureSpec` entries to
   `PURE_SPECS`:
   ```
   pc-exec-grep-case-insensitive / pc-exec-grep-type-filtered (L007 area)
   pc-path-join / pc-path-basename / pc-path-dirname (L011–L012 area)
   ```
   Search for their export names in `builtin_bootstrap.rs`.

4. Verify: `cargo fmt --all -- --check` and
   `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`

### Acceptance

- L001–L006 PairSpec entries added; L007–L009/L011–L012 PureSpec entries added
- `cargo clippy`: 0 warnings

### Dependencies

- Blocked by: none

---


---

## TASK-017 — Add trigger management PairSpecs to bootstrap_l013_approval

**Status:** DONE (inline, 2026-11-01T18:45:00Z)
**Package:** PKG-TRIGGER-APPROVAL

### Summary

4 trigger PairSpecs added to `TOOL_CALLING_PAIRS` in `bootstrap_l013_approval.rs`:
- `pc-exec-trigger-list` / `skill-trigger-list` / `ts-trigger-list`
- `pc-exec-trigger-list-active` / `skill-trigger-list-active` / `ts-trigger-list`
- `pc-exec-trigger-list-scheduled` / `skill-trigger-list-scheduled` / `ts-trigger-list`
- `pc-exec-trigger-resolve-and-remove` / `skill-trigger-list` / `ts-trigger-remove`
  (dependent-chain exception: trigger_list result feeds trigger_remove)

Module doc updated to mention trigger management group.

`cargo fmt --all -- --check`: CLEAN
`cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings`: 0 warnings

---

## TASK-018 — Validator Step 7-B: negative fixture tests per class route

**Status:** ASSIGNED
**Package:** PKG-VALIDATOR-STEP7B
**Assigned:** 2026-11-01T18:45:00Z

### Goal

For each of the 8 class validator routes, add at least 2 fixture tests:
1. Valid minimal candidate → `{"pass": true, "errors": []}`.
2. Invalid candidate with a class-specific defect → `{"pass": false, "errors": [<non-empty>]}`.

Tests run the PC body through the existing `brassclaw_skills` test infrastructure
(not a live Monty; parse + eval the body as Python, feed inputs, check outputs).

### Exact steps

1. **Read** `crates/brassclaw_skills/src/validator_components.rs` to understand
   the `structural_validator_recipe_drafts` API and what the 8 PC body constants
   return. The body constants for classes 0,1–3,13,21,22,23 were added in TASK-015.

2. **Locate** or **create** a test module in `validator_components.rs` (or a separate
   file `crates/brassclaw_skills/src/validator_component_tests.rs` if the module is
   too large). Use `#[cfg(test)]` blocks.

3. **For each class**, write:
   ```rust
   // Class-N valid fixture
   #[test]
   fn class_N_valid_candidate_passes() {
       let body = CLASS_N_STRUCTURAL_BODY;
       // Construct a minimal valid JSON candidate for class N (must have all
       // required fields per validator_v3.md §Step 7 for that class).
       // Feed via a mock inputs dict: {"content": serde_json::to_string(&candidate)?}.
       // Assert output has "pass": true.
       // NOTE: use the same invoke mechanism as existing tests in brassclaw_skills.
   }
   // Class-N invalid fixture (class-specific defect)
   #[test]
   fn class_N_invalid_candidate_fails() { ... }
   ```

4. **Look at existing tests** in `brassclaw_skills` to understand how PC bodies are
   invoked (e.g., `brassclaw_skills/src/tests/` or `#[cfg(test)]` blocks in
   `validator_components.rs`). Reuse the same invocation pattern — no new test harness.

5. **Verify** with:
   ```
   cargo test -p brassclaw_skills -- validator
   ```
   At least 16 tests (2 per class × 8 classes) must pass.

6. **Acceptance gate from missing4_v3_at_k3.md §Step 7-B:**
   - `cargo test -p brassclaw_skills validator_class`: ≥ 16 cases, all pass.
   - Each test uses the actual PC body constant (not a stub).

### Files to touch

- `crates/brassclaw_skills/src/validator_components.rs` — add `#[cfg(test)]` module
  (or new `validator_component_tests.rs` registered in `mod.rs`)

### Dependencies

- TASK-015 must be DONE (class-specific PC body constants in place). ✓ Already done.
- Does NOT require TASK-017 (independent).

---

## TASK-019 — Security grep pairing: add leaf skills + PairSpecs for pc-grep-* components

**Status:** ASSIGNED
**Package:** PKG-SECURITY-APPROVAL
**Assigned:** 2026-11-01T18:45:00Z

### Goal

Three seeded PythonCode components (`pc-grep-hardcoded-secrets`, `pc-grep-injection-patterns`,
`pc-grep-env-files`) call `host.grep` / `host.glob` but currently have no dedicated leaf
skill pairing in `bootstrap_l013_approval.rs`. This prevents `skill-association-approval/1`
records from being written for them at boot.

Resolution: add 3 lightweight leaf skill rows in `builtin_bootstrap.rs` (Pass 13 area,
after the existing PC upserts) and add 3 PairSpecs to `TOOL_CALLING_PAIRS`.

### Exact steps

**Step 1 — Identify exact export function names:**
```
pc-grep-hardcoded-secrets  → export: "usage_l084"  (calls host.grep)
pc-grep-injection-patterns → export: "usage_l085"  (calls host.grep)
pc-grep-env-files          → export: "glob_env_files" (calls host.glob)
```
(Confirmed from PC body constants in `builtin_bootstrap.rs` ~line 18923–18936.)

**Step 2 — Add 3 leaf skill rows in `builtin_bootstrap.rs`:**

In Pass 13 (security-review stack), after `let pc_grep_env_files = ...` (~line 16793)
and before the leaf skills block, add:

```rust
let skill_grep_hardcoded_secrets = stores
    .upsert_skill(
        leaf_skill(
            &tenant,
            "skill-grep-hardcoded-secrets",
            "Leaf skill: grep for hardcoded secret patterns using host.grep.",
            "Use host.grep to search for hardcoded credential patterns \
             (password/api_key/token/secret/credential). Supply a path input. \
             Returns matching lines with file context.",
        ),
        "skill-grep-hardcoded-secrets",
    )
    .await?;
let skill_grep_injection_patterns = stores
    .upsert_skill(
        leaf_skill(
            &tenant,
            "skill-grep-injection-patterns",
            "Leaf skill: grep for injection-risk patterns using host.grep.",
            "Use host.grep to search for injection-risk patterns \
             (eval/exec/subprocess/shell_exec/format!.*sql/query.*format). \
             Supply a path input. Returns matching lines.",
        ),
        "skill-grep-injection-patterns",
    )
    .await?;
let skill_grep_env_files = stores
    .upsert_skill(
        leaf_skill(
            &tenant,
            "skill-grep-env-files",
            "Leaf skill: glob for .env files using host.glob.",
            "Use host.glob with pattern '**/.env*' to discover all .env \
             configuration files in the workspace. Returns a list of matching paths.",
        ),
        "skill-grep-env-files",
    )
    .await?;
```

Also add the 3 new skill UUIDs to the `security_review_children` Vec.

**Step 3 — Add 3 PairSpecs to `TOOL_CALLING_PAIRS` in `bootstrap_l013_approval.rs`:**

After the trigger management group (or before — ordering doesn't matter for correctness):

```rust
// ---- Security grep group (L083–L085) ----------------------------------------
PairSpec {
    pc_name: "pc-grep-hardcoded-secrets",
    skill_name: "skill-grep-hardcoded-secrets",
    toolskill_name: "ts-grep",
    tool_name: "grep",
    callable: "host.grep",
    export: "usage_l084",
    inputs: || json!({"path":opt_string()}),
    arguments: || json!({"path":"path"}),
    code_arguments: || json!({}),
    result: any_object_result,
},
PairSpec {
    pc_name: "pc-grep-injection-patterns",
    skill_name: "skill-grep-injection-patterns",
    toolskill_name: "ts-grep",
    tool_name: "grep",
    callable: "host.grep",
    export: "usage_l085",
    inputs: || json!({"path":opt_string()}),
    arguments: || json!({"path":"path"}),
    code_arguments: || json!({}),
    result: any_object_result,
},
PairSpec {
    pc_name: "pc-grep-env-files",
    skill_name: "skill-grep-env-files",
    toolskill_name: "ts-glob",
    tool_name: "glob",
    callable: "host.glob",
    export: "glob_env_files",
    inputs: || json!({}),
    arguments: || json!({}),
    code_arguments: || json!({}),
    result: any_object_result,
},
```

**Step 4 — Verify:**
```
cargo fmt --all -- --check
cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
```
0 warnings required.

### Files to touch

- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` — 3 new leaf_skill upserts + security_review_children update
- `crates/brassclaw_reborn_composition/src/bootstrap_l013_approval.rs` — 3 new PairSpecs

### Dependencies

- TASK-017 DONE ✓ (can safely append after the trigger group)

---

## TASK-020 — ITEM-C: build monty_worker + run T2–T5 integration tests

**Status:** ASSIGNED
**Package:** PKG-MONTY-TESTS
**Assigned:** 2026-11-01T18:45:00Z

### Goal

Run the 6 `run_q1_validation_retained` integration tests (T1–T5 + T5b) that require the
`monty_worker` binary. Currently source-only; T1 passes under `--lib`; T2–T5/T5b need
the binary present.

### Exact steps

**Step 1 — Disk check (required before every Cargo invocation):**
```bash
df -h /Users/ollama/brassclaw-target
```
Abort if < 15 GiB free.

**Step 2 — Build test binary + monty_worker in a screen session:**
```bash
screen -dmS build-monty-tests bash -c '
  cd /Volumes/SSDE/brassclaw
  cargo build -p brassclaw_reborn_composition \
    --features postgres,skills-db \
    --tests \
    2>&1 | tee /tmp/monty-test-build.log
  echo "EXIT:$?" >> /tmp/monty-test-build.log
'
```
Monitor: `tail -f /tmp/monty-test-build.log`

**Step 3 — Locate test binary and confirm monty_worker present:**
```bash
ls $(cargo test -p brassclaw_reborn_composition \
  --features postgres,skills-db --no-run 2>&1 \
  | grep -oE '/[^ ]+brassclaw_reborn_composition[^ ]+' | head -1 | xargs dirname)/ \
  | grep monty_worker
```

**Step 4 — Run integration tests:**
```bash
screen -dmS run-integration bash -c '
  cd /Volumes/SSDE/brassclaw
  cargo test -p brassclaw_reborn_composition \
    --features postgres,skills-db \
    run_q1_validation_retained \
    2>&1 | tee /tmp/integration-tests.log
  echo "EXIT:$?" >> /tmp/integration-tests.log
'
```
Expected: T1 + T2 + T3 + T4 + T5 + T5b all pass (6 tests).

**Step 5 — Verify T5 evidence rows** (per missing3_v3_at_k3.md §4 Step 3):
After T5 passes, the test body asserts:
- `reborn_component_review_evidence` has ≥ 1 row linked via `reborn_validator_q1_runs`
  with `succeeded = true`.
- Validation queue row at `STATE_Q1_PASSED` (state = 2).

**Step 6 — Update docs/implementation-status.md** with exact cargo invocation, exit status,
T5 evidence confirmation, and note that `missing3_v3_at_k3.md` ITEM-C is CLOSED.

### Dependencies

- Requires the `monty_worker` binary to be available (built via Step 2).
- Independent of TASK-017/018/019.
- This is a compilation-heavy task: build it in a screen session and do other work
  while it compiles.


---

## TASK-021 — Cover remaining seeded Tool-calling PCs in bootstrap_l013_approval

**Status:** ASSIGNED
**Package:** PKG-REMAINING-PAIRS
**Assigned:** 2026-11-01T19:30:00Z

### Goal

Several seeded PythonCode components that call `host.*` are not yet covered by
`TOOL_CALLING_PAIRS` in `bootstrap_l013_approval.rs`. This task audits and adds
the missing PairSpecs (and any required paired leaf skills in `builtin_bootstrap.rs`).

### Step 1 — Identify each uncovered PC's host callable and skill pairing

For each of these component names, read their body constants in `builtin_bootstrap.rs`
to determine:
(a) which `host.*` call they make (→ determines toolskill + tool_name)
(b) whether a dedicated leaf skill already exists with the same name pattern
    (e.g., `skill-read-file-head` for `pc-exec-read-file-head`)
(c) export function name

**Known gaps (from session-5 audit):**
- `pc-exec-http-save` — writes response body to file; calls host.write_file?
- `pc-exec-list-then-grep` — dependent chain: list_dir → grep
- `pc-exec-read-file-head`, `pc-exec-read-file-tail` — calls host.read_file with head/tail variant
- `pc-exec-read-then-grep` — dependent chain: read_file → grep
- `pc-exec-shell-wc-l` — calls host.shell with ts-shell-run
- `pc-exec-time-now`, `pc-exec-time-parse`, `pc-exec-time-convert`, `pc-exec-time-diff`,
  `pc-exec-time-format` — call host.time_now / host.time_parse (management group)
- `pc-github-get-authenticated-user`, `pc-github-search-issues` — call host.http
- `pc-glob-test-files-py/rust/ts` — call host.glob
- `pc-grep-fn-tests` — call host.grep
- `pc-plan-status-update` — calls host.write_file or host.apply_patch
- `pc-exec-json-query`, `pc-exec-json-stringify`, `pc-exec-json-validate` — may be pure-logic
- `pc-json-extract-field`, `pc-url-encode` — likely pure-logic → PURE_SPECS
- `pc-exec-list-filter-by-type` — likely pure-logic → PURE_SPECS
- `pc-host-assemble-prefix-bundle`, `pc-host-post-reply` — internal orchestrator PCs;
  check if they should be excluded from approval (they may be class-10 / system-only paths)
- `pc-exec-echo` — diagnostic-only per AGENTS.md; skip

### Step 2 — For each Tool-calling PC without a dedicated skill, add a leaf skill

If the PC has no named leaf skill pairing (e.g., `skill-read-file-head`), add one in
the appropriate Pass section of `builtin_bootstrap.rs` (same pattern as TASK-019).

### Step 3 — Add PairSpecs to TOOL_CALLING_PAIRS

For dependent-chain pairs (e.g., `pc-exec-read-then-grep`), use the effect Tool's
ToolSkill as the binding ToolSkill, per the dependent-chain exception.

For pure-logic components (`pc-json-extract-field`, `pc-url-encode`,
`pc-exec-list-filter-by-type`, `pc-exec-json-*`), add PureSpec entries instead.

### Step 4 — Verify

```
cargo fmt --all -- --check
cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings
```
0 warnings required.

### Files to touch

- `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` — new leaf skills where needed
- `crates/brassclaw_reborn_composition/src/bootstrap_l013_approval.rs` — new PairSpecs + PureSpecs

### Dependencies

- TASK-019 DONE ✓

