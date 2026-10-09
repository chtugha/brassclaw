-- Cancellation waiters capture the complete acknowledged policy at request time.
-- Preserve explicit operator values; timeout is not settlement or effect recovery.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"cancellation_ack_timeout_millis":5000}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT execution_limits ? 'cancellation_ack_timeout_millis';

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_retained_attempts":256,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864,"max_actor_requests":8,"max_actor_reserved_bytes":1073741824,"max_actor_control_requests":8,"max_actor_control_reserved_bytes":1073741824,"startup_timeout_millis":30000,"response_timeout_millis":30000,"settings_source_timeout_millis":2000,"settings_uptake_timeout_millis":5000,"ownership_check_timeout_millis":2000,"ownership_heartbeat_interval_millis":2000,"max_pending_ownership_checks":8,"cancellation_ack_timeout_millis":5000}'::jsonb,
    ADD CONSTRAINT monty_cancellation_ack_deadline CHECK (COALESCE(
        CASE WHEN jsonb_typeof(execution_limits->'cancellation_ack_timeout_millis') = 'number'
            AND execution_limits->>'cancellation_ack_timeout_millis' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'cancellation_ack_timeout_millis')::numeric <= 18446744073709551615
        ELSE false END, false));
