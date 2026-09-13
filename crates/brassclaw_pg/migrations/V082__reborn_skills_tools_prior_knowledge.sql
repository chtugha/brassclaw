-- V082__reborn_skills_tools_prior_knowledge.sql
--
-- Adds the SCH-02 solution-override columns to `reborn_skills` and
-- `reborn_tools`. V046 added these columns to the eight V036-V043 component
-- tables but skipped the two earlier ones.
--
-- `prior_knowledge_content TEXT`
--   When non-NULL, used as the component's prior-knowledge text instead of
--   assembling from `body` / `description`. Default NULL → standard path.
--
-- `override_prompt_creation BOOLEAN NOT NULL DEFAULT false`
--   If true, `prior_knowledge_content` replaces the standard assembly
--   (SCH-02).
--
-- Both columns are referenced by `do_assemble_bundle` in
-- `interceptor_config_service.rs`:
--   reborn_skills:  COALESCE(NULLIF(prior_knowledge_content,''), body)
--   reborn_tools:   COALESCE(prior_knowledge_content, description)
--
-- Without these columns the `do_assemble_bundle` queries for those tables
-- fail (postgres column-not-found), causing the bundle assembler to silently
-- skip all validated skill and tool rows. The Regenerate button in the
-- Prefix tab then produces an empty / near-empty bundle.
--
-- ADD COLUMN IF NOT EXISTS / DEFAULT false makes this idempotent.

-- ── reborn_skills (classes 1/2/3/10/50) ────────────────────────────────────

ALTER TABLE reborn_skills
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

-- ── reborn_tools (class 0) ──────────────────────────────────────────────────

ALTER TABLE reborn_tools
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;
