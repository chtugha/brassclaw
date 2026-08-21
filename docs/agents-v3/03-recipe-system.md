# 03 — Recipe System

> **Subsystem:** Recipes — the unit of v3 orchestration. A recipe is a trigger plus an ordered
> list of steps; each step says exactly which skills/tools/Python code/LLM prompts are needed and
> how the orchestrator and the Rust executor should run it.
> **Grounded in:** `crates/brassclaw_engine/src/types/recipe.rs`,
> `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`, `recipe_store.rs`,
> `crates/brassclaw_pg/migrations/V033__reborn_recipes.sql`, `saved_plan_to_v3.md` (Phases A, H, L, N).

## 1. Purpose

A **recipe** is a reusable, validated solution component (class 21) that drives the v3 "match →
orchestrate" branch. When the intent system matches a user message to a recipe, the orchestrator
runs the recipe's steps in order. Each step carries everything needed: which skills/tools/Python
code to preload, how to execute, whether an LLM call is required and how its prompt is built.
Recipes are Wilson-scored and tiered: a high-confidence recipe can execute with **no LLM
round-trip** (Tier 0); a lower-confidence match injects known-good patterns into the prompt
(Tier 1); no match falls through to full LLM reasoning (Tier 2), and a success can be **extracted**
into a new recipe + ToolSkill pair.

## 2. Location

- **Data types:** `crates/brassclaw_engine/src/types/recipe.rs` (`Recipe`, `RecipeStep`,
  `ToolSkill`, `RecipeTrigger`, `RecipeValidation`, `ValidationStatus`, `RecipeSource`).
- **Production store (Postgres, class 21):** `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`
  (`PgRecipe`, `NewPgRecipe`, `PgRecipeStore`, `RECIPE_SELECT`, `decode_recipe_row`,
  `PgRecipeLibrary`, `PgRecipeStoreFacade`).
- **Legacy MemoryDoc-backed store (slated for Phase K deletion):**
  `crates/brassclaw_reborn_composition/src/recipe_store.rs` (`StoreBackedRecipeStore`),
  `crates/brassclaw_reborn_composition/src/recipe_library.rs`.
