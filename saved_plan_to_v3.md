# Recipe System Finalisation Plan

> **Status:** Draft — for review before implementation begins.  
> **Scope:** Closes all architectural gaps identified in the Vision vs. Implementation analysis.  
> **No code changes are made by this document.**

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
From this point on, the word "Extension" in a runtime context always refers to these.

**ExtensionCatalogues (class 23)** are the documentation namespace. Separate class,
separate table, separate concern.

---

### 0.2 ExtensionCatalogue — Correct Design

An ExtensionCatalogue does **not** re-document commands. Every component it owns
already documents itself:

- A **Tool** describes the binary.
- A **ToolSkill** describes how the Rust layer calls that tool.
- A **Skill** describes how the orchestrator tells Rust to use that tool.
- A **PythonCode** component carries utility code or inline orchestrator instructions.
- A **Recipe** assembles all of the above into a step plan for a specific intent.

The ExtensionCatalogue's job is to draw the **bigger picture**:
> "This catalogue covers local file management. Its Recipes handle these task groups:
> file-reading, directory-listing, file-copying, file-moving, file-permissions,
> file-creation. Here is a map of which Recipes serve which use-cases."

#### Mandatory shape

| Section | Content |
|---------|---------|
| `name` | Catalogue identifier (e.g. `local-file-management`) |
| `version` | Semver-like label |
| `description` | One-paragraph summary for LLM fallback context |
| `task_groups[]` | Named categories: `{ group_name, summary, recipe_ids[] }` |
| `child_component_ids[]` | All owned component UUIDs (any class) for lineage |
| `intent_index[]` | Read-only copy of all intent expressions owned by child Recipes — **for forensic/audit purposes only, never seeded into `reborn_intent_inputs`** |

**Example for `local-file-management`:**

```
task_groups:
  - group: directory-listing
    summary: "Reading and listing directory contents."
    recipes:
      - listing-the-contents-of-a-directory
      - listing-all-contents-of-a-directory
      - listing-contents-of-a-hidden-directory
      - listing-directory-contents-with-details
  - group: file-copying
    summary: "Copying files and directories."
    recipes:
      - copy-file-to-destination
      - copy-directory-recursively
  …
```

**When no intent matches:** The RetrievalEngine's normal assembly path already
includes all Skills, ToolSkills, and Recipes in the big basic-prompt (§0.10).
The ExtensionCatalogue's `description` and `task_groups` summaries are included
in that basic-prompt to give the LLM the larger picture — not duplicating
tool usage, but explaining the domain and what is available. The LLM can then
reason about which capability to use without being given the implementation details
again (those are already in the basic-prompt).

---

### 0.3 Recipe — Correct Design

A Recipe is a **complete turn script**. It describes, step by step, which part
of the entire turn pipeline does what, exactly when, using which component
(fetched by UUID). It is the primary intent target.

A Recipe does **not** describe how a tool works — that is the Skill's and
ToolSkill's job. A Recipe describes the **control flow of one turn**:
what the RetrievalEngine fetches, what the orchestrator does with it,
what it sends to Rust, what Rust does, and what happens with the result.

#### Mandatory shape

| Field | Content |
|-------|---------|
| `name` | Recipe identifier (e.g. `listing-the-contents-of-a-directory`) |
| `description` | One-sentence summary of what this Recipe accomplishes |
| `category` | Task group (maps to `ExtensionCatalogue.task_groups[].group_name`) |
| `variants[]` | One or more `RecipeVariant` entries; each fully specifies one turn |

#### Intent Variants — one per concrete user intent

A Recipe has one variant per distinct usage pattern. Each variant:
- owns its own set of intent expressions (stored as rows in `reborn_intent_inputs`)
- contains a full, self-contained **`BuildInstruction`**: the complete recipe for
  how to execute that specific intent from start to finish
- carries its own variable-extraction patterns for runtime parameter binding

Multiple intent rows in `reborn_intent_inputs` map to the same Recipe `component_id`
but carry different `variant_key` values, selecting the correct `BuildInstruction`.

**Example — Recipe `listing-the-contents-of-a-directory`:**

```
variant: ls-l
  intents: ["ls -l", "show me all files in the local directory",
            "list files in the local directory", "show local directory files"]
  build_instruction: { ... full turn script for ls -l ... }

variant: ls-la
  intents: ["ls -la", "show all files including hidden",
            "show really all files", "list all files including hidden ones"]
  build_instruction: { ... full turn script for ls -la ... }

variant: ls-other-dir
  intents: ["list all files of the /tmp directory",
            "show files in {{vars.dir}}"]
  variable_patterns: [{ name: "dir", pattern: r"of the (?P<dir>[/\w.-]+)" }]
  build_instruction: { ... same as ls-l but step params include {{vars.dir}} ... }
```

All variants share the same Recipe `component_id` in `reborn_intent_inputs`.
The `variant_key` column selects which `BuildInstruction` to execute.

---

### 0.4 Recipe Steps — The Turn Script

A `BuildInstruction` is a complete, ordered list of **atomic turn steps**.
Each step describes one indivisible action at one specific stage of the turn.
Nothing is combined. The turn executor reads them in sequence.

