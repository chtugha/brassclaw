-- Command execution observations are distinct from approval and Tool policy.
CREATE TABLE IF NOT EXISTS brassclaw_mcp_command_qualifications (
    checksum text PRIMARY KEY,
    catalogue_id uuid NOT NULL REFERENCES brassclaw_monty_boot_catalogues(catalogue_id),
    evidence_bytes text NOT NULL CHECK (octet_length(evidence_bytes) BETWEEN 1 AND 8388608),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK (checksum = encode(sha256(convert_to(evidence_bytes, 'UTF8')), 'hex')),
    CHECK ((evidence_bytes::jsonb->>'format' = 'mcp-command-qualification/1') IS TRUE),
    CHECK (((evidence_bytes::jsonb->>'catalogue_generation')::uuid = catalogue_id) IS TRUE)
);
CREATE FUNCTION protect_mcp_command_qualifications() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'MCP command qualifications are immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER protect_mcp_command_qualifications BEFORE UPDATE OR DELETE OR TRUNCATE
    ON brassclaw_mcp_command_qualifications FOR EACH STATEMENT
    EXECUTE FUNCTION protect_mcp_command_qualifications();
