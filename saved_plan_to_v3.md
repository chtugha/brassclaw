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
│  Explains WHAT this capability domain is for, categorises       │
│  its Recipes by task group, lists which Recipes exist.          │
│  Does NOT duplicate how-to; the components below already do.    │
├─────────────────────────────────────────────────────────────────┤
│  Recipe (class 21)                                              │
│  The primary intent target. An exact, ordered assembly plan.    │
│  Each variant encodes: which components to fetch, in what order,│
│  with what variable bindings — to build a focused              │
│  prior_knowledge patch for one specific user intent.            │
│                                                                 │
│  StepDescription (authoring layer, §0.14)                       │
│  Human-editable YAML source: what each step does, which         │
│  component is needed, who reads it (orchestrator vs. rust).     │
│  Compiled into BuildInstruction by the IBS at intent-match time.│
├─────────────────────────────────────────────────────────────────┤
│  Skill (class 1–3)         │  Action (class 16)                 │
│  Orchestrator instructions │  LLM-free execution sequences.     │
│  for using one Rust tool.  │  No LLM call needed.               │
│                            │                                    │
│  PythonCode (class 22) [NEW]                                    │
│  Code elements and inline instructions for the orchestrator     │
│  that are not full Skills (utilities, transforms, helpers).     │
├─────────────────────────────────────────────────────────────────┤
│  ToolSkill (class 13)                                           │
│  Instructions for the Rust execution layer: how to call a tool, │
│  its param schema, preconditions, error handling.               │
│  The orchestrator cannot call tools directly — it must go       │
│  through a Skill which references the ToolSkill.                │
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

An ExtensionCatalogue does **not** re-document commands. Every component it owns
already documents itself. The Catalogue draws the **bigger picture**:
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
There is no `RecipeVariant`, `BuildInstruction`, `StepDescription`, or `StepOwner`.
Phase C replaces this with the v3 design. The existing `trigger` + `steps` fields
are **preserved** as the Tier-1 / Tier-2 fallback so old Recipes continue to work.

#### How a Recipe works (v3 complete flow)

```
Author:
  1. Author writes StepDescriptions in WebUI (YAML-structured, human-readable).
  2. Each intent expression gets a link_formula pointing into StepDescriptions.

Intent match (runtime):
  1. resolve_intent(user_text) → Match { recipe_id, variant_key, link_formula }
  2. InstructionBuilder::build_from_formula(recipe_id, link_formula, user_text)
       → parses link_formula → loads StepDescription segments → compiles BuildInstruction
  3. RetrievalEngine::fetch_by_instruction(scope, build_instruction, user_text, budget)
       → fetches component bodies (Section A)
  4. BuildInstruction is split into three typed outputs:
       → OrchestratorContext   → fed into __assemble_prior_knowledge__ PKC
       → RustContext           → written to reborn_pending_rust_context
       → fetch_steps[]         → component bodies returned to PromptStage
  5. Orchestrator reads its tailored prior-knowledge.
  6. Rust layer reads its compact JSON package.
```

#### Mandatory shape

| Field | Content |
|-------|---------|
| `name` | Recipe identifier (e.g. `local-files-reading`) |
| `description` | One-sentence summary |
| `category` | Task group → `ExtensionCatalogue.task_groups[].group_name` |
| `step_descriptions JSONB` | Array of `StepDescriptionN` (YAML text + parsed fields) |
| `variants[]` | One or more `RecipeVariant` entries |
| `trigger` / `steps` | **Kept** — v2 fallback path |

#### Intent Variants

Each variant:
- Owns its own intent expressions (rows in `reborn_intent_inputs`)
- Carries a `link_formula` specifying which StepDescription ranges to compile
- The `BuildInstruction` is computed at runtime by the IBS, NOT stored as a blob

**Example — Recipe `local-files-reading`:**

```
variant: ls-l
  intents: ["ls -l", "show me all files", "list files", "show local directory files"]
  link_formula: "0:0-0:E"               ← all of Stepdescription0

variant: ls-la
  intents: ["ls -la", "show all files including hidden", "list all files"]
  link_formula: "0:0-0:30+1:0-1:E"      ← base 0..30, then all of Stepdescription1

variant: ls-other-dir
  intents: ["list files of the /tmp directory", "show files in {{vars.dir}}"]
  variable_patterns: [{ name: "dir", pattern: r"of the (?P<dir>[/\w.-]+)" }]
  link_formula: "0:0-0:31+2:0-2:E"      ← base 0..31, then all of Stepdescription2
```

---

### 0.4 BuildInstruction — Three-Audience Design

> **Key design principle:** A `BuildInstruction` serves **three distinct runtime readers**.
> Each reader gets a typed section containing exactly what it needs — nothing more.

#### Codebase reality (grounds this design)

**The Python orchestrator** (`default.py`, 1262 lines) has **zero built-in knowledge**
of tools or skills. Every turn it calls `__assemble_prior_knowledge__(goal, budget, class)`
to get its PKC from Rust. The `OrchestratorContext` must be serialized into this
callback's response.

**The Rust executioner** has no pre-loaded tool knowledge. Tools are resolved on-demand
via `LeaseManager::active_for_thread()`. ToolSkill bodies are DB-fetched at dispatch time.
The `RustContext` must be delivered via a transient per-turn table (§0.17).

