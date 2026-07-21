-- V033__reborn_recipes.sql
-- Recipes table for BrassClaw Reborn (Phase 4 Step 4.1, class 21).
--
-- Recipes are solution-class components: a trigger condition + ordered
-- steps referencing validated ToolSkills/Skills.  They have their own
-- dedicated table (NOT folded into reborn_extensions_unified) because they
-- have a distinct schema — trigger, steps, solution-class override columns.
--
-- Spec references: §3.3, §3.7 (class 21), §3.9 (consumer_tags), §3.12
-- (intent_examples), §3.13/§3.14 (prior_knowledge_content +
-- override_prompt_creation), Phase 4 Step 4.1, §7 Q15.
--
-- class_code is always 21 (Recipe, from spec §3.7).
--
-- consumer_tags[] default: {02:orchestrator, 03:llm} + 05:validator until
-- validated.  '05:validator' greys out all other tags (§3.5.1); the tag is
-- removed by the Q2 manual-validation step (AutoPassed → Validated).
--
-- prior_knowledge_content + override_prompt_creation (§3.13/§3.14):
--   Recipes are solution-class; setting prior_knowledge_content + setting
--   override_prompt_creation = true causes the Solution Override path so the
--   recipe's prior-knowledge text is used verbatim instead of assembling from
--   `steps`.
--
-- Validation lifecycle (§3.5.1):
--   pending → auto_passed/auto_failed → validated (or rejected → garbage)
--
-- The RecipeLookup trait boundary (brassclaw_turns) is preserved; this table
-- is the backing store for class-21 lookups from Phase 4 onwards.
--
-- All writes and reads must filter on the full scope tuple
-- (tenant_id, user_id, agent_id, project_id).

CREATE SEQUENCE IF NOT EXISTS reborn_recipes_prompt_uid_seq;

CREATE TABLE IF NOT EXISTS reborn_recipes (
    -- Primary key
    id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — all writes and reads must filter on the full tuple.
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    agent_id                TEXT        NOT NULL,
    project_id              TEXT        NOT NULL,

    -- Core content (validation-gated)
    name                    TEXT        NOT NULL
        CHECK (name ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' AND length(name) BETWEEN 1 AND 64),
    description             TEXT        NOT NULL
        CHECK (length(description) BETWEEN 1 AND 1024),

    -- Trigger condition: {type: "exact"|"pattern"|"keyword", payload: ...}
    -- Validated by ComponentValidator (require_activation_criteria = true).
    trigger                 JSONB,

    -- Ordered step list: [{skill: name, params: {...}}] referencing validated
    -- ToolSkills/Skills.  At least one step is required.
    steps                   JSONB       NOT NULL DEFAULT '[]',

    -- Processing status (independent of validation_status).
    status                  TEXT        NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'archived', 'draft')),

    -- Prior-knowledge content (§3.13/§3.14 — Solution Override path).
    -- When non-NULL, used instead of assembling from `steps` + `description`.
    prior_knowledge_content TEXT,
    -- If true, this recipe's prior_knowledge_content replaces standard assembly.
    override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

    -- Classification (§3.7)
    -- class_code for Recipes is always 21.
    class_code              SMALLINT    NOT NULL DEFAULT 21
        CHECK (class_code = 21),
    -- prompt_uid: monotonic sequence for deterministic prompt assembly order.
    prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_recipes_prompt_uid_seq'),

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

    -- Reward / scoring (immediate-write, §3.6)
    -- Wilson lower bound for tier classification.
    tier                    TEXT        NOT NULL DEFAULT 'seedling'
        CHECK (tier IN ('seedling', 'growing', 'mature', 'candidate')),
    usage_count             INT         NOT NULL DEFAULT 0,
    success_count           INT         NOT NULL DEFAULT 0,
    failure_count           INT         NOT NULL DEFAULT 0,
    -- Wilson lower bound at last recomputation.
    wilson_lower            FLOAT8      NOT NULL DEFAULT 0.0,

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
    source                  TEXT        NOT NULL DEFAULT 'authored',
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

    CONSTRAINT reborn_recipes_pk PRIMARY KEY (id),

    -- Scope + name uniqueness.
    CONSTRAINT reborn_recipes_scope_name_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, name)
);

-- ── Indexes ────────────────────────────────────────────────────────────────

-- Scope isolation — the most common filter.
CREATE INDEX IF NOT EXISTS reborn_recipes_scope_idx
    ON reborn_recipes (tenant_id, user_id, agent_id, project_id);

-- Validation queue lookups — used by the WebUI validation tab.
CREATE INDEX IF NOT EXISTS reborn_recipes_scope_status_idx
    ON reborn_recipes (tenant_id, user_id, agent_id, project_id, validation_status);

-- Deterministic prompt assembly (§3.7 — class 21, ordered by prompt_uid).
CREATE INDEX IF NOT EXISTS reborn_recipes_scope_uid_idx
    ON reborn_recipes (tenant_id, user_id, agent_id, project_id, prompt_uid);

-- Tag-gated consumer retrieval (§3.9).
CREATE INDEX IF NOT EXISTS reborn_recipes_consumer_tags_gin_idx
    ON reborn_recipes USING GIN (consumer_tags);

-- Tier + validation combined — used by RecipeLookup to filter Validated rows.
CREATE INDEX IF NOT EXISTS reborn_recipes_scope_validated_idx
    ON reborn_recipes (tenant_id, user_id, agent_id, project_id, tier)
    WHERE validation_status = 'validated';

-- ── updated_at trigger ──────────────────────────────────────────────────────

CREATE TRIGGER reborn_recipes_updated_at
    BEFORE UPDATE ON reborn_recipes
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
