-- V021__triggers.sql
-- Rename trigger_records → brassclaw_triggers on existing deployments.
-- On fresh deployments the DO block skips the rename and CREATE TABLE creates it.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'trigger_records'
          AND table_schema = current_schema()
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'brassclaw_triggers'
          AND table_schema = current_schema()
    ) THEN
        ALTER TABLE trigger_records RENAME TO brassclaw_triggers;
        ALTER INDEX IF EXISTS trigger_records_state_next_run_at_idx
            RENAME TO brassclaw_triggers_state_next_run_at_idx;
        ALTER INDEX IF EXISTS trigger_records_tenant_created_at_idx
            RENAME TO brassclaw_triggers_tenant_created_at_idx;
        ALTER INDEX IF EXISTS trigger_records_scoped_list_idx
            RENAME TO brassclaw_triggers_scoped_list_idx;
        ALTER INDEX IF EXISTS trigger_records_active_fire_slot_idx
            RENAME TO brassclaw_triggers_active_fire_slot_idx;
    END IF;
END $$;

-- All columns match the TRIGGER_COLUMNS constant in postgres.rs exactly.
-- Date columns are TEXT (RFC-3339 formatted) — do NOT change to TIMESTAMPTZ;
-- PostgresTriggerRepository uses fmt_ts() string round-trips.
CREATE TABLE IF NOT EXISTS brassclaw_triggers (
    trigger_id             TEXT        NOT NULL,
    tenant_id              TEXT        NOT NULL,
    creator_user_id        TEXT        NOT NULL,
    agent_id               TEXT,
    project_id             TEXT,
    name                   TEXT        NOT NULL,
    source                 TEXT        NOT NULL,
    schedule_expression    TEXT        NOT NULL,
    completion_policy      TEXT        NOT NULL,
    prompt                 TEXT        NOT NULL,
    state                  TEXT        NOT NULL,
    next_run_at            TEXT        NOT NULL,
    last_run_at            TEXT,
    last_fired_slot        TEXT,
    last_status            TEXT,
    active_fire_slot       TEXT,
    active_run_ref         TEXT,
    created_at             TEXT        NOT NULL,
    PRIMARY KEY (tenant_id, trigger_id)
);

CREATE INDEX IF NOT EXISTS brassclaw_triggers_state_next_run_at_idx
    ON brassclaw_triggers (state, next_run_at, tenant_id, trigger_id);
CREATE INDEX IF NOT EXISTS brassclaw_triggers_tenant_created_at_idx
    ON brassclaw_triggers (tenant_id, created_at, trigger_id);
CREATE INDEX IF NOT EXISTS brassclaw_triggers_scoped_list_idx
    ON brassclaw_triggers (tenant_id, creator_user_id, agent_id, project_id, created_at, trigger_id);
CREATE INDEX IF NOT EXISTS brassclaw_triggers_active_fire_slot_idx
    ON brassclaw_triggers (active_fire_slot, tenant_id, trigger_id)
    WHERE active_fire_slot IS NOT NULL;

-- Add updated_at column + trigger (not in original schema bootstrap).
DO $$
BEGIN
    -- Scope the trigger lookup to the brassclaw_triggers table by OID to avoid
    -- matching a same-named trigger on a different table in a multi-schema DB.
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger t
        JOIN pg_class c ON c.oid = t.tgrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE t.tgname = 'brassclaw_triggers_updated_at'
          AND c.relname = 'brassclaw_triggers'
          AND n.nspname = current_schema()
    ) THEN
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'brassclaw_triggers'
              AND column_name = 'updated_at'
              AND table_schema = current_schema()
        ) THEN
            ALTER TABLE brassclaw_triggers
                ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
        END IF;
        CREATE TRIGGER brassclaw_triggers_updated_at
            BEFORE UPDATE ON brassclaw_triggers
            FOR EACH ROW EXECUTE FUNCTION set_updated_at();
    END IF;
END $$;

-- Local-dev bootstrap access table.
CREATE TABLE IF NOT EXISTS brassclaw_local_access (
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT        NOT NULL,
    project_id  TEXT        NOT NULL,
    role        TEXT        NOT NULL,
    status      TEXT        NOT NULL
        CHECK (status IN ('active','inactive')),
    source      TEXT        NOT NULL,
    created_at  TEXT        NOT NULL,
    updated_at  TEXT        NOT NULL,
    PRIMARY KEY (tenant_id, user_id, agent_id, project_id)
);
