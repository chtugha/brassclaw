-- One actual related model operation under an existing native Tool invocation.
-- No Tool grant, approval, executor, new run or automatic replay is introduced.
CREATE TABLE brassclaw_monty_related_model_work (
    work_id UUID PRIMARY KEY,
    invocation_id UUID NOT NULL REFERENCES brassclaw_monty_tool_invocations(invocation_id),
    work_bytes TEXT NOT NULL CHECK (octet_length(work_bytes) > 0),
    work_checksum TEXT NOT NULL,
    request_bytes TEXT NOT NULL CHECK (octet_length(request_bytes) > 0),
    request_checksum TEXT NOT NULL,
    prefix_reference TEXT NOT NULL CHECK (octet_length(prefix_reference) > 0),
    phase TEXT NOT NULL DEFAULT 'prepared' CHECK (phase IN ('prepared','dispatch_intent')),
    dispatch_count SMALLINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    dispatched_at TIMESTAMPTZ,
    CHECK (work_id <> invocation_id),
    CHECK (work_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CHECK (work_checksum = encode(sha256(convert_to(work_bytes,'UTF8')),'hex')),
    CHECK (request_checksum = encode(sha256(convert_to(request_bytes,'UTF8')),'hex')),
    CHECK ((work_bytes::jsonb->>'work_id' = work_id::text) IS TRUE),
    CHECK ((work_bytes::jsonb->'kind'->>'kind' = 'sempai_review') IS TRUE),
    CHECK ((work_bytes::jsonb->>'model_profile_id' = 'sempai_model') IS TRUE),
    CHECK ((jsonb_typeof(work_bytes::jsonb->'resolved_model_route') = 'object') IS TRUE),
    CHECK ((request_bytes::jsonb->'resolved_model_route' = work_bytes::jsonb->'resolved_model_route') IS TRUE),
    CHECK ((request_bytes::jsonb->'model_profile_id' = work_bytes::jsonb->'model_profile_id') IS TRUE),
    CHECK ((jsonb_typeof(request_bytes::jsonb->'messages') = 'array') IS TRUE),
    CHECK ((phase='prepared' AND dispatch_count=0 AND dispatched_at IS NULL)
        OR (phase='dispatch_intent' AND dispatch_count=1 AND dispatched_at IS NOT NULL))
);

CREATE FUNCTION check_monty_related_model_work() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    original_run UUID;
    original_turn UUID;
    captured_bound BIGINT;
BEGIN
    SELECT i.run_id,a.turn_id,i.data_max_bytes INTO STRICT original_run,original_turn,captured_bound
        FROM brassclaw_monty_tool_invocations i
        JOIN brassclaw_monty_task_admissions a ON a.run_id=i.run_id AND a.admission_key=i.admission_key
        WHERE i.invocation_id=NEW.invocation_id
          AND i.selection_bytes::jsonb->>'capability_id'='host.kohai_complete';
    IF octet_length(NEW.work_bytes) > captured_bound OR octet_length(NEW.request_bytes) > captured_bound
        OR (NEW.request_bytes::jsonb->>'run_id' = original_run::text) IS NOT TRUE
        OR (NEW.request_bytes::jsonb->>'turn_id' = original_turn::text) IS NOT TRUE THEN
        RAISE EXCEPTION 'Related model request differs from its original invocation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER brassclaw_monty_related_model_work_capacity
    BEFORE INSERT ON brassclaw_monty_related_model_work
    FOR EACH ROW EXECUTE FUNCTION check_monty_related_model_work();

CREATE FUNCTION protect_monty_related_model_work() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'Related model work is retained' USING ERRCODE='23514';
    END IF;
    IF (to_jsonb(OLD) - ARRAY['phase','dispatch_count','dispatched_at'])
        IS DISTINCT FROM (to_jsonb(NEW) - ARRAY['phase','dispatch_count','dispatched_at'])
        OR OLD.phase <> 'prepared' OR NEW.phase <> 'dispatch_intent'
        OR OLD.dispatch_count <> 0 OR NEW.dispatch_count <> 1
        OR OLD.dispatched_at IS NOT NULL OR NEW.dispatched_at IS NULL THEN
        RAISE EXCEPTION 'Related model work cannot be replaced or replayed' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER brassclaw_monty_related_model_work_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_related_model_work
    FOR EACH ROW EXECUTE FUNCTION protect_monty_related_model_work();
CREATE TRIGGER brassclaw_monty_related_model_work_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_related_model_work
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_related_model_work();

-- Observation format markers and original payload bytes are unchanged. The
-- separate native address prevents overwriting the primary model's originals.
CREATE TABLE brassclaw_monty_related_model_observations (
    work_id UUID NOT NULL REFERENCES brassclaw_monty_related_model_work(work_id),
    format TEXT NOT NULL CHECK (format IN ('provider-wire-observation/1','provider-model-observation/1','loop-model-response/1')),
    response_bytes TEXT NOT NULL CHECK (octet_length(response_bytes) > 0),
    response_checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(work_id,format),
    CHECK (response_checksum = encode(sha256(convert_to(response_bytes,'UTF8')),'hex')),
    CHECK ((jsonb_typeof(response_bytes::jsonb) = 'object') IS TRUE)
);

CREATE FUNCTION check_monty_related_model_observation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    captured_bound BIGINT;
    original_route JSONB;
    observed JSONB;
BEGIN
    SELECT i.data_max_bytes,w.work_bytes::jsonb->'resolved_model_route'
        INTO STRICT captured_bound,original_route
        FROM brassclaw_monty_related_model_work w
        JOIN brassclaw_monty_tool_invocations i ON i.invocation_id=w.invocation_id
        WHERE w.work_id=NEW.work_id AND w.phase='dispatch_intent' AND w.dispatch_count=1;
    IF octet_length(NEW.response_bytes) > captured_bound THEN
        RAISE EXCEPTION 'Related model observation exceeds original capacity' USING ERRCODE='23514';
    END IF;
    observed := NEW.response_bytes::jsonb;
    IF NEW.format <> 'loop-model-response/1' THEN
        IF (observed->>'format' = NEW.format) IS NOT TRUE
            OR (observed->>'provider_id' = original_route->>'provider_id') IS NOT TRUE
            OR (observed->>'model_id' = original_route->>'model_id') IS NOT TRUE THEN
            RAISE EXCEPTION 'Related model observation identity mismatch' USING ERRCODE='23514';
        END IF;
        IF NEW.format='provider-wire-observation/1' AND (
            (observed->>'body_encoding' = 'base64') IS NOT TRUE
            OR (jsonb_typeof(observed->'body') = 'string') IS NOT TRUE
            OR (jsonb_typeof(observed->'status') = 'number') IS NOT TRUE
            OR ((observed->>'status')::integer BETWEEN 100 AND 599) IS NOT TRUE
        ) THEN
            RAISE EXCEPTION 'Related model wire envelope mismatch' USING ERRCODE='23514';
        END IF;
        IF NEW.format='provider-model-observation/1'
            AND (jsonb_typeof(observed->'response') = 'object') IS NOT TRUE THEN
            RAISE EXCEPTION 'Related model provider envelope mismatch' USING ERRCODE='23514';
        END IF;
    ELSIF (observed->>'effective_model_profile_id' = 'sempai_model') IS NOT TRUE THEN
        RAISE EXCEPTION 'Related model loop profile mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER brassclaw_monty_related_model_observations_capacity
    BEFORE INSERT ON brassclaw_monty_related_model_observations
    FOR EACH ROW EXECUTE FUNCTION check_monty_related_model_observation();
CREATE TRIGGER brassclaw_monty_related_model_observations_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_related_model_observations
    FOR EACH ROW EXECUTE FUNCTION protect_monty_model_responses();
CREATE TRIGGER brassclaw_monty_related_model_observations_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_related_model_observations
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_model_responses();
