-- Exact native login publication identity in the existing immutable receipt.
-- A login need not alter its provider definition/revision. Historical rows stay
-- NULL; neither timestamps nor a provider-revision tie selects a credential.
ALTER TABLE brassclaw_llm_provider_mutations ADD COLUMN credential_head_ref TEXT;

ALTER TABLE brassclaw_llm_provider_mutations ADD CONSTRAINT provider_mutation_native_head
CHECK (
    credential_head_ref IS NULL OR (
        credential_head_ref ~ '^[0-9a-f]{64}$'
        AND operation = 'upsert'
        AND provider_id IS NOT NULL
        AND provider_revision IS NOT NULL
        AND request->>'kind' = 'codex-device-login/1'
    ) IS TRUE
);

CREATE UNIQUE INDEX provider_mutation_native_head_identity
ON brassclaw_llm_provider_mutations(tenant_id, provider_id, credential_head_ref)
WHERE credential_head_ref IS NOT NULL;
