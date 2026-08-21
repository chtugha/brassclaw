
> **Subsystem:** Actions (class 16) — LLM-free deterministic execution sequences. An Action is
> an ordered list of typed steps (`tool_call`, `conditional`, `loop`, `call_action`, …) that the
> orchestrator runs **without an LLM round-trip** when the intent system matches it. Actions are
> the original no-LLM pattern that Phase H generalises to Tier-0 Recipes.
> **Grounded in:** `crates/brassclaw_pg/migrations/V029__reborn_actions.sql`,
> `crates/brassclaw_engine/orchestrator/default.py` (`execute_action_procedure:901`,
> `_execute_action_steps`, `call_action:839`, step-0 shim `:1010-1028`),
> `saved_plan_to_v3.md` §0.8 / §0.9 / §3.11, Phases D/E/F/G/L/N.
> **Status:** the `reborn_actions` table and `execute_action_procedure` **exist today**, but the
> production short-circuit path is **dead** (the step-0 shim never fires) — the live no-LLM
> execution is only reachable via the v3 `action_short_circuit` wiring (Phase E/F/G), which has
> not landed.

## 1. Purpose

An **Action** (class 16) is a reusable, validated component whose `steps` JSONB is an ordered
array of typed step descriptors (`tool_call`, `conditional`, `set_var`, `loop`, `return`,
`evaluate`, `call_skill`, `try_catch`, `parallel`, `call_action`, `spawn_subprocess`, `wait`,
`emit_event` — 13 types, §7 Q13). When the intent system matches a user message to an Action,
the orchestrator **executes those steps deterministically and returns the result without calling
the LLM**. This is the same "return before `__llm_complete__`" shape that Phase H generalises to
Tier-0 Recipes (`execute_recipe_orchestrator_channel`).

Actions default to **Solution Override** (`override_prompt_creation BOOLEAN DEFAULT true`):
when `prior_knowledge_content` is non-NULL it is used verbatim instead of concatenating
`steps` + `description` — but note that *today* the Solution-Override path is an **LLM** path
(it swaps `working_messages` and falls through to `__llm_complete__`); the no-LLM path is the
class-16 short-circuit, which is currently dead (see §4).

## 2. Location

- **Migration (live):** `crates/brassclaw_pg/migrations/V029__reborn_actions.sql` (class 16).
- **Python executor (live):** `crates/brassclaw_engine/orchestrator/default.py`
  - `execute_action_procedure(action_doc, goal, state)` — `:901`; the no-LLM executor.
  - `_execute_action_steps(...)` — the per-step dispatch loop (the 13 step types).
  - `call_action` branch — `:839` (references nested actions **by name** via `__retrieve_docs__`).
  - step-0 dead shim — `:1010-1028` (`docs = __retrieve_docs__(goal, 5)`; `class_code == 16`).
- **Store (production):** `crates/brassclaw_reborn_composition/src/pg_action_store.rs`.
- **Retrieval (modify):** `crates/brassclaw_engine/src/memory/retrieval_source.rs` — Phase E adds
  `ActionShortCircuit` detection immediately after `resolve_intent` returns `Match` for class 16,
  **before** the `fetch_component_by_id` call (FIND-P5-06).
- **Intent (modify):** `crates/brassclaw_engine/src/memory/intent_system.rs` — Phase D adds
  `component_name` to `IntentResolution::Match` via a scope-guarded LEFT JOIN on `reborn_actions`
  (FIND-P5-06 / FIND-P6-05).
- **Plan:** §0.8 (`ActionShortCircuit`), §0.9 (the step-0 problem + v3 single-call solution),
  §3.11 (dispatch flow), Phase G (step-0 upgrade + `call_action` migration), Phase L (seeder).

