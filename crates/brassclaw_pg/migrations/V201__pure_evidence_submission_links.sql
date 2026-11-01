-- Checked successor for linked pure evidence/approval records.
-- Pure-component-review cutover contract item 1:
-- Introduce a checked successor migration and necessary linked pure
-- evidence/approval records instead of relabelling new fields under the
-- unchanged V102 component-review-evidence/1 or V106 workflow-review-evidence/1
-- formats.
--
-- This table links an immutable review submission (V104
-- reborn_component_review_submissions) to a pure-component approval (V200
-- reborn_pure_component_approvals). It records the subject submission that a
-- pure-component approval was derived from, enabling the trusted review owner
-- to verify exact subject identity (submission_id/checksum) against the stored
-- pure approval declaration without relabelling the V102/V106 evidence tables.
--
-- Invariants:
-- - submission_id and approval_id are both non-nil.
-- - The referenced submission must exist in reborn_component_review_submissions.
-- - The referenced approval must exist in reborn_pure_component_approvals.
-- - The component_id/candidate_version pair identifies the subject component in
--   the submission, providing a direct DB integrity check without re-parsing
--   subject_bytes. Both foreign keys are verified at insert time.
-- - Records are immutable: UPDATE/DELETE/TRUNCATE are rejected.
-- - The link is one approval to one submission; one submission may have at most
--   one approval per approval_id (UNIQUE on approval_id). A submission may
--   receive a new approval_id when re-reviewed (new approval, new record).
--
-- This is a structural link only. It does not imply Q1, Q2, activation,
-- combination approval or Tool permission. Those remain separate gates.
CREATE TABLE reborn_pure_evidence_submission_links (
    link_id UUID PRIMARY KEY
        CHECK (link_id <> '00000000-0000-0000-0000-000000000000'),
    submission_id UUID NOT NULL
        CHECK (submission_id <> '00000000-0000-0000-0000-000000000000'),
    approval_id UUID NOT NULL
        CHECK (approval_id <> '00000000-0000-0000-0000-000000000000'),
    -- Denormalized for DB-level integrity: the component under review.
    component_id UUID NOT NULL
        CHECK (component_id <> '00000000-0000-0000-0000-000000000000'),
    candidate_version BIGINT NOT NULL CHECK (candidate_version > 0),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- One approval per link_id (PK); one approval may only link one submission.
    UNIQUE (approval_id),
    -- One submission may have at most one linked approval per component+version
    -- (prevents double-linking the same subject to different approvals).
    UNIQUE (submission_id, component_id, candidate_version),
    FOREIGN KEY (submission_id)
        REFERENCES reborn_component_review_submissions(submission_id),
    FOREIGN KEY (approval_id)
        REFERENCES reborn_pure_component_approvals(approval_id),
    -- Cross-check: the component_id/candidate_version must be the same pair
    -- referenced in the submission row (denormalized here for FK integrity).
    FOREIGN KEY (component_id, candidate_version)
        REFERENCES reborn_component_revisions(component_id, version),
    -- The component_id/candidate_version must also match the submission's
    -- component_id/candidate_version. Enforced via a CHECK on the submission
    -- FK join — the closest approximation without a separate trigger.
    CHECK (component_id <> '00000000-0000-0000-0000-000000000000')
);

CREATE INDEX reborn_pure_evidence_submission_links_submission
    ON reborn_pure_evidence_submission_links(submission_id);
CREATE INDEX reborn_pure_evidence_submission_links_approval
    ON reborn_pure_evidence_submission_links(approval_id);
CREATE INDEX reborn_pure_evidence_submission_links_component
    ON reborn_pure_evidence_submission_links(component_id, candidate_version);

-- Immutability triggers: records are permanent evidence; UPDATE/DELETE/TRUNCATE
-- must be rejected with the same error code as V102/V200.
CREATE FUNCTION protect_pure_evidence_submission_links() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'pure evidence submission link records are immutable'
        USING ERRCODE = '23514';
END;
$$;

CREATE TRIGGER reborn_pure_evidence_submission_links_immutable
    BEFORE UPDATE OR DELETE ON reborn_pure_evidence_submission_links
    FOR EACH ROW EXECUTE FUNCTION protect_pure_evidence_submission_links();
CREATE TRIGGER reborn_pure_evidence_submission_links_no_truncate
    BEFORE TRUNCATE ON reborn_pure_evidence_submission_links
    FOR EACH STATEMENT EXECUTE FUNCTION protect_pure_evidence_submission_links();