#### Three readers, three typed sections

**Section A — RetrievalEngine** (`fetch_steps[]`):  
Consumed by `fetch_by_instruction`. Reads `FetchComponents` steps to know which component
UUIDs to load from the DB into the context window.

**Section B — Orchestrator** (`orchestrator_context: OrchestratorContext`):  
Serialized into the `formatted_content` surface of `__assemble_prior_knowledge__`.
Contains: Skill UUIDs to fetch, PythonCode UUIDs to fetch, `step_formatter_id`
(optional PythonCode UUID that reformats step descriptions into LLM-optimal prose),
and orchestrator control-flow steps.

**Section C — Rust Layer** (`rust_context: RustContext`):  
Written to `reborn_pending_rust_context` (V053). Contains: ToolSkill UUIDs and
`ToolBinding[]` (tool_name + params + `ErrorPolicy`). The orchestrator never sees this.

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
```

#### Structure

```
BuildInstruction
├── llm_call_required: bool           ← false for Tier0/Actions, true for Tier1+
├── variable_patterns[]               ← applied before any section is read
├── basic_prompt_section_refs[]       ← navigation hints into cached basic-prompt
│
├── fetch_steps[]                     ← SECTION A: RetrievalEngine reads this only
│   └── FetchComponentsStep { step_id, context_spec: StepContextSpec, description }
│
├── orchestrator_context              ← SECTION B: serialized into PKC __assemble_prior_knowledge__
│   ├── skill_ids[]                   ← Skill component UUIDs → fetch_component_by_id
│   ├── python_code_ids[]             ← PythonCode component UUIDs → fetch_component_by_id
│   ├── step_formatter_id             ← Optional PythonCode UUID: reformats step descriptions
│   └── control_flow_steps[]          ← RunPythonCode, ConditionalBranch, SetVariable, etc.
│
└── rust_context                      ← SECTION C: written to reborn_pending_rust_context
    ├── tool_skill_ids[]              ← ToolSkill component UUIDs
    └── tool_bindings[]               ← ToolBinding[] with ErrorPolicy per invocation
```

**Invariant:** Sections must not overlap. An orchestrator step never references a ToolSkill.
A Rust ToolBinding never references a Skill.

#### Complete example — Recipe `local-files-reading`, variant `ls-la`

(All examples use the recipe whose intents match "show me all files of the current directory")

```
BuildInstruction for variant: ls-la
  (intent: "show all files including hidden in the current directory")

llm_call_required: false   ← Tier 0: skip LLM, execute directly
variable_patterns: []      ← no vars in this variant

# ── SECTION A: RetrievalEngine ──────────────────────────────────────────
fetch_steps:
  - step_id: "fetch-ls"
    context_spec:
      component_ids:
        - "<uuid:skill-ls>"
        - "<uuid:pythoncode-ls>"
        - "<uuid:toolskill-ls>"
    description: "Load ls skill, orchestrator PythonCode, and ls ToolSkill"

# ── SECTION B: Orchestrator ──────────────────────────────────────────────
orchestrator_context:
  skill_ids:
    - "<uuid:skill-ls>"
  python_code_ids:
    - "<uuid:pythoncode-ls>"
  step_formatter_id: "<uuid:terse-cli-formatter>"   ← optional; omit for raw descriptions
  control_flow_steps:
    - step_id: "exec-ls"
      step_type: RunPythonCode
      component_id: "<uuid:pythoncode-ls>"
      description: "Use skill-ls to call the rust executioner; write output to chat"

# ── SECTION C: RustLayer ─────────────────────────────────────────────────
rust_context:
  tool_skill_ids:
    - "<uuid:toolskill-ls>"
  tool_bindings:
    - tool_name: "ls"
      params: { flags: "-la" }
      error_policy: { policy: "fail" }
