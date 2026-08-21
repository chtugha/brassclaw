# 15 — Component Catalog (class codes 0–23)

> **Subsystem:** The unified set of agent components — every versioned,
> validated, retrievable unit the agent is made of. Each component lives in
> a class-specific Postgres table keyed by a numeric `class_code` (0–23),
> carries a per-table `prompt_uid` sequence (the stable retrieval/prefix
> ordering key), and is gated by `validation_status`. This doc is the
> catalog: the class-code taxonomy, the common table shape, the
> content-column variations, the legacy `MemoryDoc` → class-table
> migration, and the two new v3 classes (22 PythonCode, 23
> ExtensionCatalogue).
> **Grounded in:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
> (`PostgresSource::fetch_for_consumer` UNION ALL :283-441,
> `fetch_component_by_id` table-and-content dispatch :573-627,
> `doc_type_to_class_code` :693),
> `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
> (`COMPONENT_TABLES:47`, `class_label:65`),
> `crates/brassclaw_reborn_composition/src/component_import.rs`
> (legacy `MemoryDoc` → class-table importer),
> `crates/brassclaw_pg/migrations/V036__reborn_specs.sql` (canonical
> content-table shape), `saved_plan_to_v3.md` §0.1 hierarchy (239-272),
> §0.2 ExtensionCatalogue (276-290), §0.5 StepContextSpec (746-793),
> §0.11 cognitive weights (1395-1431), §2 migration sequence (6761-6776).

## 1. Purpose

A BrassClaw agent is not a monolithic prompt — it is a **library of
components**, each owned by a class-specific table, each independently
versioned, validated, and retrievable. The catalog is what retrieval
(`PostgresSource`), the prefix-base-prompt assembler (`do_reassemble`),
and the IBS (`build_instruction`) all draw from. The user's Task 3
"selected memories" and the match-path "recipe" are both catalog
components; the "base prompt … contains every information, documentation,
explanation, recipe etc. of the complete agent" is the validated catalog
reassembled.

Two facts frame the whole subsystem:

1. **Postgres is mandatory.** Every component is a row. There is no
   DB-less component storage (the legacy filesystem/`MemoryDoc` path was
   migrated away — §4). `RamSource` (keyword retrieval over a `Store`)
   is keyword-retrieval *over Postgres*, not a postgres-less path, and is
   deleted in v3 Phase K.3 (see `11-retrieval-system.md`).
2. **The class code is the primary axis.** `class_code` decides the table,
   the content column, the validator arm, the formatter heading, and the
   retrieval sort position. It is an integer (the `DocType` enum is
   `#[deprecated]` and frozen — `FINDING B`; new classes 22/23 use integer
   dispatch only, never a `DocType` variant).

## 2. Location

### Class-code taxonomy (live + planned)

| class | Label | Table | Migration | Content column | Status |
|------:|-------|-------|----------|---------------|--------|
| 0 | Tool | `reborn_tools` | V030 | *(none — no prompt text, excluded from retrieval)* | live |
| 1–3 | Skill (`skill_rusty`/`monty`/`llm`) | `reborn_skills` | V027 | `body` | live |
| 4–9 | Extension (MCP / capability / monty-plan / LLM-template) | `reborn_extensions_unified` | V032 | `description` | live |
| 10 | Orchestrator | `reborn_orchestrators` | *(future migration)* | `body` (via skills table — see §3) | live-graceful-skip |
| 12 | Spec | `reborn_specs` | V036 | `content` | live |
| 13 | ToolSkill | `reborn_tool_skills` | V037 | `content` (Rust-channel only) | live |
| 14 | Plan | `reborn_plans` | V038 | `content` | live |
| 15 | Summary | `reborn_summaries` | V039 | `content` | live |
| 16 | Action | `reborn_actions` | V029 | `description` (steps is JSONB) | live |
| 17 | Docu | `reborn_docus` | V040 | `content` | live |
| 18 | Lesson | `reborn_lessons` | V041 | `content` | live |
| 19 | Issue | `reborn_issues` | V042 | `content` | live |
| 20 | Note | `reborn_notes` | V043 | `content` | live |
| 21 | Recipe | `reborn_recipes` | V033 | `''` (no plain content — `steps` is JSONB; PKC only) | live |
| 22 | PythonCode | `reborn_python_code` | V052 (planned) | `content` | **v3 NEW** |
| 23 | ExtensionCatalogue | `reborn_extension_catalogues` | V053 (planned) | `overview_doc`→`content` | **v3 NEW** |
| 50 | Scaffold | `reborn_scaffolds` | *(future migration)* | `body` (via skills table) | live-graceful-skip |

### Code locations

