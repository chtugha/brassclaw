# 05 — Skills System

> **Subsystem:** The skills system — four kinds of "skill" that teach the agent how to act:
> Classic Claude-style skills (SKILL.md format, DB-stored), ToolSkills (for the Rust executor),
> Orchestrator Skills (narrative task-pattern guidance), and ExtensionCatalogues (the
> documentation namespace, class 23).
> **Grounded in:** `crates/brassclaw_skills/` (types, parser, v2, selector), `crates/brassclaw_engine/src/types/recipe.rs` (`ToolSkill`), `crates/brassclaw_engine/src/memory/skill_tracker.rs`, `crates/brassclaw_pg/migrations/V027`/`V037`/`V053`, `saved_plan_to_v3.md` §0.1/§0.2/§0.16, Phases C/L.

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
- **Python orchestrator selection:** `orchestrator/default.py` `select_skills()` / `__list_skills__()`
  (v2 selection happens in Python, not Rust — `brassclaw_skills` is used only for types + parsing).
- **Migrations:** `V027__reborn_skills.sql` (classes 1–3), `V037__reborn_tool_skills.sql` (class 13),
  `V053__reborn_extension_catalogues.sql` (class 23 — Phase C, **not yet created**).
- **Stores (production):** `crates/brassclaw_reborn_composition/src/pg_skill_store.rs`,
  `pg_tool_skill_store.rs`, `pg_extension_catalogue_store.rs` (Phase C, new).
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
   queue (`ValidationQueueStore::submit(scope, component_id, class)`); the ExtensionCatalogue
   Phase-C save path must call `submit(scope, id, 23)` on creation.
2. **Selection (today, Tier 2):** `default.py` step 0 calls `__list_skills__()` then
   `select_skills(all_skills, goal, ...)` — keyword scoring. The Rust `selector` (`prefilter_skills`)
   is v1-only and deprecated for v2. Activation criteria (keywords/patterns/tags) gate candidates;
   installed skills are lower-trust than user/workspace skills (tool-ceiling attenuation).
3. **v3 selection (intent-driven):** a `Match` → recipe → the IBS emits `orchestrator_steps[]`
   whose `include` UUIDs fetch the exact Skills + PythonCode; `matched_component_ids` drives
   `_set_active_skills`. Selection is **exact** (UUIDs), not scored — the intent system already
   resolved the match.
4. **ToolSkill at runtime:** the IBS routes ToolSkill UUIDs to `rust_steps[]`; `RecipeStage` applies
   them silently to the Rust execution context; the orchestrator never sees the body.
5. **ExtensionCatalogue at runtime:** surfaced as LLM fallback context (the domain overview) and
   as the grouping for the builtin bootstrap; its `intent_index` is audit-only (never an intent
   input).
6. **SKILL.md export:** the WebUI reconstructs `SKILL.md` (frontmatter YAML + markdown body) from a
   `reborn_skills` row on demand — no on-disk file exists otherwise.
7. **Builtin bootstrap (Phase L):** seeds `~23 Tools + 23 ToolSkills + 12–15 Skills + 4–5
   PythonCode + 18–20 Recipes + 5 ExtensionCatalogues` (≈ 85–90 components), all
   `source='system'`, `validation_status='validated'`, grouped into **5 ExtensionCatalogues** by
   cognitive domain (`builtin-filesystem`, `builtin-network`, `builtin-memory`,
   `builtin-process`, `builtin-management`). Idempotent (only if the scope has no existing
   builtin components).

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

## 6. Status — today vs. v3

**Today:**
- `brassclaw_skills` crate exists with `SkillManifest`, `parse_skill_md`, `V2SkillMetadata`,
  `selector`, `gating`, `registry`, `catalog` — but `lib.rs` notes `selector`/`gating`/`registry`/
  `catalog` are **v1-only** (remove after migration); v2 selection lives in `default.py`.
- The `ToolSkill` struct exists (`types/recipe.rs`); `reborn_skills` (V027) and
  `reborn_tool_skills` (V037) tables are live and structurally ready but contain **zero builtin
  rows** (Phase L seeds them).
- `reborn_skills.source` CHECK has **no `'system'`** (V057 adds it — FIND-P7-12).
- **ExtensionCatalogue does not exist** — `V053` migration and `pg_extension_catalogue_store.rs`
  are Phase C work.
- No `capability_id` on `reborn_tools` (Phase L/V057).
- No SKILL.md DB→file export UI.
- `DocType` is `#[deprecated]` and frozen — classes 22/23 are integer-class-code only (§0.11
  FINDING B); `doc_type_weight(DocType)` has no 22/23 arms (obsolete — `ORDER BY class_code ASC`
  orders automatically).

**v3 plan adds:**
- **Phase C (V053):** `reborn_extension_catalogues` + `pg_extension_catalogue_store.rs`; the
  validator gains a class-23 arm using `GenericComponent { name, description, content }` +
  `extra: Option<serde_json::Value>` for `task_groups` (COMP-04; FIND-P10-03 — `GenericComponent`
  does NOT need `class_code`, it's implicit from the dispatch arm). `class_label` gains
  `23 => "extension_catalogue"`.
- **Phase L (V057):** the builtin bootstrap seeder inserts all system skills/ToolSkills/catalogues
  (`source='system'`, `validation_status='validated'`); V057 adds `'system'` to the `source` CHECK
  on `reborn_tools`/`reborn_tool_skills`/`reborn_skills`/`reborn_recipes`.
- **Phase E:** `fetch_for_consumer` UNION ALL gains class-23 (and 22) arms; `class_code_to_table`
  includes the `10 | 50` arm (FIND-NEW-AUDIT-03). Ordering is `class_code ASC, prompt_uid ASC`.
- **WebUI (Phase K.1):** SKILL.md export + skill/ToolSkill/ExtensionCatalogue authoring UI.

## 7. LLM-relevant summary

Four skill kinds: **Classic skills** (`SKILL.md` format, `reborn_skills` classes 1–3 —
`skill_rusty`/`skill_monty`/`skill_llm`, DB-stored frontmatter+body, WebUI-exportable to `SKILL.md`,
no on-disk file); **ToolSkills** (class 13, `reborn_tool_skills`, < 5000-token tool-usage patterns,
Rust-channel only, orchestrator never reads them); **Orchestrator Skills** (the narrative task-pattern
role of a class 1–3 Skill — grain rule: Skill = capability spanning tools, PythonCode = sub-orchestrator
helper); **ExtensionCatalogues** (class 23, `reborn_extension_catalogues` V053, documentation
namespace, one per domain, `overview_doc`+`task_groups`+`child_component_ids`, `intent_index`
audit-only). Selection is deterministic; today it is keyword scoring in `default.py`
(`brassclaw_skills` selector is v1-only), in v3 it is exact UUID fetch from the IBS. `DocType` is
frozen (no 22/23 variants); ordering is automatic via `class_code ASC, prompt_uid ASC`. The builtin
bootstrap (Phase L) seeds ≈ 85–90 system components into 5 ExtensionCatalogues, all
`source='system'`/`validated`; V057 adds `'system'` to the source CHECKs. Phase C creates the
ExtensionCatalogue table + store + class-23 validator arm; Phase E adds the retrieval UNION ALL arm.
