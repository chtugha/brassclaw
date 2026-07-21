-- V030__reborn_tools.sql
-- DB-stored Tools table for BrassClaw Reborn.
--
-- Stores Rusty tool definitions (class_code = 00).  Monty/LLM consumers are
-- instructed via Skills (V027); tools carry Rusty-only metadata.
--
-- Spec references: Phase 2 Step 2.1, §3.9, §4 (scope tuple, consumer_tags,
-- validation lifecycle), §7 Q9.
--
-- All writes and reads must filter on the full scope tuple
-- (tenant_id, user_id, agent_id, project_id).
--
-- Validation lifecycle: pending → auto_passed/auto_failed → validated
--                        (or rejected → garbage)
--
-- consumer_tags[] default: {00:rusty} + 05:validator until validated.
-- '05:validator' greys out all other tags (§3.5.1); the tag is removed by
-- the Q2 manual-validation step (AutoPassed → Validated).
--
-- Tools do not carry prompt text for Monty/LLM — Rusty-only.

CREATE SEQUENCE IF NOT EXISTS reborn_tools_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_tools (
    -- Primary key
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — all writes and reads must filter on the full tuple.
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    -- Core content (validation-gated)
    name                    TEXT        NOT NULL
        CHECK (name ~ '^[a-z0-9]([a-z0-9._-]*[a-z0-9])?$' AND length(name) BETWEEN 1 AND 128),
    description             TEXT        NOT NULL
        CHECK (length(description) BETWEEN 1 AND 1024),

    -- Structured invocation spec (validation-gated)
    -- JSON Schema describing the tool's input parameters.
    param_schema            JSONB,
    -- Concrete parameter template for structured invocation.
    param_template          JSONB,

    -- Execution semantics (validation-gated)
    -- One of: read, write, exec, network, mixed — describes side-effect class.
    effect_type             TEXT        NOT NULL DEFAULT 'read'
        CHECK (effect_type IN ('read', 'write', 'exec', 'network', 'mixed')),
    -- Optional precondition expression evaluated before invocation.
    preconditions           TEXT,
    -- Error handling policy description (Rusty-level).
    error_handling          TEXT,

    -- Classification
    -- class_code for Tools is always 00 (Rusty).
    class_code              SMALLINT    NOT NULL DEFAULT 0
        CHECK (class_code = 0),
    -- prompt_uid: monotonic sequence for deterministic prompt assembly order.
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_tools_prompt_uid_seq'),

    -- Consumer tags (validation-gated §3.9).
    -- Each entry: '^[0-9]{2}(:[a-z0-9-]+)?$'
    -- Tools always carry {00:rusty} + 05:validator until validated.
    -- '05:validator' greys out all other tags until Step-2 manual validation.
    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}'
        CHECK (
            -- Every entry must match the tag code pattern.
            array_length(array(
                SELECT t FROM unnest(consumer_tags) t
                WHERE t !~ '^[0-9]{2}(:[a-z0-9-]+)?$'
            ), 1) IS NULL
        ),

    -- Provenance (immediate-write)
    source                  TEXT        NOT NULL DEFAULT 'authored'
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported')),

    -- Decision / validation-lifecycle columns (validation-gated)
    validation_status       TEXT        NOT NULL DEFAULT 'pending'
        CHECK (validation_status IN (
            'pending', 'upgrade_queued', 'auto_failed', 'auto_passed',
            'validated', 'review_requested', 'rejected', 'garbage'
        )),
    validation_errors       TEXT[]      NOT NULL DEFAULT '{}',
    review_feedback         TEXT,
    review_attempts         INT         NOT NULL DEFAULT 0,
    rejected_at             TIMESTAMPTZ,
    queue_code              TEXT
        CHECK (queue_code IS NULL OR queue_code IN (
            'q1_auto', 'q2_manual', 'q3_revision', 'q4_rejection'
        )),

    -- Lineage columns
    similarity_parent_id    UUID,
    replaces_id             UUID,
    parent_version          TEXT,
    content_hash            TEXT        NOT NULL DEFAULT '',
    last_audit_at           TIMESTAMPTZ,
    audit_failure_count     INT         NOT NULL DEFAULT 0,
    parent_mission_id       UUID,

    -- Timestamps
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (id),

    CONSTRAINT reborn_tools_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- GIN index on consumer_tags[] for tag-gated retrieval (§3.9).
CREATE INDEX IF NOT EXISTS reborn_tools_consumer_tags_gin_idx
    ON reborn_tools USING GIN (consumer_tags);

-- Index for validation queue queries.
CREATE INDEX IF NOT EXISTS reborn_tools_scope_status_idx
    ON reborn_tools (tenant_id, user_id, agent_id, project_id, validation_status);

-- Index for deterministic prompt assembly order.
CREATE INDEX IF NOT EXISTS reborn_tools_scope_class_uid_idx
    ON reborn_tools (tenant_id, user_id, agent_id, project_id, class_code ASC, prompt_uid ASC);

-- Partial index for fast validated-tool reads by the capability surface
-- (fetch_for_consumer: validation_status = 'validated' AND
--  NOT ('05:validator' = ANY(consumer_tags))).
CREATE INDEX IF NOT EXISTS reborn_tools_validated_idx
    ON reborn_tools (tenant_id, user_id, agent_id, project_id, name)
    WHERE validation_status = 'validated';

-- Auto-update updated_at on every row change.
CREATE TRIGGER reborn_tools_updated_at
    BEFORE UPDATE ON reborn_tools
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
