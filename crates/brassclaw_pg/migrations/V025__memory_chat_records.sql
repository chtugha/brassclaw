-- V025__memory_chat_records.sql
-- Path A: human-readable, structured chat-memory records.
-- Written unconditionally on every memory_write call.
-- source_ref (revision 17): VFS path of chunk subtree; NULL until Path B runs.
-- forensic_packet_id: soft reference to brassclaw_forensic_packets.
-- tsv: generated stored column — auto-maintained on every INSERT/UPDATE.
CREATE TABLE IF NOT EXISTS brassclaw_memory_chat_records (
    id               TEXT        NOT NULL PRIMARY KEY,
    tenant_id        TEXT        NOT NULL,
    user_id          TEXT        NOT NULL,
    project_id       TEXT,
    agent_id         TEXT,
    session_thread_id TEXT,
    run_id           TEXT,
    kind             TEXT        NOT NULL DEFAULT 'observation',
    content          TEXT        NOT NULL,
    summary          TEXT,
    context          JSONB       NOT NULL DEFAULT '{}',
    importance       NUMERIC(5,4)
        CHECK (importance IS NULL OR (importance >= 0.0 AND importance <= 1.0)),
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    source_ref       TEXT,
    forensic_packet_id TEXT,
    tsv              tsvector    GENERATED ALWAYS AS (
                         to_tsvector('english',
                             coalesce(content, '') || ' ' || coalesce(summary, ''))
                     ) STORED,
    success_score    NUMERIC(5,4)
        CHECK (success_score IS NULL OR (success_score >= 0.0 AND success_score <= 1.0)),
    reinforcement    NUMERIC(5,4)
        CHECK (reinforcement IS NULL OR (reinforcement >= 0.0 AND reinforcement <= 1.0)),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_tenant_user_idx
    ON brassclaw_memory_chat_records (tenant_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_thread_idx
    ON brassclaw_memory_chat_records (tenant_id, session_thread_id)
    WHERE session_thread_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_project_idx
    ON brassclaw_memory_chat_records (tenant_id, project_id, created_at DESC)
    WHERE project_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_source_ref_idx
    ON brassclaw_memory_chat_records (tenant_id, source_ref)
    WHERE source_ref IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_packet_idx
    ON brassclaw_memory_chat_records (forensic_packet_id)
    WHERE forensic_packet_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_run_idx
    ON brassclaw_memory_chat_records (tenant_id, run_id, created_at DESC)
    WHERE run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_fts_idx
    ON brassclaw_memory_chat_records USING GIN (tsv);
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_tags_idx
    ON brassclaw_memory_chat_records USING GIN (tags);
CREATE TRIGGER brassclaw_memory_chat_records_updated_at
    BEFORE UPDATE ON brassclaw_memory_chat_records
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
