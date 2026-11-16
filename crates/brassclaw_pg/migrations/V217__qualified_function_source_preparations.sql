-- /1 and /2 retain their exact historical scope. /3 is a joint Skill/code
-- source prerequisite only: no full Q1, behavior, human Q2 or activation.
-- Rust additionally authenticates closed fields, exact graph and subject bytes.
ALTER TABLE reborn_component_validator_selections
DROP CONSTRAINT validator_preparation_version_scope;

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
    OR
    (selection_bytes::jsonb->>'format'='component-validator-selection/3'
        AND selection_bytes::jsonb->>'scope'='qualified-function-source-prerequisite'
        AND selection_bytes::jsonb->'candidate'->>'class_code' IN ('1','2','3','22')
        AND jsonb_typeof(selection_bytes::jsonb->'qualification')='object'
        AND selection_bytes::jsonb->'qualification'->>'profile'='function_source_structure'
        AND ((selection_bytes::jsonb->'qualification') - ARRAY['profile','workflow_proof','prerequisite_proofs','required_checks_checksum','worker_checksum','package_checksum'])='{}'::jsonb
        AND jsonb_typeof(selection_bytes::jsonb->'qualification'->'workflow_proof')='object'
        AND jsonb_typeof(selection_bytes::jsonb->'qualification'->'prerequisite_proofs')='array'
        AND jsonb_array_length(selection_bytes::jsonb->'qualification'->'prerequisite_proofs')>0
        AND selection_bytes::jsonb->'qualification'->>'required_checks_checksum' ~ '^[0-9a-f]{64}$'
        AND selection_bytes::jsonb->'qualification'->>'worker_checksum' ~ '^[0-9a-f]{64}$'
        AND selection_bytes::jsonb->'qualification'->>'package_checksum' ~ '^[0-9a-f]{64}$')
) IS TRUE);
