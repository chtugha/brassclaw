-- The default is a reservation estimate, never a provider output cap.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"model_quote_output_tokens":8192}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT (execution_limits ? 'model_quote_output_tokens');

DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (''{"model_quote_output_tokens":8192}''::jsonb || (%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_model_quote_output CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'model_quote_output_tokens')='number'
        AND execution_limits->>'model_quote_output_tokens' ~ '^[1-9][0-9]*$'
    THEN (execution_limits->>'model_quote_output_tokens')::numeric <= 4294967295
    ELSE false END, false)
);
