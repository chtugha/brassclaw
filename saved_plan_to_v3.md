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
  1. RecipeStage (agent_loop) calls fetch_for_turn(scope, user_text, budget, "02"):
       a. resolve_intent(user_text) → Match { recipe_id, class_code:21, step_link }
       b. Fetch step_descriptions JSONB + variable_patterns + wilson_lower + tier
       c. IBS: build_instruction(step_link, step_descriptions, variable_patterns)
              → BuildInstruction { rust_steps[], orchestrator_steps[] }
       d. Apply {{vars.name}} substitution
       e. fetch_component_by_id for each UUID in rust_steps → rust_items
       f. fetch_component_by_id for each UUID in orchestrator_steps → orchestrator_items
       g. Return FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }

  ── Tier 0 (routing.tier0_eligible = true, llm_call_required = false): ────
  2. RecipeStage applies rust_items to Rust execution context (silently).
     PromptStage and ModelStage are SKIPPED.
     Python scripting engine is called with pre-loaded orchestrator_items.

  ── Tier 1 (routing.tier0_eligible = false OR llm_call_required = true): ──
  2. RecipeStage stores rust_items → state.recipe_rust_context.
     RecipeStage stores orchestrator_items → state.recipe_hint.
  3. Executor applies rust_items before Python script starts.
  4. Python step 0 calls __assemble_prior_knowledge__: handler returns the
     pre-stashed orchestrator_items as orchestrator_content (no second fetch).
  5. LLM is called; guided by the orchestrator_content recipe hint.
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

> **Key design principle — three parties, one artefact:**
>
> | Party | Role |
> |-------|------|
> | **Human author** (WebUI) | Writes `StepDescriptions` — YAML-structured, readable. Never touches a `BuildInstruction`. |
> | **IBS** (Instruction-Building-System) | Sole producer. Compiles `StepDescriptions` → `BuildInstruction` at intent-match time. Never stores the result — it is ephemeral per-call (memoised in-process; see §0.7). |
> | **Rust executor** (`RecipeStage`) | Reads `rust_steps[]` only. Applies ToolSkill UUIDs and ToolBindings to the Rust execution context. Never touches orchestrator content. |
> | **Orchestrator** (`handle_assemble_prior_knowledge`) | Reads `orchestrator_steps[]` only. Serialises component bodies into `orchestrator_content`. Never touches rust channel content. |
>
> The two runtime readers (**Rust executor** and **Orchestrator**) each see exactly one channel.
> Neither reader sees the other channel. The IBS is the sole bridge.
>
> **BuildInstructions are never stored** — not in the DB, not in session state.
> The IBS compiles them on demand from the `step_descriptions` JSONB column plus the
> resolved `step_link` formula. In-process memoisation (§0.7) eliminates the per-call cost
> for repeated identical intents without requiring persistence.

#### Why two channels, not three

Earlier drafts described a three-section design (RetrievalEngine / Orchestrator / Rust).
The v3 design simplifies: `fetch_steps` is eliminated as a separate section. The IBS
directly emits `rust_steps[]` and `orchestrator_steps[]`, each containing `IbsRecipeStep`
entries with UUIDs. `PostgresSource::fetch_for_turn` calls `fetch_component_by_id` for
each UUID immediately after IBS compilation.

> **No `fetch_by_instruction` method.**
> There is no `RetrievalSource` method named `fetch_by_instruction` or similar.
> The IBS runs synchronously *inside* `fetch_for_turn`, not as a separate retrieval pass.
> `fetch_for_turn` calls `build_instruction(...)`, then immediately calls
> `fetch_component_by_id` for every UUID the IBS emitted.
> The result is `FetchForTurnResult::SplitResult` with two pre-fetched item lists.
> Any design that adds a `fetch_by_instruction` method to `RetrievalSource` is wrong.

#### Two readers, two typed channels

**Channel R — Rust (`rust_steps[]`)**  
Steps with `knowledge: "rust"` or `"both"`.  
Contains: ToolSkill UUIDs + ToolBinding params + ErrorPolicy.  
Applied silently to the Rust execution context by `RecipeStage`. Never forwarded to the orchestrator.

**Channel O — Orchestrator (`orchestrator_steps[]`)**  
Steps with `knowledge: "orchestrator"` or `"both"`.  
Contains: Skill UUIDs and PythonCode UUIDs. PythonCode component bodies ARE the
orchestrator instructions — authored with the correct content and formatting.
`type: "text"` steps are authoring annotations only (WebUI documentation); they have
no runtime emission.
Serialized into `orchestrator_content` by the v3 `handle_assemble_prior_knowledge` in `orchestrator.rs`.

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
                     info: Option<String> }       ← WebUI annotation only; NOT emitted to orchestrator
```

**Invariant:** Channels must not overlap.  
A ToolSkill UUID must never appear in `orchestrator_steps`.  
A Skill UUID must never appear in `rust_steps`.  
An orchestrator step never references a ToolSkill. A rust step never references a Skill.  
Runtime content for the orchestrator lives in component bodies loaded by `type: "component"` steps — not in `type: "text"` step `info` fields. Step type and component class are orthogonal.

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
    # The PythonCode component body is the formatted instruction — no separate formatter needed.
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
(added in V047). Each element of the JSONB array holds **two representations** of the
same StepDescription, kept in sync on every WebUI save:

```json
[
  {
    "desc_idx": 0,
    "label": "base path (ls -l, current directory)",
    "yaml_source": "steps:\n  - stepnumber: 1\n    knowledge: orchestrator\n ...",
    "steps": [
      {
        "stepnumber": 1,
        "knowledge": "orchestrator",
        "goal": "Provide task context",
        "content": "Information explaining the task",
        "type": "text",
        "info": "Task performed by orchestrator only...",
        "include": [],
        "dependencies": ""
      }
    ]
  }
]
```

- **`yaml_source`** — the raw YAML string as typed by the author. Preserved verbatim.
  Used by the WebUI renderer (syntax-highlighted YAML editor). Never read by the IBS.
- **`steps`** — the pre-parsed structured array. Used exclusively by the IBS.
  Written by the WebUI on save: parse `yaml_source` → produce `steps` array.

The IBS never parses YAML at runtime — it reads the pre-parsed `steps` array directly.
YAML parsing happens exactly once, at WebUI save time, before Q1 runs. If `yaml_source`
fails to parse (malformed YAML), the save is rejected before Q1 with a parse error shown
inline in the WebUI. The `steps` array is therefore always consistent with `yaml_source`.

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
| `info` | text | Human-readable documentation about what this step does. Visible in the WebUI component page to help the author understand the step's purpose. **Not emitted to the orchestrator at runtime.** Orchestrator instructions are delivered by `type: "component"` steps — the body of the referenced component (Skill, PythonCode, or any other orchestrator-channel class) is what the orchestrator receives. |
| `include` | UUID[] | Component UUIDs needed at this step. IBS emits a fetch for each UUID. |
| `codesnippet` | text | Inline Python code. On WebUI save: creates a PythonCode component (class 22), enters Q1 queue. Step greyed out until Q1+Q2 pass; promoted to `type: "component"` with the new UUID on Q2 pass. |
| `dependencies` | string | Traversal expression into this step's component's `dependency_registry` (see §0.19). E.g. `"1[all], 5[2,6], 17[3, 7[1,4]]"`. Resolved at fetch time by `fetch_for_turn`. Absent or empty string = no dependencies. |

#### Step types

| Type | IBS behaviour |
|------|--------------|
| `text` | Authoring annotation only. No component fetch. No runtime emission — `type: "text"` steps produce nothing in `orchestrator_content`. They exist solely for WebUI readability: documenting what a step does, why it is here, what the author should know. |
| `component` | Emits a fetch for each UUID in `include`. Routes item to rust or orchestrator channel based on `knowledge`. |
| `snippet` | WebUI-only authoring shortcut. **IBS refuses to assemble** a BuildInstruction while any step has this type — it returns `IbsError::UnpromotedSnippet`. The step must be promoted to `type: "component"` after the created PythonCode passes Q1+Q2. |

> **`type: "text"` steps and the IBS:** The IBS produces no output for `type: "text"` steps.
> They are pure WebUI annotations. Runtime content reaches the orchestrator exclusively via
> `type: "component"` steps: the body of the referenced component — whether a Skill (class 1–3),
> PythonCode (class 22), or any other orchestrator-channel class — is what the orchestrator
> receives. Step type and component class are orthogonal: the step type determines IBS
> handling; the component class and the step's `knowledge` field determine channel routing.
> A `type: "text"` step with no `info` is a Q1 **warning** (undocumented step), not an error.

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
    dependencies: "0[all]"        # load all of pythoncode-ls's declared dependencies
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

#### StepContextSpec — typed context for each step's IBS output

When the IBS compiles a `BuildInstruction`, each orchestrator-channel step has a **context type**
(`StepContextSpec`) that determines how `handle_assemble_prior_knowledge` formats its body
into `orchestrator_content`. The context type is inferred from the component's `class_code`
after `fetch_component_by_id` returns — authors do not set it manually.

**Class-code → StepContextSpec mapping** (computed at fetch time, not stored):

| `class_code` | Class | `StepContextSpec` | Formatter heading |
|---|---|---|---|
| 1–3 | Skill | `Skill` | `## [Skill: {name}]` |
| 12 | Spec | `Spec` | `## [Spec: {name}]` |
| 13 | ToolSkill | *(never in orchestrator channel)* | — |
| 21 | Recipe | `Recipe` | `## [Recipe: {name}]` |
| 22 | PythonCode | `PythonCode` | `## [PythonCode: {name}]` |
| 23 | ExtensionCatalogue | `Catalogue` | `## [Catalogue: {name}]` |
| *(type: "text" step)* | *(no component fetch)* | `Annotation` | *(nothing emitted)* |

```rust
/// Describes the kind of content emitted to the orchestrator for one step.
/// Inferred from the component's class_code in handle_assemble_prior_knowledge.
/// `Annotation` is assigned when the step type is "text" (no component involved).
pub enum StepContextSpec {
    Skill,           // class 1–3
    Spec,            // class 12
    Recipe,          // class 21 (nested recipe reference)
    PythonCode,      // class 22
    Catalogue,       // class 23
    Annotation,      // type: "text" step — never emitted; WebUI-only
}
```

The formatter in `handle_assemble_prior_knowledge` iterates `orchestrator_items`, derives
`StepContextSpec` from each item's `class_code`, and emits a labelled block:

```
## [Skill: ls]
<skill body>

## [PythonCode: ls-result-handler]
<pythoncode body>
```

This makes `orchestrator_content` self-describing. Authors do not need to add type headers
to their PythonCode bodies or Skill bodies — the formatter generates them from class_code.

**Invariant — ToolSkill is never in `orchestrator_items`.**
`fetch_component_by_id` for a ToolSkill UUID (class 13) called from an orchestrator-channel
step is a Q1 hard error. ToolSkills are Rust-channel-only. If a class 13 UUID appears in
`orchestrator_steps[].include`, the IBS or Q1 must catch it before it reaches the formatter.

`StepContextSpec` is a **derived type** — computed once per component fetch, never stored.
It is not part of the `StepEntry` JSONB. It exists only in the formatter code path inside
`handle_assemble_prior_knowledge` when iterating over `orchestrator_items`.

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
     type == "text"      → no runtime emission; step is WebUI annotation only; skip
     type == "component" → emit component fetch step; route by knowledge; emit UUIDs from include
     type == "snippet"   → return Err(IbsError::UnpromotedSnippet)
4. Validate:
     step numbers must be monotonically increasing within each StepDescription
     rust-channel steps must have type:component with non-empty include
     all include UUIDs must parse as valid UUID v4
     S7 guard: if any rust_steps emit tool_bindings, orchestrator_steps must contain ≥1 skill_id
     dependency expressions: parse each step's `dependencies` string into DependencyExpr tree
       → parse errors are hard IBS errors (IbsError::InvalidDependencyExpr)
       → out-of-range indices are not checked here (registry is in DB; checked at Q1)
5. Partition:
     rust_steps[]         ← steps where knowledge ∈ {"rust", "both"}
     orchestrator_steps[] ← steps where knowledge ∈ {"orchestrator", "both"}
6. Attach parsed DependencyExpr to each RecipeStep that declared a dependencies field.
   The IBS does NOT resolve UUIDs — it only parses the expression into a typed tree.
   Resolution (recursive DB fetching) happens in fetch_for_turn (§0.19).
7. Return BuildInstruction { rust_steps, orchestrator_steps,
                              variable_patterns, basic_prompt_refs,
                              llm_call_required }
