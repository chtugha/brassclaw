# 05 — Skills System

> **Subsystem:** The skills system — four kinds of "skill" that teach the agent how to act:
> Classic Claude-style skills (SKILL.md format, DB-stored), ToolSkills (for the Rust executor),
> Orchestrator Skills (narrative task-pattern guidance), and ExtensionCatalogues (the
> documentation namespace, class 23).
> **Grounded in:** `crates/brassclaw_skills/` (types, parser, v2 — selector/gating/registry/catalog v1-only+dormant), `crates/brassclaw_engine/src/types/recipe.rs` (`ToolSkill`), `crates/brassclaw_engine/src/memory/composition.rs` (`SkillRef`, `ComposedProgram.skills`), `crates/brassclaw_reborn_composition/src/pg_composition_port.rs` + `db_skill_store.rs` + `db_skill_loader.rs`, `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs`, `crates/brassclaw_pg/migrations/V053`/`V070`/`V071`/`V072`, `saved_plan_to_v3.md` §0.1/§0.2/§0.16, Steps C.2/C.4.5.

## 1. Purpose

"Skill" is overloaded in BrassClaw. The v3 system distinguishes **four kinds**, each with a
different class code, storage table, runtime reader, and authoring grain:

1. **Classic Claude-style skills** — the Anthropic `SKILL.md` format (YAML frontmatter + markdown
   prompt body) that feeds the LLM. In v3 the **parts are stored in the database** (`reborn_skills`,
   classes 1–3); an actual `SKILL.md` file does not exist on disk, but can be **exported** via the
   WebUI on demand.
2. **ToolSkills** (class 13) — tight, < 5000-token descriptions of one tool-usage pattern for the
   **Rust execution layer** (param schema, preconditions, error handling). The orchestrator never
   reads ToolSkill bodies.
3. **Orchestrator Skills** — a *role* of a Skill (classes 1–3): narrative instructions for the
   orchestrator on how to perform a task pattern (often spanning multiple tools) with the help of
   the executor. Distinguished from PythonCode by the **grain rule**.
4. **ExtensionCatalogues** (class 23) — the documentation namespace. One per cognitive domain; it
   draws the bigger picture (which Recipes cover which task groups) and never re-documents the
   components it owns.

## 2. Location

- **Skill crate (Classic skills / parsing / selection):** `crates/brassclaw_skills/` —
  `types.rs` (`SkillManifest`, `ActivationCriteria`, `SkillSource`), `parser.rs` (`parse_skill_md`),
  `v2.rs` (`V2SkillMetadata`, `CodeSnippet`, `SkillMetrics`), `selector.rs` (`prefilter_skills`,
  `extract_skill_mentions`), `validation.rs`, `gating.rs`, `registry.rs`, `catalog.rs`.
- **ToolSkill type:** `crates/brassclaw_engine/src/types/recipe.rs` (`ToolSkill`, `ToolSkillParam`,
  `tool_skill_to_memory_doc`).