```

---

### 0.5 StepContextSpec — Per-Step Context Narrowing

```rust
/// Attached to FetchComponentsStep. Tells RetrievalEngine precisely what to load.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepContextSpec {
    /// Exact component UUIDs → fetch_component_by_id.
    #[serde(default)]
    pub component_ids: Vec<String>,
    /// Coarse class codes → narrow UNION ALL (usually empty when component_ids is set).
    #[serde(default)]
    pub class_codes: Vec<i32>,
}
```

---

### 0.6 The RetrievalEngine and `fetch_by_instruction`

#### Current state (grounded in code)

`RetrievalSource` has two methods today:
- `fetch_for_consumer` — keyword-scored UNION ALL, consumer-tag filtered
- `fetch_for_turn` — intent-resolution then `fetch_component_by_id`, falls back to `fetch_for_consumer`

`fetch_for_turn` returns a single matched component. It does not execute a `BuildInstruction`; that is entirely new in v3.

`fetch_component_by_id` handles classes 1–3, 4–9, 12–21 (0 returns None).
Classes 22 and 23 added in Phases A and B.

#### New method: `fetch_by_instruction`

```rust
/// Execute a BuildInstruction's fetch plan (Section A only).
/// Returns ordered prior_knowledge patch for injection as memory_snippets.
/// Variable substitution is applied to each component's effective_content.
/// Default implementation falls back to fetch_for_consumer (RamSource / tests).
async fn fetch_by_instruction(
    &self,
    scope: &ComponentScope,
    instruction: &BuildInstruction,
    user_text: &str,
    token_budget: usize,
) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
    self.fetch_for_consumer(scope, user_text, token_budget, "02").await
}
```

`PostgresSource` overrides this:
1. Extract variables from `user_text` via `instruction.variable_patterns`.
2. Iterate `instruction.fetch_steps`; collect all `FetchComponents` steps.
3. For each `context_spec.component_ids`: call `fetch_component_by_id(uuid)`.
4. For each `context_spec.class_codes`: run narrow UNION ALL.
5. Apply `{{vars.name}}` substitution to each `ComponentItem.effective_content`.
6. Return in fetch-step order, capped to `token_budget`.

#### Updated `fetch_for_turn` flow

```
fetch_for_turn(scope, query, budget, sender_class)
  → resolve_intent(scope, query)
      → Match { component_id, class_code, variant_key, link_formula }
          → InstructionBuilder::build_from_formula(recipe_id, link_formula, query)
          → fetch_by_instruction(scope, &build_instruction, query, budget)
          → write rust_context → reborn_pending_rust_context
          → return FetchForTurnResult::Components(orchestrator_patch)
      → Disambiguation { candidates }
          → return FetchForTurnResult::Disambiguation(candidates)
      → NoMatch | DbLessFallback
          → fetch_for_consumer(...)
          → return FetchForTurnResult::Components(broad_scan)
```

`IntentResolution::Match` must carry `variant_key: Option<String>` and `link_formula: Option<String>`.

---

### 0.7 Current Turn Pipeline (Actual Code)

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

---

### 0.8 Normal Assembly — No-Match Path (UNION ALL weights)

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

Bold rows are new additions for v3.

---

### 0.9 Actions — LLM-Bypass

Actions (class 16) already default to `override_prompt_creation = true` in V029.
When an Action is the matched component, its `BuildInstruction` has
`llm_call_required: false`. The orchestrator executes steps directly.

---

### 0.10 KV-Cache / LMCache-Aware Design

**Basic-prompt:** Pre-assembled `InstructionBundle` stored in `reborn_basic_prompt_store`
(V051). Manual trigger only. Stale when any component passes Gate 2.

**BuildInstruction patch rules:**
- Must NOT repeat content already in the stored basic-prompt.
- `basic_prompt_section_refs` carries navigation hints (pointers, not content).
- Target patch size: < 4 k tokens.
- Orchestrator patch: PRIORITY 2 (instruction snippets) in the bundle.
- Memory: PRIORITY 3 (memory snippets).
- Rust context: transient table, not in the bundle at all.

---

### 0.11 Extensions as Plugins — Translation Layer

MCP → BrassClaw native: Tool (0) → ToolSkill (13) → Skill (1) → Recipe (21) → ExtensionCatalogue (23).
All at `pending`, through Q1+Q2 before active.

---

### 0.12 Interceptor System

Saves each turn's prompt composition plan (`BuildInstruction` + assembled patch).
If Sempai connected: reviews before shipping to Kohai. Can flag patterns for Recipe creation.

---

### 0.13 Validation System — Two-Gate Pipeline

**Gate 1 (Q1 — automatic):** Injection scan, schema conformance, S7 guard, cross-references.  
**Gate 2 (Q2 — manual):** WebUI review; approve → `validated`.

---

### 0.14 StepDescription Authoring Layer

**StepDescription is the human-editable source of truth** for what a Recipe does. It is
authored in the WebUI and compiled into a `BuildInstruction` at intent-match time by the
Instruction-Building-System (§0.16). It is **not** the BuildInstruction.

#### Mandatory fields per step

| Field | Type | Meaning |
|-------|------|---------|
| `step_number` | int | 1-based position in the sequence |
| `knowledge` | `"orchestrator" \| "rust" \| "both"` | Which runtime reads this step |
| `goal` | string | What this step accomplishes |
| `content` | string | Short description of step content |
| `type` | `"text" \| "component" \| "snippet"` | Determines additional fields |

#### Optional fields per step

| Field | Type | Meaning |
|-------|------|---------|
| `info` | text | Long-form explanation, examples, warnings |
| `include` | UUID[] | Component IDs (Tool / ToolSkill / Skill / PythonCode) needed here |
| `code_snippet` | text | Inline Python code → auto-creates PythonCode component on save; sent to Q1 |

#### Storage

Stored as `step_descriptions JSONB` column on `reborn_recipes` (added in V054).
The YAML-structured text is preserved inside the JSONB for human readability
and WebUI rendering.

#### Multi-StepDescription pattern (variants)

- `Stepdescription0` — base, most common use-case  
- `Stepdescription1` — variant 1 (individual part only; base provides common part via link_formula)  
- `Stepdescription2`, `Stepdescription3`, ...

#### Example — Recipe `local-files-reading`, Stepdescription0 (partial)

```yaml
description: "Stepdescription0 — base path (ls -l, current directory)"

