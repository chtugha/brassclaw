-- V005__approvals.sql
CREATE TABLE IF NOT EXISTS brassclaw_approvals (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    status      TEXT        NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','denied','expired')),
    request     JSONB       NOT NULL,
    response    JSONB,
    expires_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_approvals_run_idx
    ON brassclaw_approvals (run_id);
CREATE INDEX IF NOT EXISTS brassclaw_approvals_pending_idx
    ON brassclaw_approvals (tenant_id, status) WHERE status = 'pending';
CREATE TRIGGER brassclaw_approvals_updated_at
    BEFORE UPDATE ON brassclaw_approvals
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
