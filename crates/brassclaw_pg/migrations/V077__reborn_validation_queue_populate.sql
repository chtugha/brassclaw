-- V077__reborn_validation_queue_populate.sql
-- Phase N — Validation Queue Populate + Drop
--
-- Step 1: CREATE TABLE is NOT here — reborn_validation_queue was created in
-- V051__reborn_validation_queue.sql (Phase A.5). Do NOT re-create the table.
--
-- Step 2: populate from existing component table state.
--
-- FIND-N-03: V070–V075 already dropped the five legacy columns from 12 of the
-- 13 original component tables. As of Phase N, ONLY reborn_recipes still carries
-- review_attempts / review_feedback / validation_errors / rejected_at / queue_code.
-- Every other table's arm uses literal defaults for those fields.
--
-- Two patterns:
--   Pattern A — reborn_recipes (only table that still has the 5 legacy columns)
--   Pattern B — all 14 other tables (cleaned by V070–V075 or never had them)
--
-- FIND-P6-08: Use class_code::SMALLINT for variable-class tables
-- (reborn_skills: 1/2/3/10/50; reborn_extensions_unified: 4-9).
-- Use a literal for tables with a fixed class code.
--
-- SCHEMA-01: reborn_recipes.review_attempts is SMALLINT → ::INT cast required.
-- All Pattern-B arms use the literal 0, so no cast needed there.
--
-- ON CONFLICT DO NOTHING: idempotent — re-running the migration skips rows that
-- were already submitted between V051 landing and V077 running.

-- ─── Pattern A: reborn_recipes (class 21, has legacy columns) ───────────────
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id,
     component_id, component_class, state, counter,
     review_feedback, validation_errors, submitted_at)
SELECT
    tenant_id, user_id, agent_id, project_id,
    id,
    21::SMALLINT,
    CASE validation_status
        WHEN 'pending'           THEN 1
        WHEN 'upgrade_queued'    THEN 1
        WHEN 'auto_failed'       THEN 1
        WHEN 'auto_passed'       THEN 2
        WHEN 'review_requested'  THEN 2
        WHEN 'rejected'          THEN 3
        WHEN 'garbage'           THEN 4
        ELSE 1
    END::SMALLINT,
    COALESCE(review_attempts::INT, 0),
    review_feedback,
    COALESCE(validation_errors, '{}'::TEXT[]),
    created_at
FROM reborn_recipes
WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- ─── Pattern B: all 14 tables already cleaned (literals for legacy fields) ───

-- reborn_skills (variable class: 1/2/3/10/50)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, class_code::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_skills WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_tools (class 0)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 0::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_tools WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_tool_skills (class 13)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 13::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_tool_skills WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_actions (class 16)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 16::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_actions WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_specs (class 12)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 12::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_specs WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_plans (class 14)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 14::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_plans WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_summaries (class 15)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 15::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_summaries WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_docus (class 17)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 17::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_docus WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_lessons (class 18)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 18::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_lessons WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_issues (class 19)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 19::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_issues WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_notes (class 20)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 20::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_notes WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_extensions_unified (variable class: 4-9)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, class_code::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_extensions_unified WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_python_code (class 22 — Phase B; never had legacy columns)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 22::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_python_code WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- reborn_extension_catalogues (class 23 — Phase C; never had legacy columns)
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id, component_id, component_class,
     state, counter, review_feedback, validation_errors, submitted_at)
SELECT tenant_id, user_id, agent_id, project_id, id, 23::SMALLINT,
    CASE validation_status WHEN 'pending' THEN 1 WHEN 'upgrade_queued' THEN 1
        WHEN 'auto_failed' THEN 1 WHEN 'auto_passed' THEN 2
        WHEN 'review_requested' THEN 2 WHEN 'rejected' THEN 3
        WHEN 'garbage' THEN 4 ELSE 1 END::SMALLINT,
    0, NULL, '{}'::TEXT[], created_at
FROM reborn_extension_catalogues WHERE validation_status != 'validated'
ON CONFLICT (tenant_id, user_id, agent_id, project_id, component_id) DO NOTHING;

-- ─── Step 3: add last_graduation_at cursor column ────────────────────────────
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN IF NOT EXISTS last_graduation_at TIMESTAMPTZ;

-- ─── Step 4: graduation trigger ──────────────────────────────────────────────
-- Bumps last_graduation_at on the scope cursor row whenever a queue row is
-- deleted (= component graduates). Uses INSERT … ON CONFLICT so the first
-- graduation atomically creates the cursor row if it doesn't exist yet.
-- All resource columns on reborn_monty_vm_settings have NOT NULL DEFAULT (V034),
-- so the 5-column INSERT is always valid.
CREATE OR REPLACE FUNCTION reborn_validation_queue_graduation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO reborn_monty_vm_settings
        (tenant_id, user_id, agent_id, project_id, last_graduation_at)
    VALUES
        (OLD.tenant_id, OLD.user_id, OLD.agent_id, OLD.project_id, now())
    ON CONFLICT (tenant_id, user_id, agent_id, project_id)
    DO UPDATE SET last_graduation_at = now();
    -- AFTER DELETE trigger — RETURN NULL is correct (FIND-13).
    RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS reborn_validation_queue_on_delete ON reborn_validation_queue;
CREATE TRIGGER reborn_validation_queue_on_delete
    AFTER DELETE ON reborn_validation_queue
    FOR EACH ROW EXECUTE FUNCTION reborn_validation_queue_graduation();

-- ─── Step 5: drop legacy columns ─────────────────────────────────────────────
-- Only reborn_recipes carries these columns today (FIND-N-03: V070–V075 already
-- cleaned the other 12 tables). The remaining ALTERs are IF EXISTS no-ops
-- included for completeness as the workspace-wide drop V051 always intended.

ALTER TABLE reborn_recipes
    DROP COLUMN IF EXISTS queue_code,
    DROP COLUMN IF EXISTS review_attempts,
    DROP COLUMN IF EXISTS review_feedback,
    DROP COLUMN IF EXISTS rejected_at,
    DROP COLUMN IF EXISTS validation_errors;

-- No-ops (already dropped by V070–V075):
ALTER TABLE reborn_skills            DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_tools             DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_tool_skills       DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_actions           DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_specs             DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_plans             DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_summaries         DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_docus             DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_lessons           DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_issues            DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_notes             DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_extensions_unified DROP COLUMN IF EXISTS queue_code, DROP COLUMN IF EXISTS review_attempts, DROP COLUMN IF EXISTS review_feedback, DROP COLUMN IF EXISTS rejected_at, DROP COLUMN IF EXISTS validation_errors;
-- reborn_python_code and reborn_extension_catalogues never had these columns.
-- validation_status is NOT dropped — it remains as the post-validation gate.