steps:
  - step_number: 1
    knowledge: "orchestrator"
    goal: "Provide task context"
    content: "Information explaining the task"
    type: "text"
    info: |
      Task performed by orchestrator only. No LLM prompt created.
      Rust receives: tool "ls" + toolskill "ls".
      Orchestrator receives: skill "ls" + PythonCode "ls".
      Orchestrator uses PythonCode to instruct the rust executioner via the skill.
      Rust uses the toolskill to call the tool and returns output to the orchestrator,
      who writes it to the chat window.
    include: []

  - step_number: 5
    knowledge: "rust"
    goal: "Provide tool"
    content: "Tool \"ls\""
    type: "component"
    include: ["<uuid:tool-ls>"]

  - step_number: 7
    knowledge: "orchestrator"
    goal: "Provide skill"
    content: "Skill \"ls\""
    type: "component"
    include: ["<uuid:skill-ls>"]

  - step_number: 11
    knowledge: "orchestrator"
    goal: "Provide orchestrator instructions"
    content: "PythonCode \"ls\""
    type: "component"
    include: ["<uuid:pythoncode-ls>"]
    info: |
      Final step. PythonCode tells the orchestrator how to use the ls skill to call
      the rust executioner and what to do with the output (write to chat window).
```

#### WebUI interaction

- Component page → **StepDescriptions section**: all steps shown, editable on click.
- Steps use dropdown fields for enum values (`knowledge`, `type`).
- Intents section: all intent expressions editable; each shows its `link_formula`.
- `code_snippet` field: on save → new PythonCode component created → sent to Q1 validator.
  - While pending: greyed out in WebUI.
  - If Q1 fails: snippet field cleared, PythonCode removed.
  - If Q1 passes → Q2 → on validate: PythonCode added to step; parent Recipe re-queued.

---

### 0.15 Intent Linking Formula

Every intent registered in `reborn_intent_inputs` carries a **link_formula** specifying
which steps (from which StepDescriptions) to include when building the `BuildInstruction`.

#### Notation

```
<desc_id>:<start>-<desc_id>:<end>[+<desc_id>:<start>-<desc_id>:<end>]*
```

- `<desc_id>` = `0` (base), `1` (variant 1), `2` (variant 2), ...
- `<start>` = step number (1-based) or `0` = first step
- `<end>` = step number or `E` = last step
- `+` = concatenate segments in order

#### Examples

| Formula | Meaning |
|---------|---------|
| `0:0-0:E` | All steps of Stepdescription0 |
| `0:0-0:30+1:0-1:E` | Base steps 0–30, then all of Stepdescription1 |
| `0:0-0:31+2:0-2:E` | Base steps 0–31, then all of Stepdescription2 |
| `0:0-0:30+1:0-1:11+3:0-3:E` | Base 0–30, Stepdescription1 steps 0–11, all of Stepdescription3 |

#### Storage

**Migration V052:** `ADD COLUMN link_formula TEXT` to `reborn_intent_inputs`.

```
| intent_expression              | component_id  | variant_key | link_formula            |
|--------------------------------|---------------|-------------|-------------------------|
| "ls -l"                        | <recipe-uuid> | null        | "0:0-0:E"               |
| "show all files including ..." | <recipe-uuid> | "ls-la"     | "0:0-0:30+1:0-1:E"      |
| "list files of the /tmp dir"   | <recipe-uuid> | "ls-dir"    | "0:0-0:31+2:0-2:E"      |
```

---

### 0.16 Instruction-Building-System (IBS)

The IBS **compiles** human-editable StepDescriptions into machine-optimized `BuildInstruction`
structs at intent-match time. It is the central compiler layer of the v3 architecture.

#### Responsibilities

1. Parse `link_formula` → `Vec<(desc_id, step_start, step_end)>`
2. Load `StepDescriptionN` for each referenced `desc_id` from `step_descriptions JSONB`
3. Extract steps in the requested range per segment
4. Apply variable substitution (`{{vars.name}}`)
5. Separate by `knowledge` field → orchestrator steps vs rust steps
6. Build `OrchestratorContext`:
   - `skill_ids[]`: UUIDs from `include` where knowledge = "orchestrator" or "both", type = Skill
   - `python_code_ids[]`: same, type = PythonCode
   - `step_formatter_id`: per-recipe optional formatter UUID
   - `control_flow_steps[]`: from `type: "snippet"` or `type: "component"` with control-flow semantics
7. Build `RustContext`:
   - `tool_skill_ids[]`: UUIDs from `include` where knowledge = "rust" or "both", type = ToolSkill
   - `tool_bindings[]`: `ToolBinding { tool_name, params, error_policy }` per Tool UUID
8. Build `fetch_steps[]` (Section A): all unique component UUIDs across all steps
9. Return `BuildInstruction { llm_call_required, variable_patterns, fetch_steps, basic_prompt_section_refs, orchestrator_context, rust_context }`

#### Interface

```rust
// crates/brassclaw_engine/src/memory/instruction_builder.rs  (new file)

