-- Original candidate/subject identity only. Work claims, validator selection,
-- approval and dispatch authority remain with their existing owners.
ALTER TABLE reborn_validation_queue ADD COLUMN original_binding_required BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE reborn_validation_queue ALTER COLUMN original_binding_required SET DEFAULT true;

CREATE FUNCTION review_queue_bytes(q reborn_validation_queue) RETURNS text
LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT (to_jsonb(q)-ARRAY['state','validation_errors','updated_at','q2_actor',
 'q1_component_bytes','q1_component_checksum','q1_queue_bytes','q1_queue_checksum']::text[])::text
$$;
CREATE FUNCTION review_component_bytes(c jsonb) RETURNS text
LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT (c-ARRAY['created_at','updated_at','usage_count','success_count','failure_count',
 'wilson_lower','tier','confidence','last_audit_at','audit_failure_count','validation_status',
 'validation_errors','review_feedback','review_attempts','rejected_at','queue_code']::text[])::text
$$;
CREATE FUNCTION review_queue_table(code smallint) RETURNS text
LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT CASE code WHEN 0 THEN 'reborn_tools' WHEN 1 THEN 'reborn_skills'
 WHEN 2 THEN 'reborn_skills' WHEN 3 THEN 'reborn_skills' WHEN 13 THEN 'reborn_tool_skills'
 WHEN 21 THEN 'reborn_recipes' WHEN 22 THEN 'reborn_python_code'
 WHEN 23 THEN 'reborn_extension_catalogues' END
$$;
-- All existing author-controlled projection fields must be supplied explicitly.
-- v3-only metadata remains in the original revision, not synthesized here.
CREATE FUNCTION review_queue_fields(code smallint) RETURNS text[]
LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT ARRAY['name','description','consumer_tags']::text[] ||
 CASE code WHEN 0 THEN ARRAY['param_schema','param_template','effect_type','preconditions','error_handling','capability_id']
 ELSE ARRAY['prior_knowledge_content','override_prompt_creation','intent_examples','dependency_registry'] ||
 CASE WHEN code BETWEEN 1 AND 3 THEN ARRAY['body']
 WHEN code=13 THEN ARRAY['content','tool_name','param_schema','param_template','includes']
 WHEN code=21 THEN ARRAY['trigger','steps','step_descriptions','variants','validates_class_code']
 WHEN code=22 THEN ARRAY['content','includes']
 WHEN code=23 THEN ARRAY['version','overview_doc','task_groups','child_component_ids','intent_index']
 ELSE ARRAY[]::text[] END END
