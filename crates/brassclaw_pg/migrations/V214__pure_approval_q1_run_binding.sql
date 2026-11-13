-- B1-06: Bind authored pure-component-approval q1_ref to an actual validator run record.
--
-- V202 reborn_validator_q1_runs stores one row per completed single-class Q1
-- run linking a submission_id/validator_selection_checksum pair to the evidence
-- row produced by the trusted writer (persist_validator_q1_evidence). This
-- migration ties authored pure-component approvals' q1_ref to that run table
-- so that every new authored approval's Q1 evidence is verifiably associated
-- with a real, succeeded validator execution.
--
-- system_seed approvals use bootstrap evidence rows inserted directly into
-- reborn_component_review_evidence without a reborn_validator_q1_runs row.
-- The trigger permits these by checking validation_mode. The separate integrity
-- checks for system_seed rows remain in the bootstrap qualification owner.
--
-- Pre-existing authored rows are not retroactively validated; B2-09 owns the
-- historical audit and repair of records written before this migration.
-- New rows are fully checked by the trigger.

-- For authored approvals only, the q1_ref must reference a succeeded run record.
-- A plain FK on reborn_pure_component_approvals.q1_ref cannot distinguish
-- system_seed from authored rows, so a trigger provides the selective check.
CREATE FUNCTION verify_pure_approval_q1_run() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    -- system_seed rows use bootstrap evidence rows not in reborn_validator_q1_runs;
    -- skip the run-existence and success check for them.
    IF NEW.validation_mode = 'system_seed' THEN
        RETURN NEW;
    END IF;
    -- For authored approvals, q1_ref must exist in reborn_validator_q1_runs
    -- with succeeded=true. This prevents using fabricated or failed evidence
    -- as Q1 proof for an authored component approval.
    IF NOT EXISTS (
        SELECT 1 FROM reborn_validator_q1_runs
        WHERE evidence_id = NEW.q1_ref
          AND succeeded = TRUE
    ) THEN
        RAISE EXCEPTION
            'authored pure-component approval q1_ref must reference a succeeded validator run'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER pure_approval_q1_run_check
    BEFORE INSERT ON reborn_pure_component_approvals
    FOR EACH ROW EXECUTE FUNCTION verify_pure_approval_q1_run();

-- Record the existing authored approval count for reference in diagnosis.
-- This comment is intentional: existing authored rows are not invalidated here,
-- but their q1_ref binding to reborn_validator_q1_runs must be audited under B2-09.
-- The trigger only applies to future INSERT operations.
