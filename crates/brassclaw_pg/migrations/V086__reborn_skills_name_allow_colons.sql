-- V086: relax reborn_skills name constraint to allow colons.
--
-- The original V027 CHECK `'^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'` only allows
-- lowercase letters, digits, and hyphens. System-seeded class-10 and class-50
-- components use namespaced names with colons (e.g. 'orchestrator:main',
-- 'subagent:direction:general', 'codeact_preamble'). This migration widens
-- the constraint to also allow colons and underscores, which are used in
-- built-in component names.
--
-- New pattern: '^[a-z0-9_][a-z0-9_:.-]*$'
-- Allows: lowercase letters, digits, underscores, colons, dots, hyphens.
-- Still requires the name to start with a lowercase letter, digit, or underscore.
-- Max length unchanged (64 characters enforced by the length() CHECK).

ALTER TABLE reborn_skills
    DROP CONSTRAINT IF EXISTS reborn_skills_name_check;

ALTER TABLE reborn_skills
    ADD CONSTRAINT reborn_skills_name_check
    CHECK (name ~ '^[a-z0-9_][a-z0-9_:.\-]*$' AND length(name) BETWEEN 1 AND 64);
