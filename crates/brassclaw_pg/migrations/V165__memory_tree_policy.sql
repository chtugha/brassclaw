-- Preserve explicit defaults/ceilings and advance incomplete rows only once.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"default_memory_tree_depth":1,"max_memory_tree_depth":10}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'default_memory_tree_depth')
   OR NOT (execution_limits ? 'max_memory_tree_depth');

DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (''{"default_memory_tree_depth":1,"max_memory_tree_depth":10}''::jsonb || (%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_memory_tree_depth CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'default_memory_tree_depth')='number'
        AND execution_limits->>'default_memory_tree_depth' ~ '^[1-9][0-9]*$'
        AND jsonb_typeof(execution_limits->'max_memory_tree_depth')='number'
        AND execution_limits->>'max_memory_tree_depth' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'default_memory_tree_depth')::numeric <= (execution_limits->>'max_memory_tree_depth')::numeric
        AND (execution_limits->>'max_memory_tree_depth')::numeric <= 4294967295
    ELSE false END, false)
);
