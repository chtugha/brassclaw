-- Preparation only: extend the SAME immutable acceptance owner. This is NOT a
-- checked acceptance writer, a serving pointer or new approval authority.
-- Original evidence bytes/checksums and old candidate references stay unchanged.
ALTER TABLE reborn_prefix_design_proposals ADD COLUMN proposal_scope TEXT
    GENERATED ALWAYS AS (CASE WHEN profile_name IS NULL THEN 'pair' ELSE 'profile' END) STORED;
ALTER TABLE reborn_prefix_design_proposals ADD CONSTRAINT prefix_proposal_scoped_identity
    UNIQUE(proposal_id,proposal_scope);

-- Old records remain explicitly historical. A new profile qualification cannot
-- inherit the old no-parent shape or accidentally use it as a scoped parent.
ALTER TABLE reborn_prefix_design_acceptances
    ADD COLUMN acceptance_scope TEXT NOT NULL DEFAULT 'legacy-profile',
    ADD COLUMN pair_acceptance_ref UUID,
    ADD COLUMN profile_acceptance_ref UUID,
    ADD COLUMN accepted_proposal_scope TEXT GENERATED ALWAYS AS
        (CASE WHEN acceptance_scope='pair' THEN 'pair' ELSE 'profile' END) STORED,
    ADD COLUMN accepted_profile TEXT GENERATED ALWAYS AS
        (CASE WHEN acceptance_scope='pair' THEN NULL ELSE candidate_profile END) STORED,
    ADD COLUMN pair_parent_scope TEXT GENERATED ALWAYS AS
        (CASE WHEN pair_acceptance_ref IS NULL THEN NULL ELSE 'pair' END) STORED,
    ADD COLUMN profile_parent_scope TEXT GENERATED ALWAYS AS
        (CASE WHEN profile_acceptance_ref IS NULL THEN NULL ELSE 'profile' END) STORED;
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_scope
    CHECK (acceptance_scope IN ('legacy-profile','pair','profile','generation'));
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_scoped_identity
    UNIQUE(acceptance_id,acceptance_scope);
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_parentage
    CHECK (
        (acceptance_scope IN ('legacy-profile','pair')
            AND pair_acceptance_ref IS NULL AND profile_acceptance_ref IS NULL)
        OR (acceptance_scope='profile'
            AND pair_acceptance_ref IS NOT NULL AND profile_acceptance_ref IS NULL)
        OR (acceptance_scope='generation'
            AND pair_acceptance_ref IS NOT NULL AND profile_acceptance_ref IS NOT NULL)
    );

-- Resolve the actual PostgreSQL-generated FK name instead of guessing its
-- possibly truncated identifier. Unexpected historical schema aborts migration.
DO $$
DECLARE original_fk TEXT;
BEGIN
    SELECT c.conname INTO STRICT original_fk
      FROM pg_constraint c
     WHERE c.conrelid='reborn_prefix_design_acceptances'::regclass
       AND c.confrelid='reborn_prefix_design_proposals'::regclass
       AND c.contype='f'
       AND c.conkey=ARRAY[
           (SELECT attnum FROM pg_attribute
             WHERE attrelid=c.conrelid AND attname='proposal_id'),
           (SELECT attnum FROM pg_attribute
             WHERE attrelid=c.conrelid AND attname='candidate_profile')
       ]::SMALLINT[];
    EXECUTE format('ALTER TABLE reborn_prefix_design_acceptances DROP CONSTRAINT %I',original_fk);
END $$;
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_scoped_proposal
    FOREIGN KEY(proposal_id,accepted_proposal_scope)
    REFERENCES reborn_prefix_design_proposals(proposal_id,proposal_scope);
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_profile_proposal
    FOREIGN KEY(proposal_id,accepted_profile)
    REFERENCES reborn_prefix_design_proposals(proposal_id,profile_name);
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_pair_parent
    FOREIGN KEY(pair_acceptance_ref,pair_parent_scope)
    REFERENCES reborn_prefix_design_acceptances(acceptance_id,acceptance_scope);
ALTER TABLE reborn_prefix_design_acceptances ADD CONSTRAINT prefix_acceptance_profile_parent
    FOREIGN KEY(profile_acceptance_ref,profile_parent_scope)
    REFERENCES reborn_prefix_design_acceptances(acceptance_id,acceptance_scope);

-- Native checked writer/reader must additionally verify namespace/spec/artifact,
-- exact proposal/evidence contracts, matching parent pair, source/target integrity
-- and the real review/behavior evidence. Scope labels alone prove NONE of these.
-- Pair/profile candidate_generation records evaluation history; only a generation
-- qualification selects its own new candidate_generation for normal regeneration.
-- Do not add editable model/token limits or rewrite an original evidence record.
