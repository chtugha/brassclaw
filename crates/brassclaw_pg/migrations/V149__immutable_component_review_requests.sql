-- Durable intent to review an exact immutable submission. This is neither a
-- validator selection/attempt nor successful review evidence or activation.
CREATE TABLE reborn_component_review_requests (
    submission_id UUID PRIMARY KEY REFERENCES reborn_component_review_submissions(submission_id),
    subject_checksum TEXT NOT NULL CHECK (subject_checksum ~ '^[0-9a-f]{64}$'),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE FUNCTION verify_component_review_request() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM reborn_component_review_submissions s
        WHERE s.submission_id=NEW.submission_id AND s.checksum=NEW.subject_checksum
            AND s.checksum=encode(sha256(convert_to(s.subject_bytes,'UTF8')),'hex')
    ) THEN
        RAISE EXCEPTION 'component review request subject mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER component_review_request_subject
    BEFORE INSERT ON reborn_component_review_requests
    FOR EACH ROW EXECUTE FUNCTION verify_component_review_request();
CREATE TRIGGER component_review_requests_immutable
    BEFORE UPDATE OR DELETE ON reborn_component_review_requests
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER component_review_requests_no_truncate
    BEFORE TRUNCATE ON reborn_component_review_requests
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();

CREATE FUNCTION request_immutable_component_review() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO reborn_component_review_requests(submission_id,subject_checksum)
    VALUES (NEW.submission_id,NEW.checksum);
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_submission_requests_review
    AFTER INSERT ON reborn_component_review_submissions
    FOR EACH ROW EXECUTE FUNCTION request_immutable_component_review();

CREATE FUNCTION require_immutable_component_review_request() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM reborn_component_review_requests r
        WHERE r.submission_id=NEW.submission_id AND r.subject_checksum=NEW.checksum
    ) THEN
        RAISE EXCEPTION 'component submission review request missing' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER submission_requires_review_request
    AFTER INSERT ON reborn_component_review_submissions
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION require_immutable_component_review_request();

-- Retain old subjects byte-for-byte. Their absence from the former mutable
-- queue is not successful Q1 and must not erase their required review work.
INSERT INTO reborn_component_review_requests(submission_id,subject_checksum)
SELECT submission_id,checksum FROM reborn_component_review_submissions;
