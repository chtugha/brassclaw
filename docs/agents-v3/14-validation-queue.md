# 14 — Component validation, evidence and review queue

## 1. Purpose and authority

Validation establishes whether an exact component/workflow subject satisfies its
required structural, semantic and observed behavioral contract. Human Q2 is
separate from automated observations. Approval, publication, task selection and
live Tool permission remain separate boundaries. [skills.md](../../skills.md),
[recipe.md](../../recipe.md), [validator_v3.md](../../validator_v3.md) and
[Simplified v3](../../simplified_v3.md) govern the requirements. This source map
was checked on 2026-11-15; it is not a new approval or runtime acceptance receipt.

## 2. Location — verified writers, readers and callers

| Source | Actual responsibility and limit |
| --- | --- |
| [validation_queue.rs](../../crates/brassclaw_reborn_composition/src/validation_queue.rs) | `ValidationQueueStore`, queue state constants, submit/reject/approve, original-review checks and compatibility graduation. Queue state is not retained component approval. |
| [validation_review.rs](../../crates/brassclaw_reborn_composition/src/validation_review.rs) | Captures locked legacy `Q1Candidate` bytes. `gate1_pass_reviewed` and its test helper are test-gated; they are not a production full-Q1 writer. |
| [q1_orchestrator.rs](../../crates/brassclaw_reborn_composition/src/q1_orchestrator.rs) | `Q1Outcome` distinguishes `StructuralObserved`, `Passed`, `Failed`, `Deferred`. The historical `run_q1_validation_retained` preparation runner is test + skills-db gated. |
| [validator_task_driver.rs](../../crates/brassclaw_reborn_composition/src/validator_task_driver.rs) | `ValidatorTaskDriver::notify_originals` wakes the existing global driver's private-original discovery; its controlled `submit` helper is test-gated. |
| [pg_retained_q1.rs](../../crates/brassclaw_reborn_composition/src/pg_retained_q1.rs) | `StructuralReviewSet` and `BehavioralReviewSet` derive evidence from exact inspected graphs and actual executor observations for their supported adapter. They do not prove general class qualification. |
| [pg_workflow_review.rs](../../crates/brassclaw_reborn_composition/src/pg_workflow_review.rs) | `WorkflowReview` retains exact selected-variant structural/step observations; those are not semantic approval, root completion or catalogue activation. |
| [pg_association_review.rs](../../crates/brassclaw_reborn_composition/src/pg_association_review.rs) | `prepare` / `approve` connect authenticated human exact-usage review through the existing facade. Model/Recipe ports cannot write human Q2. |
| [runtime.rs](../../crates/brassclaw_reborn_composition/src/runtime.rs) | Ordinary startup binds `ValidatorTaskDriver` to the installed catalogue's validator service outside tests, using the existing global Monty driver. Notification attachment alone does not qualify a validator selection. |

There is no current engine `memory/component_validator.rs` at the path cited by
older versions of this guide. Do not describe `ComponentValidator::validate_by_class`,
`validate_python_code_body` or `validate_python_code_placeholders` as existing
production APIs. Structural scanning/parsing is not full Q1 or semantic approval.

## 3. Queue and immutable evidence are different models

The [V051 queue](../../crates/brassclaw_pg/migrations/V051__reborn_validation_queue.sql)
has component identity/class, existing scope keys, state, rejection counter,
feedback/errors and timestamps. Current source defines:

| Constant | Value | Meaning |
| --- | --- | --- |
| `STATE_Q1_PENDING` | 1 | Awaiting qualification. |
| `STATE_Q1_PASSED` | 2 | Passed required Q1, awaiting human review. |
| `STATE_REJECTED` | 3 | Rejected; revision/resubmission is required. |
| `STATE_DELETION_CANDIDATE` | 4 | Rejection threshold/manual disposition candidate. |

The rejection counter is separate from Tool dispatch attempt counts. Existing
scope columns are current storage/correlation, not extra final-v3 operator roles.
A queue row, `validation_status='validated'`, nonempty body or successful SQL write
cannot substitute for exact trusted functional/workflow evidence.

Do not claim queue state and component lifecycle metadata are physically disjoint
at every intermediate SQL statement. Submission/upgrade and graduation are
transactional transitions. In particular, the legacy queue carries proposed
payloads and graduation may update a mutable projection; this is not the immutable
v3 successor protocol and must not overwrite approved retained revision bytes.

[V077](../../crates/brassclaw_pg/migrations/V077__reborn_validation_queue_populate.sql)
already contains queue population, Recipe legacy-column drops, the
`last_graduation_at` column and queue-delete trigger. They are not missing future
Phase N migrations. [V095](../../crates/brassclaw_pg/migrations/V095__monty_graduation_cursor_revision.sql)
adjusts settings revision handling for graduation. Their presence is source/history
evidence, not proof of the database state or cache behavior of an installed
instance. Cache invalidation does not replace coherent catalogue publication.

## 4. Required qualification and current execution boundaries

The reviewed subject and selected validator are different identities. Retain the
original submission, exact candidate/base/dependency context, interface/source
references and required rule set, plus the actual selected validator, interpreter,
run/attempt and native settlement provenance. A task manifest records selection;
it does not approve the subject being reviewed.

