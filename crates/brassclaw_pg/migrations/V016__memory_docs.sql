-- V016__memory_docs.sql
-- Stores MemoryDoc records from the memory reduction-rule pipeline.
-- tsv is a generated stored column — auto-maintained on every INSERT/UPDATE.
CREATE TABLE IF NOT EXISTS brassclaw_memory_docs (
    id               TEXT        NOT NULL,
    tenant_id        TEXT        NOT NULL,
    user_id          TEXT        NOT NULL,
    project_id       TEXT        NOT NULL,
    doc_type         TEXT        NOT NULL,
    title            TEXT        NOT NULL,
    content          TEXT        NOT NULL,
    source_thread_id TEXT,
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    metadata         JSONB       NOT NULL DEFAULT '{}',
    tsv              tsvector    GENERATED ALWAYS AS (
                         to_tsvector('english',
                             coalesce(title, '') || ' ' || coalesce(content, ''))
                     ) STORED,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, project_id, id)
);
CREATE INDEX IF NOT EXISTS brassclaw_memory_docs_fts_idx
    ON brassclaw_memory_docs USING GIN (tsv);
CREATE INDEX IF NOT EXISTS brassclaw_memory_docs_tags_idx
    ON brassclaw_memory_docs USING GIN (tags);
CREATE TRIGGER brassclaw_memory_docs_updated_at
    BEFORE UPDATE ON brassclaw_memory_docs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
