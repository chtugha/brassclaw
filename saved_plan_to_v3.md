# Recipe System Finalisation Plan — v3

> **Status:** Draft — for review before implementation begins.  
> **Scope:** Closes all architectural gaps identified in the Vision vs. Implementation analysis.  
> **No code changes are made by this document.**  
> **Next migration:** V047 (current highest: V046).

---

## Working Rules

**Critical constraints for implementation:**
- Do implementations **one by one**, never batch or parallelize.
- After each phase is fully resolved: mark it **[DONE]** in this plan, commit + push to `origin/main`, then continue.
- Address everything encountered, even if out of scope or pre-existing — **never suppress/silence**.
- If a stub needs replacement: implement fully, trace the logic to all impacted locations **before** changing code.
- If a fix is complex: write a `subplan_stepX_of_v3.md` or `plan_stub_stepX_v3.md`, execute it, then resume the original step.
- Never delete upgrades found that aren't in the plan — **document and repair** them instead.
- **Do NOT run `git stash`** — commit everything, always.

---

## 0. Architecture Vision (Canonical Reference)

### 0.1 Component Hierarchy — Bottom to Top

Reading bottom-up is the correct direction. Every higher layer is composed *of* the layers below it.

```
┌─────────────────────────────────────────────────────────────────┐
│  ExtensionCatalogue (class 23)                                  │
│  Domain overview. task_groups[] → recipe names. Never re-docs.  │
├─────────────────────────────────────────────────────────────────┤
│  Recipe (class 21)                                              │
│  Primary intent target. One RecipeVariant per distinct intent.  │
│  Each variant owns: intent_examples[], step_link formula,       │
│  variable_patterns[], and StepDescriptions (the authoring       │
│  source from which the BuildInstruction is assembled by the IBS)│
├─────────────────────────────────────────────────────────────────┤
│  Skill (classes 1–3)    │  PythonCode (class 22) [NEW]          │
│  Orchestrator instruct. │  Python utilities / inline instruct.  │
│  for using one Rust tool│  for the orchestrator. Not full Skill.│
├─────────────────────────────────────────────────────────────────┤
│  ToolSkill (class 13)                                           │
│  Rust-layer only. param schema, preconditions, error handling.  │
│  The orchestrator never reads ToolSkill bodies directly.        │
├─────────────────────────────────────────────────────────────────┤
│  Tool (class 0)                                                 │
│  Rust execution layer only. No prompt text. Opaque to the       │
│  orchestrator. Excluded from all retrieval queries.             │
└─────────────────────────────────────────────────────────────────┘
```

**Runtime Extensions (classes 4–9)** remain as-is: MCP servers, Rusty capabilities,
Monty plans, LLM prompt templates. They are NOT documentation containers.

**ExtensionCatalogues (class 23)** are the documentation namespace. Separate class,
separate table, separate concern.

---

### 0.2 ExtensionCatalogue — Correct Design

An ExtensionCatalogue does **not** re-document commands. Every component it owns already
documents itself. The Catalogue draws the **bigger picture**:
> "This catalogue covers local file management. Its Recipes handle these task groups..."

| Section | Content |
|---------|---------|
| `name` | Catalogue identifier |
| `version` | Semver-like label |
| `description` | One-paragraph summary for LLM fallback context |
| `task_groups[]` | `{ group_name, summary, recipe_ids[] }` |
| `child_component_ids[]` | All owned component UUIDs (any class) for lineage |
| `intent_index[]` | Audit-only — never seeded into `reborn_intent_inputs` |

---

### 0.3 Recipe — Correct Design

A Recipe is a **complete turn script**. It is the primary intent target.

**Important — current vs. target state:**  
The live `Recipe` struct in `crates/brassclaw_engine/src/types/recipe.rs` is the
**v2 design**: `RecipeStep { skill: String, tool: String, params, description }`.
There is no `RecipeVariant`, `BuildInstruction`, `StepDescription`, or `step_link`.
Phase A establishes the v3 types. The existing `trigger` + `steps` fields are
**preserved** as the Tier-1 / Tier-2 fallback so old Recipes continue to work.

#### How a Recipe works (v3 complete flow)

```
Author:
  1. Author writes StepDescriptions in WebUI (YAML-structured, human-readable).
  2. Each intent expression gets a step_link pointing into StepDescriptions.

Intent match (runtime):
  1. resolve_intent(user_text) → Match { recipe_id, class_code:21, step_link }
  2. fetch_for_turn:
       a. Fetch step_descriptions JSONB from recipe row
       b. IBS: build_instruction(step_link, step_descriptions, variable_patterns)
              → BuildInstruction { rust_steps[], orchestrator_steps[] }
       c. Apply {{vars.name}} substitution
       d. fetch_component_by_id for each UUID in rust_steps → rust_items
       e. fetch_component_by_id for each UUID in orchestrator_steps → orchestrator_items
       f. Return FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }
  3. RecipeStage: rust_items applied to Rust execution context (silently, not via orchestrator)
  4. Orchestrator reads orchestrator_items + step annotations via __retrieve_docs__
  5. Rust layer executes using its pre-loaded context
```

#### Mandatory shape

| Field | Content |
|-------|---------|
| `name` | Recipe identifier (e.g. `local-files-reading`) |
| `description` | One-sentence summary |
| `category` | Task group → `ExtensionCatalogue.task_groups[].group_name` |
| `step_descriptions JSONB` | Array of StepDescriptionN (YAML text + parsed fields) |
| `variants[]` | One or more `RecipeVariant` entries |
| `trigger` / `steps` | **Kept** — v2 fallback path |

#### Intent Variants

Each variant:
- Owns its own intent expressions (rows in `reborn_intent_inputs`)
- Carries a `step_link` specifying which StepDescription ranges to compile
- The `BuildInstruction` is computed at runtime by the IBS from StepDescriptions — **never stored as a blob**

**Example — Recipe `local-files-reading`:**

```
variant: ls-l
  intents: ["ls -l", "show me all files", "list files", "show local directory files"]
  step_link: "0:0-0:E"               ← all of StepDescription0

variant: ls-la
  intents: ["ls -la", "show all files including hidden", "list all files"]
  step_link: "0:0-0:30+1:0-1:E"     ← SD0 steps 0..30, then all of SD1

variant: ls-other-dir
  intents: ["list files of the /tmp directory", "show files in {{vars.dir}}"]
  variable_patterns: [{ name: "dir", pattern: r"of the (?P<dir>[/\w.-]+)" }]
  step_link: "0:0-0:31+2:0-2:E"     ← SD0 steps 0..31, then all of SD2
```

---

### 0.4 BuildInstruction — Two-Channel Design

> **Key design principle:** A `BuildInstruction` serves **two runtime readers**.
> The IBS is the sole producer. BuildInstructions are never hand-authored or pre-stored.
> StepDescriptions are the authoritative source; the BuildInstruction is a derived artefact.

#### Why two channels, not three

Earlier drafts described a three-section design (RetrievalEngine / Orchestrator / Rust).
The v3 design simplifies: `fetch_steps` is eliminated as a separate section. The IBS
directly emits `rust_steps[]` and `orchestrator_steps[]`, each containing `RecipeStep`
entries with UUIDs. `PostgresSource::fetch_for_turn` calls `fetch_component_by_id` for
each UUID immediately after IBS compilation — there is no separate `fetch_by_instruction`
method. The result is a `FetchForTurnResult::SplitResult` with two pre-fetched item lists.

#### Two readers, two typed channels

**Channel R — Rust (`rust_steps[]`)**  
Steps with `knowledge: "rust"` or `"both"`.  
Contains: ToolSkill UUIDs + ToolBinding params + ErrorPolicy.  
Applied silently to the Rust execution context by `RecipeStage`. Never forwarded to the orchestrator.

**Channel O — Orchestrator (`orchestrator_steps[]`)**  
Steps with `knowledge: "orchestrator"` or `"both"`.  
Contains: Skill UUIDs, PythonCode UUIDs, `type: "text"` annotations (the `info` field text
IS the orchestrator instruction — not merely documentation), and control-flow hints.  
Serialized into `orchestrator_content` by `handle_retrieve_docs` in `orchestrator.rs`.

#### 0.4.1 ToolBinding + ErrorPolicy

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolBinding {
    pub tool_name: String,
    pub params: serde_json::Value,   // {{vars.name}} substitution applied
    pub error_policy: ErrorPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    Fail,
    Ignore,
    Retry { max_attempts: u32 },
    Fallback { step_id: String },
}

impl Default for ErrorPolicy { fn default() -> Self { ErrorPolicy::Fail } }
```

#### Structure

```
BuildInstruction
├── llm_call_required: bool           ← false for Tier-0/Actions, true for Tier-1+
├── variable_patterns[]               ← applied before any channel is read
├── basic_prompt_section_refs[]       ← navigation hints into cached basic-prompt (no re-fetch)
│
├── rust_steps[]                      ← CHANNEL R: Rust execution layer reads this only
│   └── RecipeStep { step_id, knowledge: Rust/Both,
│                    include: Vec<Uuid>,          ← ToolSkill UUIDs
│                    tool_bindings: Vec<ToolBinding> }
│
└── orchestrator_steps[]              ← CHANNEL O: serialized into orchestrator_content
    └── RecipeStep { step_id, knowledge: Orchestrator/Both,
                     step_type: Text | Component,
                     include: Vec<Uuid>,          ← Skill / PythonCode UUIDs
                     info: Option<String>,        ← IS the instruction text for type:text
                     step_formatter_id: Option<Uuid> }  ← optional per-recipe prose formatter
```

**Invariant:** Channels must not overlap.  
A ToolSkill UUID must never appear in `orchestrator_steps`.  
A Skill UUID must never appear in `rust_steps`.  
An orchestrator step never references a ToolSkill. A rust step never references a Skill.

#### Complete example — Recipe `local-files-reading`, variant `ls-la`

```
BuildInstruction for variant: ls-la
  (intent: "show all files including hidden in /tmp")

llm_call_required: false   ← Tier 0: skip LLM, execute directly
variable_patterns:
  - { name: "dir", pattern: r"in (?P<dir>[/\w.-]+)" }

# ── CHANNEL R: Rust ─────────────────────────────────────────────────────
rust_steps:
  - step_id: "r-toolskill-ls"
    knowledge: Rust
    include: ["<uuid:toolskill-ls>"]
    tool_bindings:
      - tool_name: "ls"
        params: { flags: "-la", dir: "{{vars.dir}}" }
        error_policy: { policy: "fail" }

