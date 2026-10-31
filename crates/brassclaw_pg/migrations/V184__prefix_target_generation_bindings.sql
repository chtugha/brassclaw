-- The missing relation from one target/profile to its accepted generation.
-- No provider catalogue or model limits live here. Acceptance remains immutable;
-- publication is explicit and CAS-revisioned. Captured tasks keep original IDs.
DO $$
DECLARE original_check TEXT;
BEGIN
    SELECT c.conname INTO STRICT original_check FROM pg_constraint c
     WHERE c.conrelid='reborn_prefix_generations'::regclass AND c.contype='c'
       AND c.conkey=ARRAY[(SELECT attnum FROM pg_attribute
          WHERE attrelid=c.conrelid AND attname='generation_kind')]::SMALLINT[];
    EXECUTE format('ALTER TABLE reborn_prefix_generations DROP CONSTRAINT %I',original_check);
END $$;
ALTER TABLE reborn_prefix_generations
    ADD CONSTRAINT prefix_generation_kind CHECK(generation_kind IN ('compiler-package','legacy-bundle','model-target')),
    ADD COLUMN source_generation TEXT,
    ADD CONSTRAINT prefix_target_source_shape CHECK((generation_kind='model-target')=(source_generation IS NOT NULL)),
    ADD CONSTRAINT prefix_target_source FOREIGN KEY(profile_name,source_generation)
        REFERENCES reborn_prefix_generations(profile_name,generation_id);
CREATE TABLE reborn_prefix_target_bindings (
    provider_namespace TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    profile_name TEXT NOT NULL REFERENCES reborn_prefix_profiles(name),
    revision BIGINT NOT NULL CHECK (revision > 0),
    generation_qualification UUID NOT NULL,
    acceptance_scope TEXT GENERATED ALWAYS AS ('generation'::text) STORED,
    PRIMARY KEY(provider_namespace,provider_id,model,profile_name),
    FOREIGN KEY(generation_qualification,acceptance_scope)
        REFERENCES reborn_prefix_design_acceptances(acceptance_id,acceptance_scope)
);
