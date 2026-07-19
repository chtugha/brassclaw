-- V003__secrets.sql
CREATE TABLE IF NOT EXISTS brassclaw_secrets_master (
    tenant_id       TEXT        NOT NULL,
    version         INT         NOT NULL DEFAULT 1,
    -- raw-key-on-disk ceremony: wrapped_key = '' AND algorithm = 'raw-key-on-disk'
    --   (key lives at $REBORN_HOME/.secrets-master-key, never in the DB).
    -- passphrase-wrapped ceremony: wrapped_key = base64(nonce || ciphertext),
    --   algorithm = 'aes256gcm-argon2id'
    wrapped_key     TEXT        NOT NULL DEFAULT '',
    algorithm       TEXT        NOT NULL DEFAULT 'raw-key-on-disk',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, version)
);
CREATE TRIGGER brassclaw_secrets_master_updated_at
    BEFORE UPDATE ON brassclaw_secrets_master
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_secrets (
    tenant_id   TEXT        NOT NULL,
    scope       TEXT        NOT NULL,
    name        TEXT        NOT NULL,
    ciphertext  TEXT        NOT NULL,
    key_version INT         NOT NULL DEFAULT 1,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, scope, name)
);
CREATE TRIGGER brassclaw_secrets_updated_at
    BEFORE UPDATE ON brassclaw_secrets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
