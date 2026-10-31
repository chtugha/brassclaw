-- Extend the same immutable metadata owner, not a second job queue. Pair checks
-- reference their retained source through exact request metadata; they do not
-- fabricate a role profile or copy provider/model limits.
ALTER TABLE brassclaw_prefix_job_definitions
    ALTER COLUMN profile_name DROP NOT NULL,
    ALTER COLUMN profile_revision DROP NOT NULL,
    DROP CONSTRAINT brassclaw_prefix_job_definitions_operation_check;
ALTER TABLE brassclaw_prefix_job_definitions
    ADD CONSTRAINT brassclaw_prefix_job_definitions_operation_check
    CHECK (operation IN ('generate','redesign','model_design','create','measure','check_cases')),
    ADD CONSTRAINT prefix_job_target_scope CHECK (COALESCE((
        (operation <> 'check_cases' AND profile_name IS NOT NULL AND profile_revision IS NOT NULL)
        OR
        (operation = 'check_cases' AND mode = 'target'
            AND profile_name IS NULL AND profile_revision IS NULL
            AND jsonb_typeof(request_bytes::jsonb) = 'object'
            AND request_bytes::jsonb ?& ARRAY['operation','specification','proposal']
            AND (request_bytes::jsonb - ARRAY['operation','specification','proposal']) = '{}'::jsonb
            AND request_bytes::jsonb->>'operation' = 'check_cases'
            AND jsonb_typeof(request_bytes::jsonb->'specification') = 'string'
            AND jsonb_typeof(request_bytes::jsonb->'proposal') = 'string'
            AND request_bytes::jsonb->>'specification' ~ '^[0-9a-f]{64}$'
            AND request_bytes::jsonb->>'proposal' ~ '^[0-9a-f]{64}$')
    ), FALSE));
