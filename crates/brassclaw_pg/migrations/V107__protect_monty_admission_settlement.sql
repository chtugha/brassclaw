-- Admission identity and acknowledged terminal outcomes are durable no-replay
-- evidence. These guards grant no dispatch or Tool permission.
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
    IF (to_jsonb(OLD) - ARRAY['phase','started_at','settled_at','outcome'])
        IS DISTINCT FROM
       (to_jsonb(NEW) - ARRAY['phase','started_at','settled_at','outcome']) THEN
        RAISE EXCEPTION 'Monty admission identity is immutable' USING ERRCODE='23514';
    END IF;
    -- Repeated check_and_start is an exact no-op. It cannot replace times,
    -- outcome or identity or reopen a settled record.
    IF to_jsonb(OLD) = to_jsonb(NEW) THEN
        RETURN NEW;
    END IF;
    IF OLD.phase = 'reserved' AND NEW.phase = 'started' THEN
        RETURN NEW;
    END IF;
    IF OLD.phase = 'reserved' AND NEW.phase = 'settled'
        AND NEW.started_at IS NOT DISTINCT FROM OLD.started_at
        AND NEW.outcome->>'status' = 'failed'
        AND NEW.outcome->>'reason_kind' = 'task_cancelled' THEN
        RETURN NEW;
    END IF;
    IF OLD.phase = 'started' AND NEW.phase = 'settled'
        AND NEW.started_at IS NOT DISTINCT FROM OLD.started_at THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'Monty admission settlement is monotonic and immutable' USING ERRCODE='23514';
END;
$$;

CREATE OR REPLACE TRIGGER brassclaw_monty_admissions_monotonic
    BEFORE INSERT OR UPDATE OR DELETE ON brassclaw_monty_task_admissions
    FOR EACH ROW EXECUTE FUNCTION protect_monty_task_admissions();
CREATE OR REPLACE TRIGGER brassclaw_monty_admissions_no_truncate
    BEFORE TRUNCATE ON brassclaw_monty_task_admissions
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_task_admissions();
