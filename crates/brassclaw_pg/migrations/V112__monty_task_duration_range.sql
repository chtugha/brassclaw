-- Remove the historical operational duration range without changing any
-- operator value or settings generation. Positive whole seconds are bounded
-- only by the existing PostgreSQL INT representation; default remains 600.
ALTER TABLE reborn_monty_vm_settings
    DROP CONSTRAINT reborn_monty_vm_settings_max_duration_secs_check;

ALTER TABLE reborn_monty_vm_settings
    ADD CONSTRAINT reborn_monty_vm_settings_max_duration_secs_check
    CHECK (max_duration_secs > 0);
