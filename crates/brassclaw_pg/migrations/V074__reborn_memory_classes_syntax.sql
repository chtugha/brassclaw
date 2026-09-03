-- V074__reborn_memory_classes_syntax.sql
-- C.4.5.7 — memory/instruction classes (12/14/15/17/18/19/20) DB-structure
-- standardization (item g): drop the 5 legacy pre-centralization lifecycle
-- columns from all 7 narrative memory tables, mirroring V071 (`reborn_tools`),
-- V072 (`reborn_skills`) and V073 (`reborn_actions`). Per F7=A this single
-- migration batches all 7 uniform classes into one slice (C.4.5.8–C.4.5.13
-- collapse into C.4.5.7).
--
-- The 7 tables (created V036–V043) are uniform narrative memory components:
--   reborn_specs      (class 12) — V036
--   reborn_plans      (class 14) — V038
--   reborn_summaries  (class 15) — V039
--   reborn_docus      (class 17) — V040
--   reborn_lessons    (class 18) — V041
--   reborn_issues     (class 19) — V042
--   reborn_notes      (class 20) — V043
--
-- (1) DROP the 5 legacy pre-centralization lifecycle columns
--     (`validation_errors`, `review_feedback`, `review_attempts`,
--     `rejected_at`, `queue_code`) — the SAME 5-col legacy set V071/V072/V073
--     dropped from the tool/skill/action tables. The central
--     `reborn_validation_queue` (V051) tracks the full lifecycle via `state`
--     (1-4) + its OWN `validation_errors` (V051:72) + `review_feedback`
--     (V051:69); the Q2 graduation path only ever sets `validation_status` on
--     these tables. `parent_mission_id` was already dropped workspace-wide by
--     V064.
--
--     These 5 columns have ZERO Rust readers/writers — NO paired Rust refactor
--     + NO test changes are needed (unlike V072's `DbSkillStore` refactor):
--       • The class `12 | 14 | 17..=20` Q1 gate (component_validator.rs:301) is
--         `validate_soft_budget_named` — a content check that touches no DB col.
--       • The retrieval SELECT projection (retrieval_source.rs
--         `class_code_to_table` + `fetch_component_by_id`) reads
--         id/class_code/prompt_uid/name/description/effective_content — NONE of
--         the 5 legacy.
--       • `component_import.rs` (the MemoryDoc→class-table migration) INSERTs
--         into `{table}` naming only scope/name/description/content/
--         content_hash/consumer_tags/intent_examples/source/validation_status.
--       • The only literal INSERT (`validation_queue.rs:954` into reborn_notes)
--         names only scope/name/validation_status.
--       • The legacy-col names elsewhere in .rs (similarity_checker /
--         recipe_matcher / recipe_validator) are the RETIRING `MemoryDoc`
--         struct (types/memory.rs confirms it is legacy, being replaced by
--         these class tables) — NOT reads of these 7 tables.
--
-- (2) Q1 GATE (no change). The catch-all `12 | 14 | 17..=20` arm runs
--     `validate_soft_budget_named` (soft 10k budget) — correct for narrative
--     memory content. These classes carry NO `{{ ... }}` placeholders (authored
--     text, not assembled code), so NO placeholder-grammar gate is required
--     (unlike class 10/13/22). The common-syntax treatment for the memory
--     classes is DB-cleanup only; the narrative-formatting conventions (item b)
--     + command-syntax references (item e) are docs-only (C.4.5.18).
--
-- `DROP COLUMN IF EXISTS` is forward-compatible with the future V059/Phase-N
-- workspace-wide drop (which no-ops on these tables). Any CHECK on `queue_code`
-- auto-drops with the column; these 7 tables define no inline CHECK on
-- `queue_code` in their V036–V043 DDL (unlike reborn_actions V029), so no
-- explicit constraint drop is needed.

-- reborn_specs (class 12)
ALTER TABLE reborn_specs DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_specs DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_specs DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_specs DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_specs DROP COLUMN IF EXISTS queue_code;

-- reborn_plans (class 14)
ALTER TABLE reborn_plans DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_plans DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_plans DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_plans DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_plans DROP COLUMN IF EXISTS queue_code;

-- reborn_summaries (class 15)
ALTER TABLE reborn_summaries DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_summaries DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_summaries DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_summaries DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_summaries DROP COLUMN IF EXISTS queue_code;

-- reborn_docus (class 17)
ALTER TABLE reborn_docus DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_docus DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_docus DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_docus DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_docus DROP COLUMN IF EXISTS queue_code;

-- reborn_lessons (class 18)
ALTER TABLE reborn_lessons DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_lessons DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_lessons DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_lessons DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_lessons DROP COLUMN IF EXISTS queue_code;

-- reborn_issues (class 19)
ALTER TABLE reborn_issues DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_issues DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_issues DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_issues DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_issues DROP COLUMN IF EXISTS queue_code;

-- reborn_notes (class 20)
ALTER TABLE reborn_notes DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_notes DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_notes DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_notes DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_notes DROP COLUMN IF EXISTS queue_code;
