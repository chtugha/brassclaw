# Recipe System Finalisation Plan

> **Status:** Draft v2 — upgraded with Step-Description Systematics, Instruction-Building-System,
> and v3 turn-start architecture. No code changes are made by this document.
>
> **Scope:** Closes all architectural gaps identified in the Vision vs. Implementation analysis.
> Incorporates the v3 split-channel prior-knowledge design and the simplified single-call step-0.

---

## 0. Architecture Vision (Canonical Reference)

### 0.1 Component Hierarchy — Bottom to Top

```
┌─────────────────────────────────────────────────────────────────┐
│  ExtensionCatalogue (class 23)                                  │
│  Domain overview. task_groups[] → recipe names. Never re-docs. │
├─────────────────────────────────────────────────────────────────┤
│  Recipe (class 21)                                              │
│  Primary intent target. One RecipeVariant per distinct intent.  │
│  Each variant owns: intent_examples[], step_link formula,       │
│  variable_patterns[], and StepDescriptions (the authoring       │
│  source from which the BuildInstruction is assembled).          │
├─────────────────────────────────────────────────────────────────┤
│  Skill (classes 1–3)    │  PythonCode (class 22)               │
│  Orchestrator instruct. │  Python utilities / inline instruct.  │
│  for using one Rust tool│  for the orchestrator. Not full Skill.│
├─────────────────────────────────────────────────────────────────┤
│  ToolSkill (class 13)                                           │
│  Rust-layer only. param schema, preconditions, error handling.  │
│  The orchestrator never reads ToolSkill bodies directly.        │
├─────────────────────────────────────────────────────────────────┤
│  Tool (class 0)                                                 │
│  Rust execution layer only. No prompt text. Opaque to           │
│  the orchestrator. Excluded from all retrieval queries.         │
└─────────────────────────────────────────────────────────────────┘
```

**Runtime Extensions (classes 4–9)** remain as-is.
**ExtensionCatalogues (class 23)** are the documentation namespace. Separate class, separate table.

---

### 0.2 What the Orchestrator and Rust Layer Know — and How

Neither side has built-in knowledge. Every turn they rebuild from the retrieval result.

| What | Built-in? | How delivered today | How delivered in v3 |
|------|-----------|---------------------|---------------------|
| Skill bodies (instructions) | No | `__assemble_prior_knowledge__` UNION ALL blob | `__retrieve_docs__` orchestrator channel |
| PythonCode bodies | No | Same blob | `__retrieve_docs__` orchestrator channel |
| ToolSkill bodies | No | Same blob (mixed in) | `__retrieve_docs__` rust channel — silent, never crosses to orchestrator |
| Actions (class 16) | No | `__retrieve_docs__` shim at step-0 | `action_short_circuit` flag in result |
| Active-skill tracking | No | `__list_skills__()` → `select_skills()` | `matched_component_ids` in result — direct, no extra round-trip |
| Post-LLM tool leases | No | `__get_actions__()` post-LLM | Unchanged — not a step-0 concern |
| Volatile context / memories | No | `insert_volatile_context_at_n_minus_1()` | Unchanged — injected separately, never mixed with prior knowledge |

---

### 0.3 The Current Step-0 Problem

```python
# Current default.py step 0 — three separate calls:
pkr        = __assemble_prior_knowledge__(goal, token_budget, "02")   # PRIMARY — single merged blob
docs       = __retrieve_docs__(goal, 5)                               # SHIM — dead Action detection
all_skills = __list_skills__()                                        # extra round-trip for tracking
active_skills = select_skills(all_skills, goal, ...)                  # re-selection of already-known skills
```

**Problems:**

1. `__retrieve_docs__` at step-0 uses the legacy `RetrievalEngine::retrieve_context`
   (MemoryDoc path). It returns `{type, title, content}` with no `class_code` in the
   metadata. The Action-detection check `metadata.get("class_code") == 16` at line 1022
   therefore **never fires** — the named bug acknowledged in the comment at line 1011.

2. `__assemble_prior_knowledge__` returns one merged `formatted_content` blob. Skill bodies,
   PythonCode, ToolSkills — everything goes to the orchestrator together. There is no
   channel separation.

3. `__list_skills__()` → `select_skills()` is a redundant round-trip. Since Phase 1.5,
   `select_skills()` does no scoring — it just takes the first N that fit in budget, in
   the order Rust pre-sorted them. With a BuildInstruction, the IBS already selected
   the exact Skills for this turn by UUID. The re-selection step is unnecessary.

4. `__get_actions__()` is **not** called at step-0. It is called post-LLM at line 1122
   for obligation nudging (checking live tool leases after the LLM replies with text).
   It is unrelated to prior-knowledge assembly and is not changed by this plan.

---

### 0.4 v3 Step-0 — Single Call, Everything Inside

The three-call block collapses to one call. The RetrievalEngine handles everything:

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

    # Active-skill tracking: IBS already selected the right Skills by UUID.
    # No __list_skills__() + select_skills() round-trip needed.
    _set_active_skills_from_matched_ids(pkr.get("matched_component_ids", []), state)
