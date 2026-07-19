-- V004__runs.sql
CREATE TABLE IF NOT EXISTS brassclaw_runs (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT,
    project_id  TEXT,
    thread_id   TEXT,
    status      TEXT        NOT NULL
        CHECK (status IN ('running','blocked_approval','blocked_auth','completed','failed')),
    payload     JSONB       NOT NULL DEFAULT '{}',
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_runs_tenant_status_idx
    ON brassclaw_runs (tenant_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_runs_thread_idx
    ON brassclaw_runs (tenant_id, thread_id) WHERE thread_id IS NOT NULL;
CREATE TRIGGER brassclaw_runs_updated_at
    BEFORE UPDATE ON brassclaw_runs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
