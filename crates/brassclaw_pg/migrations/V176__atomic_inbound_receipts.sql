-- One canonical event receipt across threads. These are correlation records,
-- not admission/Tool grants. Exact message and receipt writes commit together.
CREATE TABLE IF NOT EXISTS brassclaw_inbound_message_receipts (
    tenant_id TEXT NOT NULL,
    receipt_key BYTEA NOT NULL,
    agent_id TEXT NOT NULL,
    project_id TEXT,
    owner_user_id TEXT,
    source_binding_id TEXT NOT NULL,
    external_event_id TEXT NOT NULL,
    thread_id TEXT NOT NULL REFERENCES brassclaw_session_threads(id) ON DELETE CASCADE,
    message_id UUID NOT NULL,
    PRIMARY KEY (tenant_id, receipt_key),
    CHECK (receipt_key = sha256(convert_to(jsonb_build_array(
        tenant_id,agent_id,project_id,owner_user_id,source_binding_id,external_event_id
    )::text, 'UTF8')))
);
CREATE INDEX IF NOT EXISTS brassclaw_inbound_message_receipts_thread_idx
    ON brassclaw_inbound_message_receipts(tenant_id,thread_id);

-- Legacy snapshots remain retained. Validate their receipt graph before
-- backfill; no row-order winner, silent skip or changed message identity.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM brassclaw_session_threads
        WHERE deleted_at IS NULL AND metadata ? 'inbound_idempotency'
          AND jsonb_typeof(metadata->'inbound_idempotency') IS DISTINCT FROM 'array') THEN
        RAISE EXCEPTION 'invalid legacy inbound receipt collection' USING ERRCODE='23514';
    END IF;
    IF EXISTS (SELECT 1 FROM brassclaw_session_threads
        WHERE deleted_at IS NULL AND CASE WHEN jsonb_typeof(metadata->'inbound_idempotency')='array'
              THEN jsonb_array_length(metadata->'inbound_idempotency') ELSE 0 END>0
          AND jsonb_typeof(metadata->'messages') IS DISTINCT FROM 'array') THEN
        RAISE EXCEPTION 'invalid legacy inbound message collection' USING ERRCODE='23514';
    END IF;
END $$;

CREATE TEMP TABLE _brassclaw_legacy_inbound_receipts ON COMMIT DROP AS
SELECT t.id AS thread_id,t.tenant_id,t.agent_id,t.project_id,
       t.metadata#>>'{record,scope,owner_user_id}' AS owner_user_id,
       t.user_id AS row_user_id,t.version AS snapshot_version,t.metadata#>'{record}' AS record,
       e.value AS receipt,e.value->>'message_id' AS message_id,
       e.value->>'source_binding_id' AS source_binding_id,
       e.value->>'external_event_id' AS external_event_id,
       sha256(convert_to(jsonb_build_array(t.tenant_id,t.agent_id,t.project_id,
           t.metadata#>>'{record,scope,owner_user_id}',e.value->>'source_binding_id',
           e.value->>'external_event_id')::text,'UTF8')) AS receipt_key
FROM brassclaw_session_threads t
CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN t.deleted_at IS NULL
    THEN COALESCE(t.metadata->'inbound_idempotency','[]'::jsonb) ELSE '[]'::jsonb END) e
