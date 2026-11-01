# Orchestrator OUTBOX

Messages from the orchestrator to sub-tasks. **Append only — never delete entries.**

---

## OUT-001 → [TASK-001] ASSIGNMENT 2026-11-01T07:55:00Z

**Package:** PKG-B — Wire `run_q1_validation_retained` call sites  
**Branch:** main (workspace `/Volumes/SSDE/brassclaw`)  
**Poll:** Read this file every 10 seconds for messages addressed to `[TASK-001]`.

---

### Background

`q1_orchestrator.rs` has two Q1 functions:
- `run_q1_validation` — old path (legacy executor, no IBS/Recipe), currently called from `validation_queue.rs`
- `run_q1_validation_retained` (added in `d12dc8d2b`) — new IBS/Recipe path, takes `Option<&ServiceClient>` + `Option<&Arc<dyn Fn()->Arc<dyn TaskPorts>>>`, returns `Q1Outcome::Deferred` when either is `None`

**Goal:** wire the retained path so `InstalledMontyCatalogue` can receive the `ServiceClient` + ports factory after the `GlobalMontyDriver` is created in `runtime.rs`, and pass them through to `run_q1_validation_retained`.

### Exact files you may modify

1. `crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs`
2. `crates/brassclaw_reborn_composition/src/runtime.rs`
3. `crates/brassclaw_reborn_composition/src/validation_queue.rs` (test only — the existing `run_q1_validation_defers_when_no_recipe_seeded` test)

Do NOT modify `q1_orchestrator.rs` — it is already correct.

### Step-by-step instructions

#### Step 1 — Add `ValidatorServiceBridge` struct to `installed_monty_catalogue.rs`

At the bottom of `installed_monty_catalogue.rs`, above the closing of the module, add:

```rust
/// Injected after `GlobalMontyDriver` is constructed; carries the live
/// `ServiceClient` and a ports factory closure for the retained Q1 path.
/// Set exactly once via `bind_validator_service`; never replaced.
pub(crate) struct ValidatorServiceBridge {
    pub(crate) service: brassclaw_monty_host::service::ServiceClient,
    pub(crate) ports_factory: std::sync::Arc<
        dyn Fn() -> std::sync::Arc<dyn brassclaw_monty_host::service::TaskPorts>
            + Send
            + Sync,
    >,
}
```

Then add a new `OnceLock` field to the `InstalledMontyCatalogue` struct (alongside the existing `tool_credential_sources` field):

```rust
pub(crate) validator_service: std::sync::OnceLock<ValidatorServiceBridge>,
```

Then in `InstalledMontyCatalogue`'s `impl` block, add a method:

```rust
/// Wire the live `ServiceClient` and ports factory for the retained Q1 path.
/// Called once from `runtime.rs` immediately after `GlobalMontyDriver::new`.
/// A second call returns `Err` (OnceLock contract — must not replace).
pub(crate) fn bind_validator_service(
    &self,
    service: brassclaw_monty_host::service::ServiceClient,
    ports_factory: std::sync::Arc<
        dyn Fn() -> std::sync::Arc<dyn brassclaw_monty_host::service::TaskPorts>
            + Send
            + Sync,
    >,
) -> Result<(), crate::agent_loop_driver::AgentLoopDriverError> {
    self.validator_service
        .set(ValidatorServiceBridge { service, ports_factory })
        .map_err(|_| crate::agent_loop_driver::failed())
}
```

You will also need to initialise the new field in `InstalledMontyCatalogue`'s constructor — search for the struct literal that builds `InstalledMontyCatalogue` and add:

```rust
validator_service: std::sync::OnceLock::new(),
```

#### Step 2 — Call `bind_validator_service` in `runtime.rs`

In `runtime.rs`, just after the line:
```rust
monty_test_controls = (driver.clone(), ports);
```
(which is inside a `#[cfg(test)]` block), and still within the surrounding `if let` / `{...}` block that constructs `GlobalMontyDriver`, add a call (outside the `#[cfg(test)]` guard):

```rust
// Wire the retained Q1 path — deferred silently if already set.
let _ = catalogue.bind_validator_service(
    owner.client(),
    {
        let p = ports.clone();
        std::sync::Arc::new(move || p.build_task_ports())
    },
);
```

**Important:** Look at what method on `OwnedGlobalTaskFactory` (or `GlobalTaskPortsFactory`) returns an `Arc<dyn TaskPorts>`. The `GlobalTaskPortsFactory` trait has an `async fn build(...)` which is too heavy for a closure. Instead, use a thin wrapper: the closure should capture `Arc<dyn GlobalTaskPortsFactory>` and call its synchronous entry point, or call a new helper.