#[async_trait]
pub trait InstructionBuilder: Send + Sync {
    async fn build_from_formula(
        &self,
        recipe_id: Uuid,
        link_formula: &str,
        user_text: &str,   // For variable extraction
    ) -> Result<BuildInstruction, InstructionBuilderError>;
}
```

**Called by:** `PostgresSource::fetch_for_turn()` after intent resolution.

**Caching:** In-process cache keyed by `(recipe_id, variant_key, variable_hash)`.
TTL: 5 minutes. Invalidated when any included component's `updated_at` changes.

---

### 0.17 Turn DataFlow Upgrade (Three-Surface PKC)

#### Current gap

`__assemble_prior_knowledge__` returns one mixed blob containing orchestrator skills,
Rust ToolSkills, and thread memories all mixed together. The orchestrator cannot
distinguish "this is for me" from "this is for Rust".

#### Proposed: Three-Surface PKC

| Surface | PRIORITY | Content | Destination |
|---------|----------|---------|-------------|
| `orchestrator_knowledge` | 2 (instruction snippets) | Skill + PythonCode bodies, step instructions | `formatted_content` → LLM working_messages |
| `memory_knowledge` | 3 (memory snippets) | Thread notes, relevant memories | `formatted_content` → LLM working_messages |
| `rust_knowledge` | — (transient table) | ToolSkill bodies + ToolBinding params | `reborn_pending_rust_context` |

**`__assemble_prior_knowledge__` return shape (upgraded):**

```json
{
  "orchestrator_knowledge": {
    "skill_bodies":       ["<uuid>: <skill content>", "..."],
    "python_code_bodies": ["<uuid>: <pythoncode content>", "..."],
    "step_instructions":  "<formatter-rendered instructions>",
    "llm_call_required":  false
  },
  "memory_knowledge": {
    "thread_notes":      ["..."],
    "relevant_memories": ["..."]
  },
  "rust_pending_id": "<uuid-of-row-in-reborn_pending_rust_context>",
  "override_prompt_creation": true,
  "matched_component_ids": ["uuid1", "uuid2"]
}
```

The orchestrator reads `orchestrator_knowledge` and `memory_knowledge`. It receives
`rust_pending_id` as an opaque reference — it passes it to the Rust layer when
sending the tool invocation. The orchestrator never reads `rust_knowledge`.

#### Rust Context Delivery: Transient Table

**Migration V053:** New table `reborn_pending_rust_context`:

```sql
CREATE TABLE reborn_pending_rust_context (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id         TEXT    NOT NULL,
    iteration      INT     NOT NULL,
    tool_skill_ids UUID[]  NOT NULL,
    tool_bindings  JSONB   NOT NULL,
    created_at     TIMESTAMPTZ DEFAULT now(),
    UNIQUE(run_id, iteration)
);
-- TTL: rows deleted 1 hour after created_at.
```

**Lifecycle:**
1. `RecipeStage` inserts row after `fetch_by_instruction` completes.
2. Rust reads row before first tool dispatch.
3. Row deleted after turn completes (or 1-hour TTL).

---

### 0.18 v2 DocPlan Translation Layer

**Goal:** Expand legacy `MemoryDoc` rows (type `Skill`, `Recipe`, `ToolSkill`) into the
full v3 component graph. The existing `component_import.rs` already handles
`Spec`, `Plan`, `Summary`, `Lesson`, `Issue`, `Note` (classes 12–20).

#### Translation rules

**Skill MemoryDoc → 5 components:**
1. Tool (class 0) — from `param_template.tool_name`
2. ToolSkill (class 13) — param_schema from metadata
3. Skill (class 1) — body from `content`
4. Recipe (class 21) — skeleton, empty `variants[]`, pending
5. ExtensionCatalogue (class 23) — groups all above, pending

**ToolSkill MemoryDoc → 1 component:**
- ToolSkill (class 13) — direct migration

**Recipe MemoryDoc → 1 component + seed StepDescription0:**
- Recipe (class 21) — `trigger` + `steps` preserved as v2 fallback
- StepDescription0 seeded from v2 `RecipeStep[]`

**All translated components start at `pending` → Q1 → Q2 → `validated`.**  
**Original MemoryDocs are marked `archived_at = now()`, never hard-deleted (V055).**

#### CLI command (Phase J)

```bash
brassclaw translate-v2-docs --dry-run    # preview
brassclaw translate-v2-docs --execute    # insert + queue for Q1
```

---

## 1. Implementation Phases

### Phase A — PythonCode Component (class 22)

**Status:** [ ] Pending

**Files to create:**
- `crates/brassclaw_pg/migrations/V047__reborn_python_code.sql`  
  Same column shape as `V036__reborn_specs.sql`. `class_code = 22`.  
  Default consumer tags: `{02:orchestrator, 05:validator}`.

**Files to modify:**
- `crates/brassclaw_engine/src/memory/retrieval_source.rs` — add class 22 to UNION ALL + `fetch_component_by_id`
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `22 => "python_code"`
- `crates/brassclaw_engine/src/types/memory.rs` — add `DocType::PythonCode`
- `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` — add `22 => 0.42`
- `crates/brassclaw_engine/src/memory/component_validator.rs` — class 22 dispatch
- `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs` — new store

**Tests:** Unit: `class_label(22)`, `doc_type_to_class_code`. Integration: retrieve with consumer tag.

---

### Phase B — ExtensionCatalogue Component (class 23)

**Status:** [ ] Pending

**Files to create:**
- `crates/brassclaw_pg/migrations/V048__reborn_extension_catalogues.sql`  
  Columns: scope tuple + `name`, `description`, `version`, `overview_doc` (TEXT),  
  `task_groups JSONB`, `child_component_ids UUID[]`, `intent_index JSONB`,  
  `prior_knowledge_content`, `override_prompt_creation`, `class_code SMALLINT DEFAULT 23`,  
  `prompt_uid`, `consumer_tags`, `intent_examples JSONB`, validation lifecycle, timestamps.

**Files to modify:** Same engine files as Phase A, but for class 23 (weight `0.38`, content = `overview_doc`).

**Tests:** Unit: `class_label(23)`. Integration: catalogue retrieved with `overview_doc` as `effective_content`.

---

### Phase C — Recipe v3: Variants + BuildInstruction + StepDescription storage

**Status:** [ ] Pending

#### C.1 New types in `crates/brassclaw_engine/src/types/recipe.rs`

```rust
// --- Orchestrator section ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepContextSpec {
    #[serde(default)] pub component_ids: Vec<String>,
    #[serde(default)] pub class_codes: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchComponentsStep {
    pub step_id: String,
    pub context_spec: StepContextSpec,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlFlowStep {
    pub step_id: String,
    pub step_type: ControlFlowStepType,
    pub description: String,
    #[serde(default)] pub component_id: String,
    #[serde(default)] pub params: serde_json::Value,
    #[serde(default)] pub condition: Option<String>,
    #[serde(default)] pub branch_targets: Option<BranchTargets>,
    #[serde(default)] pub loop_body: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ControlFlowStepType {
    RunPythonCode,
    ConditionalBranch,
    SetVariable,
    LoopSteps,
    CallAction,
    EmitEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BranchTargets {
    pub on_true: String,
    pub on_false: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OrchestratorContext {
    #[serde(default)] pub skill_ids: Vec<String>,
    #[serde(default)] pub python_code_ids: Vec<String>,
    #[serde(default)] pub step_formatter_id: Option<String>,
    #[serde(default)] pub control_flow_steps: Vec<ControlFlowStep>,
}

// --- Rust section ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    Fail,
    Ignore,
    Retry { max_attempts: u32 },
    Fallback { step_id: String },
}

impl Default for ErrorPolicy { fn default() -> Self { ErrorPolicy::Fail } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolBinding {
    pub tool_name: String,
    pub params: serde_json::Value,
    #[serde(default)] pub error_policy: ErrorPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RustContext {
    #[serde(default)] pub tool_skill_ids: Vec<String>,
    #[serde(default)] pub tool_bindings: Vec<ToolBinding>,
}

// --- BuildInstruction ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariablePattern {
    pub name: String,
    pub pattern: String,
    #[serde(default)] pub default_value: Option<String>,
}

/// Complete turn script compiled by the Instruction-Building-System.
/// Three typed sections: fetch_steps, orchestrator_context, rust_context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BuildInstruction {
    #[serde(default)] pub llm_call_required: bool,
    #[serde(default)] pub variable_patterns: Vec<VariablePattern>,
    #[serde(default)] pub fetch_steps: Vec<FetchComponentsStep>,
    #[serde(default)] pub basic_prompt_section_refs: Vec<String>,
    #[serde(default)] pub orchestrator_context: OrchestratorContext,
    #[serde(default)] pub rust_context: RustContext,
}

