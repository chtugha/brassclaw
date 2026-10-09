-- Operator reduction rules use their supported port, not the retired MemoryDoc
-- adapter. Legacy V016 records are preserved and imported on first access by
-- the adapter, which can validate their exact historical project/title keys.
CREATE TABLE brassclaw_reduction_rulesets (
    tenant_id TEXT NOT NULL CHECK (tenant_id <> ''),
    user_id TEXT NOT NULL CHECK (user_id <> ''),
    project_id TEXT NOT NULL CHECK (project_id ~ '^[A-Za-z0-9_-]{1,64}$'),
    rules JSONB NOT NULL CHECK (jsonb_typeof(rules) = 'array'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, project_id)
);
CREATE TRIGGER brassclaw_reduction_rulesets_updated_at
    BEFORE UPDATE ON brassclaw_reduction_rulesets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
