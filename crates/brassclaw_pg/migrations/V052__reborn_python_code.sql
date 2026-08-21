-- V052__reborn_python_code.sql
-- PythonCode component table for BrassClaw Reborn (Phase B, class 22).
--
-- Executable Python bodies for Tier-0 recipe orchestration.
-- Source: 'authored' (user) or 'system' (seeded by builtin_bootstrap.rs).
-- consumer_tags default: {02:orchestrator, 05:validator} until validated.
--
-- DESIGN NOTE (§0.18): Queue-tracking columns (queue_code, review_attempts,
-- review_feedback, rejected_at, validation_errors) are NOT on this table —
-- they are centralised on reborn_validation_queue (V051). This table carries
-- validation_status only (the post-validation gate that STAYS on the component).
-- dependency_registry is included here at creation (V055 retroactively adds it
-- to the 13 older tables; new tables include it from day one — Phase J.2).

CREATE SEQUENCE IF NOT EXISTS reborn_python_code_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_python_code (
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

    -- Solution-override columns (§3.13/§3.14 — SCH-02).
    -- Already present at creation (no retrofit migration needed).
    prior_knowledge_content TEXT,
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

    -- class_code = 22 (PythonCode)
    class_code              SMALLINT    NOT NULL DEFAULT 22
        CHECK (class_code = 22),
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_python_code_prompt_uid_seq'),

    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',

    intent_examples         JSONB,

    -- Post-validation gate (STAYS on component table — see §0.18 / FIND-AUDIT-15).
    -- Queue-tracking columns (queue_code, review_attempts, review_feedback,
    -- rejected_at, validation_errors) are NOT here — centralised on
    -- reborn_validation_queue (V051).
    validation_status       TEXT        NOT NULL DEFAULT 'pending'
        CHECK (validation_status IN (
            'pending', 'auto_passed', 'auto_failed', 'validated',
            'review_requested', 'rejected', 'garbage', 'upgrade_queued'
        )),

    -- See FIND-P6-02 / FIND-AUDIT-15: 'system' must be allowed from day one
    -- (Phase L seeds rows with source = 'system'; V057 only alters older tables).
    source                  TEXT        NOT NULL DEFAULT 'authored'
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system')),
    content_hash            TEXT,
    similarity_parent_id    UUID,
    replaces_id             UUID,
    parent_version          TEXT,
    last_audit_at           TIMESTAMPTZ,
    audit_failure_count     SMALLINT    NOT NULL DEFAULT 0,
    parent_mission_id       UUID,

    -- Dependency registry (§0.19 / Phase J.2). New tables include it at creation.
    dependency_registry     JSONB,

    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_python_code_pk PRIMARY KEY (id),
    CONSTRAINT reborn_python_code_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- Required indexes (PERF-03 / FIND-AUDIT-15):
-- Without these, the UNION ALL sub-select in fetch_for_consumer degrades to seq-scan.
CREATE INDEX IF NOT EXISTS reborn_python_code_scope_idx
    ON reborn_python_code (tenant_id, user_id, agent_id, project_id);
CREATE INDEX IF NOT EXISTS reborn_python_code_scope_status_idx
    ON reborn_python_code (tenant_id, user_id, agent_id, project_id, validation_status);
CREATE INDEX IF NOT EXISTS reborn_python_code_scope_uid_idx
    ON reborn_python_code (tenant_id, user_id, agent_id, project_id, prompt_uid);
CREATE INDEX IF NOT EXISTS reborn_python_code_consumer_tags_gin_idx
    ON reborn_python_code USING GIN (consumer_tags);
CREATE INDEX IF NOT EXISTS reborn_python_code_similarity_parent_idx
    ON reborn_python_code (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS reborn_python_code_replaces_idx
    ON reborn_python_code (replaces_id)
    WHERE replaces_id IS NOT NULL;

CREATE TRIGGER reborn_python_code_updated_at
    BEFORE UPDATE ON reborn_python_code
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
