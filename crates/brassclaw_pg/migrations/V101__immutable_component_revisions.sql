-- Retain exact execution-relevant authoring documents. These are unapproved
-- revisions, not an active catalogue or implicit migration of legacy approval.
-- No FK to mutable component tables: replacing/deleting those rows must not
-- destroy retained tasks' selected content.
CREATE TABLE reborn_component_revision_heads (
    component_id UUID PRIMARY KEY CHECK (component_id <> '00000000-0000-0000-0000-000000000000'),
    class_code SMALLINT NOT NULL CHECK (class_code BETWEEN 0 AND 10 OR class_code BETWEEN 12 AND 23 OR class_code = 50),
    last_version BIGINT NOT NULL DEFAULT 0 CHECK (last_version >= 0),
    UNIQUE (component_id, class_code)
);

CREATE TABLE reborn_component_revisions (
    component_id UUID NOT NULL,
    class_code SMALLINT NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    revision_bytes TEXT NOT NULL CHECK (octet_length(revision_bytes) BETWEEN 1 AND 8388608),
    checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (component_id, version),
    FOREIGN KEY (component_id, class_code) REFERENCES reborn_component_revision_heads(component_id, class_code),
    CHECK (checksum = encode(sha256(convert_to(revision_bytes, 'UTF8')), 'hex')),
    CHECK (jsonb_typeof(revision_bytes::jsonb) = 'object'),
    CHECK ((revision_bytes::jsonb ->> 'format' = 'component-revision/1') IS TRUE),
    CHECK (((revision_bytes::jsonb ->> 'uuid')::uuid = component_id) IS TRUE),
    CHECK (((revision_bytes::jsonb ->> 'class_code')::smallint = class_code) IS TRUE),
    CHECK ((jsonb_typeof(revision_bytes::jsonb -> 'document') = 'object') IS TRUE),
    CHECK ((jsonb_typeof(revision_bytes::jsonb -> 'dependencies') = 'array') IS TRUE),
    CHECK ((CASE WHEN class_code BETWEEN 1 AND 3 THEN
        jsonb_typeof(revision_bytes::jsonb -> 'association') = 'string'
        ELSE revision_bytes::jsonb -> 'association' = 'null'::jsonb END) IS TRUE)
);

CREATE FUNCTION protect_component_revisions() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'component revisions are immutable' USING ERRCODE = '23514';
END;
$$;
CREATE TRIGGER reborn_component_revisions_immutable
    BEFORE UPDATE OR DELETE ON reborn_component_revisions
    FOR EACH ROW EXECUTE FUNCTION protect_component_revisions();
CREATE TRIGGER reborn_component_revisions_no_truncate
    BEFORE TRUNCATE ON reborn_component_revisions
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_revisions();