The steps cover **every participant** in the turn:
- the **RetrievalEngine** (fetching components into the context window)
- the **Interceptor / PromptStage** (assembling the LLM prompt, or bypassing it)
- the **orchestrator** (receiving instructions, deciding what to do)
- the **Rust execution layer** (receiving the tool call, reading ToolSkills, executing Tools)
- the **result relay** (returning output back up the chain)

#### Step types (exhaustive)

| Type | Owner | What it does |
|------|-------|-------------|
| `fetch_components` | RetrievalEngine | Fetch exact component UUIDs into the context window for this stage |
| `assemble_llm_prompt` | PromptStage | Compose the LLM prompt from fetched components + conversation history |
| `skip_llm` | PromptStage / RecipeStage | Do not call the LLM; orchestrator executes directly (Action path) |
| `run_python_code` | Orchestrator | Execute a `PythonCode` component (fetched by UUID) inline |
| `invoke_skill` | Orchestrator | Send a `Skill` component's instructions (fetched by UUID) to the Rust layer |
| `read_tool_skill` | Rust layer | Read a `ToolSkill` component (fetched by UUID) to get invocation parameters |
| `call_tool` | Rust layer | Execute a registered `Tool` with bound parameters |
| `relay_result` | Rust layer → Orchestrator | Return tool output upward to the orchestrator |
| `conditional_branch` | Orchestrator | Evaluate a condition; select next step sequence |
| `set_variable` | Any | Bind a named variable to a value for downstream parameter substitution |
| `loop_steps` | Orchestrator | Repeat a sub-sequence N times or until condition |
| `call_action` | Orchestrator | Invoke a nested `Action` component (by UUID) |
| `emit_event` | Any | Emit a named event into the turn's event stream |

#### Complete example — SSH connection

Every step names the exact participant and the exact component UUID involved.

```
BuildInstruction for variant: ssh-connect

variable_patterns:
  - { name: "host", pattern: r"(?:to|into) (?P<host>[\w.\-]+)" }

steps:
  1. fetch_components
       owner: RetrievalEngine
       component_ids: ["<uuid:ssh-connect-skill>", "<uuid:ssh-toolskill>",
                       "<uuid:ssh-result-handler>"]
       description: "Load SSH skill, ToolSkill, and result handler into context window"

  2. skip_llm
       owner: RecipeStage
       condition: "wilson_lower >= 0.70"
       description: "High-confidence recipe: skip LLM call, execute directly"
       else: assemble_llm_prompt   ← fall back to LLM if confidence not yet there

  3. run_python_code
       owner: Orchestrator
       component_id: "<uuid:ssh-pre-invocation-check>"
       description: "Orchestrator runs pre-flight check (known hosts, key availability)"
       params: { host: "{{vars.host}}" }

  4. invoke_skill
       owner: Orchestrator
       component_id: "<uuid:ssh-connect-skill>"
       description: "Orchestrator sends SSH invocation instruction to Rust layer"
       params: { host: "{{vars.host}}", timeout_secs: 10 }

  5. read_tool_skill
       owner: Rust layer
       component_id: "<uuid:ssh-toolskill>"
       description: "Rust reads ToolSkill: param schema, preconditions, error handling"

  6. call_tool
       owner: Rust layer
       tool: "ssh"
       params: { host: "{{vars.host}}", timeout_secs: 10 }
       description: "Rust executes the ssh tool"

  7. relay_result
       owner: Rust layer → Orchestrator
       description: "Rust returns stdout/stderr + exit code to orchestrator"

  8. run_python_code
       owner: Orchestrator
       component_id: "<uuid:ssh-result-handler>"
       description: "Orchestrator processes result: formats output, checks exit code"
```

Every `component_id` in the step list is fetched by the RetrievalEngine using
`fetch_component_by_id` before the step executes — only what that step needs
enters the context window at that moment.

---

### 0.5 The RetrievalEngine and Prior-Knowledge Assembly

The **RetrievalEngine** (`crates/brassclaw_engine/src/memory/retrieval.rs` and
`retrieval_source.rs`) assembles the `prior_knowledge` block that becomes part
of the LLM prompt (specifically, the `memory_snippets` section of the
`InstructionBundle` built in `instruction_bundle.rs`).

#### Current modes

| Mode | Trigger | Behaviour |
|------|---------|-----------|
| **Normal UNION ALL assembly** | No intent match | Keyword-score all validated components across all tables. Fill token budget. Order: `(class_code ASC, prompt_uid ASC)`. |
| **Override path (current)** | Intent match → `override_prompt_creation = true` | Use `prior_knowledge_content` text blob verbatim. |

#### Upgraded override path — BuildInstruction-driven

The verbatim text blob is replaced by a structured **`BuildInstruction`** stored
in each `RecipeVariant`. The RetrievalEngine interprets the instruction to fetch
and assemble exactly the right components for that specific intent.

```
RecipeStage: resolve_intent(user_text)
    → IntentResolution::Match { component_id, variant_key }
    → PostgresSource::fetch_component_by_id(component_id, class=21)  ← fetch the Recipe
    → recipe.variants.find(variant_key)
    → variant.build_instruction                                       ← the turn script
    → RetrievalEngine::fetch_by_instruction(scope, instruction)
        → for each step with component_ids[]:
              fetch_component_by_id(uuid) per ID                      ← exact fetch
        → apply variable substitution: {{vars.name}} → runtime values
    → return ordered Vec<ComponentItem> (the focused patch)
```

