# Subplan: Step 6.5 — component_import.rs document-splitting importer

## Status: ✅ IMPLEMENTED

`component_import.rs` (16KB) fully implemented with content-splitting,
intent-example extraction, content-hash idempotency, per-table upserts, and
scope mapping. `run_component_import` wired in `build_reborn_runtime` at boot
(non-fatal) behind `all(postgres, skills-db)` gate. All sub-steps 1–8 complete.

## Goal (historical)
Implement `crates/brassclaw_reborn_composition/src/component_import.rs` — a
one-shot, idempotent importer that reads `MemoryDoc` rows from
`brassclaw_memory_docs` (V016) and migrates them into the 8 class-specific
component tables (V036–V043).

## DocType → Table mapping

| DocType   | Table             | class_code | consumer_tags default            |
|-----------|-------------------|------------|----------------------------------|
| Spec      | reborn_specs      | 12         | {02:orchestrator, 05:validator}   |
| ToolSkill | reborn_tool_skills| 13         | {01:rusty, 05:validator}          |
| Plan      | reborn_plans      | 14         | {02:orchestrator, 05:validator}   |
| Summary   | reborn_summaries  | 15         | {02:orchestrator, 05:validator}   |
| Docu      | reborn_docus      | 17         | {02:orchestrator, 05:validator}   |
| Lesson    | reborn_lessons    | 18         | {02:orchestrator, 05:validator}   |
| Issue     | reborn_issues     | 19         | {02:orchestrator, 05:validator}   |
| Note      | reborn_notes      | 20         | {02:orchestrator, 05:validator}   |

Note: `DocType::Skill` → migrated via `skill_import.rs` (already done).
`DocType::Recipe` → migrated via `PgRecipeStoreFacade` / V033 (already done).
`DocType::Docu` does NOT exist in the legacy `DocType` enum (V040 is for
documentation pages added in the new system). Skip any legacy Docu entries.

## Implementation plan

### Sub-step 1 — Content-splitting helper
Content > 5000 tokens (estimated at ~4 chars/token → 20000 chars): split at
paragraph boundaries into ≤5000-token chunks. Each chunk becomes a separate
row with `name = "{base_name}_{index}"`.

### Sub-step 2 — Intent-example extraction
For each doc, extract the first 3 sentences as intent examples (class 3,
full-sentence). Additional words/phrases from the title become class 2 entries.

### Sub-step 3 — Content hash
SHA-256 of `title + "\n\n" + content` (hex string). Used for idempotency:
if a row with matching `(scope, name)` AND `content_hash` exists → skip.
If matching `(scope, name)` but different hash → update, reset to `pending`.

### Sub-step 4 — Per-table upsert
Each table has a unique constraint on `(tenant_id, user_id, agent_id, project_id, name)`.
Use `INSERT INTO ... ON CONFLICT (scope_columns, name) DO UPDATE` with:
- `content_hash != EXCLUDED.content_hash` guard to skip no-change rows
- Reset `validation_status = 'pending'`, `consumer_tags = ARRAY['05:validator']`
  on update
- `source = 'migrated'`

### Sub-step 5 — Scope mapping
Legacy `MemoryDoc` has `(tenant_id, user_id, project_id)`.
New tables have `(tenant_id, user_id, agent_id, project_id)`.
`agent_id` comes from the caller (the runtime's `agent_id`).

### Sub-step 6 — Entry point
```rust
pub async fn run_component_import(
    pool: &PgPool,
    agent_id: &str,
    tenant_id: &str,
) -> Result<ComponentImportSummary, ComponentImportError>
```
Reads all docs for `(tenant_id)`, maps by `doc_type`, splits, upserts.

### Sub-step 7 — Wire into serve path (boot-time migration)
Wire `run_component_import` in `factory.rs` / `build_local_dev` — call once
at startup when `pg_pool` is available (similar to `ensure_bundled_reborn_skills_installed`).
Gate: `#[cfg(all(feature = "postgres", feature = "skills-db"))]`.

### Sub-step 8 — Clippy + tests + update checkup.md

## Files to touch
- `crates/brassclaw_reborn_composition/src/component_import.rs` (new)
- `crates/brassclaw_reborn_composition/src/lib.rs` (add module)
- `crates/brassclaw_reborn_composition/src/factory.rs` (wire boot call)
- `checkup.md` (update Step 6.5)