```

**What `__retrieve_docs__` returns in v3:**

```python
{
    # Orchestrator channel only — Skills, PythonCode, LLM-formatted step annotations.
    # ToolSkill bodies never appear here.
    "orchestrator_content": str,

    # Routing signals:
    "override_prompt_creation": bool,
    "action_short_circuit":     bool,
    "action_component_id":      str,   # UUID (when action_short_circuit is true)
    "action_name":              str,
    "disambiguation":           bool,
    "candidates":               list,

    # Tracking:
    "matched_component_ids":    list,  # orchestrator-channel component UUIDs
                                       # (Skill + PythonCode UUIDs — used for
                                       #  __set_active_skills__ without re-selection)
}
```

The Rust channel (ToolSkills) is applied to the Rust execution context **inside the
handler, silently**. It never crosses to the orchestrator's working_messages.

---

### 0.5 `__retrieve_docs__` — What Changes and What Stays

| Call site | Old behaviour | v3 behaviour |
|-----------|--------------|--------------|
| `default.py` step-0 | Legacy MemoryDoc keyword search, returns list of `{type, title, content}` | Upgraded: delegates to intent system → IBS → split result, returns routing dict |
| `default.py` step-0 Action shim | Dead `class_code == 16` check that never fires | Removed. `action_short_circuit` flag returned directly from intent match. |
| `call_action` nested lookup (line 844) | `__retrieve_docs__(name, 1)` search-by-name | Replaced by new host fn `__fetch_component__(uuid, class_code)` — exact fetch via `fetch_component_by_id` |
| `__assemble_prior_knowledge__` | Primary path, returning merged blob | Superseded by upgraded `__retrieve_docs__`. Kept registered for backward compat; removed in Phase K cleanup. |
| `__list_skills__()` at step-0 | Extra round-trip, then `select_skills()` re-selects | Eliminated. `matched_component_ids` carries selected Skill UUIDs directly. |

---

### 0.6 `fetch_for_turn` Upgrade — Reading StepDescriptions

The existing [`PostgresSource::fetch_for_turn`](crates/brassclaw_engine/src/memory/retrieval_source.rs:646)
already calls `resolve_intent` → `fetch_component_by_id`. The v3 upgrade adds four steps
on an intent match when the matched component is a Recipe (class 21):

```
fetch_for_turn(scope, query, token_budget, consumer_tag):

  1. resolve_intent(pool, scope, query)
       → Match { component_id, class_code, step_link }

          a. class_code == 16 (Action):
               return FetchForTurnResult::ActionShortCircuit { component_id, name }

          b. class_code == 21 (Recipe) AND step_link.is_some():
               i.   Fetch Recipe row → get step_descriptions[] for this variant
               ii.  IBS: parse step_link → build_instruction(step_descriptions, variable_patterns)
                          → BuildInstruction { rust_steps[], orchestrator_steps[] }
               iii. Apply variable substitution ({{vars.name}} → captured value)
               iv.  Fetch ComponentItem for each step's include[] UUIDs:
                          rust_items         = fetch_component_by_id(uuid) per rust_step
                          orchestrator_items = fetch_component_by_id(uuid) per orchestrator_step
               → return FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }

          c. step_link.is_none():
               Existing fetch_component_by_id path — all items go to orchestrator (unchanged)

       → Disambiguation { candidates }:
               return FetchForTurnResult::Disambiguation(candidates)

       → NoMatch / DbLessFallback:
               fetch_for_consumer (UNION ALL) — all items to orchestrator (unchanged)

  2. Caller (handle_retrieve_docs) receives FetchForTurnResult and:
       SplitResult:        applies rust_items to Rust execution context silently;
                           formats orchestrator_items into orchestrator_content JSON;
                           returns routing signals to Python
       Components:         all items go into orchestrator_content (no-match path, unchanged)
       ActionShortCircuit: returns action_short_circuit flags, no content
       Disambiguation:     returns disambiguation candidates
```

**Extended `FetchForTurnResult`:**

```rust
pub enum FetchForTurnResult {
    /// No-match UNION ALL path or non-recipe intent match (existing behaviour unchanged).
    Components(Vec<ComponentItem>),
    /// Multiple near-equal intent candidates — surface disambiguation UX.
    Disambiguation(Vec<IntentCandidate>),
    /// Intent matched an Action (class 16) — execute directly, no LLM.
    ActionShortCircuit { component_id: Uuid, name: String },
    /// Intent matched a Recipe with a step_link — two channels assembled by IBS.
    SplitResult {
        rust_items:         Vec<ComponentItem>,
        orchestrator_items: Vec<ComponentItem>,
        routing:            TurnRoutingSignals,
    },
}

