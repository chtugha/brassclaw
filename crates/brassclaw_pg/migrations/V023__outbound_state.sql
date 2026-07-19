-- V023__outbound_state.sql
-- Four tables for the outbound state store.

-- Thread notification policies.
-- UNIQUE NULLS NOT DISTINCT: two NULLs compare as equal for agent_id/project_id.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_policies (
    tenant_id   TEXT        NOT NULL,
    agent_id    TEXT,
    project_id  TEXT,
    thread_id   TEXT        NOT NULL,
    policy      JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT brassclaw_outbound_policies_scope_unique
        UNIQUE NULLS NOT DISTINCT (tenant_id, agent_id, project_id, thread_id)
);
CREATE INDEX IF NOT EXISTS brassclaw_outbound_policies_tenant_thread_idx
    ON brassclaw_outbound_policies (tenant_id, thread_id);
CREATE TRIGGER brassclaw_outbound_policies_updated_at
    BEFORE UPDATE ON brassclaw_outbound_policies
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Projection subscription cursors.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_subscriptions (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    cursor      JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_outbound_subscriptions_tenant_idx
    ON brassclaw_outbound_subscriptions (tenant_id);
CREATE TRIGGER brassclaw_outbound_subscriptions_updated_at
    BEFORE UPDATE ON brassclaw_outbound_subscriptions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Delivery attempt records.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_deliveries (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    scope_key   TEXT        NOT NULL,
    payload     JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_outbound_deliveries_tenant_scope_idx
    ON brassclaw_outbound_deliveries (tenant_id, scope_key);
CREATE TRIGGER brassclaw_outbound_deliveries_updated_at
    BEFORE UPDATE ON brassclaw_outbound_deliveries
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Communication preferences.
-- updated_by: always write the real UserId from the record; DEFAULT '' is schema-only.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_preferences (
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    final_reply_target      JSONB,
    progress_target         JSONB,
    approval_prompt_target  JSONB,
    auth_prompt_target      JSONB,
    default_modality        JSONB,
    updated_by              TEXT        NOT NULL DEFAULT '',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id)
);
CREATE TRIGGER brassclaw_outbound_preferences_updated_at
    BEFORE UPDATE ON brassclaw_outbound_preferences
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
