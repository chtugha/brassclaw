-- Ownership policy participates in the acknowledged complete settings revision.
-- Preserve explicit controls; only missing fields require a successor revision.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"ownership_check_timeout_millis":2000,"ownership_heartbeat_interval_millis":2000,"max_pending_ownership_checks":8}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ?& ARRAY['ownership_check_timeout_millis','ownership_heartbeat_interval_millis','max_pending_ownership_checks']);

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_retained_attempts":256,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864,"max_actor_requests":8,"max_actor_reserved_bytes":1073741824,"max_actor_control_requests":8,"max_actor_control_reserved_bytes":1073741824,"startup_timeout_millis":30000,"response_timeout_millis":30000,"settings_source_timeout_millis":2000,"settings_uptake_timeout_millis":5000,"ownership_check_timeout_millis":2000,"ownership_heartbeat_interval_millis":2000,"max_pending_ownership_checks":8}'::jsonb,
    ADD CONSTRAINT monty_ownership_controls CHECK (COALESCE(
        CASE WHEN jsonb_typeof(execution_limits->'ownership_check_timeout_millis') = 'number'
            AND jsonb_typeof(execution_limits->'ownership_heartbeat_interval_millis') = 'number'
            AND jsonb_typeof(execution_limits->'max_pending_ownership_checks') = 'number'
            AND execution_limits->>'ownership_check_timeout_millis' ~ '^[1-9][0-9]*$'
            AND execution_limits->>'ownership_heartbeat_interval_millis' ~ '^[1-9][0-9]*$'
            AND execution_limits->>'max_pending_ownership_checks' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'ownership_check_timeout_millis')::numeric <= 18446744073709551615
            AND (execution_limits->>'ownership_heartbeat_interval_millis')::numeric <= 18446744073709551615
            AND (execution_limits->>'max_pending_ownership_checks')::numeric <= 4294967295
        ELSE false END, false));
