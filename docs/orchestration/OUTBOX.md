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
