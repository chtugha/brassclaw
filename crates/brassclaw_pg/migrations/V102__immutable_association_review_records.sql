-- Storage for the exact combination/review contract, separate from legacy
-- mutable labels and draft revision retention. No legacy receipt is imported
-- or approved here. Trusted review producers still require application wiring;
-- authoring APIs must never offer direct INSERT of successful evidence.
CREATE TABLE reborn_component_review_evidence (
    evidence_id UUID PRIMARY KEY CHECK (evidence_id <> '00000000-0000-0000-0000-000000000000'),
    evidence_bytes TEXT NOT NULL CHECK (octet_length(evidence_bytes) BETWEEN 1 AND 8388608),
    checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (checksum=encode(sha256(convert_to(evidence_bytes,'UTF8')),'hex')),
    CHECK ((jsonb_typeof(evidence_bytes::jsonb)='object') IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'format'='component-review-evidence/1') IS TRUE),
    CHECK (((evidence_bytes::jsonb->>'evidence_id')::uuid=evidence_id) IS TRUE)
);

CREATE TABLE reborn_skill_association_approvals (
    approval_id UUID PRIMARY KEY CHECK (approval_id <> '00000000-0000-0000-0000-000000000000'),
    approval_bytes TEXT NOT NULL CHECK (octet_length(approval_bytes) BETWEEN 1 AND 8388608),
    checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (checksum=encode(sha256(convert_to(approval_bytes,'UTF8')),'hex')),
    CHECK ((jsonb_typeof(approval_bytes::jsonb)='object') IS TRUE),
    CHECK ((approval_bytes::jsonb->>'format'='skill-association-approval/1') IS TRUE),
    CHECK (((approval_bytes::jsonb->>'approval_id')::uuid=approval_id) IS TRUE)
);

CREATE FUNCTION protect_component_review_records() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'component review records are immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER reborn_component_review_evidence_immutable
    BEFORE UPDATE OR DELETE ON reborn_component_review_evidence
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER reborn_component_review_evidence_no_truncate
    BEFORE TRUNCATE ON reborn_component_review_evidence
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER reborn_skill_association_approvals_immutable
    BEFORE UPDATE OR DELETE ON reborn_skill_association_approvals
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER reborn_skill_association_approvals_no_truncate
    BEFORE TRUNCATE ON reborn_skill_association_approvals
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
