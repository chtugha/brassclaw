-- V073__reborn_actions_syntax.sql
-- C.4.5.6 — `action` (class 16) DB-structure standardization (item g): drop the
-- 5 legacy pre-centralization lifecycle columns from `reborn_actions`, mirroring
-- V071 (`reborn_tools`) and V072 (`reborn_skills`).
--
-- (1) DROP the 5 legacy pre-centralization lifecycle columns. V029 (the original
--     reborn_actions DDL) lines 93-100 carried `validation_errors` (TEXT[]),
--     `review_feedback`, `review_attempts`, `rejected_at` and `queue_code` —
--     the SAME 5-col legacy set V071 dropped from reborn_tools and V072 dropped
--     from reborn_skills. The central `reborn_validation_queue` (V051) tracks
--     the full lifecycle via `state` (1-4) + its OWN `validation_errors`
--     (V051:72) + `review_feedback` (V051:69); the Q2 graduation path only ever
--     sets `validation_status` on this table. `parent_mission_id` was already
--     dropped workspace-wide by V064.
--
--     Unlike reborn_skills (V072, where the columns WERE actively read/written
--     by `DbSkillStore` under the `db-store` feature), `reborn_actions` has NO
--     dedicated store: there is no `PgAction`/`pg_action_store` and the only
--     INSERTs are raw SQL in tests + `retrieval_lookup_impl.rs` seed. NONE of
--     those INSERTs name any of the 5 legacy columns (they specify only
--     id/scope/name/description/class_code|validation_status|steps|
--     allowed_tools), and the class-16 SELECT projection in
--     `retrieval_source.rs` (id, class_code, prompt_uid, name, description,
--     effective_content, override_prompt_creation, steps, allowed_tools) reads
--     NONE of them. So the 5 columns have ZERO Rust readers/writers — this
--     migration needs NO paired Rust refactor (unlike V072). `DROP COLUMN IF
--     EXISTS` is forward-compatible with the future V059/Phase-N workspace-wide
--     drop (which no-ops on this table). Dropping `queue_code` automatically
--     drops its `CHECK (queue_code IS NULL OR queue_code IN (...))` constraint
--     (it references only that column), so no explicit constraint drop is
--     needed (unlike the class_code CHECK widening in V072).
--
-- (2) SCOPE NOTE (F6=A): this slice is DB-structure standardization ONLY. The
--     Action step-machine — the `steps` JSONB 13-step-type model
--     (tool_call/conditional/set_var/loop/return/evaluate/call_skill/try_catch/
--     parallel/call_action/spawn_subprocess/wait/emit_event) + the
--     `ActionShortCircuit` intent path (retrieval_source.rs:727, documented
--     "vestigial under Q2") — was driven by the retired `default.py` +
--     `__execute_action__` meta-primitive. Its retire-vs-reformulate fate is
--     entangled with C.4.5.17 (the composition system, not yet built) and
--     C.5/C.6/C.7 (the retirement phase, not yet done). Per F6=A, the
--     step-machine / Q1-gate / syntax alignment is DEFERRED to those slices;
--     this migration only drops the dead legacy columns (the one change that
--     persists regardless of the step-machine's fate).

-- drop the 5 legacy pre-centralization lifecycle columns.
ALTER TABLE reborn_actions DROP COLUMN IF EXISTS validation_errors;
ALTER TABLE reborn_actions DROP COLUMN IF EXISTS review_feedback;
ALTER TABLE reborn_actions DROP COLUMN IF EXISTS review_attempts;
ALTER TABLE reborn_actions DROP COLUMN IF EXISTS rejected_at;
ALTER TABLE reborn_actions DROP COLUMN IF EXISTS queue_code;
