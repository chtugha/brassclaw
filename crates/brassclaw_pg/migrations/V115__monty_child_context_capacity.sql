-- Materialize the historical child-context default in the existing settings
-- generation. Preserve explicit values and fail on exhausted revision arithmetic.
UPDATE reborn_monty_vm_settings
SET execution_limits = execution_limits || '{"max_recipe_contexts":64}'::jsonb,
    revision = revision + 1
WHERE NOT execution_limits ? 'max_recipe_contexts';

ALTER TABLE reborn_monty_vm_settings
    ALTER COLUMN execution_limits SET DEFAULT
    '{"max_recipe_contexts":64,"max_source_bytes":1048576,"max_compiled_source_bytes":2097152,"max_feeds":128,"max_stdout_bytes":1048576,"execution_slice_millis":5,"max_value_depth":48,"max_value_nodes":1048576,"max_value_bytes":67108864}'::jsonb,
    ADD CONSTRAINT monty_child_context_capacity CHECK (
        jsonb_typeof(execution_limits->'max_recipe_contexts') = 'number'
        AND CASE
            WHEN execution_limits->>'max_recipe_contexts' ~ '^[1-9][0-9]*$'
            THEN (execution_limits->>'max_recipe_contexts')::numeric <= 4294967295
            ELSE false
        END
    );
