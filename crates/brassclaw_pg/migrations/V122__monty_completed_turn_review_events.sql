-- Durable post-turn handoff, not a review, approval, Tool grant or replay job.
-- The admission's settlement transaction produces exactly one immutable event.
-- A future review Recipe must resolve/validate the complete evidence and use its
-- own admission. It must never restore the original task's dispatch authority.
ALTER TABLE brassclaw_monty_task_admissions
    ADD COLUMN IF NOT EXISTS trusted_internal_turn BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE IF NOT EXISTS brassclaw_monty_review_events (
    run_id UUID PRIMARY KEY REFERENCES brassclaw_monty_task_admissions(run_id),
    event_bytes TEXT NOT NULL,
    event_checksum TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (event_checksum = encode(sha256(convert_to(event_bytes, 'UTF8')), 'hex')),
    CHECK ((jsonb_typeof(event_bytes::jsonb) = 'object') IS TRUE),
    CHECK ((event_bytes::jsonb->>'format' = 'monty-completed-turn-review/1') IS TRUE),
    CHECK (((event_bytes::jsonb->>'run_id')::uuid = run_id) IS TRUE)
);

CREATE OR REPLACE FUNCTION protect_monty_review_events() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Monty completed-turn review evidence is immutable'
        USING ERRCODE='23514';
END;
$$;
CREATE OR REPLACE TRIGGER brassclaw_monty_review_events_immutable
    BEFORE UPDATE OR DELETE ON brassclaw_monty_review_events
    FOR EACH ROW EXECUTE FUNCTION protect_monty_review_events();
CREATE OR REPLACE TRIGGER brassclaw_monty_review_events_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_review_events
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_review_events();

CREATE OR REPLACE FUNCTION retain_monty_completed_turn_review() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    event_text TEXT;
BEGIN
    -- Only actual No-Match routing creates Tier-2 learning work. A missing
    -- route, matching error or failed matched Recipe cannot masquerade as it.
    IF NEW.phase <> 'settled' OR OLD.phase = 'settled' OR NEW.trusted_internal_turn
        OR NEW.outcome->'execution'->>'intent_outcome' IS DISTINCT FROM 'no_match' THEN
        RETURN NEW;
    END IF;
    event_text := jsonb_build_object(
        'format', 'monty-completed-turn-review/1',
        'run_id', NEW.run_id,
        'turn_id', NEW.turn_id,
        'scope', NEW.scope,
        'accepted_message_ref', NEW.accepted_message_ref,
        'runner_id', NEW.runner_id,
        'settled_at', NEW.settled_at,
        'outcome', NEW.outcome,
        -- Pin packet bytes now: later forensic upserts cannot change the input
        -- of an already enqueued review. Ordering and tenant/run correlation
        -- are explicit. Original prompt capture alone is not complete evidence.
        'model_packets', COALESCE((
            SELECT jsonb_agg(to_jsonb(p) ORDER BY p.iteration, p.id)
            FROM brassclaw_forensic_packets p
            WHERE p.tenant_id = NEW.scope->>'tenant_id'
                AND p.run_id = NEW.run_id::text
        ), '[]'::jsonb),
        'tool_invocations', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
                'invocation_id', i.invocation_id, 'recipe_id', i.recipe_id,
                'step_id', i.step_id, 'selection_checksum', i.selection_checksum,
                'arguments_bytes', i.arguments_bytes,
                'arguments_checksum', i.arguments_checksum,
                'attempt_count', i.attempt_count, 'phase', i.phase,
                'answer_bytes', i.answer_bytes, 'answer_checksum', i.answer_checksum,
                'recorded_at', i.recorded_at, 'answered_at', i.answered_at
            ) ORDER BY i.recorded_at, i.invocation_id)
            FROM brassclaw_monty_tool_invocations i WHERE i.run_id = NEW.run_id
        ), '[]'::jsonb),
        'recipe_selections', COALESCE((
            SELECT jsonb_agg(jsonb_build_object(
                'recipe_id', s.recipe_id, 'selection_bytes', s.selection_bytes,
                'selection_checksum', s.selection_checksum
            ) ORDER BY s.recipe_id)
            FROM brassclaw_monty_recipe_selections s WHERE s.run_id = NEW.run_id
        ), '[]'::jsonb),
        -- Neither prompt tuples nor daily memory prove complete task data.
        -- The consumer must resolve transcript/model/effect references and
        -- retain its own reviewed bundle; unknown effects stay unknown.
        'evidence_complete', false
    )::text;
    INSERT INTO brassclaw_monty_review_events(run_id, event_bytes, event_checksum)
        VALUES (NEW.run_id, event_text,
            encode(sha256(convert_to(event_text, 'UTF8')), 'hex'));
    RETURN NEW;
END;
$$;
CREATE OR REPLACE TRIGGER brassclaw_monty_completed_turn_review
    AFTER UPDATE ON brassclaw_monty_task_admissions
    FOR EACH ROW EXECUTE FUNCTION retain_monty_completed_turn_review();

-- Upgrade retention: old settled No-Match admissions have no immutable packet
-- snapshot from their settlement time. Do not fabricate one or automatically
-- replay them; an explicit reconciliation owner must handle historical data.
