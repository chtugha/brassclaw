-- L-01: append immutable original parser provenance to existing /1 revisions.
-- NULL/NULL retains the exact historical profile; never rewrite old content,
-- checksums, subjects, selections, approvals, or task pins.
ALTER TABLE reborn_component_revisions
    ADD COLUMN revision_preparation_bytes TEXT,
    ADD COLUMN revision_preparation_checksum TEXT;
DO $$ DECLARE c record; removed integer := 0;
BEGIN
    FOR c IN SELECT conname FROM pg_constraint
        WHERE conrelid='reborn_component_revisions'::regclass AND contype='c'
          AND pg_get_constraintdef(oid) LIKE '%octet_length(revision_bytes)%'
    LOOP
        EXECUTE format('ALTER TABLE reborn_component_revisions DROP CONSTRAINT %I',c.conname);
        removed := removed + 1;
    END LOOP;
    IF removed <> 1 THEN RAISE EXCEPTION 'unexpected revision length constraints' USING ERRCODE='23514'; END IF;
END $$;
ALTER TABLE reborn_component_revisions ADD CONSTRAINT revision_original_preparation CHECK ((
    CASE WHEN revision_preparation_bytes IS NULL AND revision_preparation_checksum IS NULL THEN
        octet_length(revision_bytes) BETWEEN 1 AND 8388608
    WHEN revision_preparation_bytes IS NOT NULL AND revision_preparation_checksum IS NOT NULL THEN
        brassclaw_checked_preparation_policy_v1(revision_preparation_bytes,revision_preparation_checksum) IS NOT NULL
        AND (brassclaw_checked_preparation_policy_v1(revision_preparation_bytes,revision_preparation_checksum)->>'max_value_depth')::numeric BETWEEN 1 AND 64
        AND octet_length(revision_bytes)::numeric BETWEEN 1 AND
            (brassclaw_checked_preparation_policy_v1(revision_preparation_bytes,revision_preparation_checksum)->>'max_value_bytes')::numeric
    ELSE false END
) IS TRUE);

-- Detached /2 binds the root's actual original row policy in its existing
-- definition checksum. /1 remains historical and may only name a legacy row.
DO $$ DECLARE c record; removed integer := 0;
BEGIN
    FOR c IN SELECT conname FROM pg_constraint
        WHERE conrelid='brassclaw_monty_private_preparations'::regclass AND contype='c'
          AND pg_get_constraintdef(oid) LIKE '%private-work-definition/1%'
    LOOP
        EXECUTE format('ALTER TABLE brassclaw_monty_private_preparations DROP CONSTRAINT %I',c.conname);
        removed := removed + 1;
    END LOOP;
    IF removed <> 1 THEN RAISE EXCEPTION 'unexpected private definition format constraints' USING ERRCODE='23514'; END IF;
END $$;
ALTER TABLE brassclaw_monty_private_preparations ADD CONSTRAINT private_definition_format
    CHECK ((definition_bytes::jsonb->>'format' IN ('private-work-definition/1','private-work-definition/2')) IS TRUE);

ALTER TABLE brassclaw_monty_private_preparations ADD CONSTRAINT private_v2_original_size CHECK ((
    CASE WHEN definition_bytes::jsonb->>'format'='private-work-definition/2' THEN
        brassclaw_checked_preparation_policy_v1(definition_bytes::jsonb->>'preparation_bytes',definition_bytes::jsonb->>'preparation_checksum') IS NOT NULL
        AND octet_length(definition_bytes)::numeric <=
            (brassclaw_checked_preparation_policy_v1(definition_bytes::jsonb->>'preparation_bytes',definition_bytes::jsonb->>'preparation_checksum')->>'max_value_bytes')::numeric
    ELSE true END
) IS TRUE);