The focused patch replaces the UNION ALL scan entirely for intent-match turns.
Each component enters the `memory_snippets` section of the `InstructionBundle`
in the order the `BuildInstruction` step sequence specifies.

#### No-match path (unchanged structure, weight update needed)

When `resolve_intent` returns `NoMatch` or `DbLessFallback`, the existing
`fetch_for_consumer` UNION ALL path runs. The `doc_type_weight_by_class` table
in `retrieval_dbless.rs` needs updating to include new classes 22 and 23.

---

### 0.6 Current Turn Pipeline (Actual Code)

Reading `canonical.rs` and `recipe.rs`, the current turn pipeline is:

```
1. CheckpointStage     — cancel-check
2. BudgetStage         — token/iteration budget check
3. InputStage          — drain pending user input into LoopExecutionState
4. RecipeStage         — [STUB] intent/recipe lookup hook (always passes through today)
5. PromptStage         — assemble LLM prompt from history + prior_knowledge
6. InterceptorStage    — Sempai review of outgoing prompt (if connected)
7. ModelStage          — LLM call (Kohai)
8. ReplyAdmissionStage — validate/admit model response
9. AssistantReplyStage — emit response to user
10. CapabilityStage    — if response contains tool calls: execute tools, loop back
11. StopStage          — check for loop termination
12. ExitStage          — clean exit
```

**The critical gap:** `RecipeStage` (step 4) is currently a stub. It always
falls through to Tier 2 (full LLM). The `find_recipe` / `find_skills` calls
are not wired because `last_user_text` is not yet available in `LoopExecutionState`
at that pipeline position. (See `recipe.rs` module-level "structural debt" comment.)

**The prior_knowledge assembly** happens inside `PromptStage` (step 5) via the
`RetrievalSource` trait. `PostgresSource::fetch_for_turn` is the entry point:
it calls `resolve_intent` first, and if that returns a match, calls
`fetch_component_by_id` to get the specific component. Otherwise it falls back
to the full UNION ALL scan via `fetch_for_consumer`.

**Required pipeline change (Phase I in this plan):** Add `last_user_text` to
`LoopExecutionState` in `InputStage` so `RecipeStage` can call `find_recipe`
and route to Tier 0, Tier 1, or Tier 2 before `PromptStage` runs.

---

### 0.7 Normal Assembly — Restructuring the No-Match Path

When no intent matches, the RetrievalEngine falls back to `fetch_for_consumer`
(UNION ALL scan). This is the "big basic-prompt" path.

**Current behaviour:** keyword-scored UNION ALL across all 11+ component tables,
capped by token budget, ordered by `(class_code, prompt_uid)`.

**This path remains valid but needs a weight update** to include the new classes
(22 PythonCode, 23 ExtensionCatalogue) at appropriate priorities in
`retrieval_dbless.rs::doc_type_weight_by_class`. The ExtensionCatalogue's
`description` + `task_groups` summaries should rank near Skills/Recipes so the
LLM gets the domain overview without raw tool implementation details.

**The basic-prompt pre-computation** (§0.10) is a separate concern from the
per-turn assembly. See §0.10.

---

### 0.8 Actions — LLM-Bypass

Actions (class 16) already default to `override_prompt_creation = true` in
V029. Their `steps` JSONB encodes 13 step types (tool_call, conditional,
set_var, loop, return, evaluate, call_skill, try_catch, parallel, call_action,
spawn_subprocess, wait, emit_event).

When an Action is the matched component, the `BuildInstruction` (§0.5) carries
the flag `llm_call_required: false`. The orchestrator (Monty/default.py) reads
this flag and executes the Action steps directly without calling the LLM.
No Rust-side pipeline bypass is needed — the orchestrator owns that decision.

---

### 0.9 KV-Cache / LMCache-Aware Design

#### The basic-prompt — manually triggered, stored, reused

The **basic-prompt** is the pre-assembled `InstructionBundle` that contains all
validated components from the database in full: Skills, ToolSkills, Recipes,
PythonCode, ExtensionCatalogues, Specs, Lessons, etc. — the complete knowledge
base in `instruction_bundle.rs` priority order.

**Triggering:** Manually only, via an operator action (already implemented). The
assembly is expensive (full UNION ALL + format + fingerprint); it must not run
on every turn.

**Storage:** The assembled bundle (its `InstructionBundleFingerprint` + serialised
`messages` list) is stored in a new DB table `reborn_basic_prompt_store`
(one row per scope: tenant/user/agent/project). This is a new migration.

**Fallback:** If no basic-prompt is stored for the current scope, the
`InstructionBundleBuilder` falls back to the normal per-turn UNION ALL assembly
(the current behaviour). This ensures the system works before any manual trigger
has been run.

**Invalidation:** When a new component passes Gate 2 (validated), the stored
basic-prompt for the affected scope is marked `stale`. It stays available and
usable until the next manual trigger regenerates it. Stale prompts are not
deleted automatically — a stale cached prompt is always better than no prompt.

