-- Persist the original task's provider/model selection at the same admission
-- transition as its catalogue. Historical selections remain unknown; never
-- backfill from today's settings. This is evidence, never a dispatch grant.
ALTER TABLE brassclaw_monty_task_admissions
    ADD COLUMN task_model_selection_version smallint NOT NULL DEFAULT 0
        CHECK (task_model_selection_version IN (0,1)),
    ADD COLUMN task_model_selection jsonb,
    ADD CONSTRAINT monty_task_model_selection_shape CHECK (
        task_model_selection IS NULL OR (
            task_model_selection_version=1 AND started_at IS NOT NULL
            AND jsonb_typeof(task_model_selection)='object'
            AND task_model_selection ?& ARRAY['format','checksum','selection']
            AND task_model_selection->>'format'='brassclaw-task-provider-binding/1'
            AND jsonb_typeof(task_model_selection->'selection')='object'
            AND task_model_selection->>'checksum' ~ '^[0-9a-f]{64}$') IS TRUE);

CREATE FUNCTION protect_monty_task_model_selection() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' THEN
        IF NEW.task_model_selection IS NOT NULL THEN
            RAISE EXCEPTION 'Monty admissions must begin without model selection'
                USING ERRCODE='23514';
        END IF;
    ELSIF NEW.task_model_selection_version IS DISTINCT FROM OLD.task_model_selection_version
        OR (NEW.task_model_selection IS DISTINCT FROM OLD.task_model_selection
            AND NOT (OLD.phase='reserved' AND NEW.phase='started'
                AND OLD.task_model_selection_version=1 AND OLD.task_model_selection IS NULL)) THEN
        RAISE EXCEPTION 'Original Monty task model selection is immutable'
            USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER monty_task_model_selection_immutable
    BEFORE INSERT OR UPDATE ON brassclaw_monty_task_admissions
    FOR EACH ROW EXECUTE FUNCTION protect_monty_task_model_selection();

-- V174's generic immutable-identity comparison must permit this one new
-- reference field. Its separate trigger above constrains that field's exact
-- transition; every existing claim/catalogue/phase/settlement guard is preserved.
CREATE OR REPLACE FUNCTION protect_monty_task_admissions() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.phase <> 'reserved'
            OR NEW.catalogue_id IS NOT NULL OR NEW.catalogue_checksum IS NOT NULL THEN
            RAISE EXCEPTION 'Monty admissions must begin reserved without a source pin'
                USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'Monty admission evidence cannot be removed' USING ERRCODE='23514';
    END IF;
    IF (to_jsonb(OLD) - ARRAY['phase','started_at','settled_at','outcome',
                              'catalogue_id','catalogue_checksum','task_model_selection']) IS DISTINCT FROM
       (to_jsonb(NEW) - ARRAY['phase','started_at','settled_at','outcome',
                              'catalogue_id','catalogue_checksum','task_model_selection']) THEN
        RAISE EXCEPTION 'Monty admission identity is immutable' USING ERRCODE='23514';
    END IF;
    IF OLD.catalogue_id IS DISTINCT FROM NEW.catalogue_id
        OR OLD.catalogue_checksum IS DISTINCT FROM NEW.catalogue_checksum THEN
        IF OLD.phase <> 'reserved' OR NEW.phase <> 'started'
            OR OLD.catalogue_reference_version <> 1
            OR OLD.catalogue_id IS NOT NULL OR OLD.catalogue_checksum IS NOT NULL
            OR NEW.catalogue_id IS NULL OR NEW.catalogue_checksum IS NULL THEN
            RAISE EXCEPTION 'Monty catalogue reference is immutable after initial start'
                USING ERRCODE='23514';
        END IF;
        PERFORM 1 FROM brassclaw_monty_boot_catalogues
            WHERE catalogue_id=NEW.catalogue_id AND checksum=NEW.catalogue_checksum;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'Monty catalogue reference must name exact retained bytes'
                USING ERRCODE='23514';
        END IF;
    END IF;
    IF to_jsonb(OLD) = to_jsonb(NEW) THEN RETURN NEW; END IF;
    IF OLD.phase='reserved' AND NEW.phase='started' THEN RETURN NEW; END IF;
    IF OLD.phase='reserved' AND NEW.phase='settled'
        AND NEW.started_at IS NOT DISTINCT FROM OLD.started_at
        AND NEW.outcome->>'status'='failed'
        AND (NEW.outcome->>'reason_kind'='task_cancelled' OR NEW.outcome IN (
            '{"status":"failed","reason_kind":"monty_input_rejected"}'::jsonb,
            '{"status":"failed","reason_kind":"monty_admission_backpressure"}'::jsonb,
            '{"status":"failed","reason_kind":"monty_service_closed"}'::jsonb,
            '{"status":"failed","reason_kind":"monty_admission_rejected"}'::jsonb)) THEN
        RETURN NEW;
    END IF;
    IF OLD.phase='started' AND NEW.phase='settled'
        AND NEW.started_at IS NOT DISTINCT FROM OLD.started_at THEN RETURN NEW; END IF;
    RAISE EXCEPTION 'Monty admission settlement is monotonic and immutable' USING ERRCODE='23514';
END;
$$;
