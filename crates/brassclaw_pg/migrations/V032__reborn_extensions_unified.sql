-- V032__reborn_extensions_unified.sql
-- Unified Extensions table for BrassClaw Reborn (Phase 4, Step 4.1).
--
-- Merges today's Extensions, DocPlans, and non-Recipe/non-ToolSkill Misc
-- entries into one table.  Recipes go to `reborn_recipes` (V033, class 21);
-- ToolSkills go to `reborn_tool_skills` (V037, class 13).  This table carries
-- classes 04-09 plus Plan (class 14) and Docu (class 17) as the "monty"-class
-- plan orchestration fragments produced by DocPlan dissection.
--
-- Spec references: §3.3, §3.7 (class codes), §3.9 (consumer_tags),
-- §3.13/§3.14 (prior_knowledge_content + override_prompt_creation — SCH-02),
-- Phase 4 Step 4.1, §7 Q15.
--
-- Class codes for extensions (from spec §3.7):
--   04 = Extension (Rusty)
--   05 = Extension (Monty)
--   06 = Extension (MCP-Server)
--   07 = Extension (MCP-Client)
--   08 = Extension (LLM)
--   09 = Extension (Misc — non-Recipe only; Recipes go to reborn_recipes)
--
-- consumer_tags[] default per class (§3.9):
--   Rusty  (04) → {00:rusty}
--   Monty  (05) → {01:monty, 02:orchestrator}
--   MCP-S  (06) → {01:monty, 02:orchestrator}
--   MCP-C  (07) → {01:monty, 02:orchestrator}
--   LLM    (08) → {03:llm}
--   Misc   (09) → {02:orchestrator}
--
-- All rows additionally receive 05:validator until Step-2 validation removes it.
--
-- prior_knowledge_content + override_prompt_creation (SCH-02):
--   Solution-class extensions that carry self-contained prior-knowledge text
--   set prior_knowledge_content to that text and override_prompt_creation = true.
--   Default is NULL / false — the standard content-is-king assembly path.
--
-- Validation lifecycle (§3.5.1):
--   pending → auto_passed/auto_failed → validated (or rejected → garbage)
--
-- All writes and reads must filter on the full scope tuple
-- (tenant_id, user_id, agent_id, project_id).

CREATE TYPE IF NOT EXISTS reborn_extension_class AS ENUM (
    'rusty',
    'monty',
    'mcp_server',
    'mcp_client',
    'llm',
    'misc'
);

