-- V007__capability_leases.sql
-- NOTE: The partial index uses only `status = 'active'`. Do NOT add
-- `expires_at > now()` to the partial index predicate — `now()` in a partial
-- index is evaluated once at creation time, not at query time.
-- The expiry check belongs in the WHERE clause of app-layer queries.
CREATE TABLE IF NOT EXISTS brassclaw_capability_leases (
    id              TEXT        NOT NULL PRIMARY KEY,
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    capability_id   TEXT        NOT NULL,
    -- App layer must lowercase CapabilityLeaseStatus variant names:
    -- Active→'active', Claimed→'claimed', Consumed→'consumed', Revoked→'revoked'.
    status          TEXT        NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','claimed','consumed','revoked')),
    "grant"         JSONB       NOT NULL,
    invocation_fingerprint TEXT,
    expires_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_capability_leases_user_cap_idx
    ON brassclaw_capability_leases (tenant_id, user_id, capability_id)
    WHERE status = 'active';
CREATE TRIGGER brassclaw_capability_leases_updated_at
    BEFORE UPDATE ON brassclaw_capability_leases
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
