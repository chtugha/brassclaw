# Sub-plan: Phase P.0 — Fix `find_validator_recipe` + Seed Validation-System Recipes (§0.23.3)

**Written:** during Phase P.0 implementation review.
**Trigger:** user asked "Phase L should have been implemented" — confirmed two distinct
bugs found by reading live code against the plan.

---

## Root-cause analysis

### Bug 1 — `find_validator_recipe` queries the wrong column

`crates/brassclaw_reborn_composition/src/q1_orchestrator.rs` `find_validator_recipe`:

```sql
SELECT id FROM reborn_recipes
 WHERE class_code       = $5           -- <-- BUG
   AND validation_status = 'validated'
   AND '05:validator'   = ANY(consumer_tags)
```

`reborn_recipes.class_code` is **always 21** (`CHECK (class_code = 21)` in V033 DDL).
It records "this row is a Recipe", not "this Recipe validates components of class X".

The parameter `$5` is the **component being validated**'s class code (e.g. `0` for a
Tool, `1` for a Skill, `21` for a Recipe). When validating anything other than a Recipe
(class 21), the query returns zero rows → graceful defer forever.

**The column needed is `validates_class_code SMALLINT`** — a new column on
`reborn_recipes` that records which component class this Recipe is a validator for.
`NULL` means "general-purpose Recipe" (not a validator). Non-NULL means "this Recipe
validates components of class X."

**Fix:** add `validates_class_code SMALLINT` to `reborn_recipes` (V079 migration),
update `find_validator_recipe` to filter on `validates_class_code = $5` instead of
`class_code = $5`, update `NewPgRecipe` and `PgRecipe` structs, update
`decode_recipe_row` and `RECIPE_SELECT`, update `recipe_row()` in `builtin_bootstrap.rs`
to set `validates_class_code` on validator recipes.

### Bug 2 — Phase L §0.23.3 validation-system seeding was explicitly deferred and never done

Phase L status says "The §0.23.3 trusted-root validation-system fold-in is deliberately
deferred … and is not part of L.0–L.3."

The §0.23.3 seeding means: for each retrievable component class (0, 1–3, 4–9, 12–23),
seed ONE basic validator Recipe with `consumer_tags = ['05:validator']` and
`validates_class_code = <class>`, `validation_status = 'validated'`.

Per §0.23.3 the "basic Recipe" at bootstrap time does little more than create a prompt
for the kohai-sempai system. Until the self-improvement loop matures it, the Recipe's
job is to submit the component to an LLM review and record pass/fail. The minimum viable
validator Recipe is a Tier-1 Recipe (uses LLM) that:
1. Assembles a validation prompt from the component content
2. Calls the kohai-sempai (or a simple LLM call) with the component + a validation query
3. Interprets the answer as pass/fail + records errors

For the current phase the **minimum viable implementation** is simpler: a validator
Recipe whose PythonCode step calls a built-in `builtin.validate_component` tool that
runs the legacy pure-Rust checks (injection scan, schema conformance, etc.). This is
exactly what the plan describes as "calculated risk" — the Rust checks become a Recipe
step, and the self-improvement loop can replace them with smarter checks over time.

BUT: even simpler — for the immediate goal (Q1 can run, graceful-defer is not the only
path), the minimum viable validator Recipe can be a Tier-1 (LLM-assisted) Recipe that
does a basic structural check of the component's content using the LLM. When validation
Recipes are seeded and `find_validator_recipe` works, `run_q1_validation` can call
`execute_tier_zero_channel` (Tier-0) or the full loop (Tier-1) and record a real result.

---

## Steps to execute (strictly in order, one at a time)

### Step 1 — V079 migration: add `validates_class_code` to `reborn_recipes`

**File:** `crates/brassclaw_pg/migrations/V079__reborn_recipes_validates_class_code.sql`

```sql
-- Phase P.0 fix: add validates_class_code to reborn_recipes so validator Recipes
-- can declare which component class they validate. NULL = general-purpose Recipe.
ALTER TABLE reborn_recipes
    ADD COLUMN IF NOT EXISTS validates_class_code SMALLINT;
-- No CHECK constraint: NULL is valid (most Recipes); non-NULL is any i16 class code.
```

Additive only. All existing rows get `NULL` (not a validator Recipe). No index needed
yet — the lookup is scoped + filtered and the table is small.

**After this step:** run clippy + `cargo test -p brassclaw_pg`. Confirm migration file is
picked up by `embed_migrations!`.

### Step 2 — Update `PgRecipe`, `NewPgRecipe`, `RECIPE_SELECT`, `decode_recipe_row`

**File:** `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`

- Add `validates_class_code: Option<i16>` to `PgRecipe` struct.
- Add `validates_class_code: Option<i16>` to `NewPgRecipe` struct (default `None`).
- Add `validates_class_code` to `RECIPE_SELECT` constant (append at end — it is
  currently index 31 after the Phase A columns; check current end index first).
- Update `decode_recipe_row` to decode the new column at the new index.
- Update the `INSERT` in `PgRecipeStore::insert` to include `validates_class_code`.

**IMPORTANT:** `RECIPE_SELECT` currently selects 29 columns (indices 0–28 after Phase N
dropped 5 legacy columns from reborn_recipes — confirmed in last session). Add
`validates_class_code` at index 29. Update `decode_recipe_row` accordingly.

