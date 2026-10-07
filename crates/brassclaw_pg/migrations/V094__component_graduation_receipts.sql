-- Preserve the actual legacy graduation evidence before the queue row is
-- deleted or a later upgrade replaces the live row. These receipts are NOT
-- skill-association-approval/1 records: the old queue has no exact dependency
-- graph, validator revision or behavioral acceptance references.
-- No live component FK: replacement/deletion must not erase retained evidence.
CREATE TABLE IF NOT EXISTS reborn_component_graduation_receipts (
    id UUID PRIMARY KEY,
    component_id UUID NOT NULL,
    component_class SMALLINT NOT NULL,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    queue_id UUID NOT NULL UNIQUE,
    q2_actor TEXT,
    component_bytes TEXT NOT NULL,
    component_checksum TEXT NOT NULL,
    queue_bytes TEXT NOT NULL,
    queue_checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (jsonb_typeof(component_bytes::jsonb) = 'object'),
    CHECK (jsonb_typeof(queue_bytes::jsonb) = 'object'),
    CONSTRAINT component_graduation_exact_component_checksum
        CHECK (component_checksum = encode(sha256(convert_to(component_bytes, 'UTF8')), 'hex')),
    CHECK (queue_checksum = encode(sha256(convert_to(queue_bytes, 'UTF8')), 'hex')),
    CHECK (((component_bytes::jsonb ->> 'id')::uuid = component_id) IS TRUE),
    CHECK (((component_bytes::jsonb ->> 'class_code')::int = component_class) IS TRUE),
    CHECK ((component_bytes::jsonb ->> 'validation_status' = 'validated') IS TRUE),
    CHECK ((component_bytes::jsonb ->> 'tenant_id' = tenant_id) IS TRUE),
    CHECK ((component_bytes::jsonb ->> 'user_id' = user_id) IS TRUE),
    CHECK ((component_bytes::jsonb ->> 'agent_id' = agent_id) IS TRUE),
    CHECK ((component_bytes::jsonb ->> 'project_id' = project_id) IS TRUE),
    CHECK (((queue_bytes::jsonb ->> 'id')::uuid = queue_id) IS TRUE),
    CHECK (((queue_bytes::jsonb ->> 'component_id')::uuid = component_id) IS TRUE),
    CHECK (((queue_bytes::jsonb ->> 'component_class')::int = component_class) IS TRUE),
    CHECK (((queue_bytes::jsonb ->> 'state')::int = 2) IS TRUE),
    CHECK ((queue_bytes::jsonb ->> 'tenant_id' = tenant_id) IS TRUE),
    CHECK ((queue_bytes::jsonb ->> 'user_id' = user_id) IS TRUE),
    CHECK ((queue_bytes::jsonb ->> 'agent_id' = agent_id) IS TRUE),
    CHECK ((queue_bytes::jsonb ->> 'project_id' = project_id) IS TRUE)
);

CREATE INDEX IF NOT EXISTS reborn_component_graduation_receipts_component_idx
    ON reborn_component_graduation_receipts (component_id, recorded_at, id);

CREATE FUNCTION protect_component_graduation_receipts() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'component graduation receipts are immutable'
        USING ERRCODE = '23514';
END;
$$;

CREATE TRIGGER reborn_component_graduation_receipts_immutable
    BEFORE UPDATE OR DELETE ON reborn_component_graduation_receipts
    FOR EACH ROW EXECUTE FUNCTION protect_component_graduation_receipts();

CREATE TRIGGER reborn_component_graduation_receipts_no_truncate
    BEFORE TRUNCATE ON reborn_component_graduation_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_graduation_receipts();
