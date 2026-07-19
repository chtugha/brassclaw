-- V014__token_settings.sql
CREATE TABLE IF NOT EXISTS brassclaw_token_settings (
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    provider_id TEXT        NOT NULL,
    settings    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, provider_id)
);
CREATE TRIGGER brassclaw_token_settings_updated_at
    BEFORE UPDATE ON brassclaw_token_settings
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