- **Engine skill tracking:** `crates/brassclaw_engine/src/memory/skill_tracker.rs`.
- **Composition (skill delivery):** `crates/brassclaw_engine/src/memory/composition.rs`
  (`SkillRef`, `ComposedProgram.skills`) + `crates/brassclaw_reborn_composition/src/pg_composition_port.rs`
  — the IBS composes the matched recipe's Skills into the `program.skills` array Monty consults
  while stepping. The retired `default.py` `select_skills()`/`__list_skills__()` scored-keyword
  path is gone; selection is exact (UUIDs from the recipe's `orchestrator_steps[].include`).
- **Migrations:** `V027__reborn_skills.sql` (classes 1–3), `V037__reborn_tool_skills.sql` (class 13),
  `V053__reborn_extension_catalogues.sql` (class 23 — shipped Phase C). Per-class DB-structure
  standardisation through V075 (C.4.5.0–C.4.5.16; the 5 legacy columns dropped from
  `reborn_skills`/`reborn_tool_skills`).
- **Stores (production):** `crates/brassclaw_reborn_composition/src/pg_skill_store.rs`
  (`DbSkillStore`), `pg_tool_skill_store.rs`, `pg_extension_catalogue_store.rs`.
- **Plan:** §0.1 (component hierarchy), §0.2 (ExtensionCatalogue design), §0.16 (builtin
  bootstrap), §0.16.1 (recipe list), Phase C, Phase L.

## 3. Data model — the component hierarchy (§0.1, bottom-up)

```
┌─────────────────────────────────────────────────────────────────┐
│  ExtensionCatalogue (class 23)                                  │
│  Domain overview. task_groups[] → recipe names. Never re-docs.  │
├─────────────────────────────────────────────────────────────────┤
│  Recipe (class 21) — primary intent target (see 03-recipe)      │
├─────────────────────────────────────────────────────────────────┤
│  Skill (classes 1–3)    │  PythonCode (class 22) [NEW]           │
│  Orchestrator instruct. │  Python utilities / inline instruct.  │
│  for using one Rust tool│  for the orchestrator (see 07)        │
├─────────────────────────────────────────────────────────────────┤
│  ToolSkill (class 13) — Rust-layer only. The orchestrator never │
│  reads ToolSkill bodies directly.                                │
├─────────────────────────────────────────────────────────────────┤
│  Tool (class 0) — Rust execution layer only. Opaque to the      │
│  orchestrator. Excluded from all retrieval queries. (see 06)    │
└─────────────────────────────────────────────────────────────────┘
```

### ToolSkill (class 13) — `types/recipe.rs`

```rust
pub struct ToolSkillParam { pub name: String, pub param_type: String, pub description: String, pub required: bool }

pub struct ToolSkill {
    pub name: String, pub tool_name: String, pub description: String,
    pub param_template: serde_json::Value,
    pub param_schema: Vec<ToolSkillParam>,
    pub preconditions: String, pub error_handling: String,
    pub code_snippet: Option<String>, pub category: String,
    // Wilson metrics: usage_count, success_count, failure_count, wilson_lower, tier
    // lifecycle: source, validation_status, validation_errors, review_attempts, …
}
```
Token-budget target < 5000 tokens (agentskills.io progressive disclosure); `RecipeValidator`
enforces the ceiling. `estimated_tokens()` ≈ 4 chars/token. Stored in `reborn_tool_skills` (V037).
Rust-channel only — a ToolSkill UUID in `orchestrator_steps` is a Q1 hard error (see `04-ibs.md`).

### Classic skills (classes 1–3) — `reborn_skills` (V027)

`SkillManifest` (the parsed `SKILL.md` frontmatter):

```rust
pub struct SkillManifest {
    pub name: String, pub version: String, pub description: String,
    pub activation: ActivationCriteria,   // keywords/patterns/tags/exclude_keywords/setup_marker
    pub credentials: Vec<SkillCredentialSpec>,
    pub requires: GatingRequirements,     // binaries/env/companion skills
    pub component_types: ComponentTypeSet, // which execution contexts this skill is available in
}
```
- `class_code` 1 = `skill_rusty`, 2 = `skill_monty`, 3 = `skill_llm` (the runtime that consumes the
  skill body: Rusty capability, Monty VM, or LLM prompt template).
- `ActivationCriteria` caps keywords (20), patterns (5), tags (10); filters short (< 3 char)
  tokens; `setup_marker` is for one-time onboarding skills. Selection must stay **deterministic**
  (no ambient time/network/filesystem in scoring — `brassclaw_skills/AGENTS.md`).
- `source CHECK ('authored','extracted','migrated','imported')` today — **no `'system'`** until
  V057 (FIND-P7-12). The Phase L seeder needs `'system'` (FIND-P6-02).
- The markdown **body** is the prompt text; in v3 it is stored in the DB column
  (`body` / `prior_knowledge_content`) — there is no `SKILL.md` file. The WebUI can **export** a
  row back to `SKILL.md` (frontmatter + body) on demand.

### Orchestrator Skills vs PythonCode — the grain rule (§0.16)

| Use a **Skill** when | Use **PythonCode** when |
|----------------------|--------------------------|
| The orchestrator needs **narrative instructions** for a task pattern spanning one or more tools — a complete capability description. | The component is a **utility helper** used inside another Recipe's orchestrator channel, not a standalone capability. |

`echo`, `time`, `json` → PythonCode helpers. Filesystem/network/memory/skill-management/trigger
patterns → Skills. Both live in the **orchestrator channel** (`orchestrator_steps[]` in the IBS);
the formatter derives a `StepContextSpec` (`Skill` vs `PythonCode`) from `class_code`.

### Recycling — compose recipes from one-tool leaf skills (the v3 library principle)

The grain rule above says *when* to use a Skill vs PythonCode. The **recycling
rule** says *how big* a Skill should be: **as small as practical — at best, the
description of ONE tool usage — so it can be reused across many recipes. Tools
too: one concern each.** The library (the catalog of validated Skills + Tools +
ToolSkills + PythonCode) is the asset; a Recipe is a **composition** of
already-existing library parts. Prefer reusing a library part over authoring a
new one; when a genuinely new capability is needed, add it as a small leaf so
the next recipe can reuse it too. **Never bake a whole procedure into one fat
skill** — split it into leaves the library can recycle.

Two skill grains therefore coexist (the user's two cases):

- **Leaf skill (one tool / one pythoncode)** — the reusable building block;
  describes how to drive the executor to use ONE tool (user case (a): "a
  description how to make the executioner use a tool"). This is the unit of
  reuse. **Author these.**
- **Domain skill (spans tools)** — the bigger picture (user case (b): "an
  explanation about how a filesystem works and what's needed to read/write/
  format/list"). A domain skill **references** leaf skills by name; it does
  **not** re-duplicate their tool instructions. One per task area; do not
  proliferate.

The ExtensionCatalogue (class 23, below) is the level above the domain skill and
likewise never re-documents its children. The Phase L builtin bootstrap (§4.7)
seeds the first ~85–90 library components into 5 catalogues — the starter
library every recipe composes from. `DOC_CONVERSION_MECHANISM_DESIGN.md` §4.0
applies this concretely: the `doc-sync` mechanism is one domain skill + many
reusable leaves, composed by one recipe + one action.

### ExtensionCatalogue (class 23) — `reborn_extension_catalogues` (V053, Phase C)

A documentation container that organises a capability domain. It **does not re-document**
commands — every owned component already documents itself. It draws the bigger picture:

| Section | Content |
|---------|---------|
| `name`/`version`/`description` | catalogue identifier; one-paragraph summary for LLM fallback context |
| `overview_doc` | primary text field (maps to `effective_content` via `COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)`) |
| `task_groups[]` | `{ group_name, summary, recipe_ids[] }` |
| `child_component_ids[]` | all owned component UUIDs (any class) for lineage |
| `intent_index[]` | **audit-only — never seeded into `reborn_intent_inputs`** |

DDL (V053): scope tuple, `name` (1–256), `description` (≤1024), `version` default `'1.0'`,
`overview_doc`, `task_groups` JSONB, `child_component_ids` UUID[], `intent_index` JSONB,
solution-override columns (`prior_knowledge_content`, `override_prompt_creation`), `class_code=23`,
`prompt_uid` sequence, `consumer_tags`, `intent_examples`, `validation_status`, `source`
(`CHECK ... 'system'` from day one — FIND-P6-02), `dependency_registry` JSONB from day one
(Phase J.2). **No** `queue_code`/`review_attempts`/`review_feedback`/`rejected_at`/
`validation_errors` columns — those five are centralised on `reborn_validation_queue` (V051,
§0.18). Indexes: scope, scope+status, scope+prompt_uid (UNION ALL), consumer_tags GIN,
similarity_parent, replaces. Default consumer tags `{02:orchestrator, 05:validator}`.

## 4. Behavior / flow

1. **Authoring:** a Classic skill is authored in the WebUI (frontmatter fields + markdown body) →
   `reborn_skills`; a ToolSkill in `reborn_tool_skills`; an ExtensionCatalogue in
   `reborn_extension_catalogues`. On save the WebUI submits each new component to the validation
   queue (`ValidationQueueStore::submit(scope, component_id, class)`); the ExtensionCatalogue save
   path calls `submit(scope, id, 23)` on creation.
2. **Selection (intent-driven, exact):** `host.resolve_intent` → a `Match` carries the recipe
   `component_id` + `step_link`. `host.compose_orchestrator` composes the recipe: the IBS reads
   `orchestrator_steps[].include` UUIDs and fetches the exact Skills + PythonCode + ToolSkills.
   Selection is **exact (UUIDs), not scored** — the intent system already resolved the match; the
   retired `default.py` `select_skills()`/`__list_skills__()` keyword-scoring path is gone.
3. **Skills as a first-class array (the v3 role):** the composed `program.skills` is a
   `Vec<SkillRef { id, class_code, name, body }>` Monty **consults while stepping**. Each skill
   carries the **exact usage of one or more tools**, so a `steplist` step need only name the
   approach + carry its `executable_code` — the tool-call detail lives in the skills, not the
   steplist. This is the point of the recycling rule (§3): small leaf skills (one tool each) are
   reused across many recipes, and the steplist stays lean. Monty does **not** bake skills into a
   static prefix; it reads the array as it works through the steps.
4. **ToolSkill at runtime:** the IBS routes ToolSkill UUIDs to `rust_steps[]` → composed as
   `rust_directives`/`tool_bindings`; the Executioner applies them (cdylib load via the C.3
   `DynamicToolLoader`); the Orchestrator never sees the ToolSkill body (a ToolSkill UUID in
   `orchestrator_steps` is a Q1 hard error).
5. **ExtensionCatalogue at runtime:** surfaced as LLM fallback context (the domain overview) and
   as the grouping for the builtin bootstrap; its `intent_index` is audit-only (never an intent
   input).
6. **SKILL.md export:** the WebUI reconstructs `SKILL.md` (frontmatter YAML + markdown body) from a
   `reborn_skills` row on demand — no on-disk file exists otherwise.
7. **Builtin bootstrap (shipped, C.2):** the builtin host.* seed seeds the system Skills/
   ToolSkills/PythonCode/Recipes idempotently at boot (`source='system'`,
   `validation_status='validated'`). The Phase-L `~85–90 component` library across 5 catalogues
   (`builtin-filesystem`, `builtin-network`, `builtin-memory`, `builtin-process`,
   `builtin-management`) is the starter set recipes compose from.

## 5. Relations

- **Recipe System** (`03`): recipes reference Skills/ToolSkills/PythonCode by UUID in step
  `include`; the grain rule decides Skill vs PythonCode per step.
- **IBS** (`04`): routes class 13 → `rust_steps`; classes 1–3/22 → `orchestrator_steps`;
  `StepContextSpec` derives the formatter heading from `class_code`.
- **Tools** (`06`): a ToolSkill binds to a Tool (class 0) via `tool_name`/`tool_id`; `capability_id`
  (Phase L) links the Tool row to its Rust handler.
- **PythonCode** (`07`): the orchestrator-channel sibling governed by the grain rule.
- **Validation Queue** (`14`): every authored skill/catalogue enters Q1/Q2; `source='system'`
  bypasses Q2.
- **Component Catalog** (`15`): `reborn_skills`/`reborn_tool_skills`/`reborn_extension_catalogues`
  are the class-code component tables; `DocType` is frozen (no new variants — §0.11 FINDING B).

## 6. Status — shipped vs. pending

**Shipped:**
- `reborn_skills` (V053 standardisation, dropping the 5 legacy `brassclaw_skills` columns V072),
  `reborn_tool_skills` (V037, includes column V070), `reborn_extension_catalogues` (V053) are live
  and carry real rows.
- **Builtin bootstrap (C.2):** the builtin host.* seed seeds system Skills/ToolSkills/PythonCode/
  Recipes idempotently at boot (`source='system'`, `validation_status='validated'`). `'system'` is
  in every `source` CHECK.
- **`DbSkillStore`** reads the unified `reborn_skills` shape (no legacy-column fallback); the
  `db_skill_loader` feeds skills into composition. The `brassclaw_skills` v1 `selector`/`gating`/
  `registry`/`catalog` are v1-only and dormant (the v2 selection they served lived in the retired
  `default.py`; both are gone).
- **ExtensionCatalogue** (`pg_extension_catalogue_store.rs`, V053) + class-23 validator arm +
  `fetch_for_consumer` UNION ALL arm (Phase E) are live.
- **Skills as a first-class array** is the composed-`program.skills` (`SkillRef`) shape shipped in
  C.4.5.17 (`ComposedProgram`); Monty consults it while stepping (§4.3).
- `capability_id` on `reborn_tools` (V071) links each Tool row to its Rust handler.
- `DocType` is `#[deprecated]` and frozen — classes 22/23 are integer-class-code only; ordering is
  automatic via `class_code ASC, prompt_uid ASC`.

**Pending:**
- **WebUI (Phase K.1):** SKILL.md export + skill/ToolSkill/ExtensionCatalogue authoring UI.
- **C.5/C.6 driver wiring:** the composition side is built but the engine Monty VM
  `execute_orchestrator` host-call path is dormant in production (the active Tier-0/Tier-1 path is
  the TURNS `PgOrchestratorLookup` bridge). The C.5/C.6 driver activates `host.compose_orchestrator`
  + `host.run_program` in production and applies `rust_directives` via the `DynamicToolLoader`.

## 7. LLM-relevant summary

Four skill kinds: **Classic skills** (`SKILL.md` format, `reborn_skills` classes 1–3 —
`skill_rusty`/`skill_monty`/`skill_llm`, DB-stored frontmatter+body, WebUI-exportable to `SKILL.md`,
no on-disk file); **ToolSkills** (class 13, `reborn_tool_skills`, tool-usage patterns,
Rust-channel only — composed into `rust_directives`, orchestrator never reads them);
**Orchestrator Skills** (the narrative task-pattern role of a class 1–3 Skill — grain rule: Skill =
capability spanning tools, PythonCode = sub-orchestrator helper); **ExtensionCatalogues** (class 23,
`reborn_extension_catalogues` V053, documentation namespace, one per domain,
`overview_doc`+`task_groups`+`child_component_ids`, `intent_index` audit-only). Selection is
**deterministic and exact (UUIDs)** via `host.resolve_intent` → `host.compose_orchestrator`; the
retired `default.py` keyword-scoring path is gone. Skills ride in `program.skills` as a first-class
`Vec<SkillRef>` Monty consults while stepping — they carry the exact tool usage so the steplist stays
lean. `DocType` is frozen (no 22/23 variants); ordering is automatic via `class_code ASC,
prompt_uid ASC`. The builtin bootstrap (C.2) seeds system components idempotently at boot; the
Phase-L `~85–90 component` library across 5 ExtensionCatalogues is the starter set recipes compose
from. C.5/C.6 activates the composition host-calls in production.