/// One intent variant of a Recipe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeVariant {
    pub variant_key: String,
    pub label: String,
    pub intent_examples: Vec<String>,
    pub link_formula: String,    // NEW — drives IBS; BuildInstruction computed at runtime
    pub variable_patterns: Vec<VariablePattern>,  // per-variant runtime var extraction
}
```

**Key change vs. previous plan:** `RecipeVariant` no longer contains a stored `build_instruction`.
Instead it carries a `link_formula` string. The `BuildInstruction` is **computed at runtime**
by the IBS. This means the human-readable StepDescription is always the single source of truth.

Add to `Recipe` struct:
```rust
#[serde(default)] pub variants: Vec<RecipeVariant>,
#[serde(default)] pub step_descriptions: serde_json::Value,  // JSONB — see V054
```

#### C.2 Migrations

- **V049:** `ADD COLUMN variant_key TEXT` on `reborn_intent_inputs`
- **V052:** `ADD COLUMN link_formula TEXT` on `reborn_intent_inputs`
- **V054:** `ADD COLUMN step_descriptions JSONB` on `reborn_recipes`

#### C.3 New file: `instruction_builder.rs`

See §0.16 for interface. Implements `PostgresInstructionBuilder` + `RamInstructionBuilder` (tests).

#### C.4 `fetch_by_instruction` on `RetrievalSource`

See §0.6 for signature. `PostgresSource` implementation iterates `fetch_steps`.

#### C.5 Variant seeding into `reborn_intent_inputs`

On Recipe `auto_passed`: call `seed_intent_input(expression, component_id, variant_key, link_formula)`.  
On Recipe delete/wipe: call `purge_component_inputs(component_id)`.

**Tests:**
- Unit: `BuildInstruction`, `OrchestratorContext`, `RustContext`, `ToolBinding`, `ErrorPolicy` serde roundtrips
- Unit: `FetchComponentsStep` + `StepContextSpec` roundtrips
- Unit: IBS parses `"0:0-0:30+1:0-1:E"` → correct step ranges
- Unit: IBS separates `knowledge: "orchestrator"` vs `"rust"` correctly
- Integration: intent match → correct `variant_key` + `link_formula` returned
- Integration: `fetch_by_instruction` fetches exactly the listed UUIDs, in order
- Integration: `{{vars.dir}}` substitution applied in `effective_content`
- Integration: `RustContext` written to `reborn_pending_rust_context`, readable by Rust layer

---

### Phase D — Skill Intent Wiring + `required_skills`

**Status:** [ ] Pending

**File:** `crates/brassclaw_skills/src/types.rs`  
Add `intent_examples: Vec<String>` (≤512 chars each, capped at 20) and
`required_skills: Vec<String>` (capped at 10, no self-reference) to `SkillManifest`.

**Migration V050:** `ADD COLUMN intent_examples JSONB; ADD COLUMN required_skills JSONB` to `reborn_skills`.

On skill `auto_passed`: call `seed_intent_input` for each intent.  
On skill delete: call `purge_component_inputs`.

**Tests:** Unit: YAML roundtrip, limit enforcement. Integration: resolves via `resolve_intent`.

---

### Phase E — ToolSkill-Mediation Guard (S7) + Skill Cross-References

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/recipe_validator.rs`

