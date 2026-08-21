# 04 — Instruction Builder System (IBS)

> **Subsystem:** The Instruction Builder System — compiles a recipe's human-editable
> `step_descriptions` into a machine-optimized `BuildInstruction` at intent-match time and
> splits it into two typed channels (Rust / Orchestrator).
> **Grounded in:** `saved_plan_to_v3.md` §0.4 / §0.4.1 / §0.6 / §0.7 / §0.8 / §0.20, Phases A, E, H.
> **Status:** **v3-only — does not exist in the current codebase.** Both
> `crates/brassclaw_engine/src/memory/instruction_builder.rs` and
> `crates/brassclaw_engine/src/types/ibs.rs` are absent today (FIND-NEW-PASS14-03).

## 1. Purpose

The IBS is the sole producer of `BuildInstruction`s. It **compiles** a recipe's
`step_descriptions` (a JSONB array of human-authored YAML steps) into a typed, two-channel
instruction at the moment the intent system matches a recipe. BuildInstructions are never
hand-authored or pre-stored: assembling on match (rather than pre-storing) always reads current,
validated component UUIDs with zero staleness risk — a PythonCode revision does not require a
cascade rebuild. The two channels are read by two different runtimes: the **Rust execution
layer** (ToolSkill bodies + tool bindings) and the **Python orchestrator** (Skill + PythonCode
bodies serialized into `orchestrator_content`).

## 2. Location

- **New module (builder):** `crates/brassclaw_engine/src/memory/instruction_builder.rs` — pure
  Rust, no async, no DB calls. Exposed as `crate::memory::instruction_builder::build_instruction`.
- **New module (data types):** `crates/brassclaw_engine/src/types/ibs.rs` — `VariablePattern`,
  `ToolBinding`, `ErrorPolicy` (JSONB-persisted data-model types). `types/mod.rs` gains
  `pub mod ibs`.
- **Caller:** `PostgresSource::fetch_for_turn` (`crates/brassclaw_engine/src/memory/retrieval_source.rs`)
  — calls `build_instruction` synchronously after an intent match resolves to a recipe (class 21),
  then immediately calls `fetch_component_by_id` for every emitted UUID.
- **Storage:** `step_descriptions` JSONB on `reborn_recipes` (V050, Phase A); `step_link` TEXT on
  `reborn_intent_inputs` (V054, Phase D); `variants` JSONB (V050) carrying nested `variable_patterns`.
- Both modules are **absent** today and must be created by Phase A.

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

## 5. Relations

- **Recipe System** (`03-recipe-system.md`): owns `step_descriptions`/`variants`/`dependency_registry`;
  the IBS is the consumer.
- **Intent System** (`02-intent-system.md`): `step_link` lives on `reborn_intent_inputs` (Phase D);
  a `Match` (class 21) with a non-NULL `step_link` triggers the IBS.
- **Retrieval System** (`11-retrieval-system.md`): `PostgresSource::fetch_for_turn` is the caller;
  `SplitResult` is the new `FetchForTurnResult` variant (Phase E).
- **Skills / PythonCode** (`05`/`07`): `orchestrator_steps` carry Skill + PythonCode UUIDs;
  `codesnippet` creates PythonCode (class 22) pending Q1/Q2.
- **Actions** (`08-actions-system.md`): `ActionShortCircuit` (class 16) is the no-LLM sibling of
  `SplitResult`.
- **Validation Queue** (`14-validation-queue.md`): `snippet`→`component` promotion is gated by
  Q1/Q2 (Phase N); the memo cache evicts on component `updated_at`/`last_graduation_at`.

## 6. Status — today vs. v3

**Today:** the IBS **does not exist**. `instruction_builder.rs` and `types/ibs.rs` are absent
(FIND-NEW-PASS14-03); `step_descriptions`/`variants`/`dependency_registry` columns do not exist on
`reborn_recipes`; `step_link` does not exist on `reborn_intent_inputs`; `FetchForTurnResult` has
only `Components`/`Disambiguation`; the production backend is `RamSource` (no intent path), so the
IBS would never be reached even if it existed. `RecipeStep` in `types/recipe.rs` is the legacy
`{skill, tool, params, description}` Tier-1/2 fallback shape — unrelated to `IbsRecipeStep`.

**v3 plan adds:**
- **Phase A (V050):** create `instruction_builder.rs` + `types/ibs.rs` (`VariablePattern`/
  `ToolBinding`/`ErrorPolicy`); add `step_descriptions`/`variants`/`dependency_registry` to
  `reborn_recipes`; extend `PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT`/`decode_recipe_row`/`INSERT`
  for the store round-trip (H1).
- **Phase D (V054):** add `step_link TEXT` to `reborn_intent_inputs` (nullable; NULL ⇒ legacy
  fall-through). `resolve_intent` returns `step_link` + `component_name` on a `Match`.
- **Phase E:** `PostgresSource::fetch_for_turn` calls `build_instruction`, deserializes
  `variants → Vec<RecipeVariant>` to extract `variable_patterns` (FIND-P6-06), fetches each UUID,
  and returns `SplitResult` with the full `TurnRoutingSignals` (incl. correct `tier0_eligible`).
- **Phase H:** `RecipeStage` (engine path A) / `TierZeroExecutionStage` (agent-loop path B/C)
  consume `SplitResult` — apply `rust_items` silently; stash `orchestrator_items` as
  `orchestrator_content`; Tier 0 returns without an LLM call (`tier_zero: true`), Tier 1 proceeds to
  `PromptStage`→`InterceptorStage`→`ModelStage`.

## 7. LLM-relevant summary

The IBS compiles a recipe's `step_descriptions` JSONB (V050) into a two-channel `BuildInstruction`
at intent-match time, synchronously inside `fetch_for_turn`. Channel R (Rust) gets `rust_steps`
with ToolSkill UUIDs + `ToolBinding`s (`{tool_id, tool_name, params, error_policy}`); Channel O
(orchestrator) gets `orchestrator_steps` with Skill/PythonCode UUIDs, serialized by
`handle_assemble_prior_knowledge` into self-describing `orchestrator_content` (headers derived
from `class_code` via `StepContextSpec`). `step_link` (`{desc}:{start}-{desc}:{end}+…`, on
`reborn_intent_inputs` V054) selects the steps; `variable_patterns` (nested in `variants`) drive
`{{vars.NAME}}` substitution. BuildInstructions are never pre-stored (zero staleness) and memoised
by `sha256(step_link | step_descriptions_hash | variable_patterns_hash)` (DESIGN-02 fixed the
circular key). The S7 guard enforces that rust tool_bindings always come with an orchestrator
channel (Skill for Tier 1; PythonCode for Tier 0). Result is `FetchForTurnResult::SplitResult`
{ `rust_items`, `orchestrator_items`, `TurnRoutingSignals` } — rust delivered to the execution
context with no DB round-trip. The IBS, `types/ibs.rs`, and all three recipe columns are v3-only
(Phase A); `step_link` is Phase D; the caller wiring is Phase E; the consumers are Phase H.
