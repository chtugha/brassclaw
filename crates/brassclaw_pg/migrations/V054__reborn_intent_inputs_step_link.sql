-- V054__reborn_intent_inputs_step_link.sql
-- Phase D — add `step_link` TEXT column to reborn_intent_inputs (§0.6 / §0.8).
--
-- `step_link` carries the IBS step-range formula (e.g. "1:1-1:3") for a matched
-- Recipe variant intent (class 21). resolve_intent returns it in
-- IntentResolution::Match so Phase E can run build_instruction(step_link, …)
-- synchronously inside fetch_for_turn (the SplitResult / ActionShortCircuit
-- path). Nullable: legacy rows and non-Recipe intents keep step_link = NULL and
-- continue to use the existing fetch_component_by_id path unchanged.
--
-- `step_link` replaces the earlier `variant_key` concept (no variant_key column
-- is added). CHECK length <= 4096 guards against pathological inputs.
--
-- Sequencing invariant: the Rust code that SELECTs step_link requires this
-- migration to have run first (V054 → deploy code).

ALTER TABLE reborn_intent_inputs
    ADD COLUMN IF NOT EXISTS step_link TEXT
        CHECK (length(step_link) <= 4096);
