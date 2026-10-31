-- Retain the real host ingress separately from deterministic-execution policy.
-- Legacy NULL means the historical admission did not retain its source; never
-- fabricate an original binding from current configuration or a result payload.
ALTER TABLE brassclaw_monty_task_admissions
    ADD COLUMN source_binding_ref TEXT,
    ADD COLUMN result_only_completion BOOLEAN GENERATED ALWAYS AS (
        trusted_internal_turn OR COALESCE(
            source_binding_ref = 'brassclaw-internal-model-work', false)
    ) STORED;

ALTER TABLE brassclaw_monty_task_admissions
    ALTER COLUMN result_only_completion SET NOT NULL,
    ADD CONSTRAINT monty_admission_original_source_nonempty
        CHECK (source_binding_ref IS NULL OR source_binding_ref <> ''),
    ADD CONSTRAINT monty_admission_original_deterministic_source
        CHECK (source_binding_ref IS NULL OR trusted_internal_turn =
            (source_binding_ref = 'codex-internal-prefix-regeneration'));

CREATE FUNCTION protect_monty_admission_completion_origin() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.source_binding_ref IS DISTINCT FROM OLD.source_binding_ref
        OR NEW.trusted_internal_turn IS DISTINCT FROM OLD.trusted_internal_turn THEN
        RAISE EXCEPTION 'Monty admission completion origin is immutable'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER brassclaw_monty_admission_completion_origin_immutable
    BEFORE UPDATE ON brassclaw_monty_task_admissions
    FOR EACH ROW EXECUTE FUNCTION protect_monty_admission_completion_origin();

-- PostgreSQL fills a generated column after BEFORE triggers. The generic
-- admission guard must compare original identity, not the not-yet-computed NEW
-- projection against OLD's stored value. Preserve every existing transition and
-- pin guard; the actual source/trusted fields remain in its identity comparison
-- and have the additional immutable-origin guard above.
DO $$
DECLARE definition TEXT; old_count INTEGER; new_count INTEGER;
BEGIN
    definition := pg_get_functiondef('protect_monty_task_admissions()'::regprocedure);
    old_count := (length(definition)-length(replace(definition,'to_jsonb(OLD)','')))
        / length('to_jsonb(OLD)');
    new_count := (length(definition)-length(replace(definition,'to_jsonb(NEW)','')))
        / length('to_jsonb(NEW)');
    IF old_count <> 2 OR new_count <> 2
        OR position('result_only_completion' IN definition) <> 0 THEN
        RAISE EXCEPTION 'Unexpected Monty admission guard; explicit upgrade required'
            USING ERRCODE='23514';
    END IF;
    definition := replace(definition,'to_jsonb(OLD)',
        '(to_jsonb(OLD)-''result_only_completion'')');
    definition := replace(definition,'to_jsonb(NEW)',
        '(to_jsonb(NEW)-''result_only_completion'')');
    EXECUTE definition;
END $$;
