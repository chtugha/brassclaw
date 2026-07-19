-- V020__identities.sql
-- Three tables for the full 5-part identity key.
-- provider_instance_id: stored as '' (empty string) when absent (NOT NULL).

CREATE TABLE IF NOT EXISTS brassclaw_identities (
    id                   TEXT        NOT NULL PRIMARY KEY,
    tenant_id            TEXT        NOT NULL,
    surface_kind         TEXT        NOT NULL
        CHECK (surface_kind IN ('oauth','channel_actor')),
    provider_kind        TEXT        NOT NULL,
    provider_instance_id TEXT        NOT NULL DEFAULT '',
    external_subject_id  TEXT        NOT NULL,
    user_id              TEXT        NOT NULL,
    email                TEXT,
    email_verified       BOOLEAN     NOT NULL DEFAULT false,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS brassclaw_identities_key_idx
    ON brassclaw_identities
    (tenant_id, surface_kind, provider_kind, provider_instance_id, external_subject_id);
CREATE INDEX IF NOT EXISTS brassclaw_identities_user_idx
    ON brassclaw_identities (tenant_id, user_id);
CREATE TRIGGER brassclaw_identities_updated_at
    BEFORE UPDATE ON brassclaw_identities
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_identity_users (
    user_id              TEXT        NOT NULL PRIMARY KEY,
    email                TEXT,
    display_name         TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_identity_users_email_idx
    ON brassclaw_identity_users (email) WHERE email IS NOT NULL;
CREATE TRIGGER brassclaw_identity_users_updated_at
    BEFORE UPDATE ON brassclaw_identity_users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Verified-email cross-provider link index. First writer wins (CasExpectation::Absent).
CREATE TABLE IF NOT EXISTS brassclaw_identity_email_index (
    tenant_id            TEXT        NOT NULL,
    email_lower          TEXT        NOT NULL,
    user_id              TEXT        NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, email_lower)
);