# ── CHANNEL O: Orchestrator ──────────────────────────────────────────────
orchestrator_steps:
  - step_id: "o-context"
    knowledge: Orchestrator
    step_type: Text
    info: |
      Task performed by orchestrator only. No LLM prompt created.
      Rust receives: ToolSkill "ls" + Tool "ls".
      Orchestrator receives: Skill "ls" + PythonCode "ls-result-handler".
      Orchestrator uses the skill to instruct the Rust executioner.
      Rust executes ls and returns stdout. Orchestrator formats output for chat.

  - step_id: "o-skill-ls"
    knowledge: Orchestrator
    step_type: Component
    include: ["<uuid:skill-ls>"]

  - step_id: "o-pythoncode-ls"
    knowledge: Orchestrator
    step_type: Component
    include: ["<uuid:pythoncode-ls>"]
    step_formatter_id: "<uuid:terse-cli-formatter>"   ← optional; omit for raw step text
```

---

### 0.5 StepDescription Authoring Layer

**StepDescription is the single human-editable source of truth** for what a Recipe does.
It serves two audiences simultaneously:

- **Human / WebUI editor:** A YAML-structured, readable description of every step a
  component performs after an intent match. Editable in the WebUI component page.
- **IBS (Instruction-Building-System):** The authoritative source from which the
  two-channel `BuildInstruction` is assembled at intent-match time. The IBS reads
  StepDescriptions directly — no intermediate format.

StepDescriptions are stored as a JSONB column `step_descriptions` on `reborn_recipes`
(added in V047). The YAML-formatted text is preserved inside the JSONB for human
readability and WebUI rendering.

#### Mandatory fields per step

| Field | Type | Meaning |
|-------|------|---------|
| `stepnumber` | int | 1-based ordinal position within this StepDescription's step sequence |
| `knowledge` | `"orchestrator" \| "rust" \| "both"` | Which runtime channel reads this step |
| `goal` | string | What this step accomplishes (human-readable) |
| `content` | string | Short description of step content |
| `type` | `"text" \| "component" \| "snippet"` | Determines IBS treatment (see below) |

#### Optional fields per step

| Field | Type | Meaning |
|-------|------|---------|
| `info` | text | **For `type: "text"` steps: this IS the orchestrator instruction text** — not merely documentation. The IBS emits it verbatim into `orchestrator_steps[].info` and `handle_retrieve_docs` includes it in `orchestrator_content`. |
| `include` | UUID[] | Component UUIDs needed at this step. IBS emits a fetch for each UUID. |
| `codesnippet` | text | Inline Python code. On WebUI save: creates a PythonCode component (class 22), enters Q1 queue. Step greyed out until Q1+Q2 pass; promoted to `type: "component"` with the new UUID on Q2 pass. |

#### Step types

| Type | IBS behaviour |
|------|--------------|
| `text` | Emits an annotation step. No component fetch. The `info` field text IS the instruction for the orchestrator. Routed to orchestrator channel (or both) per `knowledge`. |
| `component` | Emits a fetch for each UUID in `include`. Routes item to rust or orchestrator channel based on `knowledge`. |
| `snippet` | WebUI-only authoring shortcut. **IBS refuses to assemble** a BuildInstruction while any step has this type — it returns `IbsError::UnpromotedSnippet`. The step must be promoted to `type: "component"` after the created PythonCode passes Q1+Q2. |

> **`info` field and the IBS:** The IBS does NOT silently ignore `info` text on `type: "text"`
> steps. It is the primary instruction mechanism for context steps that carry no component UUID.
> An orchestrator step with `type: "text"` and no `info` is a Q1 validation error.

#### Multi-StepDescription pattern (variants)

- `StepDescription0` — base, most common use-case (the full shared prefix)
- `StepDescription1` — variant 1 (individual part only; SD0 provides the shared prefix via step_link)
- `StepDescription2`, `StepDescription3`, ...

Each `desc_idx` (0-based) maps to an element in the `step_descriptions` JSONB array.

#### Example — Recipe `local-files-reading`, StepDescription0 (partial)

```yaml
# desc_idx: 0  — base path (ls -l, current directory)
steps:
  - stepnumber: 1
    knowledge: orchestrator
    goal: Provide task context
    content: Information explaining the task
    type: text
    info: |
      Task performed by orchestrator only. No LLM prompt created.
      Rust receives: ToolSkill "ls" + Tool "ls".
      Orchestrator receives: Skill "ls" + PythonCode "ls-result-handler".
      Orchestrator uses the skill to instruct the Rust executioner.
      Rust executes ls and returns stdout. Orchestrator formats output for chat.

  - stepnumber: 2
    knowledge: rust
    goal: Provide ToolSkill
    content: ToolSkill "ls"
    type: component
    include: ["<uuid:toolskill-ls>"]

  - stepnumber: 3
    knowledge: orchestrator
    goal: Provide Skill
    content: Skill "ls"
    type: component
    include: ["<uuid:skill-ls>"]

  - stepnumber: 4
    knowledge: orchestrator
    goal: Provide execution instructions
    content: PythonCode "ls-result-handler"
    type: component
    include: ["<uuid:pythoncode-ls>"]
    info: |
      Final step. PythonCode tells the orchestrator how to invoke the skill,
      pass flags to Rust, and format output for the chat window.
```

#### WebUI interaction

- Component page → **Step Descriptions section**: all steps shown, editable on click.
- Dropdown fields for `knowledge` and `type`; free text for `goal`, `content`, `info`.
- `include` field: UUID autocomplete over known component names.
- `step_link` per intent: shown inline, editable with live syntax validation.
- `codesnippet` field: on save → new PythonCode component created → sent to Q1.
  **Security:** Snippet submission requires an authenticated session with `component:write`
  permission. Unauthenticated and read-only sessions must not be able to submit snippets.
  The Q1 injection scan is the technical backstop; ACL is the first line of defense.
  - While pending: step greyed out in WebUI.
  - If Q1 fails: snippet field cleared, PythonCode removed.
  - If Q1+Q2 pass: step promoted to `type: "component"` with the new UUID; parent Recipe re-queued to Q1.

---

### 0.6 Intent-Link Formula (`step_link`)

Every intent row in `reborn_intent_inputs` carries a `step_link` TEXT column encoding
which steps to assemble. This single field replaces the previously separate `variant_key`
+ `link_formula` columns: it is more expressive (encodes shared prefixes without
duplicating steps) and is the **direct input to the IBS** — no secondary lookup needed.

#### Notation

```
step_link = "{desc_idx}:{start}-{desc_idx}:{end}" [+ "+" more segments]

  {desc_idx} = StepDescription index (0 = base, 1 = first variant, …)
               Always 0-based index into the step_descriptions JSONB array.
  {start}    = step number (1-based, matching the stepnumber field), or 0 (sentinel = first step)
  {end}      = step number (1-based), or E (sentinel = last step)
  +          = concatenate segments in order
```

> **Indexing invariant:** `stepnumber` inside StepDescriptions is always 1-based.
> Formula start/end refer to `stepnumber` values (1-based), except `0` which is a
> sentinel meaning "first step in the sequence regardless of numbering gaps".
> `0:0-0:E` and `0:1-0:E` both mean "all steps of StepDescription0".

#### Examples

| Formula | Meaning |
|---------|---------|
| `0:0-0:E` | All steps of SD0 (single-variant component) |
| `0:0-0:30+1:0-1:E` | SD0 steps 0–30 (shared prefix), then all of SD1 (individual part) |
| `0:0-0:31+2:0-2:E` | SD0 steps 0–31, then all of SD2 |
| `0:0-0:30+1:0-1:11+3:0-3:E` | SD0 steps 0–30, SD1 steps 0–11, all of SD3 |

#### Storage

**Migration V050:** `ADD COLUMN step_link TEXT CHECK (length(step_link) <= 4096)` to `reborn_intent_inputs`.

**`step_link` is nullable.** Existing rows after V050 have `step_link IS NULL`. The IBS
treats a NULL `step_link` as a legacy intent — it skips IBS compilation and falls through
to the existing `fetch_component_by_id` path unchanged. Only rows seeded after Phase D
carry a non-NULL `step_link`.

**`step_link` replaces `variant_key`.** There is no `variant_key` column. New variants
are authored with `step_link` from the start.

```
| intent_expression              | component_id  | step_link               |
|--------------------------------|---------------|-------------------------|
| "ls -l"                        | <recipe-uuid> | "0:0-0:E"               |
| "show all files including ..."  | <recipe-uuid> | "0:0-0:30+1:0-1:E"      |
| "list files of the /tmp dir"   | <recipe-uuid> | "0:0-0:31+2:0-2:E"      |
```

---

### 0.7 Instruction-Building-System (IBS)

The IBS **compiles** human-editable StepDescriptions into machine-optimized `BuildInstruction`
structs at intent-match time. It is the sole producer of BuildInstructions. BuildInstructions
are never hand-authored or pre-stored.

> **Why assemble on match rather than pre-store?** Component UUIDs in `include` fields can
> be updated (a PythonCode component is revised and re-validated). Pre-stored BuildInstructions
> would require a cascade rebuild on every component update. On-match assembly always reads
> current, validated UUIDs with zero staleness risk. Hot-path memoisation (keyed on
> `sha256(step_link + sorted_include_uuids)`, evicted on validation-status change) eliminates
> the cost for repeated identical intents.

#### Location

- New module: `crates/brassclaw_engine/src/memory/instruction_builder.rs`
- **Pure Rust, no async, no DB calls.**
- Called by `PostgresSource::fetch_for_turn` after an intent match resolves to a Recipe (class 21).
- Exposed as `crate::memory::instruction_builder::build_instruction`.

#### Assembly algorithm

```
fn build_instruction(
    step_link:          &str,
    step_descriptions:  &[StepDescriptionEntry],  // from JSONB
    variable_patterns:  &[VariablePattern],
) -> Result<BuildInstruction, IbsError>

1. Parse step_link → Vec<StepRange>  (e.g. [(desc_idx:0, 0..=30), (desc_idx:1, 0..=E)])
2. For each StepRange:
     Select steps[start..=end] from step_descriptions[desc_idx]
     Append to ordered step list
3. For each step in the ordered list:
     type == "text"      → emit annotation step; route by knowledge; info IS the instruction
     type == "component" → emit component fetch step; route by knowledge; emit UUIDs from include
     type == "snippet"   → return Err(IbsError::UnpromotedSnippet)
4. Validate:
     step numbers must be monotonically increasing within each StepDescription
     rust-channel steps must have type:component with non-empty include
     all include UUIDs must parse as valid UUID v4
     S7 guard: if any rust_steps emit tool_bindings, orchestrator_steps must contain ≥1 skill_id