Verify the exact current column count before editing:
```bash
grep -c "," <<< "$RECIPE_SELECT_literal"  # rough count
```
Or read `pg_recipe_store.rs` lines 208-252 to see the current SELECT + decode.

**After this step:** `cargo clippy -p brassclaw_reborn_composition -- -D warnings`.

### Step 3 — Fix `find_validator_recipe` query

**File:** `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`

Replace:
```sql
AND class_code       = $5
```
With:
```sql
AND validates_class_code = $5
```

Also update the doc comment to describe `validates_class_code`.

**After this step:** `cargo test -p brassclaw_reborn_composition --lib q1_orchestrator`.

### Step 4 — Seed one minimal validator Recipe per class in `builtin_bootstrap.rs`

**File:** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`

For each retrievable component class that the seeder currently handles (0, 1, 13, 21,
22, 23 — the classes the seeder seeds), add a minimal validator Recipe:

```
name: "validator-class-{N}"    (e.g. "validator-class-0", "validator-class-21")
validates_class_code: N
consumer_tags: ["05:validator"]
validation_status: "validated"    (builtin — direct insert, exempt from Q2)
source: "system"
tier: "mature", wilson_lower: 1.0  (so it's Tier-0 eligible if PythonCode-only)
```

The initial Recipe body can be a minimal Tier-1 Recipe that describes what to check
for that class. The plan (§0.23.3) says: "at the start it does little more than create
a prompt for the kohai-sempai system." So the initial `step_entries` can be a single
PythonCode step that calls a simple structural validation (later: the kohai-sempai path).

**Minimum viable PythonCode body** for each class validator:
```python
# Tier-0 structural validator for class N.
# Checks: name present, description non-empty, content/steps non-empty.
# Returns pass/fail with errors list.
name = "{{vars.slot0}}"
description = "{{vars.slot1}}"
content = "{{vars.slot2}}"
errors = []
if not name:
    errors.append("name is empty")
if not description:
    errors.append("description is empty")
if not content:
    errors.append("content is empty")
result = {"pass": len(errors) == 0, "errors": errors}
```

This is a Tier-0 Recipe (PythonCode only, no LLM). It is minimal but functional.
`run_q1_validation` can call `execute_tier_zero_channel` with the assembled content,
get a real pass/fail instead of always deferring.

The `validate_class_code` for each seed call must match the class being validated.

NOTE: `recipe_row()` in `builtin_bootstrap.rs` currently does NOT set
`validates_class_code` on the `NewPgRecipe`. Add it as a parameter to `recipe_row()`
or create a new `validator_recipe_row()` helper that sets it.

**After this step:** `cargo test -p brassclaw_reborn_composition --lib`. The integration
test `tests/builtin_bootstrap_seed.rs` must still pass (row count will increase by the
number of new validator Recipes).

### Step 5 — Wire the sandboxed runner in `run_q1_validation`

**File:** `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`

Now that validator Recipes exist and the query works, wire the actual execution.

The challenge: `run_q1_validation(pool, scope, component_id, class_code, queue_store)`
does not carry the engine dependencies (`EffectExecutor`, `LeaseManager`,
`PolicyEngine`, `GateController`, `LlmBackend`, `Thread`) needed to call
`execute_tier_zero_channel`.

**Design decision (to be confirmed against the plan):** Pass these as a new parameter
bundle `Q1ExecutionContext` to `run_q1_validation`, or use a simpler intermediate:
since Tier-0 validation Recipes are PythonCode-only and do structural checks (no tool
calls), a simpler path is to execute the PythonCode steps directly via `execute_code`
with a minimal sandbox rather than the full `execute_tier_zero_channel` path.

For the minimum viable implementation: add a `Q1RunContext` parameter to
`run_q1_validation` that carries the minimum deps needed, and call
`execute_tier_zero_channel` (or a restricted subset). When context is `None`, keep the
graceful-defer behavior (existing callers that don't have the deps don't break).

**This step requires a sub-sub-plan if the wiring is complex.** Write
`subplan_stub_P0_q1_execution_context.md` if needed.

**After this step:** full workspace clippy + tests.

### Step 6 — Update `saved_plan_to_v3.md`

- Document `validates_class_code` column in §0.23.10 migration table (V079).
- Update `find_validator_recipe` description in Phase N / Phase P.0.
- Update Phase L status to note that §0.23.3 validation-system seeding is done
  (after Step 4).
- Correct the `q1_orchestrator.rs` TODO comment reference from "Phase P.0" to
  "Phase L §0.23.3 — now done; runner wired in Step 5."
- Update AGENTS.md if the Validation queue row needs adjustment.

---

## Files affected

| File | Change |
|------|--------|
| `crates/brassclaw_pg/migrations/V079__reborn_recipes_validates_class_code.sql` | NEW — additive ALTER |
| `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` | `PgRecipe`, `NewPgRecipe`, `RECIPE_SELECT`, `decode_recipe_row`, `insert` |
| `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs` | `find_validator_recipe` query + TODO stub |
| `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` | validator Recipe seed per class |
| `crates/brassclaw_reborn_composition/tests/builtin_bootstrap_seed.rs` | update row count expectation |
| `saved_plan_to_v3.md` | §0.23.10 V079 entry, Phase L §0.23.3 completion, Phase N/P.0 notes |

---

## Constraint: do NOT implement Steps 4–5 as a batch

Steps 1–3 fix the query bug. Step 4 seeds the data. Step 5 wires the runner.
These are three distinct commits. Never combine them.
