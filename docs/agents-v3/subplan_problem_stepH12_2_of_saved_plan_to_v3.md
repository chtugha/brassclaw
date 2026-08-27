# Subplan — H.12.2 production `EffectExecutor` adapter (nested under H.12)

Parent: `./subplan_problem_stepH12_of_saved_plan_to_v3.md` step H.12.2.
Root plan: `./saved_plan_to_v3.md` (v3 Phase H.12).

## Origin / why this nested subplan exists

H.12.2 builds the first **production** impl of the engine trait
`brassclaw_engine::traits::EffectExecutor` (traits/effect.rs:122) so the Monty
VM's `__execute_action__(name, params)` builtin — driven by
`execute_tier_zero_channel` (orchestrator.rs:3272) → `execute_code` →
`handle_execute_action` (orchestrator.rs:1254) → `effects.execute_action`
(orchestrator.rs:2034) — reaches the **production** Rust capability layer
(Q-H12-3: every PythonVM calls tools via the Rust execution layer). Today only
`#[cfg(test)]` impls exist (`MockEffects`/`NoopEffects`); no production impl
exists anywhere. Grounding (research subagent 2026-08-27) confirmed this is a
genuine multi-step build with 8 risk points, so per the task rules it gets its
own nested subplan rather than being stubbed or batched.

## Grounded facts (research 2026-08-27)

### Engine `EffectExecutor` contract (traits/effect.rs:121-167)

```rust
async fn execute_action(&self, action_name: &str, parameters: serde_json::Value,
    lease: &CapabilityLease, context: &ThreadExecutionContext) -> Result<ActionResult, EngineError>;
async fn available_actions(&self, leases: &[CapabilityLease],
    context: &ThreadExecutionContext) -> Result<Vec<ActionDef>, EngineError>;
async fn available_capabilities(&self, leases: &[CapabilityLease],
    context: &ThreadExecutionContext) -> Result<Vec<CapabilitySummary>, EngineError>;
// available_action_inventory has a correct default impl wrapping available_actions.
```

- `ActionResult` (types/step.rs:119): `{ call_id: String, action_name: String,
  output: Value, is_error: bool, duration: Duration }`. Tool-level failure =
  `Ok(ActionResult { is_error: true, output: safe_error_json })`. Infrastructure
  / auth / gate failures = `Err(EngineError)`.
- `ActionDef` (types/capability.rs:129): `{ name, description, parameters_schema,
  effects: Vec<EffectType>, requires_approval, model_tool_surface: FullSchema
  default, discovery: Option<_> }`. `ActionDef::matches_name` (capability.rs:236)
  does exact + discovery + hyphen/underscore normalization.
- `CapabilitySummary` (types/capability.rs:295): `{ name, display_name?, kind,
  status, description?, action_preview: Vec default, routing_hint? }`.
- `CapabilityLease` (types/capability.rs:376): `{ id, thread_id, capability_name,
  granted_actions, granted_at, expires_at?, max_uses?, uses_remaining?, revoked,
  revoked_reason? }`. Methods: `is_valid()`, `covers_action(name)`,
  `consume_use()`, `refund_use()`. **The engine already atomically consumed one
  use before calling `execute_action` (orchestrator.rs:1479-1512
  `LeaseManager::find_and_consume`); the adapter MUST NOT consume again.**
- `ThreadExecutionContext` (traits/effect.rs:18): has `thread_id`, `project_id`,
  `user_id`, `current_call_id: Option<String>`, `conversation_id`, etc. **Lacks
  tenant_id, agent_id, grants, mounts, trust, resource_scope** → adapter must
  source those from composition config.
- `EngineError` (types/error.rs:74): relevant variants `Effect { reason }`,
  `LeaseExpired { capability_name }`, `LeaseDenied { reason }`,
  `AccessDenied { user_id, entity }`, `GatePaused { ... }` (NOT used under
  choice A — see decision below).

### Engine call path (orchestrator.rs)

