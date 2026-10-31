-- Original provider bootstrap link and captured input limits in existing owners.
-- No historical link backfill and no job-to-mutation FK cycle.
ALTER TABLE brassclaw_prefix_job_definitions
    DROP CONSTRAINT brassclaw_prefix_job_definitions_operation_check,
    DROP CONSTRAINT prefix_job_target_scope;
ALTER TABLE brassclaw_prefix_job_definitions
    ADD CONSTRAINT brassclaw_prefix_job_definitions_operation_check
    CHECK (operation IN ('generate','redesign','model_design','create','measure','check_cases','bootstrap_setup')),
    ADD CONSTRAINT prefix_job_target_scope CHECK (COALESCE((
        (operation NOT IN ('check_cases','bootstrap_setup') AND profile_name IS NOT NULL AND profile_revision IS NOT NULL)
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
        OR
        (operation = 'bootstrap_setup' AND mode = 'target'
            AND profile_name IS NULL AND profile_revision IS NULL
            AND jsonb_typeof(request_bytes::jsonb) = 'object'
            AND request_bytes::jsonb ?& ARRAY['operation','origin_mutation','specification','setup_revision','executing_specification']
            AND (request_bytes::jsonb - ARRAY['operation','origin_mutation','specification','setup_revision','executing_specification']) = '{}'::jsonb
            AND request_bytes::jsonb->>'operation' = 'bootstrap_setup'
            AND jsonb_typeof(request_bytes::jsonb->'origin_mutation') = 'string'
            AND request_bytes::jsonb->>'origin_mutation' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
            AND request_bytes::jsonb->>'origin_mutation' <> '00000000-0000-0000-0000-000000000000'
            AND jsonb_typeof(request_bytes::jsonb->'specification') = 'string'
            AND request_bytes::jsonb->>'specification' ~ '^[0-9a-f]{64}$'
            AND jsonb_typeof(request_bytes::jsonb->'executing_specification') = 'string'
            AND request_bytes::jsonb->>'executing_specification' ~ '^[0-9a-f]{64}$'
            -- Same exact integer domain as the existing setup owner, not a token cap.
            AND jsonb_typeof(request_bytes::jsonb->'setup_revision') = 'number'
            AND request_bytes::jsonb->>'setup_revision' ~ '^(0|[1-9][0-9]*)$'
            AND (request_bytes::jsonb->>'setup_revision')::numeric BETWEEN 0 AND 9007199254740991)

    ), FALSE));

ALTER TABLE brassclaw_llm_provider_mutations
    ADD COLUMN bootstrap_job_id uuid REFERENCES brassclaw_prefix_job_definitions(job_id) ON DELETE RESTRICT;
CREATE UNIQUE INDEX brassclaw_provider_mutation_bootstrap_job
    ON brassclaw_llm_provider_mutations(bootstrap_job_id) WHERE bootstrap_job_id IS NOT NULL;

-- Persist the original executing-data codec boundary in the SAME immutable job
-- owner. SQL NULL is only the frozen V167 legacy interpretation; native new
-- Reservation always supplies a validated object. No historical row is updated.
ALTER TABLE brassclaw_prefix_job_definitions
    ADD COLUMN original_input_limits jsonb,
    DROP CONSTRAINT brassclaw_prefix_job_definitions_input_bytes_check;
ALTER TABLE brassclaw_prefix_job_definitions
    ADD CONSTRAINT prefix_job_original_input_limits CHECK (
        original_input_limits IS NULL OR COALESCE((
            jsonb_typeof(original_input_limits) = 'object'
            AND original_input_limits ?& ARRAY['max_depth','max_nodes','max_bytes']
            AND (original_input_limits - ARRAY['max_depth','max_nodes','max_bytes']) = '{}'::jsonb
            AND jsonb_typeof(original_input_limits->'max_depth') = 'number'
            AND jsonb_typeof(original_input_limits->'max_nodes') = 'number'
            AND jsonb_typeof(original_input_limits->'max_bytes') = 'number'
            AND original_input_limits->>'max_depth' ~ '^[1-9][0-9]*$'
            AND original_input_limits->>'max_nodes' ~ '^[1-9][0-9]*$'
            AND original_input_limits->>'max_bytes' ~ '^[1-9][0-9]*$'
            -- Existing MontyExecutionLimits field representations, not quotas.
            AND (original_input_limits->>'max_depth')::numeric <= 4294967295
            AND (original_input_limits->>'max_nodes')::numeric <= 18446744073709551615
            AND (original_input_limits->>'max_bytes')::numeric <= 18446744073709551615
        ), FALSE)
    ),
    ADD CONSTRAINT prefix_job_original_input_bytes CHECK (COALESCE((
        octet_length(input_bytes) >= 2 AND octet_length(input_bytes)::numeric <=
        CASE WHEN original_input_limits IS NULL THEN 67108864::numeric
             ELSE (original_input_limits->>'max_bytes')::numeric END
    ), FALSE));