CREATE OR REPLACE FUNCTION verify_private_preparation_original() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE envelope jsonb:=NEW.definition_bytes::jsonb; work brassclaw_turns%ROWTYPE; capture jsonb;
BEGIN
    SELECT * INTO STRICT work FROM brassclaw_turns WHERE work_id=NEW.work_id FOR UPDATE;
    IF (envelope ?& ARRAY['format','work_id','run_id','request_checksum','preparation_bytes','preparation_checksum',
            'worker_checksum','root_revision_bytes','root_revision_version','workflow_checksum','input_checksum','domain']
        AND CASE envelope->>'format'
            WHEN 'private-work-definition/1' THEN NOT (envelope ?| ARRAY['root_preparation_bytes','root_preparation_checksum'])
                AND envelope-ARRAY['format','work_id','run_id','request_checksum','preparation_bytes','preparation_checksum',
                    'worker_checksum','root_revision_bytes','root_revision_version','workflow_checksum','input_checksum','domain']='{}'::jsonb
            WHEN 'private-work-definition/2' THEN envelope ?& ARRAY['root_preparation_bytes','root_preparation_checksum']
                AND envelope-ARRAY['format','work_id','run_id','request_checksum','preparation_bytes','preparation_checksum',
                    'worker_checksum','root_revision_bytes','root_revision_version','workflow_checksum','input_checksum','domain',
                    'root_preparation_bytes','root_preparation_checksum']='{}'::jsonb
                AND jsonb_typeof(envelope->'root_preparation_bytes') IN ('string','null')
                AND jsonb_typeof(envelope->'root_preparation_checksum') IN ('string','null')
            ELSE false END
        AND work.record_kind='private_work' AND work.status='awaiting_preparation'
        AND private_work_hash_value(envelope->'worker_checksum')
        AND private_work_hash_value(envelope->'workflow_checksum')
        AND private_work_hash_value(envelope->'input_checksum')
        AND (envelope->>'run_id')::uuid=work.execution_run_id
        AND envelope->'request_checksum'=
            (SELECT jsonb_agg(get_byte(work.private_request_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        AND envelope->>'preparation_checksum'=encode(sha256(convert_to(envelope->>'preparation_bytes','UTF8')),'hex')
        AND (envelope->>'preparation_bytes')::jsonb->>'format'='task-preparation-capture/1'
        AND (envelope->'domain'->>'purpose')=(work.private_request_bytes::jsonb->'original'->>'purpose')) IS NOT TRUE THEN
        RAISE EXCEPTION 'private preparation must retain its original Work' USING ERRCODE='23514';
    END IF;
    IF work.private_request_bytes::jsonb->'original_preparation_checksum'<>'null'::jsonb
        AND work.private_request_bytes::jsonb->'original_preparation_checksum' IS DISTINCT FROM
            (SELECT jsonb_agg(get_byte(decode(envelope->>'preparation_checksum','hex'),n) ORDER BY n) FROM generate_series(0,31)n) THEN
        RAISE EXCEPTION 'private preparation cannot replace captured limits/revision' USING ERRCODE='23514';
    END IF;
    IF work.private_request_bytes::jsonb->'original_workflow_checksum'<>'null'::jsonb
        AND work.private_request_bytes::jsonb->'original_workflow_checksum' IS DISTINCT FROM envelope->'workflow_checksum' THEN
        RAISE EXCEPTION 'private preparation cannot replace a captured workflow' USING ERRCODE='23514';
    END IF;
    IF NOT EXISTS(SELECT 1 FROM reborn_component_revisions r
        WHERE r.component_id=((envelope->>'root_revision_bytes')::jsonb->>'uuid')::uuid
            AND r.version=(envelope->>'root_revision_version')::bigint AND r.class_code=10
            AND r.revision_bytes=envelope->>'root_revision_bytes'
            AND r.checksum=encode(sha256(convert_to(envelope->>'root_revision_bytes','UTF8')),'hex')
            AND CASE envelope->>'format'
                WHEN 'private-work-definition/1' THEN r.revision_preparation_bytes IS NULL AND r.revision_preparation_checksum IS NULL
                WHEN 'private-work-definition/2' THEN
                    (envelope->>'root_preparation_bytes') IS NOT DISTINCT FROM r.revision_preparation_bytes
                    AND (envelope->>'root_preparation_checksum') IS NOT DISTINCT FROM r.revision_preparation_checksum
                ELSE false END) THEN
        RAISE EXCEPTION 'private preparation root is not exact retained bytes' USING ERRCODE='23514';
    END IF;
    IF work.original_pure_case_id IS NOT NULL THEN
        IF (envelope->'domain' ?& ARRAY['purpose','case_id','case_checksum']
            AND (envelope->'domain')-ARRAY['purpose','case_id','case_checksum']='{}'::jsonb
            AND (envelope->'domain'->>'case_id')::uuid=work.original_pure_case_id
            AND EXISTS(SELECT 1 FROM reborn_original_pure_cases c WHERE c.case_id=work.original_pure_case_id
                AND envelope->'domain'->'case_checksum'=
                    (SELECT jsonb_agg(get_byte(c.checksum,n) ORDER BY n) FROM generate_series(0,31)n)
                AND envelope->'worker_checksum'=c.case_bytes::jsonb->'worker_checksum'
                AND envelope->'input_checksum'=
                    (SELECT jsonb_agg(get_byte(sha256(convert_to('{}','UTF8')),n) ORDER BY n) FROM generate_series(0,31)n))) IS NOT TRUE THEN
            RAISE EXCEPTION 'private original Pure case preparation mismatch' USING ERRCODE='23514';
        END IF;
    ELSIF work.prefix_job_id IS NOT NULL THEN
        IF ((envelope->'domain')-'purpose'-'job_id'='{}'::jsonb
            AND (envelope->'domain'->>'job_id')::uuid=work.prefix_job_id
            AND EXISTS(SELECT 1 FROM brassclaw_prefix_job_definitions d WHERE d.job_id=work.prefix_job_id
                AND d.definition_format='prefix-job-definition/2'
                AND envelope->>'preparation_bytes'=d.original_preparation_bytes
                AND envelope->>'preparation_checksum'=d.original_preparation_checksum
                AND envelope->'workflow_checksum'=
                    (SELECT jsonb_agg(get_byte(decode(d.program_checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n)
                AND envelope->'input_checksum'=
                    (SELECT jsonb_agg(get_byte(decode(d.input_checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n))) IS NOT TRUE THEN
            RAISE EXCEPTION 'private prefix preparation mismatch' USING ERRCODE='23514';
        END IF;
    ELSE
        IF (envelope->'domain' ?& ARRAY['purpose','capture_bytes','capture_checksum']
            AND (envelope->'domain')-'purpose'-'capture_bytes'-'capture_checksum'='{}'::jsonb
            AND envelope->'domain'->'capture_checksum'=
                (SELECT jsonb_agg(get_byte(sha256(convert_to(envelope->'domain'->>'capture_bytes','UTF8')),n) ORDER BY n)
                    FROM generate_series(0,31)n)) IS NOT TRUE THEN
            RAISE EXCEPTION 'private validator input capture mismatch' USING ERRCODE='23514';
        END IF;
        capture:=(envelope->'domain'->>'capture_bytes')::jsonb;
        IF (capture ?& ARRAY['format','submission_id','subject_checksum','validator_selection_bytes',
                'validator_selection_checksum','validator_bytes','validator_checksum','input_bytes','input_checksum',
                'preparation_bytes','preparation_checksum','worker_checksum']
            AND (capture-ARRAY['format','submission_id','subject_checksum','validator_selection_bytes',
                'validator_selection_checksum','validator_bytes','validator_checksum','input_bytes','input_checksum',
                'preparation_bytes','preparation_checksum','worker_checksum'])='{}'::jsonb
            AND capture->>'format'='validator-input-capture/1'
            AND (capture->>'submission_id')::uuid=work.validator_submission_id
            AND capture->>'preparation_bytes'=envelope->>'preparation_bytes'
            AND capture->>'preparation_checksum'=envelope->>'preparation_checksum'
            AND envelope->'worker_checksum'=
                (SELECT jsonb_agg(get_byte(decode(capture->>'worker_checksum','hex'),n) ORDER BY n) FROM generate_series(0,31)n)
            AND capture->>'validator_checksum'=encode(sha256(convert_to(capture->>'validator_bytes','UTF8')),'hex')
            AND capture->>'input_checksum'=encode(sha256(convert_to(capture->>'input_bytes','UTF8')),'hex')
            AND envelope->'workflow_checksum'=
                (SELECT jsonb_agg(get_byte(decode(capture->>'validator_checksum','hex'),n) ORDER BY n) FROM generate_series(0,31)n)
            AND envelope->'input_checksum'=
                (SELECT jsonb_agg(get_byte(decode(capture->>'input_checksum','hex'),n) ORDER BY n) FROM generate_series(0,31)n)
            AND EXISTS(SELECT 1 FROM reborn_component_validator_selections v
                JOIN reborn_component_review_submissions subject USING(submission_id)
                WHERE v.submission_id=work.validator_submission_id
                    AND capture->>'subject_checksum'=subject.checksum
                    AND capture->>'validator_selection_bytes'=v.selection_bytes
                    AND capture->>'validator_selection_checksum'=v.checksum
                    AND capture->>'validator_bytes'=v.selection_bytes::jsonb->>'validator_selection_bytes')) IS NOT TRUE THEN
            RAISE EXCEPTION 'private validator capture differs from its domain originals' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
