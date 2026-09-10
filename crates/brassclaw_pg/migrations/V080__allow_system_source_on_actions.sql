-- V080__allow_system_source_on_actions.sql
-- Widen the `source` CHECK on `reborn_actions` (class 16) to allow 'system',
-- matching the widening already applied to `reborn_tools` (class 0) and
-- `reborn_skills` (class 1/2) in V066.
--
-- Why needed: Phase P Step 7 seeds the `doc-sync` Action with
-- `source = 'system'` from `builtin_bootstrap.rs`. V066 missed `reborn_actions`
-- because Actions were not seeded as system builtins at that time.
--
-- `reborn_python_code` (V052) and `reborn_extension_catalogues` (V053) already
-- allow 'system'. `reborn_tool_skills` (V037) and `reborn_recipes` (V033) have
-- no CHECK on `source`. Only `reborn_actions` still forbids it.
--
-- Tenant isolation is preserved: retrieval always anchors on `tenant_id = $1`
-- and only relaxes the user/agent/project predicates for validated system rows.

ALTER TABLE reborn_actions
    DROP CONSTRAINT IF EXISTS reborn_actions_source_check;
ALTER TABLE reborn_actions
    ADD CONSTRAINT reborn_actions_source_check
    CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
