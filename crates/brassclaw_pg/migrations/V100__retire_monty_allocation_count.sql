-- Monty 1.0 removed allocation-count limits. Preserve every stored value,
-- including explicit operator edits and exhausted revisions, as read-only
-- historical data. Rename only: no data rewrite or settings generation reset.
-- Restore the matching pre-upgrade database snapshot when rolling back binaries;
-- an old writer must fail rather than silently mutate an unenforced setting.
ALTER TABLE reborn_monty_vm_settings RENAME COLUMN max_allocations TO retired_max_allocations;
ALTER TABLE reborn_monty_vm_settings ALTER COLUMN retired_max_allocations DROP DEFAULT;
ALTER TABLE reborn_monty_vm_settings ALTER COLUMN retired_max_allocations DROP NOT NULL;

COMMENT ON COLUMN reborn_monty_vm_settings.retired_max_allocations IS
'Historical allocation-count limit, retired by Monty 1.0; not an active limit and never interpreted as bytes.';

CREATE FUNCTION enforce_retired_monty_allocation_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.retired_max_allocations IS NOT NULL THEN
            RAISE EXCEPTION 'Monty allocation-count limits are retired' USING ERRCODE = '23514';
        END IF;
    ELSIF NEW.retired_max_allocations IS DISTINCT FROM OLD.retired_max_allocations THEN
        RAISE EXCEPTION 'Retired Monty allocation-count evidence is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER reborn_monty_retired_allocations_immutable
BEFORE INSERT OR UPDATE ON reborn_monty_vm_settings
FOR EACH ROW EXECUTE FUNCTION enforce_retired_monty_allocation_immutable();
