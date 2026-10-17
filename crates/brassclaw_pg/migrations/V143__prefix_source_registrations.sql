-- Saved references are pending collection/review, not approved source snapshots.
CREATE TABLE reborn_prefix_sources (
    profile_name TEXT NOT NULL REFERENCES reborn_prefix_profiles(name),
    source_id UUID NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('web','component')),
    canonical_reference TEXT NOT NULL CHECK (length(canonical_reference) BETWEEN 1 AND 8192),
    registration_bytes TEXT NOT NULL CHECK (octet_length(registration_bytes) <= 16384),
    checksum TEXT NOT NULL CHECK (checksum=encode(sha256(convert_to(registration_bytes,'UTF8')),'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_name,source_id,revision),
    UNIQUE (profile_name,canonical_reference)
);
CREATE TRIGGER protect_prefix_source_registration BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_sources FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
-- Real, separately retained observations only. No availability is seeded.
CREATE TABLE reborn_prefix_source_observations (
    observation_id UUID PRIMARY KEY,
    profile_name TEXT NOT NULL,
    source_id UUID NOT NULL,
    source_revision BIGINT NOT NULL,
    observation_bytes TEXT NOT NULL CHECK (octet_length(observation_bytes) <= 65536),
    checksum TEXT NOT NULL CHECK (checksum=encode(sha256(convert_to(observation_bytes,'UTF8')),'hex')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(profile_name,source_id,source_revision) REFERENCES reborn_prefix_sources(profile_name,source_id,revision)
);
CREATE TRIGGER protect_prefix_source_observation BEFORE UPDATE OR DELETE OR TRUNCATE
    ON reborn_prefix_source_observations FOR EACH STATEMENT
    EXECUTE FUNCTION brassclaw_protect_monty_boot_catalogue();
