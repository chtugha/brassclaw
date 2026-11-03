-- Preserve immutable historical records, including inconsistent records that
-- need explicit repair. NOT VALID avoids rewriting/grandfathering old evidence;
-- the constraints still enforce every new link. Application readers verify old
-- records before using them. Original V201 independent FKs did not bind roots.
ALTER TABLE reborn_component_review_submissions
    ADD CONSTRAINT component_review_submission_exact_candidate
    UNIQUE (submission_id, component_id, candidate_version);

ALTER TABLE reborn_pure_component_approvals
    ADD CONSTRAINT pure_approval_exact_root
    UNIQUE (approval_id, root_id, root_version);

ALTER TABLE reborn_pure_evidence_submission_links
    ADD CONSTRAINT pure_link_matches_original_submission
    FOREIGN KEY (submission_id, component_id, candidate_version)
    REFERENCES reborn_component_review_submissions
        (submission_id, component_id, candidate_version) NOT VALID;

ALTER TABLE reborn_pure_evidence_submission_links
    ADD CONSTRAINT pure_link_matches_approved_root
    FOREIGN KEY (approval_id, component_id, candidate_version)
    REFERENCES reborn_pure_component_approvals
        (approval_id, root_id, root_version) NOT VALID;

-- Also bind the revision digest/class. Keep old inconsistent rows for diagnosis;
-- neither adding this trigger nor retaining a label qualifies those records.
CREATE FUNCTION verify_exact_pure_link_root() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM reborn_pure_component_approvals p
        JOIN reborn_component_revisions r
            ON r.component_id=p.root_id AND r.version=p.root_version
        WHERE p.approval_id=NEW.approval_id
            AND p.root_id=NEW.component_id AND p.root_version=NEW.candidate_version
            AND r.class_code=22 AND p.root_checksum=r.checksum
    ) THEN
        RAISE EXCEPTION 'pure approval link root identity mismatch' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER pure_link_exact_root_digest
    BEFORE INSERT ON reborn_pure_evidence_submission_links
    FOR EACH ROW EXECUTE FUNCTION verify_exact_pure_link_root();
