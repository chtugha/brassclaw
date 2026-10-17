-- Immutable preparation metadata; not an execution claim, approval or verdict.
CREATE TABLE reborn_component_validator_selections (
    submission_id UUID PRIMARY KEY REFERENCES reborn_component_review_requests(submission_id),
    selection_bytes TEXT NOT NULL CHECK (octet_length(selection_bytes)>0),
    checksum TEXT NOT NULL CHECK (checksum=encode(sha256(convert_to(selection_bytes,'UTF8')),'hex')),
    retained_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK ((jsonb_typeof(selection_bytes::jsonb)='object') IS TRUE),
    CHECK ((selection_bytes::jsonb->>'format'='component-validator-selection/1') IS TRUE),
    CHECK (((selection_bytes::jsonb->>'submission_id')::uuid=submission_id) IS TRUE),
    CHECK ((selection_bytes::jsonb->>'scope'='preparation-only') IS TRUE)
);
CREATE FUNCTION verify_component_validator_selection_subject() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM reborn_component_review_requests r
        WHERE r.submission_id=NEW.submission_id
            AND r.subject_checksum=NEW.selection_bytes::jsonb->>'subject_checksum') THEN
        RAISE EXCEPTION 'component validator selection subject mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER component_validator_selection_subject
    BEFORE INSERT ON reborn_component_validator_selections
    FOR EACH ROW EXECUTE FUNCTION verify_component_validator_selection_subject();
CREATE TRIGGER component_validator_selections_immutable
    BEFORE UPDATE OR DELETE ON reborn_component_validator_selections
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER component_validator_selections_no_truncate
    BEFORE TRUNCATE ON reborn_component_validator_selections
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
