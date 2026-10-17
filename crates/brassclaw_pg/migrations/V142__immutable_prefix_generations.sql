-- Full immutable artifacts. Retention neither approves nor activates a prefix.
CREATE TABLE reborn_prefix_generations (
    profile_name TEXT NOT NULL REFERENCES reborn_prefix_profiles(name),
    generation_id TEXT NOT NULL CHECK (generation_id ~ '^[0-9a-f]{64}$'),
    profile_revision BIGINT NOT NULL CHECK (profile_revision > 0),
    generation_kind TEXT NOT NULL CHECK (generation_kind IN ('compiler-package','legacy-bundle')),
    generation_bytes TEXT NOT NULL CHECK (octet_length(generation_bytes) <= 67108864),
    CHECK (generation_id = encode(sha256(convert_to(generation_bytes,'UTF8')), 'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_name,generation_id)
);
CREATE TRIGGER protect_prefix_generation BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_generations FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();

-- Preserve existing compatibility data without fabricating source/token/cache
-- evidence or modifying any legacy serving pointer. Exact legacy scope is data.
WITH legacy AS (
    SELECT jsonb_build_object('format','brassclaw-legacy-prefix-bundle/1',
        'bundle_json',bundle_json,'fingerprint',fingerprint,'is_stale',is_stale,
        'assembled_at',assembled_at,'prewarm_last_at',prewarm_last_at,
        'scope',jsonb_build_object('tenant_id',tenant_id,'user_id',user_id,
            'agent_id',agent_id,'project_id',project_id),
        'qualification','legacy-unverified')::text AS bytes
    FROM reborn_basic_prompt_store
)
INSERT INTO reborn_prefix_generations(profile_name,generation_id,profile_revision,generation_kind,generation_bytes)
    SELECT 'base-prompt',encode(sha256(convert_to(bytes,'UTF8')),'hex'),1,'legacy-bundle',bytes FROM legacy
    ON CONFLICT DO NOTHING;