## 3. Data model — `reborn_actions` (V029, class 16)

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
| `allowed_tools` | TEXT[] NOT NULL DEFAULT '{}' | SEC-07 — enforced at **both** `default.py` AND `EffectExecutor` (defence-in-depth) |
| `param_schema` / `param_template` | JSONB | structured invocation |
| `prior_knowledge_content` | TEXT | §3.13/§3.14 Solution Override; used verbatim instead of `steps`+`description` |
| `override_prompt_creation` | BOOL NOT NULL DEFAULT **true** | **Actions default to Solution Override** (unlike Recipes, whose default is `false`) |
| `class_code` | SMALLINT NOT NULL DEFAULT 16 | `CHECK = 16` |
| `prompt_uid` | BIGINT | sequence → deterministic assembly order |
| `consumer_tags` | TEXT[] NOT NULL DEFAULT '{}' | default `{01:monty, 02:orchestrator}` + `05:validator` until validated |
| `intent_examples` | JSONB NOT NULL DEFAULT '[]' | **primary activation mechanism** (§3.12) |
| `source` | TEXT NOT NULL DEFAULT 'authored' | `CHECK IN ('authored','extracted','migrated','imported')` — **no `'system'` until V057** (FIND-P6-02) |
| `validation_status` | TEXT NOT NULL DEFAULT 'pending' | the 8-state lifecycle |
| `validation_errors`/`review_feedback`/`review_attempts`(INT)/`rejected_at`/`queue_code` | | the **five per-table queue columns** — centralised to `reborn_validation_queue` in Phase N (V059); `reborn_actions` uses a fixed class-code literal `16` in the populate SQL (FIND-P6-08) |
| lineage | | `similarity_parent_id`,`replaces_id`,`parent_version`,`content_hash`,`last_audit_at`,`audit_failure_count`(INT),`parent_mission_id` |
| `created_at`,`updated_at` | TIMESTAMPTZ | `set_updated_at()` trigger |

Unique: `(scope, name)`. Indexes: `consumer_tags` GIN, scope+status, scope+class+uid (assembly
order), (no partial validated index in V029 — Actions reach the orchestrator via intent match,
not the capability surface).

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
  subprocess path (`__execute_action__("spawn_subprocess")` → `brassclaw_host_runtime`
  `services/process_executor` + `sandbox_process/`, backed by `brassclaw_process_sandbox`) —
  raw `subprocess.Popen` is never used; the host enforces capability lease + approval gate +
  sandbox. `spawn_subprocess` must be in `allowed_tools`. (The v1 "script lane" this once
  named was removed in Phase 4; the SEC-08 guarantee is unchanged.)
- **`call_action` (SEC-09):** the depth/budget bounds above; references a nested Action.

## 4. Behavior / flow

### 4.1 The no-LLM executor — `execute_action_procedure` (default.py:901)

```python
def execute_action_procedure(action_doc, goal, state):
    scope_vars = {"goal": goal}
    step_counter = [0]
    result, _counter = _execute_action_steps(action_doc, scope_vars, 0, step_counter)
    if result is None:
        return complete_result(state, "completed", "Action completed.")
    if "error" in result:
        return complete_result(state, "error", None, error=result["error"])
    return complete_result(state, "completed", result.get("result", ""))
```

It returns a `complete_result` dict **directly**; `run_loop` returns it without calling
`__llm_complete__` (lines 905–906). This is the canonical "return before the LLM" pattern. The
v3 Phase H `tier_zero` branch generalises this from class-16 Actions to Tier-0 Recipes
(`execute_recipe_orchestrator_channel`).

### 4.2 The dead step-0 shim (§0.9 Problem 1) — what runs *today*

Today's `default.py` step 0 makes **three** calls:

```python
pkr        = __assemble_prior_knowledge__(goal, token_budget, "02")   # PRIMARY (works)
docs       = __retrieve_docs__(goal, 5)                               # DEAD SHIM
all_skills = __list_skills__(); select_skills(...)                   # redundant
```

- **Problem 1 — dead Action-detection shim:** `__retrieve_docs__` is the **legacy** function
  (calls `RetrievalEngine::retrieve_context`, the MemoryDoc path). It returns `{type, title,
  content}` with **no `class_code` in the metadata**. The check `metadata.get("class_code")
  == 16` (default.py:1022) therefore **never fires** — the `execute_action_procedure(doc, …)`
  call at `:1025` is unreachable in production. The comment at `:1010` literally labels it
  "Pre-Phase-5 fallback." So **no class-16 Action actually short-circuits in production today**;
  the only pre-`__llm_complete__` return is this dead shim.
- **Problem 2 — redundant skills round-trip:** `__list_skills__()` → `select_skills()` re-selects
  skills by keyword; with a BuildInstruction the IBS already selected exact Skills by UUID.
- **Problem 3 — mixed blob:** `__assemble_prior_knowledge__` returns one merged
  `formatted_content` blob — no rust/orchestrator channel separation.

`override_prompt_creation: true` today (default.py:999–1002) only swaps `working_messages` and
**falls through to `__llm_complete__`** — it is the Solution-Override *LLM* path, NOT a no-LLM
short-circuit (DRIVER-GAP-MODEL-A).

### 4.3 The v3 short-circuit — `action_short_circuit` (Phases D/E/F/G)

The dead shim is replaced by a real, intent-driven short-circuit:

