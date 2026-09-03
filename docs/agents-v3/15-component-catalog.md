# 15 — Component Catalog (class codes 0–23, 50)

> **Subsystem:** The unified set of agent components — every versioned,
> validated, retrievable unit the agent is made of. Each component lives in
> a class-specific Postgres table keyed by a numeric `class_code` (0–23, 50),
> carries a per-table `prompt_uid` sequence (the stable retrieval/prefix
> ordering key), and is gated by `validation_status`. This doc is the
> catalog: the class-code taxonomy, the common table shape, the
> content-column variations, the legacy `MemoryDoc` → class-table migration,
> and the two v3 classes (22 PythonCode, 23 ExtensionCatalogue — both
> **shipped**).
> **Grounded in:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`
> (`PostgresSource::fetch_for_consumer` UNION ALL, `fetch_component_by_id`
> table-and-content dispatch, `class_code_to_table`, `doc_type_to_class_code`,
> `estimate_tokens`), `crates/brassclaw_engine/src/memory/intent_system.rs`
> (`class_label` integer dispatch :265), `crates/brassclaw_engine/src/memory/
> component_validator.rs` (per-class Q1 arms), `crates/brassclaw_reborn_composition/
> src/interceptor_config_service.rs` (`COMPONENT_TABLES`, composition-side
> `class_label`), `crates/brassclaw_reborn_composition/src/component_import.rs`
> (legacy `MemoryDoc` → class-table importer), `crates/brassclaw_pg/migrations/`
> (V027–V075), `saved_plan_to_v3.md` §0.1 hierarchy.

## 1. Purpose

A BrassClaw agent is not a monolithic prompt — it is a **library of
components**, each owned by a class-specific table, each independently
versioned, validated, and retrievable. The catalog is what retrieval
(`PostgresSource`), the prefix-base-prompt assembler
(`SystemBundleSource::get_system_bundle` / `do_assemble_bundle`), and the IBS
(`build_instruction`) all draw from. The user's "selected memories" and the
match-path "recipe" are both catalog components; the "base prompt … contains
every information, documentation, explanation, recipe etc. of the complete
agent" is the validated catalog reassembled.

Two facts frame the whole subsystem:

1. **Postgres is mandatory.** Every component is a row. There is no DB-less
   component storage (the legacy filesystem/`MemoryDoc` path was migrated away
   — §4). `RamSource` (keyword retrieval over a `Store`) is keyword-retrieval
   *over Postgres*, not a postgres-less path, and is dormant in production
   (deleted in Phase K.3 — see `11-retrieval-system.md`).
2. **The class code is the primary axis.** `class_code` decides the table, the
   content column, the validator arm, the formatter heading, and the retrieval
   sort position. It is an integer. The `DocType` enum is `#[deprecated]` and
   frozen — `FINDING B`: new classes 22/23 use integer dispatch only
   (`class_label`, `class_code_to_table`, the UNION/by-id arms), never a
   `DocType` variant. The enum-keyed `doc_type_to_class_code` /
   `doc_type_weight(DocType)` helpers belong to the dormant `RamSource` keyword
   path and are deleted in Phase K.3.

## 2. Location

### Class-code taxonomy (all live)

