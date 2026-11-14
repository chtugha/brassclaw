-- Successor format support for monty-retained-recipe-selection.
--
-- V103 and V106 added CHECK constraints that accept only the /1 format tag.
-- This migration drops those two strict /1-only constraints and replaces them
-- with constraints that accept /1 or /3. All existing /1 rows remain valid;
-- no stored bytes are rewritten. The application parser continues to accept
-- only the two declared format versions: new format versions must add their
-- own successor migration.
--
-- Both tables are immutable (protected by triggers); altering constraints does
-- not affect the immutability guarantees on existing rows.

-- `/2` is reserved by the separate unpublished Q/N capture/profile codec.
-- This bounded package does not adopt it or alter that owner's constraints.
-- Refuse a mixed predecessor explicitly, before dropping any format gate.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid IN ('brassclaw_monty_recipe_selections'::regclass,
                           'reborn_workflow_review_evidence'::regclass)
          AND contype='c'
          AND pg_get_constraintdef(oid) LIKE '%monty-retained-recipe-selection/2%'
    ) THEN
        RAISE EXCEPTION 'retained selection /2 requires coordinated format adoption'
            USING ERRCODE='23514';
    END IF;
END $$;

-- ────────────────────────────────────────────────────────────────────────────
-- brassclaw_monty_recipe_selections (V103)
-- ────────────────────────────────────────────────────────────────────────────

-- Drop the original /1-only format check. PostgreSQL requires the exact
-- constraint name; we know it from V103 (unnamed constraints get a system
-- name; we must drop by expression via a named one). Since V103 used an
-- unnamed CHECK, we identify it by table and expression, then re-add as
-- a named constraint so future migrations can reference it cleanly.
ALTER TABLE brassclaw_monty_recipe_selections
    DROP CONSTRAINT IF EXISTS brassclaw_monty_recipe_selections_format_v1;

-- Remove the original unnamed CHECK on format if it survived under its
-- system-generated name. We use a DO block to handle the case gracefully
-- in case the constraint name differs across installations.
DO $$
DECLARE
    _con text;
BEGIN
    SELECT conname INTO _con
    FROM pg_constraint
    WHERE conrelid = 'brassclaw_monty_recipe_selections'::regclass
      AND contype = 'c'
      AND pg_get_constraintdef(oid) LIKE '%monty-retained-recipe-selection/1%'
      AND pg_get_constraintdef(oid) NOT LIKE '%monty-retained-recipe-selection/3%';
    IF _con IS NOT NULL THEN
        EXECUTE format('ALTER TABLE brassclaw_monty_recipe_selections DROP CONSTRAINT %I', _con);
    END IF;
END
$$;

-- Add the versioned successor constraint that accepts /1 and /3.
ALTER TABLE brassclaw_monty_recipe_selections
    ADD CONSTRAINT brassclaw_monty_recipe_selections_format_v1_or_v3
    CHECK (
        (
            selection_bytes::jsonb->>'format' = 'monty-retained-recipe-selection/1'
            OR selection_bytes::jsonb->>'format' = 'monty-retained-recipe-selection/3'
        ) IS TRUE
    );

-- ────────────────────────────────────────────────────────────────────────────
-- reborn_workflow_review_evidence (V106)
-- ────────────────────────────────────────────────────────────────────────────

DO $$
DECLARE
    _con text;
BEGIN
    SELECT conname INTO _con
    FROM pg_constraint
    WHERE conrelid = 'reborn_workflow_review_evidence'::regclass
      AND contype = 'c'
      AND pg_get_constraintdef(oid) LIKE '%monty-retained-recipe-selection/1%'
      AND pg_get_constraintdef(oid) NOT LIKE '%monty-retained-recipe-selection/3%';
    IF _con IS NOT NULL THEN
        EXECUTE format('ALTER TABLE reborn_workflow_review_evidence DROP CONSTRAINT %I', _con);
    END IF;
END
$$;

ALTER TABLE reborn_workflow_review_evidence
    ADD CONSTRAINT reborn_workflow_review_evidence_format_v1_or_v3
    CHECK (
        (
            selection_bytes::jsonb->>'format' = 'monty-retained-recipe-selection/1'
            OR selection_bytes::jsonb->>'format' = 'monty-retained-recipe-selection/3'
        ) IS TRUE
    );
