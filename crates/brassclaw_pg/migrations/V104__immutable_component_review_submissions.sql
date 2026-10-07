-- Authoring subjects only. Successful review evidence, activation and the
-- durable Monty review-work admission contract remain separate.
CREATE TABLE reborn_component_review_submissions (
    submission_id UUID PRIMARY KEY CHECK (submission_id <> '00000000-0000-0000-0000-000000000000'),
    actor TEXT NOT NULL CHECK (octet_length(actor) BETWEEN 1 AND 2048 AND btrim(actor) <> ''),
    component_id UUID NOT NULL,
    candidate_version BIGINT NOT NULL CHECK (candidate_version > 0),
    base_version BIGINT CHECK (base_version > 0 AND candidate_version = base_version + 1),
    subject_bytes TEXT NOT NULL CHECK (octet_length(subject_bytes) BETWEEN 1 AND 16777216),
    checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (component_id, candidate_version) REFERENCES reborn_component_revisions(component_id, version),
    FOREIGN KEY (component_id, base_version) REFERENCES reborn_component_revisions(component_id, version),
    CHECK (base_version IS NOT NULL OR candidate_version = 1),
    CHECK (checksum = encode(sha256(convert_to(subject_bytes, 'UTF8')), 'hex')),
    CHECK ((subject_bytes::jsonb ->> 'format' = 'component-review-submission/1') IS TRUE),
    CHECK (((subject_bytes::jsonb ->> 'submission_id')::uuid = submission_id) IS TRUE),
    CHECK ((subject_bytes::jsonb ->> 'actor' = actor) IS TRUE),
    CHECK (((subject_bytes::jsonb -> 'candidate' ->> 'uuid')::uuid = component_id) IS TRUE),
    CHECK (((subject_bytes::jsonb -> 'candidate' ->> 'version')::bigint = candidate_version) IS TRUE),
    CHECK ((jsonb_typeof(subject_bytes::jsonb -> 'candidate_bytes') = 'string') IS TRUE),
    CHECK ((jsonb_typeof(subject_bytes::jsonb -> 'dependencies') = 'array') IS TRUE),
    CHECK ((CASE WHEN base_version IS NULL THEN subject_bytes::jsonb -> 'base' = 'null'::jsonb
        ELSE (subject_bytes::jsonb -> 'base' ->> 'uuid')::uuid = component_id
            AND (subject_bytes::jsonb -> 'base' ->> 'version')::bigint = base_version END) IS TRUE)
);
CREATE INDEX reborn_component_review_submissions_candidate
    ON reborn_component_review_submissions(component_id, candidate_version);

CREATE TRIGGER reborn_component_review_submissions_immutable
    BEFORE UPDATE OR DELETE ON reborn_component_review_submissions
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER reborn_component_review_submissions_no_truncate
    BEFORE TRUNCATE ON reborn_component_review_submissions
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
