-- Preserve existing values and introduce the previously implicit startup limits.
-- A catalogue/settings generation changes once so stale UI writers fail CAS.
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN execution_limits JSONB NOT NULL DEFAULT
    '{"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864}'::jsonb,
    ADD CONSTRAINT reborn_monty_execution_limits_object
    CHECK (jsonb_typeof(execution_limits) = 'object');
UPDATE reborn_monty_vm_settings SET revision = revision + 1;
