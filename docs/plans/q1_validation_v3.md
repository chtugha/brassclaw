# Q1 Validation Retained — Implementation Plan

> **Revision 4 — 2026-11-01 (post-audit)**
> All instructions are grounded in direct source inspection. No stubs, no
> simulations, no magic numbers. Every deferred path returns `Q1Outcome::Deferred`
> without fabricated success; every passing path requires real evidence in the DB.
>
> This revision corrects three critical architectural errors in Revision 3:
> 1. `AdmissionMode::RetainedParent` is `#[cfg(test)]` only — cannot be used in production.
> 2. `bootstrap_validation::admit` has a different signature than described.
> 3. `GlobalTaskPortsFactory::build` cannot be called externally; it double-admits.

---

## Context and Architecture

`run_q1_validation_retained` (in [`q1_orchestrator.rs`]) is the PKG-C entry point
for the retained IBS/Recipe Q1 path. It is called fire-and-forget from
`ValidationQueueStore::submit` and `fire_q1_kick_if_wired`, after a component has
been enqueued.

The function currently receives:
- `service: Option<&ServiceClient>` — the live global Monty service client
- `ports_factory: Option<&Arc<dyn Fn() -> Arc<dyn TaskPorts> + Send + Sync>>` —
  a closure that must produce a `TaskPorts` for each validator task

**Current state (PKG-B):** `make_validator_ports_factory` returns `ValidatorDeferredPorts`
which always fails. `fire_q1_kick_if_wired` and `submit` pass `Some(&service)` and
`Some(&ports_factory)` — so the function proceeds past step 1 but the ports
returned by `ports_factory()` fail every call the orchestrator makes.

### The correct architecture for a pure-logic validator task

The structural validator Recipe is pure-logic (zero Tool bindings). When it runs
through the global Monty service:

1. The service calls `GlobalTaskPortsFactory::build(host, input)` — **but only via
   `GlobalMontyDriver::drive_turn`**, which creates the `MontyTaskHost` from a
   `MontyTaskHandoff`. `build` is never called externally.
2. `build` calls `capture_internal` on the catalogue; this returns the
   `ReviewAdmissionSource` for the validator prefix.
3. `build` calls `GlobalRecipePorts::for_internal(host, admission, source, ...)`.
4. The service submits the task with those ports.
5. The Python orchestrator calls `host.select_internal_recipe(name)`, which calls
   `GlobalRecipePorts::call("select_internal_recipe", ...)`.
6. The Python orchestrator runs the validator Recipe steps.
7. The Python orchestrator calls `host.finish(InternalCompleted { receipt_ref })`.
8. The service calls `GlobalRecipePorts::finish(outcome)`.
9. `GlobalRecipePorts::finish` calls `source.verify_receipt(receipt, selected, execution)`.
10. `verify_receipt` persists the Q1 evidence.

**The validator task must go through `drive_turn`**, not through
`service.submit_when_available` directly. Direct `submit_when_available` bypasses
`build` and thus bypasses `capture_internal` and `GlobalRecipePorts` entirely.

The current architecture's `ports_factory()` closure is the wrong abstraction for
this reason: the `TaskPorts` for a validator task is the `GlobalRecipePorts` built
by `build`, not something a factory closure can produce before admission.

### How to feed a validator task through `drive_turn`

`drive_turn` takes a `MontyTaskHandoff`. A `MontyTaskHandoff` is produced from a
`MontyTaskAttempt` + host + `AgentLoopDriverRunRequest`. The normal path
(`bootstrap_validation::admit`) produces all of these via:
1. Thread/turn creation
2. Turn claim (`state.claim_next_run`)
3. `MontyTaskHandoff::new_claimed(request, claimed, host)` — in `AdmissionMode::RetainedParent`
   (`#[cfg(test)]` only), or `MontyTaskHandoff::new(request, attempt, host)` in
   `AdmissionMode::Contained`.

