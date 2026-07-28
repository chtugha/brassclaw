-- V029__reborn_actions.sql
-- Actions class (class_code = 16) for BrassClaw Reborn.
--
-- Actions are LLM-free deterministic execution sequences.  When the intent
-- system routes a query to class_code 16, default.py performs the Action
-- directly from its step descriptors — no __llm_complete__ call.
--
-- Spec references: §3.11, §4, §7 Q13, SEC-07, SEC-08, SEC-09, PERF-18.
--
-- Hard limits (PERF-18 — compiled-in, not configurable via DB):
--   max content size  = 256 KB (enforced in Rust validation)
--   max step count    = 500     (enforced in Rust validation)
--   max allowed_tools = 50      (enforced in Rust validation)
--
-- Recursion bounds (SEC-09):
--   call_action max depth  = 5
--   total step budget      = 1000 across nesting levels
--
-- 13 step types (§7 Q13):
--   tool_call, conditional, set_var, loop, return, evaluate,
--   call_skill, try_catch, parallel, call_action,
--   spawn_subprocess, wait, emit_event

CREATE SEQUENCE IF NOT EXISTS reborn_actions_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_actions (
    -- Primary key
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — all writes and reads must filter on the full tuple.
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    -- Core content (validation-gated)
    name                    TEXT        NOT NULL
        CHECK (name ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' AND length(name) BETWEEN 1 AND 64),
    description             TEXT        NOT NULL
        CHECK (length(description) BETWEEN 1 AND 1024),

    -- Execution spec (validation-gated)
    -- Ordered array of step descriptors; 13 step types (§7 Q13).
    steps                   JSONB       NOT NULL DEFAULT '[]',
    -- Optional preconditions evaluated before step execution.
    preconditions           JSONB,
    -- Top-level error handling policy.
    error_handling          JSONB,
    -- Wall-clock timeout in seconds.
    timeout_secs            INT         NOT NULL DEFAULT 60
        CHECK (timeout_secs > 0 AND timeout_secs <= 3600),

    -- Allowed tools for this Action's tool_call steps.
    -- Enforced at both default.py AND EffectExecutor (SEC-07 defence-in-depth).
    allowed_tools           TEXT[]      NOT NULL DEFAULT '{}',

    -- Param schema / template for structured invocation.
    param_schema            JSONB,
    param_template          JSONB,

    -- Prior-knowledge content (§3.13/§3.14 — Solution Override path).
    -- When non-NULL, used instead of concatenating `steps` + `description`.
    prior_knowledge_content TEXT,
    -- Actions default to Solution Override: override_prompt_creation = true.
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT true,

    -- Classification
    -- class_code for Actions is always 16.
    class_code              SMALLINT    NOT NULL DEFAULT 16
        CHECK (class_code = 16),
    -- prompt_uid: monotonic sequence for deterministic prompt assembly order.
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_actions_prompt_uid_seq'),

    -- Consumer tags (validation-gated §3.9).
    -- Default: {01:monty, 02:orchestrator} + 05:validator until validated.
    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',
    -- Note: consumer_tags format check removed — subqueries in CHECK constraints
    -- are not supported by PostgreSQL 16. Validation is enforced at the app layer.

    -- Intent examples (validation-gated; primary activation mechanism §3.12).
    intent_examples         JSONB       NOT NULL DEFAULT '[]',

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

    CONSTRAINT reborn_actions_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- Index for consumer-tag gated retrieval.
CREATE INDEX IF NOT EXISTS reborn_actions_consumer_tags_gin_idx
    ON reborn_actions USING GIN (consumer_tags);

-- Index for validation queue queries.
CREATE INDEX IF NOT EXISTS reborn_actions_scope_status_idx
    ON reborn_actions (tenant_id, user_id, agent_id, project_id, validation_status);

-- Index for deterministic prompt assembly order.
CREATE INDEX IF NOT EXISTS reborn_actions_scope_class_uid_idx
    ON reborn_actions (tenant_id, user_id, agent_id, project_id, class_code ASC, prompt_uid ASC);
