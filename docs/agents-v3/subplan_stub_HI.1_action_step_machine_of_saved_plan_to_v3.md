# Subplan — HI.1 Action(16) Step-Machine → Composition System Fold + python_code Referential Placeholder↔Include Match

Parent: `saved_plan_to_v3.md` Phase HI (C.4.5 "Common Component Syntax + Composition System"), slice **HI.1** (audit).
Inserted under Zenflow step `5daa7f2c-5065-492e-9987-ee3906ceb5ee` (HI.1, currently `Skipped`).

## Why this subplan exists

The HI.1 audit (verified in code this turn) confirmed the audit *substance* is real (per-class Q1 gates,
migrations V069–V075, the composition system `ComposedProgram`/`compose_program`/`CompositionPort`/
`host.run_program`/`host.compose_orchestrator` all live). But it surfaced two gaps of the
"written half-way then silenced" kind:

### Gap 1 — action(16) step-machine is orphaned post-C.7

The plan **intends actions (class 16) to be executable** (Phase G built the deterministic
`execute_action_procedure` no-LLM step-machine; `DRIVER-GAP` generalizes that no-LLM pattern to Tier-0
recipes). C.7 retired `default.py` (which hosted `execute_action_procedure` + the `action_short_circuit`
step-0 branch) and replaced it with `basic_mode.py` — but `basic_mode.py` has **no action path**:

- `handle_compose_orchestrator` (`orchestrator.rs:1646`) requires a `step_link` (BuildInstruction) —
  actions have none (they use `steps` JSONB) → returns `{ok:false, error:"missing step_link"}`.
- `PersistentMontyDriver::drive_turn_inner` (`persistent_monty_driver.rs:270`) only drives
  `basic_mode.py` — it does NOT call `run_tier_zero`/`assemble_prior_knowledge`, so the
  `ActionShortCircuit`/`action_short_circuit` path is dormant.
- A class-16 intent match → `resolve_intent` returns `Match { class_code=16, step_link=None,
  component_name=<action name> }` → `basic_mode.py` `match` branch →
  `host.compose_orchestrator(action_uuid, step_link="", user_input)` → `{ok:false}` → LLM fallback.
  **Action `steps` never execute.**

Orphaned survivors (no production consumer):
1. `FetchForTurnResult::ActionShortCircuit { component_id, name }` — `retrieval_source.rs:636`
   (dormant `fetch_for_turn` path, not used by `PersistentMontyDriver`).
2. `component_name` field on `IntentResolution::Match` — `intent_system.rs:177-181`; comment says it
   was added "so `ActionShortCircuit` can carry it without a second DB fetch" → orphaned.
3. The LEFT JOIN on `reborn_actions` in `resolve_intent` — `intent_system.rs:378-384`; exists only to
   populate `component_name` for the ActionShortCircuit.
4. `ComponentItem.steps` + `ComponentItem.allowed_tools` fields — `retrieval_source.rs:57,63`.
5. The class-16 `steps`/`allowed_tools` SELECT — `retrieval_source.rs:1161,1238`.
6. Q-G-STUB1 emission in `handle_fetch_component` + `handle_resolve_component_by_name` —
   `orchestrator.rs:1435-1445, 1594-1604` (surfaces `steps`/`allowed_tools` for the retired
   `execute_action_procedure`).
7. `reborn_actions.steps` JSONB 13-step-type model + `allowed_tools` TEXT[] (V029 migration).

### Gap 2 — python_code referential placeholder↔include match deferred comment is misleading

`validate_python_code_placeholders` (`component_validator.rs:484-489`) and
`validate_tool_skill_placeholders` (`component_validator.rs:562-568`) both say
"Referential placeholder<->include matching is deferred to Phase I/N (requires a pool)."
This framing is wrong: Q1 is the wrong place for referential checks entirely, because
`component_validator.rs` is **retired at Phase N** (replaced by orchestrated Q1 in
`q1_orchestrator.rs`). The correct enforcement point is **composition time**: when
`resolver.resolve(uuid)` returns `None` for a declared `includes` UUID, the composition must
fail immediately and the declaring component must be invalidated + re-enqueued.

## Decisions (locked)

### Gap 1 → B1 — Actions ARE recipes: fold through the same IBS pipeline

An action component IS a recipe with a different DB table. The `steps` JSONB carried in
`reborn_actions` IS the `step_descriptions` JSONB — same schema, same IBS input. The correct
fix is:

- **`compose_action_program`** in `PgCompositionPort`: fetch `step_descriptions` (the `steps`
  JSONB column) + `allowed_tools` from `reborn_actions`, then run the **exact same**
  `build_instruction` + `compose_program` pipeline that the recipe path already uses.
