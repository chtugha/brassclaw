-- Phase P.0 — Add q2_actor audit column to reborn_validation_queue.
--
-- NULL   = awaiting Q2 (component has not been approved yet).
-- 'human'   = Q2 approved by a human operator via the WebUI.
-- 'builtin' = Q2 graduation recorded by the bootstrap seeder
--             (builtins are exempt from the human-Q2 rule;
--              this is an audit label only, NOT a policy bypass).
--
-- Q2 is manual and human-only for all non-builtin components.
-- The 'builtin' actor value is exclusively for builtin_bootstrap.rs.
-- No other automated caller may ever pass 'builtin'.
--
-- V061 is taken (reborn_components_registry.sql).
-- V077 = Phase N populate migration.  V078 is the next free number.
ALTER TABLE reborn_validation_queue
    ADD COLUMN IF NOT EXISTS q2_actor TEXT;