CREATE SEQUENCE IF NOT EXISTS reborn_extensions_unified_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_extensions_unified (
    -- Primary key
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — all writes and reads must filter on the full tuple.
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    -- Core content (validation-gated)
    name                    TEXT        NOT NULL
        CHECK (name ~ '^[a-z0-9]([a-z0-9_.-]*[a-z0-9])?$' AND length(name) BETWEEN 1 AND 128),
    description             TEXT        NOT NULL
        CHECK (length(description) BETWEEN 1 AND 1024),

    -- Extension class — determines class_code and consumer_tags default.
    -- Constrained to the reborn_extension_class enum above.
    class                   reborn_extension_class  NOT NULL,

    -- Payload: manifest JSONB, recipe step list, plan document body, or
    -- extension-specific configuration — shape depends on `class`.
    payload                 JSONB       NOT NULL DEFAULT '{}',

    -- Prior-knowledge content (§3.13/§3.14 — SCH-02 fix).
    -- When non-NULL, used as the component's prior-knowledge text instead of
    -- assembling from `payload`.
    prior_knowledge_content TEXT,
    -- If true, the Solution Override path is taken: this component's
    -- prior_knowledge_content replaces the standard assembly.  Default false.
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

    -- Classification (§3.7)
    -- class_code derived from `class`:
    --   rusty     → 04   monty → 05   mcp_server → 06
    --   mcp_client → 07  llm   → 08   misc        → 09
    class_code              SMALLINT    NOT NULL
        CHECK (class_code IN (4, 5, 6, 7, 8, 9)),
    -- prompt_uid: monotonic sequence for deterministic prompt assembly order.
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_extensions_unified_prompt_uid_seq'),

    -- Consumer tags (validation-gated, §3.9).
    -- Each entry must match '^[0-9]{2}(:[a-z0-9-]+)?$'.
    -- '05:validator' greys out all other tags until Step-2 validation.
    consumer_tags           TEXT[]      NOT NULL DEFAULT '{}'
        CHECK (
            array_length(array(
                SELECT t FROM unnest(consumer_tags) t
                WHERE t !~ '^[0-9]{2}(:[a-z0-9-]+)?$'
            ), 1) IS NULL
        ),

    -- Intent examples for the unified intent system (§3.12).
    -- Array of {input: text, class: 1|2|3} objects.
    intent_examples         JSONB,

    -- Validation state (§3.5, §3.5.1)
    validation_status       TEXT        NOT NULL DEFAULT 'pending'
        CHECK (validation_status IN (
            'pending', 'auto_passed', 'auto_failed', 'validated',
            'review_requested', 'rejected', 'garbage', 'upgrade_queued'
        )),
    validation_errors       TEXT[]      NOT NULL DEFAULT '{}',
    review_feedback         TEXT,
    review_attempts         SMALLINT    NOT NULL DEFAULT 0,
    rejected_at             TIMESTAMPTZ,
    -- Derived queue code; updated by queue lifecycle logic.
    -- 'q1_auto' | 'q2_manual' | 'q3_revision' | 'q4_rejection' | 'garbage'
    queue_code              TEXT,

    -- Provenance / lineage (immediate-write)
    source                  TEXT        NOT NULL DEFAULT 'imported',
    content_hash            TEXT,
    similarity_parent_id    UUID,
    replaces_id             UUID,
    parent_version          TEXT,
    last_audit_at           TIMESTAMPTZ,
    audit_failure_count     SMALLINT    NOT NULL DEFAULT 0,
    parent_mission_id       UUID,

    -- Timestamps
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT reborn_extensions_unified_pk PRIMARY KEY (id),

    -- Scope + name uniqueness.
    CONSTRAINT reborn_extensions_unified_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- ── Indexes ────────────────────────────────────────────────────────────────

-- Scope isolation — the most common filter.
CREATE INDEX IF NOT EXISTS reborn_extensions_unified_scope_idx
    ON reborn_extensions_unified (tenant_id, user_id, agent_id, project_id);

-- Validation queue lookups — used by the WebUI validation tab.
CREATE INDEX IF NOT EXISTS reborn_extensions_unified_scope_status_idx
    ON reborn_extensions_unified (tenant_id, user_id, agent_id, project_id, validation_status);

-- Deterministic prompt assembly (§3.7).
CREATE INDEX IF NOT EXISTS reborn_extensions_unified_scope_class_uid_idx
    ON reborn_extensions_unified (tenant_id, user_id, agent_id, project_id, class_code, prompt_uid);

-- Tag-gated consumer retrieval (§3.9).
CREATE INDEX IF NOT EXISTS reborn_extensions_unified_consumer_tags_gin_idx
    ON reborn_extensions_unified USING GIN (consumer_tags);

-- ── updated_at trigger ──────────────────────────────────────────────────────

CREATE TRIGGER reborn_extensions_unified_updated_at
    BEFORE UPDATE ON reborn_extensions_unified
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ── class_code consistency trigger ──────────────────────────────────────────
-- Ensures class_code always matches the `class` enum value at insert/update.

CREATE OR REPLACE FUNCTION reborn_extensions_unified_sync_class_code()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.class_code := CASE NEW.class
        WHEN 'rusty'      THEN 4
        WHEN 'monty'      THEN 5
        WHEN 'mcp_server' THEN 6
        WHEN 'mcp_client' THEN 7
        WHEN 'llm'        THEN 8
        WHEN 'misc'       THEN 9
    END;
    RETURN NEW;
END;
$$;

CREATE TRIGGER reborn_extensions_unified_sync_class_code
    BEFORE INSERT OR UPDATE OF class ON reborn_extensions_unified
    FOR EACH ROW EXECUTE FUNCTION reborn_extensions_unified_sync_class_code();
