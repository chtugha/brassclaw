-- One authored original Pure case, with separate native execution, provisional
-- observation and post-Factory delivery links. No Q1/Q2/approval or publication.
-- V207-210 retain their original bytes and meaning.
ALTER TABLE reborn_component_review_work_confirmations
    ADD CONSTRAINT review_confirmation_exact_case_reference UNIQUE(submission_id,checksum);
ALTER TABLE reborn_component_validator_selections
    ADD CONSTRAINT validator_selection_exact_case_reference UNIQUE(submission_id,checksum);

CREATE TABLE reborn_original_pure_cases (
    case_id UUID PRIMARY KEY,
    submission_id UUID NOT NULL REFERENCES reborn_component_review_submissions(submission_id) ON DELETE RESTRICT,
    subject_checksum TEXT NOT NULL CHECK(subject_checksum ~ '^[0-9a-f]{64}$'),
    source_work_id UUID NOT NULL,
    source_delivery_checksum BYTEA NOT NULL CHECK(octet_length(source_delivery_checksum)=32),
    source_confirmation_checksum BYTEA NOT NULL CHECK(octet_length(source_confirmation_checksum)=32),
    source_selection_checksum TEXT NOT NULL CHECK(source_selection_checksum ~ '^[0-9a-f]{64}$'),
    execution_context JSONB NOT NULL,
    case_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(case_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(case_id,checksum),
    CHECK(case_id<>submission_id AND source_work_id=submission_id),
    FOREIGN KEY(source_work_id,source_delivery_checksum)
        REFERENCES brassclaw_monty_private_deliveries(work_id,checksum) ON DELETE RESTRICT,
    FOREIGN KEY(submission_id,source_confirmation_checksum)
        REFERENCES reborn_component_review_work_confirmations(submission_id,checksum) ON DELETE RESTRICT,
    FOREIGN KEY(submission_id,source_selection_checksum)
        REFERENCES reborn_component_validator_selections(submission_id,checksum) ON DELETE RESTRICT
);
CREATE FUNCTION verify_original_pure_case() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE c JSONB:=NEW.case_bytes::jsonb;
BEGIN
    IF (c ?& ARRAY['format','case_id','submission_id','subject_checksum','source_delivery',
            'source_confirmation','source_selection_checksum','worker_checksum','candidate','export','inputs','context']
        AND (c-ARRAY['format','case_id','submission_id','subject_checksum','source_delivery',
            'source_confirmation','source_selection_checksum','worker_checksum','candidate','export','inputs','context'])='{}'::jsonb
        AND c->>'format'='original-pure-case/1'
        AND (c->>'case_id')::uuid=NEW.case_id AND (c->>'submission_id')::uuid=NEW.submission_id
        AND c->>'subject_checksum'=NEW.subject_checksum AND c->'context'=NEW.execution_context
        AND c->>'source_selection_checksum'=NEW.source_selection_checksum
        AND c->'source_delivery'=jsonb_build_object('id',NEW.source_work_id,'checksum',encode(NEW.source_delivery_checksum,'hex'))
        AND c->'source_confirmation'=jsonb_build_object('id',NEW.submission_id,'checksum',encode(NEW.source_confirmation_checksum,'hex'))
        AND jsonb_typeof(c->'inputs')='object' AND jsonb_typeof(c->'export')='string' AND length(c->>'export')>0
        AND EXISTS(SELECT 1 FROM reborn_component_review_submissions s
            JOIN reborn_component_validator_selections v USING(submission_id)
            JOIN reborn_component_review_work_confirmations f USING(submission_id)
            JOIN brassclaw_turns w ON w.work_id=f.work_id AND w.validator_submission_id=s.submission_id
            JOIN brassclaw_monty_private_preparations p ON p.work_id=w.work_id AND p.checksum=w.private_definition_checksum
            WHERE s.submission_id=NEW.submission_id AND s.checksum=NEW.subject_checksum
                AND w.record_kind='private_work' AND w.status='settled' AND w.private_cancel_revision=0
                AND w.private_request_bytes::jsonb->'context'=NEW.execution_context
                AND v.checksum=NEW.source_selection_checksum
                AND v.selection_bytes::jsonb->>'format'='component-validator-selection/2'
                AND v.selection_bytes::jsonb->>'scope'='qualified-pure-source-prerequisite'
                AND f.checksum=NEW.source_confirmation_checksum AND f.work_id=NEW.source_work_id
                AND f.delivery_checksum=NEW.source_delivery_checksum
                AND f.confirmation_bytes::jsonb->'checks_succeeded'='true'::jsonb
                AND f.confirmation_bytes::jsonb->'errors'='[]'::jsonb
                AND f.confirmation_bytes::jsonb->'q1_qualified'='false'::jsonb
                AND f.confirmation_bytes::jsonb->'human_q2'='false'::jsonb
                AND f.confirmation_bytes::jsonb->'activation'='false'::jsonb
                AND c->'worker_checksum'=p.definition_bytes::jsonb->'worker_checksum'
                AND c->'candidate'->>'class_code'='22'
                AND EXISTS(SELECT 1 FROM reborn_component_revisions r
                    WHERE r.component_id=s.component_id AND r.version=s.candidate_version
                        AND r.component_id=(c->'candidate'->>'uuid')::uuid
                        AND r.version=(c->'candidate'->>'version')::bigint AND r.class_code=22
                        AND c->'candidate'=jsonb_build_object('uuid',r.component_id,'class_code',r.class_code,
                            'version',r.version,'checksum',
                            (SELECT jsonb_agg(get_byte(decode(r.checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n))))) IS NOT TRUE THEN
        RAISE EXCEPTION 'original Pure case requires its exact positive Source prerequisite' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER original_pure_case_original BEFORE INSERT ON reborn_original_pure_cases
    FOR EACH ROW EXECUTE FUNCTION verify_original_pure_case();
CREATE TRIGGER original_pure_cases_immutable BEFORE UPDATE OR DELETE ON reborn_original_pure_cases
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER original_pure_cases_no_truncate BEFORE TRUNCATE ON reborn_original_pure_cases
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();

ALTER TABLE brassclaw_turns ADD COLUMN original_pure_case_id UUID
    REFERENCES reborn_original_pure_cases(case_id) ON DELETE RESTRICT;
CREATE UNIQUE INDEX private_pure_case_original_unique ON brassclaw_turns(original_pure_case_id)
    WHERE record_kind='private_work';

ALTER TABLE brassclaw_turns DROP CONSTRAINT turn_record_kind_shape;
ALTER TABLE brassclaw_turns ADD CONSTRAINT turn_record_kind_shape CHECK ((
    record_kind IN ('legacy','private_work','chat_run_address')
    AND CASE record_kind
    WHEN 'legacy' THEN turn_id IS NOT NULL AND work_id IS NULL AND execution_run_id IS NULL
        AND owner_snapshot_id IS NULL AND private_request_bytes IS NULL
        AND private_request_checksum IS NULL AND validator_submission_id IS NULL
        AND prefix_job_id IS NULL AND original_pure_case_id IS NULL AND private_definition_checksum IS NULL
        AND preparation_reason IS NULL AND ready_at IS NULL AND private_runner_id IS NULL
        AND private_claim_checksum IS NULL AND private_claim_generation IS NULL
        AND private_claim_attempts IS NULL AND private_lease_deadline IS NULL
        AND private_cancel_revision IS NULL AND private_cancel_requested_at IS NULL
        AND private_admission_run IS NULL AND private_terminal_checksum IS NULL
    WHEN 'chat_run_address' THEN turn_id IS NULL AND run_id IS NULL AND work_id IS NULL
        AND execution_run_id IS NOT NULL AND owner_snapshot_id IS NOT NULL
        AND status='address' AND payload='{}'::jsonb AND version=0
        AND private_request_bytes IS NULL AND private_request_checksum IS NULL
        AND validator_submission_id IS NULL AND prefix_job_id IS NULL AND original_pure_case_id IS NULL
        AND private_definition_checksum IS NULL AND preparation_reason IS NULL
        AND ready_at IS NULL AND private_runner_id IS NULL AND private_claim_checksum IS NULL
        AND private_claim_generation IS NULL AND private_claim_attempts IS NULL
        AND private_lease_deadline IS NULL AND private_cancel_revision IS NULL
        AND private_cancel_requested_at IS NULL AND private_admission_run IS NULL
        AND private_terminal_checksum IS NULL
    WHEN 'private_work' THEN turn_id IS NULL AND run_id IS NULL AND work_id IS NOT NULL
        AND execution_run_id IS NOT NULL AND owner_snapshot_id IS NULL AND payload='{}'::jsonb
        AND version>0 AND private_request_bytes IS NOT NULL
        AND private_request_checksum=sha256(convert_to(private_request_bytes,'UTF8'))
        AND num_nonnulls(validator_submission_id,prefix_job_id,original_pure_case_id)=1
        AND private_claim_generation>=0 AND private_claim_attempts=private_claim_generation
        AND private_cancel_revision>=0
        AND (private_cancel_revision=0)=(private_cancel_requested_at IS NULL)
        AND CASE status
        WHEN 'awaiting_preparation' THEN private_definition_checksum IS NULL AND ready_at IS NULL
            AND preparation_reason IN ('preparation_pending','qualification_unavailable')
            AND private_claim_generation=0 AND private_runner_id IS NULL
            AND private_claim_checksum IS NULL AND private_lease_deadline IS NULL
            AND private_admission_run IS NULL AND private_terminal_checksum IS NULL
        WHEN 'ready' THEN private_definition_checksum IS NOT NULL AND ready_at IS NOT NULL
            AND preparation_reason IS NULL AND private_cancel_revision=0
            AND private_claim_generation=0 AND private_runner_id IS NULL
            AND private_claim_checksum IS NULL AND private_lease_deadline IS NULL
            AND private_admission_run IS NULL AND private_terminal_checksum IS NULL
        WHEN 'cancelled' THEN private_cancel_revision>0 AND private_claim_generation=0
            AND private_runner_id IS NULL AND private_claim_checksum IS NULL
            AND private_lease_deadline IS NULL AND private_admission_run IS NULL
            AND private_terminal_checksum IS NULL
        WHEN 'claimed' THEN private_definition_checksum IS NOT NULL AND ready_at IS NOT NULL
            AND preparation_reason IS NULL AND private_claim_generation=1
            AND private_runner_id IS NOT NULL AND octet_length(private_claim_checksum)=32
            AND private_lease_deadline IS NOT NULL AND private_admission_run=execution_run_id
            AND private_terminal_checksum IS NULL
        WHEN 'reconciliation_required' THEN private_definition_checksum IS NOT NULL
            AND ready_at IS NOT NULL AND preparation_reason IS NULL AND private_claim_generation=1
            AND private_runner_id IS NOT NULL AND octet_length(private_claim_checksum)=32
            AND private_lease_deadline IS NOT NULL AND private_admission_run=execution_run_id
            AND private_terminal_checksum IS NULL
        WHEN 'settled' THEN private_definition_checksum IS NOT NULL AND ready_at IS NOT NULL
            AND preparation_reason IS NULL AND private_claim_generation=1
            AND private_runner_id IS NOT NULL AND octet_length(private_claim_checksum)=32
            AND private_lease_deadline IS NOT NULL AND private_admission_run=execution_run_id
            AND octet_length(private_terminal_checksum)=32
        ELSE false END
    ELSE false END
) IS TRUE);

-- Extend the current V209 guards, preserving every parenthesized extraction.
CREATE OR REPLACE FUNCTION protect_private_work_origin() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE req jsonb; origin jsonb; old_identity jsonb; new_identity jsonb;
BEGIN
    IF TG_OP='TRUNCATE' THEN
        RAISE EXCEPTION 'retained work origins cannot be truncated' USING ERRCODE='23514';
    END IF;
    IF TG_OP='DELETE' THEN
        IF OLD.record_kind<>'legacy' THEN
            RAISE EXCEPTION 'retained work origins cannot be removed' USING ERRCODE='23514';
        END IF;
        RETURN OLD;
    END IF;
    IF TG_OP='UPDATE' AND OLD.record_kind IS DISTINCT FROM NEW.record_kind THEN
        RAISE EXCEPTION 'work origins cannot be reinterpreted' USING ERRCODE='23514';
    END IF;
    IF NEW.record_kind='legacy' THEN RETURN NEW; END IF;
    IF NEW.record_kind='chat_run_address' THEN
        IF TG_OP='UPDATE' AND (to_jsonb(OLD)-'updated_at') IS DISTINCT FROM (to_jsonb(NEW)-'updated_at') THEN
            RAISE EXCEPTION 'chat run addresses are immutable' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    req:=NEW.private_request_bytes::jsonb;
    origin:=req->'original';
    IF (jsonb_typeof(req)='object'
        AND req-'format'-'work_id'-'run_id'-'original'-'context'-'original_preparation_checksum'-'original_workflow_checksum'='{}'::jsonb
        AND req ?& ARRAY['format','work_id','run_id','original','context','original_preparation_checksum','original_workflow_checksum']
        AND (req->'original_preparation_checksum'='null'::jsonb OR private_work_hash_value(req->'original_preparation_checksum'))
        AND (req->'original_workflow_checksum'='null'::jsonb OR private_work_hash_value(req->'original_workflow_checksum'))
        AND req->>'format'='private-work-request/1'
        AND (req->>'work_id')::uuid=NEW.work_id
        AND (req->>'run_id')::uuid=NEW.execution_run_id
        AND req->'context'->>'tenant_id'=NEW.tenant_id
        AND (req->'context')-'tenant_id'-'agent_id'-'project_id'-'actor'='{}'::jsonb
        AND req->'context' ?& ARRAY['tenant_id','agent_id','project_id','actor']
        AND jsonb_typeof(req->'context')='object'
        AND jsonb_typeof(req->'context'->'tenant_id')='string'
        AND jsonb_typeof(req->'context'->'agent_id') IN ('null','string')
        AND jsonb_typeof(req->'context'->'project_id') IN ('null','string')
        AND (req->'context'->'actor'='null'::jsonb OR
            (jsonb_typeof(req->'context'->'actor')='object'
             AND (req->'context'->'actor')-'user_id'='{}'::jsonb
             AND jsonb_typeof(req->'context'->'actor'->'user_id')='string'))) IS NOT TRUE THEN
        RAISE EXCEPTION 'private request identity is invalid' USING ERRCODE='23514';
    END IF;
    IF NEW.original_pure_case_id IS NOT NULL THEN
        IF (origin->>'purpose'='original_pure_case/1'
            AND origin ?& ARRAY['purpose','case_id','case_checksum']
            AND (origin-ARRAY['purpose','case_id','case_checksum'])='{}'::jsonb
            AND (origin->>'case_id')::uuid=NEW.original_pure_case_id
            AND NEW.work_id=NEW.original_pure_case_id AND NEW.execution_run_id=NEW.original_pure_case_id
            AND EXISTS(SELECT 1 FROM reborn_original_pure_cases c WHERE c.case_id=NEW.original_pure_case_id
                AND c.execution_context=req->'context' AND origin->'case_checksum'=
                    (SELECT jsonb_agg(get_byte(c.checksum,n) ORDER BY n) FROM generate_series(0,31)n))) IS NOT TRUE THEN
            RAISE EXCEPTION 'private original Pure case mismatch' USING ERRCODE='23514';
        END IF;
    ELSIF NEW.validator_submission_id IS NOT NULL THEN
        IF (origin->>'purpose'='validator_structural'
            AND (origin->>'submission_id')::uuid=NEW.validator_submission_id
            AND origin-'purpose'-'submission_id'-'subject_checksum'='{}'::jsonb
            AND EXISTS(SELECT 1 FROM reborn_component_review_submissions s
                WHERE s.submission_id=NEW.validator_submission_id AND origin->'subject_checksum'=
                (SELECT jsonb_agg(get_byte(decode(s.checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n))) IS NOT TRUE THEN
            RAISE EXCEPTION 'private validator original mismatch' USING ERRCODE='23514';
        END IF;
    ELSE
        IF (origin->>'purpose'='prefix_source_only'
            AND (origin->>'job_id')::uuid=NEW.prefix_job_id
            AND origin-'purpose'-'job_id'-'definition_checksum'='{}'::jsonb
            AND EXISTS(SELECT 1 FROM brassclaw_prefix_job_definitions d
                WHERE d.job_id=NEW.prefix_job_id AND d.definition_format='prefix-job-definition/2'
                AND d.private_context IS NOT DISTINCT FROM req->'context' AND origin->'definition_checksum'=
                (SELECT jsonb_agg(get_byte(sha256(decode(d.request_checksum||d.program_checksum||d.input_checksum,'hex')),n) ORDER BY n)
                    FROM generate_series(0,31)n))) IS NOT TRUE THEN
            RAISE EXCEPTION 'private prefix original mismatch' USING ERRCODE='23514';
        END IF;
    END IF;
    IF TG_OP='INSERT' THEN
        IF NEW.status<>'awaiting_preparation' OR NEW.version<>1 THEN
            RAISE EXCEPTION 'private work must begin unclaimed and unprepared' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    old_identity:=to_jsonb(OLD)-ARRAY['status','version','updated_at','private_definition_checksum',
        'preparation_reason','ready_at','private_runner_id','private_claim_checksum',
        'private_claim_generation','private_claim_attempts','private_lease_deadline',
        'private_cancel_revision','private_cancel_requested_at','private_admission_run','private_terminal_checksum'];
    new_identity:=to_jsonb(NEW)-ARRAY['status','version','updated_at','private_definition_checksum',
        'preparation_reason','ready_at','private_runner_id','private_claim_checksum',
        'private_claim_generation','private_claim_attempts','private_lease_deadline',
        'private_cancel_revision','private_cancel_requested_at','private_admission_run','private_terminal_checksum'];
    IF old_identity IS DISTINCT FROM new_identity OR NEW.version<>OLD.version+1 THEN
        RAISE EXCEPTION 'private original is immutable and versioned' USING ERRCODE='23514';
    END IF;
    IF OLD.private_definition_checksum IS NOT NULL AND
        NEW.private_definition_checksum IS DISTINCT FROM OLD.private_definition_checksum THEN
        RAISE EXCEPTION 'private preparation cannot be replaced' USING ERRCODE='23514';
    END IF;
    IF OLD.ready_at IS NOT NULL AND NEW.ready_at IS DISTINCT FROM OLD.ready_at THEN
        RAISE EXCEPTION 'private ready timestamp is immutable' USING ERRCODE='23514';
    END IF;
    IF NOT ((OLD.status='awaiting_preparation' AND NEW.status IN ('ready','cancelled'))
        OR (OLD.status='ready' AND NEW.status IN ('claimed','cancelled'))
        OR (OLD.status='claimed' AND NEW.status IN ('claimed','settled','reconciliation_required'))
        OR (OLD.status='reconciliation_required' AND NEW.status='settled')) THEN
        RAISE EXCEPTION 'private work transition is not allowed' USING ERRCODE='23514';
    END IF;
    IF OLD.private_claim_generation>0 AND
        (NEW.private_runner_id IS DISTINCT FROM OLD.private_runner_id
         OR NEW.private_claim_checksum IS DISTINCT FROM OLD.private_claim_checksum
         OR NEW.private_claim_generation<>OLD.private_claim_generation
         OR NEW.private_claim_attempts<>OLD.private_claim_attempts
         OR NEW.private_admission_run IS DISTINCT FROM OLD.private_admission_run) THEN
        RAISE EXCEPTION 'an admitted private attempt cannot be replaced' USING ERRCODE='23514';
    END IF;
    IF NEW.private_cancel_revision<OLD.private_cancel_revision
        OR NEW.private_cancel_revision>OLD.private_cancel_revision+1
        OR (NEW.private_cancel_revision=OLD.private_cancel_revision
            AND NEW.private_cancel_requested_at IS DISTINCT FROM OLD.private_cancel_requested_at) THEN
        RAISE EXCEPTION 'private cancellation is monotonic' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION verify_private_preparation_original() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE envelope jsonb:=NEW.definition_bytes::jsonb; work brassclaw_turns%ROWTYPE; capture jsonb;
BEGIN
    SELECT * INTO STRICT work FROM brassclaw_turns WHERE work_id=NEW.work_id FOR UPDATE;
    IF (envelope ?& ARRAY['format','work_id','run_id','request_checksum','preparation_bytes','preparation_checksum',
            'worker_checksum','root_revision_bytes','root_revision_version','workflow_checksum','input_checksum','domain']
        AND (envelope-ARRAY['format','work_id','run_id','request_checksum','preparation_bytes','preparation_checksum',
            'worker_checksum','root_revision_bytes','root_revision_version','workflow_checksum','input_checksum','domain'])='{}'::jsonb
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
            AND r.checksum=encode(sha256(convert_to(envelope->>'root_revision_bytes','UTF8')),'hex')) THEN
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


CREATE TABLE reborn_original_pure_case_executions (
    case_id UUID PRIMARY KEY REFERENCES reborn_original_pure_cases(case_id) ON DELETE RESTRICT,
    work_id UUID NOT NULL UNIQUE REFERENCES brassclaw_turns(work_id) ON DELETE RESTRICT,
    run_id UUID NOT NULL UNIQUE REFERENCES brassclaw_monty_task_admissions(run_id) ON DELETE RESTRICT,
    invocation_id UUID NOT NULL UNIQUE REFERENCES brassclaw_monty_tool_invocations(invocation_id) ON DELETE RESTRICT,
    execution_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(execution_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(case_id,checksum),
    CHECK(case_id=work_id AND case_id=run_id)
);
CREATE FUNCTION verify_original_pure_case_execution() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE e JSONB:=NEW.execution_bytes::jsonb;
BEGIN
    IF (e ?& ARRAY['format','case_id','case_checksum','run_id','invocation_id','task','scope','worker_checksum']
        AND (e-ARRAY['format','case_id','case_checksum','run_id','invocation_id','task','scope','worker_checksum'])='{}'::jsonb
        AND e->>'format'='original-pure-case-execution/1'
        AND (e->>'case_id')::uuid=NEW.case_id AND (e->>'run_id')::uuid=NEW.run_id
        AND (e->>'invocation_id')::uuid=NEW.invocation_id
        AND (e->>'task')::uuid<>'00000000-0000-0000-0000-000000000000'
        AND EXISTS(SELECT 1 FROM reborn_original_pure_cases c
            JOIN brassclaw_turns w ON w.original_pure_case_id=c.case_id
            JOIN brassclaw_monty_task_admissions a ON a.run_id=w.execution_run_id
            JOIN brassclaw_monty_tool_invocations i ON i.run_id=a.run_id
            WHERE c.case_id=NEW.case_id AND w.work_id=NEW.work_id AND a.run_id=NEW.run_id
                AND i.invocation_id=NEW.invocation_id AND i.attempt_count=1 AND i.phase='dispatch_intent'
                AND i.selection_bytes::jsonb->>'capability_id'='builtin.pure_review.invoke_original'
                AND i.arguments_bytes::jsonb='{}'::jsonb
                AND a.origin_kind='private_work' AND a.private_work_id=w.work_id
                AND w.status='claimed' AND w.private_cancel_revision=0
                AND w.private_lease_deadline>clock_timestamp()
                AND a.claim_checksum=w.private_claim_checksum
                AND e->'case_checksum'=(SELECT jsonb_agg(get_byte(c.checksum,n) ORDER BY n) FROM generate_series(0,31)n)
                AND e->'worker_checksum'=c.case_bytes::jsonb->'worker_checksum'
                AND e->'scope'->>'tenant_id'=c.execution_context->>'tenant_id'
                AND COALESCE(e->'scope'->'agent_id','null'::jsonb) IS NOT DISTINCT FROM c.execution_context->'agent_id'
                AND e->'scope'->'project_id' IS NOT DISTINCT FROM c.execution_context->'project_id'
                AND e->'scope'->'thread_id'='null'::jsonb
                AND e->'scope'->>'user_id'=c.execution_context->'actor'->>'user_id'
                AND (e->'scope'->>'invocation_id')::uuid=NEW.invocation_id)) IS NOT TRUE THEN
        RAISE EXCEPTION 'original Pure execution must bind its actual native intent and live Work' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER original_pure_case_execution_original BEFORE INSERT ON reborn_original_pure_case_executions
    FOR EACH ROW EXECUTE FUNCTION verify_original_pure_case_execution();

-- Full originals for a fixed set of boundaries only. Internal pure scheduling
-- ticks contribute to the checked rolling digest/count, not another DB journal.
CREATE TABLE reborn_original_pure_case_native_receipts (
    case_id UUID NOT NULL,
    stage TEXT NOT NULL CHECK(stage IN ('open','preload','preload_terminal','start','terminal','release')),
    ordinal BIGINT NOT NULL CHECK(ordinal>=0),
    execution_checksum BYTEA NOT NULL CHECK(octet_length(execution_checksum)=32),
    request_id UUID NOT NULL UNIQUE,
    receipt_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(receipt_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(case_id,stage), UNIQUE(case_id,ordinal), UNIQUE(case_id,checksum),
    FOREIGN KEY(case_id,execution_checksum) REFERENCES reborn_original_pure_case_executions(case_id,checksum) ON DELETE RESTRICT,
    CHECK((receipt_bytes::jsonb->>'format'='original-pure-native-receipt/1') IS TRUE),
    CHECK(((receipt_bytes::jsonb->>'ordinal')::bigint=ordinal) IS TRUE),
    CHECK(((receipt_bytes::jsonb->>'request_id')::uuid=request_id) IS TRUE),
    CHECK((receipt_bytes::jsonb->'command'->>'operation'='recipe') IS TRUE)
);

CREATE TABLE reborn_original_pure_case_observations (
    case_id UUID PRIMARY KEY,
    execution_checksum BYTEA NOT NULL CHECK(octet_length(execution_checksum)=32),
    observation_bytes TEXT NOT NULL,
    native_receipts_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(observation_bytes,'UTF8')||convert_to(native_receipts_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(case_id,checksum),
    FOREIGN KEY(case_id,execution_checksum) REFERENCES reborn_original_pure_case_executions(case_id,checksum) ON DELETE RESTRICT
);
CREATE FUNCTION original_pure_case_observation_valid(p_case_id UUID, p_execution_checksum BYTEA,
    p_observation_bytes TEXT, p_native_receipts_bytes TEXT) RETURNS boolean LANGUAGE plpgsql STABLE AS $$
DECLARE o JSONB:=p_observation_bytes::jsonb; n JSONB:=p_native_receipts_bytes::jsonb; e JSONB; c JSONB; last_receipt JSONB; start_receipt JSONB; terminal_receipt JSONB; input_digest BYTEA;
BEGIN
    SELECT execution_bytes::jsonb INTO STRICT e FROM reborn_original_pure_case_executions WHERE case_id=p_case_id;
    SELECT case_bytes::jsonb INTO STRICT c FROM reborn_original_pure_cases WHERE case_id=p_case_id;
    SELECT receipt_bytes::jsonb INTO STRICT last_receipt FROM reborn_original_pure_case_native_receipts
        WHERE case_id=p_case_id AND stage='release' AND ordinal=(n->>'receipt_count')::bigint-1
            AND n->'receipt_chain'=(SELECT jsonb_agg(get_byte(checksum,k) ORDER BY k) FROM generate_series(0,31)k);
    SELECT receipt_bytes::jsonb,
        sha256(convert_to((receipt_bytes::json->'command'->'command'->'inputs')::text,'UTF8'))
        INTO STRICT start_receipt,input_digest FROM reborn_original_pure_case_native_receipts WHERE case_id=p_case_id AND stage='start';
    SELECT receipt_bytes::jsonb INTO STRICT terminal_receipt FROM reborn_original_pure_case_native_receipts
        WHERE case_id=p_case_id AND stage=n->>'terminal_stage';
    RETURN (o ?& ARRAY['format','component','export','task','parent','host_key','context','inputs_checksum','invocation_checksum','outcome']
        AND (o-ARRAY['format','component','export','task','parent','host_key','context','inputs_checksum','invocation_checksum','outcome'])='{}'::jsonb
        AND n ?& ARRAY['format','task','parent','host_key','root','started','pending_request','received_request','receipt_count','receipt_chain','terminal_stage','retained_receipts']
        AND (n-ARRAY['format','task','parent','host_key','root','started','pending_request','received_request','receipt_count','receipt_chain','terminal_stage','retained_receipts'])='{}'::jsonb
        AND o->>'format'='original-pure-invocation/1'
        AND o->'task'=e->'task' AND o->'component'=c->'candidate' AND o->'export'=c->'export'
        AND n->>'format'='original-pure-native-receipts/2'
        AND n->'task'=o->'task' AND n->'parent'=o->'parent' AND n->'host_key'=o->'host_key'
        AND n->'started'='true'::jsonb AND n->'pending_request'='null'::jsonb AND n->'received_request'='null'::jsonb
        AND start_receipt->'command'->'command'->>'operation'='start'
        AND start_receipt->'command'->'command'->'context'=o->'context'
        AND start_receipt->'command'->'command'->'selected'->'checksum'=o->'invocation_checksum'
        AND o->'inputs_checksum'=(SELECT jsonb_agg(get_byte(input_digest,k) ORDER BY k) FROM generate_series(0,31)k)
        AND terminal_receipt->'command'->'command'->>'operation' IN ('start','resume_control')
        AND terminal_receipt->'command'->'command'->'context'=o->'context'
        AND terminal_receipt->'snapshot'->'root'=n->'root'
        AND ((o->'outcome'->>'kind' IN ('return','result_contract_violation')
                AND terminal_receipt->'failure'='null'::jsonb
                AND terminal_receipt->'snapshot'->'recipe'->'boundary'->>'kind'='complete'
                AND terminal_receipt->'snapshot'->'recipe'->'boundary'->'value'=o->'outcome'->'value')
            OR (o->'outcome'->>'kind'='python_exception'
                AND terminal_receipt->>'failure'='Vm(Python)'
                AND terminal_receipt->'snapshot'->'recipe'->>'kind'='failed'
                AND terminal_receipt->'python_exception'=o->'outcome'->'exception'))
        AND (n->>'receipt_count')::bigint>=4
        AND jsonb_array_length(n->'retained_receipts')=(SELECT count(*) FROM reborn_original_pure_case_native_receipts WHERE case_id=p_case_id)
        AND EXISTS(SELECT 1 FROM reborn_original_pure_case_native_receipts WHERE case_id=p_case_id AND stage='open')
        AND EXISTS(SELECT 1 FROM reborn_original_pure_case_native_receipts WHERE case_id=p_case_id AND stage='preload')
        AND EXISTS(SELECT 1 FROM reborn_original_pure_case_native_receipts WHERE case_id=p_case_id AND stage='start')
        AND EXISTS(SELECT 1 FROM reborn_original_pure_case_native_receipts WHERE case_id=p_case_id AND stage=n->>'terminal_stage')
        AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(n->'retained_receipts') ref WHERE NOT EXISTS(
            SELECT 1 FROM reborn_original_pure_case_native_receipts r WHERE r.case_id=p_case_id
                AND r.stage=ref->>'stage' AND r.ordinal=(ref->>'ordinal')::bigint AND r.request_id=(ref->>'request_id')::uuid
                AND ref->'checksum'=(SELECT jsonb_agg(get_byte(r.checksum,k) ORDER BY k) FROM generate_series(0,31)k)))
        AND last_receipt->'failure'='null'::jsonb
        AND last_receipt->'snapshot'->'root'=n->'root'
        AND last_receipt->'snapshot'->>'lifecycle'='Ready'
        AND last_receipt->'snapshot'->'recipe'->>'kind'='released'
        AND last_receipt->'snapshot'->'recipe'->'task'='null'::jsonb
        AND jsonb_array_length(last_receipt->'snapshot'->'recipe'->'contexts')=1
        AND last_receipt->'snapshot'->'recipe'->'contexts'->0->'context'=o->'context'
        AND last_receipt->'snapshot'->'recipe'->'contexts'->0->'pending_host'='null'::jsonb
        AND last_receipt->'snapshot'->'recipe'->'parent_host'=jsonb_build_object(
            'task',o->'task','context',o->'parent','key',o->'host_key')
        AND o->'outcome'->>'kind' IN ('return','python_exception','result_contract_violation')
        AND EXISTS(SELECT 1 FROM reborn_original_pure_case_executions x WHERE x.case_id=p_case_id
            AND x.checksum=p_execution_checksum AND x.checksum=sha256(convert_to(x.execution_bytes,'UTF8')))
        AND NOT EXISTS(SELECT 1 FROM reborn_original_pure_case_native_receipts r WHERE r.case_id=p_case_id
            AND (r.checksum=sha256(convert_to(r.receipt_bytes,'UTF8'))
                AND (r.receipt_bytes::jsonb->>'request_id')::uuid=r.request_id
                AND (r.receipt_bytes::jsonb->>'ordinal')::bigint=r.ordinal
                AND r.execution_checksum=p_execution_checksum) IS NOT TRUE)) IS TRUE;
END $$;
CREATE FUNCTION verify_original_pure_case_observation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT original_pure_case_observation_valid(NEW.case_id,NEW.execution_checksum,NEW.observation_bytes,NEW.native_receipts_bytes) THEN
        RAISE EXCEPTION 'original Pure observation requires its complete native chain and released child with paused parent' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER original_pure_case_observation_original BEFORE INSERT ON reborn_original_pure_case_observations
    FOR EACH ROW EXECUTE FUNCTION verify_original_pure_case_observation();

-- Failure custody has no successful observation/settlement edge. It also
-- covers a missing execution-intent ACK: the actual native journal, not an
-- assumed successful intent write, establishes the original invocation.
CREATE TABLE reborn_original_pure_case_failures (
    case_id UUID NOT NULL REFERENCES reborn_original_pure_cases(case_id) ON DELETE RESTRICT,
    invocation_id UUID NOT NULL REFERENCES brassclaw_monty_tool_invocations(invocation_id) ON DELETE RESTRICT,
    phase TEXT NOT NULL CHECK(phase IN ('operation','transport_readback','answer_ack','kernel_result')),
    failure_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(failure_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(case_id,phase)
);
CREATE FUNCTION verify_original_pure_case_failure() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE f JSONB:=NEW.failure_bytes::jsonb;
BEGIN
    IF (f ?& ARRAY['format','case_id','invocation_id','phase','execution_bytes','failure','native','originals']
        AND (f-ARRAY['format','case_id','invocation_id','phase','execution_bytes','failure','native','originals'])='{}'::jsonb
        AND f->>'format'='original-pure-case-technical-failure/1'
        AND (f->>'case_id')::uuid=NEW.case_id AND (f->>'invocation_id')::uuid=NEW.invocation_id
        AND f->>'phase'=NEW.phase
        AND jsonb_typeof(f->'failure')='object' AND jsonb_typeof(f->'originals')='object'
        AND (f->'originals'-ARRAY['open','preload','preload_terminal','start','terminal','release','unresolved'])='{}'::jsonb
        AND NOT EXISTS(SELECT 1 FROM jsonb_each(f->'originals') AS r(stage,value)
            WHERE (value ?& ARRAY['ordinal','request_id','checksum','bytes']
                AND (value-ARRAY['ordinal','request_id','checksum','bytes'])='{}'::jsonb
                AND jsonb_typeof(value->'bytes')='string'
                AND (value->>'bytes')::jsonb->>'format'='original-pure-native-receipt/1'
                AND (value->>'bytes')::jsonb->'ordinal'=value->'ordinal'
                AND (value->>'bytes')::jsonb->'request_id'=value->'request_id'
                AND value->'checksum'=(SELECT jsonb_agg(get_byte(sha256(convert_to(value->>'bytes','UTF8')),n) ORDER BY n) FROM generate_series(0,31)n)) IS NOT TRUE)
        AND f->'native'->>'format'='original-pure-native-receipts/2'
        AND f->'native'->'task'=(f->>'execution_bytes')::jsonb->'task'
        AND (f->>'execution_bytes')::jsonb->>'format'='original-pure-case-execution/1'
        AND ((f->>'execution_bytes')::jsonb->>'case_id')::uuid=NEW.case_id
        AND ((f->>'execution_bytes')::jsonb->>'invocation_id')::uuid=NEW.invocation_id
        AND EXISTS(SELECT 1 FROM brassclaw_monty_tool_invocations i
            JOIN brassclaw_monty_task_admissions a ON a.run_id=i.run_id
            JOIN brassclaw_turns w ON w.work_id=a.private_work_id
            JOIN reborn_original_pure_cases c ON c.case_id=w.original_pure_case_id
            WHERE i.invocation_id=NEW.invocation_id AND i.run_id=NEW.case_id
                AND i.attempt_count=1 AND i.selection_bytes::jsonb->>'capability_id'='builtin.pure_review.invoke_original'
                AND i.arguments_bytes::jsonb='{}'::jsonb AND a.origin_kind='private_work'
                AND w.work_id=NEW.case_id AND w.original_pure_case_id=NEW.case_id
                AND (f->>'execution_bytes')::jsonb->'case_checksum'=(SELECT jsonb_agg(get_byte(c.checksum,n) ORDER BY n) FROM generate_series(0,31)n)
                AND (f->>'execution_bytes')::jsonb->'worker_checksum'=c.case_bytes::jsonb->'worker_checksum'
                AND (f->>'execution_bytes')::jsonb->'scope'->>'tenant_id'=c.execution_context->>'tenant_id'
                AND (f->>'execution_bytes')::jsonb->'scope'->'agent_id' IS NOT DISTINCT FROM c.execution_context->'agent_id'
                AND (f->>'execution_bytes')::jsonb->'scope'->'project_id' IS NOT DISTINCT FROM c.execution_context->'project_id'
                AND (f->>'execution_bytes')::jsonb->'scope'->'thread_id'='null'::jsonb
                AND (f->>'execution_bytes')::jsonb->'scope'->>'user_id'=c.execution_context->'actor'->>'user_id')) IS NOT TRUE THEN
        RAISE EXCEPTION 'original Pure technical failure must retain its actual case/native invocation' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER original_pure_case_failure_original BEFORE INSERT ON reborn_original_pure_case_failures
    FOR EACH ROW EXECUTE FUNCTION verify_original_pure_case_failure();

CREATE TABLE reborn_original_pure_case_settlements (
    case_id UUID PRIMARY KEY,
    observation_checksum BYTEA NOT NULL CHECK(octet_length(observation_checksum)=32),
    delivery_checksum BYTEA NOT NULL CHECK(octet_length(delivery_checksum)=32),
    settlement_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(settlement_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY(case_id,observation_checksum) REFERENCES reborn_original_pure_case_observations(case_id,checksum) ON DELETE RESTRICT,
    FOREIGN KEY(case_id,delivery_checksum) REFERENCES brassclaw_monty_private_deliveries(work_id,checksum) ON DELETE RESTRICT
);
CREATE FUNCTION verify_original_pure_case_settlement() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE s JSONB:=NEW.settlement_bytes::jsonb;
BEGIN
    IF (s ?& ARRAY['format','case_id','case_checksum','execution_checksum','observation_checksum','delivery_checksum']
        AND (s-ARRAY['format','case_id','case_checksum','execution_checksum','observation_checksum','delivery_checksum'])='{}'::jsonb
        AND s->>'format'='settled-original-pure-case/1' AND (s->>'case_id')::uuid=NEW.case_id
        AND s->'observation_checksum'=(SELECT jsonb_agg(get_byte(NEW.observation_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        AND s->'delivery_checksum'=(SELECT jsonb_agg(get_byte(NEW.delivery_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        AND EXISTS(SELECT 1 FROM reborn_original_pure_cases c
            JOIN reborn_original_pure_case_executions e USING(case_id)
            JOIN reborn_original_pure_case_observations o USING(case_id)
            JOIN brassclaw_monty_private_deliveries d ON d.work_id=c.case_id
            JOIN brassclaw_monty_task_admissions a ON a.run_id=d.run_id
            JOIN brassclaw_monty_tool_invocations i ON i.invocation_id=e.invocation_id AND i.run_id=a.run_id
            WHERE c.case_id=NEW.case_id AND o.checksum=NEW.observation_checksum AND d.checksum=NEW.delivery_checksum
                AND i.phase='answered' AND i.attempt_count=1
                AND i.answer_bytes::jsonb=jsonb_build_object('kind','return','value',o.observation_bytes)
                AND (a.outcome->>'internal_receipt')::jsonb=jsonb_build_object(
                    'format','original-pure-case-receipt/1','observation',o.observation_bytes::jsonb)
                AND (a.outcome->>'internal_receipt')::jsonb->'observation'->'task'=e.execution_bytes::jsonb->'task'
                AND s->'case_checksum'=(SELECT jsonb_agg(get_byte(c.checksum,n) ORDER BY n) FROM generate_series(0,31)n)
                AND s->'execution_checksum'=(SELECT jsonb_agg(get_byte(e.checksum,n) ORDER BY n) FROM generate_series(0,31)n))) IS NOT TRUE THEN
        RAISE EXCEPTION 'original Pure case settlement requires its exact successful harness delivery' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER original_pure_case_settlement_original BEFORE INSERT ON reborn_original_pure_case_settlements
    FOR EACH ROW EXECUTE FUNCTION verify_original_pure_case_settlement();

DO $$
DECLARE table_name TEXT;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['reborn_original_pure_case_executions',
        'reborn_original_pure_case_native_receipts','reborn_original_pure_case_observations',
        'reborn_original_pure_case_failures','reborn_original_pure_case_settlements'] LOOP
        EXECUTE format('CREATE TRIGGER original_pure_case_record_immutable BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION protect_component_review_records()',table_name);
        EXECUTE format('CREATE TRIGGER original_pure_case_record_no_truncate BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records()',table_name);
    END LOOP;
END $$;
