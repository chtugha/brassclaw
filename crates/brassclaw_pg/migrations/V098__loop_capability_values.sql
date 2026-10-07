-- Retain the actual input and completed output behind transcript references.
-- Scope is an identity/correlation key, not a Tool permission or approval.
-- Immutable rows prevent a recreated host from overwriting an earlier result.
CREATE TABLE brassclaw_loop_capability_values (
    reference TEXT PRIMARY KEY,
    scope JSONB NOT NULL CHECK (jsonb_typeof(scope) = 'object'),
    run_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('input', 'result')),
    invocation_id UUID,
    capability_id TEXT,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((kind = 'input' AND invocation_id IS NULL AND capability_id IS NULL)
        OR (kind = 'result' AND invocation_id IS NOT NULL AND capability_id IS NOT NULL)),
    CHECK (octet_length(payload::text) <= 16777216)
);
CREATE INDEX brassclaw_loop_capability_values_run
    ON brassclaw_loop_capability_values (run_id);
CREATE UNIQUE INDEX brassclaw_loop_capability_values_invocation
    ON brassclaw_loop_capability_values (run_id, invocation_id)
    WHERE kind = 'result';