S7 guard: `rust_context.tool_bindings[]` non-empty but `orchestrator_context.skill_ids[]` empty → error.  
Add variant step checks to `validate_recipe`.

**Tests:** Unit: S7 violation, empty `component_id`, correct ordering.

---

### Phase F — MCP Translation Layer

**Status:** [ ] Pending

**Files:** `crates/brassclaw_extensions/src/mcp_translation.rs` (new), `lifecycle.rs`.  
MCP tool → Tool + ToolSkill + Skill + Recipe + ExtensionCatalogue, all `pending`.

**Tests:** Unit: MCP payload → component count. Integration: install → validation queue.

---

### Phase G — Q1 Validator Upgrades

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/component_validator.rs`

| Class | Rules |
|-------|-------|
| 22 PythonCode | name, non-empty content, 10k budget, injection scan |
| 23 ExtensionCatalogue | name, non-empty `overview_doc`, >=1 task_group, valid UUIDs in child_component_ids |
| 21 Recipe (variants) | non-empty `variant_key`, >=1 intent_examples, valid `link_formula` syntax, S7 guard |
| 1–3 Skills | intent_examples <=512 chars / <=20; required_skills <=10, no self-ref |
| 16 Actions | steps JSONB against 13 step types |

**Tests:** Unit: missing field → specific error for each class.

---

### Phase H — Pipeline Integration (Tier 0 / Tier 1 Activation)

**Status:** [ ] Pending

1. `crates/brassclaw_agent_loop/src/state.rs` — add `last_user_text: Option<String>`
2. `crates/brassclaw_agent_loop/src/executor/input.rs` — populate from drained input
3. `crates/brassclaw_agent_loop/src/executor/recipe.rs` — full Tier 0/1/2 dispatch:
   - **Tier 0** (`wilson_lower >= 0.70`, `llm_call_required: false`):
     - Call IBS → `fetch_by_instruction` → write `RustContext` to `reborn_pending_rust_context`
     - Serialize `OrchestratorContext` → inject into `__assemble_prior_knowledge__` response
     - Skip `PromptStage` and `ModelStage`
     - Record outcome
   - **Tier 1** (match, below Tier 0): inject ToolSkill summaries via `LoopExecutionState` hint
   - **No match:** fall through to Tier 2 (unchanged)

**Tests:** Unit: `last_user_text` set. Integration: Tier 0 skips LLM; Tier 1 injects hints; Tier 2 unchanged.

---

### Phase I — BasicPromptStore and Assembly

**Status:** [ ] Pending

**Migration V051.** New `PgBasicPromptStore` facade:
`get_for_scope`, `store`, `mark_stale`, `delete`.  
Wire into Interceptor to append stored bundle before LLM shipment.  
On component `validated` transition: call `mark_stale(scope)`.

---

### Phase J — v2 DocPlan Translation

**Status:** [ ] Pending

**New file:** `crates/brassclaw_reborn_composition/src/docplan_translator.rs`  
**CLI:** `brassclaw translate-v2-docs --dry-run | --execute`

**Skill MemoryDoc → Tool + ToolSkill + Skill + Recipe + ExtensionCatalogue (all `pending`).**  
**Recipe MemoryDoc → Recipe with v2 `trigger+steps` + seed StepDescription0.**  
**Original MemoryDocs → `archived_at = now()` (V055, not deleted).**

---

## 2. Migration Sequence

| Migration | Contents | Status |
|-----------|----------|--------|
| `V047__reborn_python_code.sql` | New table, class 22 | **Next** |
| `V048__reborn_extension_catalogues.sql` | New table, class 23 | |
| `V049__reborn_intent_inputs_variant_key.sql` | `ADD COLUMN variant_key TEXT` | |
| `V050__reborn_skills_intent_examples.sql` | `ADD COLUMN intent_examples JSONB; required_skills JSONB` to `reborn_skills` | |
| `V051__reborn_basic_prompt_store.sql` | New table, one row per scope, `bundle_json JSONB`, `is_stale BOOL` | |
| `V052__reborn_intent_inputs_link_formula.sql` | `ADD COLUMN link_formula TEXT` to `reborn_intent_inputs` | |
| `V053__reborn_pending_rust_context.sql` | Transient per-turn Rust prior-knowledge table | |
| `V054__reborn_recipes_step_descriptions.sql` | `ADD COLUMN step_descriptions JSONB` to `reborn_recipes` | |
| `V055__brassclaw_memory_docs_archived_at.sql` | `ADD COLUMN archived_at TIMESTAMPTZ` to `brassclaw_memory_docs` | |

All additive. No DROP, no renames. No existing rows break.

---

## 3. Open Questions

1. **Variable extraction:** Named capture groups vs. post-match LLM extraction?  
   → **Recommendation:** Named capture groups for Phase C; LLM fallback later.

2. **Shared step prefix:** Full independent step list per variant vs. shared prefix + divergence?  
   → **Recommendation:** Full independent list per variant (link_formula handles sharing).

3. **`required_skills` inclusion:** Always include vs. score against current query?  
   → **Recommendation:** Always include; cap at 10.

4. **`step_formatter_id` scope:** Per-recipe, per-variant, or per-step?  
   → **Recommendation:** Per-recipe. Formatting style is consistent across a capability domain.

5. **StepDescription storage format:** YAML files in git vs. JSONB in `reborn_recipes`?  
   → **Recommendation:** JSONB column (simpler, no file management). YAML-formatted text preserved inside.

6. **Rust delivery mechanism:** Transient table vs. ephemeral column vs. in-memory cache?  
   → **Recommendation:** Transient table `reborn_pending_rust_context` (V053).

7. **PKC split:** New `__retrieve_memories__` host function vs. three-surface PKC in same response?  
   → **Recommendation:** Three-surface PKC (§0.17); no new host function.

8. **v2 MemoryDoc preservation:** Delete after translation or archive?  
   → **Recommendation:** Archive (V055 `archived_at`).

---

## 4. Out of Scope (Marked Postponed)

- Full self-improvement pipeline (Interceptor-driven Recipe auto-creation)
- Component self-creation wizard
- Automatic Sempai-driven prompt rewrites
- `FormatOrchestratorPrompt` as a distinct step type (handled via `step_formatter_id` during IBS compilation)

---

## 5. Turn Flow Summary

```
User types: "show all files including hidden in the current directory"
|
+- [InputStage]
|   Sets last_user_text = "show all files including hidden in the current directory"
|
+- [RecipeStage]
|   resolve_intent → Match { recipe_id, variant_key="ls-la", link_formula="0:0-0:30+1:0-1:E" }
|   Tier 0 check: wilson_lower = 0.82, llm_call_required = false → eligible
|
+- [IBS] build_from_formula(recipe_id, "0:0-0:30+1:0-1:E", user_text)
|   Loads Stepdescription0 steps 0..30, Stepdescription1 all steps
|   Separates by knowledge:
|     orchestrator steps → skill-ls, pythoncode-ls
|     rust steps → tool-ls, toolskill-ls
|   Returns BuildInstruction
|
+- [RetrievalEngine] fetch_by_instruction(build_instruction)
|   Reads fetch_steps: fetches skill-ls + pythoncode-ls + toolskill-ls bodies
|   Returns prior_knowledge patch (orchestrator-facing content only)
|
+- [RecipeStage] writes RustContext → reborn_pending_rust_context
|   { tool_skill_ids: [toolskill-ls], tool_bindings: [{ tool:"ls", params:{flags:"-la"} }] }
|
+- [Tier 0 — no LLM call]
|   __assemble_prior_knowledge__ returns three-surface PKC:
|     orchestrator_knowledge: { skill_bodies:[skill-ls], python_code_bodies:[pythoncode-ls] }
|     memory_knowledge: { thread_notes:[] }
|     rust_pending_id: "<uuid>"
|
|   Orchestrator reads orchestrator_knowledge.
|   Orchestrator runs pythoncode-ls → invokes skill-ls → tells Rust to execute
|   Rust reads reborn_pending_rust_context by rust_pending_id
|   Rust reads toolskill-ls params → executes ls -la → returns output
|   Orchestrator writes output to chat window
|
+- [InterceptorStage]  Saves composition plan. Sempai reviews if connected.
+- [AssistantReplyStage]  Emits directory listing. Wilson score updated.
```

**No-match path (Tier 2):**

```
User types: "explain recursion to me"
+- [InputStage]        last_user_text set
+- [RecipeStage]       resolve_intent → NoMatch → falls through
+- [PromptStage]       fetch_for_consumer() → UNION ALL scan, full PKC assembled
+- [InterceptorStage]  Sempai reviews if connected
+- [ModelStage]        Full LLM call
+- [AssistantReplyStage]  Emits LLM response. No Recipe outcome recorded.
```