#### KV-cache mechanics

The `InstructionBundle` is already designed for KV-cache reuse:
`instruction_bundle.rs` places identity + skills + memory in the stable prefix
(PRIORITIES 1–3) and conversation history + inline messages in the volatile tail
(PRIORITIES 6–7). The prefix bytes stay identical across turns → the LLM KV-cache
or LMCache hits on everything up to the conversation start.

On **intent match**, the `BuildInstruction` patch is injected as `memory_snippets`
(PRIORITY 3) and `inline_messages` (PRIORITY 7) into the bundle:

- **`memory_snippets`**: the components the step needs (fetched by UUID) — replaces
  the broad UNION ALL. Placed in the stable prefix zone, after skills.
- **`inline_messages`**: the turn-specific instruction ("execute variant ls-la with
  dir=/tmp") — placed last, most volatile.

**KV-cache design rules for the patch:**
- Must NOT repeat any content already in the stored basic-prompt.
- Skill/ToolSkill bodies fetched for the orchestrator are placed in `memory_snippets`
  for the orchestrator's benefit but **do not appear in the LLM message sequence**
  (the LLM already has them from the cached basic-prompt).
- The patch may reference basic-prompt section headers as navigation hints:
  `→ see §ssh-connect-skill in basic-prompt`. These are inserted as inline
  annotations, not as full content.
- Target patch size: < 4k tokens (fits fast into new-token computation).

**The Interceptor** appends the stored basic-prompt bundle to the outgoing prompt
before it ships to the LLM (as PRIORITY 2 instruction snippets / PRIORITY 3
memory snippets from the stored bundle). The per-turn patch goes on top.

---

### 0.10 Extensions as Plugins — Translation Layer

All external plugins (MCP servers and other formats) are now called **Extensions**
(runtime Extensions, classes 4–9). They connect via a **translation layer only**.

The translation layer converts an incoming MCP description payload into
BrassClaw-native components in this order:

1. **Tools** (class 0) — one per MCP tool
2. **ToolSkills** (class 13) — one per Tool, encoding param schema, preconditions
3. **Skills** (class 1) — skeleton Skill per ToolSkill, describing orchestrator usage
4. **Recipes** — skeleton Recipes for common tool invocation patterns (intent-less at first)
5. **ExtensionCatalogue** (class 23) — one catalogue grouping all of the above

All generated components enter the validation queue at `pending` status and must
pass both Q1 (auto) and Q2 (user) gates before becoming active.

The reverse translation exports BrassClaw Skills + ToolSkills back to MCP
tool-descriptor format for MCP clients.

---

### 0.11 Interceptor System

The Interceptor saves each turn's prompt composition plan (the `BuildInstruction`
+ assembled patch, not the big basic-prompt). It adds the big basic-prompt at
the end of the final outgoing prompt.

If a **Sempai LLM** is connected, the Interceptor routes the assembled outgoing
prompt through Sempai for review before it ships to Kohai. Sempai can:
- Approve and forward as-is.
- Suggest a prompt rewrite.
- Flag the successful pattern for Recipe creation (queued in validation pipeline).

The Interceptor is also the foundation for **self-improvement and component
self-creation** — it saves the prompt-construction plans and organises the
creation of new Recipes and Skills from successful patterns, with Sempai's help.

> **Scope note:** The full self-improvement and component-self-creation pipeline
> is **out of scope** for this plan. It is marked as a future phase.

---

### 0.12 Validation System — Two-Gate Pipeline

**Gate 1 (Q1 — System, automatic):**
- Malignant pattern scan (injection vectors, shell escalation in PythonCode/Actions)
- Schema conformance: mandatory fields, length constraints, class-specific rules
- Policy compliance: tool references exist in capability surface, class codes valid
- Cross-reference check: skill names in Recipe steps resolve to known Skills
- S7 guard: Recipe steps that name a `tool` must also name a `skill`
- On pass → `auto_passed` → queued for Gate 2
- On fail → `auto_failed` → returned to author with error list

**Gate 2 (Q2 — User, manual):**
- Manual review and approval in the WebUI validation tab
- User may edit, re-submit for Q1, approve, or reject
- On approve → `validated` → enters production system
- On reject → `rejected` → 30-day re-review window → `garbage`

Q1 validator and component-creation wizard share the same validation functions.
No duplicate rule implementations.

---

## 1. Implementation Phases

### Phase A — PythonCode Component (class 22)

**Goal:** New component class for Python code/instruction elements targeted at the orchestrator.

**Files to create:**
- `crates/brassclaw_pg/migrations/V047__reborn_python_code.sql`  
  Same column shape as `V036__reborn_specs.sql`. `class_code = 22`.  
  Default consumer tags: `{02:orchestrator}` + `05:validator` until validated.

**Files to modify:**
- `crates/brassclaw_engine/src/memory/retrieval_source.rs` — add class 22 to UNION ALL and `fetch_component_by_id`
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `22 => "python_code"` to `class_label`
- `crates/brassclaw_engine/src/types/memory.rs` — add `DocType::PythonCode`; add `(22, "python_code")` mapping
- `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` — add class 22 weight (suggest `0.42`, between Skills and Extensions)
- `crates/brassclaw_engine/src/memory/component_validator.rs` — class 22 dispatch: name format + non-empty content + soft 10k token budget + no shell-injection pattern scan
- `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs` — new store (same pattern as `pg_recipe_store.rs`)

