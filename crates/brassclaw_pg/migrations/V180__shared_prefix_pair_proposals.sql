-- Extend the existing immutable proposal owner; a pair draft has no role/profile.
-- Existing per-profile proposals and their acceptance foreign keys stay intact.
ALTER TABLE reborn_prefix_design_proposals ALTER COLUMN profile_name DROP NOT NULL;
ALTER TABLE reborn_prefix_design_proposals ADD CONSTRAINT prefix_proposal_scope
    CHECK ((profile_name IS NULL) =
        COALESCE(proposal_bytes::jsonb->>'format' =
            'brassclaw-prefix-pair-construction-proposal/1', FALSE));

-- A NULL-profile draft cannot satisfy the existing (proposal_id,candidate_profile)
-- acceptance FK. Retention does not open an acceptance or activation path.
