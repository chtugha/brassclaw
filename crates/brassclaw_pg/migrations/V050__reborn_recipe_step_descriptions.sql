-- V050__reborn_recipe_step_descriptions.sql
--
-- Adds the three v3 Recipe authoring columns to reborn_recipes (class 21).
-- Phase A (StepDescription Schema + IBS Core).
--
--   step_descriptions JSONB
--     The authored StepDescription array (§0.4.1) — the IBS authoring model.
--     Phase E's PostgresSource::fetch_for_turn reads this and calls
--     build_instruction(step_link, step_descriptions, variable_patterns) to
--     compile a BuildInstruction for Tier-0 deterministic execution.
--
--   variants JSONB
--     Vec<RecipeVariant> — the variant table for a recipe (§0.16.1). Each
--     variant carries variant_key, step_link, intent_examples, and the nested
--     variable_patterns. Phase M.5 depends on variable_patterns nested in
--     variants. Recipe-specific: no other component table has this column.
--
--   dependency_registry JSONB
--     The component dependency-graph entry for this recipe (§0.19, Phase J.2).
--     Also added to the other 12 component tables in V055 (Phase J.2, was V054
--     before Decision 2); V055's reborn_recipes line is `ADD COLUMN IF NOT
--     EXISTS` → idempotent no-op here.
--
-- V050 carries ALL THREE columns so the Phase A store round-trip
-- (PgRecipe / NewPgRecipe / RECIPE_SELECT / decode_recipe_row / INSERT / UPDATE,
-- which reads+writes all three at indices 31/32/33) is never orphaned. Creating
-- all three here closes:
--   * VARPAT-COL-GAP — `variants` had no migration anywhere in V050–V058, yet
--     Phase A persists it and Phase M.5 reads variable_patterns nested in it.
--   * DEPREG-TIMING-GAP — `dependency_registry` on reborn_recipes is not
--     created until V055, but Phase A round-trips it from V050.
--
-- All statements use ADD COLUMN IF NOT EXISTS so the migration is idempotent
-- and safe to apply on a schema that was partially migrated by hand. Additive
-- only — no DROP, no renames, no existing rows break. NULLable (existing rows
-- backfill to NULL; the store treats NULL as empty/none).

ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS step_descriptions   JSONB;
ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS variants            JSONB;
ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