**Tests:**
- Unit: `class_label(22) == "python_code"`
- Unit: `doc_type_to_class_code(DocType::PythonCode).0 == 22`
- Integration: PythonCode row → retrieved via `fetch_for_consumer` with tag `02:orchestrator`

---

### Phase B — ExtensionCatalogue Component (class 23)

**Goal:** Documentation-container class that organises a capability domain.

**Files to create:**
- `crates/brassclaw_pg/migrations/V048__reborn_extension_catalogues.sql`  
  Columns: scope tuple + `name`, `description`, `version`, `overview_doc` (TEXT),  
  `task_groups` (JSONB: `[{group_name, summary, recipe_ids[]}]`),  
  `child_component_ids` (UUID[]), `intent_index` (JSONB, audit-only),  
  `prior_knowledge_content`, `override_prompt_creation`,  
  `class_code SMALLINT DEFAULT 23`, `prompt_uid`, `consumer_tags`,  
  `intent_examples` JSONB, full validation lifecycle + lineage columns, timestamps.

**Files to modify:**
- `crates/brassclaw_engine/src/memory/retrieval_source.rs` — add class 23 (content = `overview_doc`)
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `23 => "extension_catalogue"`
- `crates/brassclaw_engine/src/types/memory.rs` — add `DocType::ExtensionCatalogue`
- `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` — add class 23 weight (suggest `0.38`, near Recipes)
- `crates/brassclaw_engine/src/memory/component_validator.rs` — class 23: name format + non-empty overview_doc + at least one task_group entry + valid UUID syntax in child_component_ids
- `crates/brassclaw_reborn_composition/src/pg_extension_catalogue_store.rs` — new store

**Tests:**
- Unit: `class_label(23) == "extension_catalogue"`
- Integration: Catalogue with `task_groups` inserted → retrieved with overview in `effective_content`

---

### Phase C — Recipe Variants + BuildInstruction

**Goal:** Upgrade Recipe schema for intra-recipe variant paths and structured
assembly instructions.

#### C.1 New types in `crates/brassclaw_engine/src/types/recipe.rs`

```rust
/// Per-step prior-knowledge fetch specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepContextSpec {
    /// Exact component UUIDs to fetch via fetch_component_by_id.
    #[serde(default)]
    pub component_ids: Vec<String>,
    /// Class codes to include from UNION ALL (coarse filter).
    #[serde(default)]
    pub class_codes: Vec<i32>,
}

/// The owner/pipeline-stage of an atomic turn step.
/// Determines which part of the system reads and executes this step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepOwner {
    RetrievalEngine,   // fetch_components, no_match_union_all
    RecipeStage,       // skip_llm, assemble_llm_prompt decision
    PromptStage,       // assemble_llm_prompt
    Orchestrator,      // run_python_code, invoke_skill, conditional_branch, set_variable, loop_steps, call_action, emit_event
    RustLayer,         // read_tool_skill, call_tool
    ResultRelay,       // relay_result (Rust → Orchestrator boundary)
}

/// A single atomic turn step. Each step specifies exactly one action
/// at exactly one stage of the turn. Nothing is combined.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeStep {
    /// Unique step identifier within the BuildInstruction.
    pub step_id: String,
    /// Which pipeline stage or participant executes this step.
    pub owner: StepOwner,
    /// What this step does (one of the exhaustive step types).
    pub step_type: RecipeStepType,
    /// Human-readable description for authoring and debugging.
    pub description: String,
    /// UUID of the component this step operates on (Skill, ToolSkill, PythonCode, Tool).
    /// Empty string for steps that do not reference a stored component.
    #[serde(default)]
    pub component_id: String,
    /// Runtime parameter bindings (supports `{{vars.name}}` substitution).
    #[serde(default)]
    pub params: serde_json::Value,
    /// For conditional_branch: which step_id to jump to on true/false.
    #[serde(default)]
    pub branch_targets: Option<BranchTargets>,
    /// For skip_llm: fallback step_type when condition is false.
    #[serde(default)]
    pub else_step_type: Option<RecipeStepType>,
    /// Condition expression (for conditional_branch, skip_llm, loop_steps).
    #[serde(default)]
    pub condition: Option<String>,
    /// For loop_steps: the sub-sequence of step_ids to repeat.
    #[serde(default)]
    pub loop_body: Vec<String>,
}

/// Branch targets for conditional steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BranchTargets {
    pub on_true: String,   // step_id to execute when condition is true
    pub on_false: String,  // step_id to execute when condition is false
}

/// The exhaustive set of atomic step types in a Recipe turn script.
/// Each maps to exactly one owner (see StepOwner above).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeStepType {
    /// [RetrievalEngine] Fetch exact component UUIDs into the context window.
    FetchComponents,
    /// [PromptStage] Compose the LLM prompt from context window + conversation history.
    AssembleLlmPrompt,
    /// [RecipeStage] Skip the LLM call; orchestrator executes steps directly.
    SkipLlm,
    /// [Orchestrator] Execute a PythonCode component (fetched by component_id).
    RunPythonCode,
    /// [Orchestrator] Send a Skill's instructions (fetched by component_id) to Rust.
    InvokeSkill,
    /// [RustLayer] Read a ToolSkill component (fetched by component_id) for invocation params.
    ReadToolSkill,
    /// [RustLayer] Execute a registered Tool with bound parameters.
    CallTool,
    /// [ResultRelay] Return tool output from Rust layer back to the Orchestrator.
    RelayResult,
    /// [Orchestrator] Evaluate condition; select next step via branch_targets.
    ConditionalBranch,
    /// [Any] Bind a named variable to a value for downstream `{{vars.name}}` substitution.
    SetVariable,
    /// [Orchestrator] Repeat a sub-sequence of steps (loop_body) N times or until condition.
    LoopSteps,
    /// [Orchestrator] Invoke a nested Action component (fetched by component_id).
    CallAction,
    /// [Any] Emit a named event into the turn's event stream.
    EmitEvent,
}

/// The complete turn script stored in a RecipeVariant.
/// Drives the entire turn from intent match through to result delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BuildInstruction {
    /// Variable extraction patterns applied to the user prompt before execution.
    #[serde(default)]
    pub variable_patterns: Vec<VariablePattern>,
    /// Ordered atomic step sequence — the complete turn script.
    pub steps: Vec<RecipeStep>,
    /// Section headers in the stored basic-prompt the patch may reference
    /// (navigation hints only — content is not re-included).
    #[serde(default)]
    pub basic_prompt_section_refs: Vec<String>,
}

/// Variable extracted from the user prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariablePattern {
    pub name: String,
    /// Regex with named capture group matching the value in the user prompt.
    /// Example: r"of the (?P<dir>[/\w\-\.]+) directory"
    pub pattern: String,
    /// Fallback value when no capture matches.
    #[serde(default)]
    pub default_value: Option<String>,
}

/// One intent variant of a Recipe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeVariant {
    /// Unique key within this Recipe (e.g. "ls-l", "ls-la", "ls-other-dir").
    pub variant_key: String,
    /// Human-readable label.
    pub label: String,
    /// Intent expressions that route to this variant.
    pub intent_examples: Vec<String>,
    /// Assembly instruction for the RetrievalEngine.
    pub build_instruction: BuildInstruction,
}
```

