-- V011__resources.sql
-- version column enables CAS updates (mirrors CasSnapshotStore behaviour).
-- See §4.12 for the two-step INSERT ... ON CONFLICT DO NOTHING + CAS UPDATE pattern.
CREATE TABLE IF NOT EXISTS brassclaw_resource_accounts (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- App layer must lowercase ResourceAccount variant names:
    -- Tenant→'tenant', User→'user', Project→'project', Agent→'agent',
    -- Mission→'mission', Thread→'thread'.
    scope_kind  TEXT        NOT NULL
        CHECK (scope_kind IN ('tenant','user','project','agent','mission','thread')),
    scope_id    TEXT        NOT NULL,
    period_key  TEXT        NOT NULL,
    reserved    NUMERIC(18,6) NOT NULL DEFAULT 0,
    consumed    NUMERIC(18,6) NOT NULL DEFAULT 0,
    limit_usd   NUMERIC(18,6),
    version     BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, scope_kind, scope_id, period_key)
);
CREATE TRIGGER brassclaw_resource_accounts_updated_at
    BEFORE UPDATE ON brassclaw_resource_accounts
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
