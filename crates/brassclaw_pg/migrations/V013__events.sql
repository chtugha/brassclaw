-- V013__events.sql
-- Retention: 90-day rolling window (brassclaw_events); 1-year (brassclaw_audit_log).
-- run_id is a soft reference (no FK) — events are append-only and may outlive runs.
CREATE TABLE IF NOT EXISTS brassclaw_events (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT,
    kind        TEXT        NOT NULL,
    payload     JSONB       NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_events_run_idx
    ON brassclaw_events (run_id) WHERE run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_events_tenant_idx
    ON brassclaw_events (tenant_id, occurred_at DESC);

CREATE TABLE IF NOT EXISTS brassclaw_audit_log (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    actor_id    TEXT,
    action      TEXT        NOT NULL,
    resource    TEXT,
    payload     JSONB,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_audit_log_tenant_idx
    ON brassclaw_audit_log (tenant_id, occurred_at DESC);