- **`COMPONENT_TABLES` constant** — `interceptor_config_service.rs:47` —
  the 15-entry `&[(&str, u16)]` list `do_reassemble` walks to build the
  Sempai base prompt. Includes the two *future* tables
  (`reborn_orchestrators` class 10, `reborn_scaffolds` class 50); both are
  skipped gracefully when absent (`information_schema.tables` check, :214).
- **`class_label`** — `interceptor_config_service.rs:65` — class-code →
  header label for the reassembled `## {class}:{prompt_uid} {name}` blocks.
- **`fetch_component_by_id` content dispatch** —
  `retrieval_source.rs:573-627` — per-class `(table, content_expr)` arm.
- **`fetch_for_consumer` UNION ALL** — `retrieval_source.rs:283-441` —
  the single 12-sub-select query (PERF-05; becomes 14 after classes 22/23).
- **`doc_type_to_class_code`** — `retrieval_source.rs:693` — legacy
  `DocType` → `(class_code, label)` for the DB-less keyword path.
- **`component_import.rs`** — legacy `brassclaw_memory_docs` (V016) →
  class-table importer (§4).

## 3. Data Model

### Common content-table shape (V036 canonical — shared by classes 12–21)

```sql
CREATE TABLE reborn_{class} (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id               TEXT NOT NULL,
    user_id                 TEXT NOT NULL,
    agent_id                TEXT NOT NULL,
    project_id              TEXT NOT NULL,
    name                    TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 256),
    description             TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 1024),
    content                 TEXT NOT NULL DEFAULT '',
    prior_knowledge_content TEXT,                       -- SCH-02: PKC override text
    override_prompt_creation BOOLEAN NOT NULL DEFAULT false,
    class_code              SMALLINT NOT NULL DEFAULT {n} CHECK (class_code = {n}),
    prompt_uid              BIGINT NOT NULL DEFAULT nextval('reborn_{class}_prompt_uid_seq'),
    consumer_tags           TEXT[] NOT NULL DEFAULT '{}',
    intent_examples         JSONB,
    validation_status       TEXT NOT NULL DEFAULT 'pending' CHECK (... 8 states ...),
    validation_errors       TEXT[] NOT NULL DEFAULT '{}',   -- → queue (V059)
    review_feedback         TEXT,                            -- → queue (V059)
    review_attempts        SMALLINT NOT NULL DEFAULT 0,      -- → queue (V059)
    rejected_at             TIMESTAMPTZ,                     -- → queue (V059)
    queue_code              TEXT,                            -- → queue (V059)
    source                  TEXT NOT NULL DEFAULT 'migrated',
    content_hash            TEXT,
    similarity_parent_id    UUID,                            -- lineage
    replaces_id            UUID,                             -- lineage
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);
```

Key fields:

- **`prompt_uid`** — a per-table BIGINT sequence. This is the **stable
  ordering key** for retrieval (`ORDER BY class_code ASC, prompt_uid ASC`)
  and for the reassembled base prompt. It never changes for a row, so the
  prefix-cache prefix stays stable across edits.
- **`prior_knowledge_content`** — when non-empty, it is used as the
  component's prior-knowledge text *instead of* assembling from `content`
  (the SCH-02 fix). The content-column dispatch below resolves this.
- **`override_prompt_creation`** — the **Solution Override** path: this
  component's PKC *replaces* the standard assembly (see `08-actions-system.md`).
- **`consumer_tags`** — `02:orchestrator`, `03:llm`, `05:validator`. The
  SEC-01 gate excludes `05:validator` rows from retrieval
  (`'05:validator' != ALL(consumer_tags)`). `consumer_tag` membership also
  filters `fetch_for_consumer` (`$5 = ANY(consumer_tags)`).
- **`source`** — `'authored'`/`'extracted'`/`'migrated'`/`'imported'`/
  `'system'` (the last added in V057 for the builtin bootstrap seeder,
  `FIND-P6-02`).
- **Lineage** — `similarity_parent_id` / `replaces_id` track the
  self-improvement ancestry (a Sempai-proposed skill derived from another).

### Content-column dispatch (`fetch_component_by_id` :573)

The `effective_content` expression varies by content column:

| Classes | Table | `effective_content` |
|---------|-------|---------------------|
| 1–3 | `reborn_skills` | `COALESCE(NULLIF(prior_knowledge_content,''), body)` |
| 4–9 | `reborn_extensions_unified` | `COALESCE(prior_knowledge_content, description)` |
| 10 \| 50 | `reborn_skills` | `COALESCE(NULLIF(prior_knowledge_content,''), body)` |
| 12–15, 17–20 | `reborn_{specs\|tool_skills\|plans\|summaries\|docus\|lessons\|issues\|notes}` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 16 | `reborn_actions` | `COALESCE(prior_knowledge_content, description)` (steps is JSONB) |
| 21 | `reborn_recipes` | `COALESCE(NULLIF(prior_knowledge_content,''), '')` (no plain content) |
| 0 | `reborn_tools` | *(no arm — returns empty; no prompt text)* |
| 22, 23 | — | **absent today** (v3 adds the arms; see §6) |