5. Partition:
     rust_steps[]         ← steps where knowledge ∈ {"rust", "both"}
     orchestrator_steps[] ← steps where knowledge ∈ {"orchestrator", "both"}
6. Return BuildInstruction { rust_steps, orchestrator_steps,
                              variable_patterns, basic_prompt_refs,
                              llm_call_required }
```

#### LLM-formatted orchestrator content

After assembly, `handle_retrieve_docs` in `orchestrator.rs` renders orchestrator_steps
into a human+LLM-readable block (`orchestrator_content` in the `__retrieve_docs__` result):

```
## Task: {recipe.name} — variant: {variant_label}

Step 1 [orchestrator — text]:
  This task is performed by the orchestrator only. The Rust execution layer
  receives ToolSkill "ls"…

Step 3 [orchestrator — skill]:
  Skill "ls" (UUID: uuid-of-ls-skill) loaded.
  [skill body content]

Step 4 [orchestrator — python_code]:
  PythonCode "ls-result-handler" (UUID: uuid-of-pythoncode) loaded.
  [pythoncode body content]
  Final step: use the skill to call Rust, format output for chat window.
```

If `step_formatter_id` is set on any orchestrator_step, the referenced PythonCode body
is also loaded and used to reformat the step block into LLM-optimal prose. The
`step_formatter_id` is per-Recipe (consistent formatting style across a capability domain).

#### Memoisation

- **Key:** `sha256(step_link + "|" + sorted_include_uuids.join(","))`
- **Eviction triggers (all must be monitored):**
  1. Any `include`d component's `updated_at` changes
  2. The Recipe's own `updated_at` changes (StepDescription edited in WebUI)
  3. The `step_formatter_id` PythonCode component's `updated_at` changes
- **Cache miss:** safe at high concurrency — compilation is pure computation (no HTTP, no DB).
  Concurrent misses compile redundantly; last writer wins the cache slot (idempotent).

#### Errors

```rust
pub enum IbsError {
    UnpromotedSnippet { step_id: String },
    InvalidUuid { step_id: String, value: String },
    StepOrderViolation { desc_idx: usize, stepnumber: u32 },
    UnknownDescIdx { desc_idx: usize },
    ParseError { formula: String, reason: String },
    S7Violation,  // rust tool_bindings present but no orchestrator skill_ids
}
```

#### Interface

```rust
// crates/brassclaw_engine/src/memory/instruction_builder.rs

pub fn build_instruction(
    step_link:         &str,
    step_descriptions: &[StepDescriptionEntry],
    variable_patterns: &[VariablePattern],
) -> Result<BuildInstruction, IbsError>;

pub fn parse_step_link(step_link: &str) -> Result<Vec<StepRange>, IbsError>;
```

No trait, no async. Called synchronously inside `fetch_for_turn`.

---

### 0.8 `fetch_for_turn` Upgrade — SplitResult and ActionShortCircuit

#### Current state (grounded in code)

`PostgresSource::fetch_for_turn` (in `retrieval_source.rs`) already calls:
- `resolve_intent(pool, scope, query)` → `IntentResolution::Match { component_id, class_code }`
- `fetch_component_by_id(uuid)` on a match

`IntentResolution::Match` currently has only `{ component_id: Uuid, component_class_code: i32 }`.
`FetchForTurnResult` currently has only `Components(Vec<ComponentItem>)` and
`Disambiguation(Vec<IntentCandidate>)`.

#### Extended `FetchForTurnResult`

```rust
pub enum FetchForTurnResult {
    /// No-match UNION ALL path or non-recipe intent match (existing behaviour unchanged).
    Components(Vec<ComponentItem>),

    /// Multiple near-equal intent candidates — surface disambiguation UX.
    Disambiguation(Vec<IntentCandidate>),

    /// Intent matched an Action (class 16) — execute directly, no LLM.
    ActionShortCircuit { component_id: Uuid, name: String },

    /// Intent matched a Recipe (class 21) with a step_link.
    /// Two channels pre-fetched and ready for delivery.
    SplitResult {
        rust_items:         Vec<ComponentItem>,   // ToolSkill bodies — Rust only
        orchestrator_items: Vec<ComponentItem>,   // Skill + PythonCode bodies
        routing:            TurnRoutingSignals,
    },
}

pub struct TurnRoutingSignals {
    pub override_prompt_creation: bool,
    pub matched_component_ids:    Vec<String>,  // orchestrator-channel UUIDs (for _set_active_skills)
    pub variant_label:            String,
    pub step_link:                String,
    pub llm_call_required:        bool,
}
```

#### Updated `IntentResolution::Match`

```rust
// In intent_system.rs — add step_link field:
Match {
    component_id:        Uuid,
    component_class_code: i32,
    step_link:           Option<String>,  // None for legacy / non-variant intents
}
```

Update all match sites in `retrieval_source.rs` and `orchestrator.rs` that destructure
`IntentResolution::Match { component_id, component_class_code }` to also bind `step_link`.
Non-IBS paths treat `None` as a legacy match and fall through to `fetch_component_by_id` unchanged.

#### Updated `fetch_for_turn` flow

```
fetch_for_turn(scope, query, token_budget, consumer_tag):

  1. resolve_intent(pool, scope, query)
       → Match { component_id, class_code, step_link }

          a. class_code == 16 (Action):
               → return FetchForTurnResult::ActionShortCircuit { component_id, name }

          b. class_code == 21 (Recipe) AND step_link.is_some():
               i.   Fetch Recipe row → step_descriptions JSONB + variable_patterns
               ii.  IBS: build_instruction(step_link, step_descriptions, variable_patterns)
                         → BuildInstruction { rust_steps[], orchestrator_steps[] }
               iii. Apply {{vars.name}} substitution (captured from user_text)
               iv.  Fetch ComponentItem for each UUID in rust_steps → rust_items
                    Fetch ComponentItem for each UUID in orchestrator_steps → orchestrator_items
               → return FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }

          c. step_link.is_none() (legacy intent or non-recipe class):
               → Existing fetch_component_by_id path → Components([item]) (unchanged)

       → Disambiguation { candidates }:
               → return FetchForTurnResult::Disambiguation(candidates)

       → NoMatch / DbLessFallback:
               → fetch_for_consumer (UNION ALL) → return Components(broad_scan)
```

**No `reborn_pending_rust_context` transient table.** `rust_items` from `SplitResult`
are delivered directly into the Rust execution context by `RecipeStage` (Phase H).
There is no DB round-trip for rust channel delivery.

**No `fetch_by_instruction` method.** The IBS is called synchronously inside
`fetch_for_turn`. There is no separate `RetrievalSource` method for executing a
BuildInstruction.

---

### 0.9 The Current Step-0 Problem and v3 Solution

#### Current step-0 (three calls, two problems)

```python
# Current default.py step 0 — three separate calls:
pkr        = __assemble_prior_knowledge__(goal, token_budget, "02")  # PRIMARY: merged blob
docs       = __retrieve_docs__(goal, 5)                              # SHIM: dead Action detect
all_skills = __list_skills__()                                       # extra round-trip
active_skills = select_skills(all_skills, goal, ...)                 # re-selection
```

**Problem 1 — dead Action detection shim:**  
`__retrieve_docs__` at step-0 uses the legacy `RetrievalEngine::retrieve_context`
(MemoryDoc path). It returns `{type, title, content}` with **no `class_code`** in the
metadata. The check `metadata.get("class_code") == 16` (line 1022 of `default.py`)
therefore **never fires**. This is a known named bug.

**Problem 2 — redundant skills round-trip:**  
`__list_skills__()` → `select_skills()` does no scoring (takes first N in budget).
With a BuildInstruction, the IBS already selected the exact Skills for this turn by UUID.
The re-selection step is unnecessary.

**Problem 3 — mixed blob:**  
`__assemble_prior_knowledge__` returns one merged `formatted_content` blob. Skill bodies,
PythonCode, ToolSkills — all go to the orchestrator together. There is no channel separation.

#### v3 step-0: single call

The three-call block collapses to one call. The upgraded `__retrieve_docs__` handles
everything — intent resolution, IBS compilation, channel split, Action routing.

```python
# v3 default.py step 0:
if step == 0:
    token_budget = config.get("prior_knowledge_token_budget", 100000)
    pkr = __retrieve_docs__(goal, token_budget)

    if isinstance(pkr, dict):
        if pkr.get("action_short_circuit"):
            __emit_event__("action_started", action_name=pkr.get("action_name", ""))
            __transition_to__("running", "action execution")
            action_result = execute_action_by_id(pkr["action_component_id"], goal, state)
            __transition_to__("completed", "action completed")
            return action_result

        if pkr.get("disambiguation"):
            return handle_disambiguation(pkr["candidates"], state)

        if pkr.get("override_prompt_creation"):
            working_messages = [{"role": "User",
                                  "content": pkr.get("orchestrator_content", "")}]
        elif pkr.get("orchestrator_content"):
            insert_as_user_message_at_n_minus_1(working_messages,
                                                pkr["orchestrator_content"])

    # Volatile context injected separately — never mixed with prior knowledge.
    insert_volatile_context_at_n_minus_1(working_messages)

    # Active-skill tracking using matched UUIDs — no __list_skills__ round-trip.
    _set_active_skills_from_matched_ids(pkr.get("matched_component_ids", []), state)
