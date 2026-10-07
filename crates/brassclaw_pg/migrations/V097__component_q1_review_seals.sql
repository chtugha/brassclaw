-- Bind each Q1 pass to the exact candidate and queue submission inspected.
-- Historical pending Q2 reviews have no such evidence: retain their proposals,
-- counters and feedback, but require Q1 again rather than blessing current data.
ALTER TABLE reborn_validation_queue
    ADD COLUMN IF NOT EXISTS q1_component_bytes TEXT,
    ADD COLUMN IF NOT EXISTS q1_component_checksum TEXT,
    ADD COLUMN IF NOT EXISTS q1_queue_bytes TEXT,
    ADD COLUMN IF NOT EXISTS q1_queue_checksum TEXT;

UPDATE reborn_validation_queue
    SET state = 1,
        validation_errors = array_append(validation_errors,
            'Q1 revalidation required: exact candidate evidence was not retained')
    WHERE state = 2;

ALTER TABLE reborn_validation_queue
    ADD CONSTRAINT q1_review_seal_complete CHECK (
        (q1_component_bytes IS NULL AND q1_component_checksum IS NULL
            AND q1_queue_bytes IS NULL AND q1_queue_checksum IS NULL AND state <> 2)
        OR (q1_component_bytes IS NOT NULL AND q1_component_checksum IS NOT NULL
            AND q1_queue_bytes IS NOT NULL AND q1_queue_checksum IS NOT NULL
            AND q1_component_checksum = encode(sha256(convert_to(q1_component_bytes, 'UTF8')), 'hex')
            AND q1_queue_checksum = encode(sha256(convert_to(q1_queue_bytes, 'UTF8')), 'hex'))
    );
