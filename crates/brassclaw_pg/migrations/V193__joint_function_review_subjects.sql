-- A function subject retains changed Skill/code roots atomically in the existing
-- review submission and V149 intent. It grants no Q1/Q2, execution or activation.
-- Preserve all original /1 and /2 bytes and meanings.
ALTER TABLE reborn_component_review_submissions
    DROP CONSTRAINT review_subject_allocation,
    ADD CONSTRAINT review_subject_allocation CHECK (
        CASE subject_bytes::jsonb->>'format'
        WHEN 'component-review-submission/1' THEN
            NOT (subject_bytes::jsonb ? 'expected_previous_version')
            AND NOT (subject_bytes::jsonb ? 'function_partner')
            AND candidate_version::numeric=COALESCE(base_version::numeric,0)+1
        WHEN 'component-review-submission/2' THEN
            NOT (subject_bytes::jsonb ? 'function_partner')
            AND CASE WHEN jsonb_typeof(subject_bytes::jsonb->'expected_previous_version')='number'
                AND subject_bytes::jsonb->>'expected_previous_version' ~ '^(0|[1-9][0-9]*)$'
            THEN (subject_bytes::jsonb->>'expected_previous_version')::numeric<9223372036854775807
                AND candidate_version::numeric=(subject_bytes::jsonb->>'expected_previous_version')::numeric+1
                AND COALESCE(base_version::numeric,0)<=(subject_bytes::jsonb->>'expected_previous_version')::numeric
            ELSE false END
        WHEN 'component-review-submission/3' THEN
            jsonb_typeof(subject_bytes::jsonb->'function_partner')='object'
            AND CASE WHEN jsonb_typeof(subject_bytes::jsonb->'expected_previous_version')='number'
                AND subject_bytes::jsonb->>'expected_previous_version' ~ '^(0|[1-9][0-9]*)$'
            THEN (subject_bytes::jsonb->>'expected_previous_version')::numeric<9223372036854775807
                AND candidate_version::numeric=(subject_bytes::jsonb->>'expected_previous_version')::numeric+1
                AND COALESCE(base_version::numeric,0)<=(subject_bytes::jsonb->>'expected_previous_version')::numeric
            ELSE false END
        ELSE false END IS TRUE);

CREATE FUNCTION verify_function_review_partner() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    subject jsonb := NEW.subject_bytes::jsonb;
    partner jsonb;
    selected jsonb;
    previous jsonb;
    counterpart reborn_component_revisions%ROWTYPE;
    primary_revision reborn_component_revisions%ROWTYPE;
    skill_bytes text;
    code_id uuid;
    head numeric;
BEGIN
    IF subject->>'format' <> 'component-review-submission/3' THEN RETURN NEW; END IF;
    partner := subject->'function_partner';
    IF partner->>'role' = 'changed' THEN
        selected := partner->'candidate';
    ELSIF partner->>'role' = 'context' THEN
        selected := partner->'reference';
    ELSE
        RAISE EXCEPTION 'invalid function partner role' USING ERRCODE='23514';
    END IF;
    IF (jsonb_typeof(selected)='object'
        AND jsonb_typeof(selected->'version')='number'
        AND selected->>'version' ~ '^[1-9][0-9]*$'
        AND selected->>'checksum' ~ '^[0-9a-f]{64}$') IS NOT TRUE THEN
        RAISE EXCEPTION 'invalid function partner reference' USING ERRCODE='23514';
    END IF;
    SELECT * INTO counterpart FROM reborn_component_revisions
        WHERE component_id=(selected->>'uuid')::uuid
            AND version=(selected->>'version')::bigint
            AND class_code=(selected->>'class_code')::smallint
            AND checksum=selected->>'checksum';
    IF NOT FOUND OR counterpart.component_id=NEW.component_id THEN
        RAISE EXCEPTION 'function partner revision mismatch' USING ERRCODE='23514';
    END IF;
    SELECT * INTO STRICT primary_revision FROM reborn_component_revisions
        WHERE component_id=NEW.component_id AND version=NEW.candidate_version;
    IF primary_revision.class_code BETWEEN 1 AND 3 AND counterpart.class_code=22 THEN
        skill_bytes := primary_revision.revision_bytes;
        code_id := counterpart.component_id;
    ELSIF primary_revision.class_code=22 AND counterpart.class_code BETWEEN 1 AND 3 THEN
        skill_bytes := counterpart.revision_bytes;
        code_id := primary_revision.component_id;
    ELSE
        RAISE EXCEPTION 'function subject requires one Skill and its code' USING ERRCODE='23514';
    END IF;
    IF (((skill_bytes::jsonb->>'association')::jsonb->>'python_code_uuid')::uuid=code_id) IS NOT TRUE THEN
        RAISE EXCEPTION 'function partner differs from Skill association' USING ERRCODE='23514';
    END IF;
    IF partner->>'role'='changed' THEN
        IF (jsonb_typeof(partner->'candidate_bytes')='string'
            AND partner->>'candidate_bytes'=counterpart.revision_bytes
            AND jsonb_typeof(partner->'expected_previous_version')='number'
            AND partner->>'expected_previous_version' ~ '^(0|[1-9][0-9]*)$') IS NOT TRUE THEN
            RAISE EXCEPTION 'function partner candidate or allocation mismatch' USING ERRCODE='23514';
        END IF;
        head := (partner->>'expected_previous_version')::numeric;
        IF head>=9223372036854775807 OR counterpart.version::numeric<>head+1 THEN
            RAISE EXCEPTION 'function partner allocation mismatch' USING ERRCODE='23514';
        END IF;
        previous := partner->'base';
        IF previous='null'::jsonb THEN
            IF head<>0 THEN
                RAISE EXCEPTION 'new function partner requires first allocation' USING ERRCODE='23514';
            END IF;
        ELSIF jsonb_typeof(previous)='object' THEN
            IF NOT EXISTS (
                SELECT 1 FROM reborn_component_revisions r
                WHERE r.component_id=counterpart.component_id
                    AND r.component_id=(previous->>'uuid')::uuid
                    AND r.class_code=counterpart.class_code
                    AND r.class_code=(previous->>'class_code')::smallint
                    AND r.version=(previous->>'version')::bigint AND r.version<=head
                    AND r.checksum=previous->>'checksum'
            ) THEN
                RAISE EXCEPTION 'function partner lineage mismatch' USING ERRCODE='23514';
            END IF;
        ELSE
            RAISE EXCEPTION 'function partner lineage required' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER component_review_function_partner
    BEFORE INSERT ON reborn_component_review_submissions
    FOR EACH ROW EXECUTE FUNCTION verify_function_review_partner();