- The resulting `ComposedProgram` is one program with `steplist`, `skills`, `rust_directives`,
  `assembled_program` — one `host.run_program` call, one VM, all `scope_vars` shared.
- `handle_compose_orchestrator`: when `step_link` is empty AND the component is class-16,
  dispatch to `compose_action_program` instead of returning `{ok:false, error:"missing step_link"}`.
- All 13 action step types map naturally to IBS `StepEntry` constructs — no separate lowering
  engine is needed. Control-flow steps (conditional, loop, try_catch) are expressed as Python
  in the PythonCode (class 22) includes; tool calls are `Both`-channel steps with `tool_bindings`;
  skills are `include` entries resolved into the `skills` array; nested actions (`call_action`)
  are `Orchestrator`-channel steps whose `include` resolves the nested action UUID → the nested
  action's `assembled_program` is inlined as that step's `executable_code`.
- `type: evaluate` steps are **rejected at Q1** for class-16 actions — not supported in the
  composition model.
- `emit_event`, `wait`, `spawn_subprocess` steps are registered as proper first-party tools
  with ToolSkills, referenced in action `StepDescriptions` as normal `Both`-channel steps.
  They are NOT retired intrinsics.
- `rust_directives` come from `tool_bindings` on `Both`/`Rust` steps, identical to recipe steps.
  Non-builtin tools require a `RustDirective` (resolved via the `ComponentResolver`).

**This overrides the plan's §0.12 statement "Actions ... are executed directly by the orchestrator
without going through the IBS."** — consistent with the architectural steer "The Composition
System actually is the IBS."

**Orphan clean-up:** `FetchForTurnResult::ActionShortCircuit`, `component_name` on
`IntentResolution::Match`, the LEFT JOIN on `reborn_actions` in `resolve_intent`, and the
Q-G-STUB1 `steps`/`allowed_tools` emission in `handle_fetch_component` /
`handle_resolve_component_by_name` are all removed. `ComponentItem.steps` / `.allowed_tools`
fields are removed. Actions now route entirely through `resolve_intent` → `compose_orchestrator`
→ `compose_action_program` → `host.run_program`.

### Gap 2 → Remove the misleading "deferred" comment; enforce at composition time

- **Remove** the "deferred to Phase I/N" language from `validate_python_code_placeholders` and
  `validate_tool_skill_placeholders` in `component_validator.rs`. Replace with a note that
  referential integrity is enforced at composition time (not Q1), and that `component_validator.rs`
  itself is retired at Phase N.
- **Add** a new `ComponentPortError::IncludeNotResolved { component_id, include_id }` variant to
  `composition_port.rs`. `PgCompositionPort::compose_with_pool` returns this error when
  `resolver.resolve(uuid)` returns `None` for any `includes` UUID (the component is absent or
  unvalidated).
- **Add** `ValidationQueueStore::invalidate(scope, component_id, class_code, reason)` to
  `validation_queue.rs`: sets `validation_status = 'pending'` on the component's table row +
  re-inserts into `reborn_validation_queue` (using the existing `submit()` path, which has
  `ON CONFLICT DO NOTHING`), in one transaction. Called by `handle_compose_orchestrator` when
  it receives `IncludeNotResolved`.
