-- V037__reborn_tool_skills.sql
-- ToolSkills component table for BrassClaw Reborn (Phase 5 Step 5.3, class 13).
--
-- Former DocType::ToolSkill documents migrated to a first-class component table.
-- ToolSkills describe ONE tool usage pattern (tight, token-cheap).
-- consumer_tags default: {00:rusty, 01:monty, 02:orchestrator} + 05:validator.
--
-- Spec references: §3.3, §3.7 (class 13), §3.9, §3.12, §3.13/§3.14 (SCH-02),
-- Phase 4 Step 4.1, Phase 5 Step 5.3.

CREATE SEQUENCE IF NOT EXISTS reborn_tool_skills_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_tool_skills (
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    name                    TEXT        NOT NULL
        CHECK (name ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' AND length(name) BETWEEN 1 AND 64),
    description             TEXT        NOT NULL
        CHECK (length(description) BETWEEN 1 AND 1024),
    content                 TEXT        NOT NULL DEFAULT '',

    -- Prior-knowledge content (§3.13/§3.14 — SCH-02 fix).
    -- When non-NULL, used as the component's prior-knowledge text instead of
    -- assembling from `content`.
    prior_knowledge_content TEXT,
    -- If true, the Solution Override path is taken: this component's
    -- prior_knowledge_content replaces the standard assembly.  Default false.
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

    -- Tool name this ToolSkill describes.
    tool_name               TEXT,
    -- Parameter schema for tool invocation validation.
    param_schema            JSONB,
    -- Parameter template with defaults.
    param_template          JSONB,

    -- class_code = 13 (ToolSkill)
    class_code              SMALLINT    NOT NULL DEFAULT 13
        CHECK (class_code = 13),
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_tool_skills_prompt_uid_seq'),

    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',
    -- Note: consumer_tags format check removed — subqueries in CHECK constraints
    -- are not supported by PostgreSQL 16. Validation is enforced at the app layer.

    intent_examples         JSONB,

    -- Reward / scoring (immediate-write)
    tier                    TEXT        NOT NULL DEFAULT 'seedling'
        CHECK (tier IN ('seedling', 'growing', 'mature', 'candidate')),
    usage_count             INT         NOT NULL DEFAULT 0,
    success_count           INT         NOT NULL DEFAULT 0,
    failure_count           INT         NOT NULL DEFAULT 0,
    wilson_lower            FLOAT8      NOT NULL DEFAULT 0.0,

    validation_status       TEXT        NOT NULL DEFAULT 'pending'
        CHECK (validation_status IN (
            'pending', 'auto_passed', 'auto_failed', 'validated',
            'review_requested', 'rejected', 'garbage', 'upgrade_queued'
        )),
    validation_errors       TEXT[]      NOT NULL DEFAULT '{}',
    review_feedback         TEXT,
    review_attempts         SMALLINT    NOT NULL DEFAULT 0,
    rejected_at             TIMESTAMPTZ,
    queue_code              TEXT,

    source                  TEXT        NOT NULL DEFAULT 'migrated',
    content_hash            TEXT,
    similarity_parent_id    UUID,
    replaces_id             UUID,
    parent_version          TEXT,
    last_audit_at           TIMESTAMPTZ,
    audit_failure_count     SMALLINT    NOT NULL DEFAULT 0,
    parent_mission_id       UUID,

    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_tool_skills_pk PRIMARY KEY (id),
    CONSTRAINT reborn_tool_skills_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

CREATE INDEX IF NOT EXISTS reborn_tool_skills_scope_idx
    ON reborn_tool_skills (tenant_id, user_id, agent_id, project_id);
CREATE INDEX IF NOT EXISTS reborn_tool_skills_scope_status_idx
    ON reborn_tool_skills (tenant_id, user_id, agent_id, project_id, validation_status);
CREATE INDEX IF NOT EXISTS reborn_tool_skills_scope_uid_idx
    ON reborn_tool_skills (tenant_id, user_id, agent_id, project_id, prompt_uid);
CREATE INDEX IF NOT EXISTS reborn_tool_skills_consumer_tags_gin_idx
    ON reborn_tool_skills USING GIN (consumer_tags);
CREATE INDEX IF NOT EXISTS reborn_tool_skills_similarity_parent_idx
    ON reborn_tool_skills (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS reborn_tool_skills_replaces_idx
    ON reborn_tool_skills (replaces_id)
    WHERE replaces_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS reborn_tool_skills_scope_validated_idx
    ON reborn_tool_skills (tenant_id, user_id, agent_id, project_id, tier)
    WHERE validation_status = 'validated';

CREATE TRIGGER reborn_tool_skills_updated_at
    BEFORE UPDATE ON reborn_tool_skills
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