```

#### What `__retrieve_docs__` returns in v3

```python
{
    # Orchestrator channel only.
    # Skills, PythonCode, LLM-formatted step annotations from type:text steps.
    # ToolSkill bodies NEVER appear here.
    "orchestrator_content": str,

    # Routing signals:
    "override_prompt_creation": bool,
    "action_short_circuit":     bool,
    "action_component_id":      str,   # UUID (when action_short_circuit is true)
    "action_name":              str,
    "disambiguation":           bool,
    "candidates":               list,

    # Active-skill tracking:
    "matched_component_ids":    list,  # orchestrator-channel UUIDs (Skills + PythonCode)
                                       # passed to _set_active_skills_from_matched_ids;
                                       # no __list_skills__() + select_skills() round-trip
}
```

The Rust channel (ToolSkills, ToolBindings) is applied to the Rust execution context
**inside the Rust handler, silently**. It never crosses to the orchestrator's `working_messages`.

#### `call_action` nested lookup migration

`call_action` in `default.py` (line 844) currently calls `__retrieve_docs__(name, 1)` to
look up an Action by name. This is a search-by-name — fragile and hits the legacy path.

**v3 replacement:** a new host function `__fetch_component__(uuid, class_code)` calls
`fetch_component_by_id` directly with the UUID from the BuildInstruction step.

```python
# Old (line 844):
action_docs = __retrieve_docs__(action_name, 1)
# New:
action_item = __fetch_component__(action_uuid, 16)
```

`__assemble_prior_knowledge__` is superseded. It remains registered for backward
compatibility with custom orchestrators. Removed in Phase K cleanup.

---

### 0.10 Current Turn Pipeline (Actual Code)

```
1.  CheckpointStage     — cancel-check
2.  BudgetStage         — token/iteration budget check
3.  InputStage          — drain pending user input → LoopExecutionState
4.  RecipeStage         — [STUB] always falls through (structural debt in recipe.rs)
5.  PromptStage         — assemble LLM prompt from history + prior_knowledge
6.  InterceptorStage    — Sempai review of outgoing prompt (if connected)
7.  ModelStage          — LLM call (Kohai)
8.  ReplyAdmissionStage — validate/admit model response
9.  AssistantReplyStage — emit response to user
10. CapabilityStage     — if response contains tool calls: execute, loop back
11. StopStage           — check for loop termination
12. ExitStage           — clean exit
```

**Critical gap:** `RecipeStage` (step 4) always falls through to Tier 2. Phase H closes this.

`LoopExecutionState` has no `last_user_text` field. Added in Phase H via `InputStage`.

**Prior-knowledge assembly** happens inside `PromptStage` (step 5) via the orchestrator's
`__retrieve_docs__` call, which in v3 calls `PostgresSource::fetch_for_turn` and handles
the full split and channel delivery internally.

---

### 0.11 Normal Assembly — No-Match Path (UNION ALL weights)

| Class | Label | Weight |
|-------|-------|--------|
| 50 | Scaffold | 0.55 |
| 10 | Orchestrator | 0.52 |
| 12 | Spec | 0.50 |
| 0 | Tool | 0.50 |
| 1–3 | Skills | 0.45 |
| 4–9 | Extensions | 0.42 |
| **22** | **PythonCode** | **0.42** |
| 13 | ToolSkill | 0.40 |
| 18 | Lesson | 0.40 |
| 21 | Recipe | 0.38 |
| **23** | **ExtensionCatalogue** | **0.38** |
| 16 | Action | 0.35 |
| 14 | Plan | 0.30 |
| 17 | Docu | 0.25 |
| 19 | Issue | 0.20 |
| 15 | Summary | 0.10 |
| 20 | Note | 0.05 |

Bold rows are new additions for v3. These weights are added to `doc_type_weight_by_class`
in `retrieval_dbless.rs`.

---

### 0.12 Actions — LLM-Bypass

Actions (class 16) already default to `override_prompt_creation = true` in V029.
Their `steps` JSONB encodes 13 step types and is **executed directly by the orchestrator
without going through the IBS**. The IBS applies to Recipes (class 21) only.

In v3, an Action intent match returns `FetchForTurnResult::ActionShortCircuit` — no
BuildInstruction, no IBS compilation, no prior-knowledge assembly. The `__retrieve_docs__`
return dict carries `action_short_circuit: true` + `action_component_id`. The Python
step-0 block calls `execute_action_by_id` and returns immediately.

Do not confuse the Action override mechanism (`override_prompt_creation`) with the
Recipe Tier-0 mechanism (`llm_call_required: false`). They are separate paths.

---

### 0.13 KV-Cache / LMCache-Aware Design

**Basic-prompt:** Pre-assembled `InstructionBundle` stored in `reborn_basic_prompt_store`
(V052). Manual trigger only. Stale when any component passes Gate 2.

**BuildInstruction patch rules:**
- Must NOT repeat content already in the stored basic-prompt.
- `basic_prompt_section_refs` carries navigation hints (pointers, not content):
  e.g. `"→ see §ls-skill in basic-prompt"` — the LLM already has the body from KV-cache.
- Target patch size: < 4k tokens (fast new-token computation).
- Orchestrator patch: PRIORITY 2 (instruction snippets) in the InstructionBundle.
- Memory: PRIORITY 3 (memory snippets).
- Rust context: delivered directly by `RecipeStage`, not in the bundle at all.

---

### 0.14 Interceptor System

Saves each turn's composition plan (`BuildInstruction` orchestrator_steps + routing signals,
not the basic-prompt content). If Sempai is connected: reviews the outgoing prompt before
shipping to Kohai. Can flag patterns for Recipe creation.

---

### 0.15 Validation System — Two-Gate Pipeline

**Gate 1 (Q1 — automatic):** Injection scan, schema conformance, S7 guard, cross-references.  
**Gate 2 (Q2 — manual):** WebUI review; approve → `validated`.

A component with `validation_status = 'validated'` drives two side effects:
1. Its `updated_at` is refreshed.
2. `reborn_basic_prompt_store.is_stale` is set to `true` for the affected scope.
3. The IBS memo-cache for any `step_link` that references this component's UUID is evicted.

---


### 0.16 Builtin Tool Bootstrap

#### Ground truth

All 23 first-party builtin tools are registered purely in Rust code
(`crates/brassclaw_host_runtime/src/first_party_tools/`), under provider ID `"builtin"`.
The DB tables `reborn_tools` (V030), `reborn_tool_skills` (V037), and `reborn_skills` (V027)
are live and structurally ready, but contain **zero rows for builtins** today. The
orchestrator receives no authored prior knowledge about when to use `grep` vs `read_file`,
what memory_search expects, or when shell requires approval.

`reborn_tools` currently has no column linking a DB row back to its registered capability ID
(`"builtin.read_file"`). A `capability_id TEXT` column is needed (V053) to avoid fragile
name-search lookups when the Rust execution layer needs to resolve a Tool row to its handler.

#### What gets generated

The builtin bootstrap generates the full v3 component stack for all 23 tools, grouped into
**5 ExtensionCatalogues** by cognitive domain (not one per tool):

| Catalogue | Tools covered | Recipes (approx) |
|-----------|---------------|------------------|
| `builtin-filesystem` | read_file, write_file, list_dir, glob, grep, apply_patch | 6–8 |
| `builtin-network` | http, http.save | 3–4 |
| `builtin-memory` | memory_search, memory_write, memory_read, memory_tree | 2 |
| `builtin-process` | shell, spawn_subagent, trigger_create/list/remove | 5–6 |
| `builtin-management` | skill_list/install/remove, echo, time, json | 4–5 |

For each tool the bootstrap generates:
- **Tool (class 0):** One row per `builtin.X` capability. `capability_id = "builtin.X"`,
  `effect_type` mapped from EffectKind, `param_schema` from Rust schema structs in
  `schemas.rs`, `source = "system"`.
- **ToolSkill (class 13):** One per tool. Hand-authored `content` with: the exact
  `tool_name`, annotated `param_schema`, preconditions, error handling, and critical safety
  notes (especially for `shell`, `apply_patch`, `spawn_subagent`).
- **Skill (class 1–3):** **Task-level, not tool-level.** The filesystem group gets 4–5
  Skills covering task patterns (e.g. "find files" = glob + grep combined in one Skill
  body), not 6 trivial single-tool Skills. Utilities (`echo`, `time`, `json`) get
  PythonCode helpers instead of Skills (see Grain rule below).
- **PythonCode (class 22):** For utility helpers that are sub-orchestrator patterns rather
  than standalone capabilities: `json-query-helper`, `time-format-helper`, `patch-formatter`
  (for apply_patch result formatting).
- **Recipe (class 21):** Task-level, multi-variant where the cognitive grain demands it.
  `builtin-edit-file` gets 3 variants; `builtin-http-fetch` gets 3. See §0.16.1 for the
  full recipe list.
- **ExtensionCatalogue (class 23):** One per domain. `overview_doc` describes the domain
  model, not individual tools.

#### Grain rule — Skill vs PythonCode

Use a **Skill** when: the orchestrator needs narrative instructions for a task pattern
that spans one or more tools — a complete capability description.  
Use **PythonCode** when: the component is a utility helper used inside another Recipe's
orchestrator channel, not a standalone capability.

`echo`, `time`, `json` → PythonCode helpers.  
All filesystem, network, memory, skill-management, trigger patterns → Skills.

#### Validation at bootstrap

All generated components are inserted with `source = "system"` and
`validation_status = "validated"` (bypassing Q2 for system-authored components; Q1 still
runs internally inside the seeder). This prevents the boot state from depending on a human
completing Q2 before the agent can use its own core tools.

`"system"` is a new allowed value for the `source` column on `reborn_tools`,
`reborn_tool_skills`, and `reborn_skills` (V053 adds it to the CHECK constraints).
Q1 errors in the seeder content are a build-time bug, not a runtime failure mode — they
must be caught in CI.

#### Shell + spawn_subagent safety invariants

Two invariants encoded in ToolSkill bodies and enforced at Q1:

1. **`builtin.shell`:** The shell ToolSkill `content` must include an explicit
   approval-gate description. Any Recipe whose rust channel references `builtin.shell`
   **must** have `llm_call_required: true` — enforced as a Q1 rule (see Phase I §shell-guard).
   Open-ended shell cannot be Tier 0. Known-safe commands (e.g. `cargo build`) may be
   Tier 1 at high Wilson score, but never Tier 0 without explicit allowlisting.

2. **`builtin.spawn_subagent`:** The spawn_subagent ToolSkill must document: child cannot
   exceed parent scope, budget inheritance, authorization model. Any Recipe using it must
   be Tier 1 (`llm_call_required: true` enforced at Q1 — same rule as shell).

#### §0.16.1 Full builtin Recipe list (target)

| Recipe name | Variants | Tier | ToolSkills in rust channel |
|-------------|----------|------|---------------------------|
| `builtin-read-file` | 2 (by path, by glob) | 0 | read_file |
| `builtin-write-file` | 2 (create, overwrite) | 1 | write_file |
| `builtin-list-dir` | 2 (current dir, named dir) | 0 | list_dir |
| `builtin-find-files` | 3 (by name, by ext, by pattern) | 0 | glob |
| `builtin-search-content` | 3 (literal, regex, in dir) | 0 | grep |
| `builtin-edit-file` | 3 (targeted edit, refactor, fix-line) | 1 | read_file + apply_patch |
| `builtin-http-fetch` | 3 (GET, POST, with headers) | 1 | http |
| `builtin-http-download` | 1 | 1 | http.save |
| `builtin-remember` | 1 | 0 | memory_write |
| `builtin-recall` | 1 | 0 | memory_search |
| `builtin-run-shell` | 2 (known-safe cmd, open-ended) | 1 (always) | shell |
| `builtin-spawn-subagent` | 2 (generic task, named procedure) | 1 (always) | spawn_subagent |
| `builtin-create-trigger` | 1 | 1 | trigger_create |
| `builtin-list-triggers` | 1 | 0 | trigger_list |
| `builtin-remove-trigger` | 1 | 1 | trigger_remove |
| `builtin-list-skills` | 1 | 0 | skill_list |
| `builtin-install-skill` | 1 | 1 | skill_install |
| `builtin-remove-skill` | 1 | 1 | skill_remove |

**Total: ~23 Tools + 23 ToolSkills + 12–15 Skills + 4–5 PythonCode + 18–20 Recipes + 5 ExtensionCatalogues ≈ 85–90 components.**  
All inserted at boot if the scope has no existing builtin components (idempotent).

---

## 1. Implementation Phases

### Phase A — StepDescription Schema + IBS Core

**Status:** [ ] Pending

**Goal:** Define the StepDescription types, add `step_descriptions` JSONB to `reborn_recipes`,
implement the IBS as a pure-Rust module. This is Phase A because all later phases depend on it.

#### Files to create

- `crates/brassclaw_pg/migrations/V047__reborn_recipe_step_descriptions.sql`
  ```sql
  ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS step_descriptions JSONB;
  ```

- `crates/brassclaw_engine/src/memory/instruction_builder.rs`  
  New types: `StepDescriptionEntry`, `StepRange`, `StepOwner`, `RecipeStepType`,
  `RecipeStep`, `VariablePattern`, `BuildInstruction`, `IbsError`.  
  New functions: `parse_step_link(&str) -> Result<Vec<StepRange>, IbsError>` and
  `build_instruction(step_link, step_descriptions, variable_patterns) -> Result<BuildInstruction, IbsError>`.

#### Files to modify

- `crates/brassclaw_engine/src/types/recipe.rs` — add `BuildInstruction` two-channel shape;
  full `RecipeStepType` enum; `StepOwner`; `ToolBinding`; `ErrorPolicy`.  
  Add to `Recipe` struct:
  ```rust
  #[serde(default)] pub variants: Vec<RecipeVariant>,
  #[serde(default)] pub step_descriptions: serde_json::Value,
  ```
  `RecipeVariant`:
  ```rust
  pub struct RecipeVariant {
      pub variant_key: String,       // human label only; IBS uses step_link, not this
      pub label: String,
      pub intent_examples: Vec<String>,
      pub step_link: String,         // direct IBS input
      pub variable_patterns: Vec<VariablePattern>,
  }
  ```

- `crates/brassclaw_engine/src/memory/mod.rs` — `pub mod instruction_builder`

#### Tests

- Unit: `parse_step_link("0:0-0:E")` → single range, all steps
- Unit: `parse_step_link("0:0-0:30+1:0-1:E")` → two ranges, correct desc_idx and bounds
- Unit: `build_instruction` with `knowledge: rust` step → step only in `rust_steps`
- Unit: `build_instruction` with `knowledge: both` step → step in both channels
- Unit: `build_instruction` with `snippet`-type step → `IbsError::UnpromotedSnippet`
- Unit: step numbers non-monotonic within a StepDescription → `IbsError::StepOrderViolation`
- Unit: S7 guard: rust tool_bindings present, no orchestrator skill_ids → `IbsError::S7Violation`
- Unit: `BuildInstruction`, `ToolBinding`, `ErrorPolicy` serde roundtrips

---

### Phase B — PythonCode Component (class 22)

**Status:** [ ] Pending

**Goal:** New component class for Python code/instruction elements targeted at the orchestrator.

#### Files to create

- `crates/brassclaw_pg/migrations/V048__reborn_python_code.sql`  
  Same column shape as `V036__reborn_specs.sql`. `class_code = 22`.  
  Default consumer tags: `{02:orchestrator, 05:validator}`.

- `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs` — new store

#### Files to modify

- `crates/brassclaw_engine/src/memory/retrieval_source.rs` — add class 22 to UNION ALL + `fetch_component_by_id`
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `22 => "python_code"` to `class_label`
- `crates/brassclaw_engine/src/types/memory.rs` — add `DocType::PythonCode`
- `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` — add `22 => 0.42`
- `crates/brassclaw_engine/src/memory/component_validator.rs` — class 22 dispatch:
  name format, non-empty content, soft 10k token budget, shell-injection scan

#### Tests

- Unit: `class_label(22) == "python_code"`
- Unit: `doc_type_to_class_code(PythonCode) == 22`
- Integration: PythonCode row retrieved via `fetch_for_consumer` with consumer tag `02:orchestrator`

---

### Phase C — ExtensionCatalogue Component (class 23)

**Status:** [ ] Pending

**Goal:** Documentation-container class that organises a capability domain.

#### Files to create

- `crates/brassclaw_pg/migrations/V049__reborn_extension_catalogues.sql`  
  Columns: scope tuple + `name`, `description`, `version`, `overview_doc TEXT`,
  `task_groups JSONB`, `child_component_ids UUID[]`, `intent_index JSONB` (audit-only),
  standard lifecycle columns (`validation_status`, `updated_at`, etc.),
  `class_code SMALLINT DEFAULT 23`.

- `crates/brassclaw_reborn_composition/src/pg_extension_catalogue_store.rs` — new store

#### Files to modify

Same engine files as Phase B, but for class 23:
- `retrieval_source.rs` — class 23, `effective_content = overview_doc`
- `intent_system.rs` — `23 => "extension_catalogue"`
- `types/memory.rs` — `DocType::ExtensionCatalogue`
- `retrieval_dbless.rs` — `23 => 0.38`
- `component_validator.rs` — class 23: name format, non-empty `overview_doc`, ≥1 `task_group`,
  valid UUID syntax in `child_component_ids`

#### Tests

- Unit: `class_label(23) == "extension_catalogue"`
- Integration: Catalogue with `task_groups` → retrieved with `overview_doc` as `effective_content`

---

### Phase D — `step_link` Column on Intent Inputs

**Status:** [ ] Pending

**Goal:** Add `step_link` to `reborn_intent_inputs`; wire into `resolve_intent` and
`IntentResolution::Match`.

#### Files to create

- `crates/brassclaw_pg/migrations/V050__reborn_intent_inputs_step_link.sql`
  ```sql
  ALTER TABLE reborn_intent_inputs ADD COLUMN IF NOT EXISTS step_link TEXT
      CHECK (length(step_link) <= 4096);
  ```

#### Files to modify

- `crates/brassclaw_engine/src/memory/intent_system.rs`  
  Add `step_link: Option<String>` to `IntentResolution::Match`.  
  Update the resolution query to `SELECT ... step_link FROM reborn_intent_inputs`.  
  Update `seed_intent_input` to accept and store `step_link`.

- All call sites that destructure `IntentResolution::Match { component_id, component_class_code }`:
  bind `step_link` as well. Non-IBS paths treat `None` as a legacy match (unchanged behaviour).

**Notes:**
- `step_link` replaces `variant_key`. No `variant_key` column is added to `reborn_intent_inputs`.
- `step_link` is nullable. Existing rows use the existing `fetch_component_by_id` path unchanged.

#### Tests

- Unit: intent row with `step_link` → `IntentResolution::Match { step_link: Some(...) }`
- Unit: intent row without `step_link` → `IntentResolution::Match { step_link: None }` → existing path unchanged

---

### Phase E — `fetch_for_turn` Upgrade + `FetchForTurnResult::SplitResult`

**Status:** [ ] Pending

**Goal:** Wire the IBS into `PostgresSource::fetch_for_turn`. On a Recipe intent match
with a `step_link`, call the IBS, fetch component items for each channel, and return a
`SplitResult`. Handle Action match with `ActionShortCircuit`.

#### Files to modify

- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  - Extend `FetchForTurnResult` with `ActionShortCircuit` and `SplitResult` variants (§0.8).
  - Extend `TurnRoutingSignals` struct.
  - Update `PostgresSource::fetch_for_turn`:
    1. After `resolve_intent` → `Match { class_code: 16 }`: return `ActionShortCircuit`.
    2. After `resolve_intent` → `Match { class_code: 21, step_link: Some(...) }`:
       - Fetch Recipe row's `step_descriptions` JSONB + `variable_patterns`.
       - Call `instruction_builder::build_instruction(step_link, step_descriptions, variable_patterns)`.
       - Apply `{{vars.name}}` substitution using captures from `user_text`.
       - For each UUID in `rust_steps`: call `fetch_component_by_id` → `rust_items`.
       - For each UUID in `orchestrator_steps`: call `fetch_component_by_id` → `orchestrator_items`.
       - Return `SplitResult { rust_items, orchestrator_items, routing }`.
    3. After `resolve_intent` → `Match { step_link: None }`: existing `fetch_component_by_id` path (unchanged).

#### Tests

- Unit: Recipe match with `step_link` → `SplitResult`; `rust_items` contain only ToolSkills; `orchestrator_items` contain only Skills and PythonCode
- Unit: `knowledge: both` step → UUID appears in both `rust_items` and `orchestrator_items`
- Unit: Action (class 16) match → `ActionShortCircuit { component_id, name }`
- Unit: match with `step_link: None` → existing `Components([single_item])` path unchanged
- Unit: `{{vars.dir}}` substitution applied in `orchestrator_items[].effective_content`
- Integration: full intent match → correct channel split confirmed by asserting item class_codes

---

### Phase F — `handle_retrieve_docs` Upgrade (Rust handler)

**Status:** [ ] Pending

**Goal:** Upgrade the Rust handler behind `__retrieve_docs__` to use `fetch_for_turn`
and handle all four `FetchForTurnResult` variants. Register `__fetch_component__`.

#### Files to modify

- `crates/brassclaw_engine/src/executor/orchestrator.rs`

  **`handle_retrieve_docs`:**  
  Replace the legacy `RetrievalEngine::retrieve_context` call with
  `retrieval_source.fetch_for_turn()`. Handle all four variants:
  - `SplitResult`: apply `rust_items` to Rust execution context silently;
    format `orchestrator_items` + `type:text` step `info` text into `orchestrator_content`;
    return routing dict (see §0.9 shape).
  - `Components`: all items → `orchestrator_content` (no-match path, unchanged shape).
  - `ActionShortCircuit`: return `{ action_short_circuit: true, action_component_id, action_name }`.
  - `Disambiguation`: return `{ disambiguation: true, candidates }`.

  Return value is always a dict — not a list. The Python side already guards
  `isinstance(pkr, dict)` (from the existing `__assemble_prior_knowledge__` path).

  **Register `__fetch_component__(uuid: str, class_code: int)`:**  
  New host function. Handler calls `fetch_component_by_id(uuid, class_code)` directly.
  Returns a single item dict or `None`. Used by `call_action` nested lookups (§0.9).

#### Tests

- Unit: `SplitResult` → `orchestrator_content` contains Skills/PythonCode bodies and `type:text` info text; does NOT contain any ToolSkill bodies
- Unit: `ActionShortCircuit` → `action_short_circuit: true`, empty `orchestrator_content`
- Unit: `Components` (no-match) → `orchestrator_content` contains all items (baseline preserved)
- Unit: `Disambiguation` → `disambiguation: true` with candidates list
- Integration: `__fetch_component__(uuid, 16)` → correct Action item returned

---

### Phase G — Python Step-0 Upgrade + `call_action` Migration

**Status:** [ ] Pending

**Goal:** Replace the three-call step-0 block with the single `__retrieve_docs__` call.
Migrate `call_action` nested lookup to `__fetch_component__`.

#### Files to modify

- `crates/brassclaw_engine/orchestrator/default.py`
  - Replace step-0 block (lines ~994–1059) with v3 single-call pattern (§0.9).
  - Remove `__retrieve_docs__(goal, 5)` dead Action detection shim.
  - Remove `__list_skills__()` and `select_skills()` calls.
  - Add `_set_active_skills_from_matched_ids(matched_ids, state)` helper.
  - Replace `call_action` `__retrieve_docs__(nested_name, 1)` at line ~844 with
    `__fetch_component__(uuid, 16)` (UUID sourced from the BuildInstruction step).

#### Tests

- Unit: step-0 intent match → `orchestrator_content` injected; `__list_skills__` NOT called
- Unit: action short-circuit → `execute_action_by_id` called; `__retrieve_docs__` inner shim removed
- Unit: disambiguation → `handle_disambiguation` called
- Unit: no-match → UNION ALL `orchestrator_content` injected (baseline preserved)
- Integration: `call_action` using `__fetch_component__` → correct Action fetched by UUID

---

### Phase H — RecipeStage: `last_user_text` + Tier 0/1 Dispatch

**Status:** [ ] Pending

**Goal:** Activate the RecipeStage stub so it dispatches correctly for Tier 0, Tier 1,
and falls through to Tier 2 on no match.

#### Files to modify

1. `crates/brassclaw_agent_loop/src/state.rs` — add `last_user_text: Option<String>` to `LoopExecutionState`

2. `crates/brassclaw_agent_loop/src/executor/input.rs` — populate `last_user_text` from
   drained input (the last user message text seen this turn)

3. `crates/brassclaw_agent_loop/src/executor/recipe.rs` — replace stub with full dispatch:

   ```
   RecipeStage::process(state):
     user_text = state.last_user_text (skip if None)
     result = retrieval_source.fetch_for_turn(scope, user_text, budget, "02")

     match result:
       SplitResult { rust_items, orchestrator_items, routing }:
         if routing.wilson_lower >= 0.70 && !routing.llm_call_required:
           // Tier 0: no LLM
           apply rust_items to Rust execution context
           stash orchestrator_items in state for PromptStage bypass
           return RecipeStageOutcome::TierZero { routing }

         else:
           // Tier 1: inject hint, let LLM decide
           stash orchestrator_items as hint in state.recipe_hint
           return RecipeStageOutcome::Continue

       ActionShortCircuit { component_id, name }:
         execute Action directly
         return RecipeStageOutcome::ActionExecuted

       Components(_) | Disambiguation(_) | (no match):
         return RecipeStageOutcome::Continue   // Tier 2 — unchanged
   ```

4. `PromptStage`: if `state.recipe_hint` is set (Tier 1), inject it into prior_knowledge
   before calling `fetch_for_consumer`. If `RecipeStageOutcome::TierZero`, skip `PromptStage`
   and `ModelStage`.

#### Tests

- Unit: `last_user_text` populated by `InputStage` after draining input
- Integration: Tier 0 match (wilson ≥ 0.70, `llm_call_required: false`) → `PromptStage` and `ModelStage` skipped
- Integration: Tier 1 match (wilson < 0.70) → orchestrator hint injected, LLM called normally
- Integration: no match → falls through to full LLM (Tier 2 unchanged)

---

### Phase I — Q1 Validator Upgrades

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/component_validator.rs`

