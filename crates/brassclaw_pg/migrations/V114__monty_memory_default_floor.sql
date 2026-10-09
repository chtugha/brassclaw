-- The operator-selected startup/manual floor is the existing 512 MiB default.
-- This is an allocation ceiling, not eagerly allocated memory. Increase only
-- smaller configured values; retain modes and all unrelated settings. Revision
-- overflow aborts the upgrade instead of resetting an exhausted generation.
UPDATE reborn_monty_vm_settings
SET max_memory_bytes = 536870912, revision = revision + 1
WHERE max_memory_bytes < 536870912;

ALTER TABLE reborn_monty_vm_settings
    ADD CONSTRAINT monty_memory_default_floor
    CHECK (max_memory_bytes >= 536870912);
