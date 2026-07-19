-- V022__conversation_state.sql
-- Single row per tenant; CAS via revision counter.
-- See §4.25 for the full CAS contract for PgConversationStateStore.
CREATE TABLE IF NOT EXISTS brassclaw_conversation_state (
    tenant_id   TEXT        NOT NULL PRIMARY KEY,
    state_blob  JSONB       NOT NULL DEFAULT '{}',
    revision    BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER brassclaw_conversation_state_updated_at
    BEFORE UPDATE ON brassclaw_conversation_state
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
