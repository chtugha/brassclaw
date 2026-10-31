-- New manual-token setups use live instance defaults. Retained originals keep
-- their first expiry; this upgrade never edits Auth flows or interactions.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"manual_token_setup_ttl_secs":600,"extension_manual_token_setup_ttl_secs":300}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ?& ARRAY['manual_token_setup_ttl_secs','extension_manual_token_setup_ttl_secs']);
DO $$
DECLARE prior_default TEXT;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT prior_default
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE 'ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT ('
        || quote_literal('{"manual_token_setup_ttl_secs":600,"extension_manual_token_setup_ttl_secs":300}')
        || '::jsonb || (' || prior_default || '))';
END $$;
ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT manual_token_setup_lifetime_limits CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'manual_token_setup_ttl_secs')='number'
        AND execution_limits->>'manual_token_setup_ttl_secs' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'manual_token_setup_ttl_secs')::numeric <= 4294967295
        ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'extension_manual_token_setup_ttl_secs')='number'
        AND execution_limits->>'extension_manual_token_setup_ttl_secs' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'extension_manual_token_setup_ttl_secs')::numeric <= 4294967295
        ELSE false END,false));
