-- Repair only the known legacy system Recipe shape. Preserve component IDs,
-- PythonCode bodies, operator overrides and already authored IBS metadata.
-- Fresh databases receive the canonical shape from the seeder instead.
UPDATE reborn_recipes AS r
SET step_descriptions = jsonb_build_array(jsonb_build_object(
        'desc_idx', 0,
        'label', 'Non-Matching-Mode prompt and model handoff',
        'yaml_source', '',
        'steps', jsonb_build_array(
            jsonb_build_object(
                'stepnumber', 1, 'knowledge', 'orchestrator', 'type', 'component',
                'goal', r.step_descriptions->0->>'desc', 'content', '',
                'include', jsonb_build_array(assembler.id)),
            jsonb_build_object(
                'stepnumber', 2, 'knowledge', 'rust', 'type', 'component',
                'goal', 'Bind the Kohai tool', 'content', '',
                'include', jsonb_build_array(binding.id),
                'tool_bindings', jsonb_build_array(jsonb_build_object(
                    'tool_id', tool.id, 'tool_name', tool.name, 'params', '{}'::jsonb,
                    'error_policy', jsonb_build_object('policy', 'fail')))),
            jsonb_build_object(
                'stepnumber', 3, 'knowledge', 'orchestrator', 'type', 'component',
                'goal', r.step_descriptions->1->>'desc', 'content', '',
                'include', jsonb_build_array(caller.id))))),
    variants = jsonb_build_array(jsonb_build_object(
        'variant_key', 'default',
        'description', 'Internal Tier-2 path for a genuine intent No-Match',
        'step_link', '0:1-0:E', 'intent_examples', '[]'::jsonb,
        'variable_patterns', '[]'::jsonb)),
    updated_at = now()
FROM reborn_python_code AS assembler,
     reborn_python_code AS caller,
     reborn_tool_skills AS binding,
     reborn_tools AS tool
WHERE r.name = 'host-non-match-llm-answer'
  AND r.source = 'system' AND NOT r.override_prompt_creation
  AND r.validation_status = 'validated'
  AND r.user_id = chr(31) || 'SYSTEM' || chr(31)
  AND r.agent_id = 'default' AND r.project_id = 'system'
  AND r.variants IS NULL
  AND jsonb_typeof(r.step_descriptions) = 'array'
  AND jsonb_array_length(CASE WHEN jsonb_typeof(r.step_descriptions) = 'array'
                             THEN r.step_descriptions ELSE '[]'::jsonb END) = 2
  AND r.step_descriptions->0->>'step' = '0'
  AND r.step_descriptions->0->>'action' = 'assemble_prompt'
  AND jsonb_typeof(r.step_descriptions->0->'desc') = 'string'
  AND r.step_descriptions->1->>'step' = '1'
  AND r.step_descriptions->1->>'action' = 'kohai_complete'
  AND jsonb_typeof(r.step_descriptions->1->'desc') = 'string'
  AND (assembler.tenant_id, assembler.user_id, assembler.agent_id, assembler.project_id)
      = (r.tenant_id, r.user_id, r.agent_id, r.project_id)
  AND assembler.name = 'pc-host-assemble-non-match-prompt'
  AND assembler.source = 'system' AND assembler.validation_status = 'validated'
  AND (caller.tenant_id, caller.user_id, caller.agent_id, caller.project_id)
      = (r.tenant_id, r.user_id, r.agent_id, r.project_id)
  AND caller.name = 'pc-host-kohai-complete'
  AND caller.source = 'system' AND caller.validation_status = 'validated'
  AND (binding.tenant_id, binding.user_id, binding.agent_id, binding.project_id)
      = (r.tenant_id, r.user_id, r.agent_id, r.project_id)
  AND binding.name = 'ts-host-kohai-complete'
  AND binding.source = 'system' AND binding.validation_status = 'validated'
  AND (tool.tenant_id, tool.user_id, tool.agent_id, tool.project_id)
      = (r.tenant_id, r.user_id, r.agent_id, r.project_id)
  AND tool.name = 'host.kohai_complete'
  AND tool.source = 'system' AND tool.validation_status = 'validated';