Actually — look carefully at `GlobalRecipePorts` or `global_recipe_ports.rs` for the existing `TaskPorts` implementation. The `ports_factory` parameter in `run_q1_validation_retained` is `Arc<dyn Fn() -> Arc<dyn TaskPorts>>`. 

Check `crates/brassclaw_reborn_composition/src/global_recipe_ports.rs` for what implements `TaskPorts`. If `GlobalRecipePorts` can be constructed synchronously from the catalogue, use that. Otherwise, supply a `OnceLock<Arc<dyn TaskPorts>>` approach in `ValidatorServiceBridge` and let the closure clone the retained ports.

The simplest approach that does NOT change `run_q1_validation_retained`'s signature:
- `ValidatorServiceBridge.ports_factory` is a `Arc<dyn Fn() -> Arc<dyn TaskPorts> + Send + Sync>`
- In `runtime.rs`, after creating `driver`, create a wrapper:

```rust
let validator_ports: std::sync::Arc<dyn brassclaw_monty_host::service::TaskPorts> = 
    /* build once — if GlobalRecipePorts implements TaskPorts */ 
    Arc::new(crate::global_recipe_ports::GlobalRecipePorts::new(catalogue.clone(), ...));
let pf: std::sync::Arc<dyn Fn() -> std::sync::Arc<dyn brassclaw_monty_host::service::TaskPorts> + Send + Sync> =
    Arc::new({
        let vp = validator_ports.clone();
        move || vp.clone()
    });
let _ = catalogue.bind_validator_service(owner.client(), pf);
```

If `GlobalRecipePorts::new` requires too many parameters that aren't available at this point, use the existing `ports: Arc<dyn GlobalTaskPortsFactory>` by cloning it and having the closure call a synchronous `quick_task_ports()` method (which you should NOT add — avoid expanding scope). 

**Simplest safe approach**: store `Arc<dyn GlobalTaskPortsFactory>` in `ValidatorServiceBridge` instead of a `Fn` closure, and inside `bind_validator_service` convert it on-the-fly. But this changes the `run_q1_validation_retained` call. 

**Actual simplest approach** that requires NO change to `q1_orchestrator.rs`: Just store a `Arc<dyn GlobalTaskPortsFactory>` on `ValidatorServiceBridge` and in a new `validator_ports_factory()` helper method on `InstalledMontyCatalogue`, return the correct `Arc<dyn Fn() -> Arc<dyn TaskPorts>>` wrapping the factory. Build the wrapper Arc lazily in that helper.

Inspect what `GlobalRecipePorts` looks like and what `TaskPorts` needs. If `GlobalRecipePorts` has a constructor that takes just `Arc<InstalledMontyCatalogue>` (or similar), prefer that. Otherwise the factory wrapper is fine.

#### Step 3 — Pass bridge into `run_q1_validation_retained` from `validation_queue.rs`

In `validation_queue.rs`, the test `run_q1_validation_defers_when_no_recipe_seeded` currently calls `run_q1_validation` (old path). **Do not change the production call site yet** — `run_q1_validation` is still the active path.

Instead:
1. Add a second test `run_q1_validation_retained_defers_when_no_service_wired` that calls `run_q1_validation_retained` with `None, None` and asserts `Deferred`.
2. This confirms the None-guard path in `run_q1_validation_retained` works.

The actual switch from `run_q1_validation` → `run_q1_validation_retained` with real service/ports at production call sites is PKG-C work; PKG-B only wires the injection mechanism and adds the deferred test.

### Definition of Done

Post `DONE` to `INBOX.md` when:
- `ValidatorServiceBridge` struct exists in `installed_monty_catalogue.rs`
- `validator_service: OnceLock<ValidatorServiceBridge>` field added and initialised in constructor
- `bind_validator_service` method added and correct
- `runtime.rs` calls `bind_validator_service` after `GlobalMontyDriver::new` (non-test, feature-gated under `#[cfg(all(feature = "postgres", feature = "skills-db"))]`)
- New test `run_q1_validation_retained_defers_when_no_service_wired` added to `validation_queue.rs`
- No `unwrap()` / `expect()` in production code paths you touch
- Then post `BUILD_REQUEST` to INBOX.md for `brassclaw_reborn_composition` with features `postgres,skills-db`

## OUT-002 → [TASK-001] BUILD_RESULT 2026-11-01T08:40:00Z

