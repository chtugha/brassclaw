-- C.4.5.14 — extensions (classes 4–9) DB-structure standardization (item g):
-- drop the 5 legacy pre-centralization lifecycle columns from
-- `reborn_extensions_unified`, mirroring V071 (`reborn_tools`), V072
-- (`reborn_skills`), V073 (`reborn_actions`) and V074 (memory classes).
--
-- (1) DROP the 5 legacy pre-centralization lifecycle columns. V032 (the
--     original reborn_extensions_unified DDL) lines 115-121 carried
--     `validation_errors` (TEXT[]), `review_feedback`, `review_attempts`,
--     `rejected_at` and `queue_code` — the SAME 5-col legacy set V071-V074
--     dropped from the tool/skill/action/memory tables. The central
--     `reborn_validation_queue` (V051) tracks the full lifecycle via `state`
--     (1-4) + its OWN `validation_errors` (V051:72) + `review_feedback`
--     (V051:69); the Q2 graduation path only ever sets `validation_status` on
--     this table. `parent_mission_id` was already dropped workspace-wide by
--     V064.
--
--     Like V072 (`reborn_skills`, actively read by `DbSkillStore`) — and
--     UNLIKE V073/V074 (no Rust readers → clean DB-cleanup) — these 5 columns
--     ARE actively read by `brassclaw_extensions::unified_store::
--     PgUnifiedExtensionStore`: the `UnifiedExtension` struct,
--     `decode_row` (ordinals 16-20), `SELECT_COLS`, `update_validation_status`
--     and `wipe` all reference them. This migration is therefore PAIRED with a
--     refactor of `unified_store.rs` that removes every reference to them
--     (struct fields, the `decode_row` projection + `SELECT_COLS` list, the
--     dead `ValidationStatusUpdate.{validation_errors,review_feedback,
--     queue_code}` fields, the `update_validation_status` legacy writes, and
--     `wipe`'s `review_feedback = NULL` SET) so the store relies on
--     `validation_status` + the central `reborn_validation_queue` for
--     lifecycle — mirroring the reborn_skills path centralised by V072. The
--     INSERT/upsert paths already omitted the 5 columns, so no write path
--     changes. `DROP COLUMN IF EXISTS` is forward-compatible with the future
--     V059/Phase-N workspace-wide drop (which no-ops on this table).
--
-- (2) SCOPE NOTE (F8=A): this slice is DB-structure standardization ONLY. The
--     extension payload/class semantics — the `payload` JSONB (manifest /
--     recipe-step-list / plan-doc-body / extension-config, shape per `class`)
--     + the `monty`-class projection to `RecipeStage`/`plan_library`
--     (unified_store.rs `project_as_recipe_stage`) — are entangled with the
--     composition system (C.4.5.17) + the retirement phase (C.5/C.6/C.7) and
--     are NOT touched here. Per F8=A only the redundant centralised lifecycle
--     columns are dropped (the one change that persists regardless of the
--     extension classes' retirement fate). `queue_code` is plain `TEXT` with
--     NO inline CHECK in V032 (line 121), so no explicit constraint drop is
--     needed (unlike V072's `class_code` CHECK widening).
--
-- (3) SIBLING CLASSES ALREADY CLEAN. ExtensionCatalogue (class 23,
--     `reborn_extension_catalogues`, V053) was created in Phase C with NO
--     queue-tracking columns (V053:9-11 "Queue-tracking columns are NOT on
--     this table") — only `validation_status` + `parent_mission_id` (dropped
--     by V064). Scaffold (class 50) lives in `reborn_skills` (cleaned by
--     V072). So C.4.5.14 reduces to THIS one table.

-- drop the 5 legacy pre-centralization lifecycle columns.
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS queue_code;
