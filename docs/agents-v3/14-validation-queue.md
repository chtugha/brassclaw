# 14 — Validation Queue System

> **Subsystem:** The pre-validation lifecycle of every agent component —
> the gate that decides whether a newly-authored or edited skill / recipe /
> tool / spec / plan / action / PythonCode / ExtensionCatalogue is allowed
> to reach the prompt and the executor. It has **two halves**: an
> automatic first gate (**Q1**, Gate 1, deterministic structural + injection
> scanning) and a manual second gate (**Q2**, human review). A component
> passes both before it becomes `validation_status = 'validated'` — the
> retrieval gate every consumer (`RetrievalSource`, `do_reassemble`) already
> filters on. Wilson score lower bounds + maturity tiers gate *Tier-0 direct
> execution* on top of validation.
> **Grounded in:** `saved_plan_to_v3.md` §0.18, Phase A.5, Phase N, and the
> shipped code: `crates/brassclaw_engine/src/memory/component_validator.rs`
> (Gate 1 pure logic + the C.4.5.1–5 common-syntax placeholder-grammar gates),
> `crates/brassclaw_reborn_composition/src/validation_queue.rs`
> (`ValidationQueueStore`) + `q1_orchestrator.rs` (`run_q1_validation`),
> `crates/brassclaw_pg/migrations/V031__reborn_validation_config.sql` +
> `V051__reborn_validation_queue.sql` (queue table, shipped) + `V072`/`V073`/
> `V074`/`V075` (drop the 5 legacy columns off skills/actions/memory/extensions),
> `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`
> (`record_outcome`, `is_tier0_eligible`), `crates/brassclaw_agent_loop/src/plan_scoring.rs`
> (`wilson_lower_bound`, `classify_tier`), `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs`.
> Findings `FIND-P9-01/05/08`, `FIND-P6-04/08`, `FIND-P7-11`, `SCHEMA-01`,
> `FINDING D`, `FIND-13`. This doc is **f3 — the Validation-System** and **f8 —
> the per-class new-component-creation validation criteria**.

## 1. Purpose

A component is created (WebUI authoring, MCP import, Sempai proposal, or
builtin seed) in a **pre-validation** state. It must not influence agent
behavior until it has been checked. The validation system answers two
questions, in order:

1. **Is it structurally sound and injection-safe?** (Q1 / Gate 1 —
   automatic, deterministic, no LLM.) A pure-Rust validator checks
   per-class structural rules (name/description/token-budget/tool_name/
   param_schema/activation criteria) and an injection scan. On a clean Q1
   result the row advances to "awaiting manual review".
2. **Is it correct and desirable?** (Q2 — a human reviewer approves or
   rejects.) On Q2 approval the component **graduates**: its
   `validation_status` becomes `'validated'`, its queue row is deleted, and
   it enters retrieval + the prefix cache.

The user's repeat-item framing — *"the different parts of a skill are stored
in the database … queued for validation then"* (Task description item 7,
Sempai-Kohai) — names this queue directly. Sempai proposals, MCP-imported
tools, and WebUI-authored PythonCode all land in the Q1 queue; only after
Q1+Q2 do they become usable in `type: "component"` recipe steps and in the
base-prompt bundle.

Two design invariants are load-bearing:

- **State 2 is a security invariant.** Only the Gate 1 validator function
  may write `state = 2` (Q1 passed). No API endpoint, no application-layer
  code path, no direct SQL may set it. A component that bypasses Q1 and
  reaches `validated` is a security bug.
- **The queue and `validation_status` are two non-overlapping state
  machines.** A component row is *either* in the queue (pre-validation,
  `validation_status != 'validated'`) *or* carrying a post-validation
  `validation_status` (`'validated'` / `'upgrade_queued'`). It is never in
  both. Graduation = the queue row is deleted; from that point the
  component table's `validation_status` is the sole authority.

Wilson scoring is a *third*, orthogonal layer that sits **on top of**
validation: it does not decide whether a component is allowed to run, only
*how confidently* a validated recipe may short-circuit the LLM (Tier 0).
It already exists today (per-recipe) and is not new in v3 — see §6.

## 2. Location

### Shipped

The validation lifecycle **is centralized** on the queue table; the Q1 logic
is a pure function and the cross-crate Q1 sequence has a dedicated owner:

