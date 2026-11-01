# Active Task Registry

Updated by the orchestrator. Shows all live sub-tasks, their status, and assigned files.

| Task ID | Package | Status | Assigned Files | Notes |
|---------|---------|--------|----------------|-------|
| (none active) | — | — | — | — |

## Completed Tasks

| Task ID | Package | Outcome | Commit |
|---------|---------|---------|--------|
| (session bootstrap) | PKG-A / BUILD-036 / BUILD-054..061 | DONE | d12dc8d2b |
| TASK-001 | PKG-B: ValidatorServiceBridge wiring | DONE (a92e8e5e8) | installed_monty_catalogue.rs, runtime.rs, validation_queue.rs, q1_review_fixture.rs |
| TASK-003 | PKG-E-R09: seed proposed-child-task-request | DONE (955007991) | builtin_bootstrap.rs |
| TASK-004 | PKG-D: pg_validator_q1_admission tests | DONE (955007991) | pg_validator_q1_admission.rs |
| TASK-005 | PKG-C: ValidationQueueStore.submit Q1 kick | DONE (74b446367) | validation_queue.rs |
| TASK-006 | PKG-C+: validator_bridge() accessor + PgPythonCodeStore/PgRecipeStore/PgSempaiProposalSink Q1 wiring | DONE (7985502eb) | installed_monty_catalogue.rs, pg_python_code_store.rs, pg_recipe_store.rs, sempai_proposal_sink.rs, runtime.rs, validation_queue.rs |
| TASK-007 | PKG-F L001–L003: preloadable function body migration | DONE (7985502eb) | builtin_bootstrap.rs |
| TASK-008 | PKG-F L004–L012: preloadable function body migration | DONE (017ccf2d6) | builtin_bootstrap.rs |
| TASK-009 | verify_receipt fingerprint fix: add aggregate-step checksum check, reorder observe_subject before persist_workflow_review | DONE (017ccf2d6) | pg_review_admission_source.rs |

## Open Items (not yet assigned)

| Item | Description | Owner plan |
|------|-------------|-----------|
| PKG-E-R09 preload fix | `proposed-child-task-request` uses legacy top-level body; needs `def prepare_child_task_request(inputs):` format with python-preload/2 declaration | monty.composition.implementation.md §R09 |
| L010 unblock | `pc-exec-file-exists` blocked on typed `not_found` contract | monty.composition.implementation.md |
| L013–L101 | Remaining PythonCode body migrations to preloadable format | monty.composition.implementation.md |
| Step 6 completion | Remove `run_q1_validation` legacy path; connect `PgCompositionPort` validator_bridge | validator_v3.md Step 6 |
| Validator Steps 7–12 | Eight class routes, behavioral evidence, human Q2, production cutover | validator_v3.md |
