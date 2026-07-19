-- V010__extensions.sql
CREATE TABLE IF NOT EXISTS brassclaw_extension_manifests (
    tenant_id   TEXT        NOT NULL,
    name        TEXT        NOT NULL,
    version     TEXT        NOT NULL,
    manifest    JSONB       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, name, version)
);
CREATE TRIGGER brassclaw_extension_manifests_updated_at
    BEFORE UPDATE ON brassclaw_extension_manifests
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_extensions (
    id           TEXT        NOT NULL PRIMARY KEY,
    tenant_id    TEXT        NOT NULL,
    user_id      TEXT        NOT NULL,
    name         TEXT        NOT NULL,
    version      TEXT        NOT NULL,
    activation_state TEXT     NOT NULL DEFAULT 'installed'
        CHECK (activation_state IN ('installed','disabled','enabled')),
    config       JSONB       NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    removed_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_extensions_user_idx
    ON brassclaw_extensions (tenant_id, user_id) WHERE removed_at IS NULL;
CREATE TRIGGER brassclaw_extensions_updated_at
    BEFORE UPDATE ON brassclaw_extensions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