- **Gate 1 logic:** `crates/brassclaw_engine/src/memory/component_validator.rs`
  — `ComponentValidator::validate_by_class(class_code, payload, config,
  available_tools, existing_skill_names)` dispatches to the class-appropriate
  validation path (per-class criteria in §4b). Pure functions, no I/O, no LLM.
  The C.4.5.1–5 common-syntax `{{ ... }}` placeholder-grammar gates
  (`validate_placeholder_grammar`, `validate_python_code_placeholders`,
  `validate_tool_skill_placeholders`, `validate_includes_non_nil_uuids`,
  `check_variant_descriptions`) ride alongside the per-class structural checks.
- **Per-class config:** `reborn_validation_config` (V031) — supplies
  `name_min_len`, `token_budget`, `token_budget_hard_error`,
  `require_tool_name`, etc. `ComponentValidator` reads a `ValidationConfig`
  at call time and falls back to compile-time defaults when the row is
  absent.
- **Queue table (shipped, V051):** `reborn_validation_queue` — DDL + indexes
  (scope+state, scope+class, partial `WHERE state = 4`).
- **Queue store (shipped):** `crates/brassclaw_reborn_composition/src/validation_queue.rs`
  — `ValidationQueueStore` (`submit`, `gate1_pass`/`gate1_fail` `pub(crate)`,
  `approve`, `reject`, `purge_deletion_candidates`, `list`).
- **Q1 orchestration (shipped):** `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`
  — `run_q1_validation(...)` owns the cross-crate call sequence
  (`ComponentValidator::validate_by_class` → `gate1_pass`/`gate1_fail`)
  because `gate1_pass` is `pub(crate)` and the engine cannot call across
  crate boundaries (`FIND-P9-01`).
- **Legacy columns dropped (shipped):** the five pre-validation columns
  (`queue_code`, `review_attempts`, `review_feedback`, `rejected_at`,
  `validation_errors`) were dropped off the component tables across
  `V072` (skills), `V073` (actions), `V074` (memory classes 12/14/17–20),
  `V075` (extensions 4–9). `reborn_python_code` (V052, class 22) and
  `reborn_extension_catalogues` (V053, class 23) were created **without**
  them from day one — they use the queue natively. Each table retains only
  `validation_status` (the post-validation runtime identity).
- **Wilson scoring (live):**
  - `crates/brassclaw_agent_loop/src/plan_scoring.rs` —
    `wilson_lower_bound(successes, failures, z)` and
    `classify_tier(usage_count, w_lower, promotion_threshold)`.
  - `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` —
    `PgRecipeStore::record_outcome` increments `usage_count` +
    `success_count`/`failure_count` in one transaction, recomputes
    `wilson_lower` + `tier`, and writes both back.
  - `reborn_recipes` (V033) carries `tier` (`seedling`/`growing`/`mature`/
    `candidate`), `usage_count`, `success_count`, `failure_count`,
    `wilson_lower`.
- **Tier-0 eligibility (live, incomplete):** `PgRecipe::is_tier0_eligible`
  — checks `is_deliverable() && tier ∈ {mature, candidate}` but **omits the
  `wilson_lower >= 0.70` guard** (`FIND-P7-11`).

### Pending (Phase N)

- **Graduation cache-invalidation:** the `last_graduation_at` scope cursor on
  `reborn_monty_vm_settings` + the `AFTER DELETE` graduation trigger are NOT
  yet shipped (no migration creates them). Until then graduation does not
  event-evict the `PostgresSource` SplitResult memo-cache (retrieval re-queries).
- **Tier-0 Wilson guard:** add `wilson_lower >= 0.70` to `is_tier0_eligible`
  (`FIND-P7-11`).

## 3. Data Model

### `reborn_validation_queue` (V051 — shipped)

```sql
CREATE TABLE reborn_validation_queue (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    agent_id        TEXT        NOT NULL,
    project_id      TEXT        NOT NULL,
    component_id    UUID        NOT NULL,
    component_class SMALLINT    NOT NULL,   -- class_code; for WebUI filtering
    state           SMALLINT    NOT NULL DEFAULT 1
        CHECK (state IN (1, 2, 3, 4)),
    counter         INT         NOT NULL DEFAULT 0,   -- permanent rejection count
    review_feedback TEXT,                              -- from Q2 reviewer
    validation_errors TEXT[]   NOT NULL DEFAULT '{}',  -- from Q1; cleared on pass
    submitted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)
);
```