| class | Label | Table | Migration | Content column | Status |
|------:|-------|-------|----------|---------------|--------|
| 0 | Tool | `reborn_tools` | V030 (+V071) | *(none — no prompt text, excluded from retrieval)* | live |
| 1–3 | Skill (`skill_rusty`/`monty`/`llm`) | `reborn_skills` | V027 (+V072) | `body` | live |
| 4–9 | Extension (MCP / capability / monty-plan / LLM-template) | `reborn_extensions_unified` | V032 (+V075) | `description` | live |
| 10 | Orchestrator | `reborn_orchestrators` | *(future migration)* | `body` (via skills table — see §3) | live-graceful-skip |
| 12 | Spec | `reborn_specs` | V036 (+V074) | `content` | live |
| 13 | ToolSkill | `reborn_tool_skills` | V037 (+V070) | `content` (Rust-channel only) | live |
| 14 | Plan | `reborn_plans` | V038 (+V074) | `content` | live |
| 15 | Summary | `reborn_summaries` | V039 (+V074) | `content` | live |
| 16 | Action | `reborn_actions` | V029 (+V073) | `description` (steps is JSONB) | live (vestigial — `08`) |
| 17 | Docu | `reborn_docus` | V040 (+V074) | `content` | live |
| 18 | Lesson | `reborn_lessons` | V041 (+V074) | `content` | live |
| 19 | Issue | `reborn_issues` | V042 (+V074) | `content` | live |
| 20 | Note | `reborn_notes` | V043 (+V074) | `content` | live |
| 21 | Recipe | `reborn_recipes` | V033 (+V050) | `''` (no plain content — `steps` JSONB; PKC only) | live |
| 22 | PythonCode | `reborn_python_code` | V052 (+V069) | `content` | **v3 — shipped** |
| 23 | ExtensionCatalogue | `reborn_extension_catalogues` | V053 | `overview_doc`→`content` | **v3 — shipped** |
| 50 | Scaffold | `reborn_scaffolds` | *(future migration)* | `body` (via skills table) | live-graceful-skip |

The `+V0NN` entries are the C.4.5 syntax-cleanup migrations (placeholder
grammar, `includes`, `capability_id`, legacy-col drops). `reborn_recipes` is
the **one** table whose 5 legacy lifecycle columns were NOT yet dropped —
that is Phase N (V076+, pending); see §6.

### Code locations

- **`class_label` (authoritative integer dispatch)** —
  `intent_system.rs:265` — `class_code: i32 → String` label. Arms include
  21 => "recipe", 22 => "python_code", 23 => "extension_catalogue",
  50 => "scaffold" (plus 0–20 above). Used by retrieval formatting + the
  intent legend (21/22/23/50).
- **`COMPONENT_TABLES` + composition `class_label`** —
  `interceptor_config_service.rs` — the table list + class-code → header
  label the Sempai base-prompt assembler (`do_assemble_bundle`) walks. Skips
  absent future tables (`reborn_orchestrators`, `reborn_scaffolds`) via an
  `information_schema.tables` check.
- **`fetch_component_by_id` content dispatch** — `retrieval_source.rs` —
  per-class `(table, content_expr)` arm; covers `1..=3`, `4..=9`, `10 | 50`,
  `12..=21`, `22`, `23`.
- **`fetch_for_consumer` UNION ALL** — `retrieval_source.rs` — the single
  14-sub-select query (PERF-05) across all validated content tables incl 22/23.
- **`class_code_to_table`** — `retrieval_source.rs` — the class→table map
  (incl. 22/23) shared by the by-id path.
- **`doc_type_to_class_code`** — `retrieval_source.rs:1449` — legacy
  `DocType` → `(class_code, label)` for the dormant `RamSource` keyword path
  (deleted Phase K.3).
- **`component_import.rs`** — legacy `brassclaw_memory_docs` (V016) →
  class-table importer (§4).
