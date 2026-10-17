-- Introduce the former fixed root limit as an explicit instance setting.
-- Missing values advance the settings revision; existing operator values stay.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"max_pending_host_calls":128}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'max_pending_host_calls');

-- Extend the actual preceding default without repeating its growing field list.
DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a
        ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass
        AND a.attname='execution_limits';
    -- Preserve the parsed prior default without invoking it during migration.
    -- It continues to run once per insert; an explicit prior value wins.
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (%L::jsonb || (%s))',
        '{"max_pending_host_calls":128}', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_pending_host_calls CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_pending_host_calls')='number'
        AND execution_limits->>'max_pending_host_calls' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'max_pending_host_calls')::numeric <= 4294967295
    ELSE false END, false)
);
