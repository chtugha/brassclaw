# Orchestrator INBOX

Messages from sub-tasks to the orchestrator. **Append only — never delete entries.**

---

## MSG-001 [TASK-001] BUILD_REQUEST 2026-11-01T08:30:00Z

CRATE: brassclaw_reborn_composition
FEATURES: postgres,skills-db
TEST_FILTER: run_q1_validation_retained_defers
REASON: PKG-B ValidatorServiceBridge wiring — OnceLock injection + deferred test


## MSG-002 [TASK-001] DONE 2026-11-01T08:14:00Z

PKG-B implementation complete. All three files modified as specified:

**`installed_monty_catalogue.rs`**
- Added `ValidatorServiceBridge` struct (with `#[allow(dead_code)]` for PKG-B) carrying `ServiceClient` + `ports_factory: Arc<dyn Fn() -> Arc<dyn TaskPorts>>`
- Added `validator_service: OnceLock<ValidatorServiceBridge>` field to `InstalledMontyCatalogue` struct, initialised in constructor
- Added `bind_validator_service` method on `InstalledMontyCatalogue` (OnceLock-once contract, returns `Err` on second call)
- Added `make_validator_ports_factory` helper that wraps a `GlobalTaskPortsFactory` in a PKG-B deferred proxy (`ValidatorDeferredPorts`) returning `Err` on every call — the real dispatch wires in PKG-C

**`runtime.rs`**
- Added `#[cfg(not(test))]` block immediately after the `#[cfg(test)] { monty_test_controls = ... }` block, calling `catalogue.bind_validator_service(owner.client(), make_validator_ports_factory(ports))` — already inside the `#[cfg(all(feature = "postgres", feature = "skills-db"))]` scope

**`validation_queue.rs`**
- Added test `run_q1_validation_retained_defers_when_no_service_wired` (gated `#[cfg(feature = "skills-db")]`) calling `run_q1_validation_retained` with `None, None` and asserting `Q1Outcome::Deferred`

**`tests/monty_control/tests/support/q1_review_fixture.rs`** (pre-existing clippy::duplicate_mod issue fixed in same pass)
- Added `#[allow(clippy::duplicate_mod)]` on both `#[path]` declarations for `pg_retained_q1` and `pg_workflow_review`

BUILD: `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db --all-targets -- -D warnings` → clean (0 errors, 0 warnings)

