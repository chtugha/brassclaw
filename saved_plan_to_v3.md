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

---

### 0.3 Recipe — Correct Design

A Recipe is a **complete turn script**. It describes, step by step, which part
of the entire turn pipeline does what, exactly when, using which component
(fetched by UUID). It is the primary intent target.

**Important — current vs. target state:**  
The live `Recipe` struct in `crates/brassclaw_engine/src/types/recipe.rs` is the
**v2 design**: `RecipeStep` holds `skill: String` (name reference) + `tool: String`
(denormalized). There is no `RecipeVariant`, `BuildInstruction`, or `StepOwner`.
Phase C replaces this with the v3 design described below. The existing
`trigger` + `steps` fields are **preserved** as the Tier-1 / Tier-2 fallback
so old Recipes continue to work without migration.

#### Mandatory shape

| Field | Content |
|-------|---------|
| `name` | Recipe identifier (e.g. `listing-the-contents-of-a-directory`) |
| `description` | One-sentence summary of what this Recipe accomplishes |
| `category` | Task group (maps to `ExtensionCatalogue.task_groups[].group_name`) |
| `variants[]` | One or more `RecipeVariant` entries; each fully specifies one turn |
| `trigger` / `steps` | **Kept** — v2 fallback path (Tier-1 keyword match → LLM hint injection) |

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
The `variant_key` column (added in V049) selects which `BuildInstruction` to execute.

---

### 0.4 BuildInstruction — Dual-Audience Design

> **Key design constraint:** A `BuildInstruction` serves **three distinct readers**,
> and the format must make this split explicit and type-safe.

#### Three readers, three typed sections

**Reader 1 — RetrievalEngine** (flat steps with owner `RetrievalEngine`):  
Consumed by `fetch_by_instruction` (§0.6). Reads only `FetchComponents` steps to decide
what to load into the context window. Does not touch the orchestrator or Rust contexts.

**Reader 2 — Orchestrator** (`orchestrator_context: OrchestratorContext`):  
The orchestrator (Monty/default.py) reads a **typed struct** containing:
- `skill_ids[]` — exact Skill component UUIDs to fetch
- `python_code_ids[]` — exact PythonCode component UUIDs to fetch
- `step_formatter_id` — PythonCode UUID that reformats step descriptions into LLM-optimal instructions
- `control_flow_steps[]` — orchestrator-only steps (`ConditionalBranch`, `SetVariable`, `LoopSteps`, `CallAction`, `EmitEvent`)

The orchestrator **does not** read ToolSkill bodies or Tool names — those are opaque at the orchestrator tier.

**Reader 3 — RustLayer** (`rust_context: RustContext`):  
The Rust execution layer reads a **compact JSON package** containing:
- `tool_skill_ids[]` — exact ToolSkill component UUIDs
- `tool_bindings[]` — array of `{ tool_name, params, error_policy }` (§0.4.1 below)

The Rust layer **does not** receive the LLM prompt. Its package is separate and minimal.

> **Why this split matters:**  
> The orchestrator prompt must be KV-cache friendly (< 4k tokens), while the Rust layer
> needs precise, schema-validated tool invocation data. Mixing them inflates prompt size
> and couples unrelated concerns. Step descriptions serve **dual purpose**: human-readable
> documentation **and** raw material for `step_formatter_id` to reformat into LLM-optimal
> orchestrator instructions. The formatter is a PythonCode component that adapts verbosity,
> tone, and ordering to suit the orchestrator's reasoning style without changing the
> underlying logic.

#### 0.4.1 ToolBinding + ErrorPolicy

Each tool invocation in `RustContext` is wrapped in a `ToolBinding`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolBinding {
    pub tool_name: String,
    pub params: serde_json::Value,  // {{vars.name}} substitution applied
    pub error_policy: ErrorPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    Fail,   // Halt on error (default for most tools)
    Ignore, // Continue even if tool fails
    Retry { max_attempts: u32 },
    Fallback { step_id: String },  // Jump to fallback step on error
}
```

This gives the Rust layer explicit error-handling instructions without requiring
orchestrator intervention for retries or fallbacks.

#### Structure

```
BuildInstruction
├── llm_call_required: bool              ← false for Actions/Tier0, true for Tier1+
├── variable_patterns[]                  ← applied before any step executes
├── basic_prompt_section_refs[]          ← navigation hints into cached basic-prompt
├── fetch_steps[]                        ← Section A: RetrievalEngine-only
│   └── [owner: RetrievalEngine]  FetchComponents  (with StepContextSpec)
├── orchestrator_context: OrchestratorContext  ← Section B: typed struct
│   ├── skill_ids[]                      ← UUIDs → fetch_component_by_id
│   ├── python_code_ids[]                ← UUIDs → fetch_component_by_id
│   ├── step_formatter_id                ← UUID of PythonCode that reformats step descriptions
│   └── control_flow_steps[]             ← ConditionalBranch, SetVariable, LoopSteps, etc.
└── rust_context: RustContext            ← Section C: compact JSON for Rust layer
    ├── tool_skill_ids[]                 ← UUIDs → fetch_component_by_id
    └── tool_bindings[]                  ← ToolBinding[] with ErrorPolicy per invocation
