# Recipe System v3 — Architectural Review & Recommendations

> **Context:** Pre-implementation deep dive — BuildInstruction, StepDescription authoring,
> Instruction-Building-System design, and turn dataflow.  
> **Based on:** Live codebase analysis (orchestrator, Rust executioner, retrieval engine, v2 MemoryDoc schema).  
> **Status:** Recommendations only — no code changes proposed yet.

---

## 1. Critical Findings from Codebase Analysis

### 1.1 Orchestrator Has Zero Built-In Knowledge

**File:** `crates/brassclaw_engine/orchestrator/default.py` (1262 lines)

The Python orchestrator (`default.py`) is intentionally stateless about tools and skills.
**Every turn**, it calls back to Rust via host functions:

| Host Function | Returns | Called when |
|---------------|---------|-------------|
| `__assemble_prior_knowledge__(goal, budget, class)` | JSON PKC for LLM | First turn, or on re-plan |
| `__get_actions__()` | Available tools as JSON list | Before tool dispatch |
| `__list_skills__()` | Active skills | On demand |
| `__retrieve_docs__(goal, max)` | Legacy memory doc list | Fallback path |

**Consequence for BuildInstruction design:**  
`OrchestratorContext` content **cannot be delivered as a static side-blob**. It must flow
through the `__assemble_prior_knowledge__` callback channel that the orchestrator already
calls. The RetrievalEngine must serialize `OrchestratorContext` into the existing PKC format.

---

### 1.2 Rust Executioner Has Zero Pre-Loaded Knowledge

**File:** `crates/brassclaw_host_runtime/src/lib.rs`, `EffectExecutor`

Tools are resolved **on-demand**:
1. Orchestrator sends a tool call (e.g., `"ssh host=example.com"`)
2. Rust looks up the tool in the active lease table (`LeaseManager::active_for_thread()`)
3. Rust reads the ToolSkill from DB (param schema, preconditions, error handling)
4. Rust executes the Tool with validated params
5. Returns output

**Consequence for BuildInstruction design:**  
`RustContext.tool_bindings[]` **cannot be delivered as a free-floating JSON side-channel** today.
It must be serialized into a transient per-turn table that the Rust layer queries before tool dispatch.
OR: injected into the ToolSkill row as a transient `pending_invocation_params JSONB` column.

---

### 1.3 Current PKC Is One Mixed Blob

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs`, `handle_assemble_prior_knowledge`

Today `__assemble_prior_knowledge__` returns one dict:
```json
{
  "content":                     "raw PKC text + KV fingerprint",
  "formatted_content":           "JSON list for LLM working_messages",
  "override_prompt_creation":    false,
  "matched_component_ids":       ["uuid1", "uuid2"]
}
```

It mixes Skills, ToolSkills, Memories, and thread history into one object.

**Consequence:** The orchestrator has no way to distinguish "this is for me" from
"this is for Rust". The user's requirement ("orchestrator gets only what it needs,
Rust gets its compact JSON separately") requires a **three-surface PKC** redesign.

---

### 1.4 v2 MemoryDoc Structure (Legacy)

```rust
pub struct MemoryDoc {
    pub id: DocId,
    pub project_id: ProjectId,
    pub user_id: String,
    pub doc_type: DocType,   // Skill | Plan | ToolSkill | Recipe | Summary | Lesson | Issue | Note | Spec
    pub title: String,
    pub content: String,     // YAML skill manifest for Skill; plain text otherwise
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}
```

The `component_import.rs` already migrates `Spec`, `Plan`, `Summary`, `Lesson`, `Issue`, `Note`
to their v3 tables (classes 12–20). What it **does NOT** migrate:
- `Skill` MemoryDocs → needs expansion into Tool + ToolSkill + Skill + Recipe + ExtensionCatalogue
- `Recipe` MemoryDocs → needs expansion into v3 `RecipeVariant` + StepDescriptions
- `ToolSkill` MemoryDocs → needs migration to `reborn_tool_skills` + ToolBinding schema

---

## 2. Suggested Plan Upgrades

The following sections should be **added to §0** in `saved_plan_to_v3.md`:

---

### §0.14 — StepDescription Authoring Layer (NEW)

**Purpose:** StepDescription is the **human-editable source of truth** for what a Recipe does. It
is authored in the WebUI, stored as YAML, and compiled by the Instruction-Building-System
into a machine-optimized `BuildInstruction` at intent-match time.

**StepDescription is NOT a BuildInstruction.** It describes *what* happens in human terms.
The Instruction-Building-System translates *what* into *how* (typed contexts, UUIDs, error policies).

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
| `code_snippet` | text | Inline Python code (auto-creates PythonCode component on save; sent to Q1 validator) |

#### Storage format

**Chosen: YAML column on `reborn_recipes`** as `step_descriptions JSONB`, loaded as
YAML-structured text inside JSONB. The YAML human-readable format is preserved in the
`step_text` sub-field for WebUI rendering.

**Alternative considered:** YAML files in git (`step_descriptions/<recipe-name>.yaml`).
Added to Open Questions as Q5 — see §3.

#### Multi-StepDescription pattern (variants)

| StepDescription | Contents | Maps to |
|-----------------|----------|---------|
| `Stepdescription0` | Base — most common use-case | Variant `base` |
| `Stepdescription1` | All steps of variant 1 | Variant `ls-la` |
| `Stepdescription2` | All steps of variant 2 | Variant `ls-dir` |

All variants share a **common part** (steps before the divergence point) taken from
`Stepdescription0`, and have an **individual part** from their own `StepDescriptionN`.

#### Example — Recipe `local-files-reading`, Stepdescription0 (partial)

```yaml
recipe_id: "a1b2c3d4-..."
recipe_name: "local-files-reading"
description: "Stepdescription0 — base path (ls -l)"