Structural observations establish the checks actually observed. Observed behavior
must exercise the exact selected exports and meaningful valid/invalid/boundary
cases with original inputs/results and artifact identity. Missing, skipped,
interrupted, swapped or partial observations cannot become complete Q1. Parsed
schemas, fixed body substrings, same output on unrelated fixtures or a model's
`passed=true` do not prove semantics or complete rule coverage.

Recipes own rule selection/sequencing and semantic policy; native code supplies
mechanical preparation, authenticated observations, persistence, authority,
accounting and effect custody. Required checks must run exactly within their
qualified scope. Do not infer eight production class routes from a common helper,
wrapper Recipe or retained structural report.

Production queue notifications wake original discovery. Controlled tests may
exercise the test-gated Q1 preparation path. Neither notification nor seeding a
validator Recipe turns that helper into a production full-Q1 writer. The current
source explicitly preserves `StructuralObserved` without promoting it to a passed
queue state; complete class qualification and connected writers/readers remain
with their existing owners.

`ValidationQueueStore::approve` requires human Q2, locks/checks state and retained
review bytes, and rejects unresolved original/legacy custody. Its compatibility
projection transaction is not general own-functional publication. The exact
association route separately prepares the human view and records the same exact
reviewed selection through authenticated operator access. An uncertain commit
requires original-ID/byte reconciliation, not another candidate or repeated effect.

## 5. Approval paths and classes

The [own-functional successor contract](../../skills.md#own-functional-approval-and-successor-compatibility)
is binding but not a claim that all current writers/readers implement it:

- **A, installed authored changes:** Skill and implementing PythonCode form one
  functional review subject with separate immutable IDs. Changed roots receive
  coordinated own Q1/behavior/human-Q2; an unchanged approved partner/dependency
  is context. Compatible successors preserve required consumer contracts without
  reapproving unchanged consumers or adding another A combination review.
- **S, controlled shipped qualification:** independently qualify Tool/native
  implementation, then ToolSkill, then Skill/code and consuming workflows.
  Retain exact usage/integrity/Q1/behavior evidence outside the normal authored
  queue, with null human Q2 and no additional installed/external human action.
  A source/status label or closure list cannot supply that provenance or grant
  all prerequisite approvals.
- **Current exact authored profiles:** unchanged `skill-association/1` technical
  records and `skill-association-approval/1` evidence keep their original meaning
  until the connected successor cutover. Do not rewrite historical bytes or
  relax existing readers in anticipation of new support.

The eight component routes tracked by the validator/v3.5 handoff are classes
**0, 1, 2, 3, 13, 21, 22, 23**. The class-10 orchestrator has its controlled root
qualification/lifecycle. Class 50 is historical Scaffold storage, **not a new
active type, validator or protected root**. Inventory future-use flows and use
normal class-21 Recipes while preserving historical UUIDs/pins. Canonical class
labels in [class_label](../../crates/brassclaw_engine/src/memory/intent_system.rs)
include **15 = summary, 17 = docu, 20 = note**; do not swap Notes and Summary.

Technical source/value/observation bounds remain independent of token budgets.
When token budgets are disabled, old fixed 2k/5k/10k prose-budget prescriptions
must not introduce hidden runtime retrieval/history/task caps. Actual recursive
schemas, model context limits and native resource limits still apply.

## 6. Support, open work and release-reference integrity

Use [validator_v3.md](../../validator_v3.md),
[the exact pure-review cutover contract](../reborn/pure-component-review.md#exact-remaining-cutover-contract)
and [the implementation index](../implementation-status.md) for scoped accepted
support and remaining subject/qualification/behavior/human-Q2/activation work.
V35-03/04 consume the existing independent-pure owner. V35-17–19 consume actual
class-route and review work; they do not create another approval store or replace
it with a new pure-Rust body scanner. Business review conversion and native
reconciliation retain their separate v3.5 ownership.

Wilson scoring in the legacy Recipe store measures outcomes; it is not functional
approval, a current Tool grant or a replacement for retained workflow classification.
Old queue-column/trigger and missing-Wilson-guard status lists are not an adequate
current acceptance inventory. No Markdown update activates components.

This guide is embedded by
[prefix_release_documents.rs](../../crates/brassclaw_reborn_composition/src/prefix_release_documents.rs)
and incorporated into immutable class-17 reference drafts through
[prefix_reference_seeds.rs](../../crates/brassclaw_reborn_composition/src/prefix_reference_seeds.rs).
Its entry in the [release reference inventory](../../scripts/prefix/release-reference-inventory.json)
is `development-only`, not qualified model-reference content; this edit does not
change that disposition. Changed text changes its exact reference/package bytes. Preserve historical
revisions and receipts; qualify a successor through the existing release/bootstrap
owner before claiming installed uptake. A documentation review does not relabel
old exact-byte package evidence or regenerate an active prefix.

## 7. LLM-relevant summary

Review exact subjects through trusted structural and observed behavioral evidence,
then applicable human Q2 or controlled shipped qualification. A structural report,
queue state, label or task manifest is not complete approval. Keep current exact
profiles separate from the own-functional A successor; S qualifies prerequisites
noncircularly. Publication and live Tool policy are independent. The ordinary
validator infrastructure reuses global Monty; incomplete coverage stays incomplete,
and no failure/uncertain commit authorizes replay or invented success.
