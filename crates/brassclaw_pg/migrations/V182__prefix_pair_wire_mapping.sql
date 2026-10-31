-- Version the SAME immutable pair-proposal owner. No serving selection,
-- acceptance evidence, copied token settings or historical-row mutation.
ALTER TABLE reborn_prefix_design_proposals DROP CONSTRAINT prefix_proposal_scope;
ALTER TABLE reborn_prefix_design_proposals ADD CONSTRAINT prefix_proposal_scope
    CHECK ((profile_name IS NULL) = COALESCE(proposal_bytes::jsonb->>'format' IN (
        'brassclaw-prefix-pair-construction-proposal/1',
        'brassclaw-prefix-pair-construction-proposal/2'), FALSE));