steps:
  - step_number: 1
    knowledge: "orchestrator"
    goal: "Provide task context"
    content: "Information explaining the task"
    type: "text"
    info: |
      This task is performed by the orchestrator only. An LLM prompt is not created.
      The rust execution layer will be given the tool "ls" and the toolskill "ls".
      The orchestrator will be given the skill "ls" and the PythonCode "ls".
      The orchestrator processes the PythonCode instructions and utilizes the skill
      to instruct the rust executioner. The executioner uses the toolskill to call
      the tool and returns the output to the orchestrator, who writes it to chat.
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
    goal: "Provide PythonCode orchestrator instructions"
    content: "PythonCode \"ls\""
    type: "component"
    include: ["<uuid:pythoncode-ls>"]
    info: |
      Final step. PythonCode tells the orchestrator how to use the ls skill to call
      the rust executioner and what to do with the output (write to chat window).
```

---

### §0.15 — Intent Linking Formula

Every intent registered in `reborn_intent_inputs` carries a **link formula** that
specifies exactly which steps (from which StepDescriptions) to include when building
the `BuildInstruction` for that intent.

#### Notation

```
<desc_id>:<start>-<desc_id>:<end>[+<desc_id>:<start>-<desc_id>:<end>]*
```

Where:
- `<desc_id>` = `0` (base), `1` (variant 1), `2` (variant 2), ...
- `<start>` = step number (1-based) or `0` = first step
- `<end>` = step number or `E` = last step
- `+` = concatenate segments in order

#### Examples

| Link formula | Meaning |
|-------------|---------|
| `0:0-0:E` | All steps of Stepdescription0 (base, all) |
| `0:0-0:30+1:0-1:E` | Base steps 0–30, then all of StepDesc1 |
| `0:0-0:31+2:0-2:E` | Base steps 0–31, then all of StepDesc2 |
| `0:0-0:30+1:0-1:11+3:0-3:E` | Base 0–30, StepDesc1 steps 0–11, all of StepDesc3 |

#### Storage

**Migration V052:** `ADD COLUMN link_formula TEXT` to `reborn_intent_inputs`.

**Example rows after V052:**
```
intent_expression         | component_id  | variant_key | link_formula
"ls -l"                   | <recipe-uuid> | null        | "0:0-0:E"
"ls -la"                  | <recipe-uuid> | "ls-la"     | "0:0-0:30+1:0-1:E"
"ls /tmp"                 | <recipe-uuid> | "ls-dir"    | "0:0-0:31+2:0-2:E"
```

---

### §0.16 — Instruction-Building-System

The **Instruction-Building-System (IBS)** is a new service that translates StepDescriptions
into `BuildInstruction` structs at intent-match time. It is the compiler layer between
the human-editable authoring format and the machine-optimized runtime format.

#### Responsibilities

1. Parse `link_formula` notation → `Vec<(desc_id, start, end)>`
2. Load `StepDescriptionN` for each referenced `desc_id` from `reborn_recipes`
3. Extract steps in the requested range
4. Apply variable substitution (`{{vars.name}}` → runtime values)
5. Separate steps by `knowledge` field → `orchestrator` vs `rust` vs `both`
6. Build typed `OrchestratorContext`:
   - `skill_ids[]`: all UUIDs from `include` where `knowledge == "orchestrator" or "both"` and component type = Skill
   - `python_code_ids[]`: same for PythonCode
   - `control_flow_steps[]`: from `type: "snippet"` or `type: "component"` with control-flow semantics
7. Build typed `RustContext`:
   - `tool_skill_ids[]`: UUIDs from `include` where `knowledge == "rust" or "both"` and component type = ToolSkill
   - `tool_bindings[]`: `ToolBinding { tool_name, params, error_policy }` per Tool UUID
8. Build `fetch_steps[]` (Section A): collect all unique component UUIDs across both sections
9. Return complete `BuildInstruction`

#### Interface

```rust
// crates/brassclaw_engine/src/memory/instruction_builder.rs  (new file)

