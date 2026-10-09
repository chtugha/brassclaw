-- Separate internal admissions. A reservation is never automatically reclaimed
-- on restart/lease expiry: provider dispatch or submission may have happened.
CREATE TABLE brassclaw_monty_review_work (
    source_run_id UUID PRIMARY KEY REFERENCES brassclaw_monty_review_events(run_id),
    attempt_id UUID NOT NULL UNIQUE,
    owner_id UUID NOT NULL,
    event_checksum TEXT NOT NULL,
    selection_bytes TEXT NOT NULL,
    prefix_bytes TEXT NOT NULL,
    model_identity TEXT NOT NULL,
    phase TEXT NOT NULL DEFAULT 'reserved' CHECK (phase IN
        ('reserved','qualified','claimed','model_dispatching','model_returned',
         'submitted','acknowledged','incomplete','failed','uncertain')),
    bundle_bytes TEXT,
    request_bytes TEXT,
    response_bytes TEXT,
    model_dispatch_count INTEGER NOT NULL DEFAULT 0 CHECK (model_dispatch_count BETWEEN 0 AND 1),
    receipt_bytes TEXT,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    CHECK (phase NOT IN ('model_dispatching','model_returned','submitted','acknowledged')
        OR (model_dispatch_count=1 AND request_bytes IS NOT NULL)),
    CHECK (phase NOT IN ('model_returned','submitted','acknowledged') OR response_bytes IS NOT NULL),
    CHECK (phase NOT IN ('acknowledged','incomplete') OR receipt_bytes IS NOT NULL)
);
CREATE TABLE brassclaw_monty_review_operations (
    attempt_id UUID NOT NULL REFERENCES brassclaw_monty_review_work(attempt_id),
    operation TEXT NOT NULL,
    input_bytes TEXT NOT NULL,
    input_checksum TEXT NOT NULL CHECK (input_checksum=encode(sha256(convert_to(input_bytes,'UTF8')),'hex')),
    output_bytes TEXT,
    output_checksum TEXT,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    answered_at TIMESTAMPTZ,
    PRIMARY KEY(attempt_id,operation),
    CHECK ((output_bytes IS NULL AND output_checksum IS NULL AND answered_at IS NULL)
        OR (output_bytes IS NOT NULL AND output_checksum=encode(sha256(convert_to(output_bytes,'UTF8')),'hex')
            AND answered_at IS NOT NULL))
);
CREATE FUNCTION protect_monty_review_work() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'review work retention cannot be removed' USING ERRCODE='23514';
    END IF;
    IF ROW(NEW.source_run_id,NEW.attempt_id,NEW.owner_id,NEW.event_checksum,
        NEW.selection_bytes,NEW.prefix_bytes,NEW.model_identity,NEW.recorded_at)
        IS DISTINCT FROM ROW(OLD.source_run_id,OLD.attempt_id,OLD.owner_id,OLD.event_checksum,
        OLD.selection_bytes,OLD.prefix_bytes,OLD.model_identity,OLD.recorded_at)
        OR NEW.model_dispatch_count < OLD.model_dispatch_count
        OR (OLD.bundle_bytes IS NOT NULL AND NEW.bundle_bytes IS DISTINCT FROM OLD.bundle_bytes)
        OR (OLD.request_bytes IS NOT NULL AND NEW.request_bytes IS DISTINCT FROM OLD.request_bytes)
        OR (OLD.response_bytes IS NOT NULL AND NEW.response_bytes IS DISTINCT FROM OLD.response_bytes)
        OR (OLD.receipt_bytes IS NOT NULL AND NEW.receipt_bytes IS DISTINCT FROM OLD.receipt_bytes)
        OR (OLD.phase IN ('acknowledged','incomplete','failed','uncertain') AND NEW IS DISTINCT FROM OLD)
        OR NOT (NEW.phase=OLD.phase OR
            (OLD.phase='reserved' AND NEW.phase IN ('qualified','incomplete','failed','uncertain')) OR
            (OLD.phase='qualified' AND NEW.phase IN ('claimed','failed','uncertain')) OR
            (OLD.phase='claimed' AND NEW.phase IN ('model_dispatching','failed','uncertain')) OR
            (OLD.phase='model_dispatching' AND NEW.phase IN ('model_returned','failed','uncertain')) OR
            (OLD.phase='model_returned' AND NEW.phase IN ('submitted','failed','uncertain')) OR
            (OLD.phase='submitted' AND NEW.phase IN ('acknowledged','failed','uncertain'))) THEN
        RAISE EXCEPTION 'invalid review state transition' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER protect_monty_review_work BEFORE UPDATE OR DELETE ON brassclaw_monty_review_work
    FOR EACH ROW EXECUTE FUNCTION protect_monty_review_work();
CREATE TRIGGER no_truncate_monty_review_work BEFORE TRUNCATE ON brassclaw_monty_review_work
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_review_work();
CREATE FUNCTION protect_monty_review_operations() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' OR ROW(NEW.attempt_id,NEW.operation,NEW.input_bytes,NEW.input_checksum,NEW.recorded_at)
        IS DISTINCT FROM ROW(OLD.attempt_id,OLD.operation,OLD.input_bytes,OLD.input_checksum,OLD.recorded_at)
        OR (OLD.output_bytes IS NOT NULL AND NEW IS DISTINCT FROM OLD) THEN
        RAISE EXCEPTION 'review operation evidence is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER protect_monty_review_operations BEFORE UPDATE OR DELETE ON brassclaw_monty_review_operations
    FOR EACH ROW EXECUTE FUNCTION protect_monty_review_operations();
CREATE TRIGGER no_truncate_monty_review_operations BEFORE TRUNCATE ON brassclaw_monty_review_operations
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_review_operations();

CREATE TABLE brassclaw_monty_review_settlements (
    attempt_id UUID PRIMARY KEY REFERENCES brassclaw_monty_review_work(attempt_id),
    settlement_bytes TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TRIGGER immutable_monty_review_settlements BEFORE UPDATE OR DELETE ON brassclaw_monty_review_settlements
    FOR EACH ROW EXECUTE FUNCTION protect_monty_review_events();
CREATE TRIGGER no_truncate_monty_review_settlements BEFORE TRUNCATE ON brassclaw_monty_review_settlements
    FOR EACH STATEMENT EXECUTE FUNCTION protect_monty_review_events();