```

#### LLM-formatted orchestrator content

After assembly, `handle_assemble_prior_knowledge` in `orchestrator.rs` renders
orchestrator_steps into a human+LLM-readable block (`orchestrator_content` in the
`__assemble_prior_knowledge__` result):

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

#### Memoisation

- **Key:** `sha256(step_link + "|" + sorted_include_uuids.join(","))`
- **Eviction triggers (all must be monitored):**
  1. Any `include`d component's `updated_at` changes (via `last_graduation_at` scope cursor — §0.18)
  2. The Recipe's own `updated_at` changes (StepDescription edited in WebUI)
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
    InvalidDependencyExpr { step_id: String, reason: String },
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
    /// Wilson lower-bound from the matched Recipe row (for metrics / logging).
    pub wilson_lower:             f64,
    /// Pre-computed from recipe.is_tier0_eligible(): tier ∈ {mature, candidate}
    /// AND wilson_lower ≥ 0.70 AND validated AND validation hook wired.
    /// RecipeStage checks this flag directly — does not need the full Recipe struct.
    pub tier0_eligible:           bool,
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

> **Important — which function is upgraded:**  
> `__retrieve_docs__` is the **legacy** function. It calls the old `RetrievalEngine::retrieve_context`
> (MemoryDoc path), returns a flat list `[{type, title, content}]`, and knows nothing about
> `class_code`, intent resolution, or the component class system. It is the dead shim in the
> current step-0.  
> `__assemble_prior_knowledge__` is **already** the intent-capable path. It calls
> `PostgresSource::fetch_for_turn`, handles `FetchForTurnResult::Components` and
> `Disambiguation`, and returns `{content, formatted_content, override_prompt_creation,
> matched_component_ids}`. This is the function that v3 upgrades — not `__retrieve_docs__`.  
> After the v3 upgrade, `__assemble_prior_knowledge__` handles everything in one call.
> The dead `__retrieve_docs__` shim at step-0 is removed (Phase G). The `__retrieve_docs__`
> host function registration is kept for the one release cycle that custom orchestrators may
> still call it (Phase K cleanup), then removed.

The three-call block collapses to one call. The upgraded `__assemble_prior_knowledge__`
handles everything — intent resolution, IBS compilation, channel split, Action routing.

```python
# v3 default.py step 0 — single call:
if step == 0:
    token_budget = config.get("prior_knowledge_token_budget", 100000) if isinstance(config, dict) else 100000
    pkr = __assemble_prior_knowledge__(goal, token_budget, "02")

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

    # REMOVED in v3:
    # - docs = __retrieve_docs__(goal, 5)       ← dead Action-detection shim (Phase G)
    # - all_skills = __list_skills__()          ← IBS already selected Skills by UUID
    # - active_skills = select_skills(...)      ← no longer needed
```

#### What `__assemble_prior_knowledge__` returns in v3

The existing return shape is **extended** (not replaced) to carry the new v3 routing signals.
Existing `{content, formatted_content, override_prompt_creation, matched_component_ids}`
fields are preserved for backward compatibility with custom orchestrators.

```python
{
    # EXISTING fields (preserved):
    "content":                  str,   # Raw PKC — Rust dispatch / KV-cache fingerprint only
    "formatted_content":        str,   # DEPRECATED alias — same as orchestrator_content in v3
    "override_prompt_creation": bool,

    # EXTENDED in v3 — orchestrator channel content:
    # Skill bodies, PythonCode bodies, and any other orchestrator-channel component bodies.
    # type:text step info fields are NOT included — they are WebUI annotations only.
    # ToolSkill bodies NEVER appear here (Rust channel is delivered silently by RecipeStage).
    "orchestrator_content": str,

    # v3 routing signals (new):
    "action_short_circuit":  bool,
    "action_component_id":   str,   # UUID (when action_short_circuit is true)
    "action_name":           str,
    "disambiguation":        bool,
    "candidates":            list,

    # Active-skill tracking (extended):
    "matched_component_ids": list,  # orchestrator-channel UUIDs (Skills + PythonCode)
                                    # passed to _set_active_skills_from_matched_ids;
                                    # no __list_skills__() + select_skills() round-trip
}
```

The Rust channel (ToolSkills, ToolBindings) is applied to the Rust execution context
**inside `handle_assemble_prior_knowledge`, silently**. It never crosses to the
orchestrator's `working_messages`. `formatted_content` is an alias for `orchestrator_content`
in v3 — both are set to the same value. Custom orchestrators that already check
`pkr["formatted_content"]` continue to work unchanged.

#### `call_action` nested lookup migration

`call_action` in `default.py` (line 844) currently calls `__retrieve_docs__(nested_name, 1)` to
look up an Action by name. This is a search-by-name — fragile and hits the legacy path.

**v3 replacement:** a new host function `__fetch_component__(uuid, class_code)` calls
`fetch_component_by_id` directly with the UUID from the BuildInstruction step.

```python
# Old (line 844):
action_docs = __retrieve_docs__(nested_name, 1)
# New:
action_item = __fetch_component__(action_uuid, 16)
```

`__retrieve_docs__` is the **dead legacy function** — it returns a flat `[{type, title, content}]`
list with no class_code awareness. Custom orchestrators that call it still work (list vs dict
return shapes diverge naturally), but it should not appear in v3 default.py at all.
`__retrieve_docs__` registration is kept for one release cycle, then removed in Phase K.

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
`__assemble_prior_knowledge__` call (step 0 in `default.py`), which calls
`PostgresSource::fetch_for_turn` and handles the full split and channel delivery internally.
The legacy `__retrieve_docs__` (MemoryDoc path) is NOT called in v3 step-0.

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
BuildInstruction, no IBS compilation, no prior-knowledge assembly. The `__assemble_prior_knowledge__`
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
Implemented in `component_validator.rs`. On pass: queue row transitions to state 2 (via
`gate1_pass` — callable only from the validator). On fail: queue row stays at state 1
with updated `validation_errors`.

**Gate 2 (Q2 — manual):** WebUI review. On approve: queue row deleted (graduation event),
component `validation_status` set to `'validated'`. On reject: queue row transitions to
state 3, `counter` incremented, `review_feedback` populated.

**The queue and the status are separate state machines (§0.18):**
- While a component is in `reborn_validation_queue` (states 1–4) it is pre-validation.
  Its `validation_status` on the component table is `'pending'` or similar.
- Once the queue row is deleted (Q2 approval), the component is post-validation.
  Its `validation_status = 'validated'` is the sole retrieval gate.

**Q2 approval drives three side effects:**
1. Queue row deleted → `last_graduation_at` bumped on scope cursor (via DB trigger).
2. Component `validation_status` set to `'validated'`.
3. SplitResult memo-cache for this scope evicted on next cache hit (via `last_graduation_at` check — §0.18).

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

### 0.17 Variable Intent Templates

#### The problem

`resolve_intent` matches `input_text = $query` — exact string equality. A Recipe variant
whose execution depends on a runtime value (a path, a filename, a search pattern) cannot
be expressed as a single intent row. Without a variable mechanism the author must
pre-register every possible value as a separate row, which is impossible.

#### The `%` slot marker

Intent expressions authored on Recipe variants (and Skills with `intent_examples`) may
contain `%` as a **positional slot marker**. `%` means "any sequence of tokens may appear
here". The author controls where variability is allowed; the rest of the expression is
literal and anchors the match.

```
# Literal expression (no slot) — stored and matched exactly as today:
"list files of the current directory"

# Template expressions (contain %):
"show me all files in the % directory"
"show me all files in the directory %"
"read the file at %"
"search for % in %"
"edit % and change %"
```

`%` is purely an authoring and matching marker. After a template matches, the values
captured in each `%` slot are extracted from the user text and passed to the
`variable_patterns` extraction step (or auto-extracted from template segments when
`variable_patterns` is absent — see §0.17.3).

`variable_patterns` and `%` are **separate concerns**:
- `%` drives **matching** — does this user text structurally fit this template?
- `variable_patterns` drives **extraction** — what is the value of each slot?

`variable_patterns` becomes optional for simple single-slot cases where auto-extraction
from template segments is unambiguous (see §0.17.3).

#### Terminology

| Term | Meaning |
|------|---------|
| **literal expression** | Intent text with no `%` — stored and matched exactly (existing path) |
| **template expression** | Intent text containing one or more `%` slots |
| **template_prefix** | The literal text before the first `%` in a template |
| **template_suffix** | The literal text after the last `%` in a template |
| **anchor** | A non-empty `template_prefix` or non-empty `template_suffix` |

---

### 0.17.1 Matching — Three-Path Dispatch

Template matching uses PostgreSQL's `LIKE` operator with the stored template as the
**pattern** and the user text as the **value**:

```sql
'show me all files in the /tmp directory'
  LIKE
'show me all files in the % directory'
-- → TRUE  (PostgreSQL native, no Rust pre-processing needed)
```

This is the reverse of the usual `LIKE` use — the pattern is stored in the DB, the
concrete value is the incoming query. PostgreSQL supports this natively.

Because plain sequential scanning of all template rows is too slow at scale, matching
is pre-filtered using computed anchor columns and targeted indexes. Three index paths
cover all valid templates:

**Path 0 — Exact match (existing, unchanged):**
```
input_text = $user_text
Uses the existing B-tree index on (scope, input_text, input_class).
```

**Path 1 — Prefix-anchored template (`template_prefix != ''`):**
```
template_prefix = "show me all files in the "
User text must start with this prefix.
Pre-filter: $user_text LIKE (template_prefix || '%')
Full check:  $user_text LIKE input_text
Uses B-tree index on (scope, template_prefix).
```

**Path 2 — Suffix-anchored template (`template_prefix = ''`, `template_suffix != ''`):**
```
Leading-% case: "% directory", "% in /tmp"
template_suffix = " directory"
User text must end with this suffix.
Pre-filter (reverse trick): reverse($user_text) LIKE (reverse(template_suffix) || '%')
Full check:  $user_text LIKE input_text
Uses functional B-tree index on (scope, reverse(template_suffix)).
```

**Path 3 — Dual-anchored template (`template_prefix != ''` AND `template_suffix != ''`):**
```
"search for % in the % directory"
prefix = "search for ", suffix = " directory"
Uses the prefix index as primary pre-filter (more selective),
suffix check eliminates remaining false candidates before full LIKE.
Fastest path — two anchors eliminate nearly all non-matching rows.
```

**Blocked — no anchor (`template_prefix = ''` AND `template_suffix = ''`):**
```
"% in %", "% %", "%"
Q1 hard error. Never reaches the DB.
```

The combined SQL for `resolve_intent` evaluates all four paths in a single query:

```sql
WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
  AND input_class = ANY($7)
  AND (
    -- Path 0: exact match (existing path, unchanged)
    input_text = $5

    -- Path 1: prefix-anchored template
    OR (
        is_template = true
        AND template_prefix != ''
        AND $5 LIKE (template_prefix || '%')
        AND $5 LIKE input_text
    )

    -- Path 2: suffix-anchored template (leading-% case)
    OR (
        is_template = true
        AND template_prefix = ''
        AND template_suffix != ''
        AND reverse($5) LIKE (reverse(template_suffix) || '%')
        AND $5 LIKE input_text
    )
  )
ORDER BY
  CASE WHEN input_text = $5 THEN 0 ELSE 1 END,   -- exact match always beats template
  CASE input_class WHEN $8 THEN 0 WHEN $9 THEN 1 WHEN $10 THEN 2 ELSE 3 END,
  score DESC
LIMIT 30
```

Dual-anchored templates (Path 3) are caught by Path 1 (prefix pre-filter fires, then
full `LIKE` validates the suffix naturally). No separate path 3 branch is needed in SQL.

---

### 0.17.2 New Columns and Indexes on `reborn_intent_inputs`

**Migration V054** adds three columns and two indexes:

```sql
-- V054__reborn_intent_inputs_template.sql

ALTER TABLE reborn_intent_inputs
  ADD COLUMN is_template      BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN template_prefix  TEXT,   -- literal text before first %;  NULL for literals
  ADD COLUMN template_suffix  TEXT;   -- literal text after last %;    NULL for literals

-- Path 1: prefix-anchored templates
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_prefix_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, template_prefix)
    WHERE is_template = true AND template_prefix != '';

-- Path 2: suffix-anchored templates (reverse trick for leading-% case)
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_suffix_rev_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, reverse(template_suffix))
    WHERE is_template = true AND template_prefix = '' AND template_suffix != '';
```

**Existing rows** (literal expressions) are unaffected: `is_template = false`,
`template_prefix = NULL`, `template_suffix = NULL`. The existing exact-match index
and all existing query paths are unchanged.

**Seeding a template row:**

```rust
fn seed_template_intent(expression: &str) -> (String, String, String) {
    // expression = "show me all files in the % directory"
    let prefix = expression.split('%').next().unwrap_or("").to_string();
    let suffix = expression.split('%').last().unwrap_or("").to_string();
    let suffix = if expression.contains('%') && suffix != prefix { suffix } else { String::new() };
    // → prefix = "show me all files in the "
    // → suffix = " directory"
    // input_text stored as the template string with % intact
    (expression.to_string(), prefix, suffix)
}
```

`input_text` stores the template string as-is (with `%`). The UNIQUE constraint
`(scope, input_text, input_class, component_id)` therefore naturally deduplicates
templates: two identical template expressions for the same component are one row.

---

### 0.17.3 Post-Match Value Extraction

Once a template row matches, the variable values must be extracted from the user text.

**Auto-extraction from template segments (no `variable_patterns` needed):**

Split the template on `%` → literal segments. Find each segment's position in the user
text in order. The substring between consecutive segments is the captured slot value.

```
template:  "show me all files in the % directory"
segments:  ["show me all files in the ", " directory"]
user_text: "show me all files in the /tmp directory"

