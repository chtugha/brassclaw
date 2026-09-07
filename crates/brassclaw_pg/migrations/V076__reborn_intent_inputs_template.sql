-- V076__reborn_intent_inputs_template.sql
-- Phase M — Variable Intent Templates (§0.17 / §0.17.1).
--
-- Adds `%` slot marker support to intent expressions. Authors write e.g.
--   "show me all files in the % directory"
-- as an intent expression and resolve_intent matches user text that fits the
-- template. Two computed anchor columns (template_prefix / template_suffix)
-- drive the three-path index dispatch in resolve_intent (§0.17.1):
--
--   Path 0 — exact match (existing, unchanged): input_text = $5
--   Path 1 — prefix-anchored template (template_prefix != ''):
--            $5 LIKE (template_prefix || '%')  → B-tree (scope, template_prefix)
--   Path 2 — suffix-anchored template (template_prefix = '', template_suffix != ''):
--            reverse($5) LIKE (reverse(template_suffix) || '%')
--            → functional B-tree (scope, reverse(template_suffix))
--   Path 3 — dual-anchored (prefix != '' AND suffix != '') rides Path 1's index.
--   Blocked — no anchor (prefix = '' AND suffix = ''): Q1 hard error, never
--   reaches the DB.
--
-- `is_template` is the template-row flag; `parse_template` (Phase M.2) populates
-- all three columns from the `%` markers in `input_text`. Legacy rows keep
-- is_template = false and the new columns NULL, so the existing exact-match
-- path is unchanged.
--
-- Sequencing invariant: the Rust code that SELECTs these columns (resolve_intent
-- Phase M.3) requires this migration to have run first (V076 → deploy code).
-- V076 (not the plan's stale V058) because V056-V059 were reshuffled into
-- V060-V075 by later phases; V076 is the next sequential slot.

ALTER TABLE reborn_intent_inputs
    ADD COLUMN IF NOT EXISTS is_template      BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS template_prefix  TEXT,
    ADD COLUMN IF NOT EXISTS template_suffix  TEXT;

-- Path 1 index: prefix-anchored templates (template_prefix != '').
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_prefix_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, template_prefix)
    WHERE is_template = true AND template_prefix != '';

-- Path 2 index: suffix-anchored templates (leading-% case). Functional B-tree
-- on reverse(template_suffix) so reverse($5) LIKE (reverse(suffix) || '%') is
-- index-supported.
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_suffix_rev_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, reverse(template_suffix))
    WHERE is_template = true AND template_prefix = '' AND template_suffix != '';
