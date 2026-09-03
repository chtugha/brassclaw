# 04 — Instruction Builder System (IBS) = the Composition System

> **Subsystem:** the Instruction Builder System — compiles a recipe's
> human-editable `step_descriptions` into a machine-optimized `BuildInstruction`
> at intent-match time and splits it into two typed channels (Rust /
> Orchestrator). Per the F4 lock, **the IBS IS the composition system**: the
> `host.compose_orchestrator` host call thin-calls the IBS, which composes the
> matched recipe + variant into the predefined Monty-facing structure
> `{ skills, steplist, rust_directives, variables, assembled_program, tier }`.
> The composer never runs anything and never bakes a single program string —
> Monty iterates the `steplist` and runs each step's `executable_code` via
> `host.run_program` (see `13-orchestrator-default-py.md`, f2).
> **Grounded in:** `crates/brassclaw_engine/src/memory/instruction_builder.rs`,
> `crates/brassclaw_engine/src/types/ibs.rs`,
> `crates/brassclaw_engine/src/memory/composition.rs` (`ComposedProgram`/
> `compose_program`/`ComponentResolver`), `crates/brassclaw_engine/src/executor/
> composition_port.rs` (`CompositionPort` trait),
> `crates/brassclaw_reborn_composition/src/pg_composition_port.rs`
> (`PgCompositionPort`); `saved_plan_to_v3.md` §0.4 / §0.4.1 / §0.6 / §0.7 /
> §0.8 / §0.20, Phases A, E, H, Step C / C.4.5.17.
> **Status:** **shipped.** The IBS core (`instruction_builder.rs` + `types/ibs.rs`)
> landed in Phase A (V050); the composition system (`composition.rs` +
> `CompositionPort` + `PgCompositionPort` + `host.compose_orchestrator`/
> `host.run_program`) landed in C.4.5.17. Live activation is C.5/C.6 (driver).

## 1. Purpose

The IBS is the sole producer of `BuildInstruction`s and — per the F4 lock — **the composition
system**. It does two things:

1. **Compile** a recipe's `step_descriptions` (a JSONB array of human-authored YAML steps) into a
   typed, two-channel `BuildInstruction` at the moment the intent system matches a recipe.
   BuildInstructions are never hand-authored or pre-stored: assembling on match (rather than
   pre-storing) always reads current, validated component UUIDs with zero staleness risk — a
   PythonCode revision does not require a cascade rebuild.
2. **Compose** that `BuildInstruction` (+ the matched variant's `variable_patterns` + the resolved
   included components) into the predefined Monty-facing `ComposedProgram`:
   `{ skills[], steplist[{step_id, instructions, executable_code, tool_bindings}],
   rust_directives[], variables{}, assembled_program, tier }`. This is what
   `host.compose_orchestrator(component_id, step_link, user_input)` returns to Monty.