```

The three sections **must not overlap**:
- `fetch_steps` are read by `fetch_by_instruction` only (RetrievalEngine).
- `orchestrator_context` is serialized into the LLM prompt **after** the RetrievalEngine returns the patch.
- `rust_context` is serialized as a separate JSON payload and sent directly to the Rust layer; the orchestrator never sees it.

#### Complete example — SSH connection

```
BuildInstruction for variant: ssh-connect

llm_call_required: false   ← Tier 0: skip LLM, execute directly
variable_patterns:
  - { name: "host", pattern: r"(?:to|into) (?P<host>[\w.\-]+)" }
basic_prompt_section_refs: ["§ssh-connect-skill"]

# ── Section A: RetrievalEngine ────────────────────────────────────────────
fetch_steps:
  - step_id: "fetch-ssh"
    owner: RetrievalEngine
    step_type: FetchComponents
    context_spec:
      component_ids: ["<uuid:ssh-connect-skill>", "<uuid:ssh-toolskill>",
                      "<uuid:ssh-result-handler>", "<uuid:ssh-preflight-code>"]
    description: "Load SSH skill, ToolSkill, result handler, and preflight code"

# ── Section B: Orchestrator ───────────────────────────────────────────────
orchestrator_context:
  skill_ids: ["<uuid:ssh-connect-skill>"]
  python_code_ids: ["<uuid:ssh-pre-invocation-check>", "<uuid:ssh-result-handler>"]
  step_formatter_id: "<uuid:terse-cli-formatter>"   ← reformats for CLI-style brevity
  control_flow_steps:
    - step_id: "preflight"
      step_type: RunPythonCode
      component_id: "<uuid:ssh-pre-invocation-check>"
      params: { host: "{{vars.host}}" }
      description: "Verify known_hosts entry and key availability"
    - step_id: "format"
      step_type: RunPythonCode
      component_id: "<uuid:ssh-result-handler>"
      description: "Format stdout/stderr for user; check exit code"

# ── Section C: RustLayer ──────────────────────────────────────────────────
rust_context:
  tool_skill_ids: ["<uuid:ssh-toolskill>"]
  tool_bindings:
    - tool_name: "ssh"
      params: { host: "{{vars.host}}", timeout_secs: 10 }
      error_policy: { policy: "fail" }
```

The Retrieval Engine reads `fetch_steps` only. The orchestrator receives the typed
`OrchestratorContext`. The Rust layer receives the compact `RustContext` JSON.

---

### 0.5 StepContextSpec — Per-Step Context Narrowing

`StepContextSpec` is the type attached to each `FetchComponents` step that tells
the Retrieval Engine precisely what to load for that step. It exists as a typed
sub-field of `FetchComponentsStep` rather than loose `params`, so callers get
compile-time guarantees.

```rust
/// Per-step prior-knowledge fetch specification.
/// Attached to FetchComponentsStep (step_type == FetchComponents).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepContextSpec {
    /// Exact component UUIDs to fetch via fetch_component_by_id.
    /// Empty means "no exact IDs required for this step."
    #[serde(default)]
    pub component_ids: Vec<String>,
    /// Class codes to additionally pull from UNION ALL (coarse class filter).
    /// Usually empty when component_ids fully specifies what is needed.
    #[serde(default)]
    pub class_codes: Vec<i32>,
}
```

`FetchComponentsStep.context_spec: StepContextSpec` — always present on
`FetchComponents` steps. `fetch_by_instruction` reads this field to build its
fetch list.

---

### 0.6 The RetrievalEngine and `fetch_by_instruction`

#### Current state (grounded in code)

`RetrievalSource` (in `retrieval_source.rs`) currently has two methods:
- `fetch_for_consumer` — keyword-scored UNION ALL, consumer-tag filtered
- `fetch_for_turn` — intent-resolution then `fetch_component_by_id`, falls back to `fetch_for_consumer`

`fetch_for_turn` returns a **single matched component** when intent resolves. It does
not execute a `BuildInstruction`; that is entirely new in v3.

`fetch_component_by_id` already handles classes 1–3, 4–9, 12–21 (plus 0 returns None).
Classes 22 (PythonCode) and 23 (ExtensionCatalogue) must be added in Phases A and B.

#### New method: `fetch_by_instruction`

Added to the `RetrievalSource` trait as a default method:

```rust
/// Execute a BuildInstruction's fetch plan and return an ordered
/// prior_knowledge patch ready for injection as memory_snippets.
///
/// Only FetchComponents steps (fetch_steps[]) are processed.
/// Variable substitution is applied to each component's effective_content
/// using the instruction's variable_patterns matched against user_text.
/// The RamSource default implementation falls back to fetch_for_consumer.
async fn fetch_by_instruction(
    &self,
    scope: &ComponentScope,
    instruction: &BuildInstruction,
    user_text: &str,
    token_budget: usize,
) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
    // Default: fall back to fetch_for_consumer (RamSource / test path)
    self.fetch_for_consumer(scope, user_text, token_budget, "02").await
}
```

`PostgresSource` overrides this:
1. Extract variables from `user_text` using `instruction.variable_patterns` → `HashMap<name, value>`.
2. Iterate `instruction.fetch_steps`; collect all `FetchComponents` steps.
3. For each step's `context_spec.component_ids`: call `fetch_component_by_id(uuid)`.
4. For each step's `context_spec.class_codes`: run a narrow UNION ALL (class filter only).
5. Apply `{{vars.name}}` substitution to each `ComponentItem.effective_content`.
6. Return assembled `Vec<ComponentItem>` in fetch-step order, capped to `token_budget`.

`RamSource` uses the default (falls back to `fetch_for_consumer`) — acceptable for tests.

#### Updated `fetch_for_turn` flow

```
fetch_for_turn(scope, query, budget, sender_class)
  → resolve_intent(scope, query)
      → Match { component_id, component_class_code, variant_key }
          → fetch_component_by_id(component_id, class=21)   ← fetch the Recipe
          → recipe.variants.find(variant_key)
          → fetch_by_instruction(scope, &variant.build_instruction, query, budget)
          → return FetchForTurnResult::Components(patch)
      → Disambiguation { candidates }
          → return FetchForTurnResult::Disambiguation(candidates)
      → NoMatch | DbLessFallback
          → fetch_for_consumer(...)
          → return FetchForTurnResult::Components(broad_scan)
