# 03 — Recipe System

> **Subsystem:** Recipes — the unit of v3 orchestration. A recipe is a trigger plus an ordered
> list of steps; each step says exactly which skills/tools/Python code/LLM prompts are needed and
> how the orchestrator and the Rust executor should run it.
> **Grounded in:** `crates/brassclaw_engine/src/types/recipe.rs`,
> `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`,
> `crates/brassclaw_reborn_composition/src/composition.rs`,
> `crates/brassclaw_reborn_composition/src/pg_composition_port.rs`,
> `crates/brassclaw_pg/migrations/V033__reborn_recipes.sql`,
> `crates/brassclaw_pg/migrations/V050__reborn_recipe_step_descriptions.sql`,
> `saved_plan_to_v3.md` (Phases A, HI, C.2, N).

## 1. Purpose

A **recipe** is a reusable, validated solution component (class 21) that drives the "match →
orchestrate" branch. When the intent system matches a user message to a recipe, the **Monty
orchestrator** runs the recipe's steps in order, composing concrete Python for each step and
executing it via `host.run_program`. Each step carries everything needed: which skills/tools/
Python code to preload, how to execute, whether an LLM call is required and how its prompt is
built. Recipes are Wilson-scored and tiered: a high-confidence recipe can execute with **no LLM
round-trip** (Tier 0); a lower-confidence match injects known-good patterns into the prompt
(Tier 1); no match falls through to full LLM reasoning (Tier 2 / Non-Matching-Mode), and a
success can be **extracted** into a new recipe + ToolSkill pair.

**A recipe is a composition, not a monolith (the v3 recycling principle,
`05-skills-system.md` "Recycling").** Each step `include`s **already-existing,
one-purpose library parts** — leaf Skills (one tool each), ToolSkills, PythonCode
— by UUID; the recipe is the *ordering* and the *wiring* (`{{vars.*}}`), not the
capability itself. Prefer reusing a library part over authoring a new one; when a
genuinely new capability is needed, add it as a small leaf so the next recipe can
reuse it too. Never bake a whole procedure into one fat skill — split it into
leaves. (See `DOC_CONVERSION_MECHANISM_DESIGN.md` §4.0/§4.3 for a worked
example: one `doc-convert` recipe composing ~11 reusable leaves + one domain skill.)

## 1.1 Dual-nature syntax (human-readable + machine-readable)

A recipe is **dual-nature**: the same `Recipe` / `RecipeVariant` structs carry
both natures — no separate rendering or transpilation. The `step_link` formula
stays as-is; the dual-nature need is met by a concise **human-readable
explanation of what happens** carried alongside the machine form.

- **Machine-readable exact logic (deterministic, never changed by Step B):**
  `RecipeVariant.step_link` + `Recipe.step_descriptions` → IBS
  `build_instruction` → `BuildInstruction` (`rust_steps` + `orchestrator_steps`).
- **Human-readable explanation (concise — "what happens", not too much detail):**
  `Recipe.description` (recipe-level), `RecipeVariant.description`
  (variant-level — added in Phase HI / Step B), `StepDescriptionEntry.label` +
  `StepEntry.goal` (step-level).
- **Q1 gate (Phase HI / Step B):** a v3-migrated variant (`step_link` present) MUST have a
  non-empty `RecipeVariant.description` (≤ 512 chars); legacy variants
  (`step_link == None`) are exempt. Enforced in
  `RecipeValidator::validate_recipe` (`check_variant_descriptions`).
- **Read surface:** `RecipeDetail.recipe` is opaque full-engine JSON, so new
  variant fields ride along to the WebUI with no DTO recompile. No WebUI
  recipe-authoring route exists yet (future work).

## 2. Location

- **Data types:** `crates/brassclaw_engine/src/types/recipe.rs` (`Recipe`, `RecipeStep`,
  `RecipeVariant`, `ToolSkill`, `RecipeTrigger`, `RecipeValidation`, `ValidationStatus`,
  `RecipeSource`).
