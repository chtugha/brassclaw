-- Step C.4.5.5 — Common component syntax: Skill (classes 1/2/3/10) schema
-- standardization (item g) + the rust-side common form.
--
-- (1) DROP the 5 legacy pre-centralization lifecycle columns. V027 (the
--     original reborn_skills DDL) lines 99-106 carried `validation_errors`
--     (TEXT[]), `review_feedback`, `review_attempts`, `rejected_at` and
--     `queue_code` — the SAME 5-col legacy set V071 dropped from reborn_tools.
--     The central `reborn_validation_queue` (V051) tracks the full lifecycle
--     via `state` (1-4) + its OWN `validation_errors` (V051:72) +
--     `review_feedback` (V051:69); the Q2 graduation path only ever sets
--     `validation_status` on this table. `parent_mission_id` was already
--     dropped workspace-wide by V064.
--
--     Unlike reborn_tools (where the columns had ZERO Rust readers), these 5
--     columns WERE actively read/written by `brassclaw_skills::db_store::
--     DbSkillStore` (compiled under the `db-store` feature, which the
--     composition `skills-db` feature enables). This migration is paired with
--     a refactor of `DbSkillStore` that removes every reference to them
--     (struct fields, INSERT/SELECT/UPDATE column lists, the `row_from_pg`
--     decoder, and the now-unused `mark_auto_failed` errors arg) so the store
--     relies on `validation_status` + the central `reborn_validation_queue`
--     for lifecycle — mirroring the reborn_tools path centralised by V071.
--     `DROP COLUMN IF EXISTS` is forward-compatible with the future V059/
--     Phase-N workspace-wide drop (which no-ops on this table).
--
-- (2) WIDEN the `class_code` CHECK from `IN (1, 2, 3)` to
--     `IN (1, 2, 3, 10, 50)`. V027's original CHECK only permitted the three
--     skill classes (01 Rusty / 02 Monty / 03 LLM), but the code already
--     assumes classes 10 (Orchestrator) and 50 (Scaffold) live in this table:
--       • retrieval_source.rs:1052 maps `10 | 50` -> `reborn_skills` for
--         `fetch_component_by_id` (comment FIND-NEW-AUDIT-06: "MUST be present
--         or Phase E silently loses retrieval for these classes").
--       • V061's components-registry trigger + backfill comment documents
--         `reborn_skills (classes 1-3, 10, 50)`.
--     The un-widened CHECK meant class 10/50 rows could never be INSERTed — a
--     latent contradiction. Widening it lets C.5/C.6 save the orchestrator
--     script as a class-10 row (its `body` column holds the script, matching
--     the retrieval `content_expr`). The old constraint is dropped by its
--     Postgres-assigned name `reborn_skills_class_code_check` (the same
--     `<table>_<column>_check` naming V066 relied on for
--     `reborn_skills_source_check`); `IF EXISTS` guards against a rename.

-- (1) drop the 5 legacy pre-centralization lifecycle columns.
ALTER TABLE reborn_skills DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_skills DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_skills DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_skills DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_skills DROP COLUMN IF EXISTS queue_code;

-- (2) widen the class_code CHECK so the table actually holds classes 10 + 50
--     as the retrieval code + V061 registry already assume.
ALTER TABLE reborn_skills DROP CONSTRAINT IF EXISTS reborn_skills_class_code_check;
ALTER TABLE reborn_skills ADD CONSTRAINT reborn_skills_class_code_check
    CHECK (class_code IN (1, 2, 3, 10, 50));
