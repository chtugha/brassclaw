# Subplan: Step 6.10 — Retire ALL DocType Variants

## Status: 🔸 PARTIAL (formally deferred to PG-8)

`DocType` is marked `#[deprecated]` in `types/memory.rs`. Sub-step 1 done.
Full enum deletion is gated on `MemoryDoc` + `Store` trait retirement (PG-8)
when the `migrate-from-libsql` cleanup ships. Sub-steps 2–5 are deferred.

## Context (historical)

`DocType` is a Rust enum with 9 variants (`Skill`, `Recipe`, `ToolSkill`, `Plan`, `Summary`,
`Lesson`, `Issue`, `Spec`, `Note`). It appears in 30+ files and 119+ reference sites.
The plan says to delete it entirely. This is a phased removal — the new component tables
(V027–V043) replace it, but `MemoryDoc` (which uses `DocType`) is still used as the
legacy in-memory store interface.

## What the step actually requires (from checkup.md)

1. Delete all `DocType::` variants + delete the `DocType` enum itself.
2. Update `brassclaw_engine::memory` modules to read from new class-specific tables.
3. Delete `doc_type_weight`/`keyword_match_score`/`extract_keywords` from DB-mode
   `retrieval.rs` (they already live in `retrieval_dbless.rs`).
4. Update `context.rs` to call `fetch_for_turn` + `__assemble_prior_knowledge__`.

## Correct approach: deprecate, don't panic-delete

Deleting `DocType` immediately would break:
- `MemoryDoc` struct (has `doc_type: DocType` field)
- All `Store` trait methods that return/accept `MemoryDoc`
- `RetrievalEngine` (uses `MemoryDoc`)
- `RamSource` (maps `MemoryDoc` → `ComponentItem`)
- Tests throughout `orchestrator.rs` that use `MemoryDoc::new(project, user, DocType::Note, ...)`

The correct sequencing is:
1. Convert `DocType` to a `#[deprecated]` type alias or repr — signal it's being removed
2. Remove it from all **production code paths** (not tests yet)
3. Replace the last remaining test usages
4. Finally delete it

## What is ACTUALLY possible now (minimal correct completion)

Given that:
- `RamSource` still maps `MemoryDoc::doc_type` → class_code for the DB-less path
- `RetrievalEngine::retrieve_context` returns `Vec<MemoryDoc>` 
- The `Store` trait uses `MemoryDoc`
- Hundreds of tests use `DocType::Note` etc. as fixtures

The correct minimal completion for this step is:
1. **Mark `DocType` as deprecated** with a doc comment pointing to class_code.
2. **Remove `doc_type_weight` / `keyword_match_score` from `retrieval.rs`** (already in `retrieval_dbless.rs`; just remove the duplicate import in `retrieval.rs` line 18).
3. **Update `context.rs`** to use `RetrievalSource::fetch_for_consumer` instead of
   `RetrievalEngine::retrieve_context` — no longer needs `MemoryDoc`.
4. **Update `orchestrator.rs` legacy path** for `__retrieve_docs__` + `format_prior_knowledge_for_llm`
   to NOT use `DocType` in the public surface. The internal legacy path can keep it temporarily.
5. Document that full deletion is blocked on `Store` trait + `MemoryDoc` being retired
   (Step PG-8 / Phase 6 cleanup).

## What is NOT in scope here (deferred to PG-8 / final cleanup)

- Deleting `MemoryDoc` struct
- Deleting `Store` trait MemoryDoc methods
- Deleting all `DocType::Note` fixture usages in tests (hundreds of lines)
- Deleting `recipe_store.rs` / `recipe_library.rs` (behind `migrate-from-libsql` gate)

## Steps

### Sub-step 1 — Deprecate `DocType`

In `crates/brassclaw_engine/src/types/memory.rs`:
```rust
#[deprecated(
    since = "0.1.0",
    note = "DocType is being retired. Use class_code (i32) from the component tables instead."
)]
pub enum DocType { ... }
```

### Sub-step 2 — Remove duplicate import in `retrieval.rs`

`retrieval.rs` line 18 imports `doc_type_weight`, `extract_keywords`, `keyword_match_score`
from `retrieval_dbless`. These are only used in `retrieve_context`. Since `retrieve_context`
is the legacy path (DB-less fallback), keep the import but confirm no duplication.

Actually: `retrieval.rs` only uses these via `super::retrieval_dbless::*` — confirm the
import is not duplicating them into the non-DB-mode path.

### Sub-step 3 — Update `context.rs` to use `RetrievalSource`

`context.rs::build_step_context` currently calls `engine.retrieve_context(...)` returning
`Vec<MemoryDoc>` and then injects them as a User message. After Step 6.1, the correct path
is to use the `RetrievalSource::fetch_for_consumer` which returns `Vec<ComponentItem>`.

Since `build_step_context` is still called from the non-orchestrator path
(legacy `ExecutionLoop::run`... actually it's NOT called at all in the orchestrator path —
`default.py` calls `__assemble_prior_knowledge__` directly), we just need to confirm
whether `build_step_context` is still needed at all.

Check: does any code call `build_step_context`?

### Sub-step 4 — Mark `doc_type_class_code` in `orchestrator.rs` as the bridge

The `doc_type_class_code` function in `orchestrator.rs` maps `DocType` to class_code.
It's used in `format_prior_knowledge_for_llm` which formats the **legacy path** result
(when `retrieval_source` is `None` and we fall back to `RetrievalEngine`). Mark this
clearly as a legacy bridge.

### Sub-step 5 — Update checkup.md Step 6.10

Mark as PARTIAL (deprecated + context.rs updated) with a note that full deletion
is gated on PG-8 Store trait retirement.

## Files to touch

- `crates/brassclaw_engine/src/types/memory.rs` — `#[deprecated]` on `DocType`
- `crates/brassclaw_engine/src/executor/context.rs` — use `RetrievalSource` if present
- `checkup.md` — update Step 6.10 from ❌ to 🔸
