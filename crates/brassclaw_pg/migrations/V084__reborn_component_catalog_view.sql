-- V084__reborn_component_catalog_view.sql
--
-- Materializes `reborn_component_catalog` as a real Postgres VIEW.
--
-- Background: the original Step 6.1 plan (commit 869f7f0f) explicitly named
-- the UNION ALL query in `retrieval_source.rs::fetch_for_turn` as "the
-- reborn_component_catalog read model (PERF-05)" and deferred creating a
-- named object for it ("a separate named PG VIEW (V047) can be added later
-- if the interceptor path needs it"). That follow-up was never done — V047
-- was reused for an unrelated migration — so `reborn_component_catalog` has
-- never existed as a queryable object, only as informal documentation of a
-- query shape duplicated ad hoc by callers (and, incorrectly, referenced by
-- name in AGENTS.md as if it were a real table).
--
-- This migration closes that gap: it defines `reborn_component_catalog` as a
-- read-only, unscoped UNION ALL VIEW across all 14 prompt-bearing component
-- tables, so any caller (Settings API, retrieval code, ad hoc audits) can
-- query one relation instead of re-deriving the union. It intentionally does
-- NOT bake in the per-request scope/validation/consumer-tag filtering that
-- `fetch_for_turn` applies at runtime (tenant_id/user_id/agent_id/project_id,
-- `validation_status = 'validated'`, `05:validator` exclusion, `source`
-- OR-scope) — those are request-shaped predicates, not properties of the
-- catalog itself. Callers apply their own WHERE clause on top of this view,
-- exactly as `PgSettingsListingService::list()` already does per-table.
--
-- Excludes `reborn_tools` (class 0): tools carry no prompt text and are
-- deliberately excluded from `class_code_to_table` for the same reason.
--
-- Column shape mirrors the 7-column row shape produced by
-- `component_item_from_row` in retrieval_source.rs, plus the raw scope /
-- consumer_tags / source / validation_status columns so callers can filter:
--
--   id, class_code, prompt_uid, name, description, effective_content,
--   override_prompt_creation, validation_status, consumer_tags, source,
--   tenant_id, user_id, agent_id, project_id
--
-- `effective_content` uses the exact same per-class expression as
-- `class_code_to_table` in retrieval_source.rs — this migration and that
-- function must be kept in sync if either changes. There is no compile-time
-- link between them; a future column rename here requires a matching Rust
-- change, and vice versa.

CREATE OR REPLACE VIEW reborn_component_catalog AS
SELECT
    id,
    class_code::int                                            AS class_code,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), body)        AS effective_content,
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_skills
-- class_code IN (1, 2, 3, 10, 50)

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(prior_knowledge_content, description),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_extensions_unified
-- class_code IN (4, 5, 6, 7, 8, 9)

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(prior_knowledge_content, description),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_actions
-- class_code = 16

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_specs
-- class_code = 12

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_tool_skills
-- class_code = 13

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_plans
-- class_code = 14

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_summaries
-- class_code = 15

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_docus
-- class_code = 17

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_lessons
-- class_code = 18

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_issues
-- class_code = 19

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_notes
-- class_code = 20

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), ''),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_recipes
-- class_code = 21

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), content),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_python_code
-- class_code = 22

UNION ALL

SELECT
    id,
    class_code::int,
    prompt_uid,
    name,
    description,
    COALESCE(NULLIF(prior_knowledge_content, ''), overview_doc),
    override_prompt_creation,
    validation_status,
    consumer_tags,
    source,
    tenant_id, user_id, agent_id, project_id
FROM reborn_extension_catalogues;
-- class_code = 23

COMMENT ON VIEW reborn_component_catalog IS
    'Unscoped UNION ALL read model over all 14 prompt-bearing component '
    'tables (classes 1-3, 4-9, 10, 12-23, 50). Excludes reborn_tools (class '
    '0, no prompt text). Mirrors the class_code_to_table dispatch in '
    'retrieval_source.rs — keep both in sync. Callers MUST apply their own '
    'scope (tenant_id/user_id/agent_id/project_id) and validation_status '
    'filters; this view applies none.';
