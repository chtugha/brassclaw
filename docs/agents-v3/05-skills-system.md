# 05 — Skills System

## Binding Recipe architecture (v3)

Read [recipe.md](../../recipe.md) before authoring or changing components. This contract
supersedes older examples below where they conflict; it specifies the target,
not completed runtime or database functionality.

- Rust Tools supply primitives; many ToolSkills describe their IBS bindings;
  many Skills explain one Tool usage and have associated executable PythonCode;
  many small PythonCode components provide reusable executable building blocks.
  Recipes tell the orchestrator how to use them to fulfill task goals. Prefer
  explicit reusable steps, not fewer steps or specialized Rust workflow Tools.
- Each Recipe component step references exactly one stable component UUID.
  PythonCode may internally compose smaller PythonCode components; this is not
  a multi-component Recipe step. Keep all independent Tool calls in separate
  execution steps; the existing direct dependent-chain exception still applies.
- IBS/composition reads the newest activated, approved versions at task start
  from one consistent catalogue snapshot and pins exact UUID/version/checksum
  references in BuildInstruction, including nested dependencies. Recipes carry
  no version numbers. Execution, child steps, waits and resumption retain that
  selection; do not look up latest again during the task.
- Approved versions are immutable. Changes create new versions; authored
  versions pass Q1 and human Q2 before activation. Replacement neither deletes
  nor invalidates originals used by running/suspended tasks. Current global
  Tool policy is checked independently before every dispatch.
- Inputs and results are typed data. Use the exact input-reference grammar and
  step-local binding convention in recipe.md. Runtime values never become
  Python source. Monty owns each task's intermediate results; unrelated tasks
  and attempts stay isolated, including across child execution and waits.
- Rust-channel ToolSkill binding executes nothing and grants no permission.
  Orchestrator-channel PythonCode calls host.<tool>(...). Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.

Current code still has plain text substitution, fresh state in nested step
execution and incomplete immutable version manifests/binding preparation. The
new typed inputs interface and strict single-component validation require
implementation and production-path acceptance; do not claim these are shipped.

## Binding Skill definition (v3)

A **Skill** is one reusable tool-usage pattern for the Orchestrator. It comprises
**both prose instructions and explicitly associated executable PythonCode**:
the prose explains purpose, parameters, prerequisites and result/error handling;
the PythonCode implements that usage. “Leaf Skill” means this same unit, not a
different kind of Skill. A broader domain or multi-tool overview belongs to an
**Extension**, documented by its ExtensionCatalogue; a **Recipe** defines the
ordered workflow and references reusable components by UUID.

**Tool + ToolSkill belong to the Rust side.** The Tool provides the primitive;
the ToolSkill describes its IBS binding. Binding executes nothing and grants no
permission. The Orchestrator executes the associated PythonCode, which calls
`host.<tool>(...)`; the kernel checks the current global tool policy.

**Storage is not the definition:** today Skill prose is stored in `reborn_skills`
(classes 1–3), while executable code is stored separately in `reborn_python_code`
(class 22). The current composer emits `SkillRef.body` and
`ComposedStep.executable_code` separately. The target requires an explicit,
validated UUID/revision association between the two parts; separate rows are
permitted and do not make the Skill prose-only. A code example inside prose is
documentation, not an implicit executable entry point. The class labels
`skill_rusty`, `skill_monty`, `skill_llm` are existing consumer classifications,
not leaf/domain hierarchy levels. Classes 10 and 50 are Orchestrator/Scaffold
records sharing the table, not additional tool-usage Skill types.

**Execution and validation:** deterministic Tier-0 execution uses the associated
PythonCode without an LLM interpreting prose. Tier 1 can use the prose in its
explicit LLM steps. Prose must never be executed as Python. ToolSkill UUIDs stay
in `channel:"rust"`; executable PythonCode UUIDs stay in
`channel:"orchestrator"`. Q1/Q2, isolation between unrelated tasks/attempts and the existing dependent-chain
exception remain applicable. Monty preserves needed state within each Recipe. This documentation change does not implement a
new database schema, association editor or runtime path.


> **Subsystem:** The Skill system and its storage/binding neighbours:
> Classic Claude-style skills (SKILL.md format, DB-stored), ToolSkills (for the Rust executor),
> Orchestrator Skills (prose plus associated PythonCode), and ExtensionCatalogues (the
> documentation namespace, class 23).
> **Grounded in:** `crates/brassclaw_skills/` (types, parser, v2 — selector/gating/registry/catalog v1-only+dormant), `crates/brassclaw_engine/src/types/recipe.rs` (`ToolSkill`), `crates/brassclaw_engine/src/memory/composition.rs` (`SkillRef`, `ComposedProgram.skills`), `crates/brassclaw_reborn_composition/src/pg_composition_port.rs` + `db_skill_store.rs` + `db_skill_loader.rs`, `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs`, `crates/brassclaw_pg/migrations/V053`/`V070`/`V071`/`V072`, `saved_plan_to_v3.md` §0.1/§0.2/§0.16, Steps C.2/C.4.5.