`__execute_action__(name, params, call_id="")` (builtin at :674) →
`handle_execute_action` (:1254): extracts `name: String`, `params: Value`
(default `{}`), `call_id`; builds `ThreadExecutionContext` (:1281); loads
`available_action_inventory` from `effects.available_action_inventory` (:1287);
matches `name` against `ActionDef`s (:1319); finds+consumes lease (:1479);
runs engine `PolicyEngine` (:1380 → Allow/RequireApproval/Deny); calls
`execute_single_action_with_inline_retry` (:1514) → `effects.execute_action`.
The Monty-facing return uses `ActionResult.output` + `is_error` only.

### Production dispatch — `HostRuntime` façade (the adapter's dispatch target)

`brassclaw_host_runtime::HostRuntime` (lib.rs:873):
```rust
async fn invoke_capability(RuntimeCapabilityRequest) -> Result<RuntimeCapabilityOutcome, HostRuntimeError>;
async fn visible_capabilities(VisibleCapabilityRequest) -> Result<VisibleCapabilitySurface, HostRuntimeError>;
```
- `RuntimeCapabilityRequest::new(context, capability_id, estimate, input, trust_decision)` (lib.rs:348). `estimate` + `idempotency_key` advisory; `trust_decision` transitional (DefaultHostRuntime re-resolves trust) — must NOT widen authority.
- `RuntimeCapabilityOutcome` (lib.rs:541): `Completed(Box<RuntimeCapabilityCompleted{capability_id,output,display_preview?,usage}>)` | `ApprovalRequired(RuntimeApprovalGate{approval_request_id,capability_id,reason})` | `AuthRequired(RuntimeAuthGate)` | `ResourceBlocked(RuntimeResourceGate)` | `SpawnedProcess(RuntimeProcessHandle)` | `Failed(RuntimeCapabilityFailure{capability_id,kind,message?})` | `Unknown(RuntimeCapabilityUnknown)`.
- `VisibleCapabilityRequest::new(context, surface_kind).with_provider_trust(map).with_policy(CapabilitySurfacePolicy)` (lib.rs:438). `VisibleCapabilitySurface { version, capabilities: Vec<{descriptor:{id:CapabilityId,provider,runtime,description,parameters_schema,effects}, estimated_resources}> }`.
- **Do NOT call the lower-level `CapabilityDispatcher::dispatch_json` (host_api/dispatch.rs:364) directly** — it bypasses `CapabilityHost` authorization/approval/obligations (host.rs:438). `HostRuntime::invoke_capability` is the right level.

### `CapabilityId` (host_api/ids.rs:214)

`pub struct CapabilityId(String)`; `CapabilityId::new(s)` validates the form
`<extension>.<capability>[.<sub>...]` (must contain `.`, no empty segments,
validated name segments); `as_str()`. → the engine `action_name` passed to
`__execute_action__` MUST be a valid `CapabilityId` string (e.g.
`"memory.write"`). The action registry validates this.

### `ExecutionContext` (host_api/scope.rs:33) + local-dev template

`ExecutionContext::local_default(user_id, extension_id, runtime, trust, grants, mounts)` (scope.rs:61) then override `tenant_id`/`agent_id`/`project_id`/`thread_id` + the matching `resource_scope.*` fields + `validate()`. The production template is `local_dev_visible_capability_request` (runtime/local_dev.rs:732-798):
- `extension_id = loop_driver_execution_extension_id(run_context)`
- `grants = policy.builtin_grants(extension_id, workspace_mounts, skill_mounts, memory_mounts)` + `extension_surface.grants(extension_id)`
- `user_id` from `run_context.scope.explicit_owner_user_id()` / `actor` / `fallback_user_id`
- `ExecutionContext::local_default(user_id, extension_id, RuntimeKind::FirstParty, TrustClass::UserTrusted, grants, MountView::default())` then set tenant/agent/project/thread from `run_context.scope` + resource_scope + validate
- `provider_trust` map: builtin provider → `TrustDecision{user_trusted, authority_ceiling: policy.provider.authority_effects, AdminConfig}` + extension_surface.provider_trust
- `HostVisibleCapabilityRequest::new(context, SurfaceKind::new("agent_loop")).with_policy(allow_all).with_provider_trust(...)`

