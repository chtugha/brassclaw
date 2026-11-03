-- Installation-owned custody handoff. The original filesystem entries remain
-- byte-for-byte retained; this marker fences old writers after the atomic import.
CREATE TABLE brassclaw_product_auth_filesystem_cutovers (
    tenant_id TEXT PRIMARY KEY,
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_records BIGINT NOT NULL CHECK (source_records >= 0),
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE FUNCTION brassclaw_fence_retained_auth_filesystem() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE candidate TEXT;
BEGIN
    FOR candidate IN
        SELECT value FROM unnest(ARRAY[
            CASE WHEN TG_OP <> 'INSERT' THEN OLD.path END,
            CASE WHEN TG_OP <> 'DELETE' THEN NEW.path END
        ]) AS value WHERE value IS NOT NULL
    LOOP
        IF candidate LIKE '/tenants/%/users/%/secrets/%/product-auth/%'
           AND EXISTS (SELECT 1 FROM brassclaw_product_auth_filesystem_cutovers
                       WHERE tenant_id = split_part(candidate, '/', 3)) THEN
            RAISE EXCEPTION 'product auth filesystem custody has transferred to native storage'
                USING ERRCODE = '55000';
        END IF;
    END LOOP;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$;
-- The filesystem owns its schema at attachment time. The Auth startup importer
-- installs the trigger only after that owner has created root_filesystem_entries.