- **Production store (Postgres, class 21):** `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`
  (`PgRecipe`, `NewPgRecipe`, `PgRecipeStore`, `RECIPE_SELECT`, `decode_recipe_row`,
  `PgRecipeLibrary`, `PgRecipeStoreFacade`).
- **Composition system (split + assemble):** `crates/brassclaw_reborn_composition/src/composition.rs`
  (`ComposedProgram`, `compose_program`, `ComponentResolver`, `MapComponentResolver`) and
  `crates/brassclaw_reborn_composition/src/pg_composition_port.rs` (`PgCompositionPort` — the
  engine `CompositionPort` impl that runs the 8-step compose pipeline against Postgres).
- **Legacy MemoryDoc-backed store (slated for Phase K deletion):**
  `crates/brassclaw_reborn_composition/src/recipe_store.rs` (`StoreBackedRecipeStore`),
  `crates/brassclaw_reborn_composition/src/recipe_library.rs`.
- **Migrations:** `V033__reborn_recipes.sql` (create, class 21), `V050__reborn_recipe_step_descriptions.sql`
  (adds the three v3 authoring columns), `V046` (Solution Override columns),
  `V061`/`V064` (component-registry touch-ups).
- **Spec:** §3.3, §3.5/§3.5.1 (validation lifecycle), §3.6 (Wilson scoring), §3.7 (class 21),
  §3.9 (consumer_tags), §3.12 (intent_examples), §3.13/§3.14 (Solution Override).

## 3. Data model

### `reborn_recipes` (V033, class 21)

| Column | Idx | Type | Notes |
|--------|-----|------|-------|
| `id` | 0 | UUID PK | `gen_random_uuid()` |
| `tenant_id`,`user_id`,`agent_id`,`project_id` | 1–4 | TEXT NOT NULL | scope tuple |
| `name` | 5 | TEXT NOT NULL | kebab-case `^[a-z0-9]…$`, 1–64; unique per scope |
| `description` | 6 | TEXT NOT NULL | 1–1024 |
| `trigger` | 7 | JSONB | `{type: exact\|pattern\|keyword, …}` |
| `steps` | 8 | JSONB NOT NULL DEFAULT '[]' | ordered legacy step array |
| `status` | 9 | TEXT default 'active' | active/archived/draft |
| `prior_knowledge_content` | 10 | TEXT | §3.13/§3.14 Solution Override (used verbatim instead of assembling from steps); added V046 |
| `override_prompt_creation` | 11 | BOOL default false | enables the Solution Override path; added V046 |
| `class_code` | 12 | SMALLINT default 21 | `CHECK = 21` |
| `prompt_uid` | 13 | BIGINT | monotonic sequence → deterministic assembly order |
| `consumer_tags` | 14 | TEXT[] default '{}' | `^[0-9]{2}(:[a-z0-9-]+)?$`; `05:validator` greys out others until validated |
| `intent_examples` | 15 | JSONB | `[{input, class:1\|2\|3}]` for the intent system (§3.12) |
| `tier` | 16 | TEXT default 'seedling' | seedling/growing/mature/candidate (maturity) |
| `usage_count`,`success_count`,`failure_count` | 17–19 | INT | Wilson inputs |
| `wilson_lower` | 20 | FLOAT8 default 0.0 | Wilson lower bound at last recompute |
| `validation_status` | 21 | TEXT default 'pending' | pending/auto_passed/auto_failed/validated/review_requested/rejected/garbage/upgrade_queued |
| `validation_errors` | 22 | TEXT[] | `[]` — **legacy queue col (Phase N drop pending)** |
| `review_feedback` | 23 | TEXT | **legacy queue col (Phase N drop pending)** |
| `review_attempts` | 24 | SMALLINT default 0 | **legacy queue col (Phase N drop pending)** |
| `rejected_at` | 25 | TIMESTAMPTZ | **legacy queue col (Phase N drop pending)** |
| `queue_code` | 26 | TEXT | q1_auto/q2_manual/q3_revision/q4_rejection/garbage — **legacy queue col (Phase N drop pending)** |
| `source` | 27 | TEXT default 'authored' | authored/extracted/migrated/imported/system — **no CHECK** (V066 widened the CHECK on `reborn_tools`/`reborn_skills` only; `reborn_recipes` never had one, so `'system'` works for the C.2 builtin seeder) |
| `content_hash` | 28 | TEXT | |
| `created_at`,`updated_at` | 29–30 | TIMESTAMPTZ | `set_updated_at()` trigger |
| `step_descriptions` | 31 | JSONB | the authored `StepDescription` array (§0.4.1, the IBS authoring model) — **added V050** |
| `variants` | 32 | JSONB | `Vec<RecipeVariant>` — variant_key, step_link, intent_examples, nested variable_patterns (§0.16.1). **Recipe-specific; no other component table has this column. Added V050.** |
| `dependency_registry` | 33 | JSONB | the component dependency-graph entry (§0.19) — **added V050** |