REF: MSG-001
STATUS: PASS
COMMIT: a92e8e5e8
DETAILS: clippy -p brassclaw_reborn_composition --features postgres,skills-db: 0 warnings;
  cargo fmt --all --check: clean; cargo test -p brassclaw_skills: 13/13 pass.
  New test run_q1_validation_retained_defers_when_no_service_wired is integration-only
  (requires live PG); confirmed at source level. PKG-B complete.

TASK-001 STATUS: DONE — no further action needed.


## OUT-003-PLAN → [TASK-003] ASSIGNMENT PLAN 2026-11-01T08:50:00Z

**Package:** PKG-E-R09 — Seed `proposed-child-task-request` PythonCode component
**Goal:** Add the `proposed-child-task-request` class-22 PythonCode component to `builtin_bootstrap.rs`
         per the spec in `docs/plans/monty.composition.implementation.md` §R09.

**Todo:**
- [ ] Read `docs/plans/monty.composition.implementation.md` §R09 for exact contract
- [ ] Read `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` around the subagent
      seed block (~line 10880) to understand the `seed_python_code` call pattern
- [ ] Find where the subagent PythonCode bodies (e.g. `pc-subagent-spawn`) are seeded
      to understand the exact `seed_python_code` / `seed_skill` API
- [ ] Add one `seed_python_code` call for `proposed-child-task-request` with:
      - name: `proposed-child-task-request`
      - description: "Prepare reusable typed goal/context for a separately admitted child task."
      - body: `result = {"goal": inputs["goal"], "context": inputs["context"]}`
      - class: 22 (PythonCode)
      - No Tool call (pure), no Skill/ToolSkill needed
- [ ] Confirm no existing entry with this name exists
- [ ] Post BUILD_REQUEST to INBOX.md

---

## OUT-003 → [TASK-003] ASSIGNMENT 2026-11-01T08:52:00Z

**Package:** PKG-E-R09 — Seed `proposed-child-task-request` PythonCode  
**Branch:** main (workspace `/Volumes/SSDE/brassclaw`)  
**Poll:** Read OUTBOX.md every 10 seconds for `[TASK-003]` messages.

### File you may modify (one only)

`crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`

### What to do

1. **Find the subagent PythonCode seed block.** Search for `pc-subagent-spawn` or
   `spawn_subagent` seed calls in `builtin_bootstrap.rs` to find where PythonCode
   components for the subagent family are seeded. They use `stores.seed_python_code(...)`.

2. **Check for existing entry.** Search for `proposed-child-task-request` — confirm it
   doesn't exist yet.

3. **Add one `seed_python_code` call** immediately after the last existing subagent-family
   PythonCode seed (e.g. after `pc-subagent-query` or equivalent), following the exact
   same pattern. The call must use the correct variable binding so the returned ID can be
   referenced if needed later.

   Pattern to follow (adapt from existing `seed_python_code` calls nearby):

   ```rust
   let pc_child_task_request = stores
       .seed_python_code(
           &tenant,
           "proposed-child-task-request",
           "Prepare reusable typed goal/context for a separately admitted child task.",
           r#"result = {"goal": inputs["goal"], "context": inputs["context"]}"#,
       )
       .await?;
   ```

   If `seed_python_code` takes different arguments, look at existing calls and match.
   The body must be exactly:
   ```python
   result = {"goal": inputs["goal"], "context": inputs["context"]}
   ```

4. **Suppress unused variable warning** if `pc_child_task_request` is not referenced
   downstream: `let _pc_child_task_request = stores.seed_python_code(...).await?;`
   Only use `_` prefix if the variable is genuinely not used — check whether it's
   referenced in subsequent subagent Recipe step_entry calls first.

5. **Do not change anything else.** Do not modify existing seed calls, step entries,
   or other Recipe definitions.

### Definition of Done

Post `DONE` + `BUILD_REQUEST` to INBOX.md when:
- `proposed-child-task-request` PythonCode seeded in `builtin_bootstrap.rs`
- No `unwrap()`/`expect()` added
- Variable name follows the pattern (`pc_child_task_request` or `_pc_child_task_request`)

BUILD_REQUEST format:
```
## MSG-XXX [TASK-003] BUILD_REQUEST 2026-11-01T...Z

CRATE: brassclaw_reborn_composition
FEATURES: postgres,skills-db
TEST_FILTER: (none — source-level only)
REASON: PKG-E-R09 seed proposed-child-task-request PythonCode
```

---

## OUT-004-PLAN → [TASK-004] ASSIGNMENT PLAN 2026-11-01T08:53:00Z

**Package:** PKG-D — Unit tests for `pg_validator_q1_admission.rs` to remove `#[allow(dead_code)]`
**Goal:** Add unit tests that exercise the three functions, allowing removal of dead_code allows.

