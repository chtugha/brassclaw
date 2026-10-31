-- Derive only missing standalone OAuth lifetime partners. Preserve accepted
-- originals, valid explicit settings and the existing column-default expression.
DO $$
DECLARE key_name TEXT;
BEGIN
    FOREACH key_name IN ARRAY ARRAY['oauth_setup_default_ttl_secs','oauth_setup_max_ttl_secs'] LOOP
        IF EXISTS (SELECT 1 FROM reborn_monty_vm_settings WHERE execution_limits ? key_name AND NOT
            COALESCE(CASE WHEN jsonb_typeof(execution_limits->key_name)='number'
                AND execution_limits->>key_name ~ '^[1-9][0-9]*$'
                THEN (execution_limits->>key_name)::numeric <= 4294967295
                ELSE false END,false)) THEN
            RAISE EXCEPTION 'invalid explicit standalone OAuth lifetime: %', key_name;
        END IF;
    END LOOP;
    IF EXISTS (SELECT 1 FROM reborn_monty_vm_settings
        WHERE execution_limits ? 'oauth_setup_default_ttl_secs'
          AND execution_limits ? 'oauth_setup_max_ttl_secs'
          AND (execution_limits->>'oauth_setup_default_ttl_secs')::numeric
            > (execution_limits->>'oauth_setup_max_ttl_secs')::numeric) THEN
        RAISE EXCEPTION 'explicit standalone OAuth default exceeds maximum';
    END IF;
END $$;

UPDATE reborn_monty_vm_settings
SET execution_limits = jsonb_build_object(
        'oauth_setup_default_ttl_secs', LEAST(600::bigint,
            COALESCE((execution_limits->>'oauth_setup_max_ttl_secs')::bigint,600::bigint)),
        'oauth_setup_max_ttl_secs', GREATEST(600::bigint,
            COALESCE((execution_limits->>'oauth_setup_default_ttl_secs')::bigint,600::bigint)))
        || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'oauth_setup_default_ttl_secs'
    AND execution_limits ? 'oauth_setup_max_ttl_secs');

DO $$
DECLARE prior_default TEXT; prior_value JSONB; key_name TEXT;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT prior_default
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE 'SELECT (' || prior_default || ')' INTO prior_value;
    FOREACH key_name IN ARRAY ARRAY['oauth_setup_default_ttl_secs','oauth_setup_max_ttl_secs'] LOOP
        IF prior_value ? key_name AND NOT COALESCE(CASE
            WHEN jsonb_typeof(prior_value->key_name)='number'
              AND prior_value->>key_name ~ '^[1-9][0-9]*$'
            THEN (prior_value->>key_name)::numeric <= 4294967295
            ELSE false END,false) THEN
            RAISE EXCEPTION 'invalid standalone OAuth column default: %', key_name;
        END IF;
    END LOOP;
    IF prior_value ? 'oauth_setup_default_ttl_secs' AND prior_value ? 'oauth_setup_max_ttl_secs'
        AND (prior_value->>'oauth_setup_default_ttl_secs')::numeric
          > (prior_value->>'oauth_setup_max_ttl_secs')::numeric THEN
        RAISE EXCEPTION 'standalone OAuth column default exceeds maximum';
    END IF;
    -- Preserve the existing expression and unrelated/custom fields, deriving
    -- only a missing partner around an explicitly chosen lifetime.
    EXECUTE 'ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT ('
        || 'jsonb_build_object(''oauth_setup_default_ttl_secs'', LEAST(600::bigint, COALESCE((('
        || prior_default || ')->>''oauth_setup_max_ttl_secs'')::bigint,600::bigint)), '
        || '''oauth_setup_max_ttl_secs'', GREATEST(600::bigint, COALESCE((('
        || prior_default || ')->>''oauth_setup_default_ttl_secs'')::bigint,600::bigint))) || ('
        || prior_default || '))';
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT standalone_oauth_setup_lifetimes CHECK (
    COALESCE(CASE WHEN
        jsonb_typeof(execution_limits->'oauth_setup_default_ttl_secs')='number'
        AND execution_limits->>'oauth_setup_default_ttl_secs' ~ '^[1-9][0-9]*$'
        AND jsonb_typeof(execution_limits->'oauth_setup_max_ttl_secs')='number'
        AND execution_limits->>'oauth_setup_max_ttl_secs' ~ '^[1-9][0-9]*$'
        THEN (execution_limits->>'oauth_setup_default_ttl_secs')::numeric <= 4294967295
          AND (execution_limits->>'oauth_setup_max_ttl_secs')::numeric <= 4294967295
          AND (execution_limits->>'oauth_setup_default_ttl_secs')::numeric
            <= (execution_limits->>'oauth_setup_max_ttl_secs')::numeric
        ELSE false END,false));