New dispatch cases:

| Class | Rules |
|-------|-------|
| 22 PythonCode | name format, non-empty content, soft 10k token budget, shell-injection scan |
| 23 ExtensionCatalogue | name format, non-empty `overview_doc`, ≥1 `task_group`, valid UUID syntax in `child_component_ids` |
| 21 Recipe (StepDescriptions) | call `instruction_builder::build_instruction` as pre-flight; reject on any `IbsError` with the parse message; all `include` UUIDs parse as UUID v4; no `snippet`-type steps; step numbers monotonically increasing; S7 guard |
| 1–3 Skills | `intent_examples` entries ≤ 512 chars, capped at 20; `required_skills` capped at 10, no self-reference |
| 16 Actions | `steps` JSONB validated against 13 known step types |

**The `link_formula` / `step_link` parse check uses the same `parse_step_link` function as
runtime.** Any parse error that would blow up at runtime becomes a Q1 error with the
full parse message. This is the primary correctness guard for formula authoring.

**§shell-guard — Recipes referencing `builtin.shell` or `builtin.spawn_subagent`:**  
If any `rust_steps[].include` UUID resolves to a ToolSkill whose `tool_name` is
`"builtin.shell"` or `"builtin.spawn_subagent"`, the Recipe **must** have
`llm_call_required: true`. Q1 returns a hard error if `llm_call_required: false` and
either tool appears in the rust channel. This prevents open-ended shell/spawn from
accidentally becoming a Tier 0 path.