**Adapter difference:** the adapter receives an engine `ThreadExecutionContext`
(NOT a `LoopRunContext`), so it sources `user_id`/`project_id`/`thread_id` from
the engine context and `tenant_id`/`agent_id`/`extension_id`/`grants`/`mounts`/
`policy`/`provider_trust` from composition config held at adapter construction.

### Composition wiring site (runtime.rs:2401-2468 / local_dev.rs:59-118)

`LocalDevCapabilityWiring` holds `capability_factory: Arc<dyn LoopCapabilityPortFactory>` whose factory (`LocalDevLoopCapabilityPortFactory`, local_dev.rs:68-118) holds `runtime: Arc<dyn HostRuntime>` — **the same `Arc<dyn HostRuntime>` the adapter must use**, plus `fallback_user_id`, `policy: Arc<LocalDevCapabilityPolicy>`, `workspace_mounts`/`skill_mounts`/`memory_mounts`, `extension_surface_source`, `input_resolver`, `result_writer`, `milestone_sink`, `skill_activation_source`. The adapter is constructed in composition from these same inputs.

## User decision (locked 2026-08-27)

**Q-H12-2-GATE (production approval/auth/resource gates):** choice **A — interim
non-resumable.** `RuntimeCapabilityOutcome::{ApprovalRequired,AuthRequired,
ResourceBlocked}` → `Err(EngineError::Effect { reason: <safe summary> })` so