Match segment[0] at position 0..25 → OK
Match segment[1] from right → " directory" ends at position 41
Slot[0] value = user_text[25..31] = "/tmp"
```

For multiple slots:
```
template:  "search for % in %"
segments:  ["search for ", " in ", ""]
user_text: "search for TODO in /src"

Slot[0] = "TODO"   (between segment[0] and segment[1])
Slot[1] = "/src"   (after segment[1] to end of string)
```

Auto-extraction is sufficient for most builtin Recipe variants. `variable_patterns`
is used when:
1. The extracted value needs **validation** (e.g. must start with `/`)
2. The extracted value needs **transformation** (e.g. strip quotes)
3. There are **overlapping templates** where the auto-extraction is ambiguous
4. The slot name matters for `{{vars.name}}` substitution (auto-extract assigns
   positional names `vars.slot0`, `vars.slot1`; `variable_patterns` assigns semantic names)

When `variable_patterns` is present, it runs after auto-extraction as a refinement:
the auto-extracted value is validated against the pattern. If it fails, the match is
demoted (not rejected — the template still matched; extraction just gets the raw value).

**Positional names:** Auto-extracted slots are named `slot0`, `slot1`, `slot2`, ...
in left-to-right order. `{{vars.slot0}}` in ToolBinding `params` references the first
slot. Authors who want semantic names (`{{vars.dir}}`, `{{vars.pattern}}`) add a
`variable_patterns` entry that maps the positional regex capture to the named variable.

---

### 0.17.4 Q1 Validation Rules for Templates

| Rule | Condition | Severity |
|------|-----------|----------|
| **No-anchor error** | `template_prefix = ''` AND `template_suffix = ''` (e.g. `"% in %"`, `"%"`) | **Hard error** — template too permissive; add literal text around each `%` |
| **Leading-`%` warning** | `template_prefix = ''` AND `template_suffix != ''` (e.g. `"% directory"`) | **Warning** — valid and indexed, but imprecise; consider adding a word before `%` |
| **Adjacent slots** | Two `%` with no literal text between them (e.g. `"search % %"`) | **Hard error** — adjacent slots are unextractable; separate them with literal text |
| **Dangling `variable_patterns`** | A `variable_patterns` entry whose `name` does not appear as `{{vars.name}}` in any ToolBinding `params` | **Warning** — pattern defined but never used |
| **Missing template** | `{{vars.slot0}}` used in ToolBinding `params` but expression has no `%`, and no `variable_patterns` | **Hard error** — variable referenced but no source defined |

---

### 0.17.5 Authoring in WebUI

In the intent expression field, `%` is rendered as a styled token (highlighted chip, not
plain text) so authors can see at a glance which parts of the expression are slots.

The field shows live feedback:
- **Green anchor indicator:** "Prefix anchor: `show me all files in the `" — anchored, fast.
- **Yellow anchor indicator:** "Suffix anchor only: ` directory`" — valid, leading-`%` warning shown.
- **Red indicator:** "No anchor — add literal text around `%`" — hard error, cannot save.

---

### 0.18 Validation Queue — Pre-Validation Lifecycle

#### Two separate state machines

The validation system has two distinct phases, each with its own authoritative state:

```
Component created / edited
        │
        ▼
┌─────────────────────────────────────┐
│     reborn_validation_queue         │   PRE-VALIDATION
│                                     │   All components not yet manually approved
│  state 1 — Q1 queue                 │   live here. Erased on manual approval.
│  state 2 — Q1 passed (Gate 1 only) │
│  state 3 — rejected (back to fix)  │
│  state 4 — deletion candidate      │
│  counter  — rejection count         │
└─────────────────────────────────────┘
        │
        │  Manual approval (Q2) → row DELETED from queue
        ▼
┌─────────────────────────────────────┐
│  validation_status on component     │   POST-VALIDATION
│  table (existing, unchanged)        │   'validated' = active, trusted, in retrieval
│                                     │   'upgrade_queued' = re-entering queue
│  'validated' / 'upgrade_queued'     │   This system is untouched by this design.
└─────────────────────────────────────┘
```

The two systems do not overlap. A component row is either in the queue (not yet
manually approved) OR it has a `validation_status` that reflects its post-approval
runtime identity. It cannot be in both states simultaneously.

**Every component that is not yet manually validated must have a row in `reborn_validation_queue`.**  
A component with no queue row and `validation_status != 'validated'` is an inconsistent state
— detected and reported by an integrity check that runs at boot.

---

#### The queue states

| State | Value | Meaning | Who can write it |
|-------|-------|---------|-----------------|
| Q1 queue | 1 | Submitted, awaiting Gate 1 (automatic) validation | Application layer |
| Q1 passed | 2 | Gate 1 passed; awaiting Q2 manual review | **Gate 1 only** — never the application layer |
| Rejected | 3 | Q2 reviewer rejected; author must revise and resubmit | Q2 reviewer action |
| Deletion candidate | 4 | Too many rejections or manually condemned; awaiting cleanup | System (counter threshold) or Q2 reviewer |

**State 2 is the security invariant.** No API endpoint, no application-layer code path,
no direct SQL can set `state = 2`. Only the internal Gate 1 validator function transitions
a row to state 2 after a clean Q1 result. This is enforced at the application layer
(the only write path for state 2 is inside the validator) and documented as an
inviolable rule — any code that sets `state = 2` outside the validator is a security bug.

#### The rejection counter

`counter` starts at 0 on row insert. It increments by 1 each time a component is rejected
(state 2 → state 3, or state 3 → state 1 after author resubmits and is rejected again).
It never resets. It is a permanent rejection history for this component version.

When `counter` reaches a configurable threshold (default: 3), the queue system
automatically promotes the row to state 4 (deletion candidate) without requiring a
Q2 reviewer action. This prevents perpetually-stuck components from clogging the queue.

#### Table shape

```sql
CREATE TABLE reborn_validation_queue (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope — all reads and writes filter on the full tuple.
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    agent_id        TEXT        NOT NULL,
    project_id      TEXT        NOT NULL,

    -- The component this row tracks.
    component_id    UUID        NOT NULL,
    component_class SMALLINT    NOT NULL,   -- class_code; for WebUI filtering

    -- Lifecycle state. 1=Q1_queue 2=Q1_passed 3=rejected 4=deletion_candidate.
    -- State 2 may only be written by the Gate 1 validator.
    state           SMALLINT    NOT NULL DEFAULT 1
        CHECK (state IN (1, 2, 3, 4)),

    -- Permanent rejection count. Never resets. Increments on each rejection.
    counter         INT         NOT NULL DEFAULT 0,

    -- Human-readable feedback from Q2 reviewer (populated on rejection).
    review_feedback TEXT,

    -- Q1 error messages (populated on Q1 fail, cleared on Q1 pass).
    validation_errors TEXT[]    NOT NULL DEFAULT '{}',

    -- Timestamps
    submitted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One queue row per component at any time.
    UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)
);

CREATE INDEX reborn_validation_queue_scope_state_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, state);

CREATE INDEX reborn_validation_queue_scope_class_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, component_class);

-- Partial index: state 4 (deletion candidates) for cleanup job.
CREATE INDEX reborn_validation_queue_deletion_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id)
    WHERE state = 4;
```

#### What moves to the queue table from component tables

The following columns currently exist on every component table and describe
**pre-validation** lifecycle — they belong in the queue and are removed
from component tables (see Phase N):

| Removed column | Moved to queue as | Notes |
|----------------|-------------------|-------|
| `queue_code TEXT` | `state SMALLINT` | Queue state replaces the text queue_code |
| `review_attempts INT` | `counter INT` | Same concept, renamed and centralised |
| `review_feedback TEXT` | `review_feedback TEXT` | Moved to queue |
| `rejected_at TIMESTAMPTZ` | `updated_at TIMESTAMPTZ` on queue row | Queue row's `updated_at` serves this purpose |
| `validation_errors TEXT[]` | `validation_errors TEXT[]` | Moved to queue; cleared on Q1 pass |

**Columns that stay on component tables** (post-validation runtime identity):
- `validation_status TEXT` — `'validated'` is the retrieval gate. All retrieval queries
  (`WHERE validation_status = 'validated'`) continue to work unchanged.
- All content columns, reward columns, lineage columns — unchanged.

The net result: every component table loses 4–5 columns. The validation lifecycle
is managed entirely by the queue while the component is pre-validated, and entirely
by `validation_status` once it graduates.

#### Cache invalidation via queue graduation

When a component is manually approved (Q2 pass), its queue row is deleted. This
deletion event is the authoritative cache invalidation signal.

A companion column `last_graduation_at TIMESTAMPTZ` is added to the scope-level
settings table (or a new lightweight `reborn_scope_cursors` table — one row per scope):

```sql
-- Added to existing reborn_monty_vm_settings or a new reborn_scope_cursors table:
last_graduation_at TIMESTAMPTZ;
-- Updated by a trigger on reborn_validation_queue DELETE.
```

The SplitResult cache (§0.7 memoisation) checks `last_graduation_at` on every hit.
If it is newer than the cache entry's `cached_at`: discard all cache entries for this
scope. One sub-millisecond PK read. No TTL required as primary mechanism — eviction
is exact and event-driven.

This resolves Open Question 2 completely: the cache eviction mechanism is queue
graduation events, not polling `updated_at` or TTL expiry.

---

### 0.19 Dependency Registry

#### Every component owns a flat dependency registry

Every component table gains a `dependency_registry JSONB` column. It is a flat,
zero-indexed array of entries — each entry names one component this component depends on:

```json
[
  { "idx": 0, "component_id": "<uuid:pipe-skill>",     "class_code": 1,  "label": "pipe-skill" },
  { "idx": 1, "component_id": "<uuid:json-helper>",    "class_code": 22, "label": "json-helper" },
  { "idx": 2, "component_id": "<uuid:toolskill-read>", "class_code": 13, "label": "toolskill-read" }
]
```

- `idx` is the positional index used in traversal expressions on StepDescription steps.
- `label` is human-readable, shown in the WebUI registry editor.
- `class_code` drives channel routing (orchestrator vs rust) during traversal.
- The registry is flat — sub-dependencies are declared on the referenced components
  themselves, not nested here.

The `dependency_registry` is authored in the WebUI component page, editable as a table.
It is part of the component's validated content — changes require re-entry into Q1.

---

#### Traversal expressions on StepDescription steps

A step's `dependencies` field is a **traversal expression** that walks the registry tree
selectively. The expression is a comma-separated list of traversal nodes.

**Traversal node syntax:**

```
<idx>               — load component at index <idx>; no sub-dependencies
<idx>[all]          — load component at index <idx>; recursively load ALL of its
                      dependency_registry entries and ALL of their sub-dependencies
                      (full transitive closure from this node)
<idx>[<n>,<m>,...]  — load component at index <idx>; from its registry load only
                      indices <n>, <m>, ... (no further recursion unless nested)
<idx>[<n>, <m>[all], <p>[<q>,<r>]]
                    — mixed: index <n> (no sub-deps), <m> full transitive, <p> with
                      selective sub-indices <q> and <r>
```

**Example:**

```yaml
- stepnumber: 3
  knowledge: orchestrator
  type: component
  include: ["<uuid:skill-file-editing>"]
  dependencies: "1[all], 5[2,6], 17[3, 7[1, 4]]"
```

Resolution of `1[all], 5[2,6], 17[3, 7[1,4]]` against `skill-file-editing`'s registry:

```
1[all]
  → load registry[1] of skill-file-editing → <uuid:pipe-skill>
  → load ALL of pipe-skill's dependency_registry, recursively
      → pipe-skill.registry[0] → <uuid:tokenizer>  (no further deps)
      → pipe-skill.registry[1] → <uuid:buffer>
          → buffer.registry[0] → <uuid:allocator>  (leaf)
      → ... (full transitive closure)

5[2,6]
  → load registry[5] of skill-file-editing → <uuid:formatter>
  → load formatter.registry[2] → <uuid:indent-helper>  (no sub-deps)
  → load formatter.registry[6] → <uuid:escape-helper>  (no sub-deps)

17[3, 7[1,4]]
  → load registry[17] of skill-file-editing → <uuid:validator>
  → load validator.registry[3] → <uuid:schema-checker>  (no sub-deps)
  → load validator.registry[7] → <uuid:type-coercer>
      → load type-coercer.registry[1] → <uuid:int-parser>   (no sub-deps)
      → load type-coercer.registry[4] → <uuid:float-parser> (no sub-deps)
```

---

#### Resolution algorithm (at fetch time, not pre-embedded)

The traversal expression is stored as a string in the StepDescription JSONB. The IBS
parses it into a typed `DependencyExpr` tree at compile time (pure Rust, no DB). The
**actual component fetching** happens in `fetch_for_turn` after IBS compilation, as
`fetch_component_by_id` calls per resolved UUID.

```
resolve_dependencies(
    root_component_id: Uuid,
    expr: &DependencyExpr,
    pool: &PgPool,
    visited: &mut HashSet<Uuid>,    // deduplication + cycle guard
) -> Vec<ComponentItem>

