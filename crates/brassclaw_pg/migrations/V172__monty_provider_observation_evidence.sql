-- Keep the actual typed provider-adapter result before cleaning, textual-call
-- recovery or loop-output classification. This is separate from validated
-- loop responses; a length/filter/schema failure must never fabricate one.
CREATE TABLE brassclaw_monty_provider_observations (
    invocation_id UUID PRIMARY KEY REFERENCES brassclaw_monty_tool_invocations(invocation_id),
    format TEXT NOT NULL CHECK (format = 'provider-model-observation/1'),
    prefix_reference TEXT NOT NULL CHECK (octet_length(prefix_reference) > 0),
    response_bytes TEXT NOT NULL CHECK (octet_length(response_bytes) > 0),
    response_checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (response_checksum = encode(sha256(convert_to(response_bytes, 'UTF8')), 'hex')),
    CHECK ((jsonb_typeof(response_bytes::jsonb) = 'object') IS TRUE),
    CHECK ((response_bytes::jsonb->>'format' = format) IS TRUE),
    CHECK ((jsonb_typeof(response_bytes::jsonb->'response') = 'object') IS TRUE),
    CHECK (coalesce(length(response_bytes::jsonb->>'provider_id'),0) > 0),
    CHECK (coalesce(length(response_bytes::jsonb->>'model_id'),0) > 0)
);
-- The existing bound checks the original admitted kohai Tool occurrence and
-- its captured data_max_bytes. No new fixed capacity or mutable latest setting.
CREATE TRIGGER brassclaw_monty_provider_observations_capacity
    BEFORE INSERT ON brassclaw_monty_provider_observations
    FOR EACH ROW EXECUTE FUNCTION check_monty_model_response();
CREATE TRIGGER brassclaw_monty_provider_observations_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_provider_observations
    FOR EACH ROW EXECUTE FUNCTION protect_monty_model_responses();
CREATE TRIGGER brassclaw_monty_provider_observations_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_provider_observations
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_model_responses();
