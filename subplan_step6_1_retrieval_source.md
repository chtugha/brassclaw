# Subplan: Step 6.1 — RetrievalSource Trait + PostgresSource + RamSource

## Context

The `__assemble_prior_knowledge__` host function in `orchestrator.rs` is a **stub** that
delegates to the old `retrieve_context` path (which reads `memory_docs` / `DocType`-based
store). The plan requires replacing this with:

1. A `RetrievalSource` trait with `fetch_for_consumer(scope, query, token_budget, consumer_tag)`.
2. `PostgresSource` — reads from all class-specific component tables
   (skills, specs, tool_skills, plans, summaries, docus, lessons, issues, notes, actions,
   extensions, recipes, tools) via a single UNION query or a PG VIEW.
3. `RamSource` — keyword-based fallback using existing `extract_keywords`/`keyword_match_score`
   over `MemoryDoc` store. Used when no DB is available.
4. Both enforce `validation_status = 'validated' AND '05:validator' != ANY(consumer_tags)`.
5. Wire these into `handle_assemble_prior_knowledge` so the stub is replaced.

## What already exists (do not duplicate)

- `extract_keywords`, `keyword_match_score`, `doc_type_weight` — in `retrieval_dbless.rs`
- `RetrievalEngine::retrieve_context` — old MemoryDoc-based path (keep as legacy, not deleted here)
- `class_label(class_code)` in `intent_system.rs` — authoritative label table
- `format_prior_knowledge_for_llm` + `doc_type_class_code` in `orchestrator.rs`
- Intent system `resolve_intent` in `intent_system.rs` (DB path) — Step 6.7 wires it into PKC
- `MemoryDoc` struct — still used by legacy path; do NOT delete here (Step 6.10 handles that)

## Dependencies

- Requires `skills-db` feature for the Postgres path
- `brassclaw_pg::PgPool` for DB queries
- All component tables already exist (V027–V043, V046)

## Canonical component row for retrieval

All component tables share these columns:
```
id UUID, class_code SMALLINT, prompt_uid BIGINT,
name TEXT, content TEXT,
prior_knowledge_content TEXT (nullable),
override_prompt_creation BOOLEAN,
consumer_tags TEXT[], validation_status TEXT
```

`reborn_tools` has no `content` column (Rusty-only, no prompt text) — exclude from PKC assembly
(class 00, not a prior-knowledge source).

`reborn_skills` has `body` not `content` — map accordingly.
`reborn_extensions_unified` has `prior_knowledge_content` (V032).
`reborn_recipes` has `prior_knowledge_content` (V033).
All V036–V043 tables have `prior_knowledge_content` from V046.

## Exclusion list for PKC

Per spec: Issues/Notes/Summaries excluded from the static fallback-content file priority
but INCLUDED in DB-backed retrieval (they are valid components). Only class 00 (tools)
has no prompt text and is excluded.

## Plan

### Sub-step 1 — Define `ComponentItem` (the retrieval row type)

In `crates/brassclaw_engine/src/memory/retrieval_source.rs` (new file):

```rust
/// A single retrieved component row from any class table.
#[derive(Debug, Clone)]
pub struct ComponentItem {
    pub id: uuid::Uuid,
    pub class_code: i32,
    pub prompt_uid: i64,
    pub name: String,
    /// Effective prior-knowledge text. If `prior_knowledge_content` is Some,
    /// that takes precedence over `content` (Solution Override path §3.13).
    pub effective_content: String,
    pub override_prompt_creation: bool,
}
```

### Sub-step 2 — Define `RetrievalSource` trait

```rust
#[async_trait::async_trait]
pub trait RetrievalSource: Send + Sync {
    /// Fetch validated components for the given scope and consumer tag.
    ///
    /// `consumer_tag` — class-code prefix of the calling component
    ///   (e.g. `"02"` for orchestrator). The tag is used to filter
    ///   `consumer_tags[] @> ARRAY[$consumer_tag]` in DB mode.
    ///   In RamSource mode it falls back to all validated docs.
    ///
    /// Returns components ordered by `(class_code ASC, prompt_uid ASC)`.
    async fn fetch_for_consumer(
        &self,
        scope: &ComponentScope,
        query: &str,
        token_budget: usize,
        consumer_tag: &str,
    ) -> Result<Vec<ComponentItem>, RetrievalSourceError>;
}
```

`ComponentScope` holds the 4-part scope (tenant_id, user_id, agent_id, project_id).

### Sub-step 3 — `RamSource` implementation

