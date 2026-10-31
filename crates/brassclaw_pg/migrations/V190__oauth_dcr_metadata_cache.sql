-- Separate metadata reuse from accepted Auth-flow expiry. Preserve overrides.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"oauth_dcr_metadata_cache_ttl_secs":600}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'oauth_dcr_metadata_cache_ttl_secs');
DO $$
DECLARE prior_default TEXT;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT prior_default
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE 'ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT ('
        || quote_literal('{"oauth_dcr_metadata_cache_ttl_secs":600}')
        || '::jsonb || (' || prior_default || '))';
END $$;
ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT oauth_dcr_metadata_cache_limits CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'oauth_dcr_metadata_cache_ttl_secs')='number'
        AND execution_limits->>'oauth_dcr_metadata_cache_ttl_secs' ~ '^(0|[1-9][0-9]*)$'
        THEN (execution_limits->>'oauth_dcr_metadata_cache_ttl_secs')::numeric <= 4294967295
        ELSE false END,false));
