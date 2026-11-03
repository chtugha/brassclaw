# Bob implementation reconciliation after validator commits

Baseline: `0f1236a6c8a0f9d5ba4d7145bd040bf8c4576149`, local `main`.
Only `.DS_Store` was uncommitted at the start of this audit. No preceding
temporary patch or older source tree was restored. The four component guides
and v3.5 roadmap are unchanged from the already reviewed Bob baseline.

## Shared source and responsibility map

| Boundary | Existing owner and current behavior | Remaining repair |
| --- | --- | --- |
| Immutable subject | skills `review_submission_store`, V104/V149/V193; exact single/joint subject and durable request | Mutable queue has no original submission binding; Q1 chooses latest by component UUID |
| Structural graph | skills `validator_components`; check/collect/aggregate/receipt PythonCode plus private Recipe | Complete class-specific Q1/behavior coverage and explicit historical format compatibility |
| Rule observation | engine `retained_validator`; actual per-step fingerprints and separate receipt-step observation | Trusted full required-check manifest and terminal-settlement proof |
| Private admission | composition `pg_validator_q1_admission`, `validator_task_driver`, `pg_review_admission_source` | Qualified coherent validator selection, explicit original work and pre-/post-terminal verdict stages |
| Q1 graduation | `q1_orchestrator`, `validation_review` | Latest run lookup remains too broad; preserve exact negative diagnostics and bind original subject/candidate/attempt/settlement |
| Shipped qualification | `bootstrap_l013_approval`, `bootstrap_validator_approval`, `bootstrap_evidence` | Placeholder inputs, unperformed positive reports, revision-insensitive IDs and successful existence-only skips remain |
| Approval writer | `pg_pure_component_approval`, `pg_pure_approval_store`, association/workflow review owners | Trusted evidence kinds/subjects/provenance/Q2 and exact linked root enforcement |
| Catalogue and retention | `authored_monty_catalogue`, `installed_monty_catalogue`, engine retained selection and review selection store | UUID-only pure coverage, preparation/recheck disagreement and successor approval pins remain |
| Native Codex | actual PG Auth Arc in `factory`, existing native transport, `llm_catalog`/`llm_reload` supplier and access/publication owners | Default-tenant migration, original refresh, full caller/readiness and issuer acceptance remain |
| Work scheduling | immutable review requests exist; `ValidationQueueStore` still starts a detached ignored-result kick with default limits | Bind original work, use existing durable admission/discovery, capture effective policy and reconcile without replay |
| Global driver | `drive_admitted` now resolves trusted-internal thread namespace from the admitted pool | Lifecycle still duplicated with `drive_turn`; explicit provenance and acknowledged proof transfer/cleanup remain |
| Product/build tooling | existing provider UI custody repairs, independent local/Modal tooling and native CLI receipts | Remaining original-review UI/class dispatch and complete product gates remain |

All generic class routes remain 0, 1, 2, 3, 13, 21, 22 and 23. Class 10 retains
its protected review owner; class 50 remains historical. Tools/ToolSkills remain
native primitives/bindings; Recipe/PythonCode owns checking sequence. No extra
queue, approval verifier, provider store or VM is proposed.

## Reused committed results

`0f1236a6c` supplies the separate receipt encoder, actual fingerprint checks,
canonical five-draft construction, corrected rule result shape and admission
pool wiring. `71d5c4f49` reads persisted `succeeded` instead of treating every
ResultOnly completion as passing. The implementation status records six actual
T1–T5b cases and twelve utility body cases. These close those narrow repairs;
they do not prove eight complete class reviews, trusted seed behavior, durable
scheduling, settled verdicts or general pure publication.

B1-01, B2-01 and B3-01 are source-reconciliation/inventory gates and are closed
by this checked current map plus the existing detailed reader inventory. Other
steps remain open where any required connected behavior/evidence is absent.
The next changes extend these named owners; commits do not end implementation.