For each node in expr:
  1. Look up root_component.dependency_registry[node.idx]
       → (dep_uuid, dep_class_code)
  2. If dep_uuid ∈ visited → skip (already collected or cycle)
  3. visited.insert(dep_uuid)
  4. fetch_component_by_id(dep_uuid) → ComponentItem
  5. Route by dep_class_code → orchestrator or rust channel
  6. If node has sub-expression:
       If sub-expression == All:
           fetch dep_component.dependency_registry (one DB read)
           for each entry: resolve_dependencies(dep_uuid, All, pool, visited)
       Else:
           resolve_dependencies(dep_uuid, sub_expr, pool, visited)
  7. Collect result
```

**Deduplication:** `visited` is shared across the entire `fetch_for_turn` call for a
given turn. A component UUID collected by any step's dependency traversal is not
fetched again — regardless of which step triggered it.

**Eager-loading rule:** On the first occurrence of a Skill UUID in the assembled step
list, all dependencies declared on that step are resolved immediately. On subsequent
steps that reference the same Skill UUID: the Skill itself is already in `visited` →
skipped. Its dependencies were already loaded at the first occurrence.

**Cycle protection:** The `visited` set prevents infinite recursion. If component A
depends on B and B depends on A (or any longer cycle), the second encounter of either
UUID is skipped silently. Q1 detects and rejects cycles statically (see below).

---

#### KV-cache interaction

In steady state, the basic-prompt prefix already contains the bodies of all commonly-used
Skills, ToolSkills, and PythonCode helpers. The dependency traversal is a graph walk
over components that are **already in the LLM's context**. For each resolved dependency
UUID, the IBS checks whether that UUID appears in `basic_prompt_section_refs` — if so,
it emits a section reference instead of re-injecting the body. The token cost of
dependency resolution in steady state is therefore near zero: the graph walk happens in
Rust (fast), the components are already in the KV-cache prefix (no token cost).

New or recently-validated components not yet in the prefix do incur a full body
injection into the per-turn patch. This is the transient case — it resolves after the
next basic-prompt rebuild.

---

#### Q1 validation rules for dependency_registry and traversal expressions

| Rule | Condition | Severity |
|------|-----------|----------|
| **Self-reference** | Registry entry points to the same component's own UUID | Hard error |
| **Invalid UUID** | Registry entry UUID does not resolve to any known component in this scope | Hard error |
| **Out-of-range index** | Traversal expression references an index that does not exist in the component's registry | Hard error |
| **Cycle detection** | Traversal expression (with `[all]`) would follow a dependency cycle | Hard error (static DFS from the traversal root) |
| **Adjacent `[all]` depth** | `[all]` on a component whose own registry also contains `[all]` entries — warn author of potential large transitive closure | Warning |
| **Unparseable expression** | `dependencies` string fails the traversal expression parser | Hard error (parse error message included) |

Cycle detection at Q1 is a **static DFS** starting from the component being validated,
following all `[all]` and explicit sub-expressions, using the current state of all
referenced components' registries. Any back-edge in the DFS is a cycle → hard error on
the component that closes the cycle.

---

#### Relationship to `required_skills` (Phase J)

**`required_skills` on the `reborn_skills` table does not exist.** It was a previous
design that placed dependency declarations on the Skill component itself. Under the
dependency registry model, dependencies are declared:
1. On the component's own `dependency_registry` (what it depends on)
2. On the StepDescription step via the traversal expression (how deep to follow)

Phase J is replaced by the dependency registry implementation (see revised Phase J below).

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
  `IbsRecipeStep` (renamed from `RecipeStep` to avoid collision with the existing
  v2 `RecipeStep { skill, tool, params, description }` in `types/recipe.rs`),
  `VariablePattern`, `BuildInstruction`, `IbsError`,
  `DependencyExpr`, `DependencyNode` (the parsed traversal tree — see §0.19),
  `StepContextSpec` (derived content type for orchestrator formatting — see §0.5).

  **`StepDescriptionEntry` shape** (maps to one element of the `step_descriptions` JSONB array):
  ```rust
  pub struct StepDescriptionEntry {
      pub desc_idx:    usize,
      pub label:       String,
      pub yaml_source: String,              // preserved verbatim; never read by IBS
      pub steps:       Vec<StepEntry>,      // pre-parsed; IBS reads this only
  }

  pub struct StepEntry {
      pub stepnumber:   u32,
      pub knowledge:    StepOwner,          // Orchestrator | Rust | Both
      pub goal:         String,
      pub content:      String,
      pub step_type:    RecipeStepType,     // Text | Component | Snippet
      pub info:         Option<String>,     // WebUI annotation only; not emitted at runtime
      pub include:      Vec<uuid::Uuid>,    // component UUIDs
      pub dependencies: Option<String>,     // traversal expression string (§0.19)
  }
  ```  
  New functions: `parse_step_link(&str) -> Result<Vec<StepRange>, IbsError>`,
  `parse_dependency_expr(&str) -> Result<DependencyExpr, IbsError>`, and
  `build_instruction(step_link, step_descriptions, variable_patterns) -> Result<BuildInstruction, IbsError>`.

  **`DependencyExpr` / `DependencyNode` types:**
  ```rust
  pub enum DependencySubExpr {
      All,                          // [all] — full transitive closure
      Selective(Vec<DependencyNode>), // [n, m[...], ...] — selective indices
  }

  pub struct DependencyNode {
      pub idx: usize,
      pub sub: Option<DependencySubExpr>, // None = load component only, no sub-deps
  }

  pub type DependencyExpr = Vec<DependencyNode>;
  ```

#### Files to modify

- `crates/brassclaw_engine/src/types/recipe.rs` — add `BuildInstruction` two-channel shape;
  full `RecipeStepType` enum; `StepOwner`; `ToolBinding`; `ErrorPolicy`.  
  Add to `Recipe` struct (all `#[serde(default)]` so existing rows deserialise unchanged):
  ```rust
  #[serde(default)] pub variants: Vec<RecipeVariant>,
  #[serde(default)] pub step_descriptions: serde_json::Value,
  #[serde(default)] pub dependency_registry: serde_json::Value,  // per-component, see §0.19
  ```
  Note: `dependency_registry` is also added to `ToolSkill`, `Skill`, `PythonCode`,
  and all other component types that participate in dependency traversal. Each component
  owns its own flat indexed registry.

  > **Naming note:** The existing `types/recipe.rs` already defines `RecipeStep { skill, tool,
  > params, description }` — a name-based v2 type. The IBS module introduces a **different**
  > `RecipeStep` type in `instruction_builder.rs` that uses UUIDs and channels. To avoid a
  > naming collision: the IBS type is named `IbsRecipeStep` in `instruction_builder.rs`; the
  > existing v2 `RecipeStep` in `types/recipe.rs` is NOT renamed (backward compatibility).

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

- Unit: JSONB round-trip: `StepDescriptionEntry` with `yaml_source` + `steps` serialises and deserialises correctly
- Unit: `yaml_source` field is preserved verbatim (not re-serialised from `steps`)
- Unit: `parse_step_link("0:0-0:E")` → single range, all steps
- Unit: `parse_step_link("0:0-0:30+1:0-1:E")` → two ranges, correct desc_idx and bounds
- Unit: `build_instruction` with `knowledge: rust` step → step only in `rust_steps`
- Unit: `build_instruction` with `knowledge: both` step → step in both channels
- Unit: `build_instruction` with `snippet`-type step → `IbsError::UnpromotedSnippet`
- Unit: step numbers non-monotonic within a StepDescription → `IbsError::StepOrderViolation`
- Unit: S7 guard: rust tool_bindings present, no orchestrator skill_ids → `IbsError::S7Violation`
- Unit: `parse_dependency_expr("1[all], 5[2,6], 17[3, 7[1,4]]")` → correct `DependencyExpr` tree
- Unit: `parse_dependency_expr("0")` → single node, no sub-expr
- Unit: `parse_dependency_expr("1[all]")` → node with `DependencySubExpr::All`
- Unit: `parse_dependency_expr("")` → empty vec (no dependencies)
- Unit: malformed expression `"1[all"` → `IbsError::InvalidDependencyExpr`
- Unit: `BuildInstruction`, `ToolBinding`, `ErrorPolicy`, `DependencyNode` serde roundtrips

---

### Phase B — PythonCode Component (class 22)

**Status:** [ ] Pending

**Goal:** New component class for Python code/instruction elements targeted at the orchestrator.

#### Files to create

- `crates/brassclaw_pg/migrations/V048__reborn_python_code.sql`  
  Same column shape as `V036__reborn_specs.sql`. `class_code = 22`.  
  Default consumer tags: `{02:orchestrator, 05:validator}`.  
  **Do NOT include** `queue_code`, `review_attempts`, `review_feedback`, `rejected_at`,
  or `validation_errors` columns — this table is created after Phase N is planned and
  uses `reborn_validation_queue` from day one (see §0.18, Phase N.4).

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
  `validation_status TEXT` (post-validation gate only), `updated_at`, `created_at`,
  `class_code SMALLINT DEFAULT 23`.  
  **Do NOT include** `queue_code`, `review_attempts`, `review_feedback`, `rejected_at`,
  or `validation_errors` columns — uses `reborn_validation_queue` from day one (§0.18, Phase N.4).

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
  - **Extend `fetch_component_by_id` match arm for new classes 22 and 23** (added in Phases B and C):
    the current `match component_class_code` in `retrieval_source.rs` has no arm for 22 or 23 —
    those class codes currently return `None` (empty vec). Phase E adds:
    ```rust
    22 => Some(("reborn_python_code",    "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    23 => Some(("reborn_extension_catalogues", "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)")),
    ```
    This is required before any Recipe step can reference a class 22 or 23 component UUID.
    > **Security note:** `fetch_component_by_id` uses `format!()` to interpolate the table
    > name and content expression into the SQL query string. This is safe **only because**
    > both values come from a `match` on `component_class_code` and are hard-coded `&'static str`
    > literals — never from user input. This pattern must NEVER be extended to accept
    > user-supplied table names or column expressions. The class code itself is an `i32`
    > from the DB, not from user input, so the dispatch is safe. Document this constraint
    > in a code comment above the match arm when implementing.

#### Tests

- Unit: Recipe match with `step_link` → `SplitResult`; `rust_items` contain only ToolSkills; `orchestrator_items` contain only Skills and PythonCode
- Unit: `knowledge: both` step → UUID appears in both `rust_items` and `orchestrator_items`
- Unit: Action (class 16) match → `ActionShortCircuit { component_id, name }`
- Unit: match with `step_link: None` → existing `Components([single_item])` path unchanged
- Unit: `{{vars.dir}}` substitution applied in `orchestrator_items[].effective_content`
- Unit: `routing.wilson_lower` populated from Recipe row's `wilson_lower` field
- Integration: full intent match → correct channel split confirmed by asserting item class_codes

---

### Phase F — `handle_assemble_prior_knowledge` Upgrade (Rust handler)

**Status:** [ ] Pending

**Goal:** Upgrade the Rust handler behind `__assemble_prior_knowledge__` to handle all
four `FetchForTurnResult` variants — including the new `SplitResult` and
`ActionShortCircuit` variants added in Phase E. Register `__fetch_component__`.
Fix the hardcoded `tenant_id: "default"` scope bug (see §below).

> **Clarification — which handler is upgraded:**  
> `handle_retrieve_docs` calls `RetrievalEngine::retrieve_context` (legacy MemoryDoc path).
> It is **not** upgraded — it is left registered for backward compatibility and removed in
> Phase K.  
> `handle_assemble_prior_knowledge` already calls `fetch_for_turn` via `PostgresSource`.
> This is the handler that v3 extends to handle `SplitResult` and `ActionShortCircuit`.

#### Files to modify

- `crates/brassclaw_engine/src/executor/orchestrator.rs`

  **`handle_assemble_prior_knowledge`:**  
  The existing handler already calls `retrieval_source.fetch_for_turn()` and handles
  `Components` and `Disambiguation`. Extend it to handle the two new variants:
  - `SplitResult`: format `orchestrator_items` into `orchestrator_content` (channel O);
    set `formatted_content = orchestrator_content` (backward compat alias);
    populate `action_short_circuit: false`, `disambiguation: false`;
    include `rust_items` serialized in the return dict under `"rust_items"` (for the
    caller — see note below);
    return extended routing dict (§0.9 shape).
    > **Important — rust_items delivery:** `handle_assemble_prior_knowledge` runs inside
    > the Python scripting engine and has NO access to the Rust execution context (the
    > tool-dispatch layer managed at `RecipeStage` level). The handler CANNOT "apply"
    > rust_items directly. Instead, `RecipeStage` (Phase H) calls `fetch_for_turn` BEFORE
    > the Python script starts. For Tier 1 (where Python does run), `RecipeStage` stores the
    > rust_items in the loop state and applies them to the execution context during that
    > pre-Python pass. When Python later calls `__assemble_prior_knowledge__`, the handler
    > returns the stashed orchestrator_content. The `"rust_items"` field in the dict is
    > informational only — the Python side never calls Rust tools directly. Do NOT add
    > rust_items application logic inside `handle_assemble_prior_knowledge`.
  - `ActionShortCircuit`: return `{ action_short_circuit: true, action_component_id, action_name,
    orchestrator_content: "", formatted_content: "", override_prompt_creation: false,
    matched_component_ids: [] }`.
  - `Components` (no-match UNION ALL): all items → `orchestrator_content` **and**
    `formatted_content` (both set) — existing behaviour, unchanged shape.
  - `Disambiguation`: existing behaviour. Return `{ disambiguation: true, candidates }`.

  Return value is always a dict. The Python side already guards `isinstance(pkr, dict)`
  from the existing `__assemble_prior_knowledge__` usage.

  **`handle_retrieve_docs` — no change.** Left as-is. It is the legacy path, kept for
  one release cycle for custom orchestrators. Phase K removes it.

  **Register `__fetch_component__(uuid: str, class_code: int)`:**  
  New host function. Handler calls `fetch_component_by_id(uuid, class_code)` directly.
  Returns a single item dict or `None`. Used by `call_action` nested lookups (§0.9).