- **Update** the two deferred tests at `component_validator.rs:1084-1085, 1212-1231` — their
  assertion comments ("Q1 must not enforce referential placeholder<->include match") remain
  factually correct (Q1 still doesn't enforce it). Only the comment about *why* (the wrong "Phase
  I/N pool" framing) is updated.

## Steps

### A.0 — Record grounding ✅ DONE
Recovered the 13 step types + SEC-09 guards + retired dispatch + orphan inventory. Recorded above.

### A.1 — Design forks ✅ DONE (resolved in conversation, 2026-09-07)

All forks resolved:
- **Fork-A** B1a-i: one program, one `run_program` — actions go through same IBS pipeline as recipes.
- **Fork-B** `parallel` step: serialised in the IBS-generated PythonCode (no new Rust).
- **Fork-C** `call_skill`: resolved by IBS at composition time — skill UUID in `include` of a
  `Both`-channel step; `compose_program` adds it to `skills` array; tool binding goes to
  `rust_directives`.
- **Fork-D** `call_action`: dissolved — nested action UUID in `include` of an `Orchestrator`-channel
  step; resolver returns nested action's `assembled_program` as `executable_code`; inlined flat.
  SEC-09 depth enforced at resolver level.
- **Fork-E** `type: evaluate`: disabled — rejected at Q1 for class-16.
- **Fork-F** `emit_event`/`wait`/`spawn_subprocess`: registered as proper first-party tools,
  referenced as normal `Both`-channel steps in action `StepDescriptions`.
- **Fork-G** `rust_directives`: dissolved into IBS — `tool_bindings` on `Both`/`Rust` steps,
  resolver resolves tool UUID → cdylib path → `RustDirective`. Same as recipe path.
- **Fork-H/I** (Gap 2 pool/strictness): dissolved — referential check is not a Q1 concern at all.
  Enforced at composition time via `IncludeNotResolved` error + `ValidationQueueStore::invalidate`.

### A.2 — Implement `compose_action_program` in `PgCompositionPort`
Fetch from `reborn_actions` (`step_descriptions`/`steps` JSONB + `allowed_tools` + `tier` +
`validation_status`). Run `build_instruction` + `compose_program` (same pipeline as recipes).
Return `ComposedProgram`. Unit tests with a fixture resolver covering the action path.

### A.3 — Relax `handle_compose_orchestrator` for class-16
When `step_link` is empty: fetch the component's `class_code` via the port (or thread it from
`basic_mode.py`'s match dict). If class-16 → dispatch to `compose_action_program`. Otherwise
keep the existing `{ok:false, error:"missing step_link"}`.

### A.4 — Verify `basic_mode.py` match branch
Confirm the `status=="match"` → `_compose_and_run(component_id, step_link, user_input)` path
works for class-16 (step_link="" or None). No `action_short_circuit` branch needed.

### A.5 — Remove orphaned ActionShortCircuit path
- Remove `FetchForTurnResult::ActionShortCircuit` variant + class-16 arm in `fetch_for_turn`
  (`retrieval_source.rs:634-640`).
- Remove `component_name` from `IntentResolution::Match` + LEFT JOIN on `reborn_actions` in
  `resolve_intent` (`intent_system.rs:177-181, 378-384`).
- Remove `ComponentItem.steps` / `.allowed_tools` fields + class-16 SELECT
  (`retrieval_source.rs:57,63,1161,1238`).
- Remove Q-G-STUB1 `steps`/`allowed_tools` emission (`orchestrator.rs:1435-1445, 1594-1604`).
- Update `IntentResolution::Match` consumers (orchestrator.rs:1786-1801; retrieval_source.rs:618-625;
  retrieval_lookup_impl.rs; tests) for the removed `component_name` field.
- Update comments from "for execute_action_procedure" / "Q-G-STUB1" to reflect the new path.

### B — Gap 2: composition-time referential integrity
- Add `ComponentPortError::IncludeNotResolved { component_id, include_id }`.
- `PgCompositionPort::compose_with_pool`: return `IncludeNotResolved` when any include UUID fails
  to resolve.
- Add `ValidationQueueStore::invalidate` (sets `validation_status = 'pending'` + re-submits to
  queue in one transaction).
- `handle_compose_orchestrator`: on `IncludeNotResolved` → call `invalidate` + return
  `{ok:false, error:"include_not_resolved: <uuid>"}`.
- Update the "deferred to Phase I/N" comments in `component_validator.rs` to the correct framing.
- Update deferred test comments (assertions themselves are still correct).

### Final — clippy + tests + commit + push + Zenflow sync
- `cargo clippy -p brassclaw_engine -p brassclaw_reborn_composition --all-targets -- -D warnings`
- `cargo test -p brassclaw_engine -p brassclaw_reborn_composition`
- Commit each A.x / B step individually; push to `origin/main`.
- Flip Zenflow HI.1 step `5daa7f2c-...` from `Skipped` → `Completed`.

## Invariants to preserve

- **ISOLATION:** `host.run_program` fresh-VM-per-call — one `run_program` for the whole action program.
- **No `__execute_action__`** — retired in C.7. Tools are `host.<tool>(kwargs)`.
- **No `.unwrap()`/`.expect()`** in production; `thiserror` + mapped errors; clippy zero warnings.
- **Never edit applied migrations** (refinery checksums). V029 `reborn_actions.steps` stays as-is.
- **`component_validator.rs` is not the right place** for referential checks — it is retired at Phase N.
- **`ValidationQueueStore::invalidate`** must set `validation_status = 'pending'` + re-submit in one
  transaction; use the existing `submit()` `ON CONFLICT DO NOTHING` for idempotency.
- **Scope discipline:** touch only the action-composer + validator comments + orphan-removal sites +
  Gap-2 invalidation path. No unrelated churn.
