-- V043__reborn_notes.sql
-- Notes component table for BrassClaw Reborn (Phase 5 Step 5.3, class 20).
--
-- Former DocType::Note documents (volatile scratch notes) migrated to a
-- first-class component table.  Also serves as the destination for chat records
-- from brassclaw_memory_chat_records (Phase 5 Step 5.4 — flat Note rows,
-- no embedding index, retrieved by intent system or project-scope lookup).
-- consumer_tags default: {02:orchestrator} + 05:validator until validated.
--
-- Note: Issues/Notes/Summaries are excluded from the DB-less fallback file
-- (volatile / low-value for fallback — spec §3.4, compiled-in priority list).
--
-- Spec references: §3.3, §3.7 (class 20), §3.9, §3.12, §7 Q4, Phase 5 Step 5.3/5.4.

CREATE SEQUENCE IF NOT EXISTS reborn_notes_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_notes (
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

    -- For chat-record notes: source_thread_id links back to the originating thread.
    source_thread_id        UUID,

    -- class_code = 20 (Note)
    class_code              SMALLINT    NOT NULL DEFAULT 20
        CHECK (class_code = 20),
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_notes_prompt_uid_seq'),

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

    CONSTRAINT reborn_notes_pk PRIMARY KEY (id),
    CONSTRAINT reborn_notes_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

CREATE INDEX IF NOT EXISTS reborn_notes_scope_idx
    ON reborn_notes (tenant_id, user_id, agent_id, project_id);
CREATE INDEX IF NOT EXISTS reborn_notes_scope_status_idx
    ON reborn_notes (tenant_id, user_id, agent_id, project_id, validation_status);
CREATE INDEX IF NOT EXISTS reborn_notes_scope_uid_idx
    ON reborn_notes (tenant_id, user_id, agent_id, project_id, prompt_uid);
CREATE INDEX IF NOT EXISTS reborn_notes_consumer_tags_gin_idx
    ON reborn_notes USING GIN (consumer_tags);
-- Thread-linked notes (chat records) — fast lookup by source thread.
CREATE INDEX IF NOT EXISTS reborn_notes_thread_idx
    ON reborn_notes (source_thread_id)
    WHERE source_thread_id IS NOT NULL;

CREATE TRIGGER reborn_notes_updated_at
    BEFORE UPDATE ON reborn_notes
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
