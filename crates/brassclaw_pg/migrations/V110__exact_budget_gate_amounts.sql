-- V019's six-decimal projection rounded otherwise exact payload amounts.
-- Restore only proven rounding differences. Malformed or conflicting records
-- require repair; do not invent an amount or overwrite unrelated corruption.
DO $$
DECLARE
    gate RECORD;
    requested JSONB;
    amount_text TEXT;
    amount NUMERIC;
BEGIN
    FOR gate IN SELECT id, payload, requested_amount FROM brassclaw_budget_gates LOOP
        requested := gate.payload #> '{needed,requested}';
        amount_text := requested ->> 'value';
        IF requested IS NULL OR jsonb_typeof(requested) <> 'object'
            OR amount_text IS NULL OR length(amount_text) > 32
            OR (
                (requested ->> 'kind' = 'decimal'
                 AND jsonb_typeof(requested -> 'value') = 'string'
                 AND amount_text ~ '^-?[0-9]+(\.[0-9]+)?$')
                OR
                (requested ->> 'kind' = 'integer'
                 AND jsonb_typeof(requested -> 'value') = 'number'
                 AND amount_text ~ '^[0-9]+$')
            ) IS NOT TRUE
        THEN
            RAISE EXCEPTION 'Malformed budget gate amount for %', gate.id;
        END IF;
        amount := amount_text::numeric;
        IF gate.requested_amount <> round(amount, 6) THEN
            RAISE EXCEPTION 'Inconsistent budget gate amount for %', gate.id;
        END IF;
    END LOOP;
END $$;

ALTER TABLE brassclaw_budget_gates ALTER COLUMN requested_amount TYPE NUMERIC;
UPDATE brassclaw_budget_gates
SET requested_amount = (payload #>> '{needed,requested,value}')::numeric
WHERE requested_amount <> (payload #>> '{needed,requested,value}')::numeric;