#### Phase F security fix — hardcoded `tenant_id: "default"` in scope

> **Bug found (orchestrator.rs line 2581):** `handle_assemble_prior_knowledge` currently
> constructs `ComponentScope` as:
> ```rust
> ComponentScope {
>     tenant_id: "default".to_string(),   // ← HARDCODED — wrong for multi-tenant
>     user_id: thread.user_id.clone(),
>     agent_id: String::new(),            // ← EMPTY — wrong for agent scoping
>     project_id: thread.project_id.to_string(),
> }
> ```
> This means all intent lookups ignore the real tenant_id and agent_id. In a
> multi-tenant deployment, User A could match intents seeded by User B's tenant.
> Phase F MUST fix this: the scope must be constructed from the actual thread's
> tenant, agent, and project identities. The `Thread` struct must carry `tenant_id`
> and `agent_id` (verify if they already exist; if not, they must be added).
> This is a **correctness and isolation bug** — fix it as part of Phase F, not deferred.

#### Tests

- Unit: `SplitResult` → `orchestrator_content` contains Skill bodies and PythonCode bodies; does NOT contain ToolSkill bodies; does NOT contain `type:text` step info text
- Unit: `SplitResult` → `formatted_content` equals `orchestrator_content` (alias preserved)
- Unit: `ActionShortCircuit` → `action_short_circuit: true`, `orchestrator_content: ""`
- Unit: `Components` (no-match) → `orchestrator_content` contains all items (baseline preserved)
- Unit: `Disambiguation` → `disambiguation: true` with candidates list
- Unit: `handle_retrieve_docs` remains untouched — still returns flat `[{type, title, content}]` list
- Unit: `ComponentScope` in `handle_assemble_prior_knowledge` uses correct tenant_id and agent_id from thread (not hardcoded "default")
- Integration: `__fetch_component__(uuid, 16)` → correct Action item returned
- Integration: two-tenant setup → tenant A's intents do NOT match for tenant B's thread

---

### Phase G — Python Step-0 Upgrade + `call_action` Migration

**Status:** [ ] Pending

**Goal:** Remove the dead step-0 shim calls from `default.py` so it makes a single
`__assemble_prior_knowledge__` call (which is already the primary call at line 997).
Migrate `call_action` nested lookup to `__fetch_component__`.

> **What the current code does (lines 994–1032):**  
> 1. `pkr = __assemble_prior_knowledge__(goal, token_budget, "02")` — PRIMARY call (works)  
> 2. `docs = __retrieve_docs__(goal, 5)` — dead Action-detection shim (broken: `class_code`
>    never in metadata, bug known, documented in §0.9 Problem 1)  
> 3. `all_skills = __list_skills__()` + `select_skills(...)` — unnecessary round-trip
>    (IBS already selected Skills by UUID; §0.9 Problem 2)  
>
> Phase G removes items 2 and 3. The primary `__assemble_prior_knowledge__` call (item 1)
> stays. After Phase F upgrades the handler, `pkr` already carries `action_short_circuit`,
> `disambiguation`, and `orchestrator_content` — no shim needed.

#### Files to modify

- `crates/brassclaw_engine/orchestrator/default.py`
  - Remove the `docs = __retrieve_docs__(goal, 5)` block (lines ~1018–1028): dead shim, never fires.
  - Remove `all_skills = __list_skills__()` and `select_skills()` calls (lines ~1031–1050).
  - Extend the `pkr` dict handling after `__assemble_prior_knowledge__` to check the new
    v3 fields: `action_short_circuit`, `disambiguation`, `orchestrator_content` (as described in §0.9).
  - Add `_set_active_skills_from_matched_ids(pkr.get("matched_component_ids", []), state)` helper.
  - Replace `call_action` `__retrieve_docs__(nested_name, 1)` at line ~844 with
    `__fetch_component__(action_uuid, 16)` (UUID sourced from the BuildInstruction step).
  - `pkr["formatted_content"]` remains supported (backward compat alias) — code that checks
    it continues to work. New code uses `pkr["orchestrator_content"]`.

#### Tests

- Unit: step-0 with upgraded pkr → `orchestrator_content` injected; `__list_skills__` NOT called; `__retrieve_docs__` shim NOT called
- Unit: pkr has `action_short_circuit: true` → `execute_action_by_id` called, no LLM
- Unit: pkr has `disambiguation: true` → `handle_disambiguation` called
- Unit: no-match path → UNION ALL `orchestrator_content` injected (baseline preserved)
- Integration: `call_action` using `__fetch_component__` → correct Action fetched by UUID

---

### Phase H — RecipeStage: `last_user_text` + Tier 0/1 Dispatch

**Status:** [ ] Pending

**Goal:** Activate the RecipeStage stub so it dispatches correctly for Tier 0, Tier 1,
and falls through to Tier 2 on no match.

#### Files to modify

1. `crates/brassclaw_agent_loop/src/state.rs` — add to `LoopExecutionState`:
   ```rust
   /// Last user-visible input text; populated by InputStage on each drain.
   /// Required by RecipeStage for fetch_for_turn query. See recipe.rs module doc.
   #[serde(default)] pub last_user_text: Option<String>,
   /// Stashed rust_items from a Tier 1 SplitResult. Applied to the Rust execution
   /// context before the Python scripting engine starts each turn. Cleared after use.
   #[serde(default)] pub recipe_rust_context: Vec<serde_json::Value>,
   /// Stashed orchestrator_items hint from a Tier 1 SplitResult. PromptStage
   /// injects this before the UNION ALL scan. Cleared after use.
   #[serde(default)] pub recipe_hint: Option<serde_json::Value>,
   ```
   All three fields are `#[serde(default)]` so existing checkpoint payloads deserialise correctly.
   > **Crate boundary constraint:** `brassclaw_agent_loop` depends on `brassclaw_turns` but NOT
   > on `brassclaw_engine`. `ComponentItem` is defined in `brassclaw_engine`. Therefore
   > `recipe_rust_context` and `recipe_hint` CANNOT be typed as `Vec<ComponentItem>` or
   > `Vec<ComponentItem>` — doing so would create a forbidden crate dependency.
   > They are typed as `serde_json::Value` (pre-serialized at `RecipeStage` before being
   > stored in state). The executor deserializes them back to component data when
   > applying them to the execution context, using types from `brassclaw_turns` only.

2. `crates/brassclaw_agent_loop/src/executor/input.rs` — populate `last_user_text` from
   drained input (the last user message text seen this turn)

3. `crates/brassclaw_agent_loop/src/executor/recipe.rs` — replace stub with full dispatch:

   > **Enum note:** The current `RecipeStep` enum has only `Continue { state }`. Phase H
   > adds `TierZero` and `ActionExecuted` variants. The internal enum is `RecipeStep`
   > (the type alias `RecipeStageOutcome` used in comments below maps to it).
   >
   > **RecipeLookup vs fetch_for_turn:** `ctx.host.recipe_lookup()` is backed by
   > `PgRecipeLibrary` (in `pg_recipe_store.rs`) — a real Postgres implementation that
   > queries `reborn_recipes` using trigger-based scoring (exact/keyword/pattern match).
   > It is NOT a dead v2 path; it is wired and used in production via `runtime.rs`.
   > However, it uses the old `trigger` JSONB scoring — not the intent-system (`resolve_intent`).
   > Phase H uses `PostgresSource::fetch_for_turn` (intent-driven) instead.
   > The `recipe_lookup()` port **must be kept wired and functional** — it provides the
   > outcome recording path (`record_recipe_outcome`) that updates wilson_lower/tier.
   > Phase H adds a parallel intent-driven lookup; both paths coexist during the v3 transition.
   > `record_recipe_outcome` must be called from the v3 path too (same Wilson update needed).
   >
   > **rust_items application:** `RecipeStage` runs at the agent loop level (above the
   > Python scripting engine) so it CAN apply rust_items to the Rust execution context
   > directly. For Tier 1, rust_items are stashed in `state.recipe_rust_context` and
   > applied by the executor before the Python script is invoked.

   ```
   RecipeStage::process(state):
     user_text = state.last_user_text (return Continue if None)
     result = retrieval_source.fetch_for_turn(scope, user_text, budget, "02")

     match result:
       SplitResult { rust_items, orchestrator_items, routing }:
         if routing.tier0_eligible && !routing.llm_call_required:
           // Tier 0: no LLM — tier0_eligible = tier∈{mature,candidate} + wilson≥0.70
           //                                    + validated + validation hook wired
           apply rust_items to Rust execution context (RecipeStage has direct access)
           stash orchestrator_items in state for PromptStage bypass
           return RecipeStep::TierZero { routing }       // NEW variant

         else:
           // Tier 1: inject hint, let LLM decide.
           stash rust_items in state.recipe_rust_context  // applied before Python starts
           stash orchestrator_items in state.recipe_hint
           return RecipeStep::Continue { state }

       ActionShortCircuit { component_id, name }:
         execute Action or stash for Python step-0 to handle
         return RecipeStep::ActionExecuted { component_id, name }  // NEW variant

       Components(_) | Disambiguation(_) | (no match):
         return RecipeStep::Continue { state }  // Tier 2 — unchanged
   ```

4. `crates/brassclaw_agent_loop/src/executor/canonical.rs` — **executor loop restructuring
   required**. The current dispatch at line 94 is:
   ```rust
   state = match self.recipe.process(ctx, RecipeInput { state }).await? {
       RecipeStep::Continue { state: next } => *next,
   };
   ```
   This is an exhaustive match. Adding `TierZero` and `ActionExecuted` variants causes a
   **compile error** until canonical.rs handles them. Restructure using an intermediate enum:

   ```rust
   /// Produced by the RecipeStage dispatch inside canonical.rs.
   /// Determines which pipeline stages run after RecipeStage.
   enum PostRecipeOutcome {
       /// Normal path — PromptStage, InterceptorStage, ModelStage all run.
       NeedsPrompt(Box<LoopExecutionState>),
       /// Tier 0: rust_items applied, orchestrator_items stashed.
       /// PromptStage and ModelStage are SKIPPED.
       /// Python scripting engine runs directly with stashed orchestrator context.
       TierZero {
           state:        Box<LoopExecutionState>,
           routing:      TurnRoutingSignals,
       },
       /// Action short-circuit: no LLM, no prompt.
       /// Python step-0 receives pkr["action_short_circuit"] = true.
       ActionExecuted {
           state:        Box<LoopExecutionState>,
           component_id: Uuid,
           name:         String,
       },
   }
   ```

   The canonical loop becomes:
   ```
   let outcome = match recipe_step {
       RecipeStep::Continue { state }        => PostRecipeOutcome::NeedsPrompt(state),
       RecipeStep::TierZero { state, routing } => PostRecipeOutcome::TierZero { state, routing },
       RecipeStep::ActionExecuted { state, component_id, name }
                                             => PostRecipeOutcome::ActionExecuted { ... },
   };

   match outcome {
       PostRecipeOutcome::NeedsPrompt(state) => {
           // run PromptStage → InterceptorStage → ModelStage (unchanged)
       }
       PostRecipeOutcome::TierZero { state, routing } => {
           // skip PromptStage and ModelStage
           // CapabilityStage handles tool execution using pre-loaded rust context
           // AssistantReplyStage emits the result
       }
       PostRecipeOutcome::ActionExecuted { state, .. } => {
           // skip PromptStage and ModelStage
           // Python script already handled the action in step-0
           // AssistantReplyStage emits the result
       }
   }
   ```