Add `variants: Vec<RecipeVariant>` to `Recipe`. The existing `trigger` + `steps`
remain as the Tier-1 / Tier-2 fallback when no variant matches directly.

#### C.2 `variant_key` column on intent inputs

**File:** `crates/brassclaw_pg/migrations/V049__reborn_intent_inputs_variant_key.sql`
```sql
ALTER TABLE reborn_intent_inputs
    ADD COLUMN IF NOT EXISTS variant_key TEXT;
```

Update `IntentCandidate` and `IntentResolution::Match` to carry `variant_key: Option<String>`.  
Update `seed_intent_input` to accept and store `variant_key`.

#### C.3 RetrievalSource — `fetch_by_instruction`

**File:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`

Add to the `RetrievalSource` trait:
```rust
async fn fetch_by_instruction(
    &self,
    scope: &ComponentScope,
    instruction: &BuildInstruction,
    token_budget: usize,
) -> Result<Vec<ComponentItem>, RetrievalSourceError>;
```

`PostgresSource` implements this by iterating `instruction.component_ids` and
calling the existing `fetch_component_by_id` per ID, then applying variable
substitution from `instruction.variable_patterns` to each component's
`effective_content`. `RamSource` falls back to `fetch_for_consumer`.

#### C.4 Pipeline — wire `last_user_text` into RecipeStage

**File:** `crates/brassclaw_agent_loop/src/state.rs` (or equivalent)  
Add `last_user_text: Option<String>` to `LoopExecutionState`.

**File:** `crates/brassclaw_agent_loop/src/executor/input.rs`  
Populate `last_user_text` from the drained user input in `InputStage`.

**File:** `crates/brassclaw_agent_loop/src/executor/recipe.rs`  
In `RecipeStage::process`:
1. Read `state.last_user_text`.
2. Call `host.recipe_lookup().find_recipe(user_text)`.
3. If Tier 0 match: execute steps directly, skip PromptStage and ModelStage.
4. If Tier 1 match: inject ToolSkill summaries into a hint that PromptStage picks up.
5. If no match: fall through to Tier 2 (unchanged).

**Tests:**
- Unit: `RecipeVariant` + `BuildInstruction` serde roundtrips
- Unit: `seed_intent_input` with `variant_key` stores non-null column
- Integration: intent match → correct `variant_key` returned → correct variant steps used
- Integration: `fetch_by_instruction` fetches exactly the listed IDs

---

### Phase D — Skill Intent Wiring + `required_skills`

#### D.1 `intent_examples` on SkillManifest

**File:** `crates/brassclaw_skills/src/types.rs`  
Add to `SkillManifest`:
```rust
#[serde(default)]
pub intent_examples: Vec<String>,