pub struct TurnRoutingSignals {
    pub override_prompt_creation: bool,
    pub matched_component_ids:    Vec<String>,  // orchestrator-channel UUIDs
    pub variant_label:            String,
    pub step_link:                String,
}
```

---

### 0.7 Current Turn Pipeline

```
1.  CheckpointStage     — cancel-check
2.  BudgetStage         — token/iteration budget check
3.  InputStage          — drain pending user input into LoopExecutionState
4.  RecipeStage         — [STUB] intent/recipe lookup hook — always passes through today
5.  PromptStage         — assemble LLM prompt from history + prior_knowledge
6.  InterceptorStage    — Sempai review of outgoing prompt (if connected)
7.  ModelStage          — LLM call (Kohai)
8.  ReplyAdmissionStage — validate/admit model response
9.  AssistantReplyStage — emit response to user
10. CapabilityStage     — if response contains tool calls: execute tools, loop back
11. StopStage           — check for loop termination
12. ExitStage           — clean exit
```

**RecipeStage (step 4) is currently a stub.** It always falls through to Tier 2 (full LLM).
The module-level comment in `recipe.rs` documents the required fix: add `last_user_text`
to `LoopExecutionState` (populated by `InputStage`). This is Phase H.

The prior-knowledge assembly happens inside `PromptStage` (step 5) via the orchestrator's
`__retrieve_docs__` call, which in v3 calls `PostgresSource::fetch_for_turn` and handles
the full split internally.

---

### 0.8 KV-Cache Design

The intent-matched prior-knowledge patch (orchestrator_items) is injected as
`memory_snippets` (PRIORITY 3 in the `InstructionBundle`) — the stable prefix zone.
Volatile context goes into `inline_messages` (PRIORITY 7) — the volatile tail.
This boundary ensures the LLM KV-cache prefix covers everything up to the conversation
start on repeated similar intents.

**Rules for the patch:**
- Must NOT repeat content already in the stored basic-prompt.
- Patch size target: < 4k tokens (fast new-token computation).
- Skill/ToolSkill bodies already in the basic-prompt are referenced by section header
  annotation only: `→ see §ls-skill in basic-prompt`.

---

### 0.9 Normal Assembly — No-Match Path

When `resolve_intent` returns `NoMatch` or `DbLessFallback`, `fetch_for_consumer` runs
(UNION ALL across all validated component tables, keyword-scored, token-budget-capped).
This is the existing behaviour. The `doc_type_weight_by_class` table in
`retrieval_dbless.rs` needs updating to include classes 22 and 23 at appropriate weights.

---

## 1. Step-Description Systematics

### 1.1 Purpose and Dual Role

A **StepDescription** serves two distinct audiences simultaneously:

- **Human/editor:** a YAML-structured, editable description of every step a component
  performs after an intent match. Visible and editable in the WebUI component page.
- **Instruction-Building-System:** the authoritative source from which the
  `BuildInstruction` (two-channel: rust + orchestrator) is assembled.

StepDescriptions are stored as a JSONB column `step_descriptions` on `reborn_recipes`,
attached to each `RecipeVariant`. They are authored in YAML (most readable for
multi-line text fields), stored as JSON internally. The IBS normalises to JSON at
assembly time.

---

### 1.2 Required Fields per Step

| Field | Type | Description |
|-------|------|-------------|
| `stepnumber` | int | Ordinal position in the sequence (1-based) |
| `knowledge` | string | Which channel owns this step: `orchestrator`, `rust`, or `both` |
| `goal` | string | Human-readable aim of this step |
| `content` | string | Short description of what this step contains/does |
| `type` | string | `text`, `component`, or `snippet` |

### 1.3 Optional Fields per Step

| Field | Description |
|-------|-------------|
| `info` | Free-text annotation. Used by IBS for control-flow descriptions where no component reference is needed. |
| `include` | Component UUID(s) needed for this step. IBS emits a fetch for each UUID listed. |
| `codesnippet` | Inline Python code. When saved from WebUI, triggers creation of a new PythonCode component (class 22) that enters the Q1 validation queue. Step is greyed out until validated; upgraded to `type: component` with the new UUID on Q2 pass. |

### 1.4 Step Types

| Type | IBS behaviour |
|------|--------------|
| `text` | Emits an annotation into the BuildInstruction. No component fetch. Carries `info` text into the LLM-formatted orchestrator content. |
| `component` | Emits a fetch for each UUID in `include`. Routes items to rust or orchestrator channel based on `knowledge`. |
| `snippet` | WebUI-only authoring shortcut. IBS **refuses to assemble** a BuildInstruction while any step has this type — it must be promoted to `component` after the created PythonCode passes Q1+Q2. |

### 1.5 Basic and Variant StepDescriptions

A component with one use case has **StepDescription0** (the basic description).
A component with multiple use cases has:

- **StepDescription0** — the basic/most-common use case.
- **StepDescription1, 2, …** — each covers an additional use case, composed of a
  *common part* (steps shared with SD0 up to a divergence point) and an *individual part*
  (steps unique to this variant).

### 1.6 Intent-Link Formula (`step_link`)

Each intent in `reborn_intent_inputs` carries a `step_link` TEXT column encoding which
steps to assemble:

```
step_link = "{desc_idx}:{start}-{desc_idx}:{end}" [+ "+" more segments]

  {desc_idx} = StepDescription index (0 = basic, 1 = first variant, …)
  {start}    = step number or 0 (first step)
  {end}      = step number or E (last step)

Examples:
  "0:0-0:E"                        → All steps of SD0 (single-variant component)
  "0:0-0:30+1:0-1:E"               → Steps 0..30 of SD0 (common) + all of SD1 (individual)
  "0:0-0:31+2:0-2:E"               → Steps 0..31 of SD0 + all of SD2
  "0:0-0:30+1:0-1:11+3:0-3:E"     → Steps 0..30 of SD0 + steps 0..11 of SD1 + all of SD3
