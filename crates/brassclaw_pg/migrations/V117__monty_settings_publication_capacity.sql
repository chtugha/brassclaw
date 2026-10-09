-- Configure the control lane separately from work backlog. Retain explicit
-- values and fail transactionally on invalid data or exhausted revisions.
UPDATE reborn_monty_vm_settings
SET execution_limits = execution_limits || '{"max_pending_settings":8}'::jsonb,
    revision = revision + 1
WHERE NOT execution_limits ? 'max_pending_settings';

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864}'::jsonb,
    ADD CONSTRAINT monty_settings_publication_capacity CHECK (
        COALESCE(jsonb_typeof(execution_limits->'max_pending_settings') = 'number'
            AND CASE WHEN execution_limits->>'max_pending_settings' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_pending_settings')::numeric <= 4294967295
                ELSE false END, false)
    );
