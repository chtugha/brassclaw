-- Bound each protected ProductAuth backend call independently of issuer HTTP.
-- Preserve explicit instance values and all unrelated INSERT defaults.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"auth_backend_timeout_millis":30000}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'auth_backend_timeout_millis');
DO $$
DECLARE prior_default TEXT;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT prior_default
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE 'ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT ('
        || quote_literal('{"auth_backend_timeout_millis":30000}')
        || '::jsonb || (' || prior_default || '))';
END $$;
ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT product_auth_backend_timeout_limits CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'auth_backend_timeout_millis')='number'
        AND execution_limits->>'auth_backend_timeout_millis' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'auth_backend_timeout_millis')::numeric <= 4294967295
        ELSE false END,false));
