# OUTBOX — Coordinator Assignment Queue

_Coordinator writes ASSIGNMENT blocks here. Workers poll every 10 s._

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
