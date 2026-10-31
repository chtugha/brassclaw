-- Original operator requests share the definition/key/selection transaction.
-- This is publication custody, not another provider catalogue or permission.
CREATE TABLE brassclaw_llm_provider_mutations (
    tenant_id text NOT NULL,
    mutation_id uuid NOT NULL CHECK (mutation_id <> '00000000-0000-0000-0000-000000000000'),
    operation text NOT NULL CHECK (operation IN ('upsert', 'deactivate', 'select')),
    request jsonb NOT NULL CHECK (jsonb_typeof(request) = 'object'),
    actor_id text NOT NULL CHECK (actor_id <> ''),
    provider_id text,
    provider_revision bigint CHECK (provider_revision > 0),
    role_selections jsonb NOT NULL CHECK (jsonb_typeof(role_selections) = 'object'),
    recorded_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, mutation_id),
    CHECK ((provider_id IS NULL) = (provider_revision IS NULL)),
    FOREIGN KEY (tenant_id, provider_id, provider_revision)
        REFERENCES brassclaw_llm_provider_revisions(tenant_id, provider_id, revision)
        ON DELETE RESTRICT
);

-- Request fingerprints containing credential identity live only in the
-- authenticated encrypted secret store, under this receipt's unique handle.
-- No key or publicly guessable hash of a key belongs in this table.
CREATE TRIGGER brassclaw_provider_mutations_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_llm_provider_mutations
    FOR EACH ROW EXECUTE FUNCTION protect_brassclaw_provider_revision_history();
CREATE TRIGGER brassclaw_provider_mutations_no_truncate
    BEFORE TRUNCATE ON brassclaw_llm_provider_mutations
    FOR EACH STATEMENT EXECUTE FUNCTION protect_brassclaw_provider_revision_history();
