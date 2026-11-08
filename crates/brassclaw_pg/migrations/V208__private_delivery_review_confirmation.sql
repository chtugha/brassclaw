-- Native post-Factory acknowledgement and domain consumption are evidence, not
-- another Work queue, claim authority, Q1 approval or replay permission.
CREATE TABLE brassclaw_monty_private_deliveries (
    work_id UUID PRIMARY KEY,
    run_id UUID NOT NULL UNIQUE,
    terminal_checksum BYTEA NOT NULL,
    delivery_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(delivery_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(work_id,checksum),
    FOREIGN KEY(work_id,run_id) REFERENCES brassclaw_turns(work_id,execution_run_id) ON DELETE RESTRICT,
    FOREIGN KEY(work_id,terminal_checksum) REFERENCES brassclaw_monty_private_terminals(work_id,checksum) ON DELETE RESTRICT
);
CREATE FUNCTION verify_private_delivery_original() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE d jsonb:=NEW.delivery_bytes::jsonb;
BEGIN
    IF (d ?& ARRAY['format','scope','work_id','run_id','terminal_checksum']
        AND d-ARRAY['format','scope','work_id','run_id','terminal_checksum']='{}'::jsonb
        AND d->>'format'='monty-private-delivery/1'
        AND d->>'scope'='native-post-factory-acknowledgement'
        AND (d->>'work_id')::uuid=NEW.work_id AND (d->>'run_id')::uuid=NEW.run_id
        AND d->'terminal_checksum'=(SELECT jsonb_agg(get_byte(NEW.terminal_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        AND EXISTS(SELECT 1 FROM brassclaw_turns w
            JOIN brassclaw_monty_task_admissions a ON a.run_id=w.execution_run_id
            JOIN brassclaw_monty_private_terminals t ON t.work_id=w.work_id AND t.run_id=a.run_id
            WHERE w.record_kind='private_work' AND w.work_id=NEW.work_id AND w.execution_run_id=NEW.run_id
                AND w.status='settled' AND a.phase='settled' AND a.origin_kind='private_work'
                AND w.private_cancel_revision=0 AND w.private_terminal_checksum=NEW.terminal_checksum
                AND t.checksum=NEW.terminal_checksum AND a.private_work_id=w.work_id
                AND t.terminal_bytes::jsonb->'completion'->>'completion_origin'='native_receipt'
                AND t.terminal_bytes::jsonb->'completion'->'outcome'->>'status'='internal_completed'
                AND a.outcome->>'status'='completed'
                AND (t.execution_report_bytes::jsonb->>'root_completed')::boolean
                AND (t.execution_report_bytes::jsonb->>'all_selected_recipes_complete')::boolean
                AND a.outcome->'execution'=t.execution_report_bytes::jsonb)) IS NOT TRUE THEN
        RAISE EXCEPTION 'private delivery requires its successful native original' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER private_delivery_original BEFORE INSERT ON brassclaw_monty_private_deliveries
    FOR EACH ROW EXECUTE FUNCTION verify_private_delivery_original();
CREATE TRIGGER private_deliveries_immutable BEFORE UPDATE OR DELETE ON brassclaw_monty_private_deliveries
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER private_deliveries_no_truncate BEFORE TRUNCATE ON brassclaw_monty_private_deliveries
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();

CREATE TABLE reborn_component_review_work_confirmations (
    submission_id UUID PRIMARY KEY REFERENCES reborn_component_review_requests(submission_id) ON DELETE RESTRICT,
    work_id UUID NOT NULL UNIQUE,
    delivery_checksum BYTEA NOT NULL,
    confirmation_bytes TEXT NOT NULL,
    checksum BYTEA NOT NULL CHECK(checksum=sha256(convert_to(confirmation_bytes,'UTF8'))),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY(work_id,delivery_checksum) REFERENCES brassclaw_monty_private_deliveries(work_id,checksum) ON DELETE RESTRICT
);
CREATE FUNCTION verify_review_work_confirmation() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE c jsonb:=NEW.confirmation_bytes::jsonb;
BEGIN
    IF (c ?& ARRAY['format','scope','submission_id','subject_checksum','work_id','run_id','delivery_checksum',
        'validator_selection_checksum','evidence_id','evidence_checksum','checks_succeeded','errors','queue_bindings',
        'q1_qualified','human_q2','activation']
        AND c-ARRAY['format','scope','submission_id','subject_checksum','work_id','run_id','delivery_checksum',
        'validator_selection_checksum','evidence_id','evidence_checksum','checks_succeeded','errors','queue_bindings',
        'q1_qualified','human_q2','activation']='{}'::jsonb
        AND c->>'format'='component-review-work-confirmation/1' AND c->>'scope'='structural-observations-only'
        AND c->'q1_qualified'='false'::jsonb AND c->'human_q2'='false'::jsonb AND c->'activation'='false'::jsonb
        AND (c->>'submission_id')::uuid=NEW.submission_id AND (c->>'work_id')::uuid=NEW.work_id
        AND c->'delivery_checksum'=(SELECT jsonb_agg(get_byte(NEW.delivery_checksum,n) ORDER BY n) FROM generate_series(0,31)n)
        AND EXISTS(SELECT 1 FROM reborn_component_review_requests r
            JOIN brassclaw_turns w ON w.validator_submission_id=r.submission_id
            JOIN brassclaw_monty_private_deliveries d ON d.work_id=w.work_id
            JOIN reborn_validator_q1_runs v ON v.submission_id=r.submission_id
            JOIN reborn_component_review_evidence e ON e.evidence_id=v.evidence_id
            WHERE r.submission_id=NEW.submission_id AND r.subject_checksum=c->>'subject_checksum'
                AND w.work_id=NEW.work_id AND w.execution_run_id=(c->>'run_id')::uuid
                AND d.checksum=NEW.delivery_checksum AND w.private_cancel_revision=0
                AND v.validator_selection_checksum=c->>'validator_selection_checksum'
                AND v.evidence_id=(c->>'evidence_id')::uuid AND e.checksum=c->>'evidence_checksum'
                AND to_jsonb(v.succeeded)=c->'checks_succeeded'
                AND c->'errors'=COALESCE((SELECT jsonb_agg(
                    to_jsonb((x.rule->>'rule_id')||': '||(y.message #>> '{}')) ORDER BY x.ordinal,y.ordinal)
                    FROM jsonb_array_elements(e.evidence_bytes::jsonb->'report'->'aggregation'->'errors') WITH ORDINALITY x(rule,ordinal)
                    CROSS JOIN LATERAL jsonb_array_elements(x.rule->'errors') WITH ORDINALITY y(message,ordinal)), '[]'::jsonb)
                AND c->'queue_bindings'=(SELECT COALESCE(jsonb_agg(jsonb_build_object(
                    'queue_id',b.queue_id,'checksum',b.checksum) ORDER BY ids.ordinal),'[]'::jsonb)
                    FROM reborn_component_review_queue_requests q
                    CROSS JOIN LATERAL unnest(q.queue_ids) WITH ORDINALITY ids(id,ordinal)
                    JOIN reborn_component_review_queue_bindings b ON b.queue_id=ids.id AND b.submission_id=q.submission_id
                    WHERE q.submission_id=r.submission_id))) IS NOT TRUE THEN
        RAISE EXCEPTION 'review confirmation differs from delivered original observations' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER review_work_confirmation_original BEFORE INSERT ON reborn_component_review_work_confirmations
    FOR EACH ROW EXECUTE FUNCTION verify_review_work_confirmation();
CREATE TRIGGER review_work_confirmations_immutable BEFORE UPDATE OR DELETE ON reborn_component_review_work_confirmations
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER review_work_confirmations_no_truncate BEFORE TRUNCATE ON reborn_component_review_work_confirmations
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
CREATE INDEX component_review_requests_discovery ON reborn_component_review_requests(requested_at,submission_id);
