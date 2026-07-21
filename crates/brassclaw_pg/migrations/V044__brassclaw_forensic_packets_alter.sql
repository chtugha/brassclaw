-- V044__brassclaw_forensic_packets_alter.sql
--
-- ALTER the existing brassclaw_forensic_packets table (created by V026).
--
-- The interceptor's new storage model avoids double-saving prompt content that
-- is already stored as component rows:
--
--   component_refs JSONB
--     Array of {class_code, prompt_uid, component_id, schema_version} objects
--     drawn from PriorKnowledgeResult.matched_component_ids.  Replaces the role
--     of the prompt JSONB column for the interceptor's forensic record.
--     New interceptor code writes here; old code and old packets still use
--     prompt JSONB — both columns coexist for backward compatibility.
--
--   volatile_tail TEXT
--     Thread history and per-turn inline nudges — the volatile portion of the
--     assembled prompt that was NOT covered by the stable component base.
--
-- The existing prompt JSONB column is kept for backward compatibility: old rows
-- already have it populated.  New interceptor code writes to component_refs +
-- volatile_tail instead.  Old packets remain readable via prompt JSONB.
--
-- No index on component_refs is created here; the interceptor reads packets by
-- id (primary key) or by (tenant_id, captured_at DESC) (existing index).
-- A GIN index can be added in a later migration if component-manifest queries
-- become a hot path.

ALTER TABLE brassclaw_forensic_packets
    ADD COLUMN IF NOT EXISTS component_refs JSONB,
    ADD COLUMN IF NOT EXISTS volatile_tail  TEXT;
