-- These are immutable model/design evidence records, not another provider list.
-- Provider revisions remain owned by the existing catalogue/history.
CREATE TABLE reborn_prefix_model_specifications (
    specification_id TEXT PRIMARY KEY CHECK (specification_id ~ '^[0-9a-f]{64}$'),
    provider_namespace TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_revision BIGINT NOT NULL CHECK (provider_revision > 0),
    model TEXT NOT NULL CHECK (length(model) BETWEEN 1 AND 2048),
    specification_bytes TEXT NOT NULL CHECK (octet_length(specification_bytes) <= 1048576),
    CHECK (specification_id=encode(sha256(convert_to(specification_bytes,'UTF8')),'hex')),
    FOREIGN KEY(provider_namespace,provider_id,provider_revision)
        REFERENCES brassclaw_llm_provider_revisions(tenant_id,provider_id,revision),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER protect_prefix_model_specification BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_model_specifications FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
CREATE TABLE reborn_prefix_design_proposals (
    proposal_id TEXT PRIMARY KEY CHECK (proposal_id ~ '^[0-9a-f]{64}$'),
    profile_name TEXT NOT NULL REFERENCES reborn_prefix_profiles(name),
    specification_id TEXT NOT NULL REFERENCES reborn_prefix_model_specifications(specification_id),
    proposal_bytes TEXT NOT NULL CHECK (octet_length(proposal_bytes) <= 1048576),
    CHECK (proposal_id=encode(sha256(convert_to(proposal_bytes,'UTF8')),'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(proposal_id,profile_name)
);
CREATE TRIGGER protect_prefix_design_proposal BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_design_proposals FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
-- Acceptance evidence must be written by the qualified validation/acceptance
-- owner. No draft/status label or operator-supplied success flag selects a design.
CREATE TABLE reborn_prefix_design_acceptances (
    acceptance_id UUID PRIMARY KEY,
    proposal_id TEXT NOT NULL REFERENCES reborn_prefix_design_proposals(proposal_id),
    candidate_profile TEXT NOT NULL,
    candidate_generation TEXT NOT NULL,
    evidence_bytes TEXT NOT NULL CHECK (octet_length(evidence_bytes) <= 1048576),
    checksum TEXT NOT NULL CHECK (checksum=encode(sha256(convert_to(evidence_bytes,'UTF8')),'hex')),
    FOREIGN KEY(candidate_profile,candidate_generation) REFERENCES reborn_prefix_generations(profile_name,generation_id),
    FOREIGN KEY(proposal_id,candidate_profile) REFERENCES reborn_prefix_design_proposals(proposal_id,profile_name),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER protect_prefix_design_acceptance BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_design_acceptances FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
