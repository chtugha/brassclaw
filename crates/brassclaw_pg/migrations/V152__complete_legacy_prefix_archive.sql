-- Preserve the complete current compatibility row, including its serving
-- generation, timings and update metadata. V142's historical partial archive
-- remains immutable; this successor never invents missing historical values.
-- No legacy serving pointer, scope, profile setting or qualification is changed.
WITH legacy AS (
    SELECT jsonb_build_object(
        'format','brassclaw-legacy-prefix-bundle/2',
        'original_row',to_jsonb(s),
        'qualification','legacy-unverified'
    )::text AS bytes
    FROM reborn_basic_prompt_store s
)
INSERT INTO reborn_prefix_generations(
    profile_name,generation_id,profile_revision,generation_kind,generation_bytes
)
SELECT p.name,encode(sha256(convert_to(l.bytes,'UTF8')),'hex'),p.revision,
       'legacy-bundle',l.bytes
FROM legacy l JOIN reborn_prefix_profiles p ON p.name='base-prompt'
ON CONFLICT DO NOTHING;