5. **Stash / unstash protocol — how RecipeStage and `handle_assemble_prior_knowledge` coordinate (Tier 1):**

   > This is the trickiest coordination point in the whole architecture. Both `RecipeStage`
   > (in the agent loop) and `handle_assemble_prior_knowledge` (inside the Python scripting
   > engine) call `fetch_for_turn`. They must NOT both do a full IBS compilation + component fetch.

   **The protocol:**

   - **Tier 1 path in `RecipeStage`:**
     1. Calls `fetch_for_turn` → `SplitResult { rust_items, orchestrator_items, routing }`.
     2. Stores `orchestrator_items` serialized as `state.recipe_hint` (JSONB).
     3. Stores `rust_items` serialized as `state.recipe_rust_context` (JSONB).
     4. Returns `RecipeStep::Continue { state }` — does NOT skip PromptStage.

   - **Tier 1 path in `handle_assemble_prior_knowledge`** (called by Python step-0):
     1. Checks `state.recipe_hint`: **if set**, skip `fetch_for_turn` entirely.
     2. Deserialize `state.recipe_hint` back to `Vec<ComponentItem>` as `orchestrator_items`.
     3. Clear `state.recipe_hint` (consumed — one-shot).
     4. Format `orchestrator_items` → `orchestrator_content` using StepContextSpec formatter.
     5. Return the extended pkr dict.

   **In other words:** For Tier 1, `RecipeStage` is the actual fetcher. The Python handler
   just reads the stash. There is no double-fetch, no second `resolve_intent`, no second
   IBS compilation. The handler's `fetch_for_turn` call is bypassed whenever a stash is present.

   - **Tier 0 path:** `RecipeStage` returns `TierZero`. `PromptStage` and `ModelStage` are
     skipped. The Python script still runs (the scripting engine is not the LLM call), but
     it receives `pkr` from `__assemble_prior_knowledge__` which returns the stashed content
     directly (same stash/unstash as Tier 1, just the PromptStage/ModelStage stages are absent).

   **Rust state type constraint (repeated for clarity):** `state.recipe_hint` and
   `state.recipe_rust_context` are typed as `serde_json::Value` — NOT `Vec<ComponentItem>`.
   `ComponentItem` is in `brassclaw_engine`; `LoopExecutionState` is in `brassclaw_agent_loop`
   which does NOT depend on `brassclaw_engine`. Serialization to `serde_json::Value` happens
   at `RecipeStage` before storing in state. Deserialization from `Value` happens in
   `handle_assemble_prior_knowledge` (which is in `brassclaw_engine` and CAN use `ComponentItem`).

6. `PromptStage`: if `state.recipe_hint` is set (Tier 1), inject it into prior_knowledge
   before calling `fetch_for_consumer`. If `PostRecipeOutcome::TierZero`, `PromptStage`
   and `ModelStage` are skipped entirely via the `PostRecipeOutcome` dispatch above.

#### Tests

- Unit: `last_user_text` populated by `InputStage` after draining input
- Integration: Tier 0 match (wilson ≥ 0.70, `llm_call_required: false`) → `PromptStage` and `ModelStage` skipped
- Integration: Tier 1 match (wilson < 0.70) → orchestrator hint injected, LLM called normally
- Integration: no match → falls through to full LLM (Tier 2 unchanged)
- Integration: Tier 0 success → `record_recipe_outcome(recipe_id, true)` called → wilson_lower updated
- Integration: Tier 0 failure → `record_recipe_outcome(recipe_id, false)` called → tier possibly downgraded

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
| 1–3 Skills | `intent_examples` entries ≤ 512 chars, capped at 20; `dependency_registry` entries must have valid UUID syntax and non-empty `label` |
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

**§template-rules — Intent expression templates (applies to all component classes):**  
Q1 runs `parse_template` against every intent expression in `intent_examples`. Rules:

| Condition | Severity |
|-----------|----------|
| `template_prefix = ''` AND `template_suffix = ''` — e.g. `"% in %"`, `"%"` | **Hard error** — no anchor; add literal text around each `%` |
| Two `%` with no literal text between them — e.g. `"search % %"` | **Hard error** — adjacent slots are unextractable |
| `template_prefix = ''` AND `template_suffix != ''` — e.g. `"% directory"` | **Warning** — leading-`%` is valid and indexed via suffix; consider adding a word before `%` for precision |
| `{{vars.name}}` in ToolBinding `params` but no `%` in any expression AND no `variable_patterns` for that name | **Hard error** — variable referenced but no source defined |
| `variable_patterns` entry whose `name` does not appear in any `{{vars.name}}` reference | **Warning** — pattern defined but never used |

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

### Phase J — Skill `intent_examples` + Dependency Registry

**Status:** [ ] Pending

**Note:** `required_skills` does not exist. Dependencies between components are expressed
via each component's `dependency_registry` JSONB and step-level traversal expressions (§0.19).
Phase J covers two concerns: (1) Skill intent_examples seeding, and (2) `dependency_registry`
column on all component tables.

#### J.1 Skill `intent_examples` seeding

- `crates/brassclaw_skills/src/types.rs` — add `intent_examples: Vec<String>` (≤512 chars
  each, capped at 20) to `SkillManifest`; enforce limits in `ActivationCriteria::enforce_limits`.
- On skill `auto_passed` transition: call `seed_intent_input` for each intent expression.
- On skill wipe/delete: call `purge_component_inputs(component_id)`.

**Migration V051:**
```sql
ALTER TABLE reborn_skills ADD COLUMN IF NOT EXISTS intent_examples JSONB;
```

#### J.2 `dependency_registry` column on all component tables

Add `dependency_registry JSONB` to every component table that participates in dependency
traversal. This is a nullable column — components with no declared dependencies have
`dependency_registry = NULL` or `[]`.

**Tables to add the column to** (one ALTER TABLE per table; can be a single migration):
`reborn_skills`, `reborn_tools`, `reborn_tool_skills`, `reborn_recipes`, `reborn_actions`,
`reborn_specs`, `reborn_plans`, `reborn_summaries`, `reborn_lessons`, `reborn_docus`,
`reborn_issues`, `reborn_notes`, `reborn_extensions`.

New tables (Phases B, C) include the column from creation: `reborn_python_code`,
`reborn_extension_catalogues`.

**Migration V051** (same file, additional statements):
```sql
ALTER TABLE reborn_skills        ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_tools         ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_tool_skills   ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_recipes       ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
-- ... repeated for all 13 tables
```

#### J.3 `resolve_dependencies` in `fetch_for_turn`

**File:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`

After the IBS compiles a `BuildInstruction`, `fetch_for_turn` calls
`resolve_dependencies` for each `RecipeStep` that carries a non-empty `DependencyExpr`:

```rust
async fn resolve_dependencies(
    pool: &PgPool,
    root_component_id: Uuid,
    expr: &DependencyExpr,
    visited: &mut HashSet<Uuid>,
) -> Result<Vec<ComponentItem>, RetrievalSourceError>
```

Implements the algorithm from §0.19. Results are partitioned into rust/orchestrator
channels by `class_code` and merged into the corresponding `SplitResult` item lists.

#### Tests

- Unit: `SkillManifest` with `intent_examples` YAML roundtrip
- Unit: entry > 512 chars → rejected by `enforce_limits`
- Integration: Skill with `intent_examples` → resolves via `resolve_intent`
- Integration: `resolve_dependencies` with `"1[all]"` → full transitive closure fetched
- Integration: `resolve_dependencies` with `"5[2,6]"` → only indices 2 and 6 fetched, no sub-deps
- Integration: `resolve_dependencies` — UUID already in `visited` → skipped (deduplication)
- Integration: `resolve_dependencies` with cycle in registries → cycle node skipped (visited guard)
- Integration: dependency components routed to correct channel by class_code

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

- Remove `__retrieve_docs__` handler registration from `orchestrator.rs`.
  It is the legacy MemoryDoc path, superseded by the v3-upgraded `__assemble_prior_knowledge__`.
  Remains registered for one release cycle (custom orchestrators may call it);
  then removed.
- Remove step-0 shim comment block from `default.py` (the `# Pre-Phase-5 fallback`
  comment block around the dead `__retrieve_docs__(goal, 5)` call — Phase G already
  removes the call itself; Phase K removes the comment artefact).
- Add deprecation notice to `__list_skills__`: no longer called from default step-0;
  remains callable for external/custom orchestrators.
- `__assemble_prior_knowledge__` is **not removed**. It is the primary prior-knowledge
  assembly function and stays as the canonical call in `default.py`.

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

### Phase M — Variable Intent Templates

**Status:** [ ] Pending

**Goal:** Add `%` slot marker support to intent expressions. Authors can write
`"show me all files in the % directory"` as an intent expression and `resolve_intent`
will match user text that fits the template. Value extraction is automatic from
template segments; `variable_patterns` remains optional refinement.

#### M.1 New migration: V054

**File:** `crates/brassclaw_pg/migrations/V054__reborn_intent_inputs_template.sql`

```sql
ALTER TABLE reborn_intent_inputs
  ADD COLUMN is_template      BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN template_prefix  TEXT,
  ADD COLUMN template_suffix  TEXT;

CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_prefix_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, template_prefix)
    WHERE is_template = true AND template_prefix != '';

CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_suffix_rev_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, reverse(template_suffix))
    WHERE is_template = true AND template_prefix = '' AND template_suffix != '';
```

#### M.2 `seed_intent_input` upgrade

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs`

Extend `seed_intent_input` to detect `%` in `input_text` and populate the new columns:

```rust
pub fn parse_template(expression: &str) -> Option<(String, String)> {
    // Returns None if expression has no %, Some((prefix, suffix)) if it does.
    // prefix = text before first %, suffix = text after last %.
    // Adjacent-slot validation (two % with no literal between) done by caller (Q1).
    if !expression.contains('%') { return None; }
    let prefix = expression.splitn(2, '%').next().unwrap_or("").to_string();
    let suffix = expression.rsplitn(2, '%').next().unwrap_or("").to_string();
    // If suffix == prefix the expression is a bare "%" — both anchors empty.
    let suffix = if suffix.as_str() == expression { String::new() } else { suffix };
    Some((prefix, suffix))
}
```

`seed_intent_input` sets `is_template`, `template_prefix`, `template_suffix` from
`parse_template`. UNIQUE constraint already deduplicates on `(scope, input_text,
input_class, component_id)` so re-seeding is idempotent.

#### M.3 `resolve_intent` SQL upgrade

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs`

Replace the single `AND input_text = $5` predicate with the three-path query from
§0.17.1. The existing exact-match path (Path 0) is unchanged; Paths 1 and 2 are
new OR branches. `ORDER BY` gains the `CASE WHEN input_text = $5 THEN 0 ELSE 1 END`
tiebreaker so exact matches always outrank template matches for the same component.

Pass `$5 = raw user_text` (exact match) — no normalisation step in Rust needed.
PostgreSQL evaluates `$5 LIKE input_text` directly.

#### M.4 Post-match extraction: `extract_template_slots`

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs` (or new
`crates/brassclaw_engine/src/memory/template_extractor.rs`)

```rust
/// Given a matched template expression and the user text, extract slot values.
/// Returns a Vec of (slot_index, value) pairs in left-to-right order.
/// Slot names are "slot0", "slot1", ... unless overridden by variable_patterns.
pub fn extract_template_slots(
    template: &str,    // "show me all files in the % directory"
    user_text: &str,   // "show me all files in the /tmp directory"
) -> Vec<(String, String)>   // [("slot0", "/tmp")]
```

Algorithm: split `template` on `%` → literal segments. Find each segment left-to-right
in `user_text`. Each gap between consecutive segments is a slot value.

Called by `fetch_for_turn` (in `retrieval_source.rs`) after a template match resolves,
before `variable_patterns` validation/refinement. The extracted `(name, value)` pairs
feed the `{{vars.name}}` substitution step.

#### M.5 `variable_patterns` as optional post-extract refinement

When a template match occurs and `variable_patterns` is non-empty on the variant:
1. Auto-extract slot values via `extract_template_slots`.
2. For each `variable_patterns` entry: apply its regex to the auto-extracted value
   (not to the full `user_text`). If the regex matches a named group, that group's
   value replaces the positional slot name in the vars map.
3. If `variable_patterns` is empty: use positional names `slot0`, `slot1`, ...

This means an author can choose:
- **Simple case:** `"show me files in the % directory"` — no `variable_patterns`. Slot
  auto-extracted as `vars.slot0`. ToolBinding params reference `{{vars.slot0}}`.
- **Semantic case:** Same template + `variable_patterns: [{name: "dir", pattern: ...}]`.
  Auto-extract produces the raw value; the pattern validates and names it `vars.dir`.
  ToolBinding params reference `{{vars.dir}}`.

#### M.6 WebUI — template authoring feedback

In the intent expression input field:
- `%` characters rendered as a distinct chip/token (not plain text).
- Live feedback line shows computed prefix/suffix anchors and their classification
  (green = anchored, yellow = suffix-only leading-`%`, red = no-anchor blocked).
- On save: Q1 template rules run immediately; error/warning shown inline.

#### Files to create

- `crates/brassclaw_pg/migrations/V054__reborn_intent_inputs_template.sql`
- `crates/brassclaw_engine/src/memory/template_extractor.rs` (or inline in `intent_system.rs`)

#### Files to modify

- `crates/brassclaw_engine/src/memory/intent_system.rs`
  — `parse_template` helper
  — `seed_intent_input`: detect `%`, populate `is_template`/`template_prefix`/`template_suffix`
  — `resolve_intent`: three-path SQL query
- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  — `fetch_for_turn`: after template match, call `extract_template_slots` before `variable_patterns` refinement
- `crates/brassclaw_engine/src/memory/component_validator.rs` (Phase I)
  — template Q1 rules (adjacent slots, no-anchor, dangling patterns) — add here when Phase I runs

#### Tests

- Unit: `parse_template("show me files in the % directory")` → `Some(("show me files in the ", " directory"))`
- Unit: `parse_template("% directory")` → `Some(("", " directory"))`
- Unit: `parse_template("% in %")` → `Some(("", ""))` → Q1 rejects (no anchor)
- Unit: `parse_template("search for % in %")` → `Some(("search for ", ""))` — prefix-anchored, valid
- Unit: `parse_template("no slots here")` → `None`
- Unit: `extract_template_slots("show me files in the % dir", "show me files in the /tmp dir")` → `[("slot0", "/tmp")]`
- Unit: `extract_template_slots("search for % in %", "search for TODO in /src")` → `[("slot0", "TODO"), ("slot1", "/src")]`
- Unit: `extract_template_slots` with adjacent slots `"% %"` → empty / error (undefined behaviour blocked by Q1)
- Integration: `resolve_intent("show me all files in the /tmp directory")` → matches template row `"show me all files in the % directory"`
- Integration: `resolve_intent("show me all files in the /tmp directory")` — exact literal row present → ranks above template match for same component
- Integration: `resolve_intent("/tmp directory")` → matches suffix-anchored template `"% directory"` via reverse index
- Integration: slot values flow through to `{{vars.slot0}}` substitution in ToolBinding params

---

### Phase N — Validation Queue

**Status:** [ ] Pending

**Goal:** Introduce `reborn_validation_queue` as the single authoritative pre-validation
lifecycle table. Move `queue_code`, `review_attempts`, `review_feedback`, `rejected_at`,
and `validation_errors` off the 13 component tables and onto the queue. The component
tables' `validation_status` column and all retrieval queries are unchanged.

> **Pre-requisite awareness:** Phase N touches 13 component tables. Each column removal
> is a two-step migration: (1) add the column to the queue table and populate it from
> the existing component columns (data migration), (2) drop the now-redundant columns.
> Both steps are in a single migration file (V055). The migration is additive-first,
> destructive-second within one transaction to guarantee atomicity.

#### N.1 New migration: V055

**File:** `crates/brassclaw_pg/migrations/V055__reborn_validation_queue.sql`

```sql
-- Step 1: create the queue table (see §0.18 for full DDL)
CREATE TABLE reborn_validation_queue ( ... );

