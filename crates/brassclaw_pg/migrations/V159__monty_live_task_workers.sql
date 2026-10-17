-- Add the former two-task startup default as a live instance policy. One
-- coordinator needs its own pending host-call slot. Preserve larger limits;
-- raise smaller pending-call settings only to the required technical minimum.
UPDATE reborn_monty_vm_settings
SET execution_limits = jsonb_set(
        '{"max_workers":2}'::jsonb || execution_limits,
        '{max_pending_host_calls}',
        to_jsonb(GREATEST((execution_limits->>'max_pending_host_calls')::bigint,
            COALESCE((execution_limits->>'max_workers')::bigint, 2) + 1))),
    revision = revision + 1
WHERE NOT (execution_limits ? 'max_workers')
    OR (execution_limits->>'max_pending_host_calls')::numeric
        <= (execution_limits->>'max_workers')::numeric;

-- Evaluate the preceding default exactly once per insert, preserving any
-- operator-supplied expression while making its pool dependency consistent.
CREATE FUNCTION brassclaw_monty_pool_default(previous jsonb) RETURNS jsonb
LANGUAGE SQL IMMUTABLE STRICT
RETURN jsonb_set('{"max_workers":2}'::jsonb || previous,
    '{max_pending_host_calls}',
    to_jsonb(GREATEST((previous->>'max_pending_host_calls')::bigint,
        COALESCE((previous->>'max_workers')::bigint, 2) + 1)));

DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a
        ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass
        AND a.attname='execution_limits';
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT brassclaw_monty_pool_default((%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_live_task_workers CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_workers')='number'
        AND execution_limits->>'max_workers' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'max_workers')::numeric < 4294967295
        AND (execution_limits->>'max_pending_host_calls')::numeric
            > (execution_limits->>'max_workers')::numeric
    ELSE false END, false)
);
