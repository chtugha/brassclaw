-- V006__turns.sql
-- id stores the TurnRunId ULID. run_id is a FK to brassclaw_runs(id).
-- PgTurnStateStore::create must write the same ULID into both id and run_id.
CREATE TABLE IF NOT EXISTS brassclaw_turns (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    turn_id     TEXT        NOT NULL,
    -- status: snake_case derived from TurnStatus PascalCase variant names
    -- via heck::ToSnakeCase (NOT .to_lowercase()). RecoveryRequired → 'recovery_required'.
    status      TEXT        NOT NULL
        CHECK (status IN (
            'queued',
            'running',
            'blocked_approval',
            'blocked_auth',
            'blocked_resource',
            'blocked_dependent_run',
            'cancel_requested',
            'cancelled',
            'completed',
            'failed',
            'recovery_required'
        )),
    payload     JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_turns_turn_idx
    ON brassclaw_turns (tenant_id, turn_id);
CREATE INDEX IF NOT EXISTS brassclaw_turns_tenant_idx
    ON brassclaw_turns (tenant_id, run_id);
CREATE TRIGGER brassclaw_turns_updated_at
    BEFORE UPDATE ON brassclaw_turns
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