#[async_trait]
pub trait InstructionBuilder: Send + Sync {
    async fn build_from_formula(
        &self,
        recipe_id: Uuid,
        link_formula: &str,
        user_text: &str,  // For variable extraction via VariablePattern
    ) -> Result<BuildInstruction, InstructionBuilderError>;
}

// PostgresInstructionBuilder implements this trait.
// RamInstructionBuilder returns a minimal BuildInstruction (for tests).
```

#### Where it is called

In `PostgresSource::fetch_for_turn()`, after `resolve_intent` returns a `Match`:

```
resolve_intent → Match { component_id, variant_key, link_formula }
    → InstructionBuilder::build_from_formula(recipe_id, link_formula, user_text)
    → fetch_by_instruction(scope, &build_instruction, user_text, budget)
    → return FetchForTurnResult::Components(patch)
```

#### Caching

The IBS **caches compiled `BuildInstruction`** in-process:
- Key: `(recipe_id, variant_key, variable_hash)` where `variable_hash = SHA256(sorted variable bindings)`
- TTL: 5 minutes (or until affected component is re-validated)
- Invalidated when: any `include`d component's `updated_at` changes

**Risk mitigation:** The cache avoids YAML parsing + formula parsing on every turn for
high-traffic intents. Cache miss is acceptable (<5ms for a simple recipe).

---

### §0.17 — Turn DataFlow Upgrade

#### Current problem

`__assemble_prior_knowledge__` returns one mixed blob containing:
- Orchestrator-facing skills and PythonCode
- Rust-facing ToolSkills (which the orchestrator does not need!)
- Memories and thread history
- All in one `formatted_content` JSON

The orchestrator cannot distinguish "this is for me" from "this is for Rust".

#### Proposed: Three-Surface PKC

The RetrievalEngine serializes `BuildInstruction` into **three typed surfaces**:

| Surface | PRIORITY | Content | Destination |
|---------|----------|---------|-------------|
| `orchestrator_knowledge` | 2 (instruction snippets) | Skills + PythonCode bodies, step instructions | LLM working_messages |
| `memory_knowledge` | 3 (memory snippets) | Thread history, relevant notes | LLM working_messages |
| `rust_knowledge` | — (transient table) | ToolSkill bodies + ToolBinding params | `reborn_pending_rust_context` |

**`__assemble_prior_knowledge__` return shape (upgraded):**

```json
{
  "orchestrator_knowledge": {
    "skill_bodies":        ["<uuid>: <skill content>", ...],
    "python_code_bodies":  ["<uuid>: <pythoncode content>", ...],
    "step_instructions":   "<formatter-rendered instructions for orchestrator>",
    "llm_call_required":   false
  },
  "memory_knowledge": {
    "thread_notes":        ["..."],
    "relevant_memories":   ["..."]
  },
  "rust_pending_id": "<uuid>",  // Row ID in reborn_pending_rust_context
  "override_prompt_creation": true,
  "matched_component_ids": ["uuid1", "uuid2"]
}
```

The orchestrator reads `orchestrator_knowledge` and `memory_knowledge` directly.
It receives `rust_pending_id` but does not read `rust_knowledge` itself — it passes the
pending ID to the Rust layer when it sends the tool invocation.

#### Rust Context Delivery: Transient Table

**Migration V053:** New table `reborn_pending_rust_context`:

```sql
CREATE TABLE reborn_pending_rust_context (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id       TEXT    NOT NULL,
    iteration    INT     NOT NULL,
    tool_skill_ids UUID[]  NOT NULL,
    tool_bindings  JSONB   NOT NULL,  -- serialized Vec<ToolBinding>
    created_at   TIMESTAMPTZ DEFAULT now(),
    UNIQUE(run_id, iteration)
);
```

**Lifecycle:**
1. `RecipeStage` inserts row after `fetch_by_instruction` completes
2. Rust reads row before first tool dispatch
3. Row deleted after turn completes (or 1-hour TTL trigger)

**Benefit:** Rust gets a compact, typed package with exactly the ToolSkill bodies and
ToolBinding params it needs — no LLM prompt content, no skills, no memories.

---

### §0.18 — v2 DocPlan Translation Layer

**Goal:** Automatically expand legacy `MemoryDoc` rows (type `Skill`, `Recipe`, `ToolSkill`)
into the full v3 component graph.

The existing `component_import.rs` already handles `Spec`, `Plan`, `Summary`, `Lesson`,
`Issue`, `Note`. This new layer handles the harder expansion cases.

#### Translation rules

**Skill MemoryDoc → 5 components:**
1. Tool (class 0) — extracted from `param_template.tool_name`
2. ToolSkill (class 13) — param_template + param_schema from metadata
3. Skill (class 1) — body from `content`
4. Recipe (class 21) — skeleton, empty `variants[]`, pending
5. ExtensionCatalogue (class 23) — groups the above, pending

**ToolSkill MemoryDoc → 1 component:**
- ToolSkill (class 13) — direct migration (already handled partially by `component_import.rs`)

**Recipe MemoryDoc → 1 component + N StepDescriptions:**
- Recipe (class 21) — with `trigger` + `steps` (v2 format, used as fallback)
- StepDescriptions seeded from the v2 `RecipeStep[]` (step_number = index, knowledge = "orchestrator", type = "component")

#### All translated components enter the validation queue at `pending`.

#### Suggested CLI command

```bash
brassclaw translate-v2-docs --dry-run   # preview
brassclaw translate-v2-docs --execute   # insert + queue
```

---

## 3. Answers to User's Two Open Questions

### Q: Do the orchestrator and Rust executioner have all basic capabilities built-in?

**Orchestrator:** No. Zero built-in knowledge of tools, skills, or procedures. Every turn
it requests PKC from Rust via `__assemble_prior_knowledge__`. The v3 `OrchestratorContext`
must be serialized into the `formatted_content` surface of that response.

**Rust executioner:** No permanent pre-load. Tools are resolved per-thread via the lease
manager. ToolSkill bodies are DB-fetched at tool-dispatch time. The `RustContext` provides
exactly the ToolSkill bodies and ToolBinding params Rust needs for the current turn —
delivered via the transient `reborn_pending_rust_context` table.

**What they both have built-in (hardcoded):**
- Orchestrator: all 16 host function signatures (`__llm_complete__`, `__execute_action__`, etc.)
- Rust: first-party capability registry (memory, http, shell, json, etc.)
- Rust: ToolSkill schema validation logic
- Rust: lease manager and action executor

**Conclusion:** Both need full prior-knowledge delivery every turn. The StepDescription
authoring layer and Instruction-Building-System provide this in a structured, typed,
and audience-separated way for the first time.

---

### Q: How to translate v2 DocPlans into the new architecture?

See §0.18 above. The recommended approach:

1. **Extend `component_import.rs`** with a new `translate_skills()` function
2. **New Phase J** (after all other phases): CLI command + DB procedure that expands
   `Skill` and `Recipe` MemoryDocs into the full v3 component graph
3. All translated components start at `pending` → Q1 → Q2 → `validated`
4. Original MemoryDocs are marked `archived_at = now()`, not deleted
5. After user review and validation, the archived rows can be hard-deleted

---

## 4. Additions to Open Questions (§3)

**Q5 — StepDescription storage:** YAML files in git vs. JSONB column in `reborn_recipes`?
→ **Recommendation:** JSONB column (simpler, no file management). Store YAML-formatted text
as a text field inside the JSONB for human readability. WebUI renders it as structured YAML.

**Q6 — `step_formatter_id` scope:** Per-recipe, per-variant, or per-step?
→ **Recommendation:** Per-recipe. Formatting style is consistent across a capability domain.

**Q7 — Rust delivery mechanism:** Transient table vs. ephemeral column vs. in-memory cache?
→ **Recommendation:** Transient table `reborn_pending_rust_context` (V053). Most reliable, testable, audit-friendly.

**Q8 — PKC split:** New `__retrieve_memories__` host function vs. three-surface PKC in same response?
→ **Recommendation:** Three-surface PKC (§0.17). No new host function signature needed.

**Q9 — v2 MemoryDoc preservation:** Delete after translation or archive?
→ **Recommendation:** Archive (`archived_at TIMESTAMPTZ`). Keep for forensics and rollback.

---

## 5. Additional Migrations Required

| Migration | Contents |
|-----------|----------|
| `V052__reborn_intent_inputs_link_formula.sql` | `ADD COLUMN link_formula TEXT` to `reborn_intent_inputs` |
| `V053__reborn_pending_rust_context.sql` | New transient table for per-turn Rust prior-knowledge delivery |
| `V054__reborn_recipes_step_descriptions.sql` | `ADD COLUMN step_descriptions JSONB` to `reborn_recipes`; index on `recipe_id` |
| `V055__brassclaw_memory_docs_archived_at.sql` | `ADD COLUMN archived_at TIMESTAMPTZ` to `brassclaw_memory_docs` |

---

*End of review and recommendations.*