```

`IntentResolution::Match` must carry `variant_key: Option<String>` (added in Phase C).

---

### 0.7 Current Turn Pipeline (Actual Code)

Reading `canonical.rs` and `recipe.rs`, the current turn pipeline is:

```
1.  CheckpointStage     — cancel-check
2.  BudgetStage         — token/iteration budget check
3.  InputStage          — drain pending user input into LoopExecutionState
4.  RecipeStage         — [STUB] intent/recipe lookup hook (always falls through today)
5.  PromptStage         — assemble LLM prompt from history + prior_knowledge
6.  InterceptorStage    — Sempai review of outgoing prompt (if connected)
7.  ModelStage          — LLM call (Kohai)
8.  ReplyAdmissionStage — validate/admit model response
9.  AssistantReplyStage — emit response to user
10. CapabilityStage     — if response contains tool calls: execute, loop back
11. StopStage           — check for loop termination
12. ExitStage           — clean exit
```

**The critical gap:** `RecipeStage` (step 4) always falls through to Tier 2.
The `recipe.rs` module-level "structural debt" comment documents two resolution
paths; option 1 (add `last_user_text` to `LoopExecutionState`) is the chosen approach.

`LoopExecutionState` (in `state.rs`) currently has no `last_user_text` field.
It must be added and populated in `InputStage` so `RecipeStage` can read it.

**Phase H** (§1.8 below) wires this.

---

### 0.8 Normal Assembly — No-Match Path

When `resolve_intent` returns `NoMatch` or `DbLessFallback`, `fetch_for_consumer`
runs (keyword-scored UNION ALL). This is the "big basic-prompt" path.

**Current `doc_type_weight_by_class` weights (retrieval_dbless.rs):**

| Class | Label | Weight |
|-------|-------|--------|
| 50 | Scaffold | 0.55 |
| 10 | Orchestrator | 0.52 |
| 12 | Spec | 0.50 |
| 0 | Tool | 0.50 |
| 1–3 | Skills | 0.45 |
| 4–9 | Extensions | 0.42 |
| 13 | ToolSkill | 0.40 |
| 18 | Lesson | 0.40 |
| 21 | Recipe | 0.38 |
| 16 | Action | 0.35 |
| 14 | Plan | 0.30 |
| 17 | Docu | 0.25 |
| 19 | Issue | 0.20 |
| 15 | Summary | 0.10 |
| 20 | Note | 0.05 |

**Additions needed for v3:**

| Class | Label | Suggested weight | Rationale |
|-------|-------|-----------------|-----------|
| 22 | PythonCode | 0.42 | Peer of Extensions — orchestrator utility code |
| 23 | ExtensionCatalogue | 0.38 | Peer of Recipes — domain overview, not tool implementation |

---

### 0.9 Actions — LLM-Bypass

Actions (class 16) already default to `override_prompt_creation = true` in V029.
Their `steps` JSONB encodes 13 step types. When an Action is the matched component,
its `BuildInstruction` has `llm_call_required: false`. The orchestrator reads
this flag and executes the Action steps directly without calling the LLM.

---

### 0.10 KV-Cache / LMCache-Aware Design

#### Basic-prompt

The **basic-prompt** is the pre-assembled `InstructionBundle` containing all
validated components. Stored in `reborn_basic_prompt_store` (one row per scope).

- **Triggering:** Manual only (operator action).
- **Fallback:** If no stored bundle exists, normal per-turn UNION ALL runs.
- **Invalidation:** When a component passes Gate 2, the stored bundle is marked `stale`.
  Stale prompts remain usable; they are never auto-deleted.

#### KV-cache rules for the BuildInstruction patch

- The patch **must not repeat** any content already in the stored basic-prompt.
- Skill/ToolSkill bodies fetched for the orchestrator go into `memory_snippets`
  (for orchestrator benefit) but **are not re-sent in the LLM message sequence**.
- The patch may reference basic-prompt section headers as navigation hints:
  `→ see §ssh-connect-skill` (the `basic_prompt_section_refs` field).
- Target patch size: < 4 k tokens.

---

### 0.11 Extensions as Plugins — Translation Layer

All external plugins (MCP servers and other formats) connect via a **translation
layer only**. The translation layer converts an incoming MCP description payload
into BrassClaw-native components:

1. **Tools** (class 0) — one per MCP tool
2. **ToolSkills** (class 13) — one per Tool
3. **Skills** (class 1) — skeleton Skill per ToolSkill
4. **Recipes** — skeleton Recipes for common invocation patterns (intent-less at first)
5. **ExtensionCatalogue** (class 23) — one catalogue grouping all of the above

All generated components enter the validation queue at `pending` and must pass
Q1 + Q2 before becoming active.

---

### 0.12 Interceptor System

The Interceptor saves each turn's prompt composition plan (the `BuildInstruction` +
assembled patch, not the big basic-prompt). If a Sempai LLM is connected, the
assembled outgoing prompt is routed through Sempai for review before shipping to
Kohai. Sempai can approve, suggest a rewrite, or flag the pattern for Recipe creation.

> **Scope note:** Full self-improvement and component-self-creation pipeline is
> **out of scope** for this plan.

---

### 0.13 Validation System — Two-Gate Pipeline

**Gate 1 (Q1 — automatic):**
- Malignant pattern scan (injection vectors, shell escalation in PythonCode/Actions)
- Schema conformance: mandatory fields, length constraints, class-specific rules
- Policy compliance: tool references exist in capability surface, class codes valid
- Cross-reference check: Skill names in Recipe steps resolve to known Skills
- S7 guard: Recipe steps that name a `tool` must also name a `skill`
- On pass → `auto_passed` → queued for Gate 2
- On fail → `auto_failed` → returned to author with error list

**Gate 2 (Q2 — manual):**
- Manual review in WebUI validation tab
- User may edit, re-submit for Q1, approve, or reject
- On approve → `validated` → enters production
- On reject → `rejected` → 30-day window → `garbage`

Q1 validator and component-creation wizard share the same validation functions.

---

## 1. Implementation Phases

### Phase A — PythonCode Component (class 22)

**Status:** [ ] Pending

**Goal:** New component class for Python code/instruction elements targeted at the orchestrator.

**Files to create:**
- `crates/brassclaw_pg/migrations/V047__reborn_python_code.sql`  
  Same column shape as `V036__reborn_specs.sql`. `class_code = 22`.  
  Default consumer tags: `{02:orchestrator}` + `05:validator` until validated.

**Files to modify:**
- `crates/brassclaw_engine/src/memory/retrieval_source.rs`  
  — add class 22 to UNION ALL in `fetch_for_consumer` (content expr: `COALESCE(NULLIF(prior_knowledge_content,''), body)`)  
  — add `22 => ("reborn_python_code", "COALESCE(NULLIF(prior_knowledge_content,''), body)")` to `fetch_component_by_id` match arm
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `22 => "python_code"` to `class_label`
- `crates/brassclaw_engine/src/types/memory.rs` — add `DocType::PythonCode`; add `(22, "python_code")` mapping
- `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` — add `22 => 0.42` to weight map
- `crates/brassclaw_engine/src/memory/component_validator.rs` — class 22: name format + non-empty content + soft 10k token budget + shell-injection scan
- `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs` — new store (same pattern as `pg_recipe_store.rs`)

**Tests:**
- Unit: `class_label(22) == "python_code"`
- Unit: `doc_type_to_class_code(DocType::PythonCode).0 == 22`
- Integration: PythonCode row → retrieved via `fetch_for_consumer` with tag `02:orchestrator`

---

### Phase B — ExtensionCatalogue Component (class 23)

**Status:** [ ] Pending

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
- `crates/brassclaw_engine/src/memory/retrieval_source.rs`  
  — add class 23 to UNION ALL (content = `overview_doc`)  
  — add `23 => ("reborn_extension_catalogues", "overview_doc")` to `fetch_component_by_id`
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `23 => "extension_catalogue"`
- `crates/brassclaw_engine/src/types/memory.rs` — add `DocType::ExtensionCatalogue`
- `crates/brassclaw_engine/src/memory/retrieval_dbless.rs` — add `23 => 0.38`
- `crates/brassclaw_engine/src/memory/component_validator.rs`  
  — class 23: name format + non-empty `overview_doc` + ≥1 `task_groups` entry + valid UUID syntax in `child_component_ids`
- `crates/brassclaw_reborn_composition/src/pg_extension_catalogue_store.rs` — new store

**Tests:**
- Unit: `class_label(23) == "extension_catalogue"`
- Integration: Catalogue with `task_groups` → retrieved with `overview_doc` as `effective_content`

---

### Phase C — Recipe v3: Variants + BuildInstruction

**Status:** [ ] Pending

**Goal:** Upgrade the `Recipe` struct (currently v2 — `RecipeStep { skill, tool, params, description }`)
to the v3 variant model with structured `BuildInstruction`. The existing `trigger` + `steps`
fields are **preserved** for backwards compatibility; they continue to serve the Tier-1 path.

#### C.1 New types in `crates/brassclaw_engine/src/types/recipe.rs`

```rust
/// Per-step context-fetch specification (present on FetchComponents steps).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StepContextSpec {
    #[serde(default)]
    pub component_ids: Vec<String>,  // exact UUIDs → fetch_component_by_id
    #[serde(default)]
    pub class_codes: Vec<i32>,       // coarse UNION ALL class filter
}

