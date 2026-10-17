-- Origin is correlation/fencing metadata, never a Tool grant. Old exchanges
-- retain NULL: inventing a current attempt for historical evidence is unsafe.
ALTER TABLE brassclaw_mcp_exchanges ADD COLUMN parent_surface_version text
    CHECK (parent_surface_version IS NULL OR
        (octet_length(parent_surface_version) BETWEEN 1 AND 128
            AND parent_surface_version !~ '[^A-Za-z0-9_.:-]'));

CREATE OR REPLACE FUNCTION protect_mcp_exchange_identity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'MCP exchange evidence must be retained' USING ERRCODE='23514';
    END IF;
    -- All identity/provenance columns, including future additions, are immutable.
    IF (to_jsonb(NEW) - ARRAY['active','disconnected_at']) IS DISTINCT FROM
       (to_jsonb(OLD) - ARRAY['active','disconnected_at']) OR
       (NOT OLD.active AND NEW.active) OR
       (OLD.disconnected_at IS NOT NULL AND NEW.disconnected_at IS DISTINCT FROM OLD.disconnected_at) THEN
        RAISE EXCEPTION 'MCP exchange identity cannot be replaced or reopened' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
