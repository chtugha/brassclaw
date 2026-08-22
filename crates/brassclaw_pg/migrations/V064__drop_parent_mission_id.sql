--
-- V064__drop_parent_mission_id.sql
--
-- Drops the dead `parent_mission_id UUID` column from every table that
-- carried it (V027 reborn_skills + the 14 component class tables introduced
-- in V029–V053).
--
-- Why: `parent_mission_id` was the foreign link from a recipe/skill/component
-- back to the v1-routines-reborn "mission" that authored or owned it. The
-- mission system (`brassclaw_engine` `runtime/mission.rs` +
-- `types/mission.rs`, v1 routines reborn) is obsolete and is being deleted
-- in the Phase H.5 obsolescence cleanup (see
-- `docs/agents-v3/subplan_problem_stepH5_obsolescence_of_saved_plan_to_v3.md`,
-- sub-step O2.1). A workspace-wide audit confirmed ZERO Rust code reads or
-- writes `parent_mission_id` — grep surfaces only these migration files and
-- documentation. The column is nullable with no default, no index, no
-- constraint, and no CHECK referencing it, so dropping it is a pure schema
-- shrink with no behavior change.
--
-- Idempotent: every statement uses `DROP COLUMN IF EXISTS`, so the migration
-- is safe to re-run on a schema where the column was already dropped (or
-- never existed). This matches the repo's migration idempotency convention
-- (see `migrations.rs` §"All migration files use CREATE TABLE IF NOT EXISTS
-- and CREATE INDEX IF NOT EXISTS so they are safe to re-run").
--
-- Tracked refinery migration (not a manual deploy script): versioned,
-- re-runnable, recorded in `refinery_schema_history`, applied atomically
-- with the rest of the chain. refinery auto-discovers this file via
-- `embed_migrations!("migrations")`.

-- ── reborn_skills (class 11, V027) ──────────────────────────────────────────
ALTER TABLE reborn_skills DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_actions (class 21, V029) ─────────────────────────────────────────
ALTER TABLE reborn_actions DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_tools (class 0, V030) ────────────────────────────────────────────
ALTER TABLE reborn_tools DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_extensions_unified (V032) ────────────────────────────────────────
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_recipes (V033) ───────────────────────────────────────────────────
ALTER TABLE reborn_recipes DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_specs (class 12, V036) ───────────────────────────────────────────
ALTER TABLE reborn_specs DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_tool_skills (class 13, V037) ─────────────────────────────────────
ALTER TABLE reborn_tool_skills DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_plans (class 14, V038) ───────────────────────────────────────────
ALTER TABLE reborn_plans DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_summaries (class 15, V039) ───────────────────────────────────────
ALTER TABLE reborn_summaries DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_docus (class 17, V040) ───────────────────────────────────────────
ALTER TABLE reborn_docus DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_lessons (class 18, V041) ─────────────────────────────────────────
ALTER TABLE reborn_lessons DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_issues (class 19, V042) ──────────────────────────────────────────
ALTER TABLE reborn_issues DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_notes (class 20, V043) ───────────────────────────────────────────
ALTER TABLE reborn_notes DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_python_code (V052) ───────────────────────────────────────────────
ALTER TABLE reborn_python_code DROP COLUMN IF EXISTS parent_mission_id;

-- ── reborn_extension_catalogues (V053) ──────────────────────────────────────
ALTER TABLE reborn_extension_catalogues DROP COLUMN IF EXISTS parent_mission_id;