/// A single fetch step read by the RetrievalEngine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchComponentsStep {
    pub step_id: String,
    pub context_spec: StepContextSpec,
    pub description: String,
}

/// Orchestrator-only control-flow step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlFlowStep {
    pub step_id: String,
    pub step_type: ControlFlowStepType,
    pub description: String,
    #[serde(default)]
    pub component_id: String,        // UUID for RunPythonCode, CallAction
    #[serde(default)]
    pub params: serde_json::Value,   // {{vars.name}} substitution supported
    #[serde(default)]
    pub condition: Option<String>,   // for ConditionalBranch, LoopSteps
    #[serde(default)]
    pub branch_targets: Option<BranchTargets>,
    #[serde(default)]
    pub loop_body: Vec<String>,      // step_ids to repeat
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
    pub on_true: String,   // step_id
    pub on_false: String,  // step_id
}

/// Typed context for the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OrchestratorContext {
    #[serde(default)]
    pub skill_ids: Vec<String>,       // Skill component UUIDs
    #[serde(default)]
    pub python_code_ids: Vec<String>, // PythonCode component UUIDs
    #[serde(default)]
    pub step_formatter_id: Option<String>,  // PythonCode UUID that reformats step descriptions
    #[serde(default)]
    pub control_flow_steps: Vec<ControlFlowStep>,
}

