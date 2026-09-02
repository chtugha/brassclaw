-- V063: reborn_basic_prompt_store — per-scope prefix-cache with stored bundle text.
--
-- The bundle is assembled from validated component rows on operator demand
-- (POST /api/prefixes/base-prompt/regenerate) and stored here as bundle_json.
-- Per-turn Kohai and Sempai calls read bundle_json directly — no re-assembly
-- from raw component rows on every LLM call.
--
-- vLLM automatic prefix caching (APC) fires when the client sends the same
-- token sequence on consecutive turns. Storing the bundle text ensures every
-- turn sends the exact same bytes → byte-identical tokens → KV-cache hit.
--
-- fingerprint: sha256(bundle_text) — used to detect whether a re-assembly
-- produced identical output and to skip redundant DB writes.
--
-- is_stale: set true after any Q2 graduation so the Prefix Tab shows Regenerate.

CREATE TABLE IF NOT EXISTS reborn_basic_prompt_store (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         TEXT        NOT NULL DEFAULT '',
    user_id           TEXT        NOT NULL DEFAULT '',
    agent_id          TEXT        NOT NULL DEFAULT '',
    project_id        TEXT        NOT NULL DEFAULT '',
    -- The assembled bundle stored as a JSONB string value (not an object).
    -- Default '""' (empty JSON string) until the first assembly.
    bundle_json       JSONB       NOT NULL DEFAULT '""',
    -- sha256(bundle_text) for staleness detection without re-reading the bundle.
    fingerprint       TEXT        NOT NULL DEFAULT '',
    is_stale          BOOLEAN     NOT NULL DEFAULT false,
    assembled_at      TIMESTAMPTZ,
    -- Last time this bundle was sent to the Sempai gateway (pre-warm).
    prewarm_last_at   TIMESTAMPTZ,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT reborn_basic_prompt_store_scope_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id)
);

CREATE INDEX IF NOT EXISTS reborn_basic_prompt_store_scope_idx
    ON reborn_basic_prompt_store (tenant_id, user_id, agent_id, project_id);

-- §0.23.7: component UUID reference on interceptor forensic packets.
ALTER TABLE brassclaw_forensic_packets
    ADD COLUMN IF NOT EXISTS component_uuid UUID;

-- §0.23.8: validation-improve settings on reborn_monty_vm_settings.
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN IF NOT EXISTS validation_idle_threshold_minutes INT  NOT NULL DEFAULT 120,
    ADD COLUMN IF NOT EXISTS validation_improve_start_hour     INT  NOT NULL DEFAULT 15,
    ADD COLUMN IF NOT EXISTS validation_improve_enabled        BOOL NOT NULL DEFAULT true;
