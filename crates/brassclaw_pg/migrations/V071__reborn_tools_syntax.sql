-- V071__reborn_tools_syntax.sql
-- Step C.4.5.4 — Common component syntax: Tool (class 0) schema
-- standardization (item g) + the rust-side common form.
--
-- (1) DROP the 5 legacy pre-centralization lifecycle columns. V051 (the
--     central validation queue) lines 8-9 document `queue_code`,
--     `validation_errors`, `review_feedback`, `review_attempts`,
--     `rejected_at` as pre-centralization remnants slated for the V059/Phase-N
--     workspace-wide drop: the central `reborn_validation_queue` tracks
--     lifecycle via `state` (1-4) + its OWN `validation_errors` (V051:72) +
--     `review_feedback` (V051:69), NOT via the component-table columns. The Q2
--     graduation `approve()` UPDATE (validation_queue.rs:466) only sets
--     `validation_status = 'validated'` on this table — it never touches the
--     legacy columns. No Rust reader/writer touches them either
--     (`pg_tool_store` INSERT omits them; `db_tool_source` SELECTs only `name`;
--     `06-tools-system.md:144` confirms they "still live on reborn_tools
--     (Phase N centralises them)"). `parent_mission_id` was already dropped by
--     V064. `DROP COLUMN IF EXISTS` is forward-compatible with the future
--     V059/Phase-N drop (which no-ops on this table).
--
-- (2) DROP `cdylib_artifact_path` (reverses V067). builtin_stuff_v3.md Step 1.1
--     plans Tool rows with `capability_id` (the builtin dispatch id, e.g.
--     "builtin.shell") and NO cdylib_artifact_path: Tools are ready-to-run
--     Executioner units (built-in = precompiled static dispatch; dynamic = a
--     cdylib that just needs loading) and do NOT pass through the IBS to be
--     rebuilt. The cdylib load directive is a COMPOSITION concern (which cdylib
--     a recipe step / toolskill binds + dlopens) and therefore belongs on the
--     recipe step / toolskill, not on the Tool row itself (C.4.5.17 wires the
--     load directive there). The column has zero Rust readers/writers today
--     (V067 only created the column + a partial index; `NewPgTool` never set
--     it; `db_tool_source` does not read it) — it is dead, so dropping it is
--     safe. The V067 partial index `reborn_tools_cdylib_path_idx` is dropped
--     first (explicit + forward-compatible; Postgres would auto-drop it with
--     the column anyway).
--
-- (3) ADD `capability_id TEXT NOT NULL DEFAULT ''` — the rust-side common form
--     of a Tool: the Executioner's dispatch identifier. For built-in tools it
--     is the static `match call.function_name` key (e.g. "builtin.shell"); for
--     host.* bridge tools it is the `host.X` function name (e.g.
--     "host.resolve_intent"). DEFAULT '' keeps pre-existing rows valid; the
--     backfill UPDATE below sets it from `name` (for the seeded host.* tools
--     capability_id == name). The Q1 gate (component_validator class-0 arm)
--     enforces non-empty; the DB does not (a NOT-empty CHECK would reject the
--     '' default on pre-existing rows).

-- (2a) drop the V067 partial index before the column it depends on.
DROP INDEX IF EXISTS reborn_tools_cdylib_path_idx;

-- (2b) drop the cdylib_artifact_path column (reverses V067).
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS cdylib_artifact_path;

-- (1) drop the 5 legacy pre-centralization lifecycle columns.
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS queue_code;
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS rejected_at;

-- (3a) add the rust-side common form: the Executioner dispatch identifier.
ALTER TABLE reborn_tools
    ADD COLUMN IF NOT EXISTS capability_id TEXT NOT NULL DEFAULT '';

-- (3b) backfill pre-existing rows: for the seeded host.* bridge tools the
--     dispatch id equals the row name. Future NewPgTool inserts set
--     capability_id explicitly. Idempotent (only fills empty rows).
UPDATE reborn_tools
   SET capability_id = name
 WHERE capability_id = '';
