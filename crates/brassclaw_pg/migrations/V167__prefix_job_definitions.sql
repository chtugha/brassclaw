-- Immutable prefix-operation definitions, not a second execution queue.
-- The existing turn run uses job_id as its requested correlation/run UUID.
-- State, attempts, effects and completion remain in the turn/Monty owners.
CREATE TABLE brassclaw_prefix_job_definitions (
    job_id UUID PRIMARY KEY CHECK (job_id <> '00000000-0000-0000-0000-000000000000'),
    profile_name TEXT NOT NULL REFERENCES reborn_prefix_profiles(name),
    profile_revision BIGINT NOT NULL CHECK (profile_revision > 0),
    operation TEXT NOT NULL CHECK (operation IN ('generate','redesign','model_design','create')),
    mode TEXT NOT NULL CHECK (mode IN ('source_only','target')),
    request_bytes TEXT NOT NULL CHECK (octet_length(request_bytes) BETWEEN 2 AND 65536),
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    program_bytes TEXT NOT NULL CHECK (octet_length(program_bytes) BETWEEN 2 AND 8388608),
    program_checksum TEXT NOT NULL CHECK (program_checksum ~ '^[0-9a-f]{64}$'),
    input_bytes TEXT NOT NULL CHECK (octet_length(input_bytes) BETWEEN 2 AND 67108864),
    input_checksum TEXT NOT NULL CHECK (input_checksum ~ '^[0-9a-f]{64}$'),
    scope JSONB NOT NULL CHECK (jsonb_typeof(scope)='object'),
    thread_scope JSONB NOT NULL CHECK (jsonb_typeof(thread_scope)='object'),
    created_by TEXT NOT NULL CHECK (octet_length(created_by) BETWEEN 1 AND 512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX prefix_job_definitions_profile ON brassclaw_prefix_job_definitions(profile_name,created_at,job_id);
CREATE FUNCTION protect_prefix_job_definition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'prefix job definitions are immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER prefix_job_definitions_immutable BEFORE UPDATE OR DELETE ON brassclaw_prefix_job_definitions
    FOR EACH ROW EXECUTE FUNCTION protect_prefix_job_definition();