**Critical finding:** `AdmissionMode::RetainedParent` is `#[cfg(test)]` only
(lines 133, 149, 150 of `bootstrap_validation.rs`). The
`context.with_trusted_internal_turn()` call only happens inside
`#[cfg(test)]` blocks (lines 294–297). There is **no non-test path** through
`bootstrap_validation::admit` that sets `trusted_internal_turn = true`.

For a production validator task, `trusted_internal_turn` must be set in the
`RunContext` before `drive_turn` is called. The correct approach is to add a new
`AdmissionMode` variant to `bootstrap_validation.rs` that is not `#[cfg(test)]`
and sets `trusted_internal_turn = true`.

---

## Existing infrastructure (do not duplicate)

| Component | File | Status |
|-----------|------|--------|
| `ReviewAdmissionSource` | `pg_review_admission_source.rs` | Complete |
| `verify_receipt` on `ReviewAdmissionSource` | `pg_review_admission_source.rs:242` | Complete |
| `capture_internal` branch for `VALIDATOR_REVIEW_PREFIX` | `installed_monty_catalogue.rs:1154` | Complete; requires `trusted_internal_turn = true` |
| `retain_validator_selection` | `pg_validator_q1_admission.rs:75` | Complete |
| `persist_validator_q1_evidence` | `pg_validator_q1_evidence.rs` | Complete; `#[allow(dead_code)]` pending wiring |
| `persist_workflow_review` | `pg_workflow_review.rs` | Complete; `#[allow(dead_code)]` pending wiring |
| `ValidationQueueStore::with_validator_service` | `validation_queue.rs:209` | Complete |
| `InstalledMontyCatalogue::bind_validator_service` | `installed_monty_catalogue.rs:201` | Complete |
| `GlobalMontyOwner::client` | `global_monty_owner.rs:167` | Returns `ServiceClient` |
| `GlobalMontyDriver::drive_turn` | `global_monty_driver.rs:568` | Complete; calls `build`, handles `InternalCompleted` |

**What is missing:**
1. A non-test `AdmissionMode` that sets `trusted_internal_turn = true`.
2. A `ValidatorTaskDriver` that wraps `GlobalMontyDriver` to submit validator
   tasks through `drive_turn` without needing an inbound message.
3. Update of `ValidatorServiceBridge` to hold a `GlobalMontyDriver` reference
   instead of a low-level `ports_factory` closure.
4. Update of `run_q1_validation_retained` to use the driver path.

---

## Deferred paths (must remain `Q1Outcome::Deferred`, not errors)

All six paths already in the source are correct:

1. `service`/`ports_factory` (or bridge) is `None` — line 523-527
2. `read_allocation_head(recipe_id)` returns `None` — line 538-547
3. `installed_worker()` fails — line 635-640
4. No row in `reborn_component_review_submissions` — line 675-682
5. `read_validator_subject_in_transaction` returns `None` — line 725-732
6. `capture_q1_candidate` → `ComponentMissing` — line 688-691

---

## Step 1 — Add `AdmissionMode::ValidatorInternal` to `bootstrap_validation.rs`

**File:** `crates/brassclaw_reborn_composition/src/bootstrap_validation.rs`

### What to add

A new variant `ValidatorInternal` on the `AdmissionMode` enum, **without**
`#[cfg(test)]`. This variant:
- Uses `source_binding = "validator-internal"` (a distinct value from
  `"bootstrap-internal"` and `TRUSTED_INTERNAL_SOURCE_BINDING`; verify these
  constants before writing).
- Uses `owner = Some(actor.clone())` (same as `RetainedParent`).
- Uses `project = None` (same as `RetainedParent`).
- Uses `TurnScope::new_with_owner(...)` (same as `RetainedParent`).
- Uses `coordinator.with_run_profile_resolver(...)` (same as `RetainedParent`).
- Uses `MontyTaskHandoff::new_claimed(request, Arc::new(claimed), host)` (same
  as `RetainedParent`).
- Calls `context.with_trusted_internal_turn()` (same as `RetainedParent` — move
  this line out of `#[cfg(test)]`).

