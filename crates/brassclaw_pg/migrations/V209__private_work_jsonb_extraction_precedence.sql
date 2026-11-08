-- Repair the existing private Work guards without changing V207's retained bytes.
-- Binary '-' binds above '->': group JSONB extraction before closed-key subtraction.
-- Keep the existing signatures/trigger identities and all original/claim/native checks.
-- V208 owns separate delivery/confirmation triggers, not these function bodies.

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
    IF NEW.validator_submission_id IS NOT NULL THEN
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
        AND envelope-ARRAY['format','work_id','run_id','request_checksum','preparation_bytes','preparation_checksum',
            'worker_checksum','root_revision_bytes','root_revision_version','workflow_checksum','input_checksum','domain']='{}'::jsonb
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
    IF work.prefix_job_id IS NOT NULL THEN
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
            AND capture-ARRAY['format','submission_id','subject_checksum','validator_selection_bytes',
                'validator_selection_checksum','validator_bytes','validator_checksum','input_bytes','input_checksum',
                'preparation_bytes','preparation_checksum','worker_checksum']='{}'::jsonb
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

CREATE OR REPLACE FUNCTION verify_private_terminal_original() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE envelope jsonb:=NEW.terminal_bytes::jsonb; address jsonb; completion jsonb; native_ref jsonb;
    work brassclaw_turns%ROWTYPE; admission brassclaw_monty_task_admissions%ROWTYPE; actual_refs integer;
