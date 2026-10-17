-- The cancellation index contains only active HTTP waiters. Its independent
-- transport capacity is live; completed history belongs to durable correlation.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"max_mcp_inflight_calls":4096}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'max_mcp_inflight_calls');

DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a
        ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass
        AND a.attname='execution_limits';
    -- The previous expression occurs once, including volatile operator defaults.
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (''{"max_mcp_inflight_calls":4096}''::jsonb || (%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_mcp_inflight_capacity CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_mcp_inflight_calls')='number'
        AND execution_limits->>'max_mcp_inflight_calls' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'max_mcp_inflight_calls')::numeric <= 4294967295
    ELSE false END, false)
);