/// Error policy for tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    Fail,   // Halt on error (default)
    Ignore, // Continue even if tool fails
    Retry { max_attempts: u32 },
    Fallback { step_id: String },  // Jump to fallback step on error
}

impl Default for ErrorPolicy {
    fn default() -> Self {
        ErrorPolicy::Fail
    }
}

/// Single tool invocation with error policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolBinding {
    pub tool_name: String,
    pub params: serde_json::Value,  // {{vars.name}} substitution applied
    #[serde(default)]
    pub error_policy: ErrorPolicy,
}

/// Typed context for the Rust execution layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RustContext {
    #[serde(default)]
    pub tool_skill_ids: Vec<String>,  // ToolSkill component UUIDs
    #[serde(default)]
    pub tool_bindings: Vec<ToolBinding>,
}

/// Variable extracted from the user prompt via named-capture regex.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariablePattern {
    pub name: String,
    /// Regex with named capture group. Example: r"of the (?P<dir>[/\w\-\.]+)"
    pub pattern: String,
    #[serde(default)]
    pub default_value: Option<String>,
}

/// Complete turn script stored in a RecipeVariant.
/// Three typed sections: fetch_steps (RetrievalEngine), orchestrator_context, rust_context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BuildInstruction {
    /// False for Actions/Tier0; true for Tier1+.
    #[serde(default)]
    pub llm_call_required: bool,
    #[serde(default)]
    pub variable_patterns: Vec<VariablePattern>,
    /// Fetch steps — read by RetrievalEngine only.
    #[serde(default)]
    pub fetch_steps: Vec<FetchComponentsStep>,
    /// Navigation hints into the cached basic-prompt (content NOT re-included).
    #[serde(default)]
    pub basic_prompt_section_refs: Vec<String>,
    /// Orchestrator-specific typed context.
    #[serde(default)]
    pub orchestrator_context: OrchestratorContext,
    /// Rust-layer-specific typed context (separate JSON payload).
    #[serde(default)]
    pub rust_context: RustContext,
}

/// One intent variant of a Recipe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeVariant {
    pub variant_key: String,         // e.g. "ls-l", "ls-la", "ls-other-dir"
    pub label: String,
    pub intent_examples: Vec<String>,
    pub build_instruction: BuildInstruction,
}
```

Add to the existing `Recipe` struct:
```rust
/// v3 variant paths. When non-empty, intent resolution uses variant_key
/// to select the BuildInstruction. When empty, falls back to trigger + steps (v2).
#[serde(default)]
pub variants: Vec<RecipeVariant>,
```

**Note:** The v2 `RecipeStep` is not removed — it backs the existing `trigger` + `steps` fallback.

#### C.2 `variant_key` column on intent inputs

**File:** `crates/brassclaw_pg/migrations/V049__reborn_intent_inputs_variant_key.sql`
```sql
ALTER TABLE reborn_intent_inputs
    ADD COLUMN IF NOT EXISTS variant_key TEXT;
