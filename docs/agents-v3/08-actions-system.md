# 08 — Actions System

> **Subsystem:** Actions (class 16) — LLM-free deterministic execution sequences. An Action is
> an ordered list of typed steps (`tool_call`, `conditional`, `loop`, `call_action`, …) that, in
> the v2 design, the orchestrator ran **without an LLM round-trip** when the intent system matched
> it. Actions were the original no-LLM pattern that Phase H generalised to Tier-0 Recipes.
> **Grounded in:** `crates/brassclaw_pg/migrations/V029__reborn_actions.sql`,
> `crates/brassclaw_pg/migrations/V073__reborn_actions_syntax.sql`,
> `crates/brassclaw_engine/src/executor/orchestrator.rs` (`FetchForTurnResult::ActionShortCircuit`),
> `crates/brassclaw_engine/src/memory/retrieval_source.rs` (class-16 projection),
> `saved_plan_to_v3.md` §0.8 / §0.9 / §3.11, C.4.5.6 (F6=A).
> **Status:** **vestigial.** The `reborn_actions` table is live (V029, cleaned by V073) and
> `FetchForTurnResult::ActionShortCircuit` is a shipped variant, but the **step-machine that
> actually executed Action steps was driven by the retired `default.py` orchestrator + the retired
> `__execute_action__` meta-primitive** (removed in C.1). The ActionShortCircuit path is documented
> "vestigial under Q2" — there is **no live no-LLM Action execution path today**. Per C.4.5.6
> (F6=A), the step-machine's retire-vs-reformulate fate is **deferred to C.5/C.6/C.7**; V073 did
> only the DB-structure cleanup that persists regardless of that fate.

## 1. Purpose

An **Action** (class 16) is a reusable, validated component whose `steps` JSONB is an ordered
array of typed step descriptors (`tool_call`, `conditional`, `set_var`, `loop`, `return`,
`evaluate`, `call_skill`, `try_catch`, `parallel`, `call_action`, `spawn_subprocess`, `wait`,
`emit_event` — 13 types, §7 Q13). In the v2 design, when the intent system matched a user message
to an Action, the orchestrator executed those steps deterministically and returned the result
without calling the LLM — the "return before `__llm_complete__`" shape that Phase H generalised
to Tier-0 Recipes.

**Where this stands now.** The v2 executor — `default.py`'s `execute_action_procedure` + the
`__execute_action__` / `__execute_code_step__` / `__execute_actions_parallel__` meta-primitives —
was **retired** (the orchestrator is now Monty; `__execute_action__` was removed in C.1). The
no-LLM deterministic pattern now lives in **Tier-0 Recipes** via the `TierZeroOrchestrator` (the
turns `PgOrchestratorLookup` bridge, active) + the **composition system** (`host.compose_orchestrator`
→ `host.run_program`, wired but VM-dormant pending C.5/C.6), **not** in class-16 Actions. The
`ActionShortCircuit` retrieval variant still exists but is "vestigial under Q2". Per C.4.5.6
(F6=A), the step-machine's retire-vs-reformulate fate is deferred to C.5/C.6/C.7.

Actions default to **Solution Override** (`override_prompt_creation BOOLEAN DEFAULT true`):
when `prior_knowledge_content` is non-NULL it is used verbatim instead of concatenating `steps` +
`description`. In the v2 design this was an **LLM** path (it swapped `working_messages` and fell
through to the LLM); the no-LLM path was the class-16 short-circuit, which is dormant today.

## 2. Location

- **Migration (table):** `crates/brassclaw_pg/migrations/V029__reborn_actions.sql` (class 16).
- **Migration (cleanup):** `crates/brassclaw_pg/migrations/V073__reborn_actions_syntax.sql`
  (C.4.5.6 — drops the 5 legacy lifecycle columns; F6=A DB-cleanup-only).
- **Retrieval:** `crates/brassclaw_engine/src/memory/retrieval_source.rs` — the class-16 arm of
  `fetch_for_turn` / `fetch_component_by_id`; the class-16 SELECT projection reads `(id,
  class_code, prompt_uid, name, description, effective_content, override_prompt_creation, steps,
  allowed_tools)` — it reads **none** of the (now-dropped) legacy queue columns.