**Foreseen problem:** `MontyTaskHandoff::new_claimed` may have constraints — read
its signature before writing. If it requires a live parent, find the correct
`new(...)` variant that does not.

**Foreseen problem:** The `admit` function currently checks
`if matches!(mode, AdmissionMode::Contained) { admission.check_and_start().await }`.
For `ValidatorInternal`, do not call `check_and_start` (internal tasks do not go
through the model-selection check; they bypass it via `trusted_internal_turn`).
This is already the behavior of `RetainedParent`.

**Foreseen problem:** adding a new enum variant causes compile errors at every
exhaustive match on `AdmissionMode` inside `prepare_admission`. Walk all match
arms in `bootstrap_validation.rs` and add `AdmissionMode::ValidatorInternal`
mirrors of the `RetainedParent` arms.

**Add a public non-test function** alongside `admit`:

```rust
/// Reserve a trusted internal validator task admission.
/// Sets `trusted_internal_turn = true` so that `capture_internal` is reached
/// in `InstalledMontyCatalogue`. Must be called after `retain_validator_selection`
/// has succeeded for `submission_id`.
pub(crate) async fn admit_validator_task(
    pool: Arc<PgPool>,
    submission_id: Uuid,
) -> Result<Admitted, RebornBuildError> {
    let reference = crate::pg_review_admission_source::validator_review_reference(submission_id);
    prepare_admission(pool, "validator-internal", &reference, AdmissionMode::ValidatorInternal).await
}
```

The `identity` string `"validator-internal"` becomes the `agent_id` of the
bootstrap namespace thread. Verify this does not conflict with any existing
namespace — all bootstrap threads use the `NAMESPACE = "bootstrap-validation"` as
their `tenant_id`, so agent_id uniqueness is only scoped within that tenant.

---

## Step 2 — Add `ValidatorTaskDriver` struct

**New file:** `crates/brassclaw_reborn_composition/src/validator_task_driver.rs`

### What it does

`ValidatorTaskDriver` wraps the `GlobalMontyDriver` and submits a validator task
through `drive_turn`. Because `drive_turn` takes a `MontyTaskHandoff` (which is
already produced by `admit_validator_task`), this struct just bridges the
`Admitted` result to a `drive_turn` call and collects the outcome.

```rust
pub(crate) struct ValidatorTaskDriver {
    driver: Arc<crate::global_monty_driver::GlobalMontyDriver>,
}

impl ValidatorTaskDriver {
    pub(crate) fn new(driver: Arc<crate::global_monty_driver::GlobalMontyDriver>) -> Self {
        Self { driver }
    }

    /// Submit a validator review task and await its completion.
    ///
    /// Preconditions (caller's responsibility):
    /// - `retain_validator_selection` has succeeded for `submission_id`.
    /// - The live `InstalledMontyCatalogue` will route
    ///   `VALIDATOR_REVIEW_PREFIX + submission_id` to `ReviewAdmissionSource`.
    ///
    /// Returns the `TaskOutcome` on service completion.
    pub(crate) async fn submit(
        &self,
        pool: Arc<brassclaw_pg::PgPool>,
        submission_id: uuid::Uuid,
    ) -> Result<brassclaw_monty_host::service::TaskOutcome, ValidatorDriverError> {
        // Admit through the trusted internal path. This sets trusted_internal_turn=true.
        let admitted = crate::bootstrap_validation::admit_validator_task(pool, submission_id)
            .await
            .map_err(ValidatorDriverError::Admission)?;

        // Drive the task through the live global driver. drive_turn:
        // 1. Creates MontyTaskHost from the handoff.
        // 2. Calls factory.build(host, input) which calls capture_internal.
        // 3. capture_internal returns ReviewAdmissionSource for the prefix.
        // 4. build creates GlobalRecipePorts::for_internal with that source.
        // 5. Submits through the service with real ports.
        // 6. Awaits the receipt; on InternalCompleted, calls verify_receipt
        //    via GlobalRecipePorts::finish -> source.verify_receipt.
        //
        // The handoff is inside admitted.host — extract it via the MontyTaskHost
        // API. Read MontyTaskHost to find the correct method.
        //
        // CAUTION: drive_turn consumes a MontyTaskHandoff, not a MontyTaskHost.
        // The handoff is created before MontyTaskHost::new is called in
        // prepare_admission. We need to either:
        // (a) Return the handoff from admit_validator_task before wrapping it, or
        // (b) Add a method to Admitted that returns the handoff.
        // See "Foreseen problem" below.
        todo!("see foreseen problem below")
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ValidatorDriverError {
    #[error("validator task admission failed: {0}")]
    Admission(crate::bootstrap_validation::RebornBuildError),
    #[error("validator task drive failed")]
    Drive(brassclaw_turns::run_profile::AgentLoopDriverError),
}
```