Three indexes: scope+state (list views), scope+class (WebUI filtering), and
a partial index `WHERE state = 4` for the deletion-candidate cleanup job.

**Queue states:**

| State | Value | Meaning | Who may write it |
|-------|-------|---------|------------------|
| Q1 queue | 1 | Submitted, awaiting Gate 1 | Application layer |
| Q1 passed | 2 | Gate 1 clean, awaiting Q2 | **Gate 1 only** (security invariant) |
| Rejected | 3 | Q2 reviewer rejected; author must revise | Q2 reviewer |
| Deletion candidate | 4 | Too many rejections / condemned | System (counter threshold) or Q2 |

**Rejection counter** — `counter` starts at 0, increments by 1 on every
rejection (state 2→3, or 3→1 then rejected again), and **never resets**. At
a configurable threshold (default 3) the queue auto-promotes the row to
state 4 — perpetually-stuck components never clog the queue.

### `validation_status` on component tables (unchanged, stays)

Every component table keeps `validation_status`. `'validated'` is the
retrieval gate: every `RetrievalSource` query and `do_reassemble` already
filters `WHERE validation_status = 'validated'`. `'upgrade_queued'` means
a validated component is re-entering the queue after an edit. This column
is **not** moved to the queue — it is the post-validation runtime identity.

### Columns that moved to the queue (shipped across V072–V075)

| Removed from component table | Becomes on queue | Notes |
|------------------------------|------------------|-------|
| `queue_code TEXT` | `state SMALLINT` | text code → smallint enum |
| `review_attempts INT/SMALLINT` | `counter INT` | renamed + centralized |
| `review_feedback TEXT` | `review_feedback TEXT` | moved |
| `rejected_at TIMESTAMPTZ` | (queue `updated_at`) | row's updated_at serves |
| `validation_errors TEXT[]` | `validation_errors TEXT[]` | moved; cleared on Q1 pass |

After V072–V075, every component table has lost these 4-5 columns; the
per-table decoders were re-indexed at each drop (`FIND-P6-04`).

## 4. Behavior

### Q1 (Gate 1 — automatic)

`run_q1_validation(pool, scope, component_id, class_code, payload, config, queue_store)`
(in `q1_orchestrator.rs`, composition crate) owns the sequence:

1. Load `ValidationConfig` from `reborn_validation_config` for `class_code`.
2. Call `ComponentValidator::validate_by_class(...)` (engine, pure fn) —
   the per-class structural checks (§4b) **plus** the C.4.5.1–5 common-syntax
   `{{ ... }}` placeholder-grammar / non-nil-includes / variant-description
   gates that apply to the code-bearing classes (0, 10/50, 13, 21, 22).
3. On pass → `ValidationQueueStore::gate1_pass(scope, id, &[])` (state
   1→2). On fail → `gate1_fail(scope, id, &errors)` (stays state 1,
   `validation_errors` populated, counter untouched).

**Visibility invariant (`FIND-P9-01`):** `gate1_pass` / `gate1_fail` are
`pub(crate)` on `ValidationQueueStore` in `brassclaw_reborn_composition`.
The engine `ComponentValidator` lives in a *different* crate and cannot
call `pub(crate)` methods. Therefore the whole Q1 sequence must live in
the composition crate (call the engine pure fn, then call the crate-local
gate methods). Never call `gate1_pass` from `brassclaw_engine`. The API
layer (webui_v2, ingress) can only call `submit` — it can never reach
`gate1_pass`, which is the Rust-level enforcement of the state-2
invariant.

### Q2 (manual review)

- `reject(scope, id, feedback)` — state 2→3, increments `counter`,
  auto-promotes to state 4 if `counter >= threshold`.
- `approve(scope, id)` — graduation. One transaction (`FIND-P9-05`):
  1. `BEGIN`
  2. `UPDATE {component_table} SET validation_status='validated'
     WHERE id=$1 AND scope` — dispatch on `component_class` to find the
     target table using the same class→table map as
     `fetch_component_by_id`; unknown class → error *before* the
     transaction.
  3. `DELETE FROM reborn_validation_queue WHERE component_id=$1 AND scope`
     — (the graduation trigger that bumps `last_graduation_at` is Phase N,
     not yet shipped; graduation currently does not event-evict the cache).
  4. `COMMIT`
  
  **Ordering: UPDATE before DELETE** so the component is validated before
  the queue row is removed, keeping the non-overlapping-states invariant
  intact at every intermediate point.