- **Short-circuit variant:** `crates/brassclaw_engine/src/executor/orchestrator.rs` —
  `FetchForTurnResult::ActionShortCircuit { component_id, name }` (the no-LLM sibling of
  `SplitResult`); `PkrAssemblyResult::action_short_circuit: bool` (+ `action_component_id` /
  `action_name`), documented "vestigial under Q2".
- **Store:** there is **no `PgAction` / `pg_action_store.rs`**. The only `reborn_actions`
  INSERTs are raw SQL in tests + the `retrieval_lookup_impl.rs` seed (specifying only
  id/scope/name/description/class_code/validation_status/steps/allowed_tools).
- **Plan:** §0.8 (`ActionShortCircuit`), §0.9 (the step-0 problem + v3 single-call solution),
  §3.11 (dispatch flow), C.4.5.6 (F6=A deferral).

## 3. Data model — `reborn_actions` (V029, class 16; cleaned by V073)

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `tenant_id`,`user_id`,`agent_id`,`project_id` | TEXT NOT NULL | scope tuple |
| `name` | TEXT NOT NULL | `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$`, 1–64; unique per scope |
| `description` | TEXT NOT NULL | 1–1024 |
| `steps` | JSONB NOT NULL DEFAULT '[]' | ordered array of the 13 step types |
| `preconditions` | JSONB | optional, evaluated before step execution |
| `error_handling` | JSONB | top-level error-handling policy |
| `timeout_secs` | INT NOT NULL DEFAULT 60 | `CHECK (1..3600)` |
| `allowed_tools` | TEXT[] NOT NULL DEFAULT '{}' | SEC-07 — defence-in-depth |
| `param_schema` / `param_template` | JSONB | structured invocation |
| `prior_knowledge_content` | TEXT | §3.13/§3.14 Solution Override; used verbatim instead of `steps`+`description` |
| `override_prompt_creation` | BOOL NOT NULL DEFAULT **true** | **Actions default to Solution Override** (unlike Recipes, whose default is `false`) |
| `class_code` | SMALLINT NOT NULL DEFAULT 16 | `CHECK = 16` |
| `prompt_uid` | BIGINT | sequence → deterministic assembly order |
| `consumer_tags` | TEXT[] NOT NULL DEFAULT '{}' | default `{01:monty, 02:orchestrator}` + `05:validator` until validated |
| `intent_examples` | JSONB NOT NULL DEFAULT '[]' | **primary activation mechanism** (§3.12) |
| `source` | TEXT NOT NULL DEFAULT 'authored' | `CHECK IN ('authored','extracted','migrated','imported')` — **no `'system'`** (V073 did not widen this CHECK; V066 widened only `reborn_tools`/`reborn_skills`, so system Actions cannot be seeded today) |
| `validation_status` | TEXT NOT NULL DEFAULT 'pending' | the 8-state lifecycle |
| lineage | | `similarity_parent_id`,`replaces_id`,`parent_version`,`content_hash`,`last_audit_at`,`audit_failure_count`(INT) |
| `created_at`,`updated_at` | TIMESTAMPTZ | `set_updated_at()` trigger |

**Dropped.** The five legacy pre-centralisation lifecycle columns — `validation_errors`,
`review_feedback`, `review_attempts`, `rejected_at`, `queue_code` (V029 lines ~93–100) — were
**dropped by V073** (C.4.5.6); their `queue_code` CHECK was auto-dropped with the column.
`parent_mission_id` was dropped workspace-wide by V064. The lifecycle is centralised on
`reborn_validation_queue` (V051), which tracks `state` (1–4) + its own `validation_errors` +
`review_feedback`; the Q2 graduation path only ever sets `validation_status` on this table.

Unique: `(scope, name)`. Indexes: `consumer_tags` GIN, scope+status, scope+class+uid (assembly
order). (No partial validated index in V029 — Actions reach the orchestrator via intent match,
not the capability surface.)

### Hard limits (PERF-18 — compiled-in, not DB-configurable)

- max content size = **256 KB** (Rust validation)
- max step count = **500** (Rust validation)
- max `allowed_tools` = **50** (Rust validation)

### Recursion bounds (SEC-09)

- `call_action` max depth = **5**
- total step budget = **1000** across nesting levels

### The 13 step types (§7 Q13)

`tool_call`, `conditional`, `set_var`, `loop`, `return`, `evaluate`, `call_skill`, `try_catch`,
`parallel`, `call_action`, `spawn_subprocess`, `wait`, `emit_event`.

