-- Selected Recipe-variant observations are distinct from one-Tool association
-- evidence, semantic Q2, protected-root trust and catalogue activation.
CREATE TABLE reborn_workflow_review_evidence (
    evidence_id UUID PRIMARY KEY CHECK (evidence_id <> '00000000-0000-0000-0000-000000000000'),
    recipe_id UUID NOT NULL,
    recipe_version BIGINT NOT NULL CHECK (recipe_version > 0),
    selection_bytes TEXT NOT NULL CHECK (octet_length(selection_bytes) BETWEEN 1 AND 8388608),
    selection_checksum TEXT NOT NULL,
    evidence_kind TEXT NOT NULL CHECK (evidence_kind IN ('q1','behavior')),
    evidence_bytes TEXT NOT NULL CHECK (octet_length(evidence_bytes) BETWEEN 1 AND 8388608),
    checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (recipe_id,recipe_version) REFERENCES reborn_component_revisions(component_id,version),
    CHECK (selection_checksum=encode(sha256(convert_to(selection_bytes,'UTF8')),'hex')),
    CHECK (checksum=encode(sha256(convert_to(evidence_bytes,'UTF8')),'hex')),
    CHECK ((jsonb_typeof(selection_bytes::jsonb)='object') IS TRUE),
    CHECK ((selection_bytes::jsonb->>'format'='monty-retained-recipe-selection/1') IS TRUE),
    CHECK (((selection_bytes::jsonb->'recipe'->>'uuid')::uuid=recipe_id) IS TRUE),
    CHECK (((selection_bytes::jsonb->'recipe'->>'version')::bigint=recipe_version) IS TRUE),
    CHECK (((selection_bytes::jsonb->'recipe'->>'class_code')::integer=21) IS TRUE),
    CHECK ((jsonb_typeof(evidence_bytes::jsonb)='object') IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'format'='workflow-review-evidence/1') IS TRUE),
    CHECK (((evidence_bytes::jsonb->>'evidence_id')::uuid=evidence_id) IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'kind'=evidence_kind) IS TRUE),
    CHECK ((evidence_bytes::jsonb->>'selection_checksum'=selection_checksum) IS TRUE)
);
CREATE TRIGGER reborn_workflow_review_evidence_immutable
    BEFORE UPDATE OR DELETE ON reborn_workflow_review_evidence
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER reborn_workflow_review_evidence_no_truncate
    BEFORE TRUNCATE ON reborn_workflow_review_evidence
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
