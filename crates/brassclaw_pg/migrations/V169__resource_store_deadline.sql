-- Preserve operator values and advance each incomplete row exactly once.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"resource_store_timeout_millis":2000}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'resource_store_timeout_millis');

-- Extend the existing default without reverting other resource controls.
DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (''{"resource_store_timeout_millis":2000}''::jsonb || (%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_resource_store_deadline CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'resource_store_timeout_millis')='number'
        AND execution_limits->>'resource_store_timeout_millis' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'resource_store_timeout_millis')::numeric <= 18446744073709551615
    ELSE false END, false)
);
