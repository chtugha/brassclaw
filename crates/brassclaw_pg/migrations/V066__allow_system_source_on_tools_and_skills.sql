-- V066__allow_system_source_on_tools_and_skills.sql
-- Widen the `source` CHECK on the class-0 (Tool) and class-1 (Skill) tables to
-- allow 'system', so builtin host components (Step 27 / Phase C.2) can be seeded
-- with `source = 'system'` and the retrieval UNION in `fetch_for_turn` /
-- `DbToolSource::fetch_tool_names` can surface them tenant-globally (anchored on
-- tenant_id, agnostic on user_id/agent_id/project_id) without a per-scope seed.
--
-- Why now: Phase C.2 seeds the `host.*` Tool rows (class 0) + their leaf Skills
-- (class 1) as validated builtins. `reborn_python_code` (V052) and
-- `reborn_extension_catalogues` (V053) already allow 'system'; `reborn_tool_skills`
-- (V037) and `reborn_recipes` (V033) have no CHECK on `source`. Only the two
-- older tables below forbid it. This is the "V057" referenced in the V052/V053
-- comments but never written.
--
-- Tenant isolation is preserved: the retrieval UNION keeps `tenant_id = $1`
-- anchoring (no cross-tenant leak) and only relaxes the user/agent/project
-- predicates for `source = 'system'` validated rows.

ALTER TABLE reborn_tools
    DROP CONSTRAINT IF EXISTS reborn_tools_source_check;
ALTER TABLE reborn_tools
    ADD CONSTRAINT reborn_tools_source_check
    CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));

ALTER TABLE reborn_skills
    DROP CONSTRAINT IF EXISTS reborn_skills_source_check;
ALTER TABLE reborn_skills
    ADD CONSTRAINT reborn_skills_source_check
    CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
