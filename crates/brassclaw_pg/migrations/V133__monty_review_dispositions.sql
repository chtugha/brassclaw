-- Operator observations never settle effects, release accounting or replay work.
CREATE TABLE brassclaw_monty_review_dispositions (
    disposition_id UUID PRIMARY KEY,
    attempt_id UUID NOT NULL REFERENCES brassclaw_monty_review_work(attempt_id),
    actor TEXT NOT NULL CHECK (length(actor) BETWEEN 1 AND 512),
    evidence_bytes TEXT NOT NULL CHECK (octet_length(evidence_bytes)<=33554432),
    evidence_checksum TEXT NOT NULL CHECK
        (evidence_checksum=encode(sha256(convert_to(evidence_bytes,'UTF8')),'hex')),
    note TEXT NOT NULL CHECK (octet_length(note) BETWEEN 1 AND 16384),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK ((evidence_bytes::jsonb->>'format'='monty-review-inspection/1') IS TRUE),
    CHECK (((evidence_bytes::jsonb->'work'->>'attempt_id')::uuid=attempt_id) IS TRUE),
    CHECK ((evidence_bytes::jsonb->'work'->>'phase' IN ('failed','uncertain','incomplete')) IS TRUE)
);
CREATE INDEX monty_review_dispositions_attempt ON brassclaw_monty_review_dispositions
    (attempt_id,recorded_at DESC,disposition_id);
CREATE TRIGGER immutable_monty_review_dispositions BEFORE UPDATE OR DELETE
    ON brassclaw_monty_review_dispositions FOR EACH ROW
    EXECUTE FUNCTION protect_monty_review_events();
CREATE TRIGGER no_truncate_monty_review_dispositions BEFORE TRUNCATE
    ON brassclaw_monty_review_dispositions FOR EACH STATEMENT
    EXECUTE FUNCTION protect_monty_review_events();
