-- V019__budget_gates.sql
-- status: BudgetGateStatus snake_case values.
-- NOTE: 'cancelled' NOT 'denied' — the enum has Cancelled, not Denied.
CREATE TABLE IF NOT EXISTS brassclaw_budget_gates (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    gate_kind   TEXT        NOT NULL,
    status      TEXT        NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','cancelled','expired')),
    requested_amount NUMERIC(18,6) NOT NULL,
    payload     JSONB       NOT NULL DEFAULT '{}',
    expires_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_budget_gates_tenant_status_idx
    ON brassclaw_budget_gates (tenant_id, status) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS brassclaw_budget_gates_run_idx
    ON brassclaw_budget_gates (run_id) WHERE run_id IS NOT NULL;
CREATE TRIGGER brassclaw_budget_gates_updated_at
    BEFORE UPDATE ON brassclaw_budget_gates
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