```

Update `IntentCandidate` and `IntentResolution::Match` to carry
`variant_key: Option<String>`.  
Update `seed_intent_input` to accept and store `variant_key`.

#### C.3 `fetch_by_instruction` on `RetrievalSource`

See §0.6 above for the full signature and semantics.

`PostgresSource` implementation:
1. Apply `instruction.variable_patterns` to `user_text` → `HashMap<name, value>`.
2. Collect all `FetchComponents` steps from `instruction.fetch_steps`.
3. For each step: call `fetch_component_by_id` for every UUID in `context_spec.component_ids`.
4. Apply `{{vars.name}}` substitution to each `ComponentItem.effective_content`.
5. Return assembled `Vec<ComponentItem>` in fetch-step order, capped to `token_budget`.

#### C.4 Variants seeded into `reborn_intent_inputs`

On Recipe `auto_passed` transition: call `seed_intent_input` for each
`(variant.intent_examples[i], variant.variant_key)` pair.

On Recipe delete/wipe: call `purge_component_inputs(component_id)`.

**Tests:**
- Unit: `RecipeVariant` + `BuildInstruction` serde roundtrips
- Unit: `FetchComponentsStep` with `context_spec` roundtrips
- Unit: `OrchestratorContext` + `RustContext` + `ToolBinding` + `ErrorPolicy` roundtrips
- Unit: `seed_intent_input` with `variant_key = Some("ls-la")` stores non-null column
- Integration: intent match → correct `variant_key` returned → correct variant steps used
- Integration: `fetch_by_instruction` fetches exactly the listed UUIDs, in fetch-step order
- Integration: `{{vars.dir}}` substitution applied correctly in `effective_content`
- Integration: Rust layer receives compact `RustContext` JSON, not LLM prompt

---

### Phase D — Skill Intent Wiring + `required_skills`

**Status:** [ ] Pending

#### D.1 `intent_examples` + `required_skills` on SkillManifest

**File:** `crates/brassclaw_skills/src/types.rs`  
Add to `SkillManifest`:
```rust
/// Intent expressions that should trigger this skill via the intent system.
/// Each entry <= 512 chars, capped at 20 total.
#[serde(default)]
pub intent_examples: Vec<String>,

/// Companion skills required for tool-chain or pipe constructs.
/// Always included unconditionally when this skill is fetched in a BuildInstruction.
/// Capped at 10 (author responsibility to keep the list relevant).
#[serde(default)]
pub required_skills: Vec<String>,
```

Add to `ActivationCriteria::enforce_limits`:
- `intent_examples`: each entry <= 512 chars, total capped at 20.
- `required_skills`: total capped at 10, no self-reference.

#### D.2 Seed intents on skill import/validation

On skill `auto_passed` transition: call `seed_intent_input` for each
`intent_examples` entry with `variant_key = None`.

On skill wipe/delete: call `purge_component_inputs`.

#### D.3 `required_skills` resolution in prior-knowledge builder

When a Skill is selected into a `BuildInstruction` (via `orchestrator_context.skill_ids`),
also fetch its `required_skills` list. Include all declared required Skills unconditionally.
This enables tool-chain compositions like `grep-skill` referencing `pipe-skill`.

**Tests:**
- Unit: `SkillManifest` with `intent_examples` + `required_skills` YAML roundtrip
- Unit: `intent_examples` entry of 513 chars → rejected by `enforce_limits`
- Integration: Skill with `intent_examples` → resolves via `resolve_intent`
- Integration: Skill with `required_skills: ["pipe-skill"]` → both Skills fetched in patch

---

### Phase E — ToolSkill-Mediation Guard (S7) + Skill Cross-References

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/recipe_validator.rs`

Q1 rule S7: if `rust_context.tool_bindings[]` is non-empty but
`orchestrator_context.skill_ids[]` is empty, error ("tools must be reached through a Skill").

Q1 rule: `ControlFlowStep` with `step_type: RunPythonCode` and empty `component_id` → error.

Add to `validate_recipe`: check `recipe.variants[].build_instruction` in addition to
top-level `recipe.steps`.

For `required_skills` cross-reference: during Q1, check each entry resolves to a
known Skill name in the component DB. Emit soft warning (not hard error) if the
referenced Skill is not yet in the DB (may be queued).

**Tests:**
- Unit: non-empty `tool_bindings` + empty `skill_ids` → validation error
- Unit: `RunPythonCode` with empty `component_id` → error
- Unit: correct pairing → no error

---

### Phase F — MCP Translation Layer

**Status:** [ ] Pending

**File:** `crates/brassclaw_extensions/src/mcp_translation.rs` (new)

```
translate_mcp_to_brassclaw(payload) -> Vec<NewComponent>
  for each MCP tool:
    1. NewTool (class 0, no prompt text)
    2. NewToolSkill (class 13, param schema from MCP inputSchema)
    3. NewSkill (class 1, skeleton "how to invoke <tool> via Rust layer")
    4. skeleton Recipe (class 21, one variant per common pattern — pending)
  + one NewExtensionCatalogue (class 23) grouping all of the above
  -> all inserted with validation_status = 'pending'

translate_brassclaw_to_mcp(skills) -> McpDescriptionPayload
  for each validated Skill:
    -> MCP tool descriptor { name, description, inputSchema }
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

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/component_validator.rs`

New dispatch cases (added to `validate_by_class` match):