- **`component_validator.rs`** — per-class Q1 validation arms (incl. class 22
  `Generic(GenericComponent)` + `validate_python_code_body`/
  `validate_python_code_placeholders`, class 23, class 0, class 10/50, class
  13 placeholder+includes gates — see the per-class docs).

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
    -- the 5 legacy lifecycle cols (validation_errors/review_feedback/
    -- review_attempts/rejected_at/queue_code) were present on every table
    -- created ≤ V043; DROPPED across V070–V075 (tools/skills/tool_skills/
    -- actions/memory-classes/extensions_unified). reborn_recipes STILL
    -- retains them — Phase N (V076+) pending. reborn_python_code (V052) /
    -- reborn_extension_catalogues (V053) never had them.
    source                  TEXT NOT NULL DEFAULT 'migrated',
    content_hash            TEXT,
    similarity_parent_id    UUID,                        -- lineage
    replaces_id             UUID,                        -- lineage
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);
```

Key fields:

- **`prompt_uid`** — a per-table BIGINT sequence. The **stable ordering key**
  for retrieval (`ORDER BY class_code ASC, prompt_uid ASC`) and the reassembled
  base prompt. Never changes for a row, so the prefix-cache prefix stays stable
  across edits.
- **`prior_knowledge_content`** — when non-empty, used as the component's
  prior-knowledge text *instead of* assembling from `content` (SCH-02).
- **`override_prompt_creation`** — the **Solution Override** path: this
  component's PKC *replaces* standard assembly (see `08-actions-system.md`).
- **`consumer_tags`** — `02:orchestrator`, `03:llm`, `05:validator`. The
  SEC-01 gate excludes `05:validator` rows from retrieval
  (`'05:validator' != ALL(consumer_tags)`); `consumer_tag` membership also
  filters `fetch_for_consumer` (`$5 = ANY(consumer_tags)`).
- **`source`** — provenance. CHECK varies by table (see §6): tools + skills
  widened to incl `'system'` (V066); `reborn_python_code` (V052) +
  `reborn_extension_catalogues` (V053) incl `'system'` from day one;
  `reborn_tool_skills` (V037) + `reborn_recipes` (V033) have **no CHECK**
  (any value accepted, incl `'system'`); `reborn_actions` (V029) CHECK is
  `authored/extracted/migrated/imported` — **no `'system'`** (system Actions
  cannot be seeded today).
- **`dependency_registry`** — present on `reborn_recipes` (V050),
  `reborn_python_code` (V052), `reborn_extension_catalogues` (V053) **only**
  (not on all tables). Raw JSONB; the composer reads it at compose time.
- **Lineage** — `similarity_parent_id` / `replaces_id` track self-improvement
  ancestry.

### Content-column dispatch (`fetch_component_by_id`)

| Classes | Table | `effective_content` |
|---------|-------|---------------------|
| 1–3 | `reborn_skills` | `COALESCE(NULLIF(prior_knowledge_content,''), body)` |
| 4–9 | `reborn_extensions_unified` | `COALESCE(prior_knowledge_content, description)` |
| 10 \| 50 | `reborn_skills` | `COALESCE(NULLIF(prior_knowledge_content,''), body)` |
| 12–15, 17–20 | `reborn_{specs\|tool_skills\|plans\|summaries\|docus\|lessons\|issues\|notes}` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 16 | `reborn_actions` | `COALESCE(prior_knowledge_content, description)` (steps is JSONB) |
| 21 | `reborn_recipes` | `COALESCE(NULLIF(prior_knowledge_content,''), '')` (no plain content) |
| 22 | `reborn_python_code` | `COALESCE(NULLIF(prior_knowledge_content,''), content)` |
| 23 | `reborn_extension_catalogues` | `COALESCE(prior_knowledge_content, description)` |
| 0 | `reborn_tools` | *(no arm — returns empty; no prompt text)* |

**Class 22 + 23 arms are shipped** (V052/V053). **Class 10/50** dispatch to
`reborn_skills` with the `body` expression; their dedicated tables
(`reborn_orchestrators`, `reborn_scaffolds`) are future migrations, skipped
gracefully by the base-prompt assembler when absent.

### Component hierarchy (§0.1 — bottom to top)

```
Tool(0) → ToolSkill(13) → Skill(1-3) / PythonCode(22) → Recipe(21) → ExtensionCatalogue(23)
```

- **Tool (0)** — Rust execution layer only; no prompt text; excluded from all
  retrieval queries. C.4.5.4 dropped `cdylib_artifact_path` + added
  `capability_id` (V071).
- **ToolSkill (13)** — Rust-channel only (param schema, preconditions, error
  handling). The orchestrator never reads ToolSkill bodies; a class-13 UUID in
  `orchestrator_steps[].include` is a Q1 hard error. C.4.5.3 added `includes`
  (V070).
- **Skill (1–3)** / **PythonCode (22)** — orchestrator instructions for using
  one Rust tool / Python utilities for the orchestrator. C.4.5.5 (skills) +
  C.4.5.2 (python_code `includes`, V069).
- **Recipe (21)** — a complete turn script; the primary intent target. V050
  added `step_descriptions`/`variants`/`dependency_registry`; C.4.5.1
  formalized the `{{...}}` placeholder grammar.
- **ExtensionCatalogue (23)** — the documentation namespace; draws the bigger
  picture, never re-documents commands (§5). Shipped V053.

## 4. Behavior — legacy `MemoryDoc` migration

`component_import.rs::run_component_import(pool, agent_id, tenant_id)` reads
every legacy `brassclaw_memory_docs` (V016) row and migrates it into the
appropriate class-specific table:

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
  `reborn_docus` rows are created fresh, not migrated from V016.

**Idempotency:** each row keyed by `(tenant_id, user_id, agent_id,
project_id, name)`. A `content_hash` (SHA-256 of `title + "\n\n" + content`)
decides skip (same hash), update (different hash → reset
`validation_status = 'pending'` + add `05:validator` to `consumer_tags`), or
insert.

**Splitting:** docs longer than `SPLIT_CHUNK_CHARS` (≈20 000 chars ≈ 5 000
tokens) are split at paragraph boundaries into `{base_name}-part-{N}` rows
(max 20 chunks/doc).

**Scope mapping:** legacy `MemoryDoc` carries `(tenant_id, user_id,
project_id)`; the new tables require `(tenant_id, user_id, agent_id,
project_id)`. The caller supplies `agent_id`. One-shot boot import, behind the
`#[cfg(all(feature = "postgres", feature = "skills-db"))]` gate.

