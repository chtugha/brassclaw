# K-01 review correction: Composition acquisition adapter

Base: `8dc775a003c42de4d4c18b696f0b39314100c1ca`. The own K branch was rebased
onto that approved Main head; Git recognized both earlier K commits as already
applied. No Main edit, merge, push or replacement checkout was made. This record
is source evidence, not compilation, native acceptance or installed uptake.

## Defect and existing owner boundary

K-01 moved TransactionClient into brassclaw_pg but missed Composition's inherent
impl on its EvidenceClient alias. Re-exporting a foreign type does not make it
local; that inherent impl is invalid Rust (E0116). The correction replaces it
with `bootstrap_evidence::io::acquire(&PgPool) -> Result<EvidenceClient,
RebornBuildError>`, a free Composition function. The function retains the exact
existing pool acquisition, from_client constructor and invalid error mapping.

The canonical PG struct, from_client, Drop, Deref and recycle_after_commit remain
unchanged, as does Composition's pg_transaction_client re-export. There is no new
wrapper, pool, PG constructor API, extension trait, dependency or suppression.
Composition continues to own RebornBuildError; PG has no Composition dependency.
No Recipe, primitive, component, migration or product workflow change is needed.

## Complete connected source scope

All source paths below are relative to crates/brassclaw_reborn_composition/src.

| File | Acquire-call replacements |
| --- | ---: |
| bootstrap_evidence_io.rs | 3; also replace foreign impl with free adapter |
| bootstrap_inventory_completion.rs | 2 |
| bootstrap_inventory_qualification.rs | 1 |
| bootstrap_pure_checker.rs | 1 |
| bootstrap_pure_workflow.rs | 4 |
| bootstrap_validation.rs | 1 |
| bootstrap_validator_approval.rs | 5 |
| installation_inventory_observation.rs | 5 |
| pg_private_delivery.rs | 1 |
| prefix_job_submission.rs | 1 |
| private_task_catalogue.rs | 2 |
| q1_orchestrator.rs | 1 |
| review_work_confirmation.rs | 2 |
| review_work_reconciliation.rs | 1 |
| runtime/tests/private_delivery_tests.rs | 1 |

bootstrap_inventory_primitives.rs additionally re-exports acquire in place of its
now-unused EvidenceClient type re-export. Total: 16 Rust files, 31 calls. The two
valid EvidenceClient::from_client consumers in installed_monty_catalogue.rs and
pg_pure_component_approval.rs remain unchanged. No EvidenceClient::acquire,
TransactionClient::acquire or inherent impl EvidenceClient remains in crate Rust
source.

The earlier J-copy proposal was not applied wholesale. Current preimages matched
it except for the existing Manual-memory setup in the evidence deadline test;
that setup is preserved. Actual private_task_catalogue and Delivery hunks change
only the acquisition function path. F/I/J custody and bounded scopes, transaction
isolation, awaits, error conversions, commit checks and exact readback are
preserved. L is independently based on this same Main head; any later textual
integration overlap in those files still belongs to Root review.

## Custody and verification limits

Acquisition uses the same caller pool and immediately constructs the existing
PG custody type with recyclable=false. Lost/cancelled BEGIN or COMMIT keeps the
existing detach-on-drop behavior. Only the caller's existing successful COMMIT
ACK followed by recycle_after_commit permits normal reuse. This change adds no
retry, task replay, transaction or deadline and does not infer acknowledgement.

Performed: full 16-file source diff review, mechanical caller inventory, canonical
owner diff equality, rustfmt on the 16 affected Rust files, git diff --check, and
changed-file SHA256 verification. No local Cargo, compilation, test, DB, SQL,
remote test or Modal job was run. No delegating-only unit tests were added; the
trivial change does not justify paid compilation. The adjacent manifest covers
all 16 changed source files plus this record; it excludes itself.

## Supervisor acceptance still required

Compile the combined source with the existing production/native feature matrix,
including postgres+skills-db without test-support and the actual postgres-only
production configuration. Then reuse that compilation for the existing genuine
common-boot/bootstrap qualification and relevant native selectors:

- bootstrap_evidence::io::tests::native_held_intent_commit_reports_actual_deadline_context_and_keeps_original
- runtime::tests::private_work_admission_tests::private_prefix_producer_held_begin_cancel_discards_unconfirmed_client
- runtime::tests::private_work_admission_tests::private_prefix_producer_held_commit_cancel_reads_exact_original_without_replay
- runtime::tests::private_work_admission_tests::private_prefix_producer_held_ack_release_reuses_confirmed_connection
- runtime::tests::private_delivery_tests::private_delivery_lost_commit_ack_does_not_reexecute_source_compiler
- runtime::tests::private_delivery_tests::review_confirmation_commit_ack_loss_reuses_original_delivery_with_one_connection

Root's actual commonBoot command/selector must be taken from its selected batch,
not invented here. These cases cover genuine held acknowledgement, cancellation,
pool1 replacement/reuse, immutable original readback, Delivery and Q1/Review
callers. Prior receipts do not establish acceptance of the changed source. The
existing K-01 HTTP pool1 held-BEGIN/COMMIT case can additionally exercise the
unchanged canonical PG owner, but alone cannot validate all migrated callers.
No full B2, approval, recovery, per-revision/evidence-cap or historical native
loader completion is claimed.