**§capability-id — Tool rows from builtin bootstrap:**  
For class 0 (Tool) components with `source = "system"`, Q1 validates that `capability_id`
is non-empty and matches the pattern `^[a-z0-9_-]+\.[a-z0-9_.]+$` (e.g. `builtin.read_file`).
Tool rows without a `capability_id` that are authored (not system) pass without error —
`capability_id` is optional for user-authored custom tools.

#### Tests

- Unit: Recipe with `snippet`-type step → Q1 fail with `IbsError::UnpromotedSnippet`
- Unit: Recipe with unparseable UUID in `include` → Q1 fail
- Unit: Recipe with S7 violation → Q1 fail
- Unit: PythonCode with shell-injection pattern → Q1 fail
- Unit: PythonCode with >10k tokens → Q1 warn (soft limit)
- Unit: ExtensionCatalogue with empty `overview_doc` → Q1 fail
- Unit: Skill with `intent_examples` entry > 512 chars → Q1 fail
- Unit: valid StepDescriptions, valid `step_link` → Q1 pass
- Unit: §shell-guard: Recipe with `builtin.shell` ToolSkill in rust channel + `llm_call_required: false` → Q1 fail
- Unit: §shell-guard: Recipe with `builtin.spawn_subagent` + `llm_call_required: false` → Q1 fail
- Unit: §shell-guard: Recipe with `builtin.shell` + `llm_call_required: true` → Q1 pass
- Unit: §capability-id: Tool row `source = "system"`, empty `capability_id` → Q1 fail
- Unit: §capability-id: Tool row `source = "authored"`, no `capability_id` → Q1 pass (optional for user tools)

---

### Phase J — Skill `intent_examples` + `required_skills`

**Status:** [ ] Pending

#### Files to modify

- `crates/brassclaw_skills/src/types.rs` — add `intent_examples: Vec<String>` (≤512 chars each,
  capped at 20) and `required_skills: Vec<String>` (capped at 10, no self-reference) to
  `SkillManifest`; enforce limits in `ActivationCriteria::enforce_limits`.
- `crates/brassclaw_skills/src/` — on skill `auto_passed` transition: call `seed_intent_input`
  for each intent expression.
- On skill wipe/delete: call `purge_component_inputs(component_id)`.
- IBS: when including a Skill step in orchestrator_steps, also include all declared
  `required_skills` UUIDs in the same channel.

#### Files to create

- `crates/brassclaw_pg/migrations/V051__reborn_skills_intent_examples.sql`
  ```sql
  ALTER TABLE reborn_skills ADD COLUMN IF NOT EXISTS intent_examples JSONB;
  ALTER TABLE reborn_skills ADD COLUMN IF NOT EXISTS required_skills JSONB;
  ```

#### Tests

- Unit: `SkillManifest` with `intent_examples` + `required_skills` YAML roundtrip
- Unit: entry > 512 chars → rejected by `enforce_limits`
- Integration: Skill with `intent_examples` → resolves via `resolve_intent`
- Integration: Skill with `required_skills: ["pipe-skill"]` → both Skills in `orchestrator_items`

---

### Phase K — BasicPromptStore + MCP Translation + Cleanup

**Status:** [ ] Pending

#### K.1 BasicPromptStore

**Migration V052.**

```sql
CREATE TABLE reborn_basic_prompt_store (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     TEXT NOT NULL,
    user_id       TEXT,
    agent_id      TEXT,
    project_id    TEXT,
    fingerprint   TEXT NOT NULL,   -- SHA-256 of bundle content
    bundle_json   JSONB NOT NULL,
    is_stale      BOOLEAN NOT NULL DEFAULT false,
    assembled_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, user_id, agent_id, project_id)
);
```

New `PgBasicPromptStore` facade: `get_for_scope`, `store`, `mark_stale`, `delete`.  
Wire into Interceptor to prepend stored bundle before LLM shipment.  
On any component `validated` transition: call `mark_stale(scope)`.

#### K.2 MCP Translation Layer — External MCPs Only

**File:** `crates/brassclaw_extensions/src/mcp_translation.rs` (new)

> **Scope:** This translator handles **external third-party MCP servers only**.
> Builtin first-party tools (`builtin.*`) are seeded by the separate `builtin_bootstrap.rs`
> in Phase L — with hand-authored content and system-level validation bypass.
> Do NOT run the MCP translator against builtin tools.

For each external MCP tool: generate Tool (class 0), ToolSkill (class 13), Skill (class 1), and
a skeleton Recipe (class 21) with auto-generated StepDescriptions:
- Step 1: `knowledge: orchestrator`, `type: text`, `info`: auto-generated task context (from MCP tool description)
- Step 2: `knowledge: rust`, `type: component`, `include`: [ToolSkill UUID]
- Step 3: `knowledge: orchestrator`, `type: component`, `include`: [Skill UUID]
- Default `step_link: "0:0-0:E"`

One ExtensionCatalogue (class 23) grouping all generated components.
All inserted with `validation_status = 'pending'` — external MCP content must go through Q1 + Q2.