```

The `step_link` replaces the previously planned `variant_key` TEXT column. It is more
expressive (encodes shared prefixes without duplicating steps) and is the direct input
to the IBS — no secondary lookup or branching needed.

### 1.7 Example StepDescriptions — "local-files-reading" Recipe

```yaml
# StepDescription0 — basic variant (ls -l)
steps:
  - stepnumber: 1
    knowledge: orchestrator
    goal: Provide context info
    content: Information explaining the Task
    type: text
    info: |
      This task is performed by the orchestrator only.
      The rust execution layer receives ToolSkill "ls" and Tool "ls".
      The orchestrator receives Skill "ls" and PythonCode "ls-result-handler".
      The orchestrator instructs rust via the skill; rust uses the ToolSkill
      to call the Tool and returns output. The orchestrator formats the output
      for the chat window.

  - stepnumber: 2
    knowledge: rust
    goal: Provide ToolSkill
    content: ToolSkill "ls"
    type: component
    include: "uuid-of-ls-toolskill"

  - stepnumber: 3
    knowledge: orchestrator
    goal: Provide Skill
    content: Skill "ls"
    type: component
    include: "uuid-of-ls-skill"

  - stepnumber: 4
    knowledge: orchestrator
    goal: Provide execution instructions
    content: PythonCode "ls-result-handler"
    type: component
    include: "uuid-of-ls-result-handler-pythoncode"
    info: |
      Final step. PythonCode tells the orchestrator how to invoke the skill,
      pass flags to rust, and format the output for the chat window.
```

---

## 2. Instruction-Building-System (IBS)

### 2.1 Purpose

The IBS is a pure-Rust module that reads a StepDescription set (via a `step_link` formula),
resolves the referenced component UUIDs, and emits a two-channel `BuildInstruction`.

**It is the sole producer of BuildInstructions.** BuildInstructions are never hand-authored.
StepDescriptions are the authoritative source. The BuildInstruction is a derived artefact
assembled on intent-match.

> **Why assemble on match rather than pre-store?** Component UUIDs in `include` fields can
> be updated (a PythonCode component is revised and re-validated). Pre-stored BuildInstructions
> would require a cascade rebuild on every component update. On-match assembly always reads
> current, validated UUIDs with zero staleness risk. Hot-path memoisation (per-process,
> keyed on `sha256(step_link + sorted_include_uuids)`, evicted on validation-status change)
> eliminates the cost for repeated identical intents.

### 2.2 Assembly Algorithm

```
fn build_instruction(step_link, step_descriptions, variable_patterns)
    → Result<BuildInstruction, IbsError>

1. Parse step_link into Vec<StepRange>  (e.g. [(0, 0..=30), (1, 0..=E)])
2. For each StepRange:
     Select steps[start..=end] from step_descriptions[desc_idx]
     Append to ordered step list
3. For each step in the ordered list:
     type == "component" → emit RecipeStep { include UUIDs }, route by knowledge
     type == "text"      → emit annotation step (no UUID), route by knowledge
     type == "snippet"   → return IbsError::UnpromotedSnippet
4. Partition steps:
     rust_steps[]         ← steps where knowledge ∈ {"rust", "both"}
     orchestrator_steps[] ← steps where knowledge ∈ {"orchestrator", "both"}
5. Return BuildInstruction { rust_steps, orchestrator_steps,
                              variable_patterns, basic_prompt_refs }
```

### 2.3 BuildInstruction Shape

```rust
pub struct BuildInstruction {
    /// Variable extraction patterns applied to the user prompt at match time.
    pub variable_patterns: Vec<VariablePattern>,

    /// Steps for the Rust execution layer only.
    /// Contains: read_tool_skill, call_tool, relay_result.
    /// Never contains orchestrator instructions or LLM prompt content.
    pub rust_steps: Vec<RecipeStep>,

    /// Steps for the Python orchestrator only.
    /// Contains: Skills, PythonCode, run_python_code, invoke_skill,
    ///           assemble_llm_prompt, text annotations.
    /// Never contains raw ToolSkill bodies or Tool registration data.
    pub orchestrator_steps: Vec<RecipeStep>,

    /// Navigation hints into the cached basic-prompt.
    /// Content is NOT re-fetched — the LLM already has it from KV-cache.
    pub basic_prompt_refs: Vec<String>,
}
```

### 2.4 LLM-Formatted Orchestrator Content

The IBS also produces a LLM-optimised text block from the orchestrator steps. This is
what appears in `orchestrator_content` in the `__retrieve_docs__` result:

```
## Task: {recipe.name} — variant: {variant.label}

Step 1 [orchestrator — info]:
  This task is performed by the orchestrator only. The rust execution layer
  receives ToolSkill "ls"…

Step 3 [orchestrator — skill]:
  Skill "ls" (UUID: uuid-of-ls-skill) loaded.
  [skill body content]

Step 4 [orchestrator — python_code]:
  PythonCode "ls-result-handler" (UUID: uuid-of-pythoncode) loaded.
  [pythoncode body content]
  Final step: use the skill to call rust, format output for chat window.
