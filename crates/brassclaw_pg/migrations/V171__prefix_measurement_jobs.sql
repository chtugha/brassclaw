-- Offline validation is an ordinary retained Recipe job, never a new queue or
-- a model/design acceptance. Existing definitions keep their exact meaning.
ALTER TABLE brassclaw_prefix_job_definitions
    DROP CONSTRAINT brassclaw_prefix_job_definitions_operation_check;
ALTER TABLE brassclaw_prefix_job_definitions
    ADD CONSTRAINT brassclaw_prefix_job_definitions_operation_check
    CHECK (operation IN ('generate','redesign','model_design','create','measure'));
