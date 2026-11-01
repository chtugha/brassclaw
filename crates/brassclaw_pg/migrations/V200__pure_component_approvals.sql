-- Immutable pure-component-approval/1 records: exact approved class-22 closure.
-- V102 reborn_component_review_evidence stores component-review-evidence/1 records.
-- These are the separate, distinct pure-component approval declarations that
-- reference those evidence records and attest the approved root UUID/revision.
-- No legacy receipt is imported or approved here.
-- Trusted approval producers still require application wiring; authoring APIs
-- must never offer direct INSERT of successful evidence via public routes.
CREATE TABLE reborn_pure_component_approvals (
    approval_id UUID PRIMARY KEY CHECK (approval_id <> '00000000-0000-0000-0000-000000000000'),
    root_id UUID NOT NULL CHECK (root_id <> '00000000-0000-0000-0000-000000000000'),
    root_version BIGINT NOT NULL CHECK (root_version > 0),
    root_checksum TEXT NOT NULL CHECK (length(root_checksum) = 64),
    validation_mode TEXT NOT NULL CHECK (validation_mode IN ('authored', 'system_seed')),
    q1_ref UUID NOT NULL CHECK (q1_ref <> '00000000-0000-0000-0000-000000000000'),
    q2_ref UUID CHECK (q2_ref <> '00000000-0000-0000-0000-000000000000'),
    approval_bytes TEXT NOT NULL CHECK (octet_length(approval_bytes) BETWEEN 1 AND 8388608),
    checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (checksum = encode(sha256(convert_to(approval_bytes, 'UTF8')), 'hex')),
    CHECK ((jsonb_typeof(approval_bytes::jsonb) = 'object') IS TRUE),
    CHECK ((approval_bytes::jsonb ->> 'format' = 'pure-component-approval/1') IS TRUE),
    CHECK (((approval_bytes::jsonb ->> 'approval_id')::uuid = approval_id) IS TRUE),
    CHECK (((approval_bytes::jsonb -> 'root' ->> 'uuid')::uuid = root_id) IS TRUE),
    CHECK (((approval_bytes::jsonb -> 'root' ->> 'version')::bigint = root_version) IS TRUE),
    CHECK ((approval_bytes::jsonb -> 'root' ->> 'checksum' = root_checksum) IS TRUE),
    -- authored requires a non-null q2_ref; system_seed requires null
    CHECK (
        (validation_mode = 'authored' AND q2_ref IS NOT NULL)
        OR (validation_mode = 'system_seed' AND q2_ref IS NULL)
    ),
    -- root must be a class-22 component
    FOREIGN KEY (root_id, root_version) REFERENCES reborn_component_revisions(component_id, version)
);

CREATE INDEX reborn_pure_component_approvals_root
    ON reborn_pure_component_approvals(root_id, root_version);

-- Evidence references: one row per component in the closure.
-- Constrained to existing component revisions and evidence records.
CREATE TABLE reborn_pure_component_approval_closure (
    approval_id UUID NOT NULL REFERENCES reborn_pure_component_approvals(approval_id),
    component_id UUID NOT NULL CHECK (component_id <> '00000000-0000-0000-0000-000000000000'),
    component_version BIGINT NOT NULL CHECK (component_version > 0),
    component_checksum TEXT NOT NULL CHECK (length(component_checksum) = 64),
    PRIMARY KEY (approval_id, component_id),
    FOREIGN KEY (component_id, component_version) REFERENCES reborn_component_revisions(component_id, version)
);

CREATE FUNCTION protect_pure_component_approvals() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'pure component approval records are immutable' USING ERRCODE = '23514';
END;
$$;

CREATE TRIGGER reborn_pure_component_approvals_immutable
    BEFORE UPDATE OR DELETE ON reborn_pure_component_approvals
    FOR EACH ROW EXECUTE FUNCTION protect_pure_component_approvals();
CREATE TRIGGER reborn_pure_component_approvals_no_truncate
    BEFORE TRUNCATE ON reborn_pure_component_approvals
    FOR EACH STATEMENT EXECUTE FUNCTION protect_pure_component_approvals();
CREATE TRIGGER reborn_pure_component_approval_closure_immutable
    BEFORE UPDATE OR DELETE ON reborn_pure_component_approval_closure
    FOR EACH ROW EXECUTE FUNCTION protect_pure_component_approvals();
CREATE TRIGGER reborn_pure_component_approval_closure_no_truncate
    BEFORE TRUNCATE ON reborn_pure_component_approval_closure
    FOR EACH STATEMENT EXECUTE FUNCTION protect_pure_component_approvals();
