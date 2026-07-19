-- V001__config.sql
CREATE TABLE IF NOT EXISTS brassclaw_config (
    tenant_id   TEXT        NOT NULL,
    key         TEXT        NOT NULL,
    value       TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, key)
);
CREATE TRIGGER brassclaw_config_updated_at
    BEFORE UPDATE ON brassclaw_config
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