$$;
CREATE FUNCTION review_queue_projection(code smallint, document jsonb) RETURNS jsonb
LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT jsonb_object_agg(k,document->k) FROM unnest(review_queue_fields(code)) k
$$;
CREATE FUNCTION review_queue_proposed(code smallint, component jsonb, proposal jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE result jsonb := component; k text; body text;
BEGIN
 IF proposal IS NOT NULL THEN
   IF jsonb_typeof(proposal)<>'object' OR proposal='{}'::jsonb
     OR EXISTS(SELECT 1 FROM jsonb_object_keys(proposal) key WHERE key NOT IN ('name','description','content')) THEN
     RAISE EXCEPTION 'invalid queued proposal' USING ERRCODE='23514';
   END IF;
   FOREACH k IN ARRAY ARRAY['name','description'] LOOP
     IF proposal ? k THEN
       IF jsonb_typeof(proposal->k)<>'string' THEN RAISE EXCEPTION 'invalid queued field' USING ERRCODE='23514'; END IF;
       result := jsonb_set(result,ARRAY[k],proposal->k);
     END IF;
   END LOOP;
   IF proposal ? 'content' THEN
     body := CASE WHEN code BETWEEN 1 AND 3 THEN 'body' WHEN code=21 THEN 'steps'
       WHEN code=23 THEN 'overview_doc' WHEN code IN (13,22) THEN 'content' END;
     IF body IS NULL OR proposal->'content'='null'::jsonb
       OR (code<>21 AND jsonb_typeof(proposal->'content')<>'string') THEN
       RAISE EXCEPTION 'invalid queued content' USING ERRCODE='23514';
     END IF;
     result := jsonb_set(result,ARRAY[body],proposal->'content');
   END IF;
 END IF;
 RETURN result;
END $$;

CREATE TABLE reborn_component_review_queue_bindings (
 queue_id UUID PRIMARY KEY,
 submission_id UUID NOT NULL REFERENCES reborn_component_review_submissions(submission_id),
 component_id UUID NOT NULL,
 binding_bytes TEXT NOT NULL CHECK(octet_length(binding_bytes) BETWEEN 1 AND 67108864),
 checksum TEXT NOT NULL CHECK(checksum=encode(sha256(convert_to(binding_bytes,'UTF8')),'hex')),
 UNIQUE(submission_id,component_id)
);
CREATE FUNCTION verify_original_queue_binding() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE b jsonb := NEW.binding_bytes::jsonb; s reborn_component_review_submissions%ROWTYPE;
 q reborn_validation_queue%ROWTYPE; r reborn_component_revisions%ROWTYPE;
 ref jsonb; partner jsonb; base jsonb; document jsonb; current_bytes text; expected jsonb; table_name text;
BEGIN
 IF (b->>'format'='component-review-queue-binding/1' AND (b->>'queue_id')::uuid=NEW.queue_id
   AND (b->>'submission_id')::uuid=NEW.submission_id AND (b->'candidate'->>'uuid')::uuid=NEW.component_id) IS NOT TRUE THEN
   RAISE EXCEPTION 'invalid original review binding' USING ERRCODE='23514';
 END IF;
 SELECT * INTO STRICT s FROM reborn_component_review_submissions WHERE submission_id=NEW.submission_id;
 IF b->>'subject_checksum' IS DISTINCT FROM s.checksum THEN RAISE EXCEPTION 'binding subject mismatch' USING ERRCODE='23514'; END IF;
 ref := s.subject_bytes::jsonb->'candidate'; base := s.subject_bytes::jsonb->'base';
 IF (ref->>'uuid')::uuid<>NEW.component_id THEN
   partner := s.subject_bytes::jsonb->'function_partner';
   IF partner->>'role' IS DISTINCT FROM 'changed' THEN RAISE EXCEPTION 'binding is not a changed root' USING ERRCODE='23514'; END IF;
   ref := partner->'candidate'; base := partner->'base';
 END IF;
 IF b->'candidate' IS DISTINCT FROM ref OR b->'base' IS DISTINCT FROM base THEN
   RAISE EXCEPTION 'binding root or base mismatch' USING ERRCODE='23514';
 END IF;
 SELECT * INTO STRICT r FROM reborn_component_revisions WHERE component_id=NEW.component_id
   AND version=(ref->>'version')::bigint AND class_code=(ref->>'class_code')::smallint AND checksum=ref->>'checksum';
 document := r.revision_bytes::jsonb->'document';
 IF NOT document ?& review_queue_fields(r.class_code) THEN RAISE EXCEPTION 'incomplete legacy projection' USING ERRCODE='23514'; END IF;
 IF review_queue_projection(r.class_code,document) IS DISTINCT FROM
    review_queue_projection(r.class_code,(b->>'reviewed_bytes')::jsonb) THEN
   RAISE EXCEPTION 'candidate differs from queued payload' USING ERRCODE='23514';
 END IF;
 SELECT * INTO STRICT q FROM reborn_validation_queue WHERE id=NEW.queue_id FOR UPDATE;
 IF q.component_id<>NEW.component_id OR q.component_class<>r.class_code OR q.state<>1
   OR NOT q.original_binding_required OR q.q1_component_bytes IS NOT NULL
   OR b->>'queue_bytes' IS DISTINCT FROM review_queue_bytes(q) THEN
   RAISE EXCEPTION 'queue cannot accept an original binding' USING ERRCODE='23514';
 END IF;
 table_name := review_queue_table(q.component_class);
 IF table_name IS NULL THEN RAISE EXCEPTION 'unsupported queue class' USING ERRCODE='23514'; END IF;
 EXECUTE format('SELECT review_component_bytes(to_jsonb(c)) FROM %I c WHERE id=$1 AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5 AND class_code=$6 FOR UPDATE',table_name)
 INTO STRICT current_bytes USING q.component_id,q.tenant_id,q.user_id,q.agent_id,q.project_id,q.component_class;
 expected := review_queue_proposed(q.component_class,current_bytes::jsonb,q.proposed_payload);
 IF base='null'::jsonb THEN
   IF q.proposed_payload IS NOT NULL THEN RAISE EXCEPTION 'new review cannot borrow an upgrade base' USING ERRCODE='23514'; END IF;
 ELSE
   SELECT revision_bytes::jsonb->'document' INTO STRICT document FROM reborn_component_revisions
     WHERE component_id=NEW.component_id AND version=(base->>'version')::bigint
       AND class_code=(base->>'class_code')::smallint AND checksum=base->>'checksum';
   IF NOT document ?& review_queue_fields(q.component_class)
     OR review_queue_projection(q.component_class,document) IS DISTINCT FROM review_queue_projection(q.component_class,current_bytes::jsonb) THEN
     RAISE EXCEPTION 'original base differs from live comparison' USING ERRCODE='23514';
   END IF;
 END IF;
 IF EXISTS(SELECT 1 FROM jsonb_each(r.revision_bytes::jsonb->'document') d
     JOIN jsonb_each(expected) e ON d.key=e.key WHERE d.value IS DISTINCT FROM e.value) THEN
   RAISE EXCEPTION 'shared legacy metadata differs from original revision' USING ERRCODE='23514';
 END IF;
 IF b->>'component_bytes' IS DISTINCT FROM current_bytes OR (b->>'reviewed_bytes')::jsonb IS DISTINCT FROM expected THEN
   RAISE EXCEPTION 'queued candidate changed' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER original_queue_binding_integrity BEFORE INSERT ON reborn_component_review_queue_bindings
 FOR EACH ROW EXECUTE FUNCTION verify_original_queue_binding();
CREATE TRIGGER original_queue_bindings_immutable BEFORE UPDATE OR DELETE ON reborn_component_review_queue_bindings
 FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER original_queue_bindings_no_truncate BEFORE TRUNCATE ON reborn_component_review_queue_bindings
 FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
-- No reference to the mutable queue: Q2 deletion must preserve this original.

CREATE TABLE reborn_component_review_queue_requests (
 submission_id UUID PRIMARY KEY REFERENCES reborn_component_review_submissions(submission_id),
 queue_ids UUID[] NOT NULL,
 request_kind TEXT NOT NULL CHECK(request_kind IN ('standalone','existing_queue','create_queue')),
 CHECK((request_kind='standalone')=(cardinality(queue_ids)=0)),
 CHECK(request_kind<>'create_queue' OR cardinality(queue_ids)=1)
);
CREATE TRIGGER original_queue_requests_immutable BEFORE UPDATE OR DELETE ON reborn_component_review_queue_requests
 FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER original_queue_requests_no_truncate BEFORE TRUNCATE ON reborn_component_review_queue_requests
 FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();

CREATE FUNCTION verify_original_queue_request() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF cardinality(NEW.queue_ids)<>(SELECT count(DISTINCT id) FROM unnest(NEW.queue_ids) id)
   OR EXISTS(SELECT 1 FROM unnest(NEW.queue_ids) id WHERE NOT EXISTS(
     SELECT 1 FROM reborn_component_review_queue_bindings b WHERE b.queue_id=id AND b.submission_id=NEW.submission_id))
   OR EXISTS(SELECT 1 FROM reborn_component_review_queue_bindings b WHERE b.submission_id=NEW.submission_id AND NOT b.queue_id=ANY(NEW.queue_ids)) THEN
   RAISE EXCEPTION 'original queue request has incomplete bindings' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER original_queue_request_complete AFTER INSERT ON reborn_component_review_queue_requests
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION verify_original_queue_request();
CREATE FUNCTION require_original_queue_request() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE subject jsonb; expected integer;
BEGIN
 SELECT subject_bytes::jsonb INTO STRICT subject FROM reborn_component_review_submissions WHERE submission_id=NEW.submission_id;
 expected := CASE WHEN subject->'function_partner'->>'role'='changed' THEN 2 ELSE 1 END;
 IF NOT EXISTS(SELECT 1 FROM reborn_component_review_queue_requests r WHERE r.submission_id=NEW.submission_id
    AND NEW.queue_id=ANY(r.queue_ids) AND cardinality(r.queue_ids)=expected)
   OR (SELECT count(*) FROM reborn_component_review_queue_bindings WHERE submission_id=NEW.submission_id)<>expected THEN
   RAISE EXCEPTION 'all changed roots require one original queue request' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER original_binding_requires_request AFTER INSERT ON reborn_component_review_queue_bindings
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION require_original_queue_request();
