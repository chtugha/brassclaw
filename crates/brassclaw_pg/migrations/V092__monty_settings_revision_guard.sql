-- Reject unversioned writers (including older binaries) rather than letting
-- them change settings without invalidating a caller's expected revision.
-- This is a controlled writer cutover: older writers must be stopped before
-- deploying a binary which applies this migration. Reads remain compatible.
CREATE FUNCTION enforce_monty_settings_revision() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
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

CREATE TRIGGER reborn_monty_vm_settings_revision_guard
    BEFORE UPDATE ON reborn_monty_vm_settings
    FOR EACH ROW EXECUTE FUNCTION enforce_monty_settings_revision();
