-- Extend the existing publisher. Old immutable receipts remain byte-for-byte
-- original; NULL marks the historical contract, never fabricated invalidation.
ALTER TABLE brassclaw_monty_catalogue_activations
    ADD COLUMN prefix_invalidation jsonb,
    ADD CONSTRAINT monty_activation_generation_identity
        UNIQUE (activation_id, catalogue_id);

ALTER TABLE brassclaw_monty_active_catalogue
    ADD COLUMN activation_id uuid,
    ADD CONSTRAINT monty_active_publication_reference
        FOREIGN KEY (activation_id, catalogue_id)
        REFERENCES brassclaw_monty_catalogue_activations(activation_id, catalogue_id);

-- Future writes from an older binary must fail rather than publish a generation
-- without its source/consumer invalidation. This is consistency, not approval.
CREATE FUNCTION brassclaw_require_catalogue_invalidation()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    catalogue_checksum text;
    inline_checksum text;
    publication_limit numeric;
BEGIN
    SELECT checksum, catalogue_bytes::jsonb->>'prefix_checksum',
           (catalogue_bytes::jsonb->'preparation_policy'->>'max_catalogue_bytes')::numeric
      INTO STRICT catalogue_checksum, inline_checksum, publication_limit
      FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=NEW.catalogue_id;
    IF NOT COALESCE(
        jsonb_typeof(NEW.prefix_invalidation)='object'
        AND octet_length(NEW.prefix_invalidation::text)<=publication_limit
        AND NEW.prefix_invalidation->>'format'='catalogue-prefix-invalidation/1'
        AND NEW.prefix_invalidation->>'catalogue_generation'=NEW.catalogue_id::text
        AND NEW.prefix_invalidation->>'catalogue_checksum'=catalogue_checksum
        AND NEW.prefix_invalidation->>'inline_prefix_checksum'=inline_checksum
        AND jsonb_typeof(NEW.prefix_invalidation->'profiles')='array'
        AND NEW.prefix_invalidation->'profiles'=(
            SELECT COALESCE(jsonb_agg(jsonb_build_object('name',name,'revision',revision)
                ORDER BY name COLLATE "C"), '[]'::jsonb) FROM reborn_prefix_profiles
        )
        AND jsonb_typeof(NEW.prefix_invalidation->'source_documents')='array'
        AND jsonb_typeof(NEW.prefix_invalidation->'root')='object', false)
    THEN
        RAISE EXCEPTION 'catalogue publication requires exact prefix invalidation'
            USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER require_monty_catalogue_invalidation
    BEFORE INSERT ON brassclaw_monty_catalogue_activations
    FOR EACH ROW EXECUTE FUNCTION brassclaw_require_catalogue_invalidation();