### Foreseen problem: `drive_turn` requires a `MontyTaskHandoff`, not a `MontyTaskHost`

`drive_turn` in `global_monty_driver.rs:568` takes `handoff: MontyTaskHandoff` and
calls `MontyTaskHost::new(handoff)` internally (line 572). But `admit_validator_task`
(via `prepare_admission`) already calls `MontyTaskHost::new(handoff)` at line 371
of `bootstrap_validation.rs`. By the time `Admitted` is returned, the `MontyTaskHost`
is already created and the handoff is consumed.

**Resolution:** change `admit_validator_task` to return the `MontyTaskHandoff`
directly, not wrap it in `Admitted`. Or, add an `admit_validator_handoff` function
variant that stops before `MontyTaskHost::new` and returns the raw handoff.

Read `MontyTaskHandoff` to determine if it is clonable/transferable, and whether
any of the intermediate artifacts (`state`, `attempt`, `PgMontyAdmission`) are
needed after `drive_turn` completes.

**Looking at `bootstrap_validation::settle_closed`:** after `drive_turn` returns,
the driver internally calls `self.settle_attempt` which calls
`self.ports.settle(host, receipt)` — the `OwnedGlobalTaskFactory::settle` method
(line 756 of `global_task_factory.rs`). This means `settle_closed` from
`bootstrap_validation.rs` must NOT be called after `drive_turn` — the factory's
`settle` path owns the admission lifecycle.

**The correct change to `bootstrap_validation.rs`:**

Add a function that returns the handoff before wrapping it in `MontyTaskHost`:

```rust
/// Reserve a trusted internal validator task admission and return the raw handoff.
/// The caller must pass this to `GlobalMontyDriver::drive_turn`.
/// After `drive_turn` returns, no explicit admission cleanup is required —
/// the factory's `settle`/`settle_unsubmitted` path owns the lifecycle.
pub(crate) async fn admit_validator_handoff(
    pool: Arc<PgPool>,
    submission_id: Uuid,
) -> Result<MontyTaskHandoff, RebornBuildError> {
    // prepare_admission returns Admitted { host: Arc<MontyTaskHost>, ... }
    // But we need the raw handoff before MontyTaskHost::new.
    // This requires refactoring prepare_admission to not call MontyTaskHost::new
    // at the end, but instead return the handoff separately.
    // See the refactoring plan in Step 2A below.
    todo!()
}
```

### Step 2A — Refactor `prepare_admission` to expose the raw handoff

**File:** `crates/brassclaw_reborn_composition/src/bootstrap_validation.rs`

The last lines of `prepare_admission` (around line 371) are:
```rust
let host = Arc::new(MontyTaskHost::new(handoff));
// ...
admission.persist_reservation().await.map_err(invalid)?;
if matches!(mode, AdmissionMode::Contained) {
    admission.check_and_start().await.map_err(invalid)?;
}
Ok(Admitted { host, admission, state, attempt, input: json!(...) })
```

