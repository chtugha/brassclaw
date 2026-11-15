-- Retired untouched legacy seed requests remain inspectable, not active work.
-- The seeder owns exact canonical matching. This migration never selects by
-- source/status labels and never deletes a queue row or alters approval history.
CREATE TABLE reborn_retired_seed_review_requests (
    queue_id UUID PRIMARY KEY,
    component_id UUID NOT NULL,
    component_table TEXT NOT NULL,
    format TEXT NOT NULL DEFAULT 'retired-seed-review-request/1'
        CHECK (format='retired-seed-review-request/1'),
    reason TEXT NOT NULL DEFAULT 'exact canonical shipped seed; validation inherited from provisioning',
    queue_bytes TEXT NOT NULL CHECK ((queue_bytes::jsonb->>'id')::uuid=queue_id),
    component_bytes TEXT NOT NULL CHECK ((component_bytes::jsonb->>'id')::uuid=component_id),
    package_bytes TEXT NOT NULL CHECK (jsonb_typeof(package_bytes::jsonb)='object'),
    queue_checksum TEXT NOT NULL CHECK (queue_checksum=encode(sha256(convert_to(queue_bytes,'UTF8')),'hex')),
    component_checksum TEXT NOT NULL CHECK (component_checksum=encode(sha256(convert_to(component_bytes,'UTF8')),'hex')),
    package_checksum TEXT NOT NULL CHECK (package_checksum=encode(sha256(convert_to(package_bytes,'UTF8')),'hex')),
    retired_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER retired_seed_requests_immutable BEFORE UPDATE OR DELETE ON reborn_retired_seed_review_requests
    FOR EACH ROW EXECUTE FUNCTION protect_component_review_records();
CREATE TRIGGER retired_seed_requests_no_truncate BEFORE TRUNCATE ON reborn_retired_seed_review_requests
    FOR EACH STATEMENT EXECUTE FUNCTION protect_component_review_records();