## 1. Purpose

There is one architectural Skill definition, given above: one tool-usage
pattern with prose and associated PythonCode. `SKILL.md` is an import/export
representation, not another architectural Skill kind. ToolSkill is a Rust-side
binding descriptor. ExtensionCatalogue documents the wider Extension context.

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
│  Prose usage part       │  Associated executable usage part    │
│  One tool pattern       │  Also standalone pure helpers (07)   │
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
- The markdown **body** is the prose part of the Skill; it is stored in the DB column
  (`body` / `prior_knowledge_content`) — there is no `SKILL.md` file. The WebUI can **export** a
  row back to `SKILL.md` (frontmatter + body) on demand.

### Skill parts and reuse

A Skill has two complementary parts, not a choice between prose and code.
Author the prose for one tool-usage pattern and associate the reusable class-22
PythonCode UUID/revision implementing it. Pure-logic PythonCode helpers remain
valid standalone components; they need not be mislabeled as tool-usage Skills.
Recipes supply ordering and data handoff. Extensions supply the larger domain
context through their ExtensionCatalogue and reference the Skills/Recipes.

The current formatter/composer distinguishes class-1–3 prose from class-22 code.
This is a storage/execution distinction within the complete usage unit. Do not
infer a multi-tool Skill hierarchy from class codes or from the separate tables.
The doc-sync example therefore comprises an Extension overview, reusable Skills
with associated code, Recipes and an Action; its overview is not another Skill.

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

1. **Current prose authoring:** a Skill prose row is authored in the WebUI (frontmatter fields + markdown body) →
   `reborn_skills`; a ToolSkill in `reborn_tool_skills`; an ExtensionCatalogue in
   `reborn_extension_catalogues`. On save the WebUI submits each new component to the validation
   queue (`ValidationQueueStore::submit(scope, component_id, class)`); the ExtensionCatalogue save
   path calls `submit(scope, id, 23)` on creation.
2. **Selection (intent-driven, exact):** `host.resolve_intent` → a `Match` carries the recipe
   `component_id` + `step_link`. `host.compose_orchestrator` composes the recipe: the IBS reads
   `orchestrator_steps[].include` UUIDs and fetches the exact Skills + PythonCode + ToolSkills.
   Selection is **exact (UUIDs), not scored** — the intent system already resolved the match; the
   retired `default.py` `select_skills()`/`__list_skills__()` keyword-scoring path is gone.
3. **Current representation:** `program.skills` carries Skill prose as
   `SkillRef { id, class_code, name, body }`; `steplist[].executable_code` carries
   resolved class-22 PythonCode. `executable_code` is a step field, not a field
   on `SkillRef`. The architectural Skill comprises both parts. The explicit
   validated Skill–PythonCode association remains an implementation requirement;
   do not claim that the current arrays alone prove it is implemented.
4. **ToolSkill at runtime:** the IBS routes ToolSkill UUIDs to `rust_steps[]` → composed as
   `rust_directives`/`tool_bindings`; the Executioner applies them (cdylib load via the C.3
   `DynamicToolLoader`); the Orchestrator never sees the ToolSkill body (a ToolSkill UUID in
   `orchestrator_steps` is a Q1 hard error).
5. **ExtensionCatalogue at runtime:** surfaced as LLM fallback context (the domain overview) and
   as the grouping for the builtin bootstrap; its `intent_index` is audit-only (never an intent
   input).
6. **Current prose export:** the WebUI reconstructs `SKILL.md` (frontmatter YAML + markdown body) from a
   `reborn_skills` row on demand — no on-disk file exists otherwise.
7. **Builtin bootstrap (shipped, C.2):** the builtin host.* seed seeds the system Skills/
   ToolSkills/PythonCode/Recipes idempotently at boot (`source='system'`,
   `validation_status='validated'`). The Phase-L `~85–90 component` library across 5 catalogues
   (`builtin-filesystem`, `builtin-network`, `builtin-memory`, `builtin-process`,
   `builtin-management`) is the starter set recipes compose from.

## 5. Relations

- **Recipe System** (`03`): recipes reference Skills/ToolSkills/PythonCode by UUID in step
  `include`; the composer routes the prose and executable parts separately.
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

A Skill is one reusable tool-usage pattern containing prose instructions and
associated executable PythonCode. Current storage separates the prose
(`reborn_skills`, classes 1–3) from code (`reborn_python_code`, class 22).
Consumer labels are not hierarchy levels. ToolSkill (class 13) binds the Rust
Tool and never executes it. Recipe (class 21) orders the workflow; Extension
and its ExtensionCatalogue (class 23) hold the wider domain context. Explicit
Skill–PythonCode UUID/revision association, validation and UI presentation are
target requirements; this documentation change does not implement them.