`RamSource` wraps `Arc<dyn Store>` (the MemoryDoc store). It calls
`retrieve_context` on the existing `RetrievalEngine` and maps `MemoryDoc` →
`ComponentItem` using `doc_type_class_code()` (already in `orchestrator.rs` —
move/expose it). Respects `token_budget` by accumulating token cost estimates.

Scope mapping: `ComponentScope.project_id` → `ProjectId` for store queries.

This is the DB-less path — used when no PG pool is available.

### Sub-step 4 — `PostgresSource` implementation (feature `skills-db`)

Uses a single UNION ALL query across all component tables that have prior-knowledge
content. Each sub-select projects to the same shape:

```sql
SELECT id, class_code, prompt_uid, name,
       COALESCE(prior_knowledge_content, content) AS effective_content,
       override_prompt_creation
FROM <table>
WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
  AND validation_status = 'validated'
  AND '05:validator' != ALL(consumer_tags)
  AND $consumer_tag = ANY(consumer_tags)
```

Tables to include (in UNION):
- `reborn_skills` (class 1–3, content = `body`)
- `reborn_specs` (class 12, content = `content`)
- `reborn_tool_skills` (class 13)
- `reborn_plans` (class 14)
- `reborn_summaries` (class 15)
- `reborn_actions` (class 16, content = steps JSONB serialized)
- `reborn_docus` (class 17)
- `reborn_lessons` (class 18)
- `reborn_issues` (class 19)
- `reborn_notes` (class 20)
- `reborn_recipes` (class 21)
- `reborn_extensions_unified` (class 4–9)

NOTE: `reborn_tools` (class 00) excluded — no prompt text.

Order: `ORDER BY class_code ASC, prompt_uid ASC`
Token budget: truncate after accumulating `estimated_tokens >= token_budget`.

### Sub-step 5 — Move `doc_type_class_code` to `memory/retrieval_source.rs`

It's currently private in `orchestrator.rs`. Move/expose it so both
`RamSource` and the formatter can use it.

### Sub-step 6 — Update `handle_assemble_prior_knowledge` to use `RetrievalSource`

Thread `Arc<dyn RetrievalSource>` through the orchestrator context (alongside
the existing `Option<RetrievalEngine>`). Replace the stub body with:

```rust
let components = retrieval_source
    .fetch_for_consumer(&scope, &goal, token_budget, sender_class_code)
    .await?;
```

If `retrieval_source` is not available (no DB, no store) return empty PKC
(same as current None path).

### Sub-step 7 — Wire in factory/runtime

In `crates/brassclaw_engine/src/executor/orchestrator.rs`:
- Add `retrieval_source: Option<Arc<dyn RetrievalSource>>` to `OrchestratorContext` or
  pass as a parameter alongside `retrieval`.
- When `skills-db` + `pg_pool` available: create `PostgresSource::new(pool, scope)`
- Otherwise: create `RamSource::new(store)` wrapping the existing `RetrievalEngine`'s store.

In `build_reborn_runtime` / factory wiring: thread `Arc<dyn RetrievalSource>` through.

### Sub-step 8 — Clippy + tests

- Unit test `RamSource` returns MemoryDoc-based results (no DB).
- Unit test `PostgresSource` SQL structure (snapshot test of the query string).
- Integration test (behind `#[cfg(feature = "integration")]`): insert a validated
  component into `reborn_skills`, call `fetch_for_consumer`, verify it's returned.

### Sub-step 9 — Update checkup.md Step 6.1, commit and push

## Files to touch

- `crates/brassclaw_engine/src/memory/retrieval_source.rs` (new)
- `crates/brassclaw_engine/src/memory/mod.rs` — register module + re-export
- `crates/brassclaw_engine/src/executor/orchestrator.rs` — thread `RetrievalSource`,
  update `handle_assemble_prior_knowledge`, remove stub comment
- `crates/brassclaw_engine/Cargo.toml` — ensure `uuid` + `async-trait` deps
- `checkup.md` — mark Step 6.1 ✅

## What is NOT in scope here

- `reborn_component_catalog` PG VIEW — adding a V047 migration is optional;
  the UNION ALL query in `PostgresSource` IS the catalog read model per PERF-05.
  A named VIEW can be added as V047 if needed for the interceptor path in Step 7.4.
- Retiring `DocType` (Step 6.10) — separate step.
- Full intent-driven routing via `resolve_intent` (Step 6.7) — `PostgresSource`
  here returns ALL validated components (token-budget-capped); the intent router
  sits in front in Step 6.7.
- DB-less fallback-content file (Step 6.2) — separate step.
