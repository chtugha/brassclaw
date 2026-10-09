-- One instance retention capacity shared by the admission factory and driver.
-- Preserve explicit values; fail atomically on invalid data/revision exhaustion.
UPDATE reborn_monty_vm_settings
SET execution_limits = execution_limits || '{"max_retained_attempts":256}'::jsonb,
    revision = revision + 1
WHERE NOT execution_limits ? 'max_retained_attempts';

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_retained_attempts":256,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864}'::jsonb,
    ADD CONSTRAINT monty_attempt_retention_capacity CHECK (
        COALESCE(jsonb_typeof(execution_limits->'max_retained_attempts') = 'number'
            AND CASE WHEN execution_limits->>'max_retained_attempts' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_retained_attempts')::numeric <= 4294967295
                ELSE false END, false)
    );