**Why external MCPs are treated differently from builtins:**
- MCP content comes from untrusted third-party servers — must pass Q1 injection scan and Q2 manual review.
- Skill bodies are auto-generated stubs and need human review before becoming active.
- No `capability_id` is set — external MCPs are referenced by UUID, not by a registered capability name.
- `source = "imported"` (not `"system"`).

#### K.3 Cleanup

- Remove `__assemble_prior_knowledge__` handler registration from `orchestrator.rs`.
  It is superseded by the upgraded `__retrieve_docs__`. Remains callable for backward
  compatibility with custom orchestrators for one release cycle; then removed.
- Remove step-0 shim comment block from `default.py`.
- Add deprecation notice to `__list_skills__`: no longer called from default step-0;
  remains callable for external/custom orchestrators.
- Remove dead `__retrieve_docs__(goal, 5)` Action-detection shim (lines ~1018–1028 in `default.py`).

#### Tests (K.1)

- Integration: `store` → `get_for_scope` returns bundle
- Integration: component `validated` → `is_stale = true`
- Integration: Interceptor prepends stored bundle before LLM shipment

#### Tests (K.2)

- Unit: MCP payload with 3 tools → 3 Tool + 3 ToolSkill + 3 Skill + 3 Recipe + 1 ExtensionCatalogue components created
- Integration: MCP install → components enter validation queue with `status = 'pending'`

---

### Phase L — Builtin Tool Bootstrap Seeder

**Status:** [ ] Pending

**Goal:** Seed the full v3 component stack for all 23 builtin tools at first boot.
This is a separate concern from the Phase K MCP translator. The MCP translator targets
external third-party MCP servers (unknown shape, must enter Q1/Q2). The builtin bootstrap
targets the 23 first-party tools: content is hand-authored, quality is guaranteed at
compile time, and Q2 manual review is bypassed (`source = "system"`, `validation_status = "validated"`).

#### L.1 New migration: V053

```sql
-- V053__reborn_tools_capability_id_and_system_source.sql
ALTER TABLE reborn_tools ADD COLUMN IF NOT EXISTS capability_id TEXT;
CREATE INDEX IF NOT EXISTS reborn_tools_capability_id_idx
    ON reborn_tools (tenant_id, user_id, agent_id, project_id, capability_id)
    WHERE capability_id IS NOT NULL;

-- Allow "system" as a source value (alongside existing: authored, extracted, migrated, imported)
ALTER TABLE reborn_tools
    DROP CONSTRAINT IF EXISTS reborn_tools_source_check,
    ADD CONSTRAINT reborn_tools_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
ALTER TABLE reborn_tool_skills
    DROP CONSTRAINT IF EXISTS reborn_tool_skills_source_check,
    ADD CONSTRAINT reborn_tool_skills_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
ALTER TABLE reborn_skills
    DROP CONSTRAINT IF EXISTS reborn_skills_source_check,
    ADD CONSTRAINT reborn_skills_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
```

`capability_id` links a `reborn_tools` row back to the Rust capability registry
(`"builtin.read_file"`, etc.) without fragile name-search. The Rust execution layer
uses this when resolving a Tool UUID to the registered handler.

#### L.2 New file: `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`

Seeder function: `pub async fn seed_builtin_components(pool: &PgPool, scope: &ComponentScope)`

**Structure:**
```
seed_builtin_components(pool, scope):
  if builtin components already exist for this scope → return (idempotent)
  
  for each builtin group (filesystem, network, memory, process, management):
    1. Insert ExtensionCatalogue row (validated, source=system)
    2. For each tool in group:
         Insert Tool row (capability_id = "builtin.X", source=system, validated)
         Insert ToolSkill row (tool_name = "builtin.X", source=system, validated)
    3. For each task-level Skill in group:
         Insert Skill row (body = hand-authored content, source=system, validated)
         Seed intent_examples into reborn_intent_inputs
    4. For each PythonCode helper in group:
         Insert PythonCode row (source=system, validated)
    5. For each Recipe in group (per §0.16.1):
         Insert Recipe row + step_descriptions JSONB
         Run IBS build_instruction as pre-flight (panics in debug builds on IbsError)
         Seed intent_examples into reborn_intent_inputs with correct step_link
         Insert Recipe row (source=system, validated)
```

**Hand-authored content** lives as `include_str!()` markdown files in
`crates/brassclaw_engine/prompts/builtin/` — one file per component body (ToolSkill,
Skill, PythonCode, ExtensionCatalogue overview_doc). This keeps them out of Rust source
and editable without recompiling.

**Called from** composition boot sequence, analogous to `component_import.rs`.

#### L.3 Content files to create

| Path | Component |
|------|-----------|
| `prompts/builtin/toolskill_read_file.md` | ToolSkill: annotated param schema, path rules, output size limits |
| `prompts/builtin/toolskill_write_file.md` | ToolSkill: write modes, max 6 MiB, overwrite vs create |
| `prompts/builtin/toolskill_list_dir.md` | ToolSkill: output format, scope boundary |
| `prompts/builtin/toolskill_glob.md` | ToolSkill: v1 glob syntax, result ordering, scope |
| `prompts/builtin/toolskill_grep.md` | ToolSkill: regex syntax, output modes, scope |
| `prompts/builtin/toolskill_apply_patch.md` | ToolSkill: exact vs fuzzy match, retry semantics, error codes |
| `prompts/builtin/toolskill_shell.md` | ToolSkill: **approval required**, 120s limit, quoting rules, when to prefer structured tools |
| `prompts/builtin/toolskill_http.md` | ToolSkill: 15 MiB cap, 10s/30s timeout, redirect handling |
| `prompts/builtin/toolskill_http_save.md` | ToolSkill: same as http + filesystem write scope |
| `prompts/builtin/toolskill_memory_search.md` | ToolSkill: query format, score interpretation, budget |
| `prompts/builtin/toolskill_memory_write.md` | ToolSkill: key format, TTL, scope |
| `prompts/builtin/toolskill_memory_read.md` | ToolSkill: exact key lookup vs semantic search |
| `prompts/builtin/toolskill_memory_tree.md` | ToolSkill: output format, traversal depth |
| `prompts/builtin/toolskill_skill_list.md` | ToolSkill: output format, scope isolation |
| `prompts/builtin/toolskill_skill_install.md` | ToolSkill: URL format, enters pending → Q1 → Q2 |
| `prompts/builtin/toolskill_skill_remove.md` | ToolSkill: irreversible, scope isolation |
| `prompts/builtin/toolskill_trigger_create.md` | ToolSkill: cron/interval format, ExternalWrite effect |
| `prompts/builtin/toolskill_trigger_list.md` | ToolSkill: scope filtering |
| `prompts/builtin/toolskill_trigger_remove.md` | ToolSkill: ExternalWrite, irreversibility |
| `prompts/builtin/toolskill_spawn_subagent.md` | ToolSkill: **scope isolation**, budget inheritance, auth model |
| `prompts/builtin/toolskill_echo.md` | ToolSkill: trivial passthrough |
| `prompts/builtin/toolskill_time.md` | ToolSkill: operations list, ISO 8601 format |
| `prompts/builtin/toolskill_json.md` | ToolSkill: parse/stringify/query/validate, jq-style queries |
| `prompts/builtin/skill_filesystem.md` | Skill: when to use read_file vs glob vs grep; task patterns |
| `prompts/builtin/skill_file_editing.md` | Skill: read → patch → verify flow; when to use exact vs fuzzy |
| `prompts/builtin/skill_file_search.md` | Skill: combined glob + grep patterns for code search tasks |
| `prompts/builtin/skill_http.md` | Skill: fetch vs download; API vs page; sanitize response |
| `prompts/builtin/skill_memory.md` | Skill: when to search vs read vs tree; write-back rules |
| `prompts/builtin/skill_memory_navigation.md` | Skill: how to use tree before diving into memory |
| `prompts/builtin/skill_shell.md` | Skill: prefer structured tools; only use shell when unavoidable; always confirm destructive commands |
| `prompts/builtin/skill_subagent.md` | Skill: when to delegate; how to frame child goal; handle child result |
| `prompts/builtin/skill_skill_management.md` | Skill: install/remove semantics; user confirmation required; lifecycle |
| `prompts/builtin/skill_trigger_management.md` | Skill: schedule vs ask; cron syntax; scope model |
| `prompts/builtin/pythoncode_json_helper.md` | PythonCode: chain json.query + json.stringify patterns |
| `prompts/builtin/pythoncode_time_helper.md` | PythonCode: common time format/diff patterns |
| `prompts/builtin/pythoncode_patch_formatter.md` | PythonCode: format LLM edit intent into search-replace patch object |
| `prompts/builtin/cat_filesystem.md` | ExtensionCatalogue overview_doc for builtin-filesystem |
| `prompts/builtin/cat_network.md` | ExtensionCatalogue overview_doc for builtin-network |
| `prompts/builtin/cat_memory.md` | ExtensionCatalogue overview_doc for builtin-memory |
| `prompts/builtin/cat_process.md` | ExtensionCatalogue overview_doc for builtin-process |
| `prompts/builtin/cat_management.md` | ExtensionCatalogue overview_doc for builtin-management |

#### Tests

- Unit: seeder runs on empty DB → 85–90 component rows inserted
- Unit: seeder runs twice → idempotent (same row count, no duplicates)
- Unit: all inserted Recipes pass `build_instruction` pre-flight with no `IbsError`
- Unit: `builtin.shell` ToolSkill body contains "approval" text (safety content regression guard)
- Unit: `builtin.spawn_subagent` ToolSkill body contains "scope isolation" text
- Unit: every inserted Recipe with shell in rust channel has `llm_call_required = true`
- Integration: `resolve_intent("read the file at /tmp/foo.txt")` → matches `builtin-read-file` Recipe
- Integration: `resolve_intent("show me all files")` → matches `builtin-list-dir` or `builtin-find-files`
- Integration: Tool row `capability_id = "builtin.read_file"` → look up by `capability_id` returns correct UUID

---

## 2. Migration Sequence

