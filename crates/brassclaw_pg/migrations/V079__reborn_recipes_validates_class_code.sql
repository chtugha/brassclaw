-- V079__reborn_recipes_validates_class_code.sql
-- Phase P.0 fix: add validates_class_code to reborn_recipes.
--
-- The find_validator_recipe query in q1_orchestrator.rs was filtering on
-- reborn_recipes.class_code = $target_class, but class_code is always 21
-- (enforced by CHECK constraint — it means "this row IS a Recipe").
-- That column never carries "this Recipe validates components of class X."
--
-- This migration adds validates_class_code SMALLINT (NULL = general-purpose
-- Recipe; non-NULL = validator Recipe for that component class).
--
-- NULL for all existing rows (they are general-purpose Recipes, not validators).
-- No CHECK constraint: any i16 class code is valid.
ALTER TABLE reborn_recipes
    ADD COLUMN IF NOT EXISTS validates_class_code SMALLINT;

-- Index for the validator Recipe lookup in q1_orchestrator (scoped, by class).
CREATE INDEX IF NOT EXISTS reborn_recipes_validates_class_idx
    ON reborn_recipes (tenant_id, user_id, agent_id, project_id, validates_class_code)
    WHERE validates_class_code IS NOT NULL
      AND validation_status = 'validated';