/// Companion skills needed for tool-chain or pipe constructs.
/// Each is evaluated against the upcoming use-case at prior_knowledge build time.
/// Capped at MAX_REQUIRED_SKILLS_PER_MANIFEST (10).
#[serde(default)]
pub required_skills: Vec<String>,
```

**File:** `crates/brassclaw_skills/src/types.rs` — `ActivationCriteria::enforce_limits`  
Validate `intent_examples`: each entry ≤ 512 chars, total capped at 20.

#### D.2 Seed intents on skill import/validation

On skill `auto_passed` transition: call `seed_intent_input` for each
`intent_examples` entry with `variant_key = None`.

On skill wipe/delete: call `purge_component_inputs`.

#### D.3 Prior-knowledge builder — resolve `required_skills`

When a Skill is selected into a `BuildInstruction.component_ids`, also fetch
its `required_skills` list. Include all declared required Skills unconditionally
(the author is responsible for keeping the list short and relevant). This enables
tool-chain compositions like `grep-skill` referencing `pipe-skill`.

**Tests:**
- Unit: `SkillManifest` with `intent_examples` + `required_skills` YAML roundtrip
- Unit: `intent_examples` entry > 512 chars → rejected by `enforce_limits`
- Integration: Skill with `intent_examples` → resolves via `resolve_intent`
- Integration: Skill with `required_skills: ["pipe-skill"]` → both Skills fetched in BuildInstruction

---

### Phase E — ToolSkill-Mediation Guard (S7) + Skill Cross-References

**File:** `crates/brassclaw_engine/src/memory/recipe_validator.rs`

Q1 rule: if a `RecipeStep` with `step_type: SkillInvocation` references an empty
`component_ref`, error. If `step_type: ToolCall` appears without a preceding
`SkillInvocation` step in the same variant, error ("tools must be reached through a Skill").

Add to `validate_recipe`: check all steps in `recipe.variants[].build_instruction.steps`
in addition to top-level `recipe.steps`.

For Skill `required_skills` cross-reference: during Q1 validation, check that each
entry in `required_skills` resolves to a known Skill name in the component DB
(when the available_tools list is provided). Add a soft warning if the referenced
skill does not exist yet (it may be in the validation queue).

**Tests:**
- Unit: ToolCall step without preceding SkillInvocation → validation error
- Unit: SkillInvocation with empty `component_ref` → error
- Unit: variant step sequence with correct ordering → no error

---

### Phase F — MCP Translation Layer

**File:** `crates/brassclaw_extensions/src/mcp_translation.rs` (new)

```
translate_mcp_to_brassclaw(payload) → Vec<NewComponent>
  for each MCP tool:
    1. NewTool (class 0, no prompt text)
    2. NewToolSkill (class 13, param schema from MCP inputSchema)
    3. NewSkill (class 1, skeleton "how to invoke <tool> via Rust layer")
    4. skeleton Recipe (class 21, one variant per common pattern — pending)
  + one NewExtensionCatalogue (class 23) grouping all of the above
  → all inserted with validation_status = 'pending'

translate_brassclaw_to_mcp(skills) → McpDescriptionPayload
  for each validated Skill:
    → MCP tool descriptor { name, description, inputSchema }
