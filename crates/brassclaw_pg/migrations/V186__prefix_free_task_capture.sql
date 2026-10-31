-- Forward-only capture metadata. Existing /1 and /2 rows are neither rewritten
-- nor reinterpreted. A declaration is not workflow qualification or permission.
ALTER TABLE brassclaw_monty_task_admissions DROP CONSTRAINT monty_task_model_selection_shape;
ALTER TABLE brassclaw_monty_task_admissions ADD CONSTRAINT monty_task_model_selection_shape CHECK (
    task_model_selection IS NULL OR (
        task_model_selection_version=1 AND started_at IS NOT NULL
        AND jsonb_typeof(task_model_selection)='object'
        AND task_model_selection ?& ARRAY['format','checksum','selection']
        AND task_model_selection->>'format' IN ('brassclaw-task-provider-binding/1','brassclaw-task-provider-binding/2','brassclaw-task-provider-binding/3')
        AND jsonb_typeof(task_model_selection->'selection')='object'
        AND task_model_selection->>'checksum' ~ '^[0-9a-f]{64}$'
        AND (task_model_selection->>'format' NOT IN ('brassclaw-task-provider-binding/2','brassclaw-task-provider-binding/3')
            OR jsonb_typeof(task_model_selection->'prefix_bindings')='array')
        AND (task_model_selection->>'format'<>'brassclaw-task-provider-binding/3' OR (
            task_model_selection ?& ARRAY['prefix_bindings','prefix_capture']
            AND task_model_selection - ARRAY['format','checksum','selection','prefix_bindings','prefix_capture']='{}'::jsonb
            AND task_model_selection->'prefix_bindings'='[]'::jsonb
            AND jsonb_typeof(task_model_selection->'prefix_capture')='object'
            AND task_model_selection->'prefix_capture'->>'format'='recipe-prefix-free-capture/1'
            AND task_model_selection->'prefix_capture'->>'namespace'=task_model_selection->'selection'->>'namespace'
        ))) IS TRUE);
