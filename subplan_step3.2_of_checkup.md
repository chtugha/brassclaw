# Sub-plan: Step 3.2 — Wire DbToolSource into Q1 auto-validation

## Problem

`DbToolSource` exists in `crates/brassclaw_engine/src/capability/db_tool_source.rs` and is fully
implemented. `ComponentValidator::validate_by_class` accepts `available_tools: &[String]`. But:

1. No code path calls `ComponentValidator::validate_by_class` during a status transition.
2. The `available_tools` slice is always empty at every call site (all in tests).
3. The Q1 auto-validation (for ToolSkill class 13) never fetches tool names from `reborn_tools`.

## The Gap

When a component enters `pending` (Q1), the validation gate (`ComponentValidator::validate_by_class`)
is supposed to run and produce `auto_passed` or `auto_failed`. Currently:
- `update_component_validation_status` in `recipe_store.rs` just updates status — no validation runs.
- `PgRecipeStoreFacade::update_skill_validation_status` is a no-op stub.

## Plan

### Sub-step 3.2.1 — Add `run_q1_auto_validation` to `pg_recipe_store.rs`
Wire `DbToolSource` into a new function `run_q1_auto_validation` inside `PgRecipeStoreFacade`:
- Called from `update_component_validation_status` when `new_status == "auto_pass"` or from a
  dedicated Q1 auto-pass sweep.
- Actually, the correct design is: when a new component is created with `validation_status='pending'`
  and `queue_code='q1_auto'`, a background pass should call `ComponentValidator::validate_by_class`
  and then move to `auto_passed` (→ Q2 for manual review) or `auto_failed` (stays in Q1 with errors).

The _actual implementation path_ is simpler: the `update_component_validation_status` method
in `PgRecipeStoreFacade` for `class_code = 13` (and other ToolSkill-backed classes) needs to:
1. Fetch tools via `DbToolSource::fetch_tool_names(scope)` 
2. Run `ComponentValidator::validate_by_class(class_code, payload, config, &tools, &[])`
3. If errors → set `auto_failed`; else → set `auto_passed`

But there's a subtlety: the WebUI calls `update_component_validation_status` with `new_status="validated"`
(human-driven Q2 approval). The Q1 auto-pass is a DIFFERENT step. The correct model is:
- A background task or explicit trigger sweeps `pending` items in Q1, runs validation, sets `auto_passed/auto_failed`.
- The WebUI validate action only applies to Q2 items.

### Sub-step 3.2.2 — Add `auto_validate_pending` method to `PgRecipeStoreFacade`
New async method that:
1. Fetches all `pending` rows from `reborn_recipes` (or the tool-skill store when V037 lands)
2. For each: fetches `available_tools` using `DbToolSource` with the component's scope
3. Runs `ComponentValidator::validate_by_class`
4. Sets `auto_passed` (→ q2_manual) or `auto_failed` with `validation_errors`

### Sub-step 3.2.3 — Expose `auto_validate_pending` in the `RecipeStore` trait
Add `auto_validate_pending(&self, user_id, project_id)` to `brassclaw_product_workflow::RecipeStore` trait
with a default returning `Ok(0)` (no-op for non-Postgres stores).

### Sub-step 3.2.4 — Wire a background sweep task that calls `auto_validate_pending`
In `crates/brassclaw_reborn_composition/src/runtime.rs`, spawn a periodic task
(every 30s) that calls `auto_validate_pending`. Gate on `postgres` feature.

### Sub-step 3.2.5 — Confirm `DbToolSource` exported from `brassclaw_engine` under `skills-db`
Already done. No change needed.

### Sub-step 3.2.6 — Add unit tests
Test that `auto_validate_pending` moves a pending ToolSkill row to `auto_passed` when
the tool name is known, and to `auto_failed` when the tool name is not in the registry.

### Sub-step 3.2.7 — Clippy clean + update checkup.md

## Files to touch
- `crates/brassclaw_product_workflow/src/recipes.rs` — add `auto_validate_pending` to trait
- `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` — implement auto-validation
- `crates/brassclaw_reborn_composition/src/recipe_store.rs` — add no-op default impl
- `crates/brassclaw_reborn_composition/src/runtime.rs` — spawn background sweep
- `checkup.md` — mark Step 3.2 as IMPLEMENTED
