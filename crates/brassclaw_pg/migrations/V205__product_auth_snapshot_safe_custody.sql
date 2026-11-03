-- One permanent row arbitrates each tenant's metadata custodian. A locking
-- read detects ownership changes hidden by a REPEATABLE READ/SERIALIZABLE
-- snapshot; absence in such a snapshot never grants the competing owner.
CREATE TABLE brassclaw_product_auth_custody (
    tenant_id TEXT PRIMARY KEY,
    owner TEXT NOT NULL CHECK (owner IN ('filesystem', 'native'))
);

INSERT INTO brassclaw_product_auth_custody (tenant_id, owner)
SELECT tenant_id, 'native' FROM brassclaw_product_auth_filesystem_cutovers;

-- Auth's internal schema may not have been attached on a fresh installation.
DO $$
BEGIN
    IF to_regclass('brassclaw_product_auth_filesystem_recoveries') IS NOT NULL THEN
        IF EXISTS (
            SELECT 1 FROM brassclaw_product_auth_filesystem_recoveries r
            JOIN brassclaw_product_auth_filesystem_cutovers c USING (tenant_id)
        ) THEN
            RAISE EXCEPTION 'conflicting product auth custody markers' USING ERRCODE = '55000';
        END IF;
        INSERT INTO brassclaw_product_auth_custody (tenant_id, owner)
        SELECT tenant_id, 'filesystem' FROM brassclaw_product_auth_filesystem_recoveries;
    END IF;
END;
$$;

CREATE FUNCTION brassclaw_require_product_auth_custody(candidate TEXT, expected TEXT)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual TEXT;
BEGIN
    -- An invisible conflicting insertion fails serialization under a fixed
    -- snapshot. Never interpret a missing snapshot-visible row as permission.
    INSERT INTO brassclaw_product_auth_custody (tenant_id, owner)
    VALUES (candidate, expected) ON CONFLICT (tenant_id) DO NOTHING;
    SELECT owner INTO actual FROM brassclaw_product_auth_custody
    WHERE tenant_id = candidate FOR SHARE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'product auth custody is outside this transaction snapshot'
            USING ERRCODE = '40001';
    END IF;
    IF actual <> expected THEN
        RAISE EXCEPTION 'product auth metadata belongs to another custodian'
            USING ERRCODE = '55000';
    END IF;
END;
$$;

-- Successor to V204's snapshot-only marker lookup. Keep its trigger identity
-- so already attached filesystem tables immediately use the stronger check.
CREATE OR REPLACE FUNCTION brassclaw_fence_retained_auth_filesystem() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE candidate TEXT;
BEGIN
    FOR candidate IN
        SELECT value FROM unnest(ARRAY[
            CASE WHEN TG_OP <> 'INSERT' THEN OLD.path END,
            CASE WHEN TG_OP <> 'DELETE' THEN NEW.path END
        ]) AS value WHERE value IS NOT NULL
    LOOP
        IF candidate LIKE '/tenants/%/users/%/secrets/%/product-auth/%' THEN
            PERFORM brassclaw_require_product_auth_custody(
                split_part(candidate, '/', 3), 'filesystem');
        END IF;
    END LOOP;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$;

-- Existing native triggers also acquire the new fence at upgrade time;
-- Auth's internal schema attachment installs this same trigger on new tables.
CREATE OR REPLACE FUNCTION brassclaw_fence_native_auth_during_filesystem_recovery()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE candidate TEXT;
BEGIN
    FOR candidate IN
        SELECT value FROM unnest(ARRAY[
            CASE WHEN TG_OP <> 'INSERT' THEN OLD.tenant_id END,
            CASE WHEN TG_OP <> 'DELETE' THEN NEW.tenant_id END
        ]) AS value WHERE value IS NOT NULL
    LOOP
        PERFORM brassclaw_require_product_auth_custody(candidate, 'native');
    END LOOP;
    IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END;
$$;
