-- Same admission owner, versioned metadata. Never backfill an old selection.
ALTER TABLE brassclaw_monty_task_admissions ADD COLUMN native_actor JSONB
    CHECK(native_actor IS NULL OR jsonb_typeof(native_actor) IN ('object','null'));
ALTER TABLE brassclaw_monty_task_admissions DROP CONSTRAINT monty_task_model_selection_shape;
ALTER TABLE brassclaw_monty_task_admissions ADD CONSTRAINT monty_task_model_selection_shape CHECK (
    task_model_selection IS NULL OR (
        task_model_selection_version=1 AND started_at IS NOT NULL
        AND jsonb_typeof(task_model_selection)='object'
        AND task_model_selection ?& ARRAY['format','checksum','selection']
        AND task_model_selection->>'format' IN ('brassclaw-task-provider-binding/1','brassclaw-task-provider-binding/2')
        AND jsonb_typeof(task_model_selection->'selection')='object'
        AND task_model_selection->>'checksum' ~ '^[0-9a-f]{64}$'
        AND (task_model_selection->>'format'<>'brassclaw-task-provider-binding/2'
            OR jsonb_typeof(task_model_selection->'prefix_bindings')='array')) IS TRUE);

-- The existing review admission/effect owner retains its original model family
-- and the optional wire observation. Old rows stay Unknown, not auto-qualified.
ALTER TABLE brassclaw_monty_review_work
    ADD COLUMN model_binding_bytes TEXT,
    ADD COLUMN model_binding_checksum TEXT,
    ADD COLUMN provider_wire_bytes TEXT,
    ADD COLUMN provider_wire_checksum TEXT,
    ADD CONSTRAINT review_model_binding_integrity CHECK (
        (model_binding_bytes IS NULL AND model_binding_checksum IS NULL)
        OR (model_binding_bytes IS NOT NULL AND model_binding_checksum IS NOT NULL
            AND octet_length(model_binding_bytes) BETWEEN 1 AND 8388608
            AND model_binding_checksum=encode(sha256(convert_to(model_binding_bytes,'UTF8')),'hex'))),
    ADD CONSTRAINT review_provider_wire_integrity CHECK (
        (provider_wire_bytes IS NULL AND provider_wire_checksum IS NULL)
        OR (provider_wire_bytes IS NOT NULL AND provider_wire_checksum IS NOT NULL
            AND octet_length(provider_wire_bytes) BETWEEN 1 AND 67108864
            AND provider_wire_checksum=encode(sha256(convert_to(provider_wire_bytes,'UTF8')),'hex')));
CREATE FUNCTION protect_review_model_binding() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.model_binding_bytes IS DISTINCT FROM OLD.model_binding_bytes
        OR NEW.model_binding_checksum IS DISTINCT FROM OLD.model_binding_checksum
        OR (OLD.provider_wire_bytes IS NOT NULL AND
            ROW(NEW.provider_wire_bytes,NEW.provider_wire_checksum) IS DISTINCT FROM
            ROW(OLD.provider_wire_bytes,OLD.provider_wire_checksum)) THEN
        RAISE EXCEPTION 'Original review model binding and captured wire are immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER review_model_binding_immutable BEFORE UPDATE ON brassclaw_monty_review_work
    FOR EACH ROW EXECUTE FUNCTION protect_review_model_binding();

-- Permit only first append of the original native wire after a late response
-- or lost persistence acknowledgement. All terminal outcome/identity fields
-- remain immutable, and the separate trigger above forbids wire replacement.
DO $$
DECLARE definition TEXT; original TEXT := 'AND NEW IS DISTINCT FROM OLD)';
BEGIN
    definition := pg_get_functiondef('protect_monty_review_work()'::regprocedure);
    IF length(definition)-length(replace(definition,original,'')) <> length(original) THEN
        RAISE EXCEPTION 'Unexpected retained review guard; explicit upgrade required' USING ERRCODE='23514';
    END IF;
    definition := replace(definition,original,
        'AND (to_jsonb(NEW)-ARRAY[''provider_wire_bytes'',''provider_wire_checksum'']) IS DISTINCT FROM (to_jsonb(OLD)-ARRAY[''provider_wire_bytes'',''provider_wire_checksum'']))');
    EXECUTE definition;
END $$;
