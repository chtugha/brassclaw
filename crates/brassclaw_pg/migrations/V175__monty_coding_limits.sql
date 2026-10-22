-- One acknowledged execution-policy owner for coding primitives. Old running
-- catalogue manifests retain their original preparation-policy representation.
UPDATE reborn_monty_vm_settings
SET execution_limits = '{"max_coding_read_bytes":10485760,"max_coding_write_bytes":5242880,"max_coding_patch_bytes":10485760,"default_coding_read_lines":2000,"max_coding_directory_entries":500,"default_coding_directory_depth":3,"default_coding_max_results":200,"max_coding_output_bytes":65536,"default_coding_head_lines":250,"max_coding_scan_bytes":67108864,"max_coding_visited_entries":50000,"max_coding_diff_input_bytes":262144,"max_coding_diff_preview_bytes":16384,"coding_diff_context_lines":3}'::jsonb || execution_limits,
    revision = revision + 1
WHERE NOT execution_limits ?& ARRAY['max_coding_read_bytes','max_coding_write_bytes','max_coding_patch_bytes','default_coding_read_lines','max_coding_directory_entries','default_coding_directory_depth','default_coding_max_results','max_coding_output_bytes','default_coding_head_lines','max_coding_scan_bytes','max_coding_visited_entries','max_coding_diff_input_bytes','max_coding_diff_preview_bytes','coding_diff_context_lines'];

DO $$
DECLARE expression text;
BEGIN
    SELECT pg_get_expr(d.adbin,d.adrelid) INTO STRICT expression
    FROM pg_attrdef d JOIN pg_attribute a ON a.attrelid=d.adrelid AND a.attnum=d.adnum
    WHERE d.adrelid='reborn_monty_vm_settings'::regclass AND a.attname='execution_limits';
    EXECUTE format('ALTER TABLE reborn_monty_vm_settings ALTER COLUMN execution_limits SET DEFAULT (''{"max_coding_read_bytes":10485760,"max_coding_write_bytes":5242880,"max_coding_patch_bytes":10485760,"default_coding_read_lines":2000,"max_coding_directory_entries":500,"default_coding_directory_depth":3,"default_coding_max_results":200,"max_coding_output_bytes":65536,"default_coding_head_lines":250,"max_coding_scan_bytes":67108864,"max_coding_visited_entries":50000,"max_coding_diff_input_bytes":262144,"max_coding_diff_preview_bytes":16384,"coding_diff_context_lines":3}''::jsonb || (%s))', expression);
END $$;

ALTER TABLE reborn_monty_vm_settings ADD CONSTRAINT monty_coding_limits CHECK (
    COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_read_bytes')='number' AND execution_limits->>'max_coding_read_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_read_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_write_bytes')='number' AND execution_limits->>'max_coding_write_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_write_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_patch_bytes')='number' AND execution_limits->>'max_coding_patch_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_patch_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'default_coding_read_lines')='number' AND execution_limits->>'default_coding_read_lines' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'default_coding_read_lines')::numeric BETWEEN 0 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_directory_entries')='number' AND execution_limits->>'max_coding_directory_entries' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_directory_entries')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'default_coding_directory_depth')='number' AND execution_limits->>'default_coding_directory_depth' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'default_coding_directory_depth')::numeric BETWEEN 0 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'default_coding_max_results')='number' AND execution_limits->>'default_coding_max_results' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'default_coding_max_results')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_output_bytes')='number' AND execution_limits->>'max_coding_output_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_output_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'default_coding_head_lines')='number' AND execution_limits->>'default_coding_head_lines' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'default_coding_head_lines')::numeric BETWEEN 0 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_scan_bytes')='number' AND execution_limits->>'max_coding_scan_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_scan_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_visited_entries')='number' AND execution_limits->>'max_coding_visited_entries' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_visited_entries')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_diff_input_bytes')='number' AND execution_limits->>'max_coding_diff_input_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_diff_input_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'max_coding_diff_preview_bytes')='number' AND execution_limits->>'max_coding_diff_preview_bytes' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'max_coding_diff_preview_bytes')::numeric BETWEEN 1 AND 9223372036854775807 ELSE false END,false)
    AND COALESCE(CASE WHEN jsonb_typeof(execution_limits->'coding_diff_context_lines')='number' AND execution_limits->>'coding_diff_context_lines' ~ '^(0|[1-9][0-9]*)$' THEN (execution_limits->>'coding_diff_context_lines')::numeric BETWEEN 0 AND 9223372036854775807 ELSE false END,false)
);