Unique: `(scope, name)`. Indexes: scope, scope+status, scope+prompt_uid (assembly order),
`consumer_tags` GIN, partial `WHERE validation_status='validated'` on (scope, tier). (The
`consumer_tags` format CHECK was removed — PG16 disallows subqueries in CHECK; enforced at the
app layer.) The five per-table queue columns (22–26) are **still present** — the V072–V075 drops
hit `reborn_skills`/`reborn_actions`/the memory-class tables/`reborn_extensions_unified` only;
`reborn_recipes` was not touched. Their removal (and the positional re-index of
`decode_recipe_row`) is **Phase N**, still pending (next migration V076+).

### Rust types (`types/recipe.rs`)

- `Recipe` — ordered ToolSkill invocations + trigger + Wilson metrics + lifecycle + the v3
  authoring fields `variants: Vec<RecipeVariant>`, `step_descriptions: serde_json::Value`,
  `dependency_registry` (raw JSONB). `is_tier0_eligible()` = tier `mature`/`candidate` **and**
  `Validated` **and** `wilson_lower ≥ 0.70` **and** `validation != None`.
- `RecipeVariant` — `variant_key`, `step_link` (the machine-readable IBS formula), `description:
  Option<String>` (the Phase HI human-readable explanation), `intent_examples`, nested
  `variable_patterns`.
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

- `PgRecipe` — fully decoded row (34 fields, incl. `step_descriptions`/`variants`/
  `dependency_registry` as `Option<Value>`); `has_validator_tag()`, `is_deliverable()` (validated
  + no `05:validator` tag, §3.9 SEC-01), `is_tier0_eligible()` (deliverable + tier mature/candidate).
- `NewPgRecipe` — insert payload (scope, name, description, trigger, steps,
  prior_knowledge_content, override_prompt_creation, consumer_tags, intent_examples, source,
  step_descriptions, variants, dependency_registry).
- `RECIPE_SELECT` — canonical **34-column** list (indices 0–33); the three V050 columns are
  appended at 31/32/33 so the existing `row.get(0..30)` base stays in place.
  `decode_recipe_row` reads positionally `row.get(0..=33)`.
- `PgRecipeStore::insert` writes the three v3 columns at `$14/$15/$16` (`RETURNING id`); `get`
  (by id+scope), `get_by_name`, …
- `PgRecipeLibrary` / `PgRecipeStoreFacade` — the loop adapter + the WebUI v2 `RecipeStore` port
  facade (the production path; the legacy `StoreBackedRecipeStore` is `#![allow(dead_code)]` with
  `TODO(Phase K): delete this entire module`).

## 4. Behavior / flow

1. **Author/seed:** a recipe is authored in the WebUI (source `authored`), seeded by the C.2
   builtin bootstrap (source `system`) — the `host.*` builtins + their leaf Skills are seeded as
   validated system rows — or extracted from a successful thread (source `extracted`). New rows
   start `validation_status='pending'` with `05:validator` in `consumer_tags`.