| Migration | Contents | Status |
|-----------|----------|--------|
| `V047__reborn_recipe_step_descriptions.sql` | `ADD COLUMN step_descriptions JSONB` to `reborn_recipes` | **Next** |
| `V048__reborn_python_code.sql` | New table `reborn_python_code`, class 22 | |
| `V049__reborn_extension_catalogues.sql` | New table `reborn_extension_catalogues`, class 23 | |
| `V050__reborn_intent_inputs_step_link.sql` | `ADD COLUMN step_link TEXT` to `reborn_intent_inputs` | |
| `V051__reborn_skills_intent_examples.sql` | `ADD COLUMN intent_examples JSONB; ADD COLUMN required_skills JSONB` to `reborn_skills` | |
| `V052__reborn_basic_prompt_store.sql` | New table: one row per scope, `bundle_json JSONB`, `is_stale BOOL`, `fingerprint TEXT` | |
| `V053__reborn_tools_capability_id_and_system_source.sql` | `ADD COLUMN capability_id TEXT` to `reborn_tools` + `source = 'system'` allowed on tools/tool_skills/skills | |

All additive. No DROP, no renames. No existing rows break.

**`step_link` is nullable** — existing intent rows without it use the existing
`fetch_component_by_id` path unchanged. Zero breakage on upgrade.

**`capability_id` is nullable** — existing Tool rows (user-authored) are unaffected.
Only system-seeded builtin rows carry a `capability_id`.

**No `reborn_pending_rust_context` table.** The earlier transient-table design is
superseded: `SplitResult.rust_items` is delivered directly by `RecipeStage` at runtime,
avoiding the DB round-trip entirely.

---

## 3. Open Questions

| # | Question | Recommendation |
|---|----------|----------------|
| 1 | Variable extraction: named capture groups vs. post-match LLM extraction? | Named capture groups for Phase A. LLM extraction as opt-in fallback when `variable_patterns` returns no captures and the variant carries a `llm_var_extraction_prompt` field. Keep the fast path fast. |
| 2 | BuildInstruction memoisation: per-process or always recompute? | Per-process. Key: `sha256(step_link + "\|" + sorted_include_uuids.join(","))`. Evict on any referenced component's `updated_at` change, Recipe `updated_at` change, or `step_formatter_id` component `updated_at` change. IBS is pure-Rust; memoisation avoids repeated UUID DB fetches for hot intents. |
| 3 | `required_skills` inclusion: always include vs. score against current query? | Always include; cap at 10. |
| 4 | `step_formatter_id` scope: per-recipe, per-variant, or per-step? | Per-recipe. Formatting style is consistent across a capability domain. |
| 5 | StepDescription storage format: YAML files in git vs. JSONB in `reborn_recipes`? | JSONB column (simpler, no file management). YAML-formatted text preserved inside the JSONB for human readability and WebUI rendering. |
| 6 | Legacy DocPlan → v3 translation? | See §3.1. |
| 7 | `__assemble_prior_knowledge__` removal timing? | Keep registered for one release cycle after Phase K ships. Document as deprecated in Phase K. Remove in the following cycle. |
| 8 | Should builtin Tool/ToolSkill/Skill rows bypass Q2 (auto-validated)? | Yes — `source = "system"`, `validation_status = "validated"` at seeder insert. Q1 runs inside the seeder at build time. Q1 errors in seeder content are CI build failures. This prevents boot from requiring human Q2 completion for core tools. |
| 9 | Should the MCP translator also be used for builtins? | No — wrong granularity (1:1 per tool, no task-level Skills, no PythonCode, no multi-ToolSkill Recipes). Use `builtin_bootstrap.rs` (Phase L) for builtins. MCP translator is for external third-party MCPs only. |
| 10 | What recipe variants should `builtin.shell` have? | Two: (a) known-safe commands (allowlist: `cargo build/test/fmt/clippy`, `git status/log/diff`, `npm install/build`) at Tier 1 high-confidence; (b) open-ended arbitrary command at Tier 1 always with explicit approval annotation. Both have `llm_call_required: true` — no shell is ever Tier 0. |
| 11 | How does the Rust execution layer resolve a Tool DB UUID to its registered capability handler? | Via `capability_id` column (V053). On tool dispatch: look up Tool row by UUID → read `capability_id` → look up handler in `FirstPartyCapabilityRegistry` by `capability_id`. For user-authored tools without `capability_id`, fall back to existing name-based resolution. |
| 12 | Should builtin Recipes also have `source = "system"` and bypass Q2? | Yes — same reasoning as Q8. Builtin Recipe StepDescriptions are hand-authored and IBS pre-flight-checked at seeder run time. Q2 bypass for `source = "system"` Recipes is consistent with Tools and ToolSkills. |

### 3.1 Legacy DocPlan → v3 Component Translation

The v2 system stored all knowledge as `MemoryDoc` rows. `component_import.rs` already
migrates these to class-specific tables. The gap: no v2 docs have StepDescriptions or
`step_link` formulae.

**One-time, operator-triggered pipeline (`brassclaw migrate-to-v3`):**

1. **Skills (class 1–3):** Extract imperative sentence candidates from skill `body` as
   seed intent examples. Generate StepDescription0:
   - Step 1: `knowledge: orchestrator`, `type: text`, `info`: summarised body
   - Step 2: `knowledge: orchestrator`, `type: component`, `include`: [skill UUID]
   Default `step_link: "0:0-0:E"`. Route to Q1.

2. **ToolSkills (class 13):** Generate StepDescription0 with one rust-channel `component`
   step. No intent examples (ToolSkills are referenced by Skills, not intent-matched directly).

3. **Existing Recipes (class 21) without `step_descriptions`:** Map each entry in the
   existing `steps` JSONB (13-type Action format) to the nearest StepDescription type.
   Steps with no component reference → `type: text`. Steps with a component reference →
   `type: component` with existing UUID in `include`. Route to Q1.

4. **Specs, Lessons, Notes:** Leave as-is. Served by the UNION ALL path; no StepDescriptions needed.

Idempotent: components that already have `step_descriptions` are skipped.

### 3.2 Builtin Tool Bootstrap Pipeline

This is a **separate, automatic pipeline** that runs at every boot (not operator-triggered).
It seeds the full v3 component stack for the 23 first-party builtin tools if not already present.
See §0.16 for the full specification and Phase L for the implementation plan.

**Relationship to §3.1:**  
The `brassclaw migrate-to-v3` pipeline (§3.1) handles user-authored v2 documents.  
The builtin bootstrap (§3.2 / Phase L) handles system tools that never had v2 representations.  
They are independent — running one does not affect the other.

**Idempotency guard:** The seeder checks `SELECT COUNT(*) FROM reborn_tools WHERE source = 'system'`
for the current scope at boot. If ≥ 1 row exists, the seeder skips entirely. A full re-seed
can be triggered by deleting system-sourced rows (operator action only).

---

## 4. Out of Scope (Marked Postponed)

- Full self-improvement pipeline (Interceptor-driven Recipe auto-creation)
- Component self-creation wizard
- Automatic Sempai-driven prompt rewrites
- LLM-based variable extraction fallback (Phase A uses regex only)
- Tier 0 production activation (requires Phases A–H complete + Wilson scoring validated in production for ≥2 weeks)
- `FormatOrchestratorPrompt` as a distinct step type (handled via `step_formatter_id` in the IBS)

---

## 5. Turn Flow Summary

### Tier 0 (intent match, no LLM)

```
User types: "show all files including hidden in /tmp"
│
├─ [InputStage]
│   state.last_user_text = "show all files including hidden in /tmp"
│
├─ [RecipeStage]
│   fetch_for_turn(scope, last_user_text, budget, "02")
│     → resolve_intent → Match { component_id: uuid-local-files-reading,
│                                class_code: 21,
│                                step_link: "0:0-0:30+1:0-1:E" }
│     → fetch step_descriptions[0] (steps 0..30) + step_descriptions[1] (all)
│     → IBS: build_instruction("0:0-0:30+1:0-1:E", step_descriptions, variable_patterns)
│         variable capture: dir="/tmp", flags="-la"
│         rust_steps:         [component(uuid-ls-toolskill, knowledge:rust)]
│         orchestrator_steps: [text("info…"), component(uuid-ls-skill),
│                               component(uuid-ls-result-handler)]
│     → fetch rust_items:         [ComponentItem: ls-toolskill body]
│     → fetch orchestrator_items: [ComponentItem: ls-skill body,
│                                   ComponentItem: ls-result-handler body]
│     → FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }
│   routing.wilson_lower = 0.82, llm_call_required = false → Tier 0
│   apply rust_items to Rust execution context (silent, never forwarded to orchestrator)
│   stash orchestrator_items in state
│   return RecipeStageOutcome::TierZero
│
├─ [PromptStage skipped — Tier 0]
├─ [ModelStage  skipped — Tier 0]
│
│   Orchestrator (Python) receives pkr from __retrieve_docs__:
│     pkr["orchestrator_content"]:
│       - Step 1 info text (task context annotation)
│       - ls-skill body
│       - ls-result-handler (PythonCode) body
│     pkr["matched_component_ids"]: [uuid-ls-skill, uuid-ls-result-handler]
│     pkr["override_prompt_creation"]: true
│   → No ToolSkill bodies. No memories. No UNION ALL noise.
│   → _set_active_skills_from_matched_ids([uuid-ls-skill, uuid-ls-result-handler], state)
│   → Orchestrator invokes ls-skill → instructs Rust executioner: ls /tmp -la
│   → Rust reads ls-toolskill (pre-loaded in execution context), calls ls -la /tmp
│   → Rust returns stdout to orchestrator
│   → Orchestrator runs ls-result-handler PythonCode → formats output for chat
│
├─ [InterceptorStage]  Saves composition plan. Sempai reviews if connected.
└─ [AssistantReplyStage]  Emits formatted directory listing. Wilson score updated.
```

### Tier 2 (no match — full LLM)

```
User types: "explain recursion to me"
│
├─ [InputStage]        last_user_text set
├─ [RecipeStage]       fetch_for_turn → NoMatch → fetch_for_consumer (UNION ALL)
│                       → FetchForTurnResult::Components([...])
│                       → RecipeStageOutcome::Continue (Tier 2, unchanged)
├─ [PromptStage]       normal assembly; all UNION ALL items in orchestrator_content
│                       volatile context injected separately (never mixed with prior knowledge)
├─ [InterceptorStage]  Sempai reviews if connected
├─ [ModelStage]        full LLM call
└─ [AssistantReplyStage]  emit LLM response (no Recipe outcome recorded)
```

### Action short-circuit (class 16)

```
User types: "run the daily-sync action"
│
├─ [InputStage]        last_user_text set
├─ [RecipeStage]       fetch_for_turn → Match { class_code: 16 }
│                       → FetchForTurnResult::ActionShortCircuit { component_id, name }
├─ [PromptStage skipped]
├─ [ModelStage  skipped]
│   Orchestrator receives: pkr["action_short_circuit"]: true
│   → execute_action_by_id(action_component_id, goal, state)
└─ [AssistantReplyStage]  Emits action result.
```