**Change:** split `prepare_admission` into:
1. `prepare_admission_core(...) -> Result<(MontyTaskHandoff, AdmissionContext), RebornBuildError>` —
   all the thread/turn/claim/host-factory work, returning the raw handoff plus
   the remaining admission-related data (`pool`, `context`, `attempt`, `original_source`,
   `mode`).
2. `prepare_admission(...) -> Result<Admitted, RebornBuildError>` — calls
   `prepare_admission_core`, then wraps the handoff in `MontyTaskHost::new` and
   constructs `PgMontyAdmission`. Keeps the original public interface intact.
3. A new `AdmissionCore` struct holding the data returned by step 1:

```rust
pub(crate) struct AdmissionCore {
    pub(crate) handoff: MontyTaskHandoff,
    pub(crate) pool: Arc<PgPool>,
    pub(crate) context: LoopRunContext,
    pub(crate) attempt: MontyTaskAttempt,
    pub(crate) original_source: Option<SourceBindingRef>,
    pub(crate) mode_is_contained: bool,
}
```

Then `admit_validator_handoff` calls `prepare_admission_core` and returns only the
`MontyTaskHandoff`. `PgMontyAdmission::prepare` and `persist_reservation` are
handled by `OwnedGlobalTaskFactory::build` when `drive_turn` calls it.

**Foreseen problem:** `PgMontyAdmission::prepare` is called both in
`prepare_admission` (line 377) and in `OwnedGlobalTaskFactory::build` (line 527).
If `prepare_admission_core` does NOT call `persist_reservation`, then `build` will
call it instead (line 566: `admission.persist_reservation().await?`). This is the
correct behavior — `build` owns the admission reservation for production tasks.
For `AdmissionMode::Contained`, `prepare_admission` already calls
`persist_reservation` before returning. For `ValidatorInternal`, skip it in
`prepare_admission_core` and let `build` handle it.

---

## Step 3 — Update `ValidatorServiceBridge` to hold `GlobalMontyDriver`

**File:** `crates/brassclaw_reborn_composition/src/installed_monty_catalogue.rs`

### Changes

**A. Change `ValidatorServiceBridge` struct:**

```rust
/// Injected after `GlobalMontyDriver` is constructed; carries the live driver
/// and pool for the retained Q1 validator path.
/// Set exactly once via `bind_validator_service`; never replaced.
pub(crate) struct ValidatorServiceBridge {
    pub(crate) driver: Arc<crate::validator_task_driver::ValidatorTaskDriver>,
    pub(crate) pool: Arc<brassclaw_pg::PgPool>,
}
```

Remove `service: ServiceClient` and `ports_factory`. The `ServiceClient` is no
longer needed directly — it is owned by `GlobalMontyDriver`. The `pool` is needed
by `admit_validator_handoff`.

**B. Remove `make_validator_ports_factory` entirely.** It is the PKG-B placeholder
and must not survive.

**C. Update `bind_validator_service`:**

```rust
pub(crate) fn bind_validator_service(
    &self,
    driver: Arc<crate::validator_task_driver::ValidatorTaskDriver>,
    pool: Arc<brassclaw_pg::PgPool>,
) -> Result<(), AgentLoopDriverError> {
    self.validator_service
        .set(ValidatorServiceBridge { driver, pool })
        .map_err(|_| failed())
}
```

**D. Update `validator_bridge`:**

```rust
pub(crate) fn validator_bridge(&self) -> Option<std::sync::Arc<ValidatorServiceBridge>> {
    self.validator_service.get().map(|b| {
        std::sync::Arc::new(ValidatorServiceBridge {
            driver: Arc::clone(&b.driver),
            pool: Arc::clone(&b.pool),
        })
    })
}
```

**E. Update `runtime.rs:3126`:**

```rust
#[cfg(not(test))]
{
    let validator_driver = Arc::new(
        crate::validator_task_driver::ValidatorTaskDriver::new(driver.clone())
    );
    let _ = catalogue.bind_validator_service(
        validator_driver,
        Arc::clone(&pool),   // the pool variable already in scope in runtime.rs
    );
}
```

