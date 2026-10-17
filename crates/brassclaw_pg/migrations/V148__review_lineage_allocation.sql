-- Review lineage and allocation order differ when unactivated drafts exist.
-- Preserve exact v1 subjects and accept explicitly observed allocation in v2.
DO $$
DECLARE item record; removed integer := 0;
BEGIN
    FOR item IN SELECT conname FROM pg_constraint
        WHERE conrelid='reborn_component_review_submissions'::regclass AND contype='c'
        AND (pg_get_constraintdef(oid) LIKE '%base_version + 1%'
             OR pg_get_constraintdef(oid) LIKE '%component-review-submission/1%'
             OR (pg_get_constraintdef(oid) LIKE '%base_version IS NOT NULL%'
                 AND pg_get_constraintdef(oid) LIKE '%candidate_version = 1%'))
    LOOP
        EXECUTE format('ALTER TABLE reborn_component_review_submissions DROP CONSTRAINT %I',item.conname);
        removed := removed + 1;
    END LOOP;
    IF removed <> 3 THEN RAISE EXCEPTION 'review lineage constraint inventory mismatch'; END IF;
END $$;
ALTER TABLE reborn_component_review_submissions
    ADD CONSTRAINT review_lineage_order CHECK (
        CASE WHEN base_version IS NULL THEN candidate_version=1
        ELSE base_version>0 AND candidate_version>base_version END),
    ADD CONSTRAINT review_subject_allocation CHECK (
        CASE subject_bytes::jsonb->>'format'
        WHEN 'component-review-submission/1' THEN
            NOT (subject_bytes::jsonb ? 'expected_previous_version')
            AND candidate_version::numeric=COALESCE(base_version::numeric,0)+1
        WHEN 'component-review-submission/2' THEN
            CASE WHEN jsonb_typeof(subject_bytes::jsonb->'expected_previous_version')='number'
                AND subject_bytes::jsonb->>'expected_previous_version' ~ '^(0|[1-9][0-9]*)$'
            THEN (subject_bytes::jsonb->>'expected_previous_version')::numeric<9223372036854775807
                AND candidate_version::numeric=(subject_bytes::jsonb->>'expected_previous_version')::numeric+1
                AND COALESCE(base_version::numeric,0)<=(subject_bytes::jsonb->>'expected_previous_version')::numeric
            ELSE false END
        ELSE false END IS TRUE);