> **Grounded correction to H.12.2.5 (locked 2026-08-27, decisions Q-H12-2-BUILD
> + Q-H12-2-SNAP).** `loop_driver_execution_extension_id` **requires**
> `&LoopRunContext` (reads `run_context.loop_driver_id`,
> capability_port.rs:1542) — there is NO profile-level default `extension_id` at
> `runtime.rs` build time, and `TurnScope.tenant_id`/`agent_id` are likewise
> per-run (`brassclaw_turns::scope::TurnScope`). So the `EffectExecutor`
> **cannot** be constructed once at `runtime.rs` build (the H.12.2.5 "like
> `retrieval_lookup`" wording is superseded). Corrected design (Q-H12-2-BUILD =
> **A**): `runtime.rs` builds a long-lived `TierZeroEffectExecutorBuilder`
> (holds `Arc<dyn HostRuntime>` + `fallback_user_id` + `Arc<LocalDevCapabilityPolicy>`
> + workspace/skill/memory_mounts + `LocalDevExtensionSurfaceSource`); the
> composition `OrchestratorLookup::run_tier_zero` impl calls
> `builder.build_for_run(run_context).await` per Tier-0 turn → a fresh
> `Arc<dyn EffectExecutor>` carrying the per-run `extension_id`/`tenant_id`/
> `agent_id` + a snapshotted extension surface — mirroring
> `LocalDevLoopCapabilityPortFactory::create_capability_port`'s per-run
> snapshot. Q-H12-2-SNAP = **A**: the extension surface is snapshotted once per
> `run_tier_zero` and **held** in the per-run executor/factory (sync `build()`),
> matching the local-dev precedent (NOT per-`__execute_action__`). Split to
> keep crate boundaries clean with **no `pub(super)` widening**: the
> `TierZeroExecutionContextFactory` + `ProductionEffectExecutor` live at crate
> root (`orchestrator_effect_executor.rs`, only pub types); the
> `TierZeroEffectExecutorBuilder` lives in `runtime/local_dev.rs` (where the
> `pub(super)` surface + `pub(crate)` policy are visible) and returns the trait
> object. Additional grounding: production `LoopCapabilityPort` **rejects**
> `context.mounts != MountView::default()` as `Unauthorized`
> (capability_port.rs:796) → the factory MUST pass `MountView::default()` to
> `ExecutionContext::local_default` (mounts reach capabilities via the grant
> constraints that `LocalDevCapabilityPolicy::builtin_grants` already bakes, not
> via `context.mounts`). Engine→host_api id bridge: engine `ThreadId(pub Uuid)` /
> `ProjectId(pub Uuid)` → `host_api::ThreadId::new(uuid.to_string())` /
> `ProjectId::new(...)` (uuid strings satisfy `validate_scope_id`); engine
> `user_id: String` → `UserId::new(...)` **fail-closed** on validation failure
> (`EngineError::Effect` → Tier-2 degrade) — the engine context always carries
> a `user_id` (`String`, not `Option`), so an invalid value is corruption, not a
> missing user, and must NOT be misattributed to a fallback user. (`validate_scope_id`
> only forbids empty / `>256B` / `.` / `..` / path separators `/` `\` / NUL-control
> chars — uppercase + spaces are allowed, so realistic user ids pass.) The
> factory therefore does not hold `fallback_user_id`; the local-dev
> `fallback_user_id` covers the `Option<UserId>` missing-user case that the
> engine `String` never presents.

`RuntimeCapabilityOutcome::{ApprovalRequired,AuthRequired,
ResourceBlocked}` → `Err(EngineError::Effect { reason: <safe summary> })` so
`execute_tier_zero_channel` degrades to Tier-2 (empty
`TierZeroChannelResult`); the LLM Tier-2 path owns full gate handling. Forced
by Q-H12-2 (activate Monty VM) + `CancellingGateController` (no resumability) +
the H.11 `TierZeroStep::Degrade` model. Full resumable gate-bridging
(production `ApprovalRequestId` → engine `GatePaused`/`ResumeKind`) is a
**future phase**, documented here, NOT stubbed in H.12.2. `SpawnedProcess` →
`Ok(ActionResult { is_error: false, output: json!({ "process": <safe id> }) })`
(least-invasive; engine has no process-handle field). `Failed` →
`Ok(ActionResult { is_error: true, output: json!({"error": safe_summary}) })`.
`Unknown` → `Err(EngineError::Effect)`. `Completed` →
`Ok(ActionResult { is_error: false, output: result.output, .. })`.

## Target architecture

```
Monty __execute_action__(name, params)
  → engine handle_execute_action → effects.execute_action(name, params, lease, ctx)
  → composition ProductionEffectExecutor (new, orchestrator_effect_executor.rs)
      1. validate lease (is_valid + covers_action + thread_id match) — fail closed
      2. resolve action_name → CapabilityId (validated 1:1 registry)
      3. build production ExecutionContext (composition-owned factory:
         engine ctx user_id/project_id/thread_id + held tenant/agent/extension/
         grants/mounts/policy/provider_trust)
      4. RuntimeCapabilityRequest::new(ctx, capability_id, estimate=default,
         input=params, trust_decision=transitional-default)
      5. self.runtime.invoke_capability(request)
      6. map RuntimeCapabilityOutcome → ActionResult (choice A)
  → production HostRuntime → CapabilityHost auth/obligations → RuntimeDispatcher
```

`available_actions` / `available_capabilities` call
`self.runtime.visible_capabilities(VisibleCapabilityRequest)` and project the
descriptors into `ActionDef` / `CapabilitySummary`, filtered by valid engine
leases (is_valid + covers_action + thread_id match).

## Steps (one-by-one, commit+push each; `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` on every build; `df -h` target first — `cargo clean` if Avail<15GB or >90%; selective-pathspec commit guard never staging user WIP)

### H.12.2.1 — Composition: `TierZeroExecutionContextFactory` (production `ExecutionContext` builder)

> ✅ **DONE (commit `2404b0e3`).** Implemented in
> `crates/brassclaw_reborn_composition/src/orchestrator_effect_executor.rs`
> (registered `mod orchestrator_effect_executor;` in `lib.rs`). Holds
> `tenant_id` / `agent_id` / `extension_id` / `grants` (pre-resolved per-run by
> the H.12.2.5 builder); `build(&ThreadExecutionContext) -> Result<ExecutionContext,
> EngineError>` is sync, mirroring `local_dev_visible_capability_request`.
> Engine→host_api id bridge + fail-closed user_id per the grounded correction
> above. 4 unit tests pass (default + `--features skills-db`); clippy clean both
> configs. `#![allow(dead_code)]` until wired in H.12.2.5.

New `crates/brassclaw_reborn_composition/src/orchestrator_effect_executor.rs`
(or a sibling `tier_zero_exec_context.rs`) — a composition-owned factory that
builds a production `ExecutionContext` from an engine `ThreadExecutionContext` +
composition-held config. Holds: `fallback_user_id: UserId`, `tenant_id:
TenantId`, `agent_id: Option<AgentId>`, `policy: Arc<LocalDevCapabilityPolicy>`,
`workspace_mounts/skill_mounts/memory_mounts: MountView`,
`extension_surface_source: LocalDevExtensionSurfaceSource`. Method
`build(&self, engine_ctx: &ThreadExecutionContext) -> Result<ExecutionContext,
EngineError>` mirroring `local_dev_visible_capability_request` (local_dev.rs:757-
774): `extension_id` resolution (ground `loop_driver_execution_extension_id` —
decide what extension_id a Tier-0 recipe channel uses; if run_context is not
available, use the composition's execution-extension id for the runtime
profile), `grants = policy.builtin_grants(...) + extension_surface.grants(...)`,
`ExecutionContext::local_default(engine_ctx.user_id, extension_id,
FirstParty, UserTrusted, grants, MountView::default())` then override
tenant_id/agent_id from held config, project_id/thread_id from engine_ctx,
resource_scope.* likewise, `validate()`. Map `HostApiError` →
`EngineError::Effect`. **Needs:** nothing. **Touches:** new composition module +
`lib.rs` mod. **Result:** adapter can build a production ExecutionContext per
action call. Ground `loop_driver_execution_extension_id` + `LocalDevExtension-
SurfaceSource`/`LocalDevExtensionSurface` (extension_surface.grants /
provider_trust) first; if the extension-surface resolution is large, split a
nested sub-subplan (never stub).

### H.12.2.2 — Composition: action registry (engine `action_name` → `CapabilityId`)

> ✅ **DONE.** Added `pub(crate) trait TierZeroActionResolver: Send + Sync`
> (the seam — `resolve(&self, action_name: &str) -> Result<CapabilityId,
> EngineError>`) + `pub(crate) struct TierZeroActionRegistry` (the default
> 1:1 impl: `CapabilityId::new(action_name)` pass-through, fail-closed →
> `EngineError::Effect` on any invalid `<extension>.<capability>[.<sub>...]`
> name) in `orchestrator_effect_executor.rs`. The executor (H.12.2.3) will
> hold `Arc<dyn TierZeroActionResolver>` so a future non-1:1 resolver can be
> swapped without touching the executor body. 6 registry tests (valid
> 2-segment, valid namespaced, no-dot, empty-segment, empty, uppercase) +
> the 4 factory tests pass in both configs; clippy clean default +
> `--features skills-db`.

In `orchestrator_effect_executor.rs`: a `TierZeroActionRegistry` (or a fn)
that validates `action_name` via `CapabilityId::new(action_name)` (ids.rs:217)
→ returns the `CapabilityId`. First impl is a **validated 1:1 pass-through**
(action_name == capability_id.as_str()); the registry is a seam so a future
non-1:1 mapping (synthetic capabilities, provider-tool-name transforms per
loop_support/capability_port.rs:1081-1085) can be added without re-plumbing.
Invalid action_name → `EngineError::Effect { reason: "invalid capability id
{action_name}" }`. Unit test: valid `"memory.write"` → Ok; invalid `"foo"` →
Err. **Needs:** nothing. **Touches:** the new module. **Result:** `execute_action`
resolves the dispatch target safely.

### H.12.2.3 — Composition: `ProductionEffectExecutor` struct + `execute_action`

`pub(crate) struct ProductionEffectExecutor { runtime: Arc<dyn HostRuntime>,
context_factory: Arc<TierZeroExecutionContextFactory>, }` implementing
`brassclaw_engine::traits::EffectExecutor`. `execute_action`:
1. Validate lease: `lease.is_valid()` && `lease.covers_action(action_name)` &&
   `lease.thread_id == context.thread_id` → else `EngineError::LeaseExpired` /
   `LeaseDenied` / `Effect`. Do NOT consume/refund (engine already consumed).
2. Resolve `action_name` → `CapabilityId` (H.12.2.2).
3. `let exec_ctx = self.context_factory.build(context)?`.
4. `RuntimeCapabilityRequest::new(exec_ctx, capability_id,
   ResourceEstimate::default() (ground the default ctor), parameters,
   TrustDecision::default_for_transitional() (ground — must not widen
   authority; use the same shape local_dev builds at local_dev.rs:781-789))`.
5. `let start = Instant::now(); let outcome = self.runtime.invoke_capability(req).await.map_err(|e| EngineError::Effect { reason: e.safe_summary / to_string })?; let duration = start.elapsed();`
6. Map `RuntimeCapabilityOutcome` per choice A (see User decision). `call_id =
   context.current_call_id.clone().unwrap_or_default()`, `action_name =
   action_name.to_string()`.
**Needs:** H.12.2.1 + H.12.2.2. **Touches:** the new module + `lib.rs` mod.
**Result:** the Monty VM's `__execute_action__` reaches production
`HostRuntime::invoke_capability` (honors Q-H12-3). Ground `ResourceEstimate`
default, `TrustDecision` transitional-default, `RuntimeCapabilityOutcome`
variant field shapes (lib.rs:481-551), `HostRuntimeError` safe-summary access
before writing the body.

### H.12.2.4 — Composition: `available_actions` + `available_capabilities`

`available_actions`: build a `VisibleCapabilityRequest` via the context_factory
(ground the exact request type — `VisibleCapabilityRequest` (host_runtime
lib.rs:419) vs `HostVisibleCapabilityRequest` (local_dev.rs:793); reconcile —
likely the same type re-exported, confirm), call
`self.runtime.visible_capabilities(req)`, project each descriptor → `ActionDef`
{name: capability_id.as_str().to_string(), description, parameters_schema,
effects, requires_approval: <ground — from descriptor.effects/policy or
default false>, model_tool_surface: FullSchema, discovery: None}, filtered by
leases that `is_valid` + `covers_action(name)` + `thread_id == context.thread_id`.
`available_capabilities`: project descriptors → `CapabilitySummary` {name,
display_name: None, kind: <ground CapabilitySummaryKind from runtime/descriptor>,
status: <ground CapabilityStatus — Active for visible>, description, action_preview:
vec![capability_id.as_str().to_string()], routing_hint: None}. Map
`HostRuntimeError` → `EngineError::Effect`. **Needs:** H.12.2.1, H.12.2.3.
**Touches:** the new module. **Result:** the engine Tier-0 surface enumerates
production capabilities. Ground `CapabilitySummaryKind`, `CapabilityStatus`,
`EffectType`, `ModelToolSurface::FullSchema`, the `VisibleCapabilitySurface`
descriptor field names before writing.

### H.12.2.5 — Composition: construct + wire the adapter in `runtime.rs`

At the `LocalDevCapabilityWiring` site (runtime.rs:2401-2468 / local_dev.rs:68-
118), construct `Arc<ProductionEffectExecutor>` from the SAME `Arc<dyn
HostRuntime>` + `fallback_user_id` + `policy` + mounts + `extension_surface_source`
the `LocalDevLoopCapabilityPortFactory` uses; build the
`TierZeroExecutionContextFactory` with the runtime's tenant_id/agent_id (ground
where composition holds the live tenant/agent for the runtime profile — likely
from the run-context scope at call time, but the adapter is constructed once;
decide: hold the factory inputs and resolve tenant/agent per-call from the
engine ctx's conversation/thread, OR hold a profile-default tenant/agent). Expose
the `Arc<dyn EffectExecutor>` to H.12.4 (the `TierZeroOrchestrator` construction).
Add `#[cfg(feature = "skills-db")]` gating consistent with `retrieval_lookup`
(runtime.rs:2550) since the Tier-0 path is skills-db-gated; when the feature is
off the adapter is not constructed (H.12.4 leaves `orchestrator_runtime = None`
→ `NoOrchestrator` → Tier-2 degrade). **Needs:** H.12.2.3, H.12.2.4.
**Touches:** composition runtime.rs (+ local_dev.rs if the wiring inputs live
there). **Result:** a constructed production `EffectExecutor` ready for H.12.4.

### H.12.2.6 — Tests

- `ProductionEffectExecutor::execute_action` dispatch: a mock `HostRuntime`
  (capture `invoke_capability` arg — every field: context.tenant_id/user_id/
  project_id/thread_id, capability_id, input, trust_decision class) → returns
  `Completed` → assert `ActionResult { is_error:false, output, call_id, action_name, duration>0 }`
  and the mock captured the engine `action_name`/`parameters`/thread_id faithfully.
- Lease validation: invalid/expired/wrong-thread lease → `Err(LeaseExpired|
  LeaseDenied|Effect)` and `invoke_capability` NOT called.
- Gate mapping (choice A): mock returns `ApprovalRequired`/`AuthRequired`/
  `ResourceBlocked` → `Err(EngineError::Effect)`; mock returns `Failed` →
  `Ok(ActionResult{is_error:true})`; `Unknown` → `Err(EngineError::Effect)`;
  `SpawnedProcess` → `Ok(ActionResult{is_error:false, output has process})`.
- `available_actions`/`available_capabilities`: mock `visible_capabilities`
  returns 2 descriptors → assert 2 `ActionDef`/`CapabilitySummary` with correct
  projection, filtered by a lease that covers only one action.
- `TierZeroExecutionContextFactory::build`: assert the produced
  `ExecutionContext` carries the engine ctx's user_id/project_id/thread_id +
  held tenant_id/agent_id + valid grants + passes `validate()`.
- Action registry: valid/invalid `CapabilityId` (H.12.2.2).
Mock of `HostRuntime` MUST capture every argument the production caller passes
(AGENTS.md testing rule). **Needs:** H.12.2.3-H.12.2.5. **Touches:** the new
module's `#[cfg(test)]` block. **Result:** regression coverage of the adapter.

### H.12.2.7 — Verify + mark done

`cargo fmt -p brassclaw_reborn_composition`; `cargo clippy -p
brassclaw_reborn_composition --all-targets -- -D warnings` (default +
`--features skills-db`); `cargo test -p brassclaw_reborn_composition --lib`
(default + `--features skills-db`); confirm a composition `cargo check` is clean.
Mark H.12.2 ✅ DONE in this subplan + the parent H.12 subplan doc; commit+push.
Then resume the parent H.12 subplan at H.12.3.

## Notes / risks

- **Q-H12-2-GATE = A** (interim non-resumable gates) is locked. Full resumable
  gate-bridging is a documented future phase, not a stub here.
- **Context factory tenant/agent resolution** (H.12.2.1/H.12.2.5) is the main
  open grounding point — the adapter is constructed once but tenant/agent are
  per-turn. Ground whether to hold profile-defaults or resolve per-call from
  the engine ctx; if large, split a nested sub-subplan.
- **`visible_capabilities` request type** (`VisibleCapabilityRequest` vs
  `HostVisibleCapabilityRequest`) must be reconciled at H.12.2.4.
- **Double authorization** is intentional: engine lease+policy AND production
  `CapabilityHost` grants+policy are independent checks; neither replaces the
  other. The adapter validates the lease (fail-closed) then lets
  `HostRuntime`/`CapabilityHost` enforce production authority.
- **No stubs**: every step implements real functionality. If a grounding step
  reveals a missing production API (e.g. no `ResourceEstimate::default`, no
  transitional `TrustDecision` ctor), add it or use the documented local_dev
  equivalent — do not simulate.
- Keep the diff scoped: do NOT touch user WIP (product_workflow, webui_v2, V063
  basic_prompt_store, prefix-cache, etc.) — selective-pathspec commits only.
