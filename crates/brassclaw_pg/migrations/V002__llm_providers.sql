-- V002__llm_providers.sql
CREATE TABLE IF NOT EXISTS brassclaw_llm_providers (
    tenant_id       TEXT        NOT NULL,
    id              TEXT        NOT NULL,
    definition      JSONB       NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, id)
);
CREATE TRIGGER brassclaw_llm_providers_updated_at
    BEFORE UPDATE ON brassclaw_llm_providers
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