- `purge_deletion_candidates(scope)` — deletes state-4 rows and their
  components (cleanup job).
- `list(scope, state_filter)` — WebUI validation view.

### Graduation cache invalidation (Phase N.3 — planned)

A graduation (Q2 approve → queue row DELETE) is the authoritative
cache-invalidation signal. The `AFTER DELETE` trigger on
`reborn_validation_queue` bumps `last_graduation_at` on the scope cursor:

```sql
CREATE FUNCTION reborn_validation_queue_graduation() RETURNS TRIGGER ...
BEGIN
    INSERT INTO reborn_monty_vm_settings
        (tenant_id, user_id, agent_id, project_id, last_graduation_at)
    VALUES (OLD.tenant_id, OLD.user_id, OLD.agent_id, OLD.project_id, now())
    ON CONFLICT (tenant_id, user_id, agent_id, project_id)
    DO UPDATE SET last_graduation_at = now();
    RETURN NULL;   -- FIND-13: AFTER trigger idiom
END;
CREATE TRIGGER reborn_validation_queue_on_delete
    AFTER DELETE ON reborn_validation_queue
    FOR EACH ROW EXECUTE FUNCTION reborn_validation_queue_graduation();
```

This `INSERT … ON CONFLICT DO UPDATE` form is the same pattern
`PgMontyVmSettingsStore::upsert` already uses (`pg_monty_vm_settings.rs:162`
— verified to be a true `INSERT … ON CONFLICT ON CONSTRAINT … DO UPDATE`,
**not** a bare `UPDATE`). The V034 schema (`NOT NULL DEFAULT …` on every
resource column) makes the 5-column INSERT valid on the first graduation
even when no settings row exists yet — `FINDING D` is resolved and stale;
do not "fix" the trigger or rewrite `upsert`.

The `PostgresSource` SplitResult memo-cache checks `last_graduation_at` on
every hit (one PK read, sub-millisecond): if it is newer than the cache
entry's `cached_at`, evict all cache entries for the scope and recompute.
No TTL; eviction is exact and event-driven.

### Wilson scoring (live today, on `reborn_recipes`)

`PgRecipeStore::record_outcome(tenant, user, agent, project, id, success)`
runs in one transaction:

1. Atomically increment `usage_count` + (`success_count` | `failure_count`)
   with `RETURNING` the new counts (`query_opt` → `NotFound` on wrong id/scope).
2. Compute `wilson_lower_bound(success, failure, z=1.96)` and `classify_tier`
   in Rust.
3. `UPDATE reborn_recipes SET wilson_lower = $1, tier = $2 WHERE id …`.

Tier thresholds (`classify_tier`):

| Tier | usage_count | w_lower |
|------|-------------|---------|
| Candidate | ≥ 50 | ≥ promotion_threshold (default 0.80) |
| Mature | ≥ 20 | ≥ 0.70 |
| Growing | ≥ 5 | ≥ 0.50 |
| Seedling | any | any |

Wilson scoring is invoked from `RecipeLibrary::record_recipe_outcome`
(`recipe_library.rs:824`) after a turn completes. It gates **Tier-0 direct
execution**: a recipe may short-circuit the LLM only when
`is_deliverable() && tier ∈ {mature, candidate} && wilson_lower >= 0.70`
— see §6 for the missing-guard fix.

## 4b. Per-class Q1 validation criteria (f8 — new-component-creation)

This is the **f8** content: the exact Q1 (Gate 1) criteria each component
class must satisfy at creation. `ComponentValidator::validate_by_class`
dispatches on `class_code`; Q1 is structural + injection-only (no LLM, no DB
pool) — cross-reference checks (UUIDs resolve, step-order, S7) land in Phase
I/N. The C.4.5.1–5 common-syntax `{{ ... }}` placeholder-grammar gates apply
to the code-bearing classes and are marked **PH** below.

