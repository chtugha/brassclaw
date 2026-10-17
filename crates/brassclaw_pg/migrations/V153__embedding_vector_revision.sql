-- Missing vector_revision denotes the initial configured embedding contract.
-- Do not guess a space for historic untagged vectors or rewrite provider data.
-- Later noninitial revisions are explicit operator/model-configuration changes.
CREATE OR REPLACE FUNCTION brassclaw_embedding_model_contracts_valid(models JSONB) RETURNS BOOLEAN
LANGUAGE plpgsql IMMUTABLE AS $valid$
DECLARE entry RECORD;
BEGIN
 IF models IS NULL OR jsonb_typeof(models) <> 'object' THEN RETURN false; END IF;
 FOR entry IN SELECT key,value FROM jsonb_each(models) LOOP
  IF entry.key = '' OR btrim(entry.key) <> entry.key OR entry.key ~ '[[:cntrl:]]'
    OR jsonb_typeof(entry.value) <> 'object'
    OR NOT entry.value ?& ARRAY['dimension','cache_max_entries']
    OR entry.value - ARRAY['dimension','cache_max_entries','vector_revision'] <> '{}'::jsonb THEN RETURN false; END IF;
  IF jsonb_typeof(entry.value->'dimension') <> 'number'
    OR jsonb_typeof(entry.value->'cache_max_entries') <> 'number'
    OR (entry.value->>'dimension') !~ '^[0-9]+$'
    OR (entry.value->>'cache_max_entries') !~ '^[0-9]+$' THEN RETURN false; END IF;
  IF (entry.value->>'dimension')::numeric NOT BETWEEN 1 AND 4294967295
    OR (entry.value->>'cache_max_entries')::numeric NOT BETWEEN 0 AND 4294967295 THEN RETURN false; END IF;
  IF entry.value ? 'vector_revision' THEN
   IF jsonb_typeof(entry.value->'vector_revision') <> 'number'
    OR (entry.value->>'vector_revision') !~ '^[0-9]+$' THEN RETURN false; END IF;
   IF (entry.value->>'vector_revision')::numeric NOT BETWEEN 1 AND 4294967295 THEN RETURN false; END IF;
  END IF;
 END LOOP;
 RETURN true;
END
$valid$;
