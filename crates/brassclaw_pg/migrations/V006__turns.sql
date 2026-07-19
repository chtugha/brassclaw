-- V006__turns.sql
--
-- PgTurnStateStore uses a load-snapshot / CAS-write-back strategy that mirrors
-- FilesystemTurnStateStore: the entire TurnPersistenceSnapshot is stored as a
-- single JSONB payload blob keyed by (tenant_id, turn_id) where turn_id holds
-- the ThreadId.  A version counter enables optimistic CAS.
--
-- The `id` column is set equal to `turn_id` (the ThreadId acting as the
-- snapshot row's primary key).  The `run_id` column is nullable — individual
-- TurnRunRecord rows within the snapshot do carry run_id references, but the
-- snapshot row itself is not scoped to a single run.  The `status` column is
-- set to the sentinel value 'snapshot' for snapshot rows and is not
-- CHECK-constrained to TurnStatus values, since individual TurnRunRecord
-- statuses live inside the JSONB payload.
--
-- Per §4.7 FK invariant note: when PgTurnStateStore (or a companion indexer)
-- writes individual per-run rows, it must write the same ULID into both id
-- and run_id.  For snapshot rows the run_id is NULL.
CREATE TABLE IF NOT EXISTS brassclaw_turns (
    -- id: ThreadId for snapshot rows (= turn_id); TurnRunId for per-run rows.
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- run_id: NULL for snapshot rows; TurnRunId FK to brassclaw_runs(id) for
    -- per-run rows.  ON DELETE RESTRICT applies only when run_id IS NOT NULL.
    run_id      TEXT        REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    -- turn_id: ThreadId for snapshot rows; TurnId for per-run rows.
    turn_id     TEXT        NOT NULL,
    -- status: 'snapshot' for snapshot rows; snake_case TurnStatus for per-run
    -- rows (via heck::ToSnakeCase — RecoveryRequired → 'recovery_required').
    -- The CHECK is intentionally absent here so snapshot rows do not need to
    -- match individual TurnStatus variant names.
    status      TEXT        NOT NULL DEFAULT 'snapshot',
    -- payload: TurnPersistenceSnapshot JSONB for snapshot rows; TurnRunRecord
    -- JSONB for per-run rows.
    payload     JSONB       NOT NULL DEFAULT '{}',
    -- version: optimistic CAS counter for snapshot rows.  Starts at 1 on
    -- INSERT, incremented by each successful write_snapshot CAS.  Per-run rows
    -- always have version = 0 (not used for CAS).
    version     BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Snapshot rows are keyed by (tenant_id, turn_id) — one row per thread per tenant.
-- This unique index is the CAS target for PgTurnStateStore::write_snapshot and also
-- serves "all snapshot/run records for turn X" read queries; no separate non-unique
-- index is needed for (tenant_id, turn_id).
CREATE UNIQUE INDEX IF NOT EXISTS brassclaw_turns_snapshot_idx
    ON brassclaw_turns (tenant_id, turn_id);
-- tenant_id + run_id index: needed for retention sweeps.
CREATE INDEX IF NOT EXISTS brassclaw_turns_tenant_idx
    ON brassclaw_turns (tenant_id, run_id) WHERE run_id IS NOT NULL;
CREATE TRIGGER brassclaw_turns_updated_at
    BEFORE UPDATE ON brassclaw_turns
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
