-- V068__reborn_security_settings.sql
-- Step C.4 — Mode-driven security: global operator-level per-layer overrides.
--
-- One row per tenant (the operator-level security posture; NOT per-user /
-- per-agent / per-project). The C.6 cross-turn driver loads the row via the
-- composition `SecurityConfigSource` impl, derives the per-turn `SecurityMode`
-- from `host.resolve_intent` (Matching = intent matched vs Non-Matching = LLM
-- fallback), and resolves each layer's active state through
-- `SecurityModeConfig::resolve_all(mode)`.
--
-- Per-column values:
--   'auto' = mode-driven default
--            (Matching → wrapper OFF for Q2+ validated components;
--             Non-Matching → wrapper ON because an LLM is involved)
--   'on'   = operator force-ON regardless of mode
--   'off'  = operator force-OFF regardless of mode
--
-- The six layers (Fork1=A, all individually toggleable):
--   policy                  — PolicyEngine (capability policy)
--   leases                  — LeaseManager (capability leases)
--   gate                    — GateController (approval gates)
--   event_emission          — event emission (observability; default ON in both
--                             modes — not a security gate)
--   sensitive_tool_scoping  — sensitive-tool self-scoping
--   namespace_filtering     — bind-time namespace filtering (LLM path only)
--
-- No DB-less mode (Postgres is always used): a missing tenant row yields
-- `SecurityModeConfig::default()` (all 'auto') in the store, and the WebUI PUT
-- upserts the row on first save. There is intentionally NO system seed row.

CREATE TABLE IF NOT EXISTS reborn_security_settings (
    id                                  UUID        NOT NULL DEFAULT gen_random_uuid(),
    tenant_id                           TEXT        NOT NULL,

    policy_override                     TEXT        NOT NULL DEFAULT 'auto'
        CHECK (policy_override IN ('auto','on','off')),
    leases_override                     TEXT        NOT NULL DEFAULT 'auto'
        CHECK (leases_override IN ('auto','on','off')),
    gate_override                       TEXT        NOT NULL DEFAULT 'auto'
        CHECK (gate_override IN ('auto','on','off')),
    event_emission_override             TEXT        NOT NULL DEFAULT 'auto'
        CHECK (event_emission_override IN ('auto','on','off')),
    sensitive_tool_scoping_override     TEXT        NOT NULL DEFAULT 'auto'
        CHECK (sensitive_tool_scoping_override IN ('auto','on','off')),
    namespace_filtering_override        TEXT        NOT NULL DEFAULT 'auto'
        CHECK (namespace_filtering_override IN ('auto','on','off')),

    created_at                          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at                          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_security_settings_pk PRIMARY KEY (id),
    CONSTRAINT reborn_security_settings_tenant_unique UNIQUE (tenant_id)
);

CREATE INDEX IF NOT EXISTS reborn_security_settings_tenant_idx
    ON reborn_security_settings (tenant_id);

-- ── updated_at trigger ──────────────────────────────────────────────────────

CREATE TRIGGER reborn_security_settings_updated_at
    BEFORE UPDATE ON reborn_security_settings
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