`driver` here is the `Arc<GlobalMontyDriver>` already constructed at line 3109.
`pool` is the `Arc<PgPool>` already present in the surrounding `runtime.rs` context.

---

## Step 4 — Update `ValidationQueueStore` callers

**File:** `crates/brassclaw_reborn_composition/src/validation_queue.rs`

The external change has already consolidated `submit()` to call
`fire_q1_kick_if_wired`. The `fire_q1_kick_if_wired` method (lines 225-251)
currently reads `bridge.service` and `bridge.ports_factory` directly (lines
228-229). After Step 3, neither field exists. Update it:

```rust
pub(crate) fn fire_q1_kick_if_wired(
    &self,
    scope: &ComponentScope,
    component_id: Uuid,
    component_class: i32,
) {
    if let Some(bridge) = &self.validator_bridge {
        let pool = Arc::clone(&self.pool);
        let bridge = Arc::clone(bridge);
        let scope = scope.clone();
        let queue_store =
            Self::with_reject_threshold(Arc::clone(&self.pool), self.reject_threshold);
        let preparation = brassclaw_monty_host::utility::UtilityLimits::from_execution(
            brassclaw_host_api::MontyExecutionLimits::default(),
        )
        .expect("default MontyExecutionLimits are always valid");
        tokio::spawn(async move {
            let _ = crate::q1_orchestrator::run_q1_validation_retained(
                &pool,
                &scope,
                component_id,
                component_class,
                &queue_store,
                preparation,
                Some(bridge.as_ref()),
            )
            .await;
        });
    }
}
```

---

## Step 5 — Update `run_q1_validation_retained` signature and body

**File:** `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`

### A. New signature

Replace the raw `service` and `ports_factory` parameters with a single `bridge`:

```rust
pub async fn run_q1_validation_retained(
    pool: &PgPool,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i32,
    queue_store: &ValidationQueueStore,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    bridge: Option<&crate::installed_monty_catalogue::ValidatorServiceBridge>,
) -> Result<Q1Outcome, Q1Error> {
```

### B. Step 1 check

```rust
let bridge = match bridge {
    Some(b) => b,
    None => return Ok(Q1Outcome::deferred("retained path not yet wired")),
};
```

### C. Replace Steps 6–8

Current steps 6–8 call `retain_validator_selection`, `submit_validator_review`,
and `ticket.wait()` separately. The new path:

```rust
// Step 6 — retain the sealed validator selection (idempotent).
crate::pg_validator_q1_admission::retain_validator_selection(pool, &subject, &inspected)
    .await
    .map_err(|e| Q1Error::Admission(e.to_string()))?;

// Step 7 — submit through the trusted internal drive_turn path.
// drive_turn calls build -> capture_internal -> GlobalRecipePorts::for_internal.
// On InternalCompleted, verify_receipt persists the Q1 evidence.
let outcome = bridge
    .driver
    .submit(Arc::new(pool.clone()), submission_id)
    .await
    .map_err(|e| Q1Error::Admission(e.to_string()))?;
```

### D. Outcome translation (replace lines 766-787)

`ValidatorTaskDriver::submit` returns `TaskOutcome` directly (not a `TaskReceipt`).
Update the match:

```rust
use brassclaw_monty_host::service::TaskOutcome;
match outcome {
    TaskOutcome::InternalCompleted { .. } => {
        queue_store.gate1_pass_reviewed(scope, &review, &[]).await?;
        Ok(Q1Outcome::Passed)
    }
    TaskOutcome::Failed { reason_kind } => {
        let errors = vec![format!("validator task failed: {reason_kind}")];
        queue_store.gate1_fail_reviewed(scope, &review, &errors).await?;
        Ok(Q1Outcome::Failed { errors })
    }
    TaskOutcome::Completed { .. } => {
        let errors = vec!["retained validator returned unexpected Completed outcome".into()];
        queue_store.gate1_fail_reviewed(scope, &review, &errors).await?;
        Ok(Q1Outcome::Failed { errors })
    }
}
```