**Note on class 10/50:** `fetch_component_by_id` dispatches classes 10
(Orchestrator) and 50 (Scaffold) to `reborn_skills` with the `body`
content expression. Their dedicated tables (`reborn_orchestrators`,
`reborn_scaffolds`) are future migrations; today these classes are skipped
gracefully by `do_reassemble` (absent from `information_schema`).

### Component hierarchy (§0.1 — bottom to top)

```
Tool(0) → ToolSkill(13) → Skill(1-3) / PythonCode(22) → Recipe(21) → ExtensionCatalogue(23)
```

- **Tool (0)** — Rust execution layer only; no prompt text; excluded from
  all retrieval queries.
- **ToolSkill (13)** — Rust-channel only (param schema, preconditions,
  error handling). The orchestrator never reads ToolSkill bodies; a class-13
  UUID in `orchestrator_steps[].include` is a Q1 hard error (§0.5).
- **Skill (1–3)** / **PythonCode (22)** — orchestrator instructions for
  using one Rust tool / Python utilities for the orchestrator.
- **Recipe (21)** — a complete turn script; the primary intent target.
- **ExtensionCatalogue (23)** — the documentation namespace; draws the
  bigger picture, never re-documents commands (§5).

## 4. Behavior — legacy `MemoryDoc` migration

`component_import.rs::run_component_import(pool, agent_id, tenant_id)`
reads every legacy `brassclaw_memory_docs` (V016) row and migrates it into
the appropriate class-specific table:

| `DocType` | Table | class |
|-----------|-------|------:|
| Spec | `reborn_specs` | 12 |
| ToolSkill | `reborn_tool_skills` | 13 |
| Plan | `reborn_plans` | 14 |
| Summary | `reborn_summaries` | 15 |
| Lesson | `reborn_lessons` | 18 |
| Issue | `reborn_issues` | 19 |
| Note | `reborn_notes` | 20 |

- **`DocType::Skill`** — handled by `skill_import.rs` (already migrated).
- **`DocType::Recipe`** — handled by `PgRecipeStoreFacade` / V033.
- **`DocType::Docu` (class 17)** — has **no legacy `DocType` variant**;
  `reborn_docus` rows are created fresh and are *not* migrated from V016.

**Idempotency:** each row is keyed by `(tenant_id, user_id, agent_id,
project_id, name)`. A `content_hash` (SHA-256 of `title + "\n\n" + content`)
decides skip (same hash), update (different hash → reset
`validation_status = 'pending'` + add `05:validator` to `consumer_tags`),
or insert.

**Splitting:** docs longer than `SPLIT_CHUNK_CHARS` (≈20 000 chars ≈ 5 000
tokens) are split at paragraph boundaries into `{base_name}-part-{N}` rows
(max 20 chunks/doc).

**Scope mapping:** legacy `MemoryDoc` carries `(tenant_id, user_id,
project_id)`; the new tables require `(tenant_id, user_id, agent_id,
project_id)`. The caller supplies `agent_id`. This is the one-shot boot
import; it runs behind the `#[cfg(all(feature = "postgres", feature =
"skills-db"))]` gate.

## 5. Relations

- **Retrieval → catalog:** `PostgresSource::fetch_for_consumer` UNION ALLs
  the validated rows across the 12 content tables; `fetch_for_turn` uses
  intent to fetch a specific component by ID (see `11-retrieval-system.md`).
- **Base-prompt assembly → catalog:** `do_reassemble` walks
  `COMPONENT_TABLES`, selects validated + non-validator rows ordered by
  `prompt_uid`, and concatenates `## {class_code}:{prompt_uid} {name}`
  blocks (see `10-prefix-base-prompt.md`).
- **IBS → catalog:** `build_instruction` produces `rust_steps[]` /
  `orchestrator_steps[]` UUID lists; `fetch_component_by_id` resolves
  each UUID to a `ComponentItem` (see `04-ibs.md`).
- **Validation queue → catalog:** `approve()` dispatches on
  `component_class` to set `validation_status='validated'` on the right
  table (see `14-validation-queue.md`).
- **StepContextSpec (§0.5):** the formatter in
  `handle_assemble_prior_knowledge` derives a heading from each
  orchestrator item's `class_code` — `Skill` (1–3), `Spec` (12), `Recipe`
  (21), `PythonCode` (22), `Catalogue` (23), `Annotation` (text step). It
  is a *derived* type, computed at fetch time, never stored.
- **ExtensionCatalogue (§0.2):** `task_groups[]` → recipe names;
  `child_component_ids[]` for lineage; `intent_index[]` is audit-only and
  is **never seeded into `reborn_intent_inputs`** (the catalogue documents,
  it does not create intent rows).

