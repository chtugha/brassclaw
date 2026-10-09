-- Logical actor credits cover queued/executing/uncollected IPC in separate lanes.
-- Populate only missing keys; retain explicit values and advance revision once.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"max_actor_requests":8,"max_actor_reserved_bytes":1073741824,"max_actor_control_requests":8,"max_actor_control_reserved_bytes":1073741824}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ?& ARRAY['max_actor_requests','max_actor_reserved_bytes',
    'max_actor_control_requests','max_actor_control_reserved_bytes']);

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_retained_attempts":256,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864,"max_actor_requests":8,"max_actor_reserved_bytes":1073741824,"max_actor_control_requests":8,"max_actor_control_reserved_bytes":1073741824}'::jsonb,
    ADD CONSTRAINT monty_actor_transport_capacity CHECK (
        COALESCE(jsonb_typeof(execution_limits->'max_actor_requests') = 'number'
            AND CASE WHEN execution_limits->>'max_actor_requests' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_actor_requests')::numeric <= 4294967295
                ELSE false END, false)
        AND
        COALESCE(jsonb_typeof(execution_limits->'max_actor_reserved_bytes') = 'number'
            AND CASE WHEN execution_limits->>'max_actor_reserved_bytes' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_actor_reserved_bytes')::numeric <= 18446744073709551615
                ELSE false END, false)
        AND
        COALESCE(jsonb_typeof(execution_limits->'max_actor_control_requests') = 'number'
            AND CASE WHEN execution_limits->>'max_actor_control_requests' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_actor_control_requests')::numeric <= 4294967295
                ELSE false END, false)
        AND
        COALESCE(jsonb_typeof(execution_limits->'max_actor_control_reserved_bytes') = 'number'
            AND CASE WHEN execution_limits->>'max_actor_control_reserved_bytes' ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>'max_actor_control_reserved_bytes')::numeric <= 18446744073709551615
                ELSE false END, false)
    );