Two carry extra safety invariants:
- **`spawn_subprocess` (SEC-08):** dispatches **only** through the host-runtime sandboxed
  subprocess path (`brassclaw_host_runtime` `services/process_executor` + `sandbox_process/`,
  backed by `brassclaw_process_sandbox`) — raw `subprocess.Popen` is never used; the host enforces
  capability lease + approval gate + sandbox. `spawn_subprocess` must be in `allowed_tools`.
- **`call_action` (SEC-09):** the depth/budget bounds above; references a nested Action.

> These invariants describe the v2 step-machine. The step-machine's live execution is dormant
> today (retired executor); the invariants are retained as the spec for whatever C.5/C.6/C.7
> decides to retire or reformulate.

## 4. Behavior / flow — vestigial today

### 4.1 The retired v2 executor

The v2 no-LLM executor was `default.py::execute_action_procedure(action_doc, goal, state)`, which
ran `_execute_action_steps` (the per-step dispatch over the 13 types) and returned a
`complete_result` dict **directly** — `run_loop` returned it without calling `__llm_complete__`.
This was the canonical "return before the LLM" pattern that Phase H generalised to Tier-0 Recipes.

**`default.py` is retired** (the orchestrator is now Monty). The `__execute_action__` /
`__execute_code_step__` / `__execute_actions_parallel__` meta-primitives were removed in C.1.
The executor's internal capability-dispatch method (`execute_action` on the executor context /
`EffectBridgeAdapter`) remains as the Rust-side tool-call machinery, but the Python-facing
meta-primitive that drove the Action step-machine is gone. So **no class-16 Action actually
executes its step list in production today.**

### 4.2 `ActionShortCircuit` — shipped but vestigial

`fetch_for_turn` detects `class_code == 16` immediately after `resolve_intent` returns `Match`
and returns `FetchForTurnResult::ActionShortCircuit { component_id, name }`. In
`PkrAssemblyResult` this sets `action_short_circuit: true` (+ `action_component_id` /
`action_name`) and emits empty `orchestrator_content`. The variant is documented **"vestigial
under Q2"** — it carries the match metadata but there is no live executor on the other end. The
scope-guarded LEFT JOIN on `reborn_actions` that populates `component_name` in
`IntentResolution::Match` (all four scope filters, FIND-P6-05) is shipped.

### 4.3 The retire-vs-reformulate decision (C.4.5.6 / F6=A)

Per F6=A, the Action step-machine's fate — retire it entirely (the no-LLM deterministic pattern
now lives in Tier-0 Recipes + the composition system) or reformulate it onto the new
Monty/`host.run_program` model — is **deferred to C.5/C.6/C.7**. It is entangled with the
composition system (C.4.5.17, shipped) and the driver that activates the engine Monty VM
(C.5/C.6, pending). V073 did only the one change that persists regardless: dropping the five dead
legacy columns.

### 4.4 `override_prompt_creation` vs `tier_zero` (the two no-LLM-looking paths)

| Signal | Path | LLM? | Today? |
|--------|------|------|--------|
| `override_prompt_creation: true` | Solution Override (§3.13/§3.14) — `prior_knowledge_content` becomes the user message | **LLM** (v2 fell through to the LLM) | default for Actions; dormant executor |
| `action_short_circuit` | class-16 Action short-circuit | **no LLM** (v2) | vestigial — no live executor |
| `tier_zero` | Tier-0 Recipe orchestrator channel | **no LLM** | **active** via the turns `PgOrchestratorLookup` bridge (engine VM path dormant pending C.5/C.6) |

`tier_zero` is a **dedicated** signal — it does NOT reuse `override_prompt_creation`. The live
no-LLM deterministic path today is Tier-0 Recipes, not class-16 Actions.

## 5. Relations

- **Intent System** (`02`): a `Match` with `class_code == 16` selects an Action; the LEFT JOIN
  populates `component_name` (FIND-P6-05); `ActionShortCircuit` is returned (vestigial).
- **IBS / Retrieval** (`04`/`11`): `ActionShortCircuit` is the no-LLM sibling of `SplitResult` in
  `FetchForTurnResult`; the class-16 retrieval projection reads `steps` + `allowed_tools` + the
  solution-override columns.
- **Recipe System** (`03`): the v2 `execute_action_procedure` "return before the LLM" pattern was
  generalised to Tier-0 Recipes — the live no-LLM deterministic path now lives there
  (`TierZeroOrchestrator` + the composition system).
