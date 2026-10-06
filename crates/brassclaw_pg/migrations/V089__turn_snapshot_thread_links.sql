-- Related parent/child threads share one CAS-protected turn-state aggregate.
-- Transcript rows and thread IDs remain separate; only runner state is linked.
-- Existing independent snapshots retain their keys. Never merge two aggregates.
CREATE TABLE IF NOT EXISTS brassclaw_turn_snapshot_threads (
    tenant_id TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    snapshot_thread_id TEXT NOT NULL,
    PRIMARY KEY (tenant_id, thread_id),
    FOREIGN KEY (tenant_id, snapshot_thread_id)
        REFERENCES brassclaw_turns (tenant_id, turn_id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS brassclaw_turn_snapshot_threads_owner_idx
    ON brassclaw_turn_snapshot_threads (tenant_id, snapshot_thread_id);
