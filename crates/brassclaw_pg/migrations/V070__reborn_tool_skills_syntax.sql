-- V070__reborn_tool_skills_syntax.sql
-- Step C.4.5.3 — Common component syntax: ToolSkill (class 13) schema
-- standardization (item g) + structural-include machine form.
--
-- (1) DROP legacy `queue_code` + `validation_errors` columns.
--     V051 (reborn_validation_queue) lines 8-9 document these as pre-
--     centralization remnants slated for drop: the central validation queue
--     tracks lifecycle via `state` (1-4) + its own `validation_errors` column,
--     NOT via the component-table `queue_code`. No Rust reader touches
--     `reborn_tool_skills.{queue_code,validation_errors}` (retrieval_source
--     selects neither; pg_tool_skill_store has no decode SELECT; only DDL
--     defaults ever populated them). Recipes/skills/extensions keep theirs
--     (still live in their stores) — out of scope here; the workspace-wide
--     legacy drop is V059 (Phase N), which uses DROP COLUMN IF EXISTS and so
--     no-ops on this table.
--
-- (2) ADD `includes JSONB` — machine form of `{{component_name}}` structural-
--     include placeholders (F-HI-2=A). A ToolSkill description may include
--     another ToolSkill's description text (a description includes a
--     description); the composer (C.4.5.17) inlines the referenced component's
--     description at the `{{component_name}}` slot. Mirrors PythonCode `includes`
--     (V069) + recipe `StepEntry.include` (Phase A). DEFAULT '[]' so every
--     pre-existing row stays valid (no includes).

ALTER TABLE reborn_tool_skills DROP COLUMN IF EXISTS queue_code;
ALTER TABLE reborn_tool_skills DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_tool_skills
    ADD COLUMN IF NOT EXISTS includes JSONB NOT NULL DEFAULT '[]'::jsonb;