```

**File:** `crates/brassclaw_extensions/src/lifecycle.rs`  
On MCP extension install: call `translate_mcp_to_brassclaw`, insert all
generated components, route to Q1 validation queue.

**Tests:**
- Unit: well-formed MCP payload → expected component count + types
- Unit: empty tool list → empty output, no panic
- Integration: MCP install → generated components appear in validation queue at `pending`

---

### Phase G — Q1 Validator Upgrades

**File:** `crates/brassclaw_engine/src/memory/component_validator.rs`

New dispatch cases:
- **Class 22 (PythonCode):** name format, non-empty content, soft 10k token budget, shell-injection scan
- **Class 23 (ExtensionCatalogue):** name format, non-empty `overview_doc`, ≥1 task_group, valid UUID syntax in `child_component_ids`, class codes in `intent_index` entries are known values (0–23, 50)
- **Recipe class 21 variants:** each variant must have non-empty `variant_key`, ≥1 `intent_examples` entry (each ≤ 512 chars), ≥1 step in `build_instruction.steps`; all steps pass S7 guard
- **Skill classes 1–3:** `intent_examples` entries ≤ 512 chars, capped at 20; `required_skills` capped at 10; no self-reference in `required_skills`
- **Action class 16:** validate `steps` JSONB structure against the 13 step types; `allowed_tools` entries exist in capability surface

Q1 validator and component-creation wizard share the same rule functions. No duplication.

**Tests:**
- Unit: each new class with missing mandatory field → specific error
- Unit: Recipe variant with `variant_key = ""` → error
- Unit: PythonCode with known injection pattern → error
- Unit: Skill `intent_examples` entry of 513 chars → rejected

---

### Phase H — Pipeline Integration (Tier 0 / Tier 1 activation)

**Goal:** Make RecipeStage actually dispatch instead of always passing through.

This is the phase that wires §0.6 (the current gap noted in the `recipe.rs` comment).

1. Add `last_user_text: Option<String>` to `LoopExecutionState` (populated by `InputStage`)
2. `RecipeStage` reads `last_user_text` and calls `find_recipe`
3. On Tier 0 match (validated + Wilson ≥ 0.70):
   - Call `fetch_by_instruction` with the matched variant's `BuildInstruction`
   - Execute steps directly via the orchestrator host
   - Skip `PromptStage` and `ModelStage`
   - Record outcome
4. On Tier 1 match:
   - Call `find_skills` for ToolSkill summaries
   - Store summaries as a hint in `LoopExecutionState` for `PromptStage` to inject
   - Continue through normal LLM path
5. On no match: fall through unchanged (Tier 2)

---

## 2. Migration Sequence

| Migration | Contents |
|-----------|----------|
| `V047__reborn_python_code.sql` | New table, class 22 |
| `V048__reborn_extension_catalogues.sql` | New table, class 23 |
| `V049__reborn_intent_inputs_variant_key.sql` | `ADD COLUMN variant_key TEXT` |
| `V050__reborn_skills_intent_examples.sql` | `ADD COLUMN intent_examples JSONB; ADD COLUMN required_skills JSONB` to `reborn_skills` (if not already present) |
| `V051__reborn_basic_prompt_store.sql` | New table: one row per scope (`tenant_id, user_id, agent_id, project_id`); columns: `id UUID PK`, scope tuple, `fingerprint TEXT` (SHA-256), `bundle_json JSONB` (serialised `InstructionBundle.messages`), `is_stale BOOLEAN DEFAULT false`, `assembled_at TIMESTAMPTZ`, `created_at`, `updated_at`. Unique constraint on scope tuple. |

All additive. No DROP, no renames. No existing rows break.

---

## 3. Open Questions — Decisions Needed Before Phase C

1. **Variable extraction:** Named capture groups in intent expressions (e.g.
   `r"of the (?P<dir>[/\w\-\.]+) directory"`) or a small post-match LLM
   extraction call?  
   → **Recommendation:** named capture groups in intent expressions for Phase C.
   LLM extraction as a follow-up fallback for complex prompts.

2. **Shared step prefix across variants:** Full independent step list per variant
   (simple, some duplication) or a shared prefix + divergence point (compact)?  
   → **Recommendation:** full independent list per variant. Shared prefix is a
   content convention, not enforced by schema. Keeps executor logic simple.

3. **`required_skills` inclusion threshold:** Always include all declared required
   skills, or score them against the current query first?  
   → **Recommendation:** always include if declared. Keep the list short by convention.

---

## 4. Out of Scope (Marked Postponed)

- Full self-improvement pipeline (Interceptor-driven Recipe auto-creation from successful patterns)
- Component self-creation wizard
- Automatic Sempai-driven prompt rewrites

---

## 5. Turn Flow Summary (for design review)

The following is a complete walkthrough of a single turn, from user prompt to
response, in the intended final state:

```
User types: "show all files including hidden in /tmp"
│
├─ [InputStage] Drains input into LoopExecutionState.
│   Sets last_user_text = "show all files including hidden in /tmp"
│
├─ [RecipeStage] Calls resolve_intent(query)
│   → Match: Recipe "listing-the-contents-of-a-directory"
│             variant_key = "ls-la"
│             variable_bindings = { dir: "/tmp" }
│   → Tier 0 check: wilson_lower = 0.82 → Tier 0 eligible
│
├─ [RetrievalEngine] fetch_by_instruction(build_instruction)
│   → Fetches: ssh-skill (UUID), ls-toolskill (UUID)
│   → Applies variable substitution: {{vars.dir}} → "/tmp"
│   → Produces prior_knowledge patch:
│       [ls-skill body]
│       [ls-toolskill body with /tmp substituted]
│       → base_prompt_ref: "§directory-listing" (pointer into cached basic-prompt)
│
├─ [Tier 0 path — no LLM call]
│   Orchestrator reads build_instruction.steps:
│     Step 1: prior_knowledge_fetch → already done
│     Step 2: skill_invocation → call ls-skill with { dir: "/tmp", flags: "-la" }
│     Step 3: tool_skill_read → Rust reads ls-toolskill
│     Step 4: tool_call → Rust executes ls /tmp -la
│     Step 5: result_relay → Rust returns output to orchestrator
│     Step 6: python_code → format output for user
│
├─ [InterceptorStage] Saves prompt composition plan (patch only, not basic-prompt).
│   If Sempai connected: reviews execution plan before it runs.
│
└─ [AssistantReplyStage] Emits formatted directory listing to user.
   RecipeStage records outcome (success) → Wilson score updated.
```

**No-match path (Tier 2):**
```
User types: "explain recursion to me"
│
├─ [InputStage] last_user_text set
├─ [RecipeStage] resolve_intent → NoMatch → falls through
├─ [PromptStage] PromptStage runs normally.
│   RetrievalEngine.fetch_for_consumer() → UNION ALL scan,
│   token-budget-capped, keyword-scored.
│   Big basic-prompt (from KV-cache) + small UNION ALL results assembled.
├─ [InterceptorStage] Sempai reviews if connected.
├─ [ModelStage] Full LLM call with assembled prompt.
├─ [AssistantReplyStage] Emits LLM response.
└─ (No Recipe outcome recorded — no Recipe matched.)
```

