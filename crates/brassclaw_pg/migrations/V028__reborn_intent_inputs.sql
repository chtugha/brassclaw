-- V028__reborn_intent_inputs.sql
-- Intent input table for the unified intent system (spec §3.12, §4).
--
-- One row per (scope, input_text, input_class, component_id).
-- Normalized schema (PERF-04): no uuid[] arrays; every mapping is a discrete row.
--
-- Input classes (spec §3.12 Q10):
--   1 = single word
--   2 = partial phrase (2-4 words, no terminal punctuation)
--   3 = full sentence (≥5 words OR ends with ./!/?  )
--   4 = keyword fallback (created by RetrievalEngine only, never by query classifier)
--
-- Security invariants (§6.1):
--   SEC-05: score hard cap 100; learned_llm rows flagged needs_review=true.
--   PERF-03: score increment is atomic UPDATE … RETURNING (no SELECT+UPDATE).
--
-- pg_trgm is required (CREATE EXTENSION IF NOT EXISTS pg_trgm).
-- Install script creates this extension before running migrations.

CREATE TABLE IF NOT EXISTS reborn_intent_inputs (
    -- Primary key
    id                  UUID        NOT NULL DEFAULT gen_random_uuid(),

    -- Scope tuple — all writes and reads filter on the full 4-tuple.
    tenant_id           TEXT        NOT NULL,
    user_id             TEXT        NOT NULL,
    agent_id            TEXT        NOT NULL,
    project_id          TEXT        NOT NULL,

    -- The input text as entered/learned.
    input_text          TEXT        NOT NULL
        CHECK (length(input_text) BETWEEN 1 AND 2048),

    -- Input class (1-4 per spec §3.12 Q10).
    input_class         SMALLINT    NOT NULL
        CHECK (input_class BETWEEN 1 AND 4),

    -- The component this input maps to.
    component_id        UUID        NOT NULL,
    component_class_code INT        NOT NULL,

    -- Relevance score. Hard cap = 100 (SEC-05).
    score               INT         NOT NULL DEFAULT 1
        CHECK (score BETWEEN 1 AND 100),

    -- Provenance of this input row.
    source              TEXT        NOT NULL DEFAULT 'seeded'
        CHECK (source IN ('seeded', 'learned_user', 'learned_llm', 'learned_fallback')),

    -- learned_llm rows are flagged for review (SEC-05).
    needs_review        BOOLEAN     NOT NULL DEFAULT false,

    -- Timestamps
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (id),

    -- Uniqueness: one row per (scope, text, class, component).
    CONSTRAINT reborn_intent_inputs_scope_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id, input_text, input_class, component_id)
);

-- B-tree index for exact-match intent resolution (PERF-01).
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_scope_text_class_idx
    ON reborn_intent_inputs (tenant_id, user_id, agent_id, project_id, input_text, input_class);

-- Index for (scope, input_text) — used by disambiguation + learning queries.
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_scope_text_idx
    ON reborn_intent_inputs (tenant_id, user_id, agent_id, project_id, input_text);

-- Index for component purge on component delete.
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_scope_component_idx
    ON reborn_intent_inputs (tenant_id, user_id, agent_id, project_id, component_id);

-- GIN trigram index for future fuzzy partial matching (Q16 resolved).
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_text_trgm_idx
    ON reborn_intent_inputs USING GIN (input_text gin_trgm_ops);