WHERE t.deleted_at IS NULL;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM _brassclaw_legacy_inbound_receipts
        WHERE snapshot_version<0
           OR jsonb_typeof(receipt) IS DISTINCT FROM 'object'
           OR jsonb_typeof(receipt->'source_binding_id') IS DISTINCT FROM 'string'
           OR jsonb_typeof(receipt->'external_event_id') IS DISTINCT FROM 'string'
           OR jsonb_typeof(receipt->'thread_id') IS DISTINCT FROM 'string'
           OR jsonb_typeof(receipt->'message_id') IS DISTINCT FROM 'string'
           OR jsonb_typeof(record->'thread_id') IS DISTINCT FROM 'string'
           OR jsonb_typeof(record#>'{scope,tenant_id}') IS DISTINCT FROM 'string'
           OR jsonb_typeof(record#>'{scope,agent_id}') IS DISTINCT FROM 'string'
           OR (jsonb_typeof(record#>'{scope,project_id}') IS NOT NULL
               AND jsonb_typeof(record#>'{scope,project_id}') NOT IN ('string','null'))
           OR (jsonb_typeof(record#>'{scope,owner_user_id}') IS NOT NULL
               AND jsonb_typeof(record#>'{scope,owner_user_id}') NOT IN ('string','null'))
           OR (jsonb_typeof(receipt->'scope_owner_user_id') IS NOT NULL
               AND jsonb_typeof(receipt->'scope_owner_user_id') NOT IN ('string','null'))
           OR record->>'thread_id' IS DISTINCT FROM thread_id
           OR record#>>'{scope,tenant_id}' IS DISTINCT FROM tenant_id
           OR record#>>'{scope,agent_id}' IS DISTINCT FROM agent_id
           OR record#>>'{scope,project_id}' IS DISTINCT FROM project_id
           OR COALESCE(owner_user_id,chr(31)||'SYSTEM'||chr(31)) IS DISTINCT FROM row_user_id
           OR receipt->>'thread_id' IS DISTINCT FROM thread_id
           OR receipt->>'scope_tenant_id' IS DISTINCT FROM tenant_id
           OR receipt->>'scope_agent_id' IS DISTINCT FROM agent_id
           OR receipt->>'scope_owner_user_id' IS DISTINCT FROM owner_user_id) THEN
        RAISE EXCEPTION 'legacy inbound receipt identity mismatch' USING ERRCODE='23514';
    END IF;
END $$;
-- The unique index also rejects repeated identical legacy entries. A copied
-- receipt cannot silently become a second canonical event.
CREATE UNIQUE INDEX _brassclaw_legacy_inbound_receipts_key
    ON _brassclaw_legacy_inbound_receipts(tenant_id,receipt_key);

CREATE TEMP TABLE _brassclaw_legacy_inbound_messages ON COMMIT DROP AS
SELECT t.id AS thread_id,m.value->>'message_id' AS message_id,m.value AS message
FROM brassclaw_session_threads t
CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN t.deleted_at IS NULL AND EXISTS (
    SELECT 1 FROM _brassclaw_legacy_inbound_receipts r WHERE r.thread_id=t.id)
    THEN t.metadata->'messages' ELSE '[]'::jsonb END) m
WHERE t.deleted_at IS NULL AND EXISTS (
    SELECT 1 FROM _brassclaw_legacy_inbound_receipts r WHERE r.thread_id=t.id);
CREATE UNIQUE INDEX _brassclaw_legacy_inbound_messages_id
    ON _brassclaw_legacy_inbound_messages(thread_id,message_id);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM _brassclaw_legacy_inbound_receipts r
        LEFT JOIN _brassclaw_legacy_inbound_messages m
          ON m.thread_id=r.thread_id AND m.message_id=r.message_id
        WHERE m.message IS NULL
           OR jsonb_typeof(m.message->'thread_id') IS DISTINCT FROM 'string'
           OR m.message->>'thread_id' IS DISTINCT FROM r.thread_id
           OR m.message->>'kind' IS DISTINCT FROM 'user'
           OR jsonb_typeof(m.message->'actor_id') IS DISTINCT FROM 'string'
           OR m.message->>'source_binding_id' IS DISTINCT FROM r.source_binding_id) THEN
        RAISE EXCEPTION 'legacy inbound receipt message mismatch' USING ERRCODE='23514';
    END IF;
END $$;

INSERT INTO brassclaw_inbound_message_receipts
    (tenant_id,receipt_key,agent_id,project_id,owner_user_id,source_binding_id,external_event_id,thread_id,message_id)
SELECT tenant_id,receipt_key,agent_id,project_id,owner_user_id,source_binding_id,external_event_id,thread_id,message_id::uuid
FROM _brassclaw_legacy_inbound_receipts
ON CONFLICT (tenant_id,receipt_key) DO NOTHING;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM _brassclaw_legacy_inbound_receipts l
        LEFT JOIN brassclaw_inbound_message_receipts r
          ON r.tenant_id=l.tenant_id AND r.receipt_key=l.receipt_key
        WHERE r.thread_id IS DISTINCT FROM l.thread_id OR r.message_id IS DISTINCT FROM l.message_id::uuid
           OR r.agent_id IS DISTINCT FROM l.agent_id OR r.project_id IS DISTINCT FROM l.project_id
           OR r.owner_user_id IS DISTINCT FROM l.owner_user_id
           OR r.source_binding_id IS DISTINCT FROM l.source_binding_id
           OR r.external_event_id IS DISTINCT FROM l.external_event_id) THEN
        RAISE EXCEPTION 'conflicting canonical inbound receipt' USING ERRCODE='23514';
    END IF;
END $$;
-- Index only receipt metadata, never full model/provider transcript payloads.
CREATE INDEX IF NOT EXISTS brassclaw_thread_inbound_receipts_idx
    ON brassclaw_session_threads USING GIN ((metadata->'inbound_idempotency') jsonb_path_ops)
    WHERE deleted_at IS NULL;
