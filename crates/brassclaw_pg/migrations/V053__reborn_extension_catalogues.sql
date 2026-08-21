-- V053__reborn_extension_catalogues.sql
-- ExtensionCatalogue component table for BrassClaw Reborn (Phase C, class 23).
--
-- Documentation-container that organises a capability domain.
-- Primary text field: overview_doc (maps to effective_content in UNION ALL).
-- Source: 'authored' (user) or 'system' (seeded by builtin_bootstrap.rs).
-- consumer_tags default: {02:orchestrator, 05:validator} until validated.
--
-- DESIGN NOTE (§0.18): Queue-tracking columns are NOT on this table.
-- dependency_registry is included here at creation (V055 retroactively adds it
-- to the 13 older tables; new tables include it from day one — Phase J.2).

CREATE SEQUENCE IF NOT EXISTS reborn_extension_catalogues_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_extension_catalogues (
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    name                    TEXT        NOT NULL
        CHECK (length(name) BETWEEN 1 AND 256),
    description             TEXT        NOT NULL DEFAULT ''
        CHECK (length(description) <= 1024),
    version                 TEXT        NOT NULL DEFAULT '1.0',

    -- Primary text content (maps to effective_content in UNION ALL via COALESCE):
    --   COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)
    overview_doc            TEXT        NOT NULL DEFAULT '',

    -- Structured extras (Phase C — accessed in validator via GenericComponent.extra).
    task_groups             JSONB       NOT NULL DEFAULT '[]',
    child_component_ids     UUID[]      NOT NULL DEFAULT '{}',
    intent_index            JSONB,                           -- audit-only, NOT indexed

    -- Solution-override columns (§3.13/§3.14 — SCH-02).
    -- Already present at creation (no retrofit migration needed).
    prior_knowledge_content TEXT,
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

    -- class_code = 23 (ExtensionCatalogue)
    class_code              SMALLINT    NOT NULL DEFAULT 23
        CHECK (class_code = 23),
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_extension_catalogues_prompt_uid_seq'),

    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',

    intent_examples         JSONB,

    -- Post-validation gate only (see §0.18 / FIND-AUDIT-16).
    validation_status       TEXT        NOT NULL DEFAULT 'pending'
        CHECK (validation_status IN (
            'pending', 'auto_passed', 'auto_failed', 'validated',
            'review_requested', 'rejected', 'garbage', 'upgrade_queued'
        )),

    -- See FIND-P6-02 / FIND-AUDIT-16: 'system' must be allowed from day one.
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

    CONSTRAINT reborn_extension_catalogues_pk PRIMARY KEY (id),
    CONSTRAINT reborn_extension_catalogues_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- Required indexes (PERF-03 / FIND-AUDIT-16):
-- Without these, the UNION ALL sub-select in fetch_for_consumer degrades to seq-scan.
CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_scope_idx
    ON reborn_extension_catalogues (tenant_id, user_id, agent_id, project_id);
CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_scope_status_idx
    ON reborn_extension_catalogues (tenant_id, user_id, agent_id, project_id, validation_status);
CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_scope_uid_idx
    ON reborn_extension_catalogues (tenant_id, user_id, agent_id, project_id, prompt_uid);
CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_consumer_tags_gin_idx
    ON reborn_extension_catalogues USING GIN (consumer_tags);
CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_similarity_parent_idx
    ON reborn_extension_catalogues (similarity_parent_id)
    WHERE similarity_parent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_replaces_idx
    ON reborn_extension_catalogues (replaces_id)
    WHERE replaces_id IS NOT NULL;

CREATE TRIGGER reborn_extension_catalogues_updated_at
    BEFORE UPDATE ON reborn_extension_catalogues
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
