-- V051__reborn_validation_queue.sql
-- Decision 2 / Phase A.5 — Validation Queue table.
--
-- Creates `reborn_validation_queue` (table + indexes ONLY). No data migration,
-- no graduation trigger, no legacy column DROPs — those land in V059 (Phase N):
--   * populate queue rows from the existing component tables
--   * add `last_graduation_at` to the scope cursor (reborn_monty_vm_settings)
--   * DROP the legacy `queue_code` / `review_attempts` / `review_feedback` /
--     `rejected_at` / `validation_errors` columns from the 13 component tables
--
-- Landing the table here (ahead of Phase N) means every component class —
-- including the new class 22 (Phase B) and class 23 (Phase C) — can enqueue
-- from its very first WebUI-authored save, not only after Phase N.
--
-- §0.23.5 fold-in: this migration ALSO adds `proposed_payload JSONB` (nullable)
-- — the upgrade-copy payload. When an edit to a *validated* component is
-- pending, the live validated row stays `validated` + served and a COPY of the
-- edited version is enqueued with `proposed_payload` set (NULL for
-- new-component submissions, where the component row itself is the payload at
-- `'pending'`). Q2 approval applies `proposed_payload` to the live row
-- (graduation *apply* logic wired in Phase N, §0.23.9); Q2 rejection discards
-- the copy. The queue's `UNIQUE(scope, component_id)` still holds — one pending
-- upgrade per component at a time (concurrent edits rejected while a copy is
-- queued).
--
-- Authoritative DDL: §0.18 (lines ~2128-2172). §0.18 takes precedence on any
-- discrepancy; `proposed_payload` is the only §0.23.5 addition on top of §0.18.
--
-- state values:
--   1 = Q1_pending              (just submitted, awaiting Gate 1)
--   2 = Q1_passed               (Gate 1 clean — awaiting Q2 manual review)
--   3 = rejected                (Q2 rejected, author may fix + resubmit)
--   4 = deletion_candidate      (counter ≥ threshold or manually condemned)
--
-- ⚠️ FIND-P9-08: state 2 = Gate 1 PASSED. Only Gate 1 sets state 2 — the
-- store's `pub(crate) gate1_pass` is the sole write path (Rust visibility
-- enforces the state-2 write invariant; any other writer is a security bug).
-- The Q2 reviewer approves from state 2 → `approve()` deletes the row
-- (graduation) in one transaction with the component-table UPDATE (FIND-P9-05).
--
-- ⚠️ FIND-P9-07: scope-first index ordering matches every query pattern (all
-- reads/writes filter scope first, then component_id / state / class).
-- component_id-first index order would be wrong.

CREATE TABLE IF NOT EXISTS reborn_validation_queue (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope — all reads and writes filter on the full tuple.
    tenant_id        TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    agent_id         TEXT NOT NULL,
    project_id       TEXT NOT NULL,

    -- The component this row tracks.
    component_id     UUID NOT NULL,
    component_class  SMALLINT NOT NULL,   -- class_code; for WebUI filtering

    -- Lifecycle state. 1=Q1_pending 2=Q1_passed 3=rejected 4=deletion_candidate.
    -- State 2 may only be written by the Gate 1 validator (FIND-P9-08).
    state            SMALLINT NOT NULL DEFAULT 1
        CHECK (state IN (1, 2, 3, 4)),

    -- Permanent rejection count. Never resets. Increments on each rejection.
    -- When `counter` reaches the configurable threshold (default 3) the row is
    -- auto-promoted to state 4 (deletion candidate) — §0.18.
    counter          INT NOT NULL DEFAULT 0,

    -- Human-readable feedback from the Q2 reviewer (populated on rejection).
    review_feedback  TEXT,

    -- Q1 error messages (populated on Q1 fail, cleared on Q1 pass).
    validation_errors TEXT[] NOT NULL DEFAULT '{}',

    -- §0.23.5: upgrade-copy payload. NULL for new-component submissions; set
    -- when an edit to a validated component is pending. Graduation *apply*
    -- logic lands in Phase N (§0.23.9).
    proposed_payload  JSONB,

    -- Timestamps.
    submitted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One queue row per component at any time (also enforces one pending
    -- upgrade per component at a time — §0.23.5).
    UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)
);

CREATE INDEX IF NOT EXISTS reborn_validation_queue_scope_state_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, state);

CREATE INDEX IF NOT EXISTS reborn_validation_queue_scope_class_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, component_class);

-- Partial index: state 4 (deletion candidates) for the cleanup job
-- (purge_deletion_candidates).
CREATE INDEX IF NOT EXISTS reborn_validation_queue_deletion_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id)
    WHERE state = 4;
