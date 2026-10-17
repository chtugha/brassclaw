-- Definitive host rejections are not cancellation and never imply VM execution.
-- Preserve old cancelled evidence and started-task outcomes; a new unstarted
-- rejection is exactly a classified failure, with no fabricated execution report.
ALTER TABLE brassclaw_monty_task_admissions
    DROP CONSTRAINT brassclaw_monty_task_admissions_check;
ALTER TABLE brassclaw_monty_task_admissions
    ADD CONSTRAINT brassclaw_monty_task_admissions_check CHECK (
        (phase='reserved' AND started_at IS NULL AND settled_at IS NULL AND outcome IS NULL)
        OR (phase='started' AND started_at IS NOT NULL AND settled_at IS NULL AND outcome IS NULL)
        OR (phase='settled' AND settled_at IS NOT NULL AND outcome IS NOT NULL
            AND jsonb_typeof(outcome)='object'
            AND (started_at IS NOT NULL
                OR COALESCE(outcome->>'status'='failed' AND outcome->>'reason_kind'='task_cancelled',false)
                OR outcome IN (
                    '{"status":"failed","reason_kind":"monty_input_rejected"}'::jsonb,
                    '{"status":"failed","reason_kind":"monty_admission_backpressure"}'::jsonb,
                    '{"status":"failed","reason_kind":"monty_service_closed"}'::jsonb,
                    '{"status":"failed","reason_kind":"monty_admission_rejected"}'::jsonb))));

CREATE OR REPLACE FUNCTION protect_monty_task_admissions() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.phase <> 'reserved' THEN
            RAISE EXCEPTION 'Monty admissions must begin reserved' USING ERRCODE='23514';
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'Monty admission evidence cannot be removed' USING ERRCODE='23514';
    END IF;
    IF (to_jsonb(OLD) - ARRAY['phase','started_at','settled_at','outcome']) IS DISTINCT FROM
       (to_jsonb(NEW) - ARRAY['phase','started_at','settled_at','outcome']) THEN
        RAISE EXCEPTION 'Monty admission identity is immutable' USING ERRCODE='23514';
    END IF;
    IF to_jsonb(OLD) = to_jsonb(NEW) THEN RETURN NEW; END IF;
    IF OLD.phase='reserved' AND NEW.phase='started' THEN RETURN NEW; END IF;
    IF OLD.phase='reserved' AND NEW.phase='settled'
        AND NEW.started_at IS NOT DISTINCT FROM OLD.started_at
        AND NEW.outcome->>'status'='failed'
        AND (NEW.outcome->>'reason_kind'='task_cancelled' OR NEW.outcome IN (
            '{"status":"failed","reason_kind":"monty_input_rejected"}'::jsonb,
            '{"status":"failed","reason_kind":"monty_admission_backpressure"}'::jsonb,
            '{"status":"failed","reason_kind":"monty_service_closed"}'::jsonb,
            '{"status":"failed","reason_kind":"monty_admission_rejected"}'::jsonb)) THEN
        RETURN NEW;
    END IF;
    IF OLD.phase='started' AND NEW.phase='settled'
        AND NEW.started_at IS NOT DISTINCT FROM OLD.started_at THEN RETURN NEW; END IF;
    RAISE EXCEPTION 'Monty admission settlement is monotonic and immutable' USING ERRCODE='23514';
END;
$$;
