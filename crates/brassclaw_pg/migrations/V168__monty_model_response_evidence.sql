-- Actual native model responses precede Tool output validation. A later schema,
-- budget, IPC or reply error neither removes this evidence nor permits replay.
CREATE TABLE brassclaw_monty_model_responses (
    invocation_id UUID PRIMARY KEY REFERENCES brassclaw_monty_tool_invocations(invocation_id),
    format TEXT NOT NULL CHECK (format = 'loop-model-response/1'),
    prefix_reference TEXT NOT NULL CHECK (octet_length(prefix_reference) > 0),
    response_bytes TEXT NOT NULL CHECK (octet_length(response_bytes) > 0),
    response_checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (response_checksum = encode(sha256(convert_to(response_bytes, 'UTF8')), 'hex')),
    CHECK ((jsonb_typeof(response_bytes::jsonb) = 'object') IS TRUE)
);
CREATE FUNCTION check_monty_model_response() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    captured_bound BIGINT;
BEGIN
    SELECT data_max_bytes INTO STRICT captured_bound
        FROM brassclaw_monty_tool_invocations
        WHERE invocation_id=NEW.invocation_id
          AND selection_bytes::jsonb->>'capability_id'='host.kohai_complete';
    IF octet_length(NEW.response_bytes) > captured_bound THEN
        RAISE EXCEPTION 'Native model response exceeds its captured data capacity' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER brassclaw_monty_model_responses_capacity
    BEFORE INSERT ON brassclaw_monty_model_responses
    FOR EACH ROW EXECUTE FUNCTION check_monty_model_response();
CREATE FUNCTION protect_monty_model_responses() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Native model response evidence is immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER brassclaw_monty_model_responses_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_model_responses
    FOR EACH ROW EXECUTE FUNCTION protect_monty_model_responses();
CREATE TRIGGER brassclaw_monty_model_responses_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_model_responses
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_model_responses();
