-- Embedding response/cache expectations become provider-revision data. Retain
-- old revisions, credentials, selections, tombstones and explicit overrides.
CREATE FUNCTION brassclaw_embedding_model_defaults() RETURNS JSONB
LANGUAGE sql IMMUTABLE AS $defaults$
SELECT '{
 "text-embedding-3-small":{"dimension":1536,"cache_max_entries":10000},
 "text-embedding-3-large":{"dimension":3072,"cache_max_entries":10000},
 "text-embedding-ada-002":{"dimension":1536,"cache_max_entries":10000},
 "amazon.titan-embed-text-v2:0":{"dimension":1024,"cache_max_entries":10000},
 "nomic-embed-text":{"dimension":768,"cache_max_entries":10000},
 "mxbai-embed-large":{"dimension":1024,"cache_max_entries":10000},
 "all-minilm":{"dimension":384,"cache_max_entries":10000}
}'::jsonb
$defaults$;

CREATE FUNCTION brassclaw_embedding_model_contracts_valid(models JSONB) RETURNS BOOLEAN
LANGUAGE plpgsql IMMUTABLE AS $valid$
DECLARE entry RECORD;
BEGIN
 IF models IS NULL OR jsonb_typeof(models) <> 'object' THEN RETURN false; END IF;
 FOR entry IN SELECT key,value FROM jsonb_each(models) LOOP
  IF entry.key = '' OR btrim(entry.key) <> entry.key OR entry.key ~ '[[:cntrl:]]'
    OR jsonb_typeof(entry.value) <> 'object'
    OR NOT entry.value ?& ARRAY['dimension','cache_max_entries']
    OR entry.value - ARRAY['dimension','cache_max_entries'] <> '{}'::jsonb THEN RETURN false; END IF;
  IF jsonb_typeof(entry.value->'dimension') <> 'number'
    OR jsonb_typeof(entry.value->'cache_max_entries') <> 'number'
    OR (entry.value->>'dimension') !~ '^[0-9]+$'
    OR (entry.value->>'cache_max_entries') !~ '^[0-9]+$' THEN RETURN false; END IF;
  IF (entry.value->>'dimension')::numeric NOT BETWEEN 1 AND 4294967295
    OR (entry.value->>'cache_max_entries')::numeric NOT BETWEEN 0 AND 4294967295 THEN RETURN false; END IF;
 END LOOP;
 RETURN true;
END
$valid$;

-- Only absent metadata is migrated. Existing explicit maps (including empty)
-- are operator decisions and are neither enlarged nor replaced.
WITH candidates AS (
 SELECT p.tenant_id,p.id,
  CASE WHEN p.definition->>'protocol' IN ('open_ai_completions','open_router','nearai','ollama','bedrock')
   THEN brassclaw_embedding_model_defaults() ELSE '{}'::jsonb END AS models,
  CASE WHEN s.value IS NOT NULL AND p.definition->>'protocol' IN ('open_ai_completions','open_router','nearai','ollama','bedrock')
   THEN COALESCE(NULLIF(m.value,''),p.definition->>'default_model') END AS selected_model
 FROM brassclaw_llm_providers p
 LEFT JOIN brassclaw_config s ON s.tenant_id=p.tenant_id AND s.key='embedding.provider_id'
  AND (s.value=p.id OR p.definition->'aliases' ? s.value)
 LEFT JOIN brassclaw_config m ON m.tenant_id=p.tenant_id AND m.key='embedding.model'
 WHERE NOT p.definition ? 'embedding_models'
)
-- A currently selected unknown name retains its old expected 1536/10000 shape
-- as explicit migration data, not a runtime fallback or remote-model evidence.
UPDATE brassclaw_llm_providers p SET definition=jsonb_set(p.definition,'{embedding_models}',
 CASE WHEN c.selected_model IS NOT NULL AND c.selected_model<>'' AND NOT c.models ? c.selected_model
  THEN c.models || jsonb_build_object(c.selected_model,jsonb_build_object('dimension',1536,'cache_max_entries',10000))
  ELSE c.models END)
FROM candidates c WHERE c.tenant_id=p.tenant_id AND c.id=p.id;

ALTER TABLE brassclaw_llm_providers ADD CONSTRAINT embedding_model_contracts_valid
 CHECK (NOT definition ? 'embedding_models' OR brassclaw_embedding_model_contracts_valid(definition->'embedding_models'));

-- Keep the historical initializer intact. Only its genuinely fresh insertion
-- receives installation-owned metadata. Retained inventories and later startup
-- are not rewritten, and no active role or credential is changed.
DO $migration$
DECLARE destination TEXT := current_schema();
BEGIN
 EXECUTE format('ALTER FUNCTION %I.brassclaw_initialize_provider_catalogue(TEXT) RENAME TO brassclaw_initialize_provider_catalogue_v140',destination);
 EXECUTE format('CREATE FUNCTION %I.brassclaw_initialize_provider_catalogue(owner TEXT) RETURNS BOOLEAN LANGUAGE plpgsql SET search_path=pg_catalog,%I,pg_temp AS %L',destination,destination,
 'DECLARE seeded BOOLEAN;
 BEGIN
  seeded := brassclaw_initialize_provider_catalogue_v140(owner);
  IF seeded THEN
   UPDATE brassclaw_llm_providers p SET definition=jsonb_set(p.definition,''{embedding_models}'',
    CASE WHEN p.definition->>''protocol'' IN (''open_ai_completions'',''open_router'',''nearai'',''ollama'',''bedrock'')
     THEN brassclaw_embedding_model_defaults() ELSE ''{}''::jsonb END)
    WHERE p.tenant_id=owner AND NOT p.definition ? ''embedding_models'';
  END IF;
  RETURN seeded;
 END');
END
$migration$;
