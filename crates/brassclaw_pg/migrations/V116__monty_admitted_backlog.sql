-- Materialize absent legacy queue settings without changing explicit operator
-- values. One complete settings generation covers count and payload-byte policy.
UPDATE reborn_monty_vm_settings
SET execution_limits = execution_limits
    || CASE WHEN execution_limits ? 'max_queued_tasks' THEN '{}'::jsonb
            ELSE '{"max_queued_tasks":64}'::jsonb END
    || CASE WHEN execution_limits ? 'max_queued_bytes' THEN '{}'::jsonb
            ELSE '{"max_queued_bytes":67108864}'::jsonb END,
    revision = revision + 1
WHERE NOT execution_limits ? 'max_queued_tasks'
   OR NOT execution_limits ? 'max_queued_bytes';

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864}'::jsonb,
    ADD CONSTRAINT monty_admitted_backlog CHECK (
        COALESCE(jsonb_typeof(execution_limits->'max_queued_tasks') = 'number'
            AND CASE WHEN execution_limits->>'max_queued_tasks' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_queued_tasks')::numeric <= 4294967295
                ELSE false END, false)
        AND COALESCE(jsonb_typeof(execution_limits->'max_queued_bytes') = 'number'
            AND CASE WHEN execution_limits->>'max_queued_bytes' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_queued_bytes')::numeric <= 18446744073709551615
                ELSE false END, false)
    );