- **Tools** (`06`): `allowed_tools[]` gates every `tool_call`/`parallel`/`spawn_subprocess` step
  (SEC-07); `spawn_subprocess` dispatches only through the host-runtime sandboxed subprocess path
  (SEC-08) — these invariants are retained as spec.
- **PythonCode** (`07`): a `call_skill` step may reach Skills; Actions themselves are class 16,
  not orchestrator-channel bodies.
- **Validation Queue** (`14`): authored Actions enter Q1/Q2; the five per-table queue columns
  were dropped (V073) — the lifecycle is centralised on `reborn_validation_queue` (V051).
- **Orchestrator** (`13`): the v2 `execute_action_procedure` + step-0 handler lived in
  `default.py` (retired); the orchestrator is now Monty.

## 6. Status — shipped vs. pending

**Shipped:**
- `reborn_actions` (V029) — the table (class 16; 13 step types; hard limits 256 KB / 500 steps /
  50 `allowed_tools`; recursion bounds depth 5 / 1000-step budget; `override_prompt_creation`
  DEFAULT true; `source` CHECK without `'system'`).
- V073 (C.4.5.6) — dropped the five legacy lifecycle columns + `queue_code` CHECK; no paired Rust
  refactor needed (there is no `PgAction` store; the class-16 retrieval projection reads none of
  them).
- `FetchForTurnResult::ActionShortCircuit { component_id, name }` + `PkrAssemblyResult` fields
  (`action_short_circuit` / `action_component_id` / `action_name`) — vestigial under Q2.
- The scope-guarded LEFT JOIN on `reborn_actions` populating `component_name` in
  `IntentResolution::Match` (FIND-P6-05).

**Pending / deferred:**
- **C.5/C.6/C.7 (F6=A):** the Action step-machine's retire-vs-reformulate fate. The v2 executor
  (`default.py::execute_action_procedure` + the `__execute_action__` meta-primitive) is retired;
  either retire the step-machine outright (the no-LLM pattern now lives in Tier-0 Recipes) or
  reformulate it onto the Monty `host.run_program` model. Until then there is **no live no-LLM
  Action execution path**.
- **System seed:** `source` CHECK has no `'system'` (V073 did not widen it; V066 widened only
  tools+skills) — system Actions cannot be seeded today. Adding `'system'` is a future migration
  (the never-written "V057" slot) if Actions are reformulated rather than retired.
- **Phase N:** populate `reborn_validation_queue` from `reborn_actions` (fixed class-code literal
  `16`, FIND-P6-08). The five legacy columns are already dropped (V073), so no DROP is needed
  here.
- **Store:** a `PgAction` / `pg_action_store.rs` does not exist; if Actions are reformulated,
  one would be added (today only raw SQL INSERTs in tests + the seed).

## 7. LLM-relevant summary

An Action (class 16, `reborn_actions` V029) is an LLM-free deterministic step sequence — an
ordered `steps` JSONB of 13 typed step types run, in the v2 design, by
`execute_action_procedure` (retired `default.py`), which returned before `__llm_complete__`.
**That executor is retired** (the orchestrator is now Monty; `__execute_action__` was removed in
C.1), so **no class-16 Action executes its step list in production today**. `ActionShortCircuit
{ component_id, name }` is a shipped `FetchForTurnResult` variant but is "vestigial under Q2"
(no live executor on the other end). The live no-LLM deterministic path now lives in **Tier-0
Recipes** (`TierZeroOrchestrator`, active via the turns `PgOrchestratorLookup` bridge) + the
**composition system** (`host.compose_orchestrator` → `host.run_program`, VM-dormant pending
C.5/C.6). Per C.4.5.6 (F6=A), the step-machine's retire-vs-reformulate fate is **deferred to
C.5/C.6/C.7**; V073 did only the DB cleanup (dropped the five legacy lifecycle columns; no
`PgAction` store exists, no paired Rust refactor). Hard limits 256 KB / 500 steps / 50
`allowed_tools`; recursion depth 5 / 1000-step budget; `spawn_subprocess` via the host-runtime
sandbox only (SEC-08); `allowed_tools` defence-in-depth (SEC-07) — retained as spec. Actions
default to Solution Override (`override_prompt_creation` DEFAULT true, an LLM path);
`source` CHECK has no `'system'` (system Actions cannot be seeded today).