**Remove** the `receipt.outcome` variable and the `tracing::debug!` that logs it —
or adapt it to log `outcome` directly.

---

## Step 6 — Integration test

**New file:** `crates/brassclaw_reborn_composition/src/runtime/tests/validator_retained_q1_tests.rs`

### Constraints

- No stubs, no mocked outcomes.
- Must use a real `InstalledMontyCatalogue`, real `GlobalMontyOwner`, real DB.
- Must assert a real Q1 evidence row in `reborn_validator_q1_evidence`.

### Pattern

Follow `runtime/tests/python_structure_review_tests.rs` for the runtime fixture.
Use `build_reborn_runtime` to start a complete runtime.

**Note:** in test builds, `bind_validator_service` is inside `#[cfg(not(test))]`.
The test must wire the bridge manually using the test-accessible `driver` and
`ports` from `monty_test_controls`, which are already exposed in the test path.

Read `runtime.rs` around line 3119 to find what `monty_test_controls` exposes and
how to access the driver in tests. Verify whether a test-path version of
`bind_validator_service` needs to be added to `InstalledMontyCatalogue` or if an
alternative wiring exists.

### Test case: end-to-end pass

1. Start runtime with `build_reborn_runtime`.
2. Seed validator drafts with `retain_validator_check_drafts`.
3. Create a minimal class-22 component (non-empty name/description/content).
4. Submit to review via `PgReviewSubmissionStore`.
5. Submit to validation queue via `ValidationQueueStore::submit`.
6. Wire the bridge in the test (or trigger it via `fire_q1_kick_if_wired` with a
   manually constructed bridge using the test-accessible driver).
7. Call `run_q1_validation_retained` directly.
8. Assert `Q1Outcome::Passed`.
9. Query `reborn_validator_q1_evidence` — assert row exists.
10. Query `reborn_component_workflow_reviews` — assert row exists.

### Test cases: deferred paths

Test with `bridge: None` → `Q1Outcome::Deferred("retained path not yet wired")`.
Test with real bridge but no submission row → `Q1Outcome::Deferred("no active submission...")`.

### Test case: NotFound is an error

Create a submission but do not call `ValidationQueueStore::submit` → assert
`Err(Q1Error::Queue(NotFound { ... }))`.

---

## Step 7 — Remove PKG-B `#[allow(dead_code)]` annotations

After the integration test passes:

- `pg_validator_q1_evidence.rs` — remove markers `// wired by validator Step 12 durable admission path`
- `pg_workflow_review.rs` — remove markers `// wired by validator Step 6/9 admission path`
  for `WorkflowReview::structural` and `persist_workflow_review` only; leave
  behavior-path items marked until Step 9 behavior path is exercised.
- `pg_validator_q1_admission.rs` — remove `// wired by PKG-C ValidationQueueStore integration`
  from `retain_validator_selection`, `submit_validator_review`, `Q1AdmissionError`.
- `installed_monty_catalogue.rs` — remove `// fields read by PKG-C validator dispatch; PKG-B wires only`
  from `ValidatorServiceBridge`.

---

## Step 8 — Lint and validation

```bash
# Mandatory disk space check (AGENTS.md)
df -h /Users/ollama/brassclaw-target

# Format
cargo fmt --all -- --check

# Lint affected packages
cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings
cargo clippy -p brassclaw_skills --all-targets -- -D warnings
cargo clippy -p brassclaw_engine --all-targets -- -D warnings

# Unit tests
cargo test -p brassclaw_reborn_composition q1_orchestrator
cargo test -p brassclaw_reborn_composition validator_task_driver
cargo test -p brassclaw_reborn_composition bootstrap_validation

# Integration test
cargo test -p brassclaw_reborn_composition --features integration validator_retained_q1
```

---

## Execution order