1. **Phase D:** `resolve_intent` is extended with a scope-guarded LEFT JOIN on `reborn_actions`
   to populate `component_name` in `IntentResolution::Match` (so `ActionShortCircuit` carries
   the name without a second DB fetch). The JOIN **MUST** include all four scope filters
   (`a.tenant_id = $1 AND a.user_id = $2 AND a.agent_id = $3 AND a.project_id = $4`) — omitting
   them is a cross-tenant information-leakage bug (FIND-P6-05).
2. **Phase E:** `fetch_for_turn` detects `class_code == 16` **immediately after**
   `resolve_intent` returns `Match`, **before** `fetch_component_by_id`, and returns
   `FetchForTurnResult::ActionShortCircuit { component_id, name }` (the no-LLM sibling of
   `SplitResult`). The existing class-16 path that calls `fetch_component_by_id` is replaced.
3. **Phase F:** the `__fetch_component__(uuid, class_code)` host function is registered (the
   replacement for `__retrieve_docs__`).
4. **Phase G:** step 0 collapses to one call. The new branch:

   ```python
   if pkr.get("action_short_circuit"):
       __emit_event__("action_started", action_name=pkr.get("action_name", ""))
       __transition_to__("running", "action execution")
       action_doc = __fetch_component__(pkr["action_component_id"], 16)
       action_result = execute_action_procedure(action_doc, goal, state)
       __transition_to__("completed", "action completed")
       return action_result
   ```

   > **FIND-P7-02:** do **NOT** create a new `execute_action_by_id` function. The existing
   > `execute_action_procedure` (default.py:901) is the correct executor — Phase G only adds the
   > `__fetch_component__` call + the `action_short_circuit` branch. The §0.9 pseudocode that
   > references `execute_action_by_id` must be read as `execute_action_procedure` with the
   > fetched doc.

### 4.4 `call_action` migration — by-name → by-UUID (Phase G)

The `call_action` step (default.py:839) currently resolves the nested Action **by name** via
`__retrieve_docs__(nested_name, 1)`. Phase G replaces this with `__fetch_component__(uuid, 16)`:

- **Option A (chosen, recommended):** authors add `action_id: UUID` to `call_action` step defs.
  At Phase G deploy, a **data-only** migration (not Flyway) resolves names to UUIDs in place
  (`UPDATE reborn_actions … jsonb_build_object('action_id', (SELECT a2.id … WHERE name=… AND
  scope))`). Unresolvable names leave `action_id` NULL. Runtime: `action_doc =
  __fetch_component__(step_def["action_id"], 16)`.
- **Option B (stop-gap fallback for NULL `action_id`):** `__resolve_component_by_name__(name,
  16)`. Both paths are implemented — Option A failures degrade gracefully (FIND-P7-13 /
  FIND-P9-06).

The plan's earlier statement "UUID sourced from the BuildInstruction step" is **wrong** for
`call_action`: these are *Action* internal steps (class 16), not BuildInstruction steps.

### 4.5 `override_prompt_creation` vs `tier_zero` (the two no-LLM-looking paths)

| Signal | Path | LLM? | Today? |
|--------|------|------|--------|
| `override_prompt_creation: true` | Solution Override (§3.13/§3.14) — `prior_knowledge_content` becomes the user message | **LLM** (falls through to `__llm_complete__`) | works (default.py:999) |
| `action_short_circuit` | class-16 Action short-circuit | **no LLM** | dead shim today; live after Phase E/F/G |
| `tier_zero` (Phase H) | Tier-0 Recipe orchestrator channel | **no LLM** | does not exist; lands in Phase H |

`tier_zero` is a **dedicated** signal — it must NOT reuse `override_prompt_creation` (that is the
Solution-Override LLM path). Both `action_short_circuit` and `tier_zero` return before
`__llm_complete__` via the `execute_action_procedure` pattern.

## 5. Relations

- **Intent System** (`02`): a `Match` with `class_code == 16` selects an Action; Phase D adds
  `component_name`; Phase E returns `ActionShortCircuit`.
- **IBS** (`04`): `ActionShortCircuit` is the no-LLM sibling of `SplitResult` in
  `FetchForTurnResult`.
- **Recipe System** (`03`): `execute_action_procedure` is the no-LLM pattern Phase H generalises
  to Tier-0 Recipes; both use the "return before `__llm_complete__`" shape.
- **Tools** (`06`): `allowed_tools[]` gates every `tool_call`/`parallel`/`spawn_subprocess`
  step (SEC-07 defence-in-depth at `default.py` AND `EffectExecutor`); `spawn_subprocess`
  dispatches only through the host-runtime sandboxed subprocess path (SEC-08).