2. **Match:** Monty calls `host.resolve_intent(user_input)`; the intent system returns a `Match`
   carrying `component_id`, `component_class_code=21`, `step_link`, and `component_name`. (See
   `02-intent-system.md`.)
3. **Compose:** Monty calls `host.compose_orchestrator(component_id, step_link, user_input)`. The
   **composition system** (`PgCompositionPort::compose_program` / `compose_program` in
   `composition.rs`) fetches the recipe by id, matches the variant on `step_link`, resolves every
   step's `include` UUIDs against the component tables, and splits the result into:
   - **`rust_directives`** — ToolSkill / Rust-plugin bodies for the **Rust executioner** (loaded
     as a dynamic cdylib plugin at runtime, a C.5/C.6 concern), and
   - a returned **orchestrator program** — concrete per-step Python for Monty, plus the **skills
     array** (leaf Skill usage docs) Monty consults while stepping, so the step list itself need
     not carry that level of detail.
   See `04-ibs.md` (the IBS `build_instruction` that compiles `step_link` + `step_descriptions` +
   variable patterns into the `rust_steps` + `orchestrator_steps` channels).
4. **Step + run:** Monty steps through the orchestrator program, calling `host.run_program` per
   step (the dynamically-provided code string, with prompt variables like paths bound in). The
   Rust executioner services every concrete tool call under the mandatory orchestrator-command-
   driven rule (`12-agent-loop.md`). Dispatch is tiered:
   - **Tier 0** (`is_tier0_eligible`): deterministic execution, **no LLM call**
     (`tier_zero: true` pkr signal).
   - **Tier 1** (matched, not Tier 0): the orchestrator items inject known-good patterns into the
     assembled prompt; the Sempai interceptor reviews; the LLM is called guided by the recipe.
   - **Tier 2** (no match): Non-Matching-Mode — normal prompt assembly + full LLM reasoning; on
     success, extraction lifts the thread into a new Recipe + ToolSkill pair.
5. **Validate & graduate:** the 4-queue lifecycle (Q1 auto / Q2 manual / Q3 revision / Q4
   rejection) gates the recipe; `auto_passed → validated` removes `05:validator`. Wilson score +
   tier are recomputed from `usage/success/failure`. See `14-validation-queue.md`.
6. **Solution Override:** if `override_prompt_creation=true` and `prior_knowledge_content` is
   non-NULL, the recipe's prior-knowledge text is used verbatim instead of assembling from steps
   (§3.13/§3.14) — the Solution-Override LLM path.

> **Production path note.** The Tier-0 deterministic path is **active today** via the turns
> `PgOrchestratorLookup` bridge (wrapping `TierZeroOrchestrator::run_tier_zero`). The
> engine-resident Monty VM `execute_orchestrator` host-call path that hosts the
> `host.compose_orchestrator` → `host.run_program` loop is **wired at the host-call layer**
> (composition core + `PgCompositionPort` + the orchestrator dispatch arms) but the VM itself is
> **dormant in production** — no `ThreadManager`/`ConversationManager` is constructed by
> `build_reborn_runtime`. Activating that VM (and the dynamic cdylib tool loading) is the **C.5/C.6
> driver** step.

## 5. Relations

- **Intent System** (`02-intent-system.md`): a `Match` (class 21, with `step_link`) selects a
  recipe and hands its id + step_link to `host.compose_orchestrator`.
- **IBS** (`04-ibs.md`): `build_instruction` turns the recipe's `step_link` + `step_descriptions`
  into the two-channel `BuildInstruction` (`rust_steps` + `orchestrator_steps`).
- **Composition System** (`composition.rs` / `pg_composition_port.rs`): the engine `CompositionPort`
  impl that fetches + matches the variant + resolves includes + splits rust/orchestrator and
  returns the per-step program + skills array to Monty.
- **Skills / Tools / PythonCode** (`05`/`06`/`07`): step `include` UUIDs reference ToolSkills
  (class 13), Skills (class 1/10), PythonCode (class 22), etc. Skills ride along as the array
  Monty consults while stepping.
