-- Host-boundary original/effective requests and response/error evidence.
-- Historical packets remain NULL: do not infer adjusted requests from text
-- pairs or fabricate provider outcomes. V122's packet snapshot includes this
-- column automatically for newly settled review events.
ALTER TABLE brassclaw_forensic_packets
    ADD COLUMN IF NOT EXISTS model_exchange JSONB;