## 5. Relations

- **Retrieval → catalog:** `PostgresSource::fetch_for_consumer` UNION ALLs the
  validated rows across the 14 content tables (incl 22/23); `fetch_for_turn`
  uses intent to fetch a specific component by ID (see `11-retrieval-system.md`).
- **Base-prompt assembly → catalog:** `SystemBundleSource::get_system_bundle`
  (`do_assemble_bundle`) walks `COMPONENT_TABLES`, selects validated +
  non-validator rows ordered by `prompt_uid`, and concatenates
  `## {class_code}:{prompt_uid} {name}` blocks (see `10-prefix-base-prompt.md`).
- **IBS → catalog:** `build_instruction` produces `rust_steps[]` /
  `orchestrator_steps[]` UUID lists; `fetch_component_by_id` resolves each UUID
  to a `ComponentItem` (see `04-ibs.md`).
- **Validation queue → catalog:** graduation dispatches on `component_class` to
  set `validation_status='validated'` on the right table (see
  `14-validation-queue.md`).
- **StepContextSpec (§0.5):** the prior-knowledge formatter derives a heading
  from each orchestrator item's `class_code` — `Skill` (1–3), `Spec` (12),
  `Recipe` (21), `PythonCode` (22), `Catalogue` (23), `Annotation` (text step).
  A *derived* type, computed at fetch time, never stored.
- **ExtensionCatalogue (§0.2):** `task_groups[]` → recipe names;
  `child_component_ids[]` for lineage; `intent_index[]` is audit-only and is
  **never seeded into `reborn_intent_inputs`** (the catalogue documents, it
  does not create intent rows).

## 6. Status — shipped vs. pending

