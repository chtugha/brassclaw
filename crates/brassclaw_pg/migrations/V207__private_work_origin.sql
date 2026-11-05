-- B-02: the existing turn store remains the single durable claim authority.
-- Private work has no accepted message, transcript, thread alias or chat reply.
-- Historical payloads, review records and started/unknown admissions are untouched.
ALTER TABLE brassclaw_turns
    ADD COLUMN record_kind TEXT NOT NULL DEFAULT 'legacy',
    ALTER COLUMN turn_id DROP NOT NULL,
    ADD COLUMN work_id UUID UNIQUE,
    ADD COLUMN execution_run_id UUID UNIQUE,
    ADD COLUMN owner_snapshot_id TEXT REFERENCES brassclaw_turns(id) ON DELETE RESTRICT,
    ADD COLUMN private_request_bytes TEXT,
    ADD COLUMN private_request_checksum BYTEA,
    ADD COLUMN validator_submission_id UUID
        REFERENCES reborn_component_review_submissions(submission_id) ON DELETE RESTRICT
        REFERENCES reborn_component_review_queue_requests(submission_id) ON DELETE RESTRICT,
    ADD COLUMN prefix_job_id UUID REFERENCES brassclaw_prefix_job_definitions(job_id) ON DELETE RESTRICT,
    ADD COLUMN private_definition_checksum BYTEA,
    ADD COLUMN preparation_reason TEXT,
    ADD COLUMN ready_at TIMESTAMPTZ,
    ADD COLUMN private_runner_id UUID,
    ADD COLUMN private_claim_checksum BYTEA,
    ADD COLUMN private_claim_generation BIGINT,
    ADD COLUMN private_claim_attempts BIGINT,
    ADD COLUMN private_lease_deadline TIMESTAMPTZ,
    ADD COLUMN private_cancel_revision BIGINT,
    ADD COLUMN private_cancel_requested_at TIMESTAMPTZ,
    ADD COLUMN private_admission_run UUID REFERENCES brassclaw_monty_task_admissions(run_id)
        DEFERRABLE INITIALLY DEFERRED,
    ADD COLUMN private_terminal_checksum BYTEA;

CREATE UNIQUE INDEX private_validator_original_unique ON brassclaw_turns(validator_submission_id)
    WHERE record_kind='private_work';
CREATE UNIQUE INDEX private_prefix_original_unique ON brassclaw_turns(prefix_job_id)
    WHERE record_kind='private_work';
CREATE INDEX private_work_ready ON brassclaw_turns(tenant_id,ready_at,work_id)
    WHERE record_kind='private_work' AND status='ready';

-- Immutable bytes retained by the composition preparation owner, not a queue:
-- no status, runner, scheduling, policy decision or independent claim authority.
CREATE TABLE brassclaw_monty_private_preparations (
    work_id UUID PRIMARY KEY REFERENCES brassclaw_turns(work_id) ON DELETE RESTRICT,
    definition_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(definition_bytes,'UTF8'))),
    UNIQUE(work_id,checksum),
    CHECK((jsonb_typeof(definition_bytes::jsonb)='object') IS TRUE),
    CHECK((definition_bytes::jsonb->>'format'='private-work-definition/1') IS TRUE),
    CHECK(((definition_bytes::jsonb->>'work_id')::uuid=work_id) IS TRUE)
);
CREATE TRIGGER private_preparations_immutable BEFORE UPDATE OR DELETE
    ON brassclaw_monty_private_preparations FOR EACH ROW
    EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER private_preparations_no_truncate BEFORE TRUNCATE
    ON brassclaw_monty_private_preparations FOR EACH STATEMENT
    EXECUTE FUNCTION protect_component_review_records();
ALTER TABLE brassclaw_turns ADD CONSTRAINT private_work_preparation_original
    FOREIGN KEY(work_id,private_definition_checksum)
    REFERENCES brassclaw_monty_private_preparations(work_id,checksum) ON DELETE RESTRICT;

