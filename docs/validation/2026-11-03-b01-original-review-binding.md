# B-01 original review and queue binding — source handoff

Date: 2026-11-03. Branch: `codex/b01-original-review-binding`.
Base: `a4f308dc472b6f5b63e5825ad096f626895f6040`.
Owner/scope: bounded B1-02 original submission/candidate capture; migration V206.
This is a source handoff, not a build, native-test, component-approval, activation,
or complete B1/B2/B3 acceptance record.

## Implemented source boundary

The retained Q1 driver resolves the exact submission through the current queue's
immutable original binding. It no longer selects a submission by newest timestamp
for a component UUID. Capture and final Q1/Q2 mutation compare the same queue and
component bytes. Unbound historical queues stay visible but cannot acquire an
original through a guessed match or obtain retained Q1/Q2 graduation.

V206 retains the original request's target choice and per-queue binding separately
from mutable queue lifecycle. It verifies the real immutable subject, changed-root
reference, original base, full explicit legacy projection, scope, class and actual
locked candidate bytes. Deferred constraints require all changed roots in a
queued joint review. Bindings survive Q2 queue deletion; updates, deletion and
truncate are rejected. Historical pending rows retain their bytes/state/counters
and receive a false eligibility marker; new generations default true. No original
or review evidence is backfilled.

The existing operator submission endpoint accepts either explicit `queue_ids`,
`create_queue_project` for a new single-root class-21/22/23 legacy projection, or
neither for a standalone immutable submission. Existing required subject fields
and /1, /2, /3 subject meanings remain intact. Full candidate bytes and exact
references are required; omitted projection fields are rejected. Extra v3 fields
remain in the immutable revision. Creation preallocates component/queue identity,
locks queue before component/revision heads, and commits projection, queue,
subject, target choice and bindings together. An identical original retry reads
its original result, including after Q2 removes the queue. Changing bytes,
original ID/targets or scope cannot take over an earlier operation.

The immutable completed-turn producer uses the same transaction helper with an
explicit standalone target choice and its existing host-derived original ID.
Legacy create methods keep their existing draft behavior and use the same
queue-before-component ordering. This does not convert their incomplete inputs
into v3 subjects.

## Boundaries deliberately retained

* Joint /3 subjects retain their complete changed/context partner and dependency
  selection. Current /1 validator selection cannot execute these subjects, so
  both positive and negative partial queue writes are rejected. This package
  creates neither successor approval nor validator-selection semantics (B2).
* Target-choice records contain no Work claim, lease, scheduling state or wake;
  durable Work/admission/recovery remains B3.
* The wired legacy Sempai proposal sink still receives mutable constructor data
  without the immutable packet/original response identity. Its conversion belongs
  at the upstream producer boundary; it is not represented as completed here.
  Historical Docplan conversion is likewise not certified. Ordinary Docus
  class-17 editing remains outside the eight retained validator classes.
* Classes 0, 1–3 and 13 can bind explicit existing queue candidates only when the
  caller supplies a complete original revision and exact graph. They have no new
  legacy-create projection in this package.
* Existing Recipe lifecycle test fixtures remain explicitly test-only, with an
  unreviewed original and, for upgrades, a retained actual comparison base.
  They are not executable Q1/behavior evidence or a production reconstruction API.

## Verification performed

`git diff --check` and direct `rustfmt --edition 2024` formatting/parser checks
passed for the changed Rust source. Source self-review covered all new SQL
parameters/constructor arities, feature ownership, immutable retry/readback,
queue-before-component lock order and joint-review graduation guards.

No Cargo build, test-target compilation, Clippy, PostgreSQL migration execution,
native test or remote test was run in this package chat. No cloud build was
started; there is no build receipt to reuse. The supervisor must compile and run
the exact package before acceptance. Formatting is not type checking or SQL
execution. In particular, the migration and new native fault-transport test remain
unexecuted.

## Written tests for supervisor execution

The eight tests in `pg_review_submission::queue_tests` use the real native
PostgreSQL harness and, where applicable, mounted operator HTTP:

* `original_queue_operator_create_retry_and_q2_historical_readback`
* `original_queue_a_cannot_certify_b_or_changed_cas`
* `original_queue_competing_authors_keep_one_atomic_original`
* `original_queue_lost_commit_ack_reads_the_original_without_reinsert`
* `original_queue_joint_context_is_retained_and_never_partially_graduated`
* `original_queue_recipe_and_catalogue_create_preserve_complete_documents`
* `original_queue_foreign_payload_and_historical_rows_fail_without_subject_allocation`
* `original_queue_upgrade_preserves_old_pending_bytes_without_backfill`

The existing actual retained Monty Q1 positive/negative fixtures in
`runtime/tests/validator_retained_q1_tests.rs` now capture their explicit original
queue binding before real validator execution. Re-run that module as the B00/B01
integration regression, plus affected Recipe manual Q2/rejection/upgrade and
validation-queue lifecycle tests. These are requested checks, not passing results.
Full changed-partner /3 runner acceptance remains unavailable and unclaimed.