| Class | Rules |
|-------|-------|
| 22 PythonCode | name format, non-empty content, soft 10k token budget, shell-injection scan |
| 23 ExtensionCatalogue | name format, non-empty `overview_doc`, >=1 `task_groups`, valid UUID syntax in `child_component_ids`, class codes in `intent_index` entries are 0–23 or 50 |
| 21 Recipe (variants) | each variant: non-empty `variant_key`, >=1 `intent_examples` (each <=512 chars), >=1 fetch step or orchestrator step; S7 guard (non-empty `tool_bindings` requires non-empty `skill_ids`) |
| 1–3 Skills | `intent_examples` entries <=512 chars, capped at 20; `required_skills` capped at 10; no self-reference |
| 16 Actions | validate `steps` JSONB against 13 step types; `allowed_tools` entries exist in capability surface |

Q1 validator and component-creation wizard share the same rule functions. No duplication.

**Tests:**
- Unit: each new class with missing mandatory field → specific error
- Unit: Recipe variant with `variant_key = ""` → error
- Unit: PythonCode with known injection pattern → error
- Unit: Skill `intent_examples` entry of 513 chars → rejected
- Unit: Catalogue with zero `task_groups` → error
- Unit: S7 guard: non-empty `tool_bindings` + empty `skill_ids` → error

---

### Phase H — Pipeline Integration (Tier 0 / Tier 1 Activation)

**Status:** [ ] Pending

**Goal:** Make `RecipeStage` actually dispatch instead of always passing through.
This closes the gap documented in the `recipe.rs` "structural debt" comment.

**Files to modify:**

1. `crates/brassclaw_agent_loop/src/state.rs`  
   Add to `LoopExecutionState`:
   ```rust
   /// Populated by InputStage; read by RecipeStage for intent resolution.
   pub last_user_text: Option<String>,
   ```

2. `crates/brassclaw_agent_loop/src/executor/input.rs`  
   After draining user input: `state.last_user_text = Some(drained_text)`.

3. `crates/brassclaw_agent_loop/src/executor/recipe.rs`  
   `RecipeStage::process`:
   - Read `state.last_user_text`. If `None`, fall through (Tier 2 unchanged).
   - Call `host.recipe_lookup().find_recipe(user_text)`.
   - **Tier 0** (recipe match, `wilson_lower >= 0.70`, `validated`, `llm_call_required: false`):
     - Call `fetch_by_instruction` with matched variant's `BuildInstruction`.
     - Serialize `orchestrator_context` → LLM message (reformatted by `step_formatter_id` if present).
     - Serialize `rust_context` → compact JSON sent to Rust layer separately.
     - Execute via orchestrator host.
     - Skip `PromptStage` and `ModelStage`.
     - Record outcome → Wilson score update.
   - **Tier 1** (recipe match, below Tier 0 threshold or `llm_call_required: true`):
     - Call `find_skills` for ToolSkill summaries.
     - Store summaries as hint in `LoopExecutionState` for `PromptStage` to inject.
     - Continue through normal LLM path.
   - **No match:** fall through to Tier 2 (unchanged).

**Tests:**
- Unit: `last_user_text` populated after `InputStage` processes user input
- Integration: Tier 0 match → `PromptStage` and `ModelStage` not called
- Integration: Tier 1 match → ToolSkill summaries present in assembled prompt
- Integration: no match → full Tier 2 path runs unchanged
- Integration: Rust layer receives `RustContext` JSON without LLM prompt content

---

### Phase I — BasicPromptStore and Assembly

**Status:** [ ] Pending

**Goal:** Create the basic-prompt storage infrastructure and wire it into the Interceptor.

**Files to create:**
- `crates/brassclaw_pg/migrations/V051__reborn_basic_prompt_store.sql`  
  Table: one row per scope (`tenant_id, user_id, agent_id, project_id`).  
  Columns: `id UUID PK`, scope tuple, `fingerprint TEXT` (SHA-256),  
  `bundle_json JSONB` (serialised `InstructionBundle.messages`),  
  `is_stale BOOLEAN DEFAULT false`, `assembled_at TIMESTAMPTZ`,  
  `created_at`, `updated_at`.  
  Unique constraint on scope tuple.

- `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs`  
  Facade: `BasicPromptStore` trait + `PgBasicPromptStore` implementation.  
  Methods:
  - `get_for_scope(scope) -> Option<StoredBundle>`
  - `store(scope, bundle, fingerprint) -> Result<()>`
  - `mark_stale(scope) -> Result<()>`
  - `delete(scope) -> Result<()>`

**Files to modify:**
- `crates/brassclaw_reborn_composition/src/lib.rs` — export new store
- `crates/brassclaw_interceptor/` (or equivalent) — append stored basic-prompt to outgoing message before shipping to LLM
- Component validation transition hooks — on `auto_passed` → `validated`, call `mark_stale(scope)` for affected scope

**Tests:**
- Unit: `PgBasicPromptStore::store` + `get_for_scope` roundtrip
- Unit: `mark_stale` sets `is_stale = true`
- Integration: `validated` transition → basic-prompt marked stale
- Integration: Interceptor appends stored bundle to outgoing prompt

---

## 2. Migration Sequence