1. **Step 1** — add `AdmissionMode::ValidatorInternal` and `admit_validator_handoff`.
   This is non-breaking; existing tests continue to use `AdmissionMode::Contained`
   and `AdmissionMode::RetainedParent`.
2. **Step 2** — refactor `prepare_admission` to split off `prepare_admission_core`
   (Step 2A), then add `ValidatorTaskDriver`. Compile and run existing tests before
   proceeding.
3. **Step 3** — update `ValidatorServiceBridge`, remove `make_validator_ports_factory`.
4. **Step 4** — update `fire_q1_kick_if_wired`.
5. **Step 5** — update `run_q1_validation_retained` signature and body.
6. **Step 6** — write the integration test.
7. **Step 7** — remove dead_code annotations after the test passes.
8. **Step 8** — lint and validate.

---

## Contract invariants (must hold throughout)

- `Q1Outcome::Deferred` is the only valid return when any precondition is absent.
  Never return `Q1Outcome::Passed` without a confirmed `persist_validator_q1_evidence`
  write inside `verify_receipt`. `InternalCompleted` from `drive_turn` means
  `verify_receipt` ran; it does not mean `persist_validator_q1_evidence` succeeded.
  `verify_receipt` returns `Err` on persistence failure → service emits
  `TaskOutcome::Failed` → only then does `Q1Outcome::Passed` remain unreachable.
- `Q1Outcome::Failed` with a real error list is the correct return for a validator
  that ran and produced a negative result.
- `Err(Q1Error::...)` is only correct for infrastructure failures.
  `NotFound` from `capture_q1_candidate` is a genuine caller error — the
  component has a submission but was never queued. Do not convert it to `Deferred`.
- `make_validator_ports_factory` and `ValidatorDeferredPorts` must be **deleted**,
  not refactored. They must not survive in any form.
- `trusted_internal_turn = true` is required for `capture_internal` to fire
  (line 1147 of `installed_monty_catalogue.rs`). The `ValidatorInternal` admission
  mode is the only supported non-test path to set this flag.
- `GlobalTaskPortsFactory::build` must not be called externally with an already-
  admitted host — it creates a second `PgMontyAdmission` for the same run, which
  will conflict. The validator task's admission is created inside `build` after
  `drive_turn` calls it with the handoff.
- `bootstrap_validation::settle_closed` must NOT be called after `drive_turn`
  completes. The `OwnedGlobalTaskFactory::settle` path owns the entire admission
  lifecycle for tasks submitted through the driver.

---

## Open questions requiring source inspection before writing code

1. **`MontyTaskHandoff::new_claimed` constraints** — does it require a `ClaimedRun`
   with a live DB row? Or is it only a data structure? Read
   `brassclaw_turns::run_profile::MontyTaskHandoff` before writing `admit_validator_handoff`.

2. **`prepare_admission_core` scope** — which of the fields in the current
   `prepare_admission` body (threads, state, coordinator, context, attempt,
   original_source) are needed by `OwnedGlobalTaskFactory::build`? The factory
   creates its own `PgMontyAdmission::prepare(pool, context, attempt, original_source)`.
   This means `context`, `attempt`, and `original_source` must be available from
   the handoff or from `MontyTaskHost::run_context()` when `build` is called.
   Confirm that `MontyTaskHost::run_context()` exposes all three of these.

3. **`drive_turn` `InternalCompleted` checks** — line 722 of `global_monty_driver.rs`
   checks `host.is_result_only_completion()`. This is true only when
   `trusted_internal_turn = true` was set in the `RunContext` at admission. Confirm
   that `ValidatorInternal` mode sets this flag before `drive_turn` is called.

4. **Test wiring for `bind_validator_service`** — `#[cfg(not(test))]` at
   `runtime.rs:3123` means the bridge is never wired in tests. The integration test
   must either call `bind_validator_service` directly (since `InstalledMontyCatalogue`
   is accessible in the test via `monty_test_controls`) or use
   `fire_q1_kick_if_wired` with a manually constructed bridge. Determine which
   approach is cleaner before writing the test.