| Class | Code | Payload | Q1 criteria |
|-------|------|---------|-------------|
| Skill (rusty/monty/llm) | 1–3 | `ToolSkill` / Generic | full agentskills.io validation (`validate_tool_skill`: name, description, token budget, activation criteria) + config overrides. **No** placeholder gate — skills are pure narrative. |
| Tool | 0 | `ToolSkill` | `validate_tool_skill` (tool_name = `capability_id` non-empty + param_schema) **PH** + `validate_tool_skill_placeholders` (grammar + non-nil includes). `§capability-id`: system rows need a valid `capability_id`. |
| Extension | 4–9 | Generic / `ToolSkill` | name + description + content + soft **standard** budget. No placeholder gate. |
| Orchestrator / Scaffold | 10 / 50 | Generic / `ToolSkill` / Recipe | soft **orchestrator** budget **PH** + `validate_placeholder_grammar` (the composed script carries `{{vars.NAME}}`/`{{vars.slotN}}`/`{{user_input}}`/`{{component_name}}`). Q1 never bakes — the composer is the sole baker. |
| Action | 16 | Generic / `ToolSkill` / Recipe | name + description + content, **no** token budget. (ActionShortCircuit is vestigial under Q2; the retired step-machine was deleted in C.1.) |
| Recipe | 21 | `Recipe` | `RecipeValidator::validate_recipe` (existing_skill_names) + `check_variant_descriptions` **PH** (v3 variants require a non-empty human-readable `description`; legacy `step_link==None` exempt). |
| PythonCode | 22 | Generic | soft **10k** budget + `validate_python_code_body` (shell-injection scan) **PH** + `validate_python_code_placeholders` (grammar + non-nil includes). |
| ExtensionCatalogue | 23 | Generic (`extra`) | soft standard budget + `validate_extension_catalogue_extras`: name format + non-empty `overview_doc` + ≥1 `task_group` + valid UUID syntax in `child_component_ids`. |
| Notes | 15 | Generic / `ToolSkill` / Recipe | soft **2000** budget. |
| ToolSkill | 13 | `ToolSkill` | full `validate_tool_skill` (same canonical validator as class 0 + 1–3) **PH** + `validate_tool_skill_placeholders` (grammar + non-nil includes). Generic/Recipe payloads rejected. |
| Memory (spec/lesson/issue/summary/plan) | 12, 14, 17–20 | Generic / `ToolSkill` / Recipe | name + description + content + soft **10000** budget. |
| Unknown | other | Generic / `ToolSkill` / Recipe | lightweight generic soft-standard-budget check. |

**Common-syntax placeholder grammar** (C.4.5.1): the recognised `{{ ... }}`
kinds are `{{vars.NAME}}`, `{{vars.slotN}}`, `{{user_input}}`,
`{{component_name}}` (and `{{component_id}}`/`{{step_link}}` for the
compose-orchestrator seed). An unbalanced `{{` with no closing `}}`, or an
unrecognised placeholder kind, is a Q1 hard error. Q1 checks **grammar
only** — referential placeholder↔include matching (each include consumed by
a placeholder) is deferred to Phase I/N (requires a pool).

**System-authored bypass:** builtin seeds (`source='system'`) skip Q2 but
**not** Q1 — Q1 still runs inside the seeder; a Q1 failure there is a
build-time/CI bug, not a runtime failure.

## 5. Relations

- **Component authoring paths → queue:** WebUI save (skill/recipe/tool/
  PythonCode), MCP import (`Phase K.2` inserts with
  `validation_status='pending'`), Sempai proposals (`SempaiProposalSink`),
  and builtin bootstrap (`Phase L` seeds with `validation_status='validated'`
  + system-level validation bypass — builtins skip the queue).
- **`ComponentValidator` ← `reborn_validation_config`** (V031) supplies the
  per-class `ValidationConfig`.
- **`ValidationQueueStore` → component tables:** `approve` dispatches on
  `component_class` to the right table (same map as
  `fetch_component_by_id`).
- **Graduation → retrieval/prefix cache (Phase N, pending):** queue DELETE →
  trigger → `last_graduation_at` → SplitResult cache eviction; a
  newly-validated component appears in the next `fetch_for_turn` /
  `do_reassemble`. The trigger + cursor are not yet shipped — until then
  graduation does not event-evict the cache.
- **Graduation → base-prompt store:** on any `validated` transition, Phase
  K.1 calls `PgBasicPromptStore::mark_stale(scope)` so the base prompt is
  reassembled with the new component (see `10-prefix-base-prompt.md`).
