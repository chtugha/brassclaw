-- V015__safety.sql
CREATE TABLE IF NOT EXISTS brassclaw_safety_config (
    id              TEXT        NOT NULL PRIMARY KEY,
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    -- category: SafetyCategory as_str() values.
    category        TEXT        NOT NULL
        CHECK (category IN ('sensitive_paths','workspace_rules','blocked_paths')),
    pattern         TEXT        NOT NULL,
    is_enabled      BOOLEAN     NOT NULL DEFAULT true,
    is_default      BOOLEAN     NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, user_id, category, pattern)
);
CREATE TRIGGER brassclaw_safety_config_updated_at
    BEFORE UPDATE ON brassclaw_safety_config
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_capability_permissions (
    tenant_id       TEXT        NOT NULL,
    capability_id   TEXT        NOT NULL,
    -- permission_mode: PermissionMode snake_case values.
    -- NOTE: there is NO 'org_policy' variant — that belongs to ApprovalPolicy.
    permission_mode TEXT        NOT NULL
        CHECK (permission_mode IN ('allow','ask','deny')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, capability_id)
);
CREATE TRIGGER brassclaw_capability_permissions_updated_at
    BEFORE UPDATE ON brassclaw_capability_permissions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
