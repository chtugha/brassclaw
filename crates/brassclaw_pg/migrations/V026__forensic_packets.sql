-- V026__forensic_packets.sql
-- Durable store for ForensicPacket records (one per agent-loop turn/iteration).
-- Replaces NoopInterceptorStore. PgInterceptorStore is the new implementation.
--
-- PacketStatus wire values (serde rename_all = "snake_case"):
--   'awaiting_kohai' | 'complete' | 'sempai_reviewed'
--
-- kohai_usage is stored as typed INTEGER columns (not JSONB) for analytics.
-- All other prompt/response/review fields are JSONB for forward-compatibility.
CREATE TABLE IF NOT EXISTS brassclaw_forensic_packets (
    id                TEXT        NOT NULL PRIMARY KEY,
    tenant_id         TEXT        NOT NULL,
    run_id            TEXT        NOT NULL,
    iteration         INTEGER     NOT NULL DEFAULT 0,
    status            TEXT        NOT NULL DEFAULT 'awaiting_kohai'
        CHECK (status IN ('awaiting_kohai','complete','sempai_reviewed')),
    captured_at       TIMESTAMPTZ NOT NULL,
    completed_at      TIMESTAMPTZ,
    prompt            JSONB       NOT NULL DEFAULT '{}',
    kohai_response    TEXT,
    kohai_input_tokens             INTEGER,
    kohai_output_tokens            INTEGER,
    kohai_cache_read_input_tokens       INTEGER,
    kohai_cache_creation_input_tokens   INTEGER,
    sempai_review     JSONB,
    -- chat_record_id: populated retroactively after memory_write for this run.
    -- Stores the FIRST chat_record_id written in this iteration.
    -- For multi-memory turns, query brassclaw_memory_chat_records by (run_id, iteration).
    chat_record_id    TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_tenant_captured_idx
    ON brassclaw_forensic_packets (tenant_id, captured_at DESC);
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_run_idx
    ON brassclaw_forensic_packets (tenant_id, run_id, iteration);
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_awaiting_idx
    ON brassclaw_forensic_packets (tenant_id, captured_at)
    WHERE status = 'awaiting_kohai';
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_chat_record_idx
    ON brassclaw_forensic_packets (chat_record_id)
    WHERE chat_record_id IS NOT NULL;
CREATE TRIGGER brassclaw_forensic_packets_updated_at
    BEFORE UPDATE ON brassclaw_forensic_packets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
