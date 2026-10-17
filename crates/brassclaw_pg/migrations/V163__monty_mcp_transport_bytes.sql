-- Capture the acknowledged limits per HTTP operation. Preserve explicit values,
-- and increment an existing row once even when both controls were absent.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"max_mcp_request_bytes":1048576,"max_mcp_response_bytes":1048576}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'max_mcp_request_bytes')
   OR NOT (execution_limits ? 'max_mcp_response_bytes');

DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a
        ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass
        AND a.attname='execution_limits';
    -- Keep a volatile operator default to one evaluation per inserted row.
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (''{"max_mcp_request_bytes":1048576,"max_mcp_response_bytes":1048576}''::jsonb || (%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_mcp_transport_bytes CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_mcp_request_bytes')='number'
        AND execution_limits->>'max_mcp_request_bytes' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'max_mcp_request_bytes')::numeric <= 4294967295
    ELSE false END, false)
    AND
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_mcp_response_bytes')='number'
        AND execution_limits->>'max_mcp_response_bytes' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'max_mcp_response_bytes')::numeric <= 4294967295
    ELSE false END, false)
);