| Aspect | Shipped | Pending |
|---|---|---|
| Classes present | 0–23, 50 (14 content tables in retrieval incl 22/23) | dedicated `reborn_orchestrators` (10) / `reborn_scaffolds` (50) tables |
| Class 22 PythonCode | V052 (+`includes` V069) | — |
| Class 23 ExtensionCatalogue | V053 | — |
| Recipe step columns | `step_descriptions`/`variants`/`dependency_registry` JSONB (V050) | — |
| `dependency_registry` | recipes (V050), python_code (V052), catalogues (V053) **only** | — |
| `fetch_for_consumer` sub-selects | 14 (class 22/23 arms shipped) | — |
| `fetch_component_by_id` arms | 1-3, 4-9, 10\|50, 12-21, 22, 23 | — |
| `class_label` arms | integer dispatch incl 22 `python_code`, 23 `extension_catalogue`, 50 `scaffold` | — |
| `source = 'system'` | tools+skills (V066), tool_skills (no CHECK), recipes (no CHECK), python_code (V052), catalogues (V053) | actions (no `'system'` — system Actions cannot be seeded) |
| Legacy 5 lifecycle cols | DROPPED V070–V075 (tools/skills/tool_skills/actions/memory-classes/extensions_unified); python_code/catalogues never had them | **`reborn_recipes` STILL retains them — Phase N (V076+)** |
| Cognitive weights | `PostgresSource` orders by `class_code ASC, prompt_uid ASC` (no weight function) | `RamSource` + `doc_type_weight(DocType)` + `doc_type_to_class_code` deleted Phase K.3 |

**`FINDING B` (load-bearing):** the `DocType` enum is
`#[deprecated(since = "0.1.0")]`. Adding `PythonCode` / `ExtensionCatalogue`
variants would extend a deprecated type and contradict the migration
direction. All class-code dispatch is integer-only (`class_label`,
`class_code_to_table`, the UNION/by-id arms). Classes 22/23 need only a
`class_label` arm + the `fetch_for_consumer` / `fetch_component_by_id` arms —
ordering is automatic via `class_code ASC`. The §0.11 weight table is
historical/authoring intent only; do not add weight arms.

**Migrations are additive-first** (V050–V069 add columns/tables; no DROP of
live columns). V070–V075 are the C.4.5 syntax cleanups (placeholder grammar,
`includes`, `capability_id`, and the legacy 5-col DROPs). The next migration
is **V076** (Phase N: drop the 5 legacy cols from `reborn_recipes` + decoder
re-index).

## 7. LLM Summary (machine-convertible)

The Component Catalog is the library of every validated, retrievable agent
unit, stored as Postgres rows in class-specific tables keyed by numeric
`class_code` (0–23, 50). Classes: 0 Tool (no prompt text, excluded from
retrieval), 1–3 Skill (`body`), 4–9 Extension (`description`), 10
Orchestrator / 50 Scaffold (future dedicated tables, via skills today), 12
Spec, 13 ToolSkill (Rust-channel only), 14 Plan, 15 Summary, 16 Action
(vestigial), 17 Docu, 18 Lesson, 19 Issue, 20 Note, 21 Recipe (no plain
content), 22 PythonCode, 23 ExtensionCatalogue. Each row carries a per-table
`prompt_uid` sequence (the stable retrieval/prefix ordering key), a
`prior_knowledge_content` override (SCH-02), `override_prompt_creation`
(Solution Override), `consumer_tags` (SEC-01 gate excludes `05:validator`),
and `validation_status`. `dependency_registry` (JSONB) is on recipes,
python_code, and catalogues only. Retrieval is a single 14-table UNION ALL
(`PostgresSource`, PERF-05) ordered by `class_code ASC, prompt_uid ASC`; the
by-id path covers 22/23. Class-code dispatch is integer-only (`class_label` →
"recipe"/"python_code"/"extension_catalogue"/"scaffold"; `class_code_to_table`);
the deprecated `DocType` enum + `doc_type_to_class_code`/`doc_type_weight`
belong to the dormant `RamSource` keyword path (deleted Phase K.3). The 5
legacy lifecycle columns were dropped from tools/skills/tool_skills/actions/
memory-classes/extensions_unified (V070–V075); `reborn_recipes` still retains
them — Phase N (V076+) pending. `source='system'` is allowed on tools+skills
(V066), tool_skills + recipes (no CHECK), python_code (V052), catalogues
(V053); Actions have no `'system'` (system Actions cannot be seeded). The
base prompt is reassembled from the validated catalog by
`SystemBundleSource::get_system_bundle` (`do_assemble_bundle`).
