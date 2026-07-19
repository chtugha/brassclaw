-- V009__processes.sql
CREATE TABLE IF NOT EXISTS brassclaw_processes (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    runtime     TEXT        NOT NULL
        CHECK (runtime IN ('mcp','first_party','system')),
    status      TEXT        NOT NULL
        CHECK (status IN ('running','completed','failed','killed')),
    spec        JSONB       NOT NULL,
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_processes_tenant_status_idx
    ON brassclaw_processes (tenant_id, status);
CREATE TRIGGER brassclaw_processes_updated_at
    BEFORE UPDATE ON brassclaw_processes
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Insert-only; results are never modified after write.
CREATE TABLE IF NOT EXISTS brassclaw_process_results (
    process_id  TEXT        NOT NULL PRIMARY KEY REFERENCES brassclaw_processes(id) ON DELETE RESTRICT,
    tenant_id   TEXT        NOT NULL,
    status      TEXT        NOT NULL
        CHECK (status IN ('running','completed','failed','killed')),
    output      JSONB,
    output_ref  TEXT,
    error_kind  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
