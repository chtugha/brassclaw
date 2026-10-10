-- New exchanges use this policy; retained exchanges keep their accepted bounds.
-- Preserve explicit values and revise only rows which need the default.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"max_ipc_frame_bytes":67108864}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT execution_limits ? 'max_ipc_frame_bytes';

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_queued_tasks":64,"max_queued_bytes":67108864,"max_pending_settings":8,"max_retained_attempts":256,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864,"max_actor_requests":8,"max_actor_reserved_bytes":1073741824,"max_actor_control_requests":8,"max_actor_control_reserved_bytes":1073741824,"startup_timeout_millis":30000,"response_timeout_millis":30000,"settings_source_timeout_millis":2000,"settings_uptake_timeout_millis":5000,"ownership_check_timeout_millis":2000,"ownership_heartbeat_interval_millis":2000,"max_pending_ownership_checks":8,"cancellation_ack_timeout_millis":5000,"settings_reconcile_interval_millis":1000,"status_poll_interval_millis":3000,"worker_adapter_reserve_bytes":4194304,"max_ipc_frame_bytes":67108864}'::jsonb,
    ADD CONSTRAINT monty_ipc_frame_policy CHECK (COALESCE(
        CASE WHEN jsonb_typeof(execution_limits->'max_ipc_frame_bytes') = 'number'
            AND execution_limits->>'max_ipc_frame_bytes' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'max_ipc_frame_bytes')::numeric <= 4294967295
        ELSE false END, false));
