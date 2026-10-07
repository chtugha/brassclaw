-- Stop-only flat Recipe dispatch evidence. A record is not Tool permission or
-- combination approval. Missing answers remain uncertain across process loss;
-- no writer may reset an invocation or repeat its initial dispatch.
CREATE TABLE IF NOT EXISTS brassclaw_monty_recipe_selections (
    run_id UUID NOT NULL REFERENCES brassclaw_monty_task_admissions(run_id),
    recipe_id UUID NOT NULL REFERENCES reborn_component_revision_heads(component_id),
    selection_bytes TEXT NOT NULL CHECK (octet_length(selection_bytes) BETWEEN 1 AND 8388608),
    selection_checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (run_id, recipe_id),
    CHECK (selection_checksum = encode(sha256(convert_to(selection_bytes, 'UTF8')), 'hex')),
    CHECK ((jsonb_typeof(selection_bytes::jsonb) = 'object') IS TRUE),
    CHECK ((selection_bytes::jsonb->>'format' = 'monty-retained-recipe-selection/1') IS TRUE),
    CHECK (((selection_bytes::jsonb->'recipe'->>'uuid')::uuid = recipe_id) IS TRUE)
);
CREATE FUNCTION protect_monty_recipe_selections() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Monty recipe selections are immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER brassclaw_monty_recipe_selections_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_recipe_selections
    FOR EACH ROW EXECUTE FUNCTION protect_monty_recipe_selections();
CREATE TRIGGER brassclaw_monty_recipe_selections_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_recipe_selections
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_recipe_selections();

CREATE TABLE IF NOT EXISTS brassclaw_monty_tool_invocations (
    invocation_id UUID PRIMARY KEY CHECK (invocation_id <> '00000000-0000-0000-0000-000000000000'),
    run_id UUID NOT NULL REFERENCES brassclaw_monty_task_admissions(run_id),
    recipe_id UUID NOT NULL REFERENCES reborn_component_revision_heads(component_id),
    step_id TEXT NOT NULL CHECK (octet_length(step_id) BETWEEN 1 AND 64),
    admission_key BYTEA NOT NULL CHECK (octet_length(admission_key) = 32),
    invocation_key BYTEA NOT NULL UNIQUE CHECK (octet_length(invocation_key) = 32),
    selection_bytes TEXT NOT NULL CHECK (octet_length(selection_bytes) BETWEEN 1 AND 8388608),
    selection_checksum TEXT NOT NULL,
    arguments_bytes TEXT NOT NULL CHECK (octet_length(arguments_bytes) BETWEEN 1 AND 8388608),
    arguments_checksum TEXT NOT NULL,
    attempt_count SMALLINT NOT NULL CHECK (attempt_count = 1),
    phase TEXT NOT NULL CHECK (phase IN ('dispatch_intent', 'answered')),
    answer_bytes TEXT CHECK (octet_length(answer_bytes) BETWEEN 1 AND 8388608),
    answer_checksum TEXT,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    answered_at TIMESTAMPTZ,
    UNIQUE (run_id, recipe_id, step_id),
    FOREIGN KEY (run_id, recipe_id) REFERENCES brassclaw_monty_recipe_selections(run_id, recipe_id),
    CHECK (selection_checksum = encode(sha256(convert_to(selection_bytes, 'UTF8')), 'hex')),
    CHECK (jsonb_typeof(selection_bytes::jsonb) = 'object'),
    CHECK (arguments_checksum = encode(sha256(convert_to(arguments_bytes, 'UTF8')), 'hex')),
    CHECK (jsonb_typeof(arguments_bytes::jsonb) = 'object'),
    CHECK ((phase = 'dispatch_intent' AND answer_bytes IS NULL
            AND answer_checksum IS NULL AND answered_at IS NULL)
        OR (phase = 'answered' AND answer_bytes IS NOT NULL
            AND answer_checksum IS NOT NULL AND answered_at IS NOT NULL
            AND answer_checksum = encode(sha256(convert_to(answer_bytes, 'UTF8')), 'hex')
            AND jsonb_typeof(answer_bytes::jsonb) = 'object'))
);

CREATE FUNCTION protect_monty_tool_invocations() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'Monty invocation evidence cannot be deleted' USING ERRCODE='23514';
    END IF;
    IF OLD.phase <> 'dispatch_intent' OR NEW.phase <> 'answered'
        OR (to_jsonb(OLD) - ARRAY['phase','answer_bytes','answer_checksum','answered_at'])
            IS DISTINCT FROM
           (to_jsonb(NEW) - ARRAY['phase','answer_bytes','answer_checksum','answered_at']) THEN
        RAISE EXCEPTION 'Monty invocation identity and completed answers are immutable'
            USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER brassclaw_monty_tool_invocations_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_tool_invocations
    FOR EACH ROW EXECUTE FUNCTION protect_monty_tool_invocations();
CREATE TRIGGER brassclaw_monty_tool_invocations_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_tool_invocations
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_tool_invocations();
