-- V038__reborn_plans.sql
-- Plans component table for BrassClaw Reborn (Phase 5 Step 5.3, class 14).
--
-- Former DocType::Plan documents migrated to a first-class component table.
-- Plans are structured execution blueprints with steps and progress tracking.
-- consumer_tags default: {01:monty, 02:orchestrator} + 05:validator until validated.
--
-- Spec references: §3.3, §3.7 (class 14), §3.9, §3.12, §3.13/§3.14 (SCH-02),
-- Phase 5 Step 5.3.

CREATE SEQUENCE IF NOT EXISTS reborn_plans_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_plans (
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    name                    TEXT        NOT NULL
        CHECK (length(name) BETWEEN 1 AND 256),
    description             TEXT        NOT NULL DEFAULT ''
        CHECK (length(description) <= 1024),
    content                 TEXT        NOT NULL DEFAULT '',

    -- Ordered step list (JSONB array of step descriptors).
    steps                   JSONB       NOT NULL DEFAULT '[]',

    -- Prior-knowledge content (§3.13/§3.14 — SCH-02 fix).
    -- When non-NULL, used as the component's prior-knowledge text instead of
    -- assembling from `content`.
    prior_knowledge_content TEXT,
    -- If true, the Solution Override path is taken: this component's
    -- prior_knowledge_content replaces the standard assembly.  Default false.
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

    -- class_code = 14 (Plan)
    class_code              SMALLINT    NOT NULL DEFAULT 14
        CHECK (class_code = 14),
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_plans_prompt_uid_seq'),

    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}'
        CHECK (
            array_length(array(
                SELECT t FROM unnest(consumer_tags) t
                WHERE t !~ '^[0-9]{2}(:[a-z0-9-]+)?$'
            ), 1) IS NULL
        ),

    intent_examples         JSONB,

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

    CONSTRAINT reborn_plans_pk PRIMARY KEY (id),
    CONSTRAINT reborn_plans_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

CREATE INDEX IF NOT EXISTS reborn_plans_scope_idx
    ON reborn_plans (tenant_id, user_id, agent_id, project_id);
CREATE INDEX IF NOT EXISTS reborn_plans_scope_status_idx
    ON reborn_plans (tenant_id, user_id, agent_id, project_id, validation_status);
CREATE INDEX IF NOT EXISTS reborn_plans_scope_uid_idx
    ON reborn_plans (tenant_id, user_id, agent_id, project_id, prompt_uid);
CREATE INDEX IF NOT EXISTS reborn_plans_consumer_tags_gin_idx
    ON reborn_plans USING GIN (consumer_tags);
CREATE INDEX IF NOT EXISTS reborn_plans_similarity_parent_idx
    ON reborn_plans (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS reborn_plans_replaces_idx
    ON reborn_plans (replaces_id)
    WHERE replaces_id IS NOT NULL;

CREATE TRIGGER reborn_plans_updated_at
    BEFORE UPDATE ON reborn_plans
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