```

This replaces the freeform prior-knowledge blob for intent-matched turns.

### 2.5 IBS Location

- New module: `crates/brassclaw_engine/src/memory/instruction_builder.rs`
- Pure Rust, no async, no DB calls.
- Called by `PostgresSource::fetch_for_turn` after an intent match resolves to a Recipe.
- Exposed via `crate::memory::instruction_builder::build_instruction`.

### 2.6 IBS Validation Rules (enforced at Q1)

- All `include` values must parse as valid UUID v4.
- No `snippet`-type steps (must be promoted to `component` before IBS assembly).
- Step numbers must be monotonically increasing within each StepDescription.
- Steps with `knowledge: rust` must have `type: component` with a non-empty `include`.
- Every rust-channel step that results in a `call_tool` must have a preceding
  `read_tool_skill` step in `rust_steps` (S7 guard).

---

## 3. WebUI — StepDescription Editor

The Recipe component page body gains a **Step Descriptions** section:

- All steps listed in order; each step editable on click.
- Dropdown fields for `knowledge` and `type`; free text for `goal`, `content`, `info`.
- `include` field: UUID autocomplete over known component names.
- New step of type `snippet`: shows a Python code editor. On save, creates a PythonCode
  component (class 22) and sends it to Q1. The step is greyed out until Q1+Q2 pass.
  If Q1 fails: snippet field cleared, no component created.
- Intents section: shows the `step_link` formula per intent, editable with live syntax
  validation (parse-error highlighted inline).
- On any StepDescription save: the affected Recipe variant is sent to Q1. Until Q1 passes,
  the variant is greyed out and inactive.

---

## 4. Implementation Phases

### Phase A — StepDescription Schema + IBS Core

**Goal:** Define the StepDescription types, add `step_descriptions` JSONB to `reborn_recipes`,
implement the IBS core.

**Files to create:**
- `crates/brassclaw_engine/src/memory/instruction_builder.rs`
  — `StepDescriptionEntry`, `StepLink`, `StepRange`, `parse_step_link()`,
  `build_instruction()`, `IbsError`, `BuildInstruction`, `RecipeStep`, `StepOwner`,
  `RecipeStepType`, `VariablePattern`
- `crates/brassclaw_pg/migrations/V047__reborn_recipe_step_descriptions.sql`
  — `ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS step_descriptions JSONB`

**Files to modify:**
- `crates/brassclaw_engine/src/types/recipe.rs` — add `BuildInstruction` two-channel shape;
  full `RecipeStepType` enum; `StepOwner`
- `crates/brassclaw_engine/src/memory/mod.rs` — `pub mod instruction_builder`

**Tests:**
- Unit: `parse_step_link("0:0-0:E")` → single range, all steps
- Unit: `parse_step_link("0:0-0:30+1:0-1:E")` → two ranges, correct indices
- Unit: `build_instruction` with `knowledge: rust` step → step only in `rust_steps`
- Unit: `build_instruction` with `knowledge: both` step → step in both channels
- Unit: `build_instruction` with `snippet`-type step → `IbsError::UnpromotedSnippet`
- Unit: step numbers non-monotonic → `IbsError::StepOrderViolation`

---

### Phase B — PythonCode Component (class 22)

**Goal:** New component class for Python code/instruction elements targeted at the orchestrator.

**Files to create:**
- `crates/brassclaw_pg/migrations/V048__reborn_python_code.sql`
  — same shape as `reborn_specs`; `class_code = 22`;
  default consumer tags: `{02:orchestrator, 05:validator}` until validated.
- `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs`

**Files to modify:**
- `retrieval_source.rs` — add class 22 to UNION ALL and `fetch_component_by_id`
- `intent_system.rs` — add `22 => "python_code"` to `class_label`
- `types/memory.rs` — add `DocType::PythonCode`
- `retrieval_dbless.rs` — weight 0.42 (between Skills and Extensions)
- `component_validator.rs` — class 22: name format, non-empty content,
  soft 10k token budget, shell-injection scan

**Tests:**
- Unit: `class_label(22) == "python_code"`
- Integration: PythonCode row → retrieved via `fetch_for_consumer` with tag `02:orchestrator`

---

### Phase C — ExtensionCatalogue Component (class 23)

**Goal:** Documentation-container class that organises a capability domain.

**Files to create:**
- `crates/brassclaw_pg/migrations/V049__reborn_extension_catalogues.sql`
  — columns: scope tuple + `name`, `description`, `version`, `overview_doc` (TEXT),
  `task_groups` (JSONB), `child_component_ids` (UUID[]), `intent_index` (JSONB, audit-only),
  standard lifecycle columns, `class_code SMALLINT DEFAULT 23`.
- `crates/brassclaw_reborn_composition/src/pg_extension_catalogue_store.rs`

**Files to modify:**
- `retrieval_source.rs` — add class 23 (content = `overview_doc`)
- `intent_system.rs` — add `23 => "extension_catalogue"`
- `types/memory.rs` — add `DocType::ExtensionCatalogue`
- `retrieval_dbless.rs` — weight 0.38 (near Recipes)
- `component_validator.rs` — class 23: name format, non-empty `overview_doc`,
  ≥1 task_group, valid UUID syntax in `child_component_ids`

**Tests:**
- Unit: `class_label(23) == "extension_catalogue"`
- Integration: Catalogue with `task_groups` → retrieved with `overview_doc` as `effective_content`

---

### Phase D — `step_link` Column on Intent Inputs

**Goal:** Add `step_link` to `reborn_intent_inputs`; wire into `resolve_intent`.

**Files to create:**
- `crates/brassclaw_pg/migrations/V050__reborn_intent_inputs_step_link.sql`

  ```sql
  ALTER TABLE reborn_intent_inputs ADD COLUMN IF NOT EXISTS step_link TEXT;
  ```

**Files to modify:**
- `intent_system.rs` — add `step_link: Option<String>` to `IntentResolution::Match`;
  select `step_link` column in the resolution query
- `seed_intent_input` — accept and store `step_link`

**Notes:**
- `step_link` is nullable. Existing intent rows without it fall through to the existing
  `fetch_component_by_id` path unchanged — zero breakage.
- `step_link` is the direct replacement for `variant_key`. New variants are authored with
  `step_link` from the start.

**Tests:**
- Unit: row with `step_link` → `IntentResolution::Match { step_link: Some(...) }`
- Unit: row without `step_link` → `IntentResolution::Match { step_link: None }` → existing path

---

### Phase E — `fetch_for_turn` Upgrade + `FetchForTurnResult::SplitResult`

**Goal:** Wire the IBS into `PostgresSource::fetch_for_turn`. On a Recipe intent match with
a `step_link`, call the IBS, fetch component items for each channel, and return a split result.

**Files to modify:**
- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  — extend `FetchForTurnResult` with `ActionShortCircuit` and `SplitResult` variants
  — update `PostgresSource::fetch_for_turn`:
    1. Fetch `step_descriptions` JSONB from the matched Recipe row
    2. Call `instruction_builder::build_instruction(step_link, step_descriptions, variable_patterns)`
    3. Apply variable substitution to all step params
    4. Fetch `ComponentItem` for each UUID in `rust_steps` and `orchestrator_steps`
    5. Return `FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }`
  — handle `ActionShortCircuit` when `class_code == 16`

**Tests:**
- Unit: Recipe match with step_link → `SplitResult`; rust_items contain only ToolSkills;
  orchestrator_items contain only Skills and PythonCode
- Unit: `knowledge: both` step → UUID in both `rust_items` and `orchestrator_items`
- Unit: Action (class 16) match → `ActionShortCircuit`
- Unit: match with `step_link: None` → existing `Components([single_item])` path unchanged
- Integration: full intent match → correct channel split

---

### Phase F — `handle_retrieve_docs` Upgrade (Rust handler)

**Goal:** Upgrade the Rust handler behind `__retrieve_docs__` to use `fetch_for_turn`
and handle the new `FetchForTurnResult` variants.

**Files to modify:**
- `crates/brassclaw_engine/src/executor/orchestrator.rs`
  — `handle_retrieve_docs`: replace `RetrievalEngine::retrieve_context` with
  `retrieval_source.fetch_for_turn()` (already used by `handle_assemble_prior_knowledge`);
  handle all four `FetchForTurnResult` variants:
    - `SplitResult`: apply `rust_items` to Rust execution context silently;
      format `orchestrator_items` into `orchestrator_content` JSON; return routing dict
    - `Components`: all items → `orchestrator_content` (no-match path, unchanged shape)
    - `ActionShortCircuit`: return `action_short_circuit: true` dict
    - `Disambiguation`: return `disambiguation: true` dict
  — return value is now always a dict (not a list), guarded by `isinstance(pkr, dict)`
    in Python (the existing guard from `__assemble_prior_knowledge__` already handles this)

**Tests:**
- Unit: `SplitResult` → `orchestrator_content` contains Skills/PythonCode but NOT ToolSkill bodies
- Unit: `ActionShortCircuit` → `action_short_circuit: true`, empty `orchestrator_content`
- Unit: `Components` (no-match) → `orchestrator_content` contains all items (baseline preserved)

---

### Phase G — Python Step-0 Upgrade + `call_action` Migration

**Goal:** Replace the three-call step-0 with the single `__retrieve_docs__` call.
Migrate `call_action` nested-lookup to `__fetch_component__`.

**Files to modify:**
- `crates/brassclaw_engine/orchestrator/default.py`
  — Replace step-0 block (lines 994–1059) with v3 single-call pattern (§0.4)
  — Remove `__retrieve_docs__(goal, 5)` Action shim (lines 1018–1028)
  — Remove `__list_skills__()`, `select_skills()` calls (lines 1031–1059)
  — Add `_set_active_skills_from_matched_ids(matched_ids, state)` helper
  — Replace `call_action` `__retrieve_docs__(nested_name, 1)` at line 844 with
    `__fetch_component__(uuid, class_code)` (UUID from BuildInstruction step)

- `crates/brassclaw_engine/src/executor/orchestrator.rs`
  — Register new host function `__fetch_component__(uuid: str, class_code: int)`
  — Handler: calls `fetch_component_by_id` directly; returns single item dict or None

**Tests:**
- Unit: step-0 intent match → `orchestrator_content` injected; no `__list_skills__` call
- Unit: action short-circuit → `execute_action_by_id` called; LLM not reached
- Unit: disambiguation → `handle_disambiguation` called
- Unit: no-match → UNION ALL content injected (baseline behaviour preserved)
- Integration: `call_action` using `__fetch_component__` → correct Action fetched by UUID

---

### Phase H — RecipeStage: Wire `last_user_text` + Tier 0/1 Dispatch

**Goal:** Activate the RecipeStage stub so it actually dispatches.

1. Add `last_user_text: Option<String>` to `LoopExecutionState` (populated by `InputStage`)
2. `RecipeStage::process` reads `state.last_user_text`
3. Calls `fetch_for_turn` via the host's retrieval source with `last_user_text`
4. On `SplitResult` + Wilson ≥ 0.70 (Tier 0):
   - Stash `rust_items` in Rust execution context
   - Stash `orchestrator_items` in state for orchestrator
   - Skip `PromptStage` and `ModelStage`
   - Return `RecipeStep::TierZero { routing }`
5. On `SplitResult` + Wilson < 0.70 (Tier 1):
   - Stash items as a hint in `LoopExecutionState` for `PromptStage`
   - Continue through normal LLM path
6. On `ActionShortCircuit`: execute Action directly, skip LLM
7. On `Components` / `Disambiguation` / no library: fall through (Tier 2, unchanged)

**Tests:**
- Unit: `last_user_text` populated by `InputStage` after drain
- Integration: Tier 0 match → PromptStage and ModelStage skipped
- Integration: Tier 1 match → hint injected into prompt, LLM called
- Integration: no match → falls through to full LLM (Tier 2)

---

### Phase I — Q1 Validator Upgrades

**Files to modify:**
- `crates/brassclaw_engine/src/memory/component_validator.rs`

New dispatch cases:
- **Class 22 (PythonCode):** name format, non-empty content, soft 10k token budget,
  shell-injection scan
- **Class 23 (ExtensionCatalogue):** name format, non-empty `overview_doc`, ≥1 task_group,
  valid UUID syntax in `child_component_ids`
- **Recipe class 21 (StepDescriptions):** call IBS `build_instruction` as pre-flight;
  reject on any `IbsError`; all `include` UUIDs must parse as UUID v4;
  no `snippet`-type steps; step numbers monotonically increasing; S7 guard
- **Skill classes 1–3:** `intent_examples` entries ≤ 512 chars, capped at 20;
  `required_skills` capped at 10; no self-reference

**Tests:**
- Unit: Recipe with `snippet`-type step → Q1 fail with `IbsError::UnpromotedSnippet`
- Unit: Recipe with unparseable UUID in `include` → Q1 fail
- Unit: PythonCode with shell-injection pattern → Q1 fail
- Unit: valid StepDescriptions → Q1 pass

---

### Phase J — Skill `intent_examples` + `required_skills`

**Files to modify:**
- `crates/brassclaw_skills/src/types.rs` — add `intent_examples: Vec<String>` and
  `required_skills: Vec<String>` to `SkillManifest`; enforce limits in
  `ActivationCriteria::enforce_limits`
- Seed intent inputs on skill `auto_passed` transition
- Purge intent inputs on skill wipe/delete
- IBS: when including a Skill step, also include all declared `required_skills` UUIDs
  in the same channel (orchestrator)

**Files to create:**
- `crates/brassclaw_pg/migrations/V051__reborn_skills_intent_examples.sql`
  — `ADD COLUMN intent_examples JSONB`, `ADD COLUMN required_skills JSONB`

**Tests:**
- Unit: `SkillManifest` with `intent_examples` + `required_skills` YAML roundtrip
- Unit: entry > 512 chars → rejected by `enforce_limits`
- Integration: Skill with `intent_examples` → resolves via `resolve_intent`
- Integration: Skill with `required_skills: ["pipe-skill"]` → both Skills in orchestrator channel

---

### Phase K — MCP Translation Layer + Cleanup

**MCP translation** (`crates/brassclaw_extensions/src/mcp_translation.rs`):
- For each MCP tool: generate Tool (class 0), ToolSkill (class 13), Skill (class 1),
  skeleton Recipe (class 21) with auto-generated StepDescriptions
  (one rust step for the ToolSkill + one orchestrator step for the Skill),
  default `step_link: "0:0-0:E"`
- One ExtensionCatalogue (class 23) grouping all of the above
- All inserted with `validation_status = 'pending'`

**Cleanup:**
- Remove `__assemble_prior_knowledge__` handler registration from `orchestrator.rs`
  (superseded by upgraded `__retrieve_docs__`)
- Remove step-0 shim comment block from `default.py`
- Add deprecation notice to `__list_skills__`: no longer called from default step-0;
  remains callable for external / custom orchestrators

---

## 5. Migration Sequence

| Migration | Contents |
|-----------|----------|
| `V047__reborn_recipe_step_descriptions.sql` | ADD COLUMN `step_descriptions JSONB` to `reborn_recipes` |
| `V048__reborn_python_code.sql` | New table, class 22 |
| `V049__reborn_extension_catalogues.sql` | New table, class 23 |
| `V050__reborn_intent_inputs_step_link.sql` | ADD COLUMN `step_link TEXT` to `reborn_intent_inputs` |
| `V051__reborn_skills_intent_examples.sql` | ADD COLUMN `intent_examples JSONB`, `required_skills JSONB` to `reborn_skills` |
| `V052__reborn_basic_prompt_store.sql` | New table: one row per scope (tenant/user/agent/project); columns: `id UUID PK`, scope tuple, `fingerprint TEXT` (SHA-256), `bundle_json JSONB`, `is_stale BOOLEAN DEFAULT false`, `assembled_at TIMESTAMPTZ`, timestamps. Unique constraint on scope tuple. |

All additive. No DROP, no renames. No existing rows break.
`step_link` is nullable — existing intent rows without it use the existing
`fetch_component_by_id` path unchanged.

---

## 6. Open Questions

| # | Question | Recommendation |
|---|----------|---------------|
| 1 | StepDescription format: YAML vs TOML vs JSON for authoring? | Keep YAML. Most readable for multi-line text fields. IBS normalises to JSON internally. |
| 2 | BuildInstruction memoisation: per-process or always recompute? | Per-process, keyed on `sha256(step_link + sorted_include_uuids)`. Evict on any component's `validation_status` change. IBS is pure-Rust and fast; memoisation avoids repeated UUID DB fetches for hot intents. |
| 3 | Variable extraction: named capture groups or LLM fallback? | Named capture groups for Phase A. LLM extraction as opt-in fallback when `variable_patterns` returns no captures and the variant carries a `llm_var_extraction_prompt` field. Keep the fast path fast. |
| 4 | ToolSkill pre-load timing: inline or deferred to step execution? | Deferred. IBS emits `rust_steps` with UUIDs; Rust calls `fetch_component_by_id` at the `read_tool_skill` step. Avoids loading ToolSkills that may not be reached in conditional branches. |
| 5 | Legacy DocPlan → v3 translation? | See §6.1. |

### 6.1 Legacy DocPlan → v3 Component Translation

The v2 system stored all knowledge as `MemoryDoc` rows. `component_import.rs` already
migrates these to class-specific tables. The gap: no v2 docs have StepDescriptions or
`step_link` formulae.

**One-time, operator-triggered translation pipeline (`brassclaw migrate-to-v3`):**

1. **Skills (class 1–3):** Extract imperative sentence candidates from the skill `body`
   as seed intent examples. Generate StepDescription0:
   - Step 1: `knowledge: orchestrator`, `type: text`, `info`: summarised body
   - Step 2: `knowledge: orchestrator`, `type: component`, `include`: skill UUID
   Default `step_link: "0:0-0:E"`. Route to Q1.

2. **ToolSkills (class 13):** Generate StepDescription0 with one rust-channel component
   step. No intent examples (ToolSkills are referenced by Skills, not intent-matched).

3. **Existing Recipes (class 21) without `step_descriptions`:** Map each entry in the
   existing `steps` JSONB (13-type Action format) to the nearest StepDescription type.
   Steps with no component reference → `type: text`. Steps with a component reference →
   `type: component` with the existing UUID in `include`. Route to Q1.

4. **Specs, Lessons, Notes:** Leave as-is. Served by the UNION ALL path, no StepDescriptions needed.

Idempotent: components that already have `step_descriptions` are skipped.

---

## 7. Out of Scope (Postponed)

- Full self-improvement pipeline (Interceptor-driven Recipe auto-creation)
- Component self-creation wizard
- Automatic Sempai-driven prompt rewrites
- LLM-based variable extraction fallback (Phase A: regex only)
- Tier 0 production activation (requires Phases A–H complete + Wilson scoring
  validated in production for ≥ 2 weeks)

---

## 8. Complete Turn-Flow Walkthrough (v3 final state)

```
User types: "show all files including hidden in /tmp"
│
├─ [InputStage]
│   state.last_user_text = "show all files including hidden in /tmp"
│
├─ [RecipeStage]  (Phase H — currently a stub)
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
│   Wilson score = 0.82 → Tier 0
│   → rust_items applied to Rust execution context silently
│   → orchestrator_items stashed in state
│   → RecipeStep::TierZero
│
├─ [PromptStage skipped — Tier 0]
├─ [ModelStage  skipped — Tier 0]
│
│   Orchestrator (Python) receives pkr from __retrieve_docs__:
│     pkr["orchestrator_content"] = {
│         prior_knowledge: [ls-skill body, ls-result-handler body, step-1 info text],
│         matched_component_ids: [uuid-ls-skill, uuid-ls-result-handler]
│     }
│   → No ToolSkill bodies. No memories. No UNION ALL noise.
│   → Orchestrator invokes ls-skill with { flags: "-la", dir: "/tmp" }
│   → Rust reads ls-toolskill (pre-loaded), calls: ls /tmp -la
│   → Rust returns stdout to orchestrator
│   → Orchestrator runs ls-result-handler PythonCode → formats output
│
├─ [InterceptorStage]
│   Saves composition plan (orchestrator_steps + routing, not basic-prompt).
│
└─ [AssistantReplyStage]
    Emits formatted directory listing to user.
    Wilson score incremented.

────────────────────────────────────────────────────────
No-match path (Tier 2) — "explain recursion to me"
────────────────────────────────────────────────────────

├─ [InputStage] → last_user_text set
├─ [RecipeStage] → fetch_for_turn → NoMatch
│     → fetch_for_consumer (UNION ALL)
│     → FetchForTurnResult::Components([...])
│     → RecipeStep::Continue (Tier 2, unchanged)
├─ [PromptStage] → normal assembly; all UNION ALL components in orchestrator_content
│   → volatile context injected separately (never mixed with prior knowledge)
├─ [ModelStage] → full LLM call
└─ [AssistantReplyStage] → emit LLM response
   (No Recipe outcome recorded — no match)
```
