-- Shared acknowledged execution-settings owner; preserve existing overrides.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"oauth_dcr_timeout_millis":30000,"oauth_dcr_response_body_bytes":32768}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'oauth_dcr_timeout_millis')
   OR NOT (execution_limits ? 'oauth_dcr_response_body_bytes');
DO $$
DECLARE prior_default TEXT;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT prior_default
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE 'ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT ('
        || quote_literal('{"oauth_dcr_timeout_millis":30000,"oauth_dcr_response_body_bytes":32768}')
        || '::jsonb || (' || prior_default || '))';
END $$;
ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT oauth_dcr_http_limits CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'oauth_dcr_timeout_millis')='number'
        AND execution_limits->>'oauth_dcr_timeout_millis' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'oauth_dcr_timeout_millis')::numeric <= 4294967295 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'oauth_dcr_response_body_bytes')='number'
        AND execution_limits->>'oauth_dcr_response_body_bytes' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'oauth_dcr_response_body_bytes')::numeric <= 9223372036854775807 ELSE false END,false));