- **Migration:** `crates/brassclaw_pg/migrations/V033__reborn_recipes.sql`.
- **Stage outcome enum (different type, different crate):** `crates/brassclaw_agent_loop/.../canonical.rs`
  `RecipeStep` (the pipeline stage's result — sole variant `Continue` today).
- **Spec:** §3.3, §3.5/§3.5.1 (validation lifecycle), §3.6 (Wilson scoring), §3.7 (class 21),
  §3.9 (consumer_tags), §3.12 (intent_examples), §3.13/§3.14 (Solution Override).

## 3. Data model

### `reborn_recipes` (V033, class 21)

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `tenant_id`,`user_id`,`agent_id`,`project_id` | TEXT NOT NULL | scope tuple |
| `name` | TEXT NOT NULL | kebab-case `^[a-z0-9]…$`, 1–64; unique per scope |
| `description` | TEXT NOT NULL | 1–1024 |
| `trigger` | JSONB | `{type: exact\|pattern\|keyword, …}` |
| `steps` | JSONB NOT NULL DEFAULT '[]' | ordered `[{skill, tool, params, description}]` (legacy); v3 adds `step_descriptions` |
| `status` | TEXT default 'active' | active/archived/draft |
| `prior_knowledge_content` | TEXT | §3.13/§3.14 Solution Override (used verbatim instead of assembling from steps) |
| `override_prompt_creation` | BOOL default false | enables the Solution Override path |
| `class_code` | SMALLINT default 21 | `CHECK = 21` |
| `prompt_uid` | BIGINT | monotonic sequence → deterministic assembly order |
| `consumer_tags` | TEXT[] default '{}' | `^[0-9]{2}(:[a-z0-9-]+)?$`; `05:validator` greys out others until validated |
| `intent_examples` | JSONB | `[{input, class:1\|2\|3}]` for the intent system (§3.12) |
| `tier` | TEXT default 'seedling' | seedling/growing/mature/candidate (maturity) |
| `usage_count`,`success_count`,`failure_count` | INT | Wilson inputs |
| `wilson_lower` | FLOAT8 default 0.0 | Wilson lower bound at last recompute |
| `validation_status` | TEXT default 'pending' | pending/auto_passed/auto_failed/validated/review_requested/rejected/garbage/upgrade_queued |
| `validation_errors` | TEXT[] | `[]` |
| `review_feedback` | TEXT | |
| `review_attempts` | SMALLINT default 0 | incremented by the automated review pipeline only |
| `rejected_at` | TIMESTAMPTZ | |
| `queue_code` | TEXT | q1_auto/q2_manual/q3_revision/q4_rejection/garbage |
| `source` | TEXT default 'authored' | authored/extracted/migrated/imported/system (no CHECK — `'system'` works for the Phase L seeder, FIND-P6-02) |

Unique: `(scope, name)`. Indexes: scope, scope+status, scope+prompt_uid (assembly order),
`consumer_tags` GIN, partial `WHERE validation_status='validated'` on (scope, tier). Trigger:
`set_updated_at()`. (The `consumer_tags` format CHECK was removed — PG16 disallows subqueries in
CHECK; enforced at the app layer.)

### Rust types (`types/recipe.rs`)

- `Recipe` — ordered ToolSkill invocations + trigger + Wilson metrics + lifecycle.
  `is_tier0_eligible()` = tier `mature`/`candidate` **and** `Validated` **and**
  `wilson_lower ≥ 0.70` **and** `validation != None`.
- `RecipeStep` (data model) = `{ skill: String, tool: String, params: Value, description: String }`
  — references a `ToolSkill` by name; `tool` denormalized for cheap lookup; `params` are Tier-0
  overrides; `description` is the Tier-1 prompt-injection text.
- `ToolSkill` = tight description of one tool-usage pattern; target < 5000 tokens
  (agentskills.io progressive disclosure; `RecipeValidator` enforces the ceiling). Fields include
  `param_template`, `param_schema: Vec<ToolSkillParam>`, `preconditions`, `error_handling`,
  `code_snippet`, Wilson metrics, tier.
- `RecipeTrigger` (`tag="type"`): `Exact { command }` | `Pattern { patterns }` (human-authored
  only — LLM regex is a ReDoS risk; validator blocks extracted recipes) | `Keyword { keywords,
  threshold }`. Has `signature()` (duplicate detection) and `trigger_tokens()` (FTS/Jaccard).
- `RecipeValidation` (`tag="type"`): `None` | `ShellCheck { command }` | `FileExists { path }` |
  `Custom { code }` — runs after a recipe executes.
- `ValidationStatus`: `Pending → AutoPassed/UpgradeQueued → Validated` (or `Rejected` after 3
  failed review cycles, then `Garbage` after the 30-day re-review window).
- `RecipeSource`: `Extracted` (default, lifted from a successful thread) | `Authored` |
  `Imported`.
- `recipe_to_memory_doc` / `tool_skill_to_memory_doc` — the **legacy** MemoryDoc round-trip
  (the v2 design stored both as `MemoryDoc`; the production path now uses the dedicated
  `reborn_recipes` table via `pg_recipe_store.rs`).

### Production store round-trip (`pg_recipe_store.rs`)

- `PgRecipe` — fully decoded row; `has_validator_tag()`, `is_deliverable()` (validated + no
  `05:validator` tag, §3.9 SEC-01), `is_tier0_eligible()` (deliverable + tier mature/candidate).
- `NewPgRecipe` — insert payload (scope, name, description, trigger, steps,
  prior_knowledge_content, override_prompt_creation, consumer_tags, intent_examples, source).
- `RECIPE_SELECT` — canonical **31-column** list (indices 0–30); `decode_recipe_row` reads
  positionally with `row.get(0..30)`.
- `PgRecipeStore` — `insert` (`INSERT … RETURNING id`), `get` (by id+scope), `get_by_name`, …
- `PgRecipeLibrary` / `PgRecipeStoreFacade` — the loop adapter + the WebUI v2 `RecipeStore` port
  facade (the production path; the legacy `StoreBackedRecipeStore` is `#![allow(dead_code)]` with
  `TODO(Phase K): delete this entire module`).

## 4. Behavior / flow

1. **Author/seed:** a recipe is authored in the WebUI (source `authored`) or seeded by the
   builtin bootstrap (source `system`, Phase L) or extracted from a successful thread
   (source `extracted`). New rows start `validation_status='pending'` with `05:validator` in
   `consumer_tags`.
2. **Match:** the intent system returns a `Match` with `component_class_code=21`; the
   orchestrator fetches the recipe by id (via `PgRecipeStore::get`) and reads `step_descriptions`.
3. **Build instructions:** the IBS (`build_instruction`) turns the recipe's `step_link` +
   `step_descriptions` + variable patterns into a `SplitResult` splitting work into a
   **rust_items** channel (ToolSkill bodies for the Rust executor) and an **orchestrator_items**
   channel (Skill + PythonCode bodies for the orchestrator). See `04-ibs.md`.
4. **Dispatch by tier:**
   - **Tier 0** (`is_tier0_eligible`): apply `rust_items` to the Rust execution context; stash
     `orchestrator_items`; reply **without an LLM call** (`tier_zero: true` pkr signal — Phase H).
   - **Tier 1** (matched, not Tier 0): stash both; `PromptStage` injects `orchestrator_items` as
     context; `InterceptorStage` (Sempai) reviews; `ModelStage` calls the LLM guided by the recipe.
   - **Tier 2** (no match): normal prompt assembly + full LLM reasoning; on success, extraction
     lifts the thread into a new Recipe + ToolSkill pair.
5. **Validate & graduate:** the 4-queue lifecycle (Q1 auto / Q2 manual / Q3 revision / Q4
   rejection) gates the recipe; `auto_passed → validated` removes `05:validator`. Wilson score +
   tier are recomputed from `usage/success/failure`. See `14-validation-queue.md`.
6. **Solution Override:** if `override_prompt_creation=true` and `prior_knowledge_content` is
   non-NULL, the recipe's prior-knowledge text is used verbatim instead of assembling from steps
   (§3.13/§3.14) — the Solution-Override LLM path.

## 5. Relations

- **Intent System** (`02-intent-system.md`): a `Match` with class 21 selects a recipe.
- **IBS** (`04-ibs.md`): `build_instruction` turns the recipe's steps into the two-channel
  `SplitResult`.
- **Skills / Tools / PythonCode** (`05`/`06`/`07`): step `include` UUIDs reference ToolSkills
  (class 13), Skills, PythonCode (class 22), etc.
- **Agent Loop** (`12-agent-loop.md`): the `RecipeStage` stage outcome enum (a *different*
  `RecipeStep` in `brassclaw_agent_loop`) gains `TierZero`/`ActionExecuted` variants in Phase H.
- **Validation Queue** (`14-validation-queue.md`): the Q1/Q2 graduation + Wilson scoring.

## 6. Status — today vs. v3

**Today:**
- `reborn_recipes` (V033) and the production `pg_recipe_store.rs` round-trip
  (`PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT`/`decode_recipe_row`/`PgRecipeStore`/
  `PgRecipeLibrary`/`PgRecipeStoreFacade`) **exist**.
- `Recipe`/`RecipeStep`/`ToolSkill` data types exist (`types/recipe.rs`), with `is_tier0_eligible()`.
- **No `step_descriptions` column** — only the legacy `steps` JSONB array. The v3 per-step model
  (`info`/`include`/`codesnippet`, `type: text|component|snippet`) does not exist.
- The agent-loop `RecipeStep` stage enum has only `Continue` (Tier 0/1 dispatch not wired).
- `RecipeStage` is a stub (always returns `Continue` → Tier 2); no recipe is ever selected in
  production because `last_user_text` is missing and `PostgresSource` is not wired.
- The legacy `StoreBackedRecipeStore` (MemoryDoc) is dead code awaiting Phase K deletion.

**v3 plan adds:**
- **Phase A (V050):** add `step_descriptions` JSONB (the per-step `{info, include, codesnippet,
  type}` model), plus `variants` and `dependency_registry` columns, appended at `RECIPE_SELECT`
  indices 31/32/33 (avoids re-indexing the existing 31 `row.get(N)` calls — FIND-P6-04 / H1).
  Extend `PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT`/`decode_recipe_row`/`INSERT` for the full store
  round-trip so the WebUI authoring path works (H1).
- **Phase H:** add `RecipeStep::TierZero`/`RecipeStep::ActionExecuted` to the **agent-loop** stage
  enum (COMP-02: today's single-variant exhaustive match becomes a compile error — intended); wire
  Tier 0/1 dispatch + `last_user_text`.
- **Phase L (V057):** builtin bootstrap seeds system recipes (`source='system'`); V057 adds
  `'system'` to the `source` CHECK on `reborn_recipes` (for documentation/correctness; it works
  today only because there is no CHECK — FIND-P6-02).
- **Phase N (V059):** drop the five per-table queue columns (`validation_errors`=22,
  `review_feedback`=23, `review_attempts`=24, `rejected_at`=25, `queue_code`=26) in favor of the
  central `reborn_validation_queue`; **re-index `decode_recipe_row`** positionally
  (`source` 27→22, `content_hash` 28→23, `created_at` 29→24, `updated_at` 30→25, `step_descriptions`
  31→26, `variants` 32→27, `dependency_registry` 33→28) — FIND-P6-04.

## 7. LLM-relevant summary

A recipe (class 21) is a trigger + ordered steps; each step names the skills/tools/Python code to
preload and how to run them. Recipes are Wilson-scored and tiered (seedling/growing/mature/
candidate). Tier 0 (mature/candidate + validated + wilson ≥ 0.70 + a validation hook) runs with no
LLM; Tier 1 injects known-good patterns into the prompt; Tier 2 is full LLM reasoning and can
extract a new recipe on success. The production store is `pg_recipe_store.rs`
(`PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT` 31 cols/`decode_recipe_row`). v3 Phase A adds the
`step_descriptions`/`variants`/`dependency_registry` columns (at indices 31–33) + the store
round-trip; Phase H adds Tier 0/1 dispatch; Phase L seeds system recipes; Phase N drops the five
per-table queue columns for the central `reborn_validation_queue` and re-indexes the decoder.