BEGIN
    address:=envelope->'address'; completion:=envelope->'completion';
    IF (envelope->>'format'='monty-private-terminal/1'
        AND envelope ?& ARRAY['format','address','completion','reconciliation_refs']
        AND envelope-'format'-'address'-'completion'-'reconciliation_refs'='{}'::jsonb
        AND jsonb_typeof(envelope->'reconciliation_refs')='array'
        AND jsonb_typeof(address)='object' AND jsonb_typeof(completion)='object'
        AND address ?& ARRAY['work_id','run_id','claim_generation','claim_checksum','request_checksum',
            'definition_checksum','admission_ref','cancellation_revision']
        AND address-ARRAY['work_id','run_id','claim_generation','claim_checksum','request_checksum',
            'definition_checksum','admission_ref','cancellation_revision']='{}'::jsonb
        AND (address->>'work_id')::uuid=NEW.work_id AND (address->>'run_id')::uuid=NEW.run_id) IS NOT TRUE THEN
        RAISE EXCEPTION 'private terminal format is invalid' USING ERRCODE='23514';
    END IF;
    -- Same Work -> Admission lock order as claim/cancel/settlement writers.
    SELECT * INTO STRICT work FROM brassclaw_turns WHERE work_id=NEW.work_id FOR UPDATE;
    SELECT * INTO STRICT admission FROM brassclaw_monty_task_admissions WHERE run_id=NEW.run_id FOR UPDATE;
    IF admission.origin_kind<>'private_work' OR admission.private_work_id<>NEW.work_id OR admission.phase<>'settled'
        OR work.record_kind<>'private_work' OR work.execution_run_id<>NEW.run_id
        OR work.private_claim_checksum IS DISTINCT FROM admission.claim_checksum
        OR work.private_request_checksum IS DISTINCT FROM admission.private_request_checksum
        OR (address->>'claim_generation')::bigint<>work.private_claim_generation
        OR (address->>'cancellation_revision')::bigint<>work.private_cancel_revision
        OR address->'claim_checksum' IS DISTINCT FROM
            (SELECT jsonb_agg(get_byte(admission.claim_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        OR address->'request_checksum' IS DISTINCT FROM
            (SELECT jsonb_agg(get_byte(work.private_request_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        OR address->'definition_checksum' IS DISTINCT FROM
            (SELECT jsonb_agg(get_byte(work.private_definition_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        OR address->'admission_ref' IS DISTINCT FROM
            (SELECT jsonb_agg(get_byte(sha256(admission.admission_key),n) ORDER BY n) FROM generate_series(0,31)n) THEN
        RAISE EXCEPTION 'private terminal original mismatch' USING ERRCODE='23514';
    END IF;
    IF completion->>'completion_origin'='never_submitted' THEN
        IF (completion ?& ARRAY['completion_origin','reason']) IS NOT TRUE
            OR completion-'completion_origin'-'reason'<>'{}'::jsonb
            OR completion->>'reason' NOT IN ('task_cancelled','monty_input_rejected',
                'monty_admission_backpressure','monty_service_closed','monty_admission_rejected')
            OR NEW.execution_report_bytes IS NOT NULL OR envelope->'reconciliation_refs'<>'[]'::jsonb
            OR admission.started_at IS NOT NULL OR admission.outcome->>'status'<>'failed'
            OR admission.outcome->>'reason_kind' IS DISTINCT FROM completion->>'reason'
            OR EXISTS(SELECT 1 FROM brassclaw_monty_recipe_selections WHERE run_id=NEW.run_id)
            OR EXISTS(SELECT 1 FROM brassclaw_monty_tool_invocations WHERE run_id=NEW.run_id) THEN
            RAISE EXCEPTION 'private unsubmitted original mismatch' USING ERRCODE='23514';
        END IF;
    ELSIF completion->>'completion_origin'='native_receipt' THEN
        IF (completion ?& ARRAY['completion_origin','root','outcome','accounting','execution_report_checksum']
            AND completion-ARRAY['completion_origin','root','outcome','accounting','execution_report_checksum']='{}'::jsonb
            AND NEW.execution_report_bytes IS NOT NULL
            AND NEW.execution_report_bytes::jsonb=admission.outcome->'execution'
            AND private_terminal_native_shape(completion,NEW.execution_report_bytes::jsonb)
            AND completion->'execution_report_checksum'=
                (SELECT jsonb_agg(get_byte(sha256(convert_to(NEW.execution_report_bytes,'UTF8')),n) ORDER BY n)
                    FROM generate_series(0,31)n)) IS NOT TRUE THEN
            RAISE EXCEPTION 'private native report mismatch' USING ERRCODE='23514';
        END IF;
        IF completion->'outcome'->>'status'='internal_completed' THEN
            IF ((completion->'outcome')-'status'-'receipt_checksum'='{}'::jsonb
                AND admission.outcome->>'status'='completed'
                AND completion->'outcome'->'receipt_checksum'=
                    (SELECT jsonb_agg(get_byte(sha256(convert_to(admission.outcome->>'internal_receipt','UTF8')),n) ORDER BY n)
                        FROM generate_series(0,31)n)) IS NOT TRUE THEN
                RAISE EXCEPTION 'private native receipt mismatch' USING ERRCODE='23514';
            END IF;
        ELSIF completion->'outcome'->>'status'='failed' THEN
            IF ((completion->'outcome')-'status'-'reason_kind'='{}'::jsonb
                AND admission.outcome->>'status'='failed'
                AND completion->'outcome'->>'reason_kind'=admission.outcome->>'reason_kind') IS NOT TRUE THEN
                RAISE EXCEPTION 'private native failure mismatch' USING ERRCODE='23514';
            END IF;
        ELSE RAISE EXCEPTION 'private terminal outcome is invalid' USING ERRCODE='23514';
        END IF;
        SELECT count(*) INTO actual_refs FROM brassclaw_monty_tool_invocations WHERE run_id=NEW.run_id;
        IF actual_refs<>jsonb_array_length(envelope->'reconciliation_refs')
            OR actual_refs<>(SELECT count(DISTINCT value->>'invocation_id') FROM jsonb_array_elements(envelope->'reconciliation_refs')) OR EXISTS(
            SELECT 1 FROM brassclaw_monty_tool_invocations WHERE run_id=NEW.run_id AND phase<>'answered') THEN
            RAISE EXCEPTION 'private native custody is unresolved' USING ERRCODE='23514';
        END IF;
        FOR native_ref IN SELECT value FROM jsonb_array_elements(envelope->'reconciliation_refs') LOOP
            IF (native_ref ?& ARRAY['invocation_id','selection_checksum','arguments_checksum','answer_checksum']
                AND native_ref-ARRAY['invocation_id','selection_checksum','arguments_checksum','answer_checksum']='{}'::jsonb
                AND EXISTS(SELECT 1 FROM brassclaw_monty_tool_invocations i WHERE i.run_id=NEW.run_id
                    AND i.invocation_id=(native_ref->>'invocation_id')::uuid AND i.phase='answered'
                    AND native_ref->'selection_checksum'=
                        (SELECT jsonb_agg(get_byte(decode(i.selection_checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n)
                    AND native_ref->'arguments_checksum'=
                        (SELECT jsonb_agg(get_byte(decode(i.arguments_checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n)
                    AND native_ref->'answer_checksum'=
                        (SELECT jsonb_agg(get_byte(decode(i.answer_checksum,'hex'),n) ORDER BY n) FROM generate_series(0,31)n))) IS NOT TRUE THEN
                RAISE EXCEPTION 'private native answer reference mismatch' USING ERRCODE='23514';
            END IF;
        END LOOP;
    ELSE RAISE EXCEPTION 'private completion origin is invalid' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
