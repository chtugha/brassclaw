-- A fresh root execution is never an implicit retry of an already admitted run.
-- Claims are Rust-only fencing identities, not Tool invocation grants.
-- Keep these receipts through reconciliation/checkpoint retention. There is no
-- automatic deletion, reset-to-pending or replay of an uncertain admission.
CREATE TABLE brassclaw_monty_task_admissions (
    run_id UUID PRIMARY KEY,
    turn_id UUID NOT NULL,
    scope JSONB NOT NULL CHECK (jsonb_typeof(scope) = 'object'),
    accepted_message_ref TEXT NOT NULL,
    runner_id UUID NOT NULL,
    claim_checksum BYTEA NOT NULL CHECK (octet_length(claim_checksum) = 32),
    admission_key BYTEA NOT NULL UNIQUE CHECK (octet_length(admission_key) = 32),
    phase TEXT NOT NULL CHECK (phase IN ('reserved', 'started', 'settled')),
    outcome JSONB,
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    settled_at TIMESTAMPTZ,
    CHECK ((phase = 'reserved' AND started_at IS NULL AND settled_at IS NULL AND outcome IS NULL)
        OR (phase = 'started' AND started_at IS NOT NULL AND settled_at IS NULL AND outcome IS NULL)
        OR (phase = 'settled' AND settled_at IS NOT NULL
            AND outcome IS NOT NULL AND jsonb_typeof(outcome) = 'object'
            AND (started_at IS NOT NULL
                OR (outcome->>'status' = 'failed' AND outcome->>'reason_kind' = 'task_cancelled'))))
);