- **PythonCode** (`07`): a `call_skill` step may reach Skills; Actions themselves are class 16,
  not orchestrator-channel bodies.
- **Validation Queue** (`14`): authored Actions enter Q1/Q2; `source='system'` (Phase L)
  bypasses Q2; the five per-table queue columns move to `reborn_validation_queue` (Phase N).
- **Orchestrator** (`13`): `execute_action_procedure` + the step-0 handler live in `default.py`.

## 6. Status — today vs. v3

**Today:**
- `reborn_actions` (V029) is live; `pg_action_store.rs` exists.
- `execute_action_procedure` (default.py:901) exists and is the correct no-LLM executor.
- **The production short-circuit is dead:** the step-0 `__retrieve_docs__` shim (default.py:1010)
  never fires (`class_code` is absent from MemoryDoc metadata) — no class-16 Action actually
  short-circuits in production.
- `call_action` resolves nested actions **by name** via `__retrieve_docs__` (default.py:844).
- `override_prompt_creation: true` is an **LLM** path (swaps `working_messages`, falls through to
  `__llm_complete__`); it does NOT skip the LLM.
- `source` CHECK has no `'system'` (V057 adds it); the five per-table queue columns are present
  (Phase N centralises them); `__retrieve_docs__` is still registered.

**v3 plan adds:**
- **Phase D:** `component_name` in `IntentResolution::Match` via scope-guarded LEFT JOIN
  `reborn_actions` (FIND-P6-05 security note).
- **Phase E:** `ActionShortCircuit { component_id, name }` — class-16 detection immediately after
  `resolve_intent`, before `fetch_component_by_id` (FIND-P5-06).
- **Phase F:** `__fetch_component__(uuid, class_code)` host function.
- **Phase G:** remove the dead `__retrieve_docs__` shim + the redundant `__list_skills__`/
  `select_skills` round-trip; collapse step 0 to one `__assemble_prior_knowledge__` call; add
  the `action_short_circuit` branch (fetch-by-UUID → `execute_action_procedure`); migrate
  `call_action` to `action_id` UUID (Option A data migration + Option B name-lookup fallback).
  `__retrieve_docs__` registration is removed unconditionally in Phase K (no compat window).
- **Phase L (V057):** the seeder inserts system Actions (`source='system'`, `validated`,
  Q2 bypassed); V057 adds `'system'` to the `source` CHECK.
- **Phase N (V059):** drop the five per-table queue columns; `reborn_actions` uses a fixed
  class-code literal `16` in the populate SQL (FIND-P6-08); re-index its decoder if any earlier
  column is dropped.

## 7. LLM-relevant summary

An Action (class 16, `reborn_actions` V029) is an LLM-free deterministic step sequence — an
ordered `steps` JSONB of 13 typed step types (`tool_call`/`conditional`/`loop`/`call_action`/
`spawn_subprocess`/…) run by `execute_action_procedure` (default.py:901), which returns before
`__llm_complete__`. Hard limits (PERF-18): 256 KB / 500 steps / 50 `allowed_tools`; recursion
bounds (SEC-09): `call_action` depth 5 / 1000-step budget; `spawn_subprocess` dispatches only
via the host-runtime sandboxed subprocess path (SEC-08); `allowed_tools` enforced at both `default.py` and
`EffectExecutor` (SEC-07). Actions default to Solution Override (`override_prompt_creation`
DEFAULT true) — but that is an **LLM** path today (swaps `working_messages`, falls through to
the LLM); the no-LLM path is the class-16 short-circuit, which is **dead** today (the step-0
`__retrieve_docs__` shim never fires — `class_code` is absent from MemoryDoc metadata). The v3
short-circuit is `ActionShortCircuit { component_id, name }` (Phase E, detected before
`fetch_component_by_id`), populated via a scope-guarded LEFT JOIN (Phase D, FIND-P6-05), executed
by the Phase G branch `__fetch_component__(id,16)` → `execute_action_procedure` (FIND-P7-02: no
new `execute_action_by_id`). `call_action` migrates from by-name `__retrieve_docs__` to by-UUID
`__fetch_component__(action_id,16)` (Phase G Option A data migration + Option B name fallback;
FIND-P7-13/P9-06). Phase H generalises this no-LLM pattern to Tier-0 Recipes via `tier_zero` (a
dedicated signal, NOT `override_prompt_creation`). `source` gains `'system'` (V057/Phase L); the
five queue columns centralise to `reborn_validation_queue` (Phase N/V059, literal class 16).