ALTER TABLE brassclaw_turns ADD CONSTRAINT turn_record_kind_shape CHECK ((
    record_kind IN ('legacy','private_work','chat_run_address')
    AND CASE record_kind
    WHEN 'legacy' THEN turn_id IS NOT NULL AND work_id IS NULL AND execution_run_id IS NULL
        AND owner_snapshot_id IS NULL AND private_request_bytes IS NULL
        AND private_request_checksum IS NULL AND validator_submission_id IS NULL
        AND prefix_job_id IS NULL AND private_definition_checksum IS NULL
        AND preparation_reason IS NULL AND ready_at IS NULL AND private_runner_id IS NULL
        AND private_claim_checksum IS NULL AND private_claim_generation IS NULL
        AND private_claim_attempts IS NULL AND private_lease_deadline IS NULL
        AND private_cancel_revision IS NULL AND private_cancel_requested_at IS NULL
        AND private_admission_run IS NULL AND private_terminal_checksum IS NULL
    WHEN 'chat_run_address' THEN turn_id IS NULL AND run_id IS NULL AND work_id IS NULL
        AND execution_run_id IS NOT NULL AND owner_snapshot_id IS NOT NULL
        AND status='address' AND payload='{}'::jsonb AND version=0
        AND private_request_bytes IS NULL AND private_request_checksum IS NULL
        AND validator_submission_id IS NULL AND prefix_job_id IS NULL
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
        AND (validator_submission_id IS NOT NULL)<>(prefix_job_id IS NOT NULL)
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

-- Run IDs are unique before admission, including runs embedded in chat JSON.
-- Index rows reference the actual snapshot STORAGE PK, never a descendant's
-- ThreadId. They are wholly derived and carry no lifecycle authority.
CREATE FUNCTION verify_chat_run_address() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM brassclaw_turns s WHERE s.id=NEW.owner_snapshot_id
        AND s.record_kind='legacy' AND s.status='snapshot' AND s.tenant_id=NEW.tenant_id
        AND s.payload->'runs' @> jsonb_build_array(jsonb_build_object('run_id',NEW.execution_run_id::text))) THEN
        RAISE EXCEPTION 'chat run address does not name its original snapshot' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER chat_run_address_original BEFORE INSERT OR UPDATE ON brassclaw_turns
    FOR EACH ROW WHEN(NEW.record_kind='chat_run_address') EXECUTE FUNCTION verify_chat_run_address();

CREATE FUNCTION retain_chat_run_addresses() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE run jsonb; addressed integer;
BEGIN
    FOR run IN SELECT value FROM jsonb_array_elements(NEW.payload->'runs') LOOP
        INSERT INTO brassclaw_turns(id,tenant_id,turn_id,status,payload,version,
            record_kind,execution_run_id,owner_snapshot_id)
        VALUES('chat-run-address:'||(run->>'run_id'),NEW.tenant_id,NULL,'address','{}',0,
            'chat_run_address',(run->>'run_id')::uuid,NEW.id)
        ON CONFLICT(execution_run_id) DO UPDATE SET owner_snapshot_id=EXCLUDED.owner_snapshot_id
            WHERE brassclaw_turns.record_kind='chat_run_address'
                AND brassclaw_turns.owner_snapshot_id=EXCLUDED.owner_snapshot_id
                AND brassclaw_turns.tenant_id=EXCLUDED.tenant_id;
        GET DIAGNOSTICS addressed=ROW_COUNT;
        IF addressed<>1 THEN
            RAISE EXCEPTION 'execution run id belongs to a different original' USING ERRCODE='23505';
        END IF;
    END LOOP;
    RETURN NEW;
END $$;
CREATE TRIGGER chat_run_addresses AFTER INSERT OR UPDATE OF payload ON brassclaw_turns
    FOR EACH ROW WHEN(NEW.record_kind='legacy' AND NEW.status='snapshot')
    EXECUTE FUNCTION retain_chat_run_addresses();
-- No historical payload rewrite. Any pre-existing duplicate across originals
-- fails this insertion; neither the first nor the newest row is elected winner.
INSERT INTO brassclaw_turns(id,tenant_id,turn_id,status,payload,version,
    record_kind,execution_run_id,owner_snapshot_id)
SELECT 'chat-run-address:'||(r->>'run_id'),s.tenant_id,NULL,'address','{}',0,
    'chat_run_address',(r->>'run_id')::uuid,s.id
FROM brassclaw_turns s CROSS JOIN LATERAL jsonb_array_elements(s.payload->'runs') r
WHERE s.record_kind='legacy' AND s.status='snapshot';

CREATE FUNCTION protect_private_work_origin() RETURNS trigger LANGUAGE plpgsql AS $$
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
        AND req->'context'-'tenant_id'-'agent_id'-'project_id'-'actor'='{}'::jsonb
        AND req->'context' ?& ARRAY['tenant_id','agent_id','project_id','actor']
        AND jsonb_typeof(req->'context')='object'
        AND jsonb_typeof(req->'context'->'tenant_id')='string'
        AND jsonb_typeof(req->'context'->'agent_id') IN ('null','string')
        AND jsonb_typeof(req->'context'->'project_id') IN ('null','string')
        AND (req->'context'->'actor'='null'::jsonb OR
            (jsonb_typeof(req->'context'->'actor')='object'
             AND req->'context'->'actor'-'user_id'='{}'::jsonb
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
CREATE TRIGGER private_work_origin_guard BEFORE INSERT OR UPDATE OR DELETE ON brassclaw_turns
    FOR EACH ROW EXECUTE FUNCTION protect_private_work_origin();
CREATE TRIGGER private_work_no_truncate BEFORE TRUNCATE ON brassclaw_turns
    FOR EACH STATEMENT EXECUTE FUNCTION protect_private_work_origin();

-- Exhaustive origin. Nullable chat columns are legal only for genuine Private;
-- existing chat admissions retain their exact mandatory identity.
ALTER TABLE brassclaw_turns ADD CONSTRAINT private_work_run_identity UNIQUE(work_id,execution_run_id);
ALTER TABLE brassclaw_monty_task_admissions
    ADD COLUMN origin_kind TEXT NOT NULL DEFAULT 'legacy_chat',
    ADD COLUMN private_work_id UUID,
    ADD COLUMN private_execution_context JSONB,
    ADD COLUMN private_request_checksum BYTEA,
    ADD COLUMN private_claim_generation BIGINT,
    ADD COLUMN private_submission_intent_at TIMESTAMPTZ,
    ALTER COLUMN turn_id DROP NOT NULL,
    ALTER COLUMN scope DROP NOT NULL,
    ALTER COLUMN accepted_message_ref DROP NOT NULL,
    ADD CONSTRAINT monty_private_work_identity FOREIGN KEY(private_work_id,run_id)
        REFERENCES brassclaw_turns(work_id,execution_run_id) ON DELETE RESTRICT,
    ADD CONSTRAINT monty_admission_origin_shape CHECK((
        (origin_kind='legacy_chat' AND turn_id IS NOT NULL AND scope IS NOT NULL
            AND jsonb_typeof(scope)='object' AND accepted_message_ref IS NOT NULL
            AND private_work_id IS NULL AND private_execution_context IS NULL
            AND private_request_checksum IS NULL AND private_claim_generation IS NULL
            AND private_submission_intent_at IS NULL)
        OR (origin_kind='private_work' AND turn_id IS NULL AND scope IS NULL AND accepted_message_ref IS NULL
            AND private_work_id IS NOT NULL AND private_execution_context IS NOT NULL
            AND jsonb_typeof(private_execution_context)='object'
            AND octet_length(private_request_checksum)=32 AND private_claim_generation=1
            AND NOT trusted_internal_turn AND source_binding_ref IS NULL)
    ) IS TRUE);
ALTER TABLE brassclaw_monty_task_admissions DROP COLUMN result_only_completion;
ALTER TABLE brassclaw_monty_task_admissions ADD COLUMN result_only_completion BOOLEAN
    GENERATED ALWAYS AS (origin_kind='private_work' OR trusted_internal_turn
        OR COALESCE(source_binding_ref='brassclaw-internal-model-work',false)) STORED NOT NULL;

CREATE FUNCTION protect_private_admission_origin() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE work brassclaw_turns%ROWTYPE;
BEGIN
    IF NEW.origin_kind<>'private_work' THEN RETURN NEW; END IF;
    IF TG_OP='INSERT' THEN
        SELECT * INTO STRICT work FROM brassclaw_turns WHERE work_id=NEW.private_work_id FOR UPDATE;
        IF work.record_kind<>'private_work' OR work.status<>'claimed'
            OR work.execution_run_id<>NEW.run_id OR work.private_runner_id<>NEW.runner_id
            OR work.private_claim_checksum IS DISTINCT FROM NEW.claim_checksum
            OR work.private_request_checksum IS DISTINCT FROM NEW.private_request_checksum
            OR work.private_claim_generation<>NEW.private_claim_generation
            OR work.private_request_bytes::jsonb->'context' IS DISTINCT FROM NEW.private_execution_context
            OR NEW.native_actor IS DISTINCT FROM NEW.private_execution_context->'actor'
            OR work.private_cancel_revision<>0 OR work.private_lease_deadline<=clock_timestamp()
            OR NEW.private_submission_intent_at IS NOT NULL THEN
            RAISE EXCEPTION 'private admission requires its current original work claim' USING ERRCODE='23514';
        END IF;
    ELSIF OLD.private_submission_intent_at IS DISTINCT FROM NEW.private_submission_intent_at THEN
        IF OLD.phase<>'reserved' OR NEW.phase<>'reserved'
            OR OLD.private_submission_intent_at IS NOT NULL OR NEW.private_submission_intent_at IS NULL THEN
            RAISE EXCEPTION 'private submission intent is monotonic' USING ERRCODE='23514';
        END IF;
        SELECT * INTO STRICT work FROM brassclaw_turns WHERE work_id=NEW.private_work_id FOR UPDATE;
        IF work.status<>'claimed' OR work.private_cancel_revision<>0
            OR work.private_lease_deadline<=clock_timestamp()
            OR work.private_claim_checksum IS DISTINCT FROM NEW.claim_checksum THEN
            RAISE EXCEPTION 'private submission intent is fenced' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER private_admission_origin BEFORE INSERT OR UPDATE ON brassclaw_monty_task_admissions
    FOR EACH ROW EXECUTE FUNCTION protect_private_admission_origin();

-- Keep every existing V179/V191 pin/phase/settlement guard. Permit only the
-- separately guarded private submission-intent transition in its identity view.
DO $$
DECLARE definition TEXT; matches INTEGER;
BEGIN
    definition:=pg_get_functiondef('protect_monty_task_admissions()'::regprocedure);
    matches:=(length(definition)-length(replace(definition,'''result_only_completion''','')))
        /length('''result_only_completion''');
    IF matches<>4 OR position('private_submission_intent_at' IN definition)<>0 THEN
        RAISE EXCEPTION 'Unexpected admission guard; explicit upgrade required' USING ERRCODE='23514';
    END IF;
    definition:=replace(definition,'''result_only_completion''',
        'ARRAY[''result_only_completion'',''private_submission_intent_at'']');
    EXECUTE definition;
END $$;

-- Prefix definition v2 is genuinely private; v1 retains its historical chat
-- columns and exact meaning. No fake thread, empty link or rewritten old bytes.
ALTER TABLE brassclaw_prefix_job_definitions
    ADD COLUMN definition_format TEXT NOT NULL DEFAULT 'prefix-job-definition/1',
    ADD COLUMN private_context JSONB,
    ADD COLUMN original_preparation_bytes TEXT,
    ADD COLUMN original_preparation_checksum TEXT,
    ALTER COLUMN scope DROP NOT NULL,
    ALTER COLUMN thread_scope DROP NOT NULL,
    ADD CONSTRAINT prefix_definition_origin_shape CHECK((
        (definition_format='prefix-job-definition/1' AND scope IS NOT NULL AND thread_scope IS NOT NULL
            AND private_context IS NULL AND original_preparation_bytes IS NULL
            AND original_preparation_checksum IS NULL)
        OR (definition_format='prefix-job-definition/2' AND scope IS NULL AND thread_scope IS NULL
            AND profile_name IS NOT NULL AND profile_revision IS NOT NULL
            AND operation='generate' AND mode='source_only'
            AND private_context IS NOT NULL AND jsonb_typeof(private_context)='object'
            AND original_preparation_bytes IS NOT NULL AND original_preparation_checksum IS NOT NULL
            AND original_preparation_checksum=encode(sha256(convert_to(original_preparation_bytes,'UTF8')),'hex')
            AND original_preparation_bytes::jsonb->>'format'='task-preparation-capture/1'
            AND (original_preparation_bytes::jsonb->>'revision')::bigint>0
            AND private_context->'actor'->>'user_id'=created_by)
    ) IS TRUE);

CREATE FUNCTION verify_private_work_admission_link() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_work brassclaw_turns%ROWTYPE; admission brassclaw_monty_task_admissions%ROWTYPE;
BEGIN
    -- Deferred trigger uses the final version, not an intermediate CAS image.
    SELECT * INTO STRICT current_work FROM brassclaw_turns WHERE work_id=NEW.work_id;
    IF current_work.status NOT IN ('claimed','reconciliation_required','settled') THEN RETURN NULL; END IF;
    SELECT * INTO STRICT admission FROM brassclaw_monty_task_admissions WHERE run_id=current_work.execution_run_id;
    IF admission.origin_kind<>'private_work' OR admission.private_work_id<>current_work.work_id
        OR admission.claim_checksum IS DISTINCT FROM current_work.private_claim_checksum
        OR admission.private_request_checksum IS DISTINCT FROM current_work.private_request_checksum
        OR admission.runner_id<>current_work.private_runner_id
        OR admission.private_claim_generation<>current_work.private_claim_generation
        OR admission.private_execution_context IS DISTINCT FROM current_work.private_request_bytes::jsonb->'context'
        OR ((current_work.status='settled')<>(admission.phase='settled')) THEN
        RAISE EXCEPTION 'private claim and original admission must commit coherently' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER private_work_admission_link AFTER INSERT OR UPDATE ON brassclaw_turns
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN(NEW.record_kind='private_work')
    EXECUTE FUNCTION verify_private_work_admission_link();

-- Immutable native terminal bytes, distinct from the non-deserializable live
-- SettledInternalCompletion proof. This table is evidence, never a work queue.
CREATE TABLE brassclaw_monty_private_terminals (
    work_id UUID PRIMARY KEY,
    run_id UUID NOT NULL UNIQUE REFERENCES brassclaw_monty_task_admissions(run_id) ON DELETE RESTRICT,
    terminal_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(terminal_bytes,'UTF8'))),
    execution_report_bytes TEXT,
    UNIQUE(work_id,checksum),
    FOREIGN KEY(work_id,run_id) REFERENCES brassclaw_turns(work_id,execution_run_id) ON DELETE RESTRICT
);
CREATE TRIGGER private_terminals_immutable BEFORE UPDATE OR DELETE ON brassclaw_monty_private_terminals
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER private_terminals_no_truncate BEFORE TRUNCATE ON brassclaw_monty_private_terminals
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
ALTER TABLE brassclaw_turns ADD CONSTRAINT private_work_terminal_original
    FOREIGN KEY(work_id,private_terminal_checksum) REFERENCES brassclaw_monty_private_terminals(work_id,checksum)
    ON DELETE RESTRICT;

CREATE FUNCTION verify_private_terminal_original() RETURNS trigger LANGUAGE plpgsql AS $$
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
            IF (completion->'outcome'-'status'-'receipt_checksum'='{}'::jsonb
                AND admission.outcome->>'status'='completed'
                AND completion->'outcome'->'receipt_checksum'=
                    (SELECT jsonb_agg(get_byte(sha256(convert_to(admission.outcome->>'internal_receipt','UTF8')),n) ORDER BY n)
                        FROM generate_series(0,31)n)) IS NOT TRUE THEN
                RAISE EXCEPTION 'private native receipt mismatch' USING ERRCODE='23514';
            END IF;
        ELSIF completion->'outcome'->>'status'='failed' THEN
            IF (completion->'outcome'-'status'-'reason_kind'='{}'::jsonb
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
CREATE TRIGGER private_terminal_original BEFORE INSERT ON brassclaw_monty_private_terminals
    FOR EACH ROW EXECUTE FUNCTION verify_private_terminal_original();

-- The same final-state invariant is checked when only Admission was touched.
CREATE FUNCTION verify_private_admission_work_link() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE work brassclaw_turns%ROWTYPE; admission brassclaw_monty_task_admissions%ROWTYPE;
BEGIN
    SELECT * INTO STRICT work FROM brassclaw_turns WHERE work_id=NEW.private_work_id;
    SELECT * INTO STRICT admission FROM brassclaw_monty_task_admissions WHERE run_id=NEW.run_id;
    IF work.record_kind<>'private_work' OR work.status NOT IN ('claimed','reconciliation_required','settled')
        OR work.execution_run_id<>admission.run_id OR work.private_admission_run<>admission.run_id
        OR work.private_runner_id<>admission.runner_id
        OR work.private_claim_checksum IS DISTINCT FROM admission.claim_checksum
        OR work.private_request_checksum IS DISTINCT FROM admission.private_request_checksum
        OR work.private_claim_generation<>admission.private_claim_generation
        OR ((work.status='settled')<>(admission.phase='settled')) THEN
        RAISE EXCEPTION 'private Work and Admission final state differ' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER private_admission_work_link AFTER INSERT OR UPDATE ON brassclaw_monty_task_admissions
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW WHEN(NEW.origin_kind='private_work')
    EXECUTE FUNCTION verify_private_admission_work_link();

ALTER TABLE brassclaw_prefix_job_definitions ADD CONSTRAINT private_prefix_original_input_limits CHECK((
    definition_format<>'prefix-job-definition/2'
    OR (original_input_limits IS NOT NULL AND original_input_limits=jsonb_build_object(
        'max_depth',((original_preparation_bytes::jsonb->>'execution_limits_bytes')::jsonb->'max_value_depth'),
        'max_nodes',((original_preparation_bytes::jsonb->>'execution_limits_bytes')::jsonb->'max_value_nodes'),
        'max_bytes',((original_preparation_bytes::jsonb->>'execution_limits_bytes')::jsonb->'max_value_bytes')))
) IS TRUE);

CREATE FUNCTION verify_private_preparation_original() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE envelope jsonb:=NEW.definition_bytes::jsonb; work brassclaw_turns%ROWTYPE;
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
        IF (envelope->'domain'-'purpose'-'job_id'='{}'::jsonb
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
            AND envelope->'domain'-'purpose'-'capture_bytes'-'capture_checksum'='{}'::jsonb
            AND envelope->'domain'->'capture_checksum'=
                (SELECT jsonb_agg(get_byte(sha256(convert_to(envelope->'domain'->>'capture_bytes','UTF8')),n) ORDER BY n)
                    FROM generate_series(0,31)n)) IS NOT TRUE THEN
            RAISE EXCEPTION 'private validator input capture mismatch' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER private_preparation_original BEFORE INSERT ON brassclaw_monty_private_preparations
    FOR EACH ROW EXECUTE FUNCTION verify_private_preparation_original();

CREATE FUNCTION private_work_hash_value(value JSONB) RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT CASE WHEN jsonb_typeof(value)='array' THEN
        jsonb_array_length(value)=32 AND NOT EXISTS(
            SELECT 1 FROM jsonb_array_elements(value) element WHERE
                (jsonb_typeof(element)='number' AND element::text ~ '^(0|[1-9][0-9]*)$'
                    AND (element::text)::numeric<=255) IS NOT TRUE)
        ELSE false END
$$;
CREATE FUNCTION private_terminal_native_shape(value JSONB, report JSONB) RETURNS BOOLEAN
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
DECLARE root jsonb:=value->'root'; account jsonb:=value->'accounting'; time jsonb;
BEGIN
    IF (root ?& ARRAY['vm_id','source_checksum','aliases_checksum','workers']
        AND root-'vm_id'-'source_checksum'-'aliases_checksum'-'workers'='{}'::jsonb
        AND (root->>'vm_id')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND (root->>'workers')::numeric BETWEEN 1 AND 4294967295
        AND root->'vm_id'=report->'root'->'vm_id'
        AND root->'workers'=report->'root'->'workers'
        AND private_work_hash_value(root->'source_checksum')
        AND private_work_hash_value(root->'aliases_checksum')
        AND root->'source_checksum'=(SELECT jsonb_agg(get_byte(decode(report->'root'->>'source_checksum','hex'),n) ORDER BY n)
            FROM generate_series(0,31)n)
        AND root->'aliases_checksum'=(SELECT jsonb_agg(get_byte(decode(report->'root'->>'aliases_checksum','hex'),n) ORDER BY n)
            FROM generate_series(0,31)n)) IS NOT TRUE THEN RETURN false; END IF;
    IF account='null'::jsonb THEN
        RETURN report->'accounting'='null'::jsonb AND value->'outcome'->>'status'='failed';
    END IF;
    IF (account ?& ARRAY['task','effective_revision','compute_time','failure']
        AND account-'task'-'effective_revision'-'compute_time'-'failure'='{}'::jsonb
        AND (account->>'task')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
        AND (account->>'effective_revision')::numeric BETWEEN 1 AND 18446744073709551615
        AND account->'effective_revision'=report->'accounting'->'effective_revision'
        AND jsonb_typeof(account->'failure') IN ('null','string')
        AND account->'failure'=report->'accounting'->'failure') IS NOT TRUE THEN RETURN false; END IF;
    time:=account->'compute_time';
    IF time='null'::jsonb THEN
        RETURN report->'accounting'->'compute_time'='null'::jsonb AND value->'outcome'->>'status'='failed';
    END IF;
    IF (time ?& ARRAY['seconds','nanoseconds'] AND time-'seconds'-'nanoseconds'='{}'::jsonb
        AND (time->>'seconds')::numeric BETWEEN 0 AND 18446744073709551615
        AND (time->>'nanoseconds')::numeric BETWEEN 0 AND 999999999
        AND time->'seconds'=report->'accounting'->'compute_time'->'secs'
        AND time->'nanoseconds'=report->'accounting'->'compute_time'->'nanos') IS NOT TRUE THEN RETURN false; END IF;
    IF value->'outcome'->>'status'='internal_completed' AND account->'failure'<>'null'::jsonb THEN RETURN false; END IF;
    RETURN true;
END $$;
