-- Extend the existing preparation store. /1 bytes and scope retain their
-- original meaning. /2 is a source-only prerequisite, never general Q1/Q2.
DO $$
DECLARE c RECORD; removed INTEGER := 0;
BEGIN
    FOR c IN SELECT conname FROM pg_constraint
        WHERE conrelid='reborn_component_validator_selections'::regclass AND contype='c'
        AND (pg_get_constraintdef(oid) LIKE '%component-validator-selection/1%'
             OR pg_get_constraintdef(oid) LIKE '%preparation-only%')
    LOOP
        EXECUTE format('ALTER TABLE reborn_component_validator_selections DROP CONSTRAINT %I',c.conname);
        removed := removed + 1;
    END LOOP;
    IF removed <> 2 THEN
        RAISE EXCEPTION 'unexpected validator preparation predecessor checks' USING ERRCODE='23514';
    END IF;
END $$;

ALTER TABLE reborn_component_validator_selections
ADD CONSTRAINT validator_preparation_version_scope CHECK ((
    (selection_bytes::jsonb->>'format'='component-validator-selection/1'
        AND selection_bytes::jsonb->>'scope'='preparation-only'
        AND NOT (selection_bytes::jsonb ? 'qualification'))
    OR
    (selection_bytes::jsonb->>'format'='component-validator-selection/2'
        AND selection_bytes::jsonb->>'scope'='qualified-pure-source-prerequisite'
        AND selection_bytes::jsonb->'candidate'->>'class_code'='22'
        AND jsonb_typeof(selection_bytes::jsonb->'qualification')='object'
        AND selection_bytes::jsonb->'qualification'->>'profile'='pure_source_structure'
        AND ((selection_bytes::jsonb->'qualification') - ARRAY['profile','workflow_proof','prerequisite_proofs','required_checks_checksum','worker_checksum','package_checksum'])='{}'::jsonb
        AND jsonb_typeof(selection_bytes::jsonb->'qualification'->'workflow_proof')='object'
        AND jsonb_typeof(selection_bytes::jsonb->'qualification'->'prerequisite_proofs')='array'
        AND jsonb_array_length(selection_bytes::jsonb->'qualification'->'prerequisite_proofs')>0
        AND selection_bytes::jsonb->'qualification'->>'required_checks_checksum' ~ '^[0-9a-f]{64}$'
        AND selection_bytes::jsonb->'qualification'->>'worker_checksum' ~ '^[0-9a-f]{64}$'
        AND selection_bytes::jsonb->'qualification'->>'package_checksum' ~ '^[0-9a-f]{64}$')
) IS TRUE);
