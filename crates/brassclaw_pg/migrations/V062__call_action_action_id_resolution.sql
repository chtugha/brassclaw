--
-- call_action step-name -> action_id UUID resolution (Phase G.7 / Q-G6).
--
-- FIND-P7-13 + FIND-P9-06: `call_action` steps reference nested Actions BY
-- NAME (`step->>'action'`), not by UUID. Phase G.6 migrated the runtime
-- `call_action` handler to prefer an explicit `action_id` (UUID) via
-- `__fetch_component__(action_id, 16)`, falling back to
-- `__resolve_component_by_name__(name, 16)` (§0.9 Option B) when
-- `action_id` is absent. This migration back-fills `action_id` onto every
-- existing `call_action` step so the fast Option A path is taken at runtime;
-- names that cannot be resolved within their scope are left without an
-- `action_id` and degrade gracefully to the Option B name-lookup path.
--
-- Upgrade note (task rule "do not blindly remove upgrades; document, repair,
-- complete or leave them"): the original plan (lines 5299, 5334) described
-- this as a "data-only" script "NOT a Flyway migration" run manually at
-- deploy. The Phase G subplan (Q-G6) completes it as a tracked, idempotent
-- refinery migration instead — so it is versioned, re-runnable, recorded in
-- `refinery_schema_history`, and applied atomically with the rest of the
-- migration chain. The data-migration SQL itself is unchanged from the plan.
--
-- Idempotent: the CASE condition gates on `step->>'action_id' IS NULL`, so
-- re-running never re-resolves a step that already carries an `action_id`
-- (whether set by this migration or authored manually afterwards). The
-- outer `WHERE ... @> '[{"type":"call_action"}]'::jsonb` restricts the
-- UPDATE to rows that actually contain a call_action step, so action rows
-- with no nested calls are never rewritten (and `jsonb_agg` never sees an
-- empty input — it would return NULL for zero rows, which the WHERE
-- prevents). Scope-isolated: `a2` is matched on the full
-- (tenant_id, user_id, agent_id, project_id) tuple of `a1`, so a foreign-
-- tenant Action name can never resolve (SEC-01).
--
-- Post-migration audit (run manually AFTER deploy — refinery does not
-- surface SELECT rows to the operator). Finds call_action steps whose
-- `action_id` is still null/unresolved so they can be reviewed + fixed or
-- accepted (they will use the Option B name-lookup fallback at runtime):
--
--   SELECT a.id, a.name, step->>'action' AS unresolved_action_name
--   FROM reborn_actions a, jsonb_array_elements(a.steps) AS step
--   WHERE step->>'type' = 'call_action'
--     AND (step->>'action_id' IS NULL OR step->>'action_id' = 'null');

UPDATE reborn_actions a1
SET steps = (
    SELECT jsonb_agg(
        CASE
            WHEN step->>'type' = 'call_action'
             AND step->>'action' IS NOT NULL
             AND step->>'action_id' IS NULL
            THEN step || jsonb_build_object('action_id',
                (SELECT a2.id::text
                 FROM reborn_actions a2
                 WHERE a2.name     = step->>'action'
                   AND a2.tenant_id  = a1.tenant_id
                   AND a2.user_id    = a1.user_id
                   AND a2.agent_id   = a1.agent_id
                   AND a2.project_id = a1.project_id
                 LIMIT 1)
            )
            ELSE step
        END
    )
    FROM jsonb_array_elements(a1.steps) AS step
)
WHERE a1.steps @> '[{"type":"call_action"}]'::jsonb;
