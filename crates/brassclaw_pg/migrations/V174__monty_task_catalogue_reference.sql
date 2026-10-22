-- Keep the actual ordinary task's captured catalogue, including No-Match,
-- before its first admitted host operation. Existing/internal records stay
-- unknown (NULL); never backfill from the current active generation.
ALTER TABLE brassclaw_monty_task_admissions
    -- Format zero identifies records whose original source is unknown. It is
    -- immutable, not an approval flag. A recovered legacy reservation cannot
    -- acquire today's catalogue merely because it has not reached started.
    ADD COLUMN catalogue_reference_version smallint NOT NULL DEFAULT 0
        CHECK (catalogue_reference_version IN (0,1)),
    ADD COLUMN catalogue_id uuid REFERENCES brassclaw_monty_boot_catalogues(catalogue_id),
    ADD COLUMN catalogue_checksum text,
    ADD CONSTRAINT brassclaw_monty_admission_catalogue_pair CHECK (
        (catalogue_id IS NULL) = (catalogue_checksum IS NULL)
        AND (catalogue_id IS NULL OR (
            catalogue_id <> '00000000-0000-0000-0000-000000000000'::uuid
            AND catalogue_reference_version=1
            AND catalogue_checksum ~ '^[0-9a-f]{64}$'
            AND started_at IS NOT NULL)));

-- Preserve the full V162 no-replay/settlement contract. The only new identity
-- transition is a verified source pin in the original reserved -> started write.
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
                              'catalogue_id','catalogue_checksum']) IS DISTINCT FROM
       (to_jsonb(NEW) - ARRAY['phase','started_at','settled_at','outcome',
                              'catalogue_id','catalogue_checksum']) THEN
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