- **Wilson scoring → Tier 0:** `record_recipe_outcome` → `is_tier0_eligible`
  → the Orchestrator Matching-/Non-Matching-Mode decision
  (see `12-agent-loop.md`, `13-orchestrator-default-py.md`).
- **The Phase B/C tables** (`reborn_python_code` V052, class 22;
  `reborn_extension_catalogues` V053, class 23) are designed **without** the
  five legacy columns from day one — they use the queue natively.

## 6. Shipped vs pending

| Aspect | Shipped | Pending (Phase N) |
|--------|---------|-------------------|
| Pre-validation lifecycle | centralized on `reborn_validation_queue` (V051); the 5 legacy columns dropped off component tables (V072–V075) | — |
| Queue store | `ValidationQueueStore` (`validation_queue.rs`) | — |
| Q1 orchestration | `run_q1_validation` in `q1_orchestrator.rs` owns the cross-crate sequence (`FIND-P9-01`) | — |
| Q1 common-syntax gates | C.4.5.1–5 placeholder-grammar / non-nil-includes / variant-description gates on classes 0/10/50/13/21/22 | referential placeholder↔include matching (Phase I/N — requires a pool) |
| State-2 write invariant | `gate1_pass` `pub(crate)`; only Gate 1 reaches it | — |
| Q2 approve | `approve()` one-tx UPDATE-then-DELETE, class-dispatched (`FIND-P9-05`) | — |
| Cache invalidation on graduation | — | `last_graduation_at` cursor + `AFTER DELETE` trigger → SplitResult cache eviction |
| Wilson scoring | **live** (`record_outcome`, `wilson_lower_bound`, `classify_tier`) | — |
| Tier-0 eligibility guard | `is_tier0_eligible` checks `deliverable && tier∈{mature,candidate}` | add the `wilson_lower >= 0.70` guard (`FIND-P7-11`) |
| Per-table decoders | re-indexed at each V072–V075 column drop (`FIND-P6-04`) | — |

**Boot integrity check (§0.18):** every component with
`validation_status != 'validated'` must have a queue row; a component with
no queue row and non-validated status is an inconsistent state detected at
boot.

## 7. LLM Summary (machine-convertible)

The Validation Queue System gates whether an agent component may run. It
has two sequential gates: **Q1 (Gate 1)** is an automatic, deterministic,
no-LLM structural + injection check (`ComponentValidator::validate_by_class`,
config from `reborn_validation_config`); on pass it transitions the queue
row to state 2 (Q1 passed) — and only Gate 1 may write state 2 (security
invariant, enforced by `gate1_pass` being `pub(crate)` and reachable only
through `q1_orchestrator.rs`). **Q2** is a human reviewer who approves
(graduation: `validation_status` becomes `'validated'`, queue row deleted)
or rejects (state 3, increments a permanent `counter`; at threshold → state
4 deletion candidate). The `reborn_validation_queue` table (V051, shipped)
centralizes the pre-validation lifecycle; the five legacy columns
(`queue_code`/`review_attempts`/`review_feedback`/`rejected_at`/
`validation_errors`) were dropped off the component tables across V072–V075
(classes 22/23 were created without them). Q1 also runs the C.4.5.1–5
common-syntax `{{ ... }}` placeholder-grammar / non-nil-includes /
variant-description gates on the code-bearing classes (0/10/50/13/21/22);
the per-class criteria are in §4b (f8). Graduation's `AFTER DELETE` trigger
+ `last_graduation_at` cursor (evicting the `PostgresSource` SplitResult
memo-cache) are Phase N, not yet shipped. Wilson score lower bounds (live via
`record_outcome` → `wilson_lower_bound` + `classify_tier`) gate Tier-0
direct execution on top of validation; `PgRecipe::is_tier0_eligible`
currently omits the `wilson_lower >= 0.70` guard (Phase N fix). Builtins
(`source='system'`) bypass Q2 but not Q1; external MCP imports and Sempai
proposals always enter Q1. **Status:** Gate 1 pure logic + common-syntax
gates + Wilson scoring + the queue table/store/orchestrator + the legacy
column drops are all shipped; the graduation cache-invalidation trigger +
the Tier-0 Wilson guard remain (Phase N).
