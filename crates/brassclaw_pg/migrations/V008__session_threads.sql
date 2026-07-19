-- V008__session_threads.sql
CREATE TABLE IF NOT EXISTS brassclaw_session_threads (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT        NOT NULL,
    project_id  TEXT,
    mission_id  TEXT,
    created_by_actor_id TEXT NOT NULL,
    title       TEXT,
    metadata    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_user_idx
    ON brassclaw_session_threads (tenant_id, user_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_agent_idx
    ON brassclaw_session_threads (tenant_id, agent_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_project_idx
    ON brassclaw_session_threads (tenant_id, project_id)
    WHERE deleted_at IS NULL AND project_id IS NOT NULL;
CREATE TRIGGER brassclaw_session_threads_updated_at
    BEFORE UPDATE ON brassclaw_session_threads
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