-- Step 2: populate from existing component table state
-- For every component that is NOT yet 'validated':
-- Map validation_status → state, review_attempts → counter, etc.
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id,
     component_id, component_class, state, counter,
     review_feedback, validation_errors, submitted_at)
SELECT
    tenant_id, user_id, agent_id, project_id,
    id,
    1,  -- class_code
    CASE validation_status
        WHEN 'pending'           THEN 1
        WHEN 'upgrade_queued'    THEN 1
        WHEN 'auto_failed'       THEN 1
        WHEN 'auto_passed'       THEN 2
        WHEN 'review_requested'  THEN 2
        WHEN 'rejected'          THEN 3
        WHEN 'garbage'           THEN 4
        ELSE 1
    END,
    COALESCE(review_attempts, 0),
    review_feedback,
    validation_errors,
    created_at
FROM reborn_skills
WHERE validation_status != 'validated'
-- Repeated for each of the 13 component tables with correct class_code values.
ON CONFLICT DO NOTHING;

-- Step 3: add last_graduation_at to scope cursor
-- reborn_monty_vm_settings has one guaranteed row per scope (upserted on first access).
-- The graduation trigger's UPDATE will always find a row. No separate cursor table needed.
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN IF NOT EXISTS last_graduation_at TIMESTAMPTZ;

-- Step 4: trigger — bump last_graduation_at on queue row DELETE (= graduation)
CREATE OR REPLACE FUNCTION reborn_validation_queue_graduation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    UPDATE reborn_monty_vm_settings
       SET last_graduation_at = now()
     WHERE tenant_id  = OLD.tenant_id
       AND user_id    = OLD.user_id
       AND agent_id   = OLD.agent_id
       AND project_id = OLD.project_id;
    RETURN OLD;
END;
$$;
CREATE TRIGGER reborn_validation_queue_on_delete
    AFTER DELETE ON reborn_validation_queue
    FOR EACH ROW EXECUTE FUNCTION reborn_validation_queue_graduation();

-- Step 5: drop redundant columns from component tables
-- (After data has been migrated to the queue)
ALTER TABLE reborn_skills
    DROP COLUMN IF EXISTS queue_code,
    DROP COLUMN IF EXISTS review_attempts,
    DROP COLUMN IF EXISTS review_feedback,
    DROP COLUMN IF EXISTS rejected_at,
    DROP COLUMN IF EXISTS validation_errors;
-- Repeated for all 13 component tables.
-- validation_status is NOT dropped — it remains as the post-validation gate.
```

**Note on scope cursor:** `reborn_monty_vm_settings` has a guaranteed row for every
active scope. `PgMontyVmSettingsStore::upsert` (line 103 in `pg_monty_vm_settings.rs`) is
called on every scope's first access using `ON CONFLICT ... DO UPDATE` — no scope can reach
the graduation trigger without a settings row. The trigger's `UPDATE` will always find a
row. No separate `reborn_scope_cursors` table is needed. ✓ Resolved.

#### N.2 Application-layer write paths

**File:** new `crates/brassclaw_reborn_composition/src/validation_queue.rs`

```rust
pub struct ValidationQueueStore { /* pool */ }

impl ValidationQueueStore {
    /// Submit a component to Q1 queue (state 1).
    /// Called when a component is created or edited.
    pub async fn submit(&self, scope, component_id, component_class) -> Result<()>;

    /// Transition state 1 → state 2. ONLY called by Gate 1 validator on clean pass.
    /// Returns Err if called from any other context (enforced by Rust visibility:
    /// this method is pub(crate) and only reachable from component_validator.rs).
    pub(crate) async fn gate1_pass(&self, scope, component_id, errors: &[]) -> Result<()>;

    /// Record Q1 failure — stays in state 1, increments nothing (author must fix and resubmit).
    pub(crate) async fn gate1_fail(&self, scope, component_id, errors: &[String]) -> Result<()>;

    /// Q2 rejection: state 2 → state 3. Increments counter. Promotes to state 4 if counter >= threshold.
    pub async fn reject(&self, scope, component_id, feedback: &str) -> Result<()>;

    /// Q2 approval: delete queue row → graduation. Updates component's validation_status = 'validated'.
    pub async fn approve(&self, scope, component_id) -> Result<()>;

    /// List all queue rows for a scope (WebUI validation view).
    pub async fn list(&self, scope, state_filter: Option<u8>) -> Result<Vec<QueueRow>>;

    /// Deletion candidate cleanup: delete state-4 rows and their components.
    pub async fn purge_deletion_candidates(&self, scope) -> Result<u64>;
}
```

**Visibility invariant for `gate1_pass`:** `pub(crate)` — only callable from within
`brassclaw_reborn_composition`. The Gate 1 validator lives in this crate. The API
layer (webui_v2, ingress) cannot call `gate1_pass` directly — it can only call `submit`.
This is the Rust-level enforcement of the state-2 write invariant.

#### N.3 Cache integration

The SplitResult memo-cache in `PostgresSource` gains a `last_graduation_at` check:

```rust
// On every cache hit, before returning the cached SplitResult:
let cursor = sqlx::query_scalar!(
    "SELECT last_graduation_at FROM reborn_monty_vm_settings
     WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4",
    scope.tenant_id, scope.user_id, scope.agent_id, scope.project_id
).fetch_optional(pool).await?;

if let Some(Some(graduated_at)) = cursor {
    if graduated_at > cache_entry.cached_at {
        cache.remove_scope(scope);
        // Recompute — fall through to full fetch_for_turn
    }
}
```

One PK read per cache hit. Sub-millisecond. No TTL needed. Cache entries for a scope
are evicted as a batch when any component in the scope graduates — conservative but
correct. Fine-grained per-component eviction is a future optimisation.

#### N.4 Component table cleanup

Remove from all 13 component tables: `queue_code`, `review_attempts`, `review_feedback`,
`rejected_at`, `validation_errors`.

> **Rust struct sync required:** After V055 drops these columns, ALL structs that read
> or write them must be updated atomically. Affected:
> - `Recipe` + `ToolSkill` in `crates/brassclaw_engine/src/types/recipe.rs` —
>   remove `validation_errors`, `review_feedback`, `review_attempts`, `rejected_at`.
> - `PgRecipe` in `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` —
>   this struct also selects `queue_code`, `review_attempts`, `review_feedback`,
>   `rejected_at`, `validation_errors` from `reborn_recipes`. Must be updated.
> - `component_validator.rs` — creates `Recipe` structs with `validation_errors`.
> - `recipe_matcher.rs` — reads `wilson_lower` + `tier` (these are NOT dropped by V055,
>   but the file also references `validation_errors` in some paths — audit required).
> - Any other caller that constructs or destructures these structs.
>
> **Two-phase deploy required (zero-downtime):**
> V055 drops columns. If the old binary is still running when V055 runs (rolling deploy),
> it will SELECT dropped columns → runtime panic on every request. Required deploy order:
> 1. Deploy new binary (with structs updated to not SELECT the dropped columns).
> 2. Run V055 migration (now safe — binary no longer queries dropped columns).
> This means the binary must handle `None`/missing values for these fields
> gracefully BEFORE V055 runs. Use `Option<T>` + `#[serde(default)]` on the struct
> fields in the interim. After V055 runs, fields can be removed entirely.

> **Rust struct sync required:** After V055 drops these columns, the corresponding Rust
> structs must be updated in the same phase. Affected:
> - `Recipe` in `crates/brassclaw_engine/src/types/recipe.rs` —
>   remove `validation_errors`, `review_feedback`, `review_attempts`, `rejected_at`.
> - `ToolSkill` in `crates/brassclaw_engine/src/types/recipe.rs` — same fields.
> - Any sqlx query or struct that selects these columns by name will fail to compile.
> Use `#[serde(default)]` + `Option` migration strategy: first add the queue columns to
> the queue table (step 1 of V055), then remove from component structs + tables in the
> same migration transaction. Failing to do this in sync will break deserialisation.

The 13 tables are: `reborn_skills`, `reborn_tools`, `reborn_tool_skills`,
`reborn_recipes`, `reborn_actions`, `reborn_specs`, `reborn_plans`, `reborn_summaries`,
`reborn_lessons`, `reborn_docus`, `reborn_issues`, `reborn_notes`,
`reborn_extensions` (unified), plus the new Phase B/C tables `reborn_python_code` and
`reborn_extension_catalogues` — which should be designed without these columns from
the start (they are new tables authored after this design is decided).

**`reborn_python_code` and `reborn_extension_catalogues` (Phases B and C):** These
tables are created after Phase N is planned. They must NOT include `queue_code`,
`review_attempts`, `review_feedback`, `rejected_at`, or `validation_errors` columns —
they rely on `reborn_validation_queue` from day one.

#### N.5 Integrity check at boot

A boot-time check (in `brassclaw_reborn_composition` init sequence):

```sql
-- Components not in 'validated' state that have no queue row are inconsistent.
SELECT component_id, 'skills' AS source FROM reborn_skills
WHERE validation_status != 'validated'
  AND id NOT IN (SELECT component_id FROM reborn_validation_queue
                 WHERE tenant_id = $1 AND ...)
-- UNION ALL for each table
```

Inconsistent rows are logged as warnings and automatically submitted to state 1 as
a recovery action. This covers edge cases from the V055 data migration.

#### Tests

- Unit: `gate1_pass` is `pub(crate)` — not callable from outside the crate (compile-time)
- Unit: submit → `state = 1`
- Unit: `gate1_pass` → `state = 2`; `gate1_fail` → `state = 1`, errors populated
- Unit: `reject` → `state = 3`, counter incremented
- Unit: `reject` when `counter >= threshold` → `state = 4` (auto-promotion)
- Unit: `approve` → queue row deleted; component `validation_status = 'validated'`
- Integration: component approval → `last_graduation_at` bumped on scope cursor
- Integration: cache hit after graduation → cache entry discarded, SplitResult recomputed
- Integration: cache hit with no graduation → cached result returned, no recompute
- Integration: boot integrity check → components with missing queue rows auto-submitted
- Integration: `list(state_filter: Some(2))` → returns only Q1-passed components awaiting Q2

---

## 2. Migration Sequence

