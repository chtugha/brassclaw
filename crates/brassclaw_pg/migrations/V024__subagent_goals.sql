-- V024__subagent_goals.sql
-- One goal per run (UNIQUE on tenant_id, run_id).
-- No FK to brassclaw_runs — goals may arrive before the run row is committed.
CREATE TABLE IF NOT EXISTS brassclaw_subagent_goals (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        NOT NULL,
    task        TEXT        NOT NULL,
    handoff     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT brassclaw_subagent_goals_tenant_run_unique UNIQUE (tenant_id, run_id)
);
CREATE INDEX IF NOT EXISTS brassclaw_subagent_goals_tenant_idx
    ON brassclaw_subagent_goals (tenant_id);
CREATE TRIGGER brassclaw_subagent_goals_updated_at
    BEFORE UPDATE ON brassclaw_subagent_goals
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