| Migration | Contents | Status |
|-----------|----------|--------|
| `V047__reborn_python_code.sql` | New table, class 22 | **Next** |
| `V048__reborn_extension_catalogues.sql` | New table, class 23 | |
| `V049__reborn_intent_inputs_variant_key.sql` | `ADD COLUMN variant_key TEXT` | |
| `V050__reborn_skills_intent_examples.sql` | `ADD COLUMN intent_examples JSONB; ADD COLUMN required_skills JSONB` to `reborn_skills` | |
| `V051__reborn_basic_prompt_store.sql` | New table: one row per scope; columns: `id UUID PK`, scope tuple, `fingerprint TEXT`, `bundle_json JSONB`, `is_stale BOOLEAN DEFAULT false`, `assembled_at TIMESTAMPTZ`, unique constraint on scope tuple | |

All additive. No DROP, no renames. No existing rows break.

---

## 3. Open Questions — Decisions Needed Before Phase C

1. **Variable extraction:** Named capture groups in intent expressions (e.g.
   `r"of the (?P<dir>[/\w\-\.]+) directory"`) or a small post-match LLM extraction call?  
   → **Recommendation:** named capture groups for Phase C. LLM extraction as a
   follow-up fallback for complex prompts (Phase H+ or later).

2. **Shared step prefix across variants:** Full independent step list per variant
   (simple, some duplication) or a shared prefix + divergence point (compact)?  
   → **Recommendation:** full independent list per variant. Shared prefix is a
   content convention, not enforced by schema. Keeps executor logic simple.

3. **`required_skills` inclusion threshold:** Always include all declared required
   skills, or score them against the current query first?  
   → **Recommendation:** always include if declared. Keep the list short by convention
   (capped at 10 by Q1 validator).

4. **`step_formatter_id` PythonCode component:** Should the formatter be mandatory
   or optional per variant?  
   → **Recommendation:** optional (`Option<String>`). If `None`, use the raw
   `description` fields as-is. This allows gradual adoption and A/B testing.

---

## 4. Out of Scope (Marked Postponed)

- Full self-improvement pipeline (Interceptor-driven Recipe auto-creation from successful patterns)
- Component self-creation wizard
- Automatic Sempai-driven prompt rewrites
- `FormatOrchestratorPrompt` as a distinct step type (can be added in future iteration — for now, `step_formatter_id` is applied during serialization)

---

## 5. Turn Flow Summary

Complete walkthrough of a single turn in the intended final state:

```
User types: "show all files including hidden in /tmp"
|
+- [InputStage]
|   Drains input into LoopExecutionState.
|   Sets last_user_text = "show all files including hidden in /tmp"
|
+- [RecipeStage]
|   Calls resolve_intent(query)
|   -> Match: Recipe "listing-the-contents-of-a-directory"
|             variant_key = "ls-la"
|             variable_bindings = { dir: "/tmp" }
|   -> Tier 0 check: wilson_lower = 0.82, llm_call_required = false -> eligible
|
+- [RetrievalEngine] fetch_by_instruction(build_instruction, user_text)
|   Reads fetch_steps:
|     -> fetches: <uuid:ls-skill>, <uuid:ls-toolskill>
|   Applies variable substitution: {{vars.dir}} -> "/tmp"
|   Returns prior_knowledge patch:
|     [ls-skill body]
|     [ls-toolskill body with /tmp substituted]
|     basic_prompt_section_refs: ["§directory-listing"] (pointer, no content repeat)
|
+- [Tier 0 -- no LLM call]
|   RecipeStage serializes orchestrator_context:
|     skill_ids: [<uuid:ls-skill>]
|     python_code_ids: [<uuid:ls-result-formatter>]
|     step_formatter_id: <uuid:terse-cli-formatter> (optional)
|     control_flow_steps: [RunPythonCode: format output]
|   -> LLM message assembled (if step_formatter_id present: reformat descriptions)
|
|   RecipeStage serializes rust_context (separate JSON package):
|     tool_skill_ids: [<uuid:ls-toolskill>]
|     tool_bindings: [{ tool: "ls", params: { dir: "/tmp", flags: "-la" }, error_policy: fail }]
|   -> Rust layer receives compact JSON, executes ls /tmp -la
|
|   Orchestrator reads control_flow_steps, formats result for user
|
+- [InterceptorStage]
|   Saves prompt composition plan (patch only, not the full basic-prompt).
|   If Sempai connected: reviews execution plan before it runs.
|
+- [AssistantReplyStage]
   Emits formatted directory listing to user.
   RecipeStage records outcome -> Wilson score updated.
```

**No-match path (Tier 2):**

```
User types: "explain recursion to me"
|
+- [InputStage]        last_user_text set
+- [RecipeStage]       resolve_intent -> NoMatch -> falls through
+- [PromptStage]       fetch_for_consumer() -> UNION ALL scan, keyword-scored,
|                      token-budget-capped. Big basic-prompt + UNION ALL results assembled.
+- [InterceptorStage]  Sempai reviews if connected.
+- [ModelStage]        Full LLM call with assembled prompt.
+- [AssistantReplyStage]  Emits LLM response.
   (No Recipe outcome recorded.)
```