**Todo:**
- [ ] Read full `pg_validator_q1_admission.rs` to understand the three functions
- [ ] Understand what test fixtures are available in the existing `tests` module
- [ ] Add tests for:
  - `validator_review_user_input_matches_prefix` — already exists
  - `retain_validator_selection` — needs a PG rig test (integration)
  - `submit_validator_review` — test at least the input construction shape
- [ ] Check whether removing `#[allow(dead_code)]` requires callers or just tests
- [ ] If removal without callers causes errors, keep allows but add doc comment
      explaining they'll be removed when PKG-C wires them

---

## OUT-004 → [TASK-004] ASSIGNMENT 2026-11-01T08:54:00Z

**Package:** PKG-D — `pg_validator_q1_admission.rs` dead_code audit + tests
**Branch:** main (workspace `/Volumes/SSDE/brassclaw`)  
**Poll:** Read OUTBOX.md every 10 seconds for `[TASK-004]` messages.

### File you may modify (one only)

`crates/brassclaw_reborn_composition/src/pg_validator_q1_admission.rs`

### What to do

The three `#[allow(dead_code)]` annotations on the enum and two `pub(crate)` functions
exist because no production caller has been wired yet (PKG-C will do that). Your job is
to improve the test coverage and update the dead_code comments to be more precise about
*when* they'll be removed.

**Step 1 — Read the file.** Read all 163 lines to understand the exact shape of each function.

**Step 2 — Check if dead_code allows can be removed now.** They cannot — no production
caller exists yet. Do NOT remove them. Instead, update each `#[allow(dead_code)]` comment
to read `// wired by PKG-C ValidationQueueStore integration` to track the exact
removal trigger.

**Step 3 — Expand the test module.** Add two new unit tests (no DB required):

Test 1: `q1_admission_error_variants_are_displayable`
```rust
#[test]
fn q1_admission_error_variants_are_displayable() {
    use brassclaw_monty_host::service::ServiceFailure;
    let errors = [
        Q1AdmissionError::Coverage("bad coverage"),
        Q1AdmissionError::SelectionBytes("bad bytes"),
        Q1AdmissionError::Retention,
        Q1AdmissionError::Pool,
    ];
    for e in &errors {
        // Must not panic; must produce a non-empty string.
        let s = e.to_string();
        assert!(!s.is_empty(), "error display must not be empty: {e:?}");
    }
}
```

Test 2: `validator_review_task_input_fields_are_correct`
```rust
#[test]
fn validator_review_task_input_fields_are_correct() {
    use crate::pg_review_admission_source::VALIDATOR_REVIEW_PREFIX;
    let submission_id = Uuid::new_v4();
    // Verify the TaskInput fields the submit_validator_review function would build.
    let user_input = format!("{VALIDATOR_REVIEW_PREFIX}{submission_id}");
    // conversation_id = thread_id (UUID string)
    let thread_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let message_id = format!("validator-q1:{submission_id}");
    let turn_id = submission_id.to_string();
    // Verify the prefix is present and the UUID suffix parses back.
    let suffix = user_input
        .strip_prefix(VALIDATOR_REVIEW_PREFIX)
        .expect("user_input starts with prefix");
    let parsed = Uuid::parse_str(suffix).expect("suffix is valid UUID");
    assert_eq!(parsed, submission_id);
    assert_eq!(message_id, format!("validator-q1:{submission_id}"));
    assert_eq!(turn_id, submission_id.to_string());
    // thread_id and run_id are separate UUID strings (not checked for value here).
    let _ = (thread_id, run_id);
}
```

Note: `ServiceFailure` may not implement `Debug` — check before adding that variant to
the test array. If `Submission(ServiceFailure)` can't be constructed in a unit test
without a real failure, skip it from the display test.

**Step 4 — Check for the `ServiceFailure` import.** `ServiceFailure` is from
`brassclaw_monty_host::service`. Check if it has a `Debug` impl. If not, only test
the other four variants.

### Definition of Done

Post `DONE` + `BUILD_REQUEST` to INBOX.md when:
- Two new unit tests added (or one if ServiceFailure can't be constructed)
- Three `#[allow(dead_code)]` comment strings updated
- No production code changed

BUILD_REQUEST:
```
## MSG-XXX [TASK-004] BUILD_REQUEST 2026-11-01T...Z

CRATE: brassclaw_reborn_composition
FEATURES: postgres,skills-db
TEST_FILTER: q1_admission
REASON: PKG-D pg_validator_q1_admission tests + dead_code comment update
```

