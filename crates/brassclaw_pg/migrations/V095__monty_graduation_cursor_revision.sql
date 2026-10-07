-- Graduation is evidence/bookkeeping, not a resource settings change. V077
-- updates only this cursor; it must not invalidate a settings CAS or fail a
-- second graduation. Compare the entire row except that one known field, so
-- future settings columns automatically remain protected by the revision gate.
CREATE OR REPLACE FUNCTION enforce_monty_settings_revision() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.revision = OLD.revision
       AND (to_jsonb(NEW) - 'last_graduation_at') IS NOT DISTINCT FROM
           (to_jsonb(OLD) - 'last_graduation_at') THEN
        RETURN NEW;
    END IF;
    IF OLD.revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'Monty settings revision exhausted' USING ERRCODE = '23514';
    END IF;
    IF NEW.revision <> OLD.revision + 1 THEN
        RAISE EXCEPTION 'Monty settings update requires the next revision'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
