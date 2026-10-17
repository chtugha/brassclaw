-- Native owner evidence only; original outcomes and operator notes stay intact.
CREATE TABLE brassclaw_monty_review_reconciliations (
    attempt_id UUID PRIMARY KEY REFERENCES brassclaw_monty_review_work(attempt_id),
    evidence_bytes TEXT NOT NULL CHECK (octet_length(evidence_bytes)<=33554432),
    evidence_checksum TEXT NOT NULL CHECK
        (evidence_checksum=encode(sha256(convert_to(evidence_bytes,'UTF8')),'hex')),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK ((evidence_bytes::jsonb->>'format'='completed-turn-review-reconciliation/1') IS TRUE),
    CHECK (((evidence_bytes::jsonb->>'attempt_id')::uuid=attempt_id) IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'model_accounting_settled'='true') IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'work_replayed'='false') IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'catalogue_activated'='false') IS TRUE)
);
CREATE TRIGGER immutable_monty_review_reconciliations BEFORE UPDATE OR DELETE
    ON brassclaw_monty_review_reconciliations FOR EACH ROW EXECUTE FUNCTION protect_monty_review_events();
CREATE TRIGGER no_truncate_monty_review_reconciliations BEFORE TRUNCATE
    ON brassclaw_monty_review_reconciliations FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_review_events();
