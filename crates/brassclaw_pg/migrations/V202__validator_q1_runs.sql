-- Immutable link from a completed validator Q1 run to its evidence record.
--
-- One row per completed single-class Q1 run: the `submission_id` points to
-- `reborn_component_review_submissions` (V104), the `evidence_id` points to
-- `reborn_component_review_evidence` (V102), and `validator_selection_checksum`
-- is the hex SHA-256 of the retained `monty-retained-recipe-selection/1` bytes.
--
-- The (submission_id, validator_selection_checksum) pair is the natural key:
-- the same subject may be reviewed by different validator versions; two runs
-- against the same selection are idempotent (the same evidence row and checksum).
-- Integrity: only the trusted Q1 evidence writer may INSERT here.
--
-- This table is the index side of the evidence write in pg_validator_q1_evidence.
-- It enables FK-based readers to find Q1 evidence for a given submission without
-- parsing JSONB in reborn_component_review_evidence.

CREATE TABLE reborn_validator_q1_runs (
    -- The completed run's unique identity. Derived deterministically from
    -- (submission_id || validator_selection_checksum) by the evidence writer.
    run_id UUID PRIMARY KEY
        CHECK (run_id <> '00000000-0000-0000-0000-000000000000'),

    -- Original immutable submission this run reviewed.
    submission_id UUID NOT NULL
        REFERENCES reborn_component_review_submissions(submission_id),

    -- The exact hex SHA-256 of the retained recipe-selection bytes used.
    -- 64 lowercase hex characters covering the complete retained manifest.
    validator_selection_checksum TEXT NOT NULL
        CHECK (validator_selection_checksum ~ '^[0-9a-f]{64}$'),

    -- Component class being reviewed (0, 1, 2, 3, 13, 21, 22, 23).
    candidate_class_code SMALLINT NOT NULL
        CHECK (candidate_class_code IN (0, 1, 2, 3, 13, 21, 22, 23)),

    -- Task handle UUID from the completed Monty execution. Not a run claim.
    task_handle UUID NOT NULL
        CHECK (task_handle <> '00000000-0000-0000-0000-000000000000'),

    -- FK into the immutable evidence store. The run cannot exist without
    -- its evidence record having been committed first.
    evidence_id UUID NOT NULL
        REFERENCES reborn_component_review_evidence(evidence_id),

    -- Whether the aggregation step's pass flag was true.
    succeeded BOOLEAN NOT NULL,

    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Natural key: each selection is used at most once per submission.
    UNIQUE (submission_id, validator_selection_checksum)
);

-- Index to look up all runs for a given submission, ordered by recording time.
CREATE INDEX reborn_validator_q1_runs_submission
    ON reborn_validator_q1_runs (submission_id, recorded_at);

-- Index to find all successful runs for a given class quickly.
CREATE INDEX reborn_validator_q1_runs_class_succeeded
    ON reborn_validator_q1_runs (candidate_class_code, succeeded)
    WHERE succeeded = TRUE;

-- Immutability: run records must not be mutated or removed after insertion.
CREATE FUNCTION protect_validator_q1_run_records() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'validator Q1 run records are immutable' USING ERRCODE='23514';
END;
$$;

CREATE TRIGGER reborn_validator_q1_runs_immutable
    BEFORE UPDATE OR DELETE ON reborn_validator_q1_runs
    FOR EACH ROW EXECUTE FUNCTION protect_validator_q1_run_records();

CREATE TRIGGER reborn_validator_q1_runs_no_truncate
    BEFORE TRUNCATE ON reborn_validator_q1_runs
    FOR EACH STATEMENT EXECUTE FUNCTION protect_validator_q1_run_records();
