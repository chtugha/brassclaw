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
> **Grounded in:** `saved_plan_to_v3.md` §0.18 (lines 1945-2104), Phase A.5
> (lines 2778-2870), Phase N (lines 6294-6700), the migration table (lines
> 22-29), and the live code that already exists:
> `crates/brassclaw_engine/src/memory/component_validator.rs` (Gate 1 pure
> logic), `crates/brassclaw_pg/migrations/V031__reborn_validation_config.sql`
> + `V033__reborn_recipes.sql` (current on-table validation columns),
> `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`
> (`record_outcome:464`, `is_tier0_eligible:140`, `RECIPE_SELECT:208`),
> `crates/brassclaw_agent_loop/src/plan_scoring.rs`
> (`wilson_lower_bound:201`, `classify_tier:230`),
> `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs`
> (`upsert:108` — the INSERT…ON CONFLICT pattern the graduation trigger
> mirrors). Findings `FIND-P9-01/05/08`, `FIND-P6-04/08`, `FIND-P7-11`,
`SCHEMA-01`, `FINDING D`, `FIND-13`.

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

### Today (codebase, migrations ≤ V049)

The validation lifecycle is **not yet centralized**. The pre-validation
columns live directly on every component table, and the Q1 logic is a
pure function:

- **Gate 1 logic:** `crates/brassclaw_engine/src/memory/component_validator.rs`
  — `ComponentValidator::validate_by_class(class_code, payload, config)`
  dispatches to the class-appropriate validation path. Pure functions, no
  I/O, no LLM. Skill classes (01-03) get full agentskills.io validation;
  Tool (00) gets tool_name + param_schema; Extensions (04-09) get
  name+description+content+soft budget; Actions (16) get name+content;
  Recipes (21) delegate to `RecipeValidator`; former-DocType classes
  (12-15, 17-20) get name+description+content+soft budget.
- **Per-class config:** `reborn_validation_config` (V031) — supplies
  `name_min_len`, `token_budget`, `token_budget_hard_error`,
  `require_tool_name`, etc. `ComponentValidator` reads a `ValidationConfig`
  at call time and falls back to compile-time defaults when the row is
  absent.
- **On-table lifecycle columns** (V033 recipes, identical shape on the 13
  component tables): `validation_status` (`'pending'` / `'validated'` /
  `'upgrade_queued'` / `'rejected'` / `'garbage'` / `'auto_passed'` /
  `'auto_failed'` / `'review_requested'`), `validation_errors TEXT[]`,
  `review_feedback TEXT`, `review_attempts` (SMALLINT on 10 tables, INT on
  3 — `SCHEMA-01`), `rejected_at TIMESTAMPTZ`, `queue_code TEXT`.
- **Wilson scoring (live):**
  - `crates/brassclaw_agent_loop/src/plan_scoring.rs` —
    `wilson_lower_bound(successes, failures, z)` (:201) and
    `classify_tier(usage_count, w_lower, promotion_threshold)` (:230).
  - `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` —
    `PgRecipeStore::record_outcome` (:464) increments `usage_count` +
    `success_count`/`failure_count` in one transaction, recomputes
    `wilson_lower` + `tier`, and writes both back (`:528`).
  - `reborn_recipes` (V033) carries `tier` (`seedling`/`growing`/`mature`/
    `candidate`), `usage_count`, `success_count`, `failure_count`,
    `wilson_lower`.
- **Tier-0 eligibility (live, incomplete):** `PgRecipe::is_tier0_eligible`
  (`pg_recipe_store.rs:140`) — checks `is_deliverable() && tier ∈
  {mature, candidate}` but **omits the `wilson_lower >= 0.70` guard**
  (`FIND-P7-11`). The engine-domain `Recipe::is_tier0_eligible()` also
  checks a `has_validation` (validation-hook-wired) field that `PgRecipe`
  does not carry (`FIND-P9-09`).

### v3 target (migrations V051, V058/V059 — none exist yet)

The plan centralizes the pre-validation lifecycle onto a single queue
table and adds the graduation cache-invalidation signal:

- **Queue table:** `reborn_validation_queue` — created in **V051**
  (Phase A.5; `was V050 before Decision 2`). DDL + indexes only.
- **Queue store:** `crates/brassclaw_reborn_composition/src/validation_queue.rs`
  — `ValidationQueueStore` (created in Phase A.5 per Decision 2; **not yet
  present in the codebase**).
- **Q1 orchestration:** `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`
  — `run_q1_validation(...)` owns the cross-crate call sequence
  (`ComponentValidator::validate_by_class` → `gate1_pass`/`gate1_fail`)
  because `gate1_pass` is `pub(crate)` and the engine cannot call across
  crate boundaries (`FIND-P9-01`). **Not yet present.**
- **Populate + drop migration:** `V059__reborn_validation_queue_populate.sql`
  (`was V058 before Decision 2`) — populates the queue from existing
  component rows, adds `last_graduation_at`, wires the graduation trigger,
  drops the five legacy columns off the 13 tables.
- **Scope cursor:** `last_graduation_at TIMESTAMPTZ` added to
  `reborn_monty_vm_settings` (V034) — written by an `AFTER DELETE` trigger
  on the queue.
- **Cache hook:** `PostgresSource` SplitResult memo-cache checks
  `last_graduation_at` on every hit (Phase N.3).

## 3. Data Model

### `reborn_validation_queue` (V051 — planned)

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

### Columns that move to the queue (V059 — planned)

| Removed from component table | Becomes on queue | Notes |
|------------------------------|------------------|-------|
| `queue_code TEXT` | `state SMALLINT` | text code → smallint enum |
| `review_attempts INT/SMALLINT` | `counter INT` | renamed + centralized |
| `review_feedback TEXT` | `review_feedback TEXT` | moved |
| `rejected_at TIMESTAMPTZ` | (queue `updated_at`) | row's updated_at serves |
| `validation_errors TEXT[]` | `validation_errors TEXT[]` | moved; cleared on Q1 pass |

