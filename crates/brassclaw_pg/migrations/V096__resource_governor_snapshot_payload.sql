-- PgResourceGovernorStore persists the complete governor snapshot in the
-- __governor__ row. V011 created the numeric account ledger but omitted this
-- separate JSON payload. Existing numeric rows and their balances are retained.
-- NULL remains distinct from an empty snapshot: the store rejects a malformed
-- existing governor row instead of inventing limits or zeroing consumed spend.
ALTER TABLE brassclaw_resource_accounts
    ADD COLUMN IF NOT EXISTS payload JSONB;

ALTER TABLE brassclaw_resource_accounts
    ADD CONSTRAINT resource_governor_payload_object
    CHECK (payload IS NULL OR jsonb_typeof(payload) = 'object');
