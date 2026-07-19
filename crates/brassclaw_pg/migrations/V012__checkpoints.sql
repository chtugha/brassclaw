-- V012__checkpoints.sql
-- Retention: last 10 per run + 30-day TTL (enforced by app-layer cleanup).
-- pg_cron is NOT used — retention runs inside the serve process only.
CREATE TABLE IF NOT EXISTS brassclaw_checkpoints (
    id          BIGSERIAL    PRIMARY KEY,
    tenant_id   TEXT         NOT NULL,
    turn_id     TEXT         NOT NULL,
    -- run_id: soft reference (no FK) — checkpoints may outlive turn rows.
    run_id      TEXT         NOT NULL,
    state_ref   TEXT         NOT NULL,
    schema_id   TEXT         NOT NULL,
    schema_version BIGINT    NOT NULL,
    -- kind: LoopCheckpointKind as_str() values.
    kind        TEXT         NOT NULL
        CHECK (kind IN ('before_model','before_side_effect','before_block','final')),
    -- payload: raw bytes, not JSON. BYTEA avoids the hex-encode/decode of the FS store.
    payload     BYTEA        NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS brassclaw_checkpoints_natural_key_idx
    ON brassclaw_checkpoints (tenant_id, run_id, state_ref, schema_id, schema_version, kind);
CREATE INDEX IF NOT EXISTS brassclaw_checkpoints_run_kind_idx
    ON brassclaw_checkpoints (tenant_id, run_id, kind);
CREATE INDEX IF NOT EXISTS brassclaw_checkpoints_tenant_age_idx
    ON brassclaw_checkpoints (tenant_id, created_at);