After V059, every component table loses 4-5 columns; `decode_recipe_row`
must be re-indexed (§6, `FIND-P6-04`).

## 4. Behavior

### Q1 (Gate 1 — automatic)

`run_q1_validation(pool, scope, component_id, class_code, payload, config, queue_store)`
(in `q1_orchestrator.rs`, composition crate) owns the sequence:

1. Load `ValidationConfig` from `reborn_validation_config` for `class_code`.
2. Call `ComponentValidator::validate_by_class(...)` (engine, pure fn).
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
     — the graduation trigger fires here (bumps `last_graduation_at`).
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
- **Graduation → retrieval/prefix cache:** queue DELETE → trigger →
  `last_graduation_at` → SplitResult cache eviction; a newly-validated
  component appears in the next `fetch_for_turn` / `do_reassemble`.
- **Graduation → base-prompt store:** on any `validated` transition, v3
  Phase K.1 calls `PgBasicPromptStore::mark_stale(scope)` so the base
  prompt is reassembled with the new component (see
  `10-prefix-base-prompt.md`).
- **Wilson scoring → Tier 0:** `record_recipe_outcome` →
  `is_tier0_eligible` → `RecipeStage` / `execute_recipe_orchestrator_channel`
  (see `12-agent-loop.md`, `13-orchestrator-default-py.md`).
- **Two Phase B/C tables** (`reborn_python_code` V052, class 22;
  `reborn_extension_catalogues` V053, class 23) are created *after* the
  queue exists and are designed **without** the five legacy columns from
  day one — they use the queue natively.

## 6. Today vs v3

| Aspect | Today (≤ V049) | v3 target (V051 / V058 / V059) |
|--------|----------------|--------------------------------|
| Pre-validation lifecycle | 5 columns **on each component table** (`queue_code`, `review_attempts`, `review_feedback`, `rejected_at`, `validation_errors`) | centralized on `reborn_validation_queue`; columns dropped off component tables |
| Queue store | — | `ValidationQueueStore` (`validation_queue.rs`, Phase A.5) |
| Q1 orchestration | `ComponentValidator` pure fn called ad hoc | `run_q1_validation` in `q1_orchestrator.rs` owns the cross-crate sequence (`FIND-P9-01`) |
| State-2 write invariant | (no queue state) | `gate1_pass` `pub(crate)`; only Gate 1 reaches it |
| Q2 approve | (manual SQL / API) | `approve()` one-tx UPDATE-then-DELETE, class-dispatched (`FIND-P9-05`) |
| Cache invalidation on graduation | none (retrieval re-queries every turn) | `last_graduation_at` cursor + `AFTER DELETE` trigger → SplitResult cache eviction |
| Wilson scoring | **live** (`record_outcome`, `wilson_lower_bound`, `classify_tier`) | unchanged mechanism; consumed by Tier 0 |
| Tier-0 eligibility guard | `is_tier0_eligible` checks `deliverable && tier∈{mature,candidate}` — **missing `wilson_lower >= 0.70`** (`FIND-P7-11`) | add the Wilson guard (Phase A) |
| `decode_recipe_row` indices | 0-30 (31 cols); Phase A appends 31/32/33 | after V059 drops 5 mid-list cols, re-index to 0-28 (`FIND-P6-04`) |
| `review_attempts` type | SMALLINT (10 tables) / INT (3 tables) — `SCHEMA-01` | populate arm casts `COALESCE(review_attempts::INT, 0)` uniformly |
| V059 populate class_code | — | `class_code::SMALLINT` for variable-class tables (skills 1/2/3, extensions 4-9), literal for fixed-class (recipes 21, actions 16, tools 0) — `FIND-P6-08`; Phase B/C tables substitute literal defaults for the missing columns |

**Two-phase deploy (zero-downtime, `FIND-P5-08`):** V059 drops columns. A
rolling deploy where the old binary still runs when V059 applies would
`SELECT` dropped columns → runtime panic. Order: (1) deploy new binary with
the dropped fields as `Option<T>` + `#[serde(default)]`; (2) run V059; (3)
remove the `Option` wrappers in a follow-up. Run `cargo check --all`
immediately after removing the struct fields — the compiler is the audit
that finds every dead reference, not a manual inspection.

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
4 deletion candidate). The `reborn_validation_queue` table (V051, planned)
centralizes pre-validation lifecycle; the five legacy columns
(`queue_code`/`review_attempts`/`review_feedback`/`rejected_at`/
`validation_errors`) move off the 13 component tables in V059, which
re-indexes `decode_recipe_row`. Graduation fires an `AFTER DELETE` trigger
that bumps `last_graduation_at` on `reborn_monty_vm_settings` via
`INSERT … ON CONFLICT DO UPDATE` (mirrors `PgMontyVmSettingsStore::upsert`),
evicting the `PostgresSource` SplitResult memo-cache for the scope. Wilson
score lower bounds (live today via `record_outcome` → `wilson_lower_bound` +
`classify_tier`) gate Tier-0 direct execution on top of validation;
`PgRecipe::is_tier0_eligible` currently omits the `wilson_lower >= 0.70`
guard and is fixed in v3 Phase A. Builtins (Phase L) bypass the queue with
`validation_status='validated'` + system-level validation bypass; external
MCP imports and Sempai proposals always enter Q1. **Status:** Gate 1 pure
logic + Wilson scoring exist in the codebase; the queue table, store,
orchestrator, trigger, and column drops are all v3 (V051/V058/V059), none
present yet.