The two `BuildInstruction` channels are read by two different runtimes in the
Orchestrator/Executioner split: the **Rust Executioner** (`rust_steps` → ToolSkill bodies + tool
bindings, materialised as `rust_directives`/`tool_bindings` in the composed program) and the
**Python Orchestrator** (`orchestrator_steps` → Skill + PythonCode bodies, materialised as
`skills` + each `steplist` step's `executable_code`). The composer itself **never runs anything**
and **never bakes a single program string** — Monty iterates the `steplist`, consults `skills`
for exact tool usage, and runs each step's `executable_code` via `host.run_program`.

## 2. Location

- **IBS builder (shipped, Phase A):** `crates/brassclaw_engine/src/memory/instruction_builder.rs`
  — pure Rust, no async, no DB calls. Exposed as
  `crate::memory::instruction_builder::build_instruction` (+ `capture_variables`/`substitute_vars`).
- **IBS data types (shipped, Phase A):** `crates/brassclaw_engine/src/types/ibs.rs` —
  `VariablePattern`, `ToolBinding`, `ErrorPolicy` (JSONB-persisted data-model types).
- **Composition core (shipped, C.4.5.17):** `crates/brassclaw_engine/src/memory/composition.rs`
  — `ComposedProgram`, `ComposedStep`, `RustDirective`, `SkillRef`, `ComponentResolver` (trait),
  `ResolvedComponent`, `compose_program` (pure; binds `{{vars.NAME}}`).
- **Composition port — engine side (shipped, C.4.5.17):**
  `crates/brassclaw_engine/src/executor/composition_port.rs` — the `CompositionPort` trait +
  `CompositionPortError` (`Unavailable`/`RecipeNotFound`/`NoVariantMatch`/`Failure`).
- **Composition impl — the IBS over Postgres (shipped, C.4.5.17):**
  `crates/brassclaw_reborn_composition/src/pg_composition_port.rs` — `PgCompositionPort` (owns
  `Arc<PgPool>`); runs the recipe SELECT → variant match by `step_link` → `build_instruction` →
  `capture_variables` → resolve include/tool UUIDs → `compose_program` pipeline.
- **Host-call handlers (shipped, C.2/C.4.5.17):** `crates/brassclaw_engine/src/executor/orchestrator.rs`
  — `host.compose_orchestrator` (thin-calls `CompositionPort::compose`) + `host.run_program`
  (nested `execute_code`).
- **Callers:** `PostgresSource::fetch_for_turn` / `fetch_recipe_split_result`
  (`crates/brassclaw_engine/src/memory/retrieval_source.rs`) call `build_instruction` synchronously
  after an intent match resolves to a recipe (class 21), then `fetch_component_by_id` for every
  emitted UUID. `PgCompositionPort::compose` is the composition-system entry called by the
  `host.compose_orchestrator` handler.
- **Storage:** `step_descriptions` JSONB on `reborn_recipes` (V050); `step_link` TEXT on
  `reborn_intent_inputs` (V054); `variants` JSONB (V050) carrying nested `variable_patterns`.
  Per-class DB-structure standardisation through V075 (C.4.5.0–C.4.5.16).

## 3. Data model

### Type homes (Decision 1 / FIND-NEW-01)

| Module | Types |
|--------|-------|
| `types/ibs.rs` (new) | `VariablePattern`, `ToolBinding`, `ErrorPolicy` — data-model, persisted in JSONB |
| `types/recipe.rs` (modified) | `RecipeVariant` + three new `Recipe` fields (`step_descriptions`, `variants`, `dependency_registry`) |
| `memory/instruction_builder.rs` (new) | `BuildInstruction`, `RecipeStepType`, `StepOwner`, `IbsRecipeStep`, `IbsError`, `DependencyExpr`, `DependencyNode`, `StepDescriptionEntry`, `StepContextSpec` |

### `VariablePattern` (`types/ibs.rs`)

```rust
pub struct VariablePattern {
    pub name: String,              // slot name, e.g. "dir"; matches {{vars.NAME}}
    pub pattern: Option<String>,   // regex applied after positional extraction to validate/transform
    pub description: Option<String>, // WebUI help text only
}
```
Stored nested inside each `RecipeVariant` in the `variants` JSONB column — **not** a top-level
column. Phase E deserializes `variants → Vec<RecipeVariant>`, finds the variant matching the
`step_link`, and extracts its `variable_patterns` (FIND-P6-06).

### `ToolBinding` + `ErrorPolicy` (`types/ibs.rs`)

```rust
pub struct ToolBinding {
    pub tool_id: uuid::Uuid,       // class-0 Tool row UUID (capability dispatch)
    pub tool_name: String,         // denormalized, e.g. "read_file"; for __execute_action__ w/o DB fetch
    pub params: serde_json::Value, // {{vars.name}} substitution applied before use
    pub error_policy: ErrorPolicy,
}

#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    Fail,                          // hard error, no retry (default)
    Ignore,                        // continue; orchestrator gets empty result
    Retry { max_attempts: u32 },   // retry then Fail
    Fallback { step_id: String },  // jump to step_id in the same BuildInstruction
}
```
Persisted in the `step_descriptions` JSONB inside rust-channel `IbsRecipeStep.tool_bindings`
(FIND-AUDIT-10/11 — these are the canonical definitions).

### `BuildInstruction`

```
BuildInstruction
├── llm_call_required: bool          ← false for Tier-0/Actions; true for Tier-1+
├── variable_patterns[]              ← applied before any channel is read
├── basic_prompt_section_refs[]      ← navigation hints into cached base-prompt (no re-fetch)
├── rust_steps[]                     ← CHANNEL R: Rust execution layer reads this only
│   └── IbsRecipeStep { step_id, knowledge: Rust|Both,
│                        include: Vec<Uuid>,            // ToolSkill UUIDs
│                        tool_bindings: Vec<ToolBinding> }
└── orchestrator_steps[]            ← CHANNEL O: serialized into orchestrator_content
    └── IbsRecipeStep { step_id, knowledge: Orchestrator|Both,
                        step_type: Text|Component,
                        include: Vec<Uuid>,            // Skill / PythonCode UUIDs
                        info: Option<String> }         // WebUI annotation only; NOT emitted
```

The per-step struct is **`IbsRecipeStep`** (renamed from `RecipeStep` to avoid colliding with the
existing v2 `RecipeStep { skill, tool, params, description }` in `types/recipe.rs` — FIND-NEW-02).

**Channel invariants:** channels must not overlap. A ToolSkill UUID must never appear in
`orchestrator_steps`; a Skill UUID must never appear in `rust_steps`. Step **type** and component
**class** are orthogonal: the type determines IBS handling; the class + the step's `knowledge`
field determine channel routing.

### `step_descriptions` JSONB (`reborn_recipes`, V050)

Each array element holds **two synced representations** of one StepDescription:

```json
[{
  "desc_idx": 0,
  "label": "base path (ls -l, current directory)",
  "yaml_source": "steps:\n  - stepnumber: 1\n    knowledge: orchestrator\n …",
  "steps": [ { "stepnumber": 1, "knowledge": "orchestrator", "goal": …,
               "content": …, "type": "text", "info": …, "include": [], "dependencies": "" } ]
}]
```

- `yaml_source` — raw YAML as typed by the author; WebUI renderer only; **never read by the IBS**.
- `steps` — pre-parsed structured array; used **exclusively** by the IBS; written by the WebUI on
  save. The IBS never parses YAML at runtime. If `yaml_source` fails to parse, the save is rejected
  before Q1.

**Per-step fields:**

| Field | Required | Meaning |
|-------|----------|---------|
| `stepnumber` | yes | 1-based ordinal within this StepDescription's sequence |
| `knowledge` | yes | `orchestrator` \| `rust` \| `both` — which channel reads the step |
| `goal` | yes | human-readable what this step accomplishes |
| `content` | yes | short description of step content |
| `type` | yes | `text` \| `component` \| `snippet` (IBS treatment, see below) |
| `info` | no | WebUI documentation; **not emitted to the orchestrator** |
| `include` | no | component UUIDs needed at this step; IBS emits a fetch per UUID |
| `codesnippet` | no | inline Python → creates a PythonCode (class 22, `pending`) on save → Q1; promoted to `type:"component"` on Q1+Q2 pass (Phase N/V059 gate) |
| `dependencies` | no | traversal into the component's `dependency_registry` (§0.19), e.g. `"1[all], 5[2,6], 17[3, 7[1,4]]"`; resolved at fetch time |

**Step types:**

| Type | IBS behavior |
|------|--------------|
| `text` | Authoring annotation only — no fetch, no runtime emission. A `text` step with no `info` is a Q1 **warning**, not an error. |
| `component` | Emit a fetch for each `include` UUID; route to rust or orchestrator channel by `knowledge`. |
| `snippet` | WebUI authoring shortcut. IBS **refuses to assemble** — returns `IbsError::UnpromotedSnippet`; must be promoted to `component` after the created PythonCode passes Q1+Q2. |

### `step_link` notation (`reborn_intent_inputs.step_link`, V054)

```
step_link = "{desc_idx}:{start}-{desc_idx}:{end}" [+ more segments]
  {desc_idx} = 0-based index into the step_descriptions JSONB array
  {start}/{end} = stepnumber (1-based) | 0 (sentinel = first) | E (sentinel = last)
  + = concatenate segments in order
```

| Formula | Meaning |
|---------|---------|
| `0:0-0:E` | all steps of StepDescription0 (single-variant) |
| `0:0-0:30+1:0-1:E` | SD0 steps 0–30 (shared prefix) + all of SD1 |
| `0:0-0:31+2:0-2:E` | SD0 steps 0–31 + all of SD2 |

`step_link` is **nullable**; a NULL value means a legacy intent — the IBS is skipped and
`fetch_for_turn` falls through to the existing `fetch_component_by_id` path. `step_link`
replaces the old `variant_key`/`link_formula` columns (no `variant_key` column exists).

## 4. Behavior / flow

### `build_instruction` (assembly algorithm)

```rust
pub fn build_instruction(
    step_link:          &str,
    step_descriptions:   &[StepDescriptionEntry],  // from JSONB
    variable_patterns:   &[VariablePattern],
) -> Result<BuildInstruction, IbsError>;
```

1. **Parse** `step_link` → `Vec<StepRange>` (e.g. `[(desc_idx:0, 0..=30), (desc_idx:1, 0..=E)]`).
2. **Select** `steps[start..=end]` from each `step_descriptions[desc_idx]` and append in order.
3. **Classify** each step: `text` → skip (no emission); `component` → emit fetch + route by
   `knowledge`; `snippet` → `Err(UnpromotedSnippet)`.
4. **Validate:** monotonic `stepnumber`s; rust-channel steps are `type:"component"` with non-empty
   `include`; all `include` UUIDs are valid v4; the **S7 guard** (rust `tool_bindings` present ⇒
   `orchestrator_steps` has ≥1 skill_id for Tier 1; for Tier 0 — `llm_call_required==false` and
   rust has `tool_bindings` ⇒ `orchestrator_steps` has ≥1 **PythonCode** UUID (class 22), not a
   Skill, because Skill bodies need an LLM interpreter; empty is a Q1 hard error); parse each
   `dependencies` string into a `DependencyExpr` tree (parse errors are hard; out-of-range indices
   are checked later at Q1, not here).
5. **Partition:** `rust_steps[]` ← `knowledge ∈ {rust, both}`; `orchestrator_steps[]` ←
   `knowledge ∈ {orchestrator, both}`.
6. **Attach** the parsed `DependencyExpr` to each step that declared `dependencies`. The IBS does
   **not** resolve UUIDs — it only parses the expression into a typed tree; recursive DB resolution
   happens later in `fetch_for_turn` (§0.19, `15-component-catalog.md`).
7. **Return** `BuildInstruction { rust_steps, orchestrator_steps, variable_patterns,
   basic_prompt_refs, llm_call_required }`.

**No `fetch_by_instruction` method.** The IBS runs synchronously *inside* `fetch_for_turn`, not as
a separate retrieval pass. `fetch_for_turn` calls `build_instruction` then immediately calls
`fetch_component_by_id` for every emitted UUID → `FetchForTurnResult::SplitResult`.

### `StepContextSpec` (derived, computed at fetch time)

The formatter in `handle_assemble_prior_knowledge` derives a context type from each fetched
component's `class_code` (authors never set it) and emits a labelled block into `orchestrator_content`:

| `class_code` | Class | `StepContextSpec` | Heading |
|---|---|---|---|
| 1–3 | Skill | `Skill` | `## [Skill: {name}]` |
| 12 | Spec | `Spec` | `## [Spec: {name}]` |
| 13 | ToolSkill | *(never in orchestrator channel — Q1 hard error)* | — |
| 21 | Recipe | `Recipe` | `## [Recipe: {name}]` |
| 22 | PythonCode | `PythonCode` | `## [PythonCode: {name}]` |
| 23 | ExtensionCatalogue | `Catalogue` | `## [Catalogue: {name}]` |
| *(text step)* | — | `Annotation` | *(nothing emitted)* |

`orchestrator_content` is therefore self-describing; authors do not add type headers to component
bodies. **Invariant: ToolSkill is never in `orchestrator_items`.**

### `SplitResult` + `TurnRoutingSignals`

```rust
pub enum FetchForTurnResult {
    Components(Vec<ComponentItem>),                 // NoMatch UNION ALL / non-recipe
    Disambiguation(Vec<IntentCandidate>),
    ActionShortCircuit { component_id: Uuid, name: String }, // class 16 — execute, no LLM
    SplitResult {                                  // class 21 with a step_link
        rust_items:         Vec<ComponentItem>,    // ToolSkill bodies — Rust only
        orchestrator_items: Vec<ComponentItem>,    // Skill + PythonCode bodies
        routing:            TurnRoutingSignals,
    },
}

pub struct TurnRoutingSignals {
    pub override_prompt_creation: bool,
    pub matched_component_ids:    Vec<String>, // orchestrator-channel UUIDs (for _set_active_skills)
    pub variant_label:            String,
    pub step_link:                String,
    pub llm_call_required:        bool,
    pub wilson_lower:             f64,
    pub tier0_eligible:           bool,         // full Recipe::is_tier0_eligible (see note)
}
```

**No `reborn_pending_rust_context` transient table** — `rust_items` are delivered directly into the
Rust execution context by `RecipeStage` (Phase H); no DB round-trip for rust-channel delivery.

> **Tier-0 eligibility discrepancy (plan §0.8):** `PgRecipe::is_tier0_eligible()` (in
> `pg_recipe_store.rs`) only checks `is_deliverable() && tier ∈ {mature, candidate}` — it omits the
> `wilson_lower ≥ 0.70` and validation-hook guards. The v3 `TurnRoutingSignals.tier0_eligible` must
> use the **full** `Recipe::is_tier0_eligible()` from `types/recipe.rs`. Phase E must compute this
> correctly when building `TurnRoutingSignals`.

### Memoisation

- **Key:** `sha256(step_link + "|" + sha256(step_descriptions_json) + "|" + sha256(variable_patterns_sorted_json))`.
  `variable_patterns` are sorted by `name` before hashing (stability under authoring order changes).
  > **DESIGN-02** — the original key `sha256(step_link + "|" + sorted_include_uuids + "|" + …)` was
  > **circular**: computing `sorted_include_uuids` requires the IBS to have already compiled the
  > `BuildInstruction`. The corrected key uses `step_descriptions_hash`, which is fully computable
  > from the Recipe row *before* compilation, and is invalidated by any StepDescription or
  > `variable_patterns` change (the `step_link` embeds which steps are selected).
- **Eviction triggers:** (1) any `include`d component's `updated_at` changes (tracked via the
  `last_graduation_at` scope cursor, §0.18 / `14-validation-queue.md`); (2) the Recipe's own
  `updated_at` changes (StepDescription edited or `variable_patterns` changed).
