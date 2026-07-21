-- V034__reborn_monty_vm_settings.sql
-- Monty VM runtime settings table for BrassClaw Reborn (Phase 5 Step 5.1a).
--
-- One row per scope tuple via upsert (spec §7 Q8 resolved — single row per scope).
-- Stores resource limits, active orchestrator pointer, prior-knowledge token
-- budget, and retention windows.  All values fall back to compiled-in constants
-- when a DB row is absent (DB-less / RamSource mode).
--
-- Spec references: §3.10, §4, §7 Q7 (q4_retention_days), §7 Q8 (single row),
-- §7 Q21 (forensic_packet_retention_days), PERF-16 (drain + admission control).
--
-- All writes and reads must filter on the full scope tuple
-- (tenant_id, user_id, agent_id, project_id).

CREATE TABLE IF NOT EXISTS reborn_monty_vm_settings (
    id                              UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — single row per scope (unique constraint enforces this).
    tenant_id                       TEXT        NOT NULL,
    user_id                         TEXT        NOT NULL,
    agent_id                        TEXT        NOT NULL,
    project_id                      TEXT        NOT NULL,

    -- Orchestrator execution time limit (seconds).
    -- BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS env var overrides this in DB-less mode.
    max_duration_secs               INT         NOT NULL DEFAULT 300
        CHECK (max_duration_secs BETWEEN 30 AND 3600),

    -- Memory allocation limit for the Python VM (bytes).
    max_allocations                 BIGINT      NOT NULL DEFAULT 5000000
        CHECK (max_allocations > 0),

    -- Maximum memory the Python VM may use (bytes). Default = 128 MiB.
    max_memory_bytes                BIGINT      NOT NULL DEFAULT 134217728
        CHECK (max_memory_bytes > 0),

    -- Auto-rollback threshold: consecutive orchestrator failures before
    -- the previous validated version is restored.
    failure_rollback_threshold      SMALLINT    NOT NULL DEFAULT 3
        CHECK (failure_rollback_threshold > 0),

    -- FK to reborn_orchestrators.id (the active orchestrator).
    -- NULL = use compiled-in DEFAULT_ORCHESTRATOR.
    -- Must point to a Validated row without 05:validator (enforced at application layer).
    active_orchestrator_id          UUID,

    -- Prior-knowledge token budget (§3.13 — replaces the hardcoded 5-doc limit).
    -- The __assemble_prior_knowledge__ assembler truncates to this budget.
    prior_knowledge_token_budget    INT         NOT NULL DEFAULT 2000
        CHECK (prior_knowledge_token_budget > 0),

    -- Q4 rejection-queue retention window (days) before terminal wipe (§7 Q7).
    q4_retention_days               INT         NOT NULL DEFAULT 30
        CHECK (q4_retention_days > 0),

    -- ForensicPacket retention window (days) before pruning (§7 Q21).
    -- Set to 0 to disable pruning.
    forensic_packet_retention_days  INT         NOT NULL DEFAULT 90
        CHECK (forensic_packet_retention_days >= 0),

    updated_at                      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_monty_vm_settings_pk PRIMARY KEY (id),

    -- Enforce single row per scope.
    CONSTRAINT reborn_monty_vm_settings_scope_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id)
);

-- Scope index for fast upsert and reads.
CREATE INDEX IF NOT EXISTS reborn_monty_vm_settings_scope_idx
    ON reborn_monty_vm_settings (tenant_id, user_id, agent_id, project_id);

-- ── updated_at trigger ──────────────────────────────────────────────────────

CREATE TRIGGER reborn_monty_vm_settings_updated_at
    BEFORE UPDATE ON reborn_monty_vm_settings
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ── Seed global system defaults ─────────────────────────────────────────────
-- The host-level composition layer seeds per-user rows via upsert on first
-- access.  This system row serves as the spec-compliant fallback reference.

INSERT INTO reborn_monty_vm_settings
    (tenant_id, user_id, agent_id, project_id,
     max_duration_secs, max_allocations, max_memory_bytes,
     failure_rollback_threshold, active_orchestrator_id,
     prior_knowledge_token_budget, q4_retention_days,
     forensic_packet_retention_days)
VALUES
    ('__system__', '__system__', '__system__', '__system__',
     300, 5000000, 134217728,
     3, NULL,
     2000, 30,
     90)
ON CONFLICT ON CONSTRAINT reborn_monty_vm_settings_scope_unique DO NOTHING;