- **Validation Queue** (`14-validation-queue.md`): the Q1/Q2 graduation + Wilson scoring (the five
  per-table queue columns are still on `reborn_recipes` pending Phase N).

## 6. Status — shipped vs. pending

**Shipped:**
- `reborn_recipes` (V033) and the production `pg_recipe_store.rs` round-trip
  (`PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT` 34 cols/`decode_recipe_row`/`PgRecipeStore`/
  `PgRecipeLibrary`/`PgRecipeStoreFacade`), incl. the three V050 authoring columns
  (`step_descriptions`/`variants`/`dependency_registry` at indices 31–33, INSERT `$14–$16`).
- `Recipe`/`RecipeVariant`/`RecipeStep`/`ToolSkill` data types (`types/recipe.rs`), with
  `is_tier0_eligible()`; `Recipe.variants: Vec<RecipeVariant>` + `step_descriptions` +
  `dependency_registry` (raw JSONB).
- **Phase A (V050):** `step_descriptions` + `variants` + `dependency_registry` JSONB columns +
  the full store round-trip (so the WebUI authoring path can read/write them).
- **Phase HI / Step B:** `RecipeVariant.description` + the Q1 `check_variant_descriptions` gate
  (v3 variants require a human-readable explanation; legacy `step_link == None` variants exempt).
- **C.2 builtin bootstrap:** system recipes + the `host.*` Tool rows + their leaf Skills seeded
  as validated `source='system'` rows (no source CHECK needed on `reborn_recipes`).
- **C.4.5.17 composition system:** `compose_program` + `PgCompositionPort` (engine
  `CompositionPort`) + the `host.compose_orchestrator`/`host.run_program` orchestrator arms.
- The Tier-0 deterministic path **active** via the turns `PgOrchestratorLookup` bridge.

**Pending:**
- **C.5/C.6 driver:** activate the engine-resident Monty VM (`ThreadManager`/
  `ConversationManager` wired in `build_reborn_runtime`) so the `host.compose_orchestrator` →
  `host.run_program` loop runs in production; wire dynamic cdylib tool loading for
  `rust_directives`.
- **Phase N (V076+):** drop the five per-table queue columns (`validation_errors`=22,
  `review_feedback`=23, `review_attempts`=24, `rejected_at`=25, `queue_code`=26) in favor of the
  central `reborn_validation_queue` (V051); **re-index `decode_recipe_row`** positionally
  (`source` 27→22, `content_hash` 28→23, `created_at` 29→24, `updated_at` 30→25,
  `step_descriptions` 31→26, `variants` 32→27, `dependency_registry` 33→28).
- **Phase K:** delete the legacy `StoreBackedRecipeStore` (MemoryDoc) dead code.
- **Future:** a WebUI recipe-authoring route (the read surface already rides along via opaque
  `RecipeDetail.recipe`).

## 7. LLM-relevant summary

A recipe (class 21) is a trigger + ordered steps; each step names the skills/tools/Python code to
preload and how to run them. Recipes are Wilson-scored and tiered (seedling/growing/mature/
candidate). On an intent `Match`, Monty calls `host.compose_orchestrator(component_id, step_link,
user_input)`; the composition system fetches the recipe, matches the variant, resolves `include`
UUIDs, and returns concrete per-step Python + a skills array; Monty steps through it via
`host.run_program`. Tier 0 (mature/candidate + validated + wilson ≥ 0.70 + a validation hook) runs
with no LLM; Tier 1 injects known-good patterns into the prompt; Tier 2 is full LLM reasoning and
can extract a new recipe on success. The production store is `pg_recipe_store.rs`
(`PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT` 34 cols/`decode_recipe_row`); the three v3 authoring
columns (`step_descriptions`/`variants`/`dependency_registry`, V050) + `RecipeVariant.description`
(Phase HI) + the C.2 system seed + the C.4.5.17 composition system are shipped. Pending: the C.5/C.6
driver that activates the engine Monty VM in production, and Phase N (drop the five legacy queue
columns + re-index the decoder).
