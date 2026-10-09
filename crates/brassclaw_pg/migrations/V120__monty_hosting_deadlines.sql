-- Startup is a future supervised boot bound; IPC applies to new exchanges live.
-- Preserve explicit values and increment a partially migrated row only once.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"startup_timeout_millis":30000,"response_timeout_millis":30000}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ?& ARRAY['startup_timeout_millis','response_timeout_millis']);

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_retained_attempts":256,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864,"max_actor_requests":8,"max_actor_reserved_bytes":1073741824,"max_actor_control_requests":8,"max_actor_control_reserved_bytes":1073741824,"startup_timeout_millis":30000,"response_timeout_millis":30000}'::jsonb,
    ADD CONSTRAINT monty_hosting_deadlines CHECK (COALESCE(
        CASE WHEN jsonb_typeof(execution_limits->'startup_timeout_millis') = 'number'
            AND jsonb_typeof(execution_limits->'response_timeout_millis') = 'number'
            AND jsonb_typeof(execution_limits->'execution_slice_millis') = 'number'
            AND execution_limits->>'startup_timeout_millis' ~ '^[1-9][0-9]*$'
            AND execution_limits->>'response_timeout_millis' ~ '^[1-9][0-9]*$'
            AND execution_limits->>'execution_slice_millis' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'startup_timeout_millis')::numeric <= 18446744073709551615
            AND (execution_limits->>'response_timeout_millis')::numeric <= 18446744073709551615
            AND (execution_limits->>'response_timeout_millis')::numeric >=
                (execution_limits->>'startup_timeout_millis')::numeric
            AND (execution_limits->>'execution_slice_millis')::numeric <
                (execution_limits->>'response_timeout_millis')::numeric
        ELSE false END, false));