- **Cache miss:** safe under high concurrency — compilation is pure computation (no HTTP, no DB);
  concurrent misses compile redundantly and the last writer wins the slot (idempotent).

### `IbsError`

```rust
pub enum IbsError {
    UnpromotedSnippet { step_id: String },
    InvalidUuid { step_id: String, value: String },
    StepOrderViolation { desc_idx: usize, stepnumber: u32 },
    UnknownDescIdx { desc_idx: usize },
    ParseError { formula: String, reason: String },
    S7Violation,                       // rust tool_bindings present but no orchestrator skill_ids
    InvalidDependencyExpr { step_id: String, reason: String },
}
```

## 4a. The composition system (C.4.5.17 — the IBS IS the composition system)

`build_instruction` produces the typed two-channel `BuildInstruction`. The **composition system**
turns that into the predefined Monty-facing `ComposedProgram` that
`host.compose_orchestrator(component_id, step_link, user_input)` returns.

### `ComposedProgram` (`memory/composition.rs`)

```
ComposedProgram
├── skills: Vec<SkillRef>            ← first-class array Monty consults while stepping
│   └── SkillRef { id, class_code, name, body }   (Skill / ToolSkill bodies)
├── steplist: Vec<ComposedStep>      ← Monty iterates this; runs each executable_code
│   └── ComposedStep { step_id, instructions, executable_code, tool_bindings }
├── rust_directives: Vec<RustDirective>  ← cdylib load directives for the Executioner
│   └── RustDirective { tool_id, tool_name, artifact_path }
├── variables: Vec<(String, String)> ← {{vars.NAME}} slots bound from user_input
├── assembled_program: String        ← the human-readable assembled view (NOT run as one program)
└── tier: String                     ← "tier0" | "tier1" …
```

