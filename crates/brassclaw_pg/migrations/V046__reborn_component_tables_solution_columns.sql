-- V046__reborn_component_tables_solution_columns.sql
--
-- Adds the SCH-02 solution-override columns to the eight former-DocType
-- component tables introduced in Phase 5 Step 5.3 (V036–V043):
--
--   prior_knowledge_content TEXT
--     When non-NULL, used as the component's prior-knowledge text instead of
--     assembling from `content`.  Default NULL → standard content-is-king path.
--
--   override_prompt_creation BOOLEAN NOT NULL DEFAULT false
--     If true, the Solution Override path is taken and this component's
--     prior_knowledge_content replaces the standard assembly.
--
-- These columns are already present on reborn_extensions_unified (V032) and
-- reborn_recipes (V033).  Spec references: §3.13/§3.14 (SCH-02).
--
-- Also adds missing similarity/lineage indexes on similarity_parent_id and
-- replaces_id for each of the eight tables (V036–V043).  The columns exist
-- since those migrations but the indexes were omitted from the initial DDL.
--
-- All ALTER TABLE / CREATE INDEX statements use IF NOT EXISTS / ADD COLUMN IF
-- NOT EXISTS so the migration is idempotent and safe to apply on a schema that
-- was partially migrated by hand.

-- ── reborn_specs (class 12) ─────────────────────────────────────────────────

ALTER TABLE reborn_specs
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_specs_similarity_parent_idx
    ON reborn_specs (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_specs_replaces_idx
    ON reborn_specs (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_tool_skills (class 13) ───────────────────────────────────────────

ALTER TABLE reborn_tool_skills
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_tool_skills_similarity_parent_idx
    ON reborn_tool_skills (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_tool_skills_replaces_idx
    ON reborn_tool_skills (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_plans (class 14) ─────────────────────────────────────────────────

ALTER TABLE reborn_plans
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_plans_similarity_parent_idx
    ON reborn_plans (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_plans_replaces_idx
    ON reborn_plans (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_summaries (class 15) ─────────────────────────────────────────────

ALTER TABLE reborn_summaries
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_summaries_similarity_parent_idx
    ON reborn_summaries (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_summaries_replaces_idx
    ON reborn_summaries (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_docus (class 17) ─────────────────────────────────────────────────

ALTER TABLE reborn_docus
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_docus_similarity_parent_idx
    ON reborn_docus (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_docus_replaces_idx
    ON reborn_docus (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_lessons (class 18) ───────────────────────────────────────────────

ALTER TABLE reborn_lessons
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_lessons_similarity_parent_idx
    ON reborn_lessons (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_lessons_replaces_idx
    ON reborn_lessons (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_issues (class 19) ────────────────────────────────────────────────

ALTER TABLE reborn_issues
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_issues_similarity_parent_idx
    ON reborn_issues (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_issues_replaces_idx
    ON reborn_issues (replaces_id)
    WHERE replaces_id IS NOT NULL;

-- ── reborn_notes (class 20) ─────────────────────────────────────────────────

ALTER TABLE reborn_notes
    ADD COLUMN IF NOT EXISTS prior_knowledge_content  TEXT,
    ADD COLUMN IF NOT EXISTS override_prompt_creation BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS reborn_notes_similarity_parent_idx
    ON reborn_notes (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS reborn_notes_replaces_idx
    ON reborn_notes (replaces_id)
    WHERE replaces_id IS NOT NULL;
