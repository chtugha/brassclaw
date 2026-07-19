-- V018__root_filesystem.sql
-- VFS blob store + sibling index and event tables.
-- tenant_id is required on all three tables for multi-tenant isolation.
-- The rewrap encrypted-row check (§4.4) scopes to tenant_id.

CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem (
    tenant_id    TEXT        NOT NULL,
    path         TEXT        NOT NULL,
    contents     BYTEA,
    is_dir       BOOLEAN     NOT NULL DEFAULT false,
    content_type TEXT,
    kind         TEXT,
    indexed      JSONB,
    version      BIGINT      NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, path)
);
CREATE INDEX IF NOT EXISTS brassclaw_root_filesystem_tenant_encrypted_idx
    ON brassclaw_root_filesystem (tenant_id)
    WHERE contents IS NOT NULL AND kind = 'encrypted';
CREATE TRIGGER brassclaw_root_filesystem_updated_at
    BEFORE UPDATE ON brassclaw_root_filesystem
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem_index_specs (
    tenant_id   TEXT        NOT NULL,
    prefix      TEXT        NOT NULL,
    name        TEXT        NOT NULL,
    keys        TEXT        NOT NULL,
    kind        TEXT        NOT NULL,
    PRIMARY KEY (tenant_id, prefix, name)
);

CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem_events (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    path        TEXT        NOT NULL,
    payload     BYTEA       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_root_filesystem_events_path_seq_idx
    ON brassclaw_root_filesystem_events (tenant_id, path, seq);