| Migration | Contents | Status |
|-----------|----------|--------|
| `V047__reborn_recipe_step_descriptions.sql` | `ADD COLUMN step_descriptions JSONB` to `reborn_recipes` | **Next** |
| `V048__reborn_python_code.sql` | New table `reborn_python_code`, class 22 | |
| `V049__reborn_extension_catalogues.sql` | New table `reborn_extension_catalogues`, class 23 | |
| `V050__reborn_intent_inputs_step_link.sql` | `ADD COLUMN step_link TEXT` to `reborn_intent_inputs` | |
| `V051__reborn_skills_intent_examples.sql` | `ADD COLUMN intent_examples JSONB` to `reborn_skills`; `ADD COLUMN dependency_registry JSONB` to all 13 component tables (see Phase J.2 — §0.19) | |
| `V052__reborn_basic_prompt_store.sql` | New table: one row per scope, `bundle_json JSONB`, `is_stale BOOL`, `fingerprint TEXT` | |
| `V053__reborn_tools_capability_id_and_system_source.sql` | `ADD COLUMN capability_id TEXT` to `reborn_tools` + `source = 'system'` allowed on tools/tool_skills/skills | |
| `V054__reborn_intent_inputs_template.sql` | `ADD COLUMN is_template BOOL`, `template_prefix TEXT`, `template_suffix TEXT` to `reborn_intent_inputs`; two new partial indexes for prefix/suffix-anchored template matching (see §0.17.2) | |
| `V055__reborn_validation_queue.sql` | New table `reborn_validation_queue` (§0.18); populate from existing component table state; add `last_graduation_at` to scope cursor; graduation trigger; drop `queue_code`/`review_attempts`/`review_feedback`/`rejected_at`/`validation_errors` from all 13 component tables | |

All additive-first. No DROP, no renames. No existing rows break.

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
| 1 | Variable extraction: named capture groups vs. post-match LLM extraction? | **Resolved — see §0.17.** Intent expressions use `%` slot markers for matching (Phase M). Slot values are auto-extracted from template segments (positional names `slot0`, `slot1`, …). `variable_patterns` is optional post-extraction refinement for semantic naming and validation. LLM extraction remains a future opt-in via `llm_var_extraction_prompt` on `RecipeVariant` (out of scope). |
| 2 | BuildInstruction memoisation: per-process or always recompute? | **Resolved — see §0.18 + Phase N.** Per-process SplitResult cache keyed on `sha256(step_link + "\|" + sorted_include_uuids.join(","))` per scope. Eviction is event-driven via `last_graduation_at` on the scope cursor (bumped by DB trigger when a component graduates from `reborn_validation_queue`). One sub-millisecond PK read per cache hit. No TTL required as primary mechanism. |
| 3 | `required_skills` inclusion: always include vs. score against current query? | **Resolved — see §0.19.** `required_skills` does not exist. Dependencies are declared per-component in `dependency_registry` JSONB and referenced from StepDescription steps via typed traversal expressions (`1[all], 5[2,6], 17[3, 7[1,4]]`). Always resolved fully per the traversal expression — no scoring, no cap. KV-cache prefix absorbs token cost in steady state. |
| 4 | `step_formatter_id` scope: per-recipe, per-variant, or per-step? | **Resolved — not needed.** `step_formatter_id` does not exist. Formatting is achieved by authoring PythonCode component bodies with the correct content and prose style. `type: "text"` steps are WebUI annotations only with no runtime emission. All three intent-match cases (Recipe match, near-miss, full fallback) have their formatting handled by PythonCode bodies, prepared prompt templates, and the KV-cache prefix respectively. |
| 5 | StepDescription storage format: YAML files in git vs. JSONB in `reborn_recipes`? | **Resolved — JSONB (§0.5).** YAML files in git are structurally incompatible (no WebUI write path, no scope isolation, requires deploy cycle). JSONB column on `reborn_recipes` is the correct choice. Each JSONB element holds a dual representation: `yaml_source` (raw YAML, WebUI display) + `steps` (pre-parsed array, IBS reads). YAML is parsed once at WebUI save time — the IBS never parses YAML at runtime. |
| 6 | Legacy DocPlan → v3 translation? | **Resolved — see §3.1.** JSONB is the storage format (Q5). The translation pipeline creates new v3 components from legacy MemoryDoc rows; dependency registries are decided at authoring time, not inferred by translation. Action-format steps with name references must be resolved to UUIDs; unresolvable names are Q1 hard errors. Step type (text/component/snippet) and component class are orthogonal. |
| 7 | `__assemble_prior_knowledge__` removal timing? | **Not removed.** `__assemble_prior_knowledge__` IS the v3-upgraded primary function — it already calls `fetch_for_turn` and returns `{content, formatted_content, override_prompt_creation, matched_component_ids}`. Phase F extends it to handle `SplitResult` and `ActionShortCircuit`. Phase G removes the dead `__retrieve_docs__(goal, 5)` shim from step-0 (NOT the `__assemble_prior_knowledge__` call). Phase K removes `__retrieve_docs__` handler registration (the legacy MemoryDoc path). |
| 8 | Should builtin Tool/ToolSkill/Skill rows bypass Q2 (auto-validated)? | Yes — `source = "system"`, `validation_status = "validated"` at seeder insert. Q1 runs inside the seeder at build time. Q1 errors in seeder content are CI build failures. This prevents boot from requiring human Q2 completion for core tools. |
| 9 | Should the MCP translator also be used for builtins? | No — wrong granularity (1:1 per tool, no task-level Skills, no PythonCode, no multi-ToolSkill Recipes). Use `builtin_bootstrap.rs` (Phase L) for builtins. MCP translator is for external third-party MCPs only. |
| 10 | What recipe variants should `builtin.shell` have? | Two: (a) known-safe commands (allowlist: `cargo build/test/fmt/clippy`, `git status/log/diff`, `npm install/build`) at Tier 1 high-confidence; (b) open-ended arbitrary command at Tier 1 always with explicit approval annotation. Both have `llm_call_required: true` — no shell is ever Tier 0. |
| 11 | How does the Rust execution layer resolve a Tool DB UUID to its registered capability handler? | Via `capability_id` column (V053). On tool dispatch: look up Tool row by UUID → read `capability_id` → look up handler in `FirstPartyCapabilityRegistry` by `capability_id`. For user-authored tools without `capability_id`, fall back to existing name-based resolution. |
| 12 | Should builtin Recipes also have `source = "system"` and bypass Q2? | Yes — same reasoning as Q8. Builtin Recipe StepDescriptions are hand-authored and IBS pre-flight-checked at seeder run time. Q2 bypass for `source = "system"` Recipes is consistent with Tools and ToolSkills. |
| 13 | If `RecipeStage` already stashed the items (Tier 1), how does `handle_assemble_prior_knowledge` know not to call `fetch_for_turn` again? | **Resolved — stash/unstash protocol (Phase H §5).** `handle_assemble_prior_knowledge` checks `state.recipe_hint` before doing anything. If set, it skips `fetch_for_turn` entirely, deserializes the stashed `Vec<ComponentItem>` from `serde_json::Value`, clears the field (one-shot consume), and formats. If absent (Tier 2 / no-match), calls `fetch_for_turn` as before. No double-fetch, no second `resolve_intent`, no second IBS compilation. |
| 14 | In Tier 0, `PromptStage` and `ModelStage` are skipped — but the Python script calls `__assemble_prior_knowledge__`. Where does Python execute in Tier 0? | The Python scripting engine is **not** the LLM call. `PromptStage` assembles the LLM input prompt; `ModelStage` sends it to the model. Both are skipped in Tier 0. `default.py` is invoked by `CapabilityStage` (or equivalent) independently. In Tier 0, Python runs step-0, calls `__assemble_prior_knowledge__` (gets the stash), and invokes skills/tools directly — no LLM round-trip in the middle. "Tier 0: no LLM" means no LLM call, not no Python execution. |
| 15 | What happens if `build_instruction` returns an `IbsError` during the builtin seeder (Phase L)? `panic!` or return an error? | **Debug builds: `panic!`** — seeder content is hand-authored; an IbsError here is a compile-time bug. **Release builds:** `error!`-log, skip the Recipe row, continue. The seeder is idempotent — skipped rows do not block boot. CI must run the seeder in debug mode so IbsErrors become build failures before reaching production. |

### 3.1 Legacy DocPlan → v3 Component Translation

The v2 system stored all knowledge as `MemoryDoc` rows. `component_import.rs` already
migrates these to class-specific tables. The gap: no v2 docs have StepDescriptions or
`step_link` formulae.

**One-time, operator-triggered pipeline (`brassclaw migrate-to-v3`):**

1. **Skills (class 1–3):** Extract imperative sentence candidates from skill `body` as
   seed intent examples. Generate StepDescription0:
   - Step 1: `knowledge: orchestrator`, `type: text`, `info`: summary of what this skill does (WebUI annotation)
   - Step 2: `knowledge: orchestrator`, `type: component`, `include`: [skill UUID]
   Default `step_link: "0:0-0:E"`. Route to Q1.

2. **ToolSkills (class 13):** Generate StepDescription0 with one rust-channel `component`
   step. No intent examples (ToolSkills are referenced by Skills, not intent-matched directly).

3. **Existing Recipes (class 21) without `step_descriptions`:** Map each entry in the
   existing `steps` JSONB (13-type Action format) to v3 StepDescription steps:
   - Steps with a component reference by **name**: resolve the name to a UUID by querying
     the component tables (any class) within the same scope. On success → `type: component`
     with the resolved UUID in `include`. On failure (name not found) → Q1 **hard error**
     on the translated Recipe ("unresolvable component reference: <name>"); the Recipe is
     flagged and requires manual correction before it can activate.
   - Steps with no component reference → `type: text` (WebUI annotation only; no runtime effect).
   Route to Q1. `yaml_source` is synthesised from the generated steps array.

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
- `FormatOrchestratorPrompt` as a distinct step type (not needed — formatting handled by PythonCode component bodies)

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
│   routing.tier0_eligible = true (wilson_lower=0.82, tier=mature, validated)
│   routing.llm_call_required = false → Tier 0
│   apply rust_items to Rust execution context (silent, never forwarded to orchestrator)
│   serialize orchestrator_items → state.recipe_hint (JSONB stash, one-shot)
│   serialize rust_items        → state.recipe_rust_context (JSONB stash, one-shot)
│   return PostRecipeOutcome::TierZero
│
├─ [PromptStage SKIPPED — no LLM prompt assembly needed]
├─ [ModelStage  SKIPPED — no LLM call in Tier 0]
│
│   NOTE: The Python scripting engine (default.py) DOES run in Tier 0.
│   "No LLM" means PromptStage (prompt assembly) + ModelStage (LLM call) are skipped.
│   The Python engine is separate from the LLM call and is NOT skipped.
│
├─ [CapabilityStage / Python execution]
│   Python runs default.py step 0:
│     pkr = __assemble_prior_knowledge__(goal, budget, "02")
│     handler checks state.recipe_hint → SET → unstash, skip fetch_for_turn
│     clears state.recipe_hint (consumed)
│     pkr["orchestrator_content"]:
│       ## [Skill: ls]
│         <ls-skill body>
│       ## [PythonCode: ls-result-handler]
│         <ls-result-handler body>
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

### Tier 1 (intent match, LLM-guided)

```
User types: "edit main.rs and refactor the error handler"
(wilson_lower = 0.61 — confident match, but llm_call_required = true)
│
├─ [InputStage]
│   state.last_user_text = "edit main.rs and refactor the error handler"
│
├─ [RecipeStage]
│   fetch_for_turn(scope, last_user_text, budget, "02")
│     → resolve_intent → Match { component_id: uuid-builtin-edit-file,
│                                class_code: 21,
│                                step_link: "0:0-0:E+2:0-2:E" }
│     → IBS: build_instruction("0:0-0:E+2:0-2:E", step_descriptions, variable_patterns)
│         rust_steps:         [component(uuid-edit-toolskill, knowledge:rust)]
│         orchestrator_steps: [component(uuid-edit-skill), component(uuid-patch-formatter)]
│         llm_call_required:  true
│     → fetch rust_items, orchestrator_items
│     → FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }
│   routing.tier0_eligible = false (wilson_lower=0.61 < 0.70 or llm_call_required=true)
│   serialize orchestrator_items → state.recipe_hint        (JSONB stash)
│   serialize rust_items        → state.recipe_rust_context (JSONB stash)
│   return PostRecipeOutcome::NeedsPrompt → (does NOT skip PromptStage/ModelStage)
│
├─ [PromptStage]
│   state.recipe_hint present → deserialize orchestrator_items from stash
│   inject as prior_knowledge BEFORE calling fetch_for_consumer (UNION ALL)
│   recipe hint always injected first; UNION ALL fills remaining budget only
│   volatile context injected separately (never mixed with prior knowledge)
│   NOTE: recipe_hint is NOT consumed here (handler consumes it in step-0 below)
│
├─ [InterceptorStage]  Sempai reviews outgoing prompt (recipe hint visible to Sempai)
│
├─ [ModelStage]        LLM call with injected recipe context
│
├─ [CapabilityStage / Python execution]
│   Python runs default.py step 0:
│     pkr = __assemble_prior_knowledge__(goal, budget, "02")
│     handler checks state.recipe_hint → SET → unstash, skip fetch_for_turn
│     clears state.recipe_hint (consumed — one-shot)
│     pkr["orchestrator_content"]:
│       ## [Skill: file-editing]
│         <skill body>
│       ## [PythonCode: patch-formatter]
│         <pythoncode body>
│     pkr["override_prompt_creation"]: false (Tier 1 uses LLM)
│   → LLM guided by skill bodies; capability calls execute via pre-loaded rust_context
│   → rust_items applied from state.recipe_rust_context before Python starts
│
└─ [AssistantReplyStage]  emit LLM response; Wilson score updated
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