### `ComponentResolver` (trait, `memory/composition.rs`)

A **sync** trait: `resolve(&self, id: Uuid) -> Option<ResolvedComponent>`. The composition impl
pre-populates a `MapComponentResolver` (`HashMap`-backed) with every included component fetched
in one batched `fetch_components_by_ids` pass, so `compose_program` resolves includes with no DB
round-trip. `ResolvedComponent` carries the component's `class_code` + `effective_content`;
`compose_program` routes by class: PythonCode (22) → `executable_code`; Skill/ToolSkill (1/2/3/13)
→ `skills`; Tool (0) → `rust_directives`.

### `compose_program` (pure, `memory/composition.rs`)

Takes the `BuildInstruction` + a `ComponentResolver` + captured `variables` and produces the
`ComposedProgram`. It **binds `{{vars.NAME}}`** (data substitution only; baked as JSON-encoded
Python string literals to prevent injection) and **inlines `{{component_name}}` includes**
(structural include — the referenced mini-PythonCode component's body, one function each).
It never runs anything and never bakes a single program string: `assembled_program` is a
human-readable view, not the run unit — Monty runs each `steplist` step's `executable_code`
individually via `host.run_program`.

### `CompositionPort` + `PgCompositionPort` (the live pipeline)

- **Engine side** (`executor/composition_port.rs`): the `CompositionPort` trait —
  `compose(&self, scope, component_id, step_link, user_input) -> BoxFuture<Result<ComposedProgram,
  CompositionPortError>>`. `CompositionPortError`: `Unavailable` (no bridge wired),
  `RecipeNotFound`, `NoVariantMatch`, `Failure`. The `host.compose_orchestrator` handler
  thin-calls this; `None` port → `{ok:false, error:"composition_unavailable"}` (degrade to
  Non-Matching-Mode).
- **Composition impl** (`pg_composition_port.rs`): `PgCompositionPort` owns `Arc<PgPool>` and
  runs: recipe SELECT (scope) → `tier0_eligible` (tier/validation/wilson ≥ 0.70) →
  `llm_call_required` → variant match by `step_link` (`NoVariantMatch` if none) →
  `build_instruction` → `capture_variables(user_input, …)` → collect orchestrator+rust include
  UUIDs + rust `tool_bindings` tool_ids → `lookup_component_class` + `fetch_components_by_ids`
  (batched) → `MapComponentResolver` → `compose_program` → `ComposedProgram`.
  `RustDirective.artifact_path` is empty until the C.5/C.6 loader applies `rust_directives`
  (V071 dropped `cdylib_artifact_path`; class-0 tools carry no prompt text).

### Activation

The engine Monty VM `execute_orchestrator` host-call path (and thus `host.compose_orchestrator` /
`PgCompositionPort`) is constructed + unit-tested but **inert in production until the C.5/C.6
driver** wires `PgCompositionPort` into `ThreadManager` and applies `rust_directives` via the C.3
`DynamicToolLoader`. The live Tier-0/Tier-1 path today runs through the turns
`PgOrchestratorLookup` bridge. `#![allow(dead_code)]` covers the inert window.

## 5. Relations

- **Recipe System** (`03-recipe-system.md`): owns `step_descriptions`/`variants`/`dependency_registry`;
  the IBS is the consumer.
- **Intent System** (`02-intent-system.md`): `step_link` lives on `reborn_intent_inputs` (Phase D);
  a `Match` (class 21) with a non-NULL `step_link` triggers the IBS; `host.resolve_intent` surfaces
  `component_id` + `step_link` to `host.compose_orchestrator`.
- **Retrieval System** (`11-retrieval-system.md`): `PostgresSource::fetch_for_turn` /
  `fetch_recipe_split_result` is the legacy caller; `PgCompositionPort::compose` is the
  composition-system entry; `SplitResult` is the `FetchForTurnResult` variant.
- **Skills / PythonCode** (`05`/`07`): `orchestrator_steps` carry Skill + PythonCode UUIDs;
  `codesnippet` creates PythonCode (class 22) pending Q1/Q2; `skills` + `executable_code` are the
  composed materialisations.
- **Actions** (`08-actions-system.md`): `ActionShortCircuit` (class 16) is the no-LLM sibling of
  `SplitResult`.
- **Validation Queue** (`14-validation-queue.md`): `snippet`→`component` promotion is gated by
  Q1/Q2 (Phase N); the memo cache evicts on component `updated_at`/`last_graduation_at`.

## 6. Status — shipped vs. pending

**Shipped:**
- **Phase A (V050):** `instruction_builder.rs` + `types/ibs.rs` (`VariablePattern`/`ToolBinding`/
  `ErrorPolicy`); `step_descriptions`/`variants`/`dependency_registry` on `reborn_recipes`; the
  `PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT`/`decode_recipe_row`/`INSERT` store round-trip.
- **Phase D (V054):** `step_link TEXT` on `reborn_intent_inputs` (nullable; NULL ⇒ legacy
  fall-through). `resolve_intent` returns `step_link` + `component_name` on a `Match`.
- **Phase E:** `PostgresSource::fetch_for_turn` calls `build_instruction`, deserializes
  `variants → Vec<RecipeVariant>` to extract `variable_patterns`, fetches each UUID, and returns
  `SplitResult` with the full `TurnRoutingSignals` (incl. correct `tier0_eligible`).
- **Phase H:** `RecipeStage` / `TierZeroExecutionStage` consume `SplitResult`; Tier 0 returns
  without an LLM call, Tier 1 proceeds to `PromptStage`→`InterceptorStage`→`ModelStage`.
- **C.4.5.17:** the composition system — `composition.rs` (`ComposedProgram`/`compose_program`/
  `ComponentResolver`), `CompositionPort` trait, `PgCompositionPort` impl, `host.compose_orchestrator`
  + `host.run_program` handlers. Per-class DB-structure standardisation through V075
  (C.4.5.0–C.4.5.16). DB-less mode removed.

**Pending:**
- **C.5/C.6:** the driver that wires `PgCompositionPort` into `ThreadManager` + applies
  `rust_directives` via the C.3 `DynamicToolLoader`, activating the engine Monty VM host-call path
  as the primary driver (today inert; the turns `PgOrchestratorLookup` bridge is the live path).
- **C.7 / Phase A (reshaped H.12.6):** final cleanup after C.

## 7. LLM-relevant summary

The IBS — **the composition system** (F4) — compiles a recipe's `step_descriptions` JSONB (V050)
into a two-channel `BuildInstruction` at intent-match time, then composes it (+ the matched
variant's `variable_patterns` + resolved includes) into the predefined `ComposedProgram`
`{ skills, steplist, rust_directives, variables, assembled_program, tier }` returned by
`host.compose_orchestrator(component_id, step_link, user_input)`. Channel R (Rust Executioner) gets
`rust_steps` → `rust_directives` (ToolSkill UUIDs + `ToolBinding`s
`{tool_id, tool_name, params, error_policy}`, applied as cdylib load directives by the C.3
`DynamicToolLoader`); Channel O (Python Orchestrator) gets `orchestrator_steps` → `skills` (the
first-class array Monty consults while stepping) + each `steplist` step's `executable_code`.
`step_link` (`{desc}:{start}-{desc}:{end}+…`, on `reborn_intent_inputs` V054) selects the steps;
`variable_patterns` (nested in `variants`) drive `{{vars.NAME}}` substitution (data only; baked as
JSON-encoded Python string literals). BuildInstructions are never pre-stored (zero staleness) and
memoised by `sha256(step_link | step_descriptions_hash | variable_patterns_hash)` (DESIGN-02 fixed
the circular key). The S7 guard enforces that rust `tool_bindings` always come with an orchestrator
channel (Skill for Tier 1; PythonCode for Tier 0). The composer never runs anything and never bakes
a single program string — Monty iterates the `steplist` and runs each step's `executable_code` via
`host.run_program`. The IBS core is Phase A; `step_link` is Phase D; the caller wiring is Phase E;
the composition system + `host.*` handlers are C.4.5.17; live activation is C.5/C.6.
