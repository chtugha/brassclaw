-- V027__reborn_skills.sql
-- DB-stored Skills table for BrassClaw Reborn.
--
-- Replaces on-disk SKILL.md files with a fully relational store.
-- Schema follows the agentskills.io specification (v5.5):
--   • No `trust` column — validation_status == 'validated' is the sole trust gate.
--   • Explicit class_code: 01 (Rusty), 02 (Monty), 03 (LLM).
--   • prompt_uid: monotonic sequence at insert; never reused/reordered.
--   • consumer_tags[]: component tag system (§3.9); each entry matches
--     '^[0-9]{2}(:[a-z0-9-]+)?$'. Tag '05:validator' greys out all others.
--   • intent_examples jsonb: array of {input, class} entries for the intent
--     system (§3.12); replaces keywords[]/patterns[]/tags[] as primary routing.
--   • Full (tenant_id, user_id, agent_id, project_id) scope isolation.
--   • Unique on (scope, name).
--
-- Validation lifecycle (§3.5.1):
--   pending → auto_passed/auto_failed → validated (or rejected → garbage)
--
-- Reward columns are immediate-write; all content/activation/tag columns are
-- validation-gated (save → Q1 Step-1 validation → commit on pass; Q2 manual).

CREATE SEQUENCE IF NOT EXISTS reborn_skills_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_skills (
    -- Primary key
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — all writes and reads must filter on the full tuple.
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    -- Content columns (validation-gated)
    name                    TEXT        NOT NULL
        CHECK (name ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' AND length(name) BETWEEN 1 AND 64),
    description             TEXT        NOT NULL
        CHECK (length(description) BETWEEN 1 AND 1024),
    body                    TEXT        NOT NULL DEFAULT '',
    compatibility           TEXT        NOT NULL DEFAULT '',
    license                 TEXT        NOT NULL DEFAULT '',
    allowed_tools           TEXT[]      NOT NULL DEFAULT '{}',
    version                 TEXT        NOT NULL DEFAULT '0.0.0',

    -- Classification (validation-gated)
    -- class_code: 01 = Rusty, 02 = Monty, 03 = LLM
    class_code              SMALLINT    NOT NULL DEFAULT 3
        CHECK (class_code IN (1, 2, 3)),
    -- prompt_uid: assigned from sequence at insert; stable ordering for
    -- deterministic prompt assembly: ORDER BY (class_code ASC, prompt_uid ASC).
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_skills_prompt_uid_seq'),

    -- Activation columns (validation-gated; legacy — intent_examples is primary)
    keywords                TEXT[]      NOT NULL DEFAULT '{}',
    exclude_keywords        TEXT[]      NOT NULL DEFAULT '{}',
    patterns                TEXT[]      NOT NULL DEFAULT '{}',
    tags                    TEXT[]      NOT NULL DEFAULT '{}',
    max_context_tokens      INT         NOT NULL DEFAULT 2500
        CHECK (max_context_tokens > 0),
    setup_marker            TEXT,
    required_binaries       TEXT[]      NOT NULL DEFAULT '{}',
    required_env            TEXT[]      NOT NULL DEFAULT '{}',
    required_config         TEXT[]      NOT NULL DEFAULT '{}',

    -- Intent examples (validation-gated; primary activation mechanism §3.12).
    -- Array of {input: text, class: 1|2|3} objects.
    intent_examples         JSONB       NOT NULL DEFAULT '[]',

    -- Consumer tags (validation-gated §3.9).
    -- Each entry: '^[0-9]{2}(:[a-z0-9-]+)?$'
    -- Rusty defaults:  {00:rusty,01:monty}
    -- Monty defaults:  {01:monty,02:orchestrator}
    -- LLM defaults:    {02:orchestrator,03:llm}
    -- All new/updated rows also receive 05:validator until Step-2 validation.
    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',
    -- Note: a per-entry format check on consumer_tags ('^[0-9]{2}(:[a-z0-9-]+)?$')
    -- cannot use a subquery in a CHECK constraint on PostgreSQL 16.
    -- Format validation is enforced at the application layer instead.

    -- Reward columns (immediate-write — no validation gate)
    tier                    TEXT        NOT NULL DEFAULT 'seedling'
        CHECK (tier IN ('seedling', 'growing', 'mature', 'candidate')),
    usage_count             BIGINT      NOT NULL DEFAULT 0,
    success_count           BIGINT      NOT NULL DEFAULT 0,
    failure_count           BIGINT      NOT NULL DEFAULT 0,
    wilson_lower            DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    confidence              DOUBLE PRECISION NOT NULL DEFAULT 1.0,

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

    -- Lineage columns (immediate-write for provenance; content-gated for parent refs)
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

    CONSTRAINT reborn_skills_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- Scope + validation_status index for validation queue queries.
CREATE INDEX IF NOT EXISTS reborn_skills_scope_status_idx
    ON reborn_skills (tenant_id, user_id, agent_id, project_id, validation_status);

-- Scope + (class_code, prompt_uid) for ordered prompt assembly.
CREATE INDEX IF NOT EXISTS reborn_skills_scope_class_uid_idx
    ON reborn_skills (tenant_id, user_id, agent_id, project_id, class_code ASC, prompt_uid ASC);

-- GIN index on consumer_tags[] for tag-gated retrieval (§3.9).
CREATE INDEX IF NOT EXISTS reborn_skills_consumer_tags_idx
    ON reborn_skills USING GIN (consumer_tags);

-- GIN index on intent_examples jsonb for intent system queries.
CREATE INDEX IF NOT EXISTS reborn_skills_intent_examples_idx
    ON reborn_skills USING GIN (intent_examples);

-- Scope isolation: a partial index on validated skills for fast consumer reads.
-- fetch_for_consumer filters: validation_status = 'validated'
--   AND '05:validator' != ANY(consumer_tags)
CREATE INDEX IF NOT EXISTS reborn_skills_validated_idx
    ON reborn_skills (tenant_id, user_id, agent_id, project_id, class_code, prompt_uid)
    WHERE validation_status = 'validated';

-- Auto-update updated_at on every row change.
CREATE TRIGGER reborn_skills_updated_at
    BEFORE UPDATE ON reborn_skills
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