## 6. Today vs v3

| Aspect | Today (≤ V049) | v3 target (V050–V059) |
|--------|-----------------|----------------------|
| Classes present | 0–21 (12 content tables in retrieval) | + 22 PythonCode (V052), + 23 ExtensionCatalogue (V053) |
| Recipe step columns | `trigger` + `steps` (v2 shape) | + `step_descriptions`/`variants`/`dependency_registry` JSONB (V050) |
| `dependency_registry` | absent | added to all 13 component tables (V055, §0.19) |
| `fetch_for_consumer` sub-selects | 12 | 14 (add class 22/23 arms, PERF-05 label) |
| `fetch_component_by_id` arms | 1-3, 4-9, 10\|50, 12-21 | + 22, + 23 arms |
| `class_label` arms | 0,1,9,10,12-21 | + 22 `PythonCode`, + 23 `Catalogue` (`FINDING B`: integer-only, no `DocType` variant) |
| `source = 'system'` | allowed on tools/tool_skills/skills (V057) | also on recipes + the two new tables (V052/V053 from day one, `FIND-P6-02`) |
| Pre-validation columns | 5 columns on each of 13 tables | centralized on `reborn_validation_queue`; dropped in V059 (see `14-validation-queue.md`) |
| Cognitive weights | `doc_type_weight(DocType)` (frozen, deprecated) — used by `RamSource` keyword path | **no weight function** for `PostgresSource` (orders by `class_code ASC, prompt_uid ASC`); `RamSource` + `doc_type_weight` deleted in K.3. The §0.11 weight table is **historical/authoring intent only** — classes 22/23 sort automatically; do not add weight arms |

**`FINDING B` (load-bearing):** the `DocType` enum is
`#[deprecated(since = "0.1.0")]`. Adding `PythonCode` / `ExtensionCatalogue`
variants to it would extend a deprecated type and contradict the
migration direction. All new class-code dispatch is integer-only. The
i32-keyed `doc_type_weight_by_class(i32)` does **not exist**; the only
weight function is the enum-keyed `doc_type_weight(DocType)`, which
cannot be extended. Classes 22/23 need only a `class_label` arm + the
`fetch_for_consumer` / `fetch_component_by_id` arms — ordering is
automatic via `class_code ASC`.

**Migrations are all additive-first** (V050–V058 add columns/tables; no
DROP, no renames). V059 is the only migration with DROP statements
(the legacy pre-validation columns). See §2 migration table at
`saved_plan_to_v3.md:6761`.

## 7. LLM Summary (machine-convertible)

The Component Catalog is the library of every validated, retrievable agent
unit, stored as Postgres rows in class-specific tables keyed by numeric
`class_code` (0–23). Classes: 0 Tool (no prompt text, excluded from
retrieval), 1–3 Skill (`body`), 4–9 Extension (`description`), 10
Orchestrator / 50 Scaffold (future tables, via skills today), 12 Spec, 13
ToolSkill (Rust-channel only — never in orchestrator items), 14 Plan, 15
Summary, 16 Action (`description`, steps JSONB), 17 Docu, 18 Lesson, 19
Issue, 20 Note, 21 Recipe (primary intent target, PKC-only), 22 PythonCode
(v3/V052 NEW), 23 ExtensionCatalogue (v3/V053 NEW — documentation
namespace, never re-docs commands). Every content table shares a common
shape: scope 4-tuple, `name`/`description`/`content`, `prior_knowledge_content`
(SCH-02 override), `override_prompt_creation` (Solution Override),
`class_code` CHECK, per-table `prompt_uid` BIGINT sequence (stable
retrieval/prefix ordering key), `consumer_tags` (SEC-01 validator gate),
`validation_status`, lineage (`similarity_parent_id`/`replaces_id`),
`source`. The `effective_content` expression varies by content column
(body / content / description). `PostgresSource::fetch_for_consumer` is a
single 12-sub-select UNION ALL (PERF-05; 14 after classes 22/23) ordered by
`class_code ASC, prompt_uid ASC` — no weight function is used; the §0.11
cognitive-weight table is historical intent only and `doc_type_weight` is a
frozen deprecated enum. `component_import.rs` does the one-shot legacy
`brassclaw_memory_docs` (V016) → class-table migration (idempotent by
content_hash, splits long docs, maps DocType→class; Docu/17 is created
fresh, not migrated). The v3 additions are classes 22/23 (integer dispatch
only, `FINDING B`), the recipe `step_descriptions`/`variants`/
`dependency_registry` JSONB (V050), `dependency_registry` on all 13 tables
(V055), and the validation-lifecycle column centralization onto
`reborn_validation_queue` (V051 table, V059 drop) — all migrations
additive-first except V059. **Status:** classes 0–21 and the common table
shape are live; classes 22/23 and the V050–V059 migrations are v3 (none
present yet).
