-- Keep the exact JSONB identity as evidence without putting arbitrarily long
-- request strings directly into a B-tree key. Numeric spelling is insignificant
-- for JSONB equality, so 1, 1.0 and 1e0 must have the same digest.
-- trim_scale preserves the exact numeric value; the UTF-8 encoding and
-- pg_catalog-only implementation are invariant.
CREATE FUNCTION brassclaw_mcp_request_id_digest(id jsonb) RETURNS bytea
LANGUAGE sql IMMUTABLE STRICT PARALLEL SAFE
SET search_path = pg_catalog
AS $$
    SELECT sha256(convert_to(
        CASE jsonb_typeof(id)
            WHEN 'number' THEN 'number:' || trim_scale((id #>> '{}')::numeric)::text
            WHEN 'string' THEN 'string:' || id::text
            ELSE NULL
        END, 'UTF8'))
$$;

ALTER TABLE brassclaw_mcp_chat_calls ADD COLUMN request_id_digest bytea
    GENERATED ALWAYS AS (brassclaw_mcp_request_id_digest(request_id)) STORED NOT NULL;
ALTER TABLE brassclaw_mcp_chat_calls
    ADD CONSTRAINT brassclaw_mcp_chat_calls_request_identity_key
    UNIQUE(parent_run_id, owner_scope, request_id_digest);
-- PostgreSQL shortens automatically generated constraint names. Resolve the
-- predecessor by its actual ordered column identities rather than guessing it.
DO $$
DECLARE legacy_name text;
BEGIN
    SELECT c.conname INTO STRICT legacy_name
    FROM pg_constraint c
    WHERE c.conrelid = 'brassclaw_mcp_chat_calls'::regclass AND c.contype = 'u'
      AND (SELECT array_agg(a.attname::text ORDER BY k.ordinality)
           FROM unnest(c.conkey) WITH ORDINALITY AS k(attnum, ordinality)
           JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum)
          = ARRAY['parent_run_id','owner_scope','request_id'];
    EXECUTE format('ALTER TABLE brassclaw_mcp_chat_calls DROP CONSTRAINT %I', legacy_name);
END $$;
-- Existing immutable rows, phases and receipts remain intact. Every reservation
-- lookup also compares the original JSONB ID: a hypothetical digest collision
-- fails closed before admission, never returning another request's receipt.
