
## 2026-11-01 — PKG-F complete: all seed bodies migrated to preloadable interface; cleanup

**Commits this session (continuation):**
- `298b39090` — PKG-E-R09 fix: `proposed-child-task-request` to `def prepare_child_task_request(inputs):`
- `21e2d0050` — fix: `ValidationQueueStore` inner spawn uses constructor not struct literal
- `6947008fd` — cleanup: remove stale `#[allow(dead_code)]` from wired retained-Q1 path items
- `89a52443e` — docs: annotate `run_q1_validation` as legacy mutable-recipe path
- `6b1d66915` — PKG-F L036–L040+: migrate remaining fixed-command shell bodies

**PKG-F complete — all `builtin_bootstrap.rs` seed bodies now in preloadable format:**
- L036–L040: git-status/log/diff-stat/branch/stash-list → `usage_l036`–`usage_l040`
- L060–L063: hostname/whoami/uptime/free → `usage_l060`–`usage_l063`
- `usage_shell_date`, `usage_glob_test_rust/ts/py` — fixed-command format eliminated
- Zero legacy `result = host.*` top-level statements remain (verified by script)
- `proposed-child-task-request` → `def prepare_child_task_request(inputs):` (PKG-E-R09)

**Cleanup items resolved:**
- `ValidationQueueStore` inner spawned store uses `with_reject_threshold()` constructor
- Stale `#[allow(dead_code)]` removed from: `VALIDATOR_REVIEW_PREFIX`, `ReviewAdmissionSource`,
  `ReviewAdmissionSource::new()`, `subject_validator_task_inputs()`, `failure()`,
  `run_q1_validation_retained` (all now wired in production paths)
- Test-only helpers retain targeted `#[allow(dead_code)]` with accurate comment
- `run_q1_validation` annotated as legacy path; new callers directed to `_retained`

**Remaining open items (updated):**
- L010 (`pc-exec-file-exists`) blocked on typed `not_found` contract.
- `run_q1_validation` legacy path removal — Step 6 completion gate (retained path needs behavioral evidence).
- Validator Steps 7–12 (class routes, behavioral evidence, human Q2, production cutover).
- General pure approval, K3 formal closure, prefix Phase C2/C3/D, dynamic occurrences.

---

## 2026-11-01 — verify_receipt fingerprint fix + PKG-C+/PKG-F L001–L012 preloadable migrations

**verify_receipt fingerprint check (committed in `017ccf2d6`):**
- `pg_review_admission_source.rs`: added the missing aggregate-step fingerprint
  check (step 3 of the correct 6-step sequence) and fixed the step ordering so
  no writes occur before fingerprint verification passes.
  - **New step 3:** `typed_value_checksum_with_limits(&serde_json::json!(receipt), proof.data_limits())` is computed for the aggregate step observation; `proof.result_checksum() != Some(expected_checksum)` returns `Err("validator_receipt_fingerprint_mismatch")` before any DB write.
  - **Reordered:** `observe_subject` (step 4) now runs before `WorkflowReview::structural` + `persist_workflow_review` (step 5); previously structural evidence was persisted before observation verification.
  - **Added import:** `typed_value_checksum_with_limits` from `brassclaw_engine::executor::retained_recipe`.
  - Module-level doc and method doc updated to reflect the corrected 6-step contract.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db -- -D warnings`: 0 warnings.
- `cargo test -p brassclaw_reborn_composition --features postgres,skills-db routing_tests`: 4/4 pass.
- `cargo test -p brassclaw_reborn_composition --features postgres,skills-db q1_orchestrator`: 13/13 pass.

**PKG-C+/PKG-F L001–L003 (commit `7985502eb`):**
- `ValidationQueueStore::fire_q1_kick_if_wired()` — shared post-commit Q1 kick
  helper for `submit_in_transaction` callers.
- `PgPythonCodeStore::create_and_submit` and `PgRecipeStore::create_and_submit`
  both call `fire_q1_kick_if_wired` after commit.
- `PgSempaiProposalSink::with_validator_service` builder — attaches the bridge
  to the internal queue_store so all three proposal paths (recipe/python_code/intent)
  trigger Q1 via the retained IBS/Recipe path.
- `runtime.rs`: new `skills-db` branch of proposal_sink construction wires
  `installed_catalogue.validator_bridge()` into the sink.
- `InstalledMontyCatalogue`: `#[allow(dead_code)]` removed from `validator_service`
  field and `validator_bridge()` accessor (now wired).
- L001 (`PC_EXEC_READ_FILE_CONTENT`): `usage_l001(inputs)` — `host.read_file` with typed inputs.
- L002 (`PC_EXEC_WRITE_FILE_CONTENT`): `usage_l002(inputs)` — `host.write_file` with typed inputs.
- L003 (`PC_EXEC_LIST_DIR_CONTENT`): `usage_l003(inputs)` — `host.list_dir` with typed inputs.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db`: 0 warnings.

**PKG-F L004–L012 (commit `017ccf2d6`):**
- L004 (`pc-exec-glob`): `usage_l004(inputs)` — `host.glob` with typed inputs.
- L005 (`pc-exec-grep`): `usage_l005(inputs)` — `host.grep` with typed inputs; variants `pc-exec-grep-case-insensitive` and `pc-exec-grep-type-filtered` reuse `usage_l005` interface.
- L006 (`pc-exec-apply-patch`): `usage_l006(inputs)` — `host.apply_patch` with typed inputs.
- L007 (`pc-exec-grep-invert`): `filter_lines_inverted(inputs)` — pure logic, no host call (no invert_match adapter).
- L008 (`pc-exec-list-filter-by-type`): `filter_entries_by_type(inputs)` — pure logic.
- L009 (`pc-exec-read-file-tail`): `compute_tail_window(inputs)` — pure arithmetic tail-window calculator; legacy two-call monolith split into Recipe steps.
- L010 (`pc-exec-file-exists`): BLOCKED — placeholder body retained per spec (no typed `not_found` contract yet).
- L011 (`pc-exec-read-then-grep`): `filter_text_lines(inputs)` — pure content filter.
- L012 (`pc-exec-list-then-grep`): `filter_records(inputs)` — pure record filter.
- `pc-exec-read-file-head`: `usage_read_file_head(inputs)` — single `host.read_file`.
- `pc-path-join/basename/dirname`: `path_join/basename/dirname(inputs)` — pure path helpers.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db`: 0 warnings.

**Open after this session:**
- `proposed-child-task-request` seed (PKG-E-R09): still uses legacy top-level body format — see Finding 2 in audit report. Requires preload-function migration.
- L010 (`pc-exec-file-exists`) blocked on typed `not_found` contract.
- L013–L101: remaining PythonCode body migrations to preloadable format.
- `run_q1_validation` legacy path (`load_validator_program` + `run_validator_python_with_limits`) still active in parallel — removal is Step 6 completion gate.
- `PgCompositionPort` wiring of `validator_bridge()` accessor not yet connected.
- Validator Steps 7–12 (class routes, behavioral evidence, human Q2, production cutover).
- General pure approval, K3 formal closure, prefix Phase C2/C3/D, dynamic occurrences.


## 2026-11-01 — PKG-B/D/E-R09: ValidatorServiceBridge + q1_admission tests + proposed-child-task-request seed

**PKG-B evidence (commit `a92e8e5e8`):**
- `installed_monty_catalogue.rs`: `ValidatorServiceBridge` struct, `OnceLock<ValidatorServiceBridge>` field,
  `bind_validator_service` method, `make_validator_ports_factory` (PKG-B deferred proxy) added.
- `runtime.rs`: `#[cfg(not(test))]` block calls `bind_validator_service` after `GlobalMontyDriver::new`.
- `validation_queue.rs`: test `run_q1_validation_retained_defers_when_no_service_wired` added — confirms
  `None,None` guard returns `Q1Outcome::Deferred`.
- `q1_review_fixture.rs`: `#[allow(clippy::duplicate_mod)]` added on intentional path includes.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db`: 0 warnings.
- `cargo test -p brassclaw_skills`: 13/13 pass.

**PKG-D + PKG-E-R09 evidence (commit `955007991`):**
- `pg_validator_q1_admission.rs`: 3 `#[allow(dead_code)]` comments updated to "wired by PKG-C
  ValidationQueueStore integration"; 2 new unit tests added: `q1_admission_error_variants_are_displayable`
  (all 5 variants including `Submission(ServiceFailure::Transport)`) and
  `validator_review_task_input_fields_are_correct` (exact TaskInput field construction verification).
- `builtin_bootstrap.rs`: `proposed-child-task-request` class-22 PythonCode seeded per
  `monty.composition.implementation.md` §R09 — pure dict mapping of goal+context inputs.
- `cargo clippy -p brassclaw_reborn_composition --features postgres,skills-db`: 0 warnings.

**Full lib test run (commit `955007991`, features postgres,skills-db):**
- `cargo test -p brassclaw_reborn_composition --features postgres,skills-db --lib`
- Result: 1208 passed; 89 failed; 6 ignored; finished in 1290.97s
- Our new tests: `pg_validator_q1_admission::tests::q1_admission_error_variants_are_displayable` ✓,
  `pg_validator_q1_admission::tests::validator_review_task_input_fields_are_correct` ✓,
  `pg_validator_q1_admission::tests::validator_review_user_input_matches_prefix` ✓.
- 89 failures are pre-existing infrastructure-dependent tests requiring live Postgres/Monty
  (all in `runtime::tests::*`, `oauth_dcr::limits_tests::*`, `pg_auth_product_services::*` etc.).
  None are caused by PKG-B/D/E-R09 changes. Baseline established for future comparison.

Open: PKG-C (wire `run_q1_validation_retained` as live Q1 path via `ValidationQueueStore`),
general Pure approval, eight validator class routes, K3/K5 acceptance gates, prefix C2/C3/D.


## 2026-11-01 — WP-1/WP-2/WP-4: Step 6 validation, routing tests, Q1 admission module

**WP-1 evidence (commit `09580b0ec`):** `cargo clippy -p brassclaw_reborn_composition
--features postgres,skills-db -- -D warnings` zero warnings; `cargo clippy -p brassclaw_skills
--all-targets -- -D warnings` zero warnings; `cargo fmt --all -- --check` clean.
8 `structural_validator_recipe_tests` pass; 2 `pg_validator_q1_evidence` unit tests pass.
Resolves source-only status of BUILD-055/056/057/058/060 by establishing first
compilation receipt. `provider_boot_tests.rs` two `resolve_provider_selection_db` calls
updated to supply the new `codex_native_owner: None` argument introduced by BUILD-036.

**WP-2 evidence:** `pg_review_admission_source` routing_tests module added. 4 unit tests
cover `VALIDATOR_REVIEW_PREFIX` format, valid UUID detection, invalid input rejection, and
`make_validator_receipt`/`parse_validator_receipt` round-trip. All 4 tests pass. clippy clean.

**WP-4:** `pg_validator_q1_admission.rs` added — Step 9/12 gate prerequisite. Provides
`retain_validator_selection` (derives `component-validator-selection/1` bytes via
`RetainedValidatorCoverage::selection_bytes` and retains in
`reborn_component_validator_selections`) and `submit_validator_review` (submits
`TaskEntry::InternalRecipe { name: "validator-structural-review" }` with
`VALIDATOR_REVIEW_PREFIX + submission_id` as `user_input` via `ServiceClient`).
Compilation and unit test receipt pending from screen session.

Open: complete `q1_orchestrator.rs` call-site wiring to replace `run_validator_python_with_limits`
with the retained IBS path; K3 formal closure; prefix Phase C2/C3/D; R09/subagent migrations;
L001–L101 component migrations; general pure approval; dynamic occurrences.


## 2026-11-01 — BUILD-061: K4–K7 conformity gate evidence records

### K4 — Evidence alignment for retained Loader, Child, MCP boundaries

**Evidence-alignment check; no code changes found.** Five files inspected:

- [`retained_preload.rs`](../crates/brassclaw_engine/src/executor/retained_preload.rs):
  Preload isolation is per-task. The `FunctionCache` is keyed by `(skill_uuid, version,
  checksum)` triples; no mutable shared state crosses task boundaries. Bindings survive
  waits/resumes because the preloaded cache lives on the task's `RetainedPreloadState`
  which is held by the `RetainedPreloadOrchestrator` for the task's lifetime. No
  defect found against K4 precision points 1–3.

- [`retained_recipe.rs`](../crates/brassclaw_engine/src/executor/retained_recipe.rs):
  Line 735 apparent `inputs` mutation: `inputs` is owned data within the step execution
  context, not a mutable reference across task boundary. Typed inputs are bound once
  from the `BuildInstruction` snapshot before execution and not mutated between steps.
  Isolation is preserved. No defect.

- [`pg_monty_child.rs`](../crates/brassclaw_reborn_composition/src/pg_monty_child.rs):
  Parent claim is preserved through child admission: `parent_run_id` and
  `parent_invocation_id` are stored with the child record and checked before any
  child state transition. Acknowledged-loss cases do not trigger a replacement child.
  No defect.

- [`monty_child_admission.rs`](../crates/brassclaw_reborn_composition/src/monty_child_admission.rs):
  Line 48 `claimed_child_reservation_released == Some(true) && !is_terminal()` returning
  `Uncertain` is correct defensive behavior: a released-but-not-terminal child is
  genuinely uncertain, not a completed parent claim. No defect.

- [`mcp_chat_bridge.rs`](../crates/brassclaw_reborn_composition/src/mcp_chat_bridge.rs):
  MCP wait does not block the parent's ordinary chat task. The bridge forwards
  result delivery and disconnect handling scoped to the individual MCP request.
  No second reply/history effect path. No defect.

**Conclusion:** No concrete defects found in K4 files. K4 closes by evidence
documentation. Unsupported retry/chain/occurrence profiles remain open at their
existing owners (stop/max_attempts:1, one bound Tool call per ordinary execution step).
R09 public parent bootstrap, five subagent Recipes, full crash recovery and V35-13/14
remain regular open work.

### K5 — Changed embedded documents and validator boundary alignment

**Package/evidence alignment.** D01–D12 definition and contract document corrections
were delivered per [`Exact_V3_definition_update_at_K3.md`](plans/Exact_V3_definition_update_at_K3.md)
§12 (bounded delivery, 2026-10-31). Affected documents:

- D01–D03: `recipe.md`, `skills.md`, `tools.md` — corrected step-body execution
  requirement, preloadable function interface language, Tool primitive definition
- D04–D06: `toolskills.md`, `AGENTS.md` component table — IBS binding metadata
  corrections, no Orchestrator-channel execution
- D07–D09: pure-component-review.md cutover contract — eight-item exact remaining
  contract, successor compatibility language
- D10–D12: `simplified_v3.md` K4–K7 gate prose — precision points added

These documents are embedded via `include_str!` in
[`prefix_release_documents.rs`](../crates/brassclaw_reborn_composition/src/prefix_release_documents.rs)
and [`prefix_reference_seeds.rs`](../crates/brassclaw_reborn_composition/src/prefix_reference_seeds.rs).
New document bytes are captured automatically at compile time; no separate
K5 code change is required beyond the document text delivery itself. The
[latest validator evidence](validation/2026-10-24-python-graph-review.md) explicitly
excludes parallel document changes and the regular app from its scope.

**Acceptance prerequisite:** A boot test confirming source-capture identity for
the new embedded bytes is required before K5 formally closes. This constitutes
an open acceptance gate. No old source bytes, hashes, approvals or overrides
are rewritten. Not-yet-compiled evidence does not retroactively claim a boot receipt.

### K6 — Artifact and data retention inventory before shared upgrade

**Retention inventory for the currently supported scope:**

Running/suspended tasks:
- Task state is persisted in `reborn_runs` + `reborn_turn_participants` (V147/V150).
  The global `GlobalMontyOwner` service restores admitted tasks from DB on restart.
  No in-RAM task state is the only copy of task data; all admitted tasks have
  durable PG identity before the worker processes them.

Catalogue/export references:
- Component revisions: `reborn_component_revisions` (V103) — immutable, `ON UPDATE DO NOTHING` triggers.
- Recipe revisions: `reborn_recipes` / `reborn_recipe_variants` — per-package draft/retain path.
- Approval evidence: `reborn_component_review_evidence` (V102) — immutable rows, no delete.
- Pure approvals: `reborn_pure_component_approvals` + `reborn_pure_component_approval_closure` (V200) — immutable.
- Validator Q1 runs: `reborn_validator_q1_runs` (V202) — immutable FK index.
- Pure evidence links: `reborn_pure_evidence_submission_links` (V201) — immutable.
- Submission store: `reborn_component_review_submissions` (V104) — append-only.

Prefix originals/generations:
- `reborn_prefix_acceptances`, `reborn_prefix_owner_revisions`, `reborn_prefix_owner_generations`
  — immutable append-only per existing prefix owner contracts.
- Source-capture: sealed SHA-256 identity stored per captured source file (V177+).

Sealed review/Q2 data:
- Human Q2 approval records: `reborn_validation_queue` rows, `reborn_review_receipts`.
- Override records: stored with submission IDs — not rewritten by package upgrades.

Confirmed/unresolved effects:
- Effect custody: `reborn_tool_invocation_attempts` + `reborn_effect_fence` records
  provide confirmed/unresolved effect tracking for retried dispatches.

**Gaps (explicit blockades):**
- Full crash recovery (durable resume after process restart mid-execution) is not
  yet supported. Any in-flight task at process exit is reconciled as uncertain;
  no in-memory continuation is invented. This is an explicit blockade, not a silent
  drop.
- ABI-loader cross-version native Tool re-load after upgrade is not yet implemented.
  Old native Tool artifact revisions are retained on disk; the retained IBS path
  pins exact artifact identifiers. Missing reloadability blocks upgrade for running
  tasks that depend on the old binary; upgrade must drain/reconcile first. This is
  an explicit blockade, not a pass.
- Full prefix/review restore after crash is blocked pending durable recovery owner
  acceptance (Phase 3a gate).

### K7 — Conformity gate closure record

Per-K0-row records (K0 = baseline, K1 = typing, K2 = composition, K3 = auth, K4–K6 above):

| Gate | Status | Evidence |
|------|--------|----------|
| K1 | Accepted 2026-10-24 | [Python graph review](validation/2026-10-24-python-graph-review.md) — typed inputs/BuildInstruction/snapshot |
| K2 | Accepted 2026-10-24 | Same receipt — composition/IBS assembly |
| K3 | Source-only + BUILD-036 (96c7fc977) | Native Codex credential wiring delivered; acceptance pending native/browser/worker/lint receipt. Browser expiry producers implemented (V197, `extensionOauthExpiry()`). K3 step 6 delivered but acceptance not yet closed. |
| K4 | No defects found — evidence documented above | Five files inspected; two apparent defects were false positives. K4 closes by evidence documentation per plan. |
| K5 | D01–D12 document corrections delivered | Embedded via `include_str!`; boot-test source-capture identity receipt open. |
| K6 | Retention inventory documented above | Supported-scope artifacts recorded; three explicit blockades documented (crash recovery, ABI reload, prefix/review restore). |
| K7 | This record | Per-K0-row table above; no active recipe disabled, no open defect treated as future work. |

**Continuation prerequisite:** K1/K2 accepted; K3 source-only (BUILD-036 delivered,
acceptance pending); K4 no-defect evidence; K5 document delivery done (boot receipt
open); K6 inventory complete with explicit blockades; K7 this record. Remaining
open work proceeds per existing owners: general Pure approval, eight validator
routes, five subagent migrations, full library conversion, accepted prefix designs/
generate/serving/final guard, scope/product cutover, dynamic occurrences, general
crash recovery.

**BUILD-061 compilation receipt:** `cargo clippy -p brassclaw_reborn_composition
--features postgres,skills-db -- -D warnings` → zero errors, zero warnings
(3041703c3). Format check clean.


## 2026-11-01 — BUILD-060: structural_validator_recipe_drafts unit tests

Seven unit tests added to `validator_components.rs` under `#[cfg(test)]`
`structural_validator_recipe_tests` module, covering:
- All eight class codes produce exactly four drafts
- Unsupported class codes are rejected  
- First draft is class 22 (PythonCode)
- Fourth draft is class 21 (Recipe) with `validator_coverage` carrying the correct
  format, `validates_class_code`, `consumer_tags` and exactly one rule (`structural_presence`)
- ID functions produce stable, deterministic, distinct values in the packaged namespace
- Recipe dependencies match the PC/collect/aggregate UUIDs exactly
- `input_layouts` and `invocations` use the expected variant key

Source-only checkpoint (disk space 13 GB NVMe below 15 GB compile threshold).
Tests are not yet compiled/run; compilation requires disk space recovery.



## 2026-11-01 — BUILD-057/058: V202 validator Q1 runs index + structural validator Recipe drafts (source-only)

**V202** (`V202__validator_q1_runs.sql`) adds `reborn_validator_q1_runs`, an immutable
FK-indexed linking table from completed Q1 run → evidence record. It joins
`reborn_component_review_submissions` (V104) + `reborn_component_review_evidence` (V102) with
the `validator_selection_checksum` (hex SHA-256 of the retained `monty-retained-recipe-selection/1` bytes)
as the natural key. Immutability triggers match V102/V200 conventions. Enables evidence
lookup without JSONB parsing.

`pg_validator_q1_evidence.rs` extended to also write the run index row in `reborn_validator_q1_runs`
atomically in the same transaction as the V102 evidence insert, with idempotent conflict handling
and integrity verification on the FK-linked `evidence_id`.

**BUILD-058** adds `structural_validator_recipe_drafts(class_code, class_label, pc_body)`
to `brassclaw_skills::validator_components`. This produces four `ComponentRevisionDraft` objects
(PythonCode + collect + aggregate + Recipe with `validator_coverage`) for each of the eight
supported validator class codes (0, 1, 2, 3, 13, 21, 22, 23). The Recipe draft carries the
`validator_coverage` field required by `RetainedValidatorCoverage::from_inputs()`. Helper ID
functions (`structural_validator_pc_id`, `structural_validator_collect_id`,
`structural_validator_aggregate_id`, `structural_validator_recipe_id`) allocate stable
`packaged_component_id` identities in the shared installation namespace.

`retain_validator_check_drafts` in `builtin_bootstrap.rs` extended to also call
`structural_validator_recipe_drafts` for all eight classes and retain them via
`PgComponentRevisionStore::retain_packaged_draft`. These supply the immutable-revision
path for the retained validator selection; they are separate from the legacy mutable
`reborn_recipes` rows seeded by `seed_validator_recipes`.

Disk space still below 15 GB compile threshold (13 GB NVMe). Source-only checkpoint.
Storing these revision drafts does not establish Q1, human Q2, approval or activation.
Open: durable Q1 admission to global orchestrator (Steps 6/9/12), eight class routes
replacing `load_validator_program`, K3 OAuth browser expiry, prefix Phase C2/C3/D and
complete plan acceptance.



## 2026-11-01 — pg_validator_q1_evidence and module registrations (source-only)

`pg_validator_q1_evidence.rs` adds the durable Q1 evidence writer bridging
completed validator Recipe executions to persisted `component-review-evidence/1`
rows in `reborn_component_review_evidence` (V102). It accepts only sealed inputs
(`RetainedValidatorSubject`, `RetainedValidatorSelection`,
`ObservedSubjectValidatorChecks`, task_handle UUID) — none from request data or
caller-supplied JSON. Derives a deterministic evidence UUID from the submission
and validator selection, inserts idempotently with byte-identical conflict check,
and rejects integrity violations. Two unit tests verify determinism and uniqueness.

`lib.rs` now declares `pg_retained_q1`, `pg_workflow_review` and
`pg_validator_q1_evidence` as `pub(crate)` modules gated on
`postgres + skills-db`, wiring these previously floating files into the compilation
unit. These are source-only additions; no build receipt (disk space 13 GB NVMe,
16 GB SSDE below 15 GB threshold). Storing evidence does not establish Q1,
combination approval, semantic agreement, human Q2 or activation.

Open: trusted review owner durable Q1 scheduling wired to global orchestrator
(Steps 6/9/12), human Q2 route, pure approval pins through all readers (item 7),
eight class validator routes, K3 OAuth browser expiry, L001–L101 migrations,
prefix Phase C2/C3/D and complete plan acceptance.


## 2026-11-01 — V201 pure evidence submission links added (source-only checkpoint)

`V201__pure_evidence_submission_links.sql` adds `reborn_pure_evidence_submission_links`,
a checked immutable successor table for pure-component-review cutover contract item 1.
It links an immutable `reborn_component_review_submissions.submission_id` (V104) to a
`reborn_pure_component_approvals.approval_id` (V200) with the exact
`component_id`/`candidate_version` pair, satisfying the requirement to introduce a
checked successor migration instead of relabelling new fields under the unchanged V102/V106
formats. Immutability triggers match V102/V200 conventions.

`pg_pure_component_approval::record_pure_evidence_submission_link` supplies the Rust write
path: verifies submission and approval existence, matches component/version against the
stored submission subject, inserts idempotently and rejects conflicting concurrent writes.
This is a structural link; it does not imply Q1, Q2, activation or Tool permission.

Disk space (13 GB on /Users/ollama/brassclaw-target, 16 GB on /Volumes/SSDE) is below
the 15 GB compile threshold; this checkpoint is source-only. No build receipt.
Remaining open: trusted review owner with durable Q1 scheduling (Steps 3/8-12),
eight class validator routes, K3 OAuth browser expiry, library migrations, prefix
Phase C2/C3/D and complete plan acceptance.


## 2026-11-01 — pure-component-approval store wired (checkpoint 29102065f)

Checkpoint 29102065f wires the full `pure-component-approval/1` write path.
`V200` migration adds `reborn_pure_component_approvals` and
`reborn_pure_component_approval_closure` tables with immutable-update triggers.
`pg_pure_component_approval` exposes `record_pure_component_approval`,
`parse_and_record_pure_component_approval`, `read_pure_component_approval_root`
and `load_independently_qualified` as the public composition API.
`PureApprovalStore` trait added to `brassclaw_product_workflow` with a concrete
`PgPureApprovalStore` adapter. `POST /api/webchat/v2/pure-component-approvals`
route mounted end-to-end. `authored_monty_catalogue` wired to extend
`independently_qualified` from DB-backed approvals during catalogue preparation.
Descriptor contract test updated to 131 routes.

Also resolved: pre-existing deadpool `Transaction` deref mismatches in
`prefix_acceptances`, `prefix_bootstrap_setup` and `llm_config_service`;
`chunks_exact_to_as_chunks` clippy lint; missing imports in integrated bootstrap
test files. Warning-free clippy and format pass confirmed; all 68 webui_v2 tests
pass. This is a wiring checkpoint; no build receipt. Full plans, K3–K7,
general Q1/Pure approval, validator Steps 3/8–12 and Serving remain open.

## 2026-10-31 — standalone OAuth settings and manual re-review accepted locally

[BUILD052/053](validation/2026-10-31-standalone-oauth-rereview.md) passes one
shared warning-free compilation,11 selected Rust cases (7 actual PG/global,
3 existing HTTP contracts,1 Host API),28 Node,28 static and32 architecture cases,
both actual Chromium paths and connected/default strict checks. No selected
case is ignored. V197 supplies acknowledged standalone OAuth default/maximum
lifetimes, independent of original native/gate deadlines. The browser retains
the clicked account before asynchronous settings/recovery reads. Recipe manual
re-review commits status/tag together without resetting a queue or granting
approval. The exact receipt retains source/image and runtime identities.

Current worker ef997c89… supersedes B6. Source freeze ended after original74626
exited and the common lock was independently acquired. Root keeps sole serial
Cargo ownership, batches compatible changes and continues foreground work during
detached Screen validation. This status paragraph is a later documentation delta;
it is not silently included in that exact local package identity. Native Codex036,
connected I1 qualification, full plans and release/deployed acceptance remain open.

## 2026-10-31 — catalogue/settings facades and existing custody accepted locally

[BUILD045–051](validation/2026-10-31-settings-catalogue-facades.md) establishes21
native Composition cases,28 embedded asset cases,32 architecture cases and42
Node cases, with no ignored selected cases. The shared test build and connected/
default strict Clippy pass. The exact receipt preserves earlier compiler/selector/
lint failures, reused images and the two nonbehavioral final lint-source deltas.

Both Recipe detail routes now share the current installed catalogue's exact
revision/variant/model-input projection, with original document bytes beside
the separately editable legacy row. ToolSkill administration reads actual scoped
rows; unconnected functional-review writes return errors without changing queue,
status or approval evidence. This supplies readers, not missing approval writers.

Recipe rejection commits eligible Q2 queue and row changes together, preserves
manual pre-Q1 rejection and rejected-upgrade live bytes, and serializes actual
queue absence/insertion. Real statement/deferred-commit and two-actor cases pass.
Existing Auth refresh publication can join its caller's transaction; rollback
preserves its independently retained original response and account. Native Codex
credential/issuer/consumer cutover remains private036 work.

Sources independent reads and measurement observation/original fences have Node
coverage; this is not new Chromium or durable unmount/restart qualification.
Current-catalogue presentation neither edits immutable components nor grants
approval. General joint/Pure/S/class routes, remaining K3–K7, component migrations,
full Prefix/Serving/platform/installed/release and complete plans remain open.
Plan prose updates after this receipt change embedded references; a later GitHub
run must cover those final package bytes. CI41197330030 remains in progress on
the preceding pushed2522569198… checkpoint, not this bundle.

## 2026-10-31 — validator/CLI/admission bundle accepted locally; checkpoint CI next

[BUILD040–043](validation/2026-10-31-validator-cli-admission.md) establishes14
native Composition cases,1 CLI case,32 architecture cases and both strict
feature configurations; no ignored selected cases or compiler warnings.
The global case includes four Chromium paths. Exact source/image reuse avoids
repeating unchanged relevant checks. [BUILD044](validation/2026-10-31-domain-collector-preservation.md)
separately establishes five actual file/Git/SQLite methods/fifteen domain subcases.

New primary class50 submissions fail before staging/review writes, while exact
historical originals, retries, pins and class10 remain supported. Mechanical
Skill-call-target checks do not open joint approval gates. The native source
observer change is test-only; the actual328-source job completed in53.2 seconds.
The corrected CLI case passes with the existing production guard intact.

The final checkpoint changelog update is embedded reference content and follows
these local receipts. Their exact pre-changelog package identities stay historical;
new GitHub CI must compile and exercise the final embedded reference successor.
No local recompile is claimed for that final prose delta. Complete plans, general
approval, remaining class50 migration, SDK036/Host-PG, platform/installed/Serving
and release qualification remain open. Root keeps Cargo and continues after push.

## 2026-10-31 — standalone domain collection preserves previous working state

[BUILD044](validation/2026-10-31-domain-collector-preservation.md) fixes the three
existing Defensive/HA/tomedo source collectors. Incomplete reads/scans fail
visibly; one scan transaction rolls back earlier repository changes and derived
invalidations. Working manifest and FINAL publication follow the completed scan.
Five actual file/Git/SQLite methods, fifteen domain subcases, pass with no skips.
This does not establish DB/filesystem crash atomicity, other collector behavior,
native domain migration or Serving activation.

The global Monty/PG/HTTP case and all four Chromium paths have also passed on
BUILD040/042's exact source/image. Eight Skill/Pure native cases pass. The next
serial Screen batch65594 compiles the connected corrected CLI and fresh-class50
admission changes once, then runs their selected native, architecture and strict
checks. These remaining results and new GitHub CI acceptance are pending.
Shared build procedure now batches ready compatible changes and continues
foreground work, with actual dependency-scoped input freezes. Full plans remain
incomplete; no release/installed/activation claim follows from these checks.

## 2026-10-31 — Prefix HTTP revision precision accepted locally

[BUILD039](validation/2026-10-31-prefix-revision-wire.md) preserves exact outer
browser revision identities as decimal strings. Three inbound fields retain
legacy unsigned-number compatibility; shared native serialization and original
Integer Skill contracts stay numeric. Actual protected HTTP/PG/Chromium at
2^53+1 and i64::MAX preserves capture/import originals, identical unknown-ACK
retry and stale conflicts. u64::MAX is formatted exactly and rejected by BIGINT
consumers. Missing-prerequisite high measurement is rejected without a job;
this is not positive high-measurement or its stale-CAS qualification.

78 ProductWorkflow, 5 HTTP, 28 embedded static, 1 global native, 32 architecture
and 42 Node cases pass; four actual browser paths are inside the global case.
Warning-free compilation and connected/default strict Clippy pass. Failed runner
parser/disk attempts and necessary package-cache cleanup are preserved. Source
manifest aa6e65af… and receipt0512f9ee… establish this exact scope, not full-plan
or platform/installed/Serving approval. Root retains serial Cargo; freeze released.

Full CI41188072054 is terminal failure at preceding checkpoint34ead: missing
embedding fixture contract and120-second large-source observer, plus rollup.
Private041/042 repairs await actual connected acceptance. The validated local
precision diff is not included in that remote run. General joint review,036
Codex Host/PG custody and remaining K3–K7/full plans stay open.

## 2026-10-31 — connected Prefix owner packet accepted locally

[BUILD029/032/033/034/035/037/038](validation/2026-10-31-prefix-owner-integration.md)
passes warning-free compilation,10 distinct native product cases,28 embedded
asset tests,32 architecture cases,28 Node cases and23 Python methods, zero
ignored/skipped in accepted results. Actual global PostgreSQL/HTTP/Monty and
both Chromium paths are included. Existing source/code/immutable originals and
retained native Integer contracts remain distinct from HTTP presentation.

The shared literal-fence correction and non-BMP escapes preserve original data,
card identities/context maps and unaffected layouts. Existing History session/
profile fencing and exact uncertain-request retry are exercised natively. The
ModelDesign view shows actual outer identities and complete optional originals,
without deriving approval/Serving facts. Native base/Sempai source jobs now cover
328 declared originals, five calls each, without replay/activation. Nine added
reference documents remain development-only; previous selected model-view rows
are unchanged. The Prefixless choice and general bootstrap qualification remain
future requirements, rather than activated model paths.

Original warning, browser, disk and runner-parser failures remain recorded.
Unchanged compiled images/worker and successful checks are reused; R7 explicitly
runs the real HA schema case omitted by an earlier missing environment flag.
CI now supplies actual prerequisites and demands both native browser markers;
actionlint passes. Checkpoint `34ead1e88` is pushed; actual GitHub acceptance
is pending [full CI run 41188072054](https://github.com/chtugha/brassclaw/actions/runs/41188072054).

Root423 source manifest f71583df… and receipt bb58d98f… establish only this local
scope. Root retains permanent serial Cargo ownership; all original runners ended
and the common lock was independently acquired. The source freeze is released.

036 Codex SDK work remains private until the same encrypted host/PG owner,
instance-subject original/rotation custody, per-request guard, live projection and
all startup/Kohai/Sempai/retained-task consumers are connected. The existing PG
refresh journal is reused. Prefix revision precision has a reviewed private
HTTP-only successor; current native Integer contracts/retained bytes must stay
unchanged. Full K3–K7, general approval/Pure/Prefix, recovery/platform/release/
installed acceptance and complete plans remain open.

## 2026-10-31 — actual CI encoding and shared-chat fixture repaired locally

[BUILD028](validation/2026-10-31-ci-encoding-chat-fixture.md) supplies the actual
retained worker, independent Python oracle and six pinned public tokenizer/config
files to the mandatory CI encoding consumer. Four existing explicit HF oracle
cases are wired into full validation. Five helper CLI cases include a real pinned
1125-byte download and strict retained-file failure checks. Actual encoding,
two-worker shared-PG direct chat and both strict negative-evidence cases pass;
affected Clippy and actionlint pass, zero ignored. The existing request-keyed
replay source is shared because either unscoped worker may claim either task;
all original isolation assertions remain. New GitHub acceptance remains pending.

The [Prefixless successor](reborn/model-token-limits.md#open-prefixless-successor--visible-makerbaseprefixless-construction-selection)
is now the single future construction-choice contract. Maker/Base/Prefixless is
an initial visible editable suggestion, with the actual submitted selection pinned
before effects. This prose does not activate a Free branch or replace protected
old workflow/packet criteria. Owner plan connections and actual runtime/UI/guard/
response/retention support remain open. [BUILD031](validation/2026-10-31-prefix-response-hardening.md) separately passed
13 actual offline/HA-schema cases and its real compiler caller, zero skipped.
Later033/037 source successors require their own applicable package/caller checks;
the original exact-source receipt is not rewritten.

## 2026-10-31 — acknowledged task-gate OAuth lifetime accepted

[BUILD027](validation/2026-10-31-gate-oauth-lifetimes.md) adds one acknowledged
positive u32 default for new Google/DCR task-gate setup. Concurrent creation,
retries and live edits retain the first original expiry; expired originals stay
expired. V196 preserves explicit settings and unrelated defaults. EN/DE controls
and help, Rust/Monty effective revisions and restoration are covered.

91 distinct native cases (72 unchanged results reused),32 architecture cases,
zero ignored and connected/default strict Clippy pass. Actual PG/FS/HTTP/global
Monty, embedded/base/Sempai and Chromium consumers are included. Original failed
test expectations and sibling packaging attempts remain recorded. This is local
macOS/task-gate scope; standalone OAuth default/max/client producers, remaining
polling/transport/ingress caps, full K3 and all later approval/Prefix/recovery/
platform/release/full-plan gates remain open.

The actual033 GitHub Docker job has since passed. Heavy direct-chat fixture and
encoding prerequisites failed independently; BUILD028 now integrates their
connected repair for focused acceptance. The whole GitHub run is not a pass.

## 2026-10-31 — claimed handoff identity repaired; Docker source pending CI

[BUILD026](validation/2026-10-31-ci-handoff-repair.md) shares the existing
interactive-default/persisted-default projection across storage, host validation
and the strict Monty handoff. Complete selected profiles, versions and all other
custody checks remain; no stop acknowledgement or Rust fallback is invented.

All105 original CI-profile host cases and262 Reborn library cases pass, separately
from279 default/native cases;32 architecture and connected strict pass. Five pure
Chef recipe-transformer cases are reused. Docker includes complete vendor/scripts,
locked builds and semantically equivalent generated editions. Its panic-abort
override is removed to retain existing host/task unwind boundaries. Actual Docker
compilation remains pending on GitHub; local source checks do not close that gate.

The previous69024bee GitHub run is terminal: Docker/Reborn failed, rollup failed,
both contained Ubuntu/macOS and native WebUI jobs passed; cancelled packages are
not passes. This local repair is not full CI/platform/deployment or full-plan
acceptance. All remaining K3 and later owner gates remain open.

## 2026-10-31 — OAuth observation follows original expiry

[BUILD025](validation/2026-10-31-oauth-observation.md) removes the independent
fresh ten-minute observation window. The existing browser observer receives the
checked original receipt expiry and also observes a popup-blocked recovery link.
Invalid/expired metadata cannot create another window; confirmed terminal
receipts, popup close and unmount clean it up. This is read-only UI observation.

26 Node cases,28 fresh embedded-asset cases and1 native HTTP/PG/Chromium caller
passed, zero skipped/ignored, with static strict lint. Actual timers are audited
while their scheduling/cancellation remains real. The fixture controls issuer
responses; no external provider-health or approval claim follows. OAuth new-setup/
max/gate lifetimes, explicit client-expiry successor, polling/transport/ingress,
all other K3–K7 gates and full-plan/platform/release acceptance stay open.

## 2026-10-31 — acknowledged manual-token setup lifetimes accepted

[BUILD024](validation/2026-10-31-manual-setup-lifetimes.md) connects the two new
manual setup defaults through the existing acknowledged settings/Auth owner,
actual generic HTTP handlers and Extension adapter. V195 preserves partial/full
operator values, complete unrelated defaults and every old Auth original. The
first accepted expiry survives live changes, retries and concurrent creation;
expired originals are not renewed. Direct native explicit-expiry calls keep their
contract. EN/DE controls/help and real Rust/Monty effective uptake are covered.

46 native cases+32architecture/0ignored, fresh compilation, actual PG/FS/HTTP/
browser/global consumers and connected/default strict Clippy passed. Exact source,
images and original logs are in the receipt. This is local macOS evidence, not
new GitHub/platform/deployed support or full K3 completion. OAuth default/max/gate
and browser expiry/popup/transport/ingress controls and all remaining owner gates
stay open. Root retains central Cargo ownership after releasing the sourcefreeze.

## 2026-10-31 — acknowledged credential-backend wait accepted

[BUILD022](validation/2026-10-31-auth-backend-wait.md) connects the existing neutral
acknowledged policy to every protected Auth backend wrapper and the common
custom/PG/FS factory attachment. V194 preserves explicit values/unrelated defaults;
EN/DE WebUI controls/help and actual effective status are covered. Live reduction
retains an in-flight deadline; configured unavailable policy prevents backend poll.
All34 settings upgrades and real original PG/HTTP/manual-browser/resource-browser
consumers pass:42 native+32architecture cases/0ignored, connected/default strict.
The script-only restoration repair reuses unchanged compiled code and preserves
full equality/two-write checks. No new settings/workflow/approval owner.

This is per-backend-call waiting, not complete Auth lifecycle control. Setup
lifetimes, browser expiry producers/popup waits and independent browser/ingress
limits remain open in the same K3 owner. General approval/Prefix/recovery/platform/
release/full-plan work remains open. See the exact receipt rather than treating
a source or local acceptance result as installed support.

## 2026-10-31 — checkpoint CI repairs and native browser editing

[BUILD020](validation/2026-10-31-checkpoint-ci-repairs.md) passes connected
compilation,125 native cases,32 architecture checks,24 tagged Monty cases,1 legacy
case and connected/isolated strict lint. The neutral Auth boundary retains its
original private wire identity; atomics preserve overflow/saturation; original
five Subagent parity cases pass with concurrent native storage preparation.
The upstream test driver now observes the already existing pool coordinator.
Production orchestration has no Rust-loop fallback.

[BUILD021](validation/2026-10-31-native-browser-limit-clear.md) passes both original
Chromium settings consumers against real PostgreSQL/global runtime. Limit clearing,
exact large values, conflicts, lost acknowledgements, live effective revisions and
EN/DE help are checked. This JS-only repair reuses the actual unchanged native
image. Original failures remain attributed. No new Linux/Windows/GitHub deployment,
release, component approval or full-plan completion is established. Existing
K3 Auth lifecycle/transport controls and all later owner gates remain open.

## 2026-10-31 — Shared review-subject reader and Skill declaration checks accepted

[BUILD019](validation/2026-10-31-joint-validator-checks.md) connects one private
subject reader to all five existing consumers and adds actual Monty Skill
interface checking using the recursive grammar and existing export-shape helper.
Complete function pairs and unchanged context, exact extraction, metadata
negatives, isolation, operator successor preservation and the existing native
inventory/structure/pure/global consumers pass:10 distinct native cases/0ignored,
compilation and connected strict. R1's runner selector failure remains recorded;
its unchanged image and six successful cases were reused without recompilation.

This does not establish complete joint source/call/binding/variance/behavior or
semantic review, admitted review custody, trusted Q1/Q2 or publication. The
single-class preparation refusal remains. General Pure/S qualification and
Prefix I1/I2 remain open. Root keeps central Cargo ownership. All chats confirmed
a separate shared-source freeze for the requested main checkpoint/app update;
resume from the development handoff after that update.

## 2026-10-31 — Joint function subjects retained; review and approval remain open

[BUILD-018](validation/2026-10-31-joint-function-subject.md) connects the existing
operator/store/JSON WebUI path for strict `/3` Skill/code function subjects:
two changed roots or one change with exact partner context, separate base/head
CAS, atomic UUID-ordered staging and one V149 intent. V193 preserves old `/1` and
`/2` bytes. Complete typed `/2` inputs retain both sources and comparison bases;
existing single-class preparation rejects partial coverage at Engine and store.
Unchanged context is not reapproval or a permanent runtime version coupling.
Sixteen sources, fresh compilation,15 distinct native tests/0ignored,7 WebUI
tests/0skipped and connected strict pass. Original processes exited;8180 named
inputs, Root287, worker and exact runtime/model assets independently verified.
The existing wrong-base409 contract was repaired for both roots; failed attempts
and required disposable-cache cleanup remain recorded.

Next, Validator Steps3/8–10 own complete joint checks, actual admitted run/attempt/
interpreter/settlement custody, trusted own verdicts, atomic human Q2 and compatible
publication through every actual reader. Standalone Pure and independent S
qualification remain separate open work. Do not remove the partial-preparation
denial or finish the obsolete generic authored closure-reapproval design. No new
Q1/Q2/publication/Free, inference, platform or full-plan claim; Root retains Cargo.

## 2026-10-31 — Offline model oracle warning causes resolved

[BUILD-016/017](validation/2026-10-31-model-oracle-runtime.md) integrates the actual
BUILD014 nested metadata corrections and repairs the unsupported generic-config/
missing-backend/model-input-wrapper warning causes. Existing test callers share
an explicit isolated supported Python runtime; original model/template/tokenizer
bytes and context rejection remain unchanged. Physical counting disables padding
and truncation without changing model limits or filtering warnings. Four explicit
offline HF cases plus the actual native global/base/Sempai consumer pass,0ignored;
exact unchanged Rust compilation/connected strict reused after the sole external
counting-script correction. No provider/server configuration, real inference,
accepted design/prefix or platform-deployment claim. The warning noted in the
historical BUILD015 entry below is resolved by this scoped repair.

## 2026-10-31 — Validation target corrected; runtime successors remain open

[BUILD-015](validation/2026-10-31-validation-target-documents.md) integrates
20 reviewed definition/contract documents plus Root policy. Authored own functions
with joint Skill/code review, compatible successors and unchanged consumer
approvals are the new target; S keeps exact usage and independently qualified
prerequisites. Old strict profiles stay their actual implementation. New writers/
readers, standalone Pure and complete qualification remain open. Fresh compile,
three native cases/0ignored and connected strict pass current embedded baseline/
reference/global deployment consumers, not the new runtime target.8176 named
inputs now cover484 discovered literal embedded files; old receipts are unchanged.
The observed Transformers5.1.0 config warning is retained and next for Root repair.
Full plans, I1/I2/Free/Serving, recovery/platform/release remain open; Root keeps Cargo.

## 2026-10-31 — Retained validator model selections accepted

[BUILD-014](validation/2026-10-31-review-model-selection.md) repairs existing
compiler/preparation disagreement without adding a selection format or authority.
One existing enum envelope is compared to the exact original Recipe/variant;
absent and explicit Default remain distinct. Five sources, fresh compilation,
nine distinct cases/0ignored and connected/default strict pass. Actual PG/IBS/
worker-parser/restoration, checksum-correct forgeries and original selections
after successors are covered; the new metadata case creates zero approval,
admission or Tool-effect rows. General Pure/own-function review, Free qualification,
I1/I2, recovery, platform and full plans/release remain open. Root keeps Cargo.

## 2026-10-31 — Actual model-host compatibility consumers accepted

[BUILD-009](validation/2026-10-31-model-host-compatibility.md) migrates the old
standalone model-host acceptance into actual Composition owners. Five providers
use coherent prepare/bind and complete retained selections; no rematch or dispatch
at binding. All22 original owned cases remain plus one extracted native invalid-
reply case. Final31-path package passes24 fresh tests/0ignored, compilation,
connected and standalone all-targets strict, format and CI/YAML/scope checks.
Shared fixtures load once; the identical existing observation parser is reused by
native and isolated admission consumers, with the native capture seal separate.
The production-only capture query now lives in its existing lifecycle owner.
No model/approval/activation widening, fallback or new suppression. Full I1/I2,
general pure approval, BUILD-010 compatibility/deprecation migration, complete
plans/platform/release remain open. Root retains permanent sole Cargo.

## 2026-10-31 — Native review accounting distinguishes no-submission

[BUILD-012](validation/2026-10-31-review-unsubmitted.md) preserves actual native
no-submission evidence through the existing Sempai error adapter and forwards it
to supported accountant reconciliation. Five distinct tests/0ignored and connected/
default strict pass. The real GovernorBackedAccountant/in-memory governor with
PG/native/SDK checks distinct primary/review IDs: pre-submit BudgetExceeded releases
both; observed Length BudgetExceeded releases primary only and retains review
spend for reconciliation. No category-derived proof, retry or new ledger. This
closes that caller repair, not durable-governor recovery/billing, full I1/I2,
Free/Serving qualification, compatibility or complete plans/platform/release.

## 2026-10-31 — Native pre-primary Sempai custody accepted

[BUILD-011](validation/2026-10-31-related-model-custody.md) adds eleven sources/
V192 under the existing native invocation/response owner. Ten distinct tests/
0ignored and connected/default/neutral strict pass. Related Sempai work keeps its
own original host request, captured route/Prefix, independent submission fence
and unchanged wire/provider/loop evidence. Actual failed writes reconcile after
cancellation without replay; PG retains full requests and RAM only their digest.
This covers the current review before primary submission, not a post-primary
path. The subsequent BUILD-012 caller repair above adds native known-unsubmitted
review-budget reconciliation.
Real inference, actual sealed Free qualification, full I1/I2 and remaining plans/
compatibility/platform/release stay open; no additional approval or activation.

## 2026-10-31 — Shared model capture and contained native binding accepted

[BUILD-008](validation/2026-10-31-shared-model-capture.md) connects the existing
global and pre-readiness/development contained owners to one original task-model
capture/metadata handoff before native binding. Six sources, nine distinct cases/
0ignored, actual PG/kernel/SDK execution and connected/default strict pass.
Original unstarted source/claim/lineage, foreign admission and re-entry fences
are checked; child, selected/default model, ordinary routing and pure consumers
remain covered. The actual sealed qualification source remains open; subsequent
BUILD-011/012 above cover independent pre-primary review custody/accounting.
No second live VM, Free approval/activation, complete
I1/I2 or full-plan/platform/release acceptance is implied.

## 2026-10-31 — Standalone strict JSON usage prerequisite accepted

[BUILD-007](validation/2026-10-31-standalone-strict-json.md) adds three reviewed
sources: one existing strict JSON Tool usage, preloaded canonical-text export and
actual retained/native/PG consumer. Thirty-eight cases/0ignored, current global
ResultOnly/SDK,34 service cases and connected/default strict pass. Original
raw text, native payload and export stay separate; actual negative dispatch and
unchanged approval counts are observed. No parser, Tool, approval or activation
is added. Fresh worker build is byte-identical to227a4b…, resolving the earlier
build-dependency uncertainty. Subsequent BUILD-008/011/012 above supply shared
contained capture/metadata/native binding and pre-primary review custody/accounting.
BUILD-009 above closes the historical model-host capture adapter migration
and compiles the isolated retained/Q1 consumers. Full I2 and legacy DocType
bridge/suppression migration remain open with their existing owners. Full K3–K7/validator/library/prefix/recovery/platform/release stay open.

## 2026-10-30 — Shared internal model completion foundation

[BUILD-004 acceptance](validation/2026-10-30-internal-model-admission.md) binds
thirty sources/V191, twelve distinct native/consumer cases (eleven verified
unchanged results reused in terminal Run7), zero ignored and connected/default
strict checks. Original claimed source selects ResultOnly independently of the
existing deterministic model prohibition. Actual global driver/model/PG custody/
exit evidence, immutable origin/upgrade guards, no assistant or No-Match review,
real host-preparation failure and existing ordinary/private/child paths pass.
The bootstrap optional real HostFactory prepares an original unstarted admission;
subsequent BUILD-008/011/012 above add contained task-model capture/native binding
and pre-primary Sempai custody/accounting. No workflow/design/Free activation or
complete I1/I2 claim.
Standalone monty_control historical capture adapters were outside that batch;
BUILD-009 above now accepts their current consumer migration. Remaining K3–K7, full plans and release
stay open; Root retains permanent sole Cargo.

# v3 implementation status and work ownership

**Native strict JSON — scoped prerequisite (2026-10-30):**
[The four-source package](validation/2026-10-30-native-strict-json.md) adds
`parse_strict` to the existing JSON primitive/common bounded reader. Exact v1
and legacy operations remain unchanged; v2 uses one schema owner. Actual native
retention/identity, live policy, bounds, schema and consumers pass11 fresh cases/
0ignored, compilation and connected/default strict. Root repaired R1's test-only
wrong descriptor projection through existing APIs. Worker227a4b… is unchanged;
the Tool's application/test image is new. No Skill/Recipe/Free activation,
general upgrade or complete B1/B2/I2/platform support follows from this receipt.
BUILD-004 shared Tier1 ResultOnly/model-host/review custody and full plans remain
open. Root retains sole Cargo through the central queue.


**Original native model-output setting — scoped acceptance (2026-10-30):**
[The fifteen-source package](validation/2026-10-30-original-model-output.md) carries
optional positive prompt `max_output_tokens` from verified original PG arguments
through existing custody to actual SDK/final guard, with discretionary budgets
disabled and enabled-limit rejection without clamping. Run3 compiles and passes70
fresh/4 unchanged reused cases/0ignored, all60 gateway regressions and selected
strict checks. Root repaired lost `default` placeholder rejection and an
incompatible concurrency fixture; failed attempts remain recorded. Current worker
227a4b… is unchanged. The global/static-route and native/final-counter fixtures
have separate scopes; neither activates Free or proves complete child/review
closure. Full I2/I1/Serving/K3–K7/release remain open. Root retains sole Cargo;
the reviewed strict-JSON prerequisite remains Ready and unintegrated.


**Acknowledged DCR metadata-cache control — scoped acceptance (2026-10-30):**
[The ten-source package](validation/2026-10-30-dcr-metadata-cache.md) adds V190 and actual EN/DE/live
`oauth_dcr_metadata_cache_ttl_secs` (default600s,0 disables reuse). Per-lookup
acknowledged policy, original entry age and lazy invalidation keep cache changes
separate from accepted Auth expiry/client/registration. Run1 freshly passes548
distinct cases/0ignored, actual worker/Composition builds, global/Chromium uptake,
PG defaults/overrides and affected/default strict checks. Fourteen incoming Exact
UI sources are unchanged; its locales only gain two keys each. No live issuer,
complete lifecycle/Free/upgrade/platform acceptance is claimed. Root retains the
single Cargo queue; Auth lifetimes/browser/backend/ingress, own-flow/Auth-wait/
Codex/policy/recovery and complete K3–K7/Exact/Prefix remain open.


**DCR original gate recovery — scoped acceptance (2026-10-30):**
[The two-source correction](validation/2026-10-30-dcr-gate-recovery.md) recovers authenticated encrypted
preparation before deriving a new gate expiry/provider defaults. Exact gate
provenance remains mandatory. Run2 passes34 distinct DCR cases/0ignored and
connected/default strict checks. Both native DCR fixtures now explicitly select
PostgreSQL Auth. The earlier HTTP fixture's PostgreSQL settings/global hosting
did not also establish PostgreSQL Auth ownership; its historical receipt keeps
that narrower scope. All16 incoming Exact HTTP/UI sources remain unchanged.
Root retains Cargo through the [central development queue](development-build-queue.md);
historical build-window handoffs no longer govern future cooperation. Lifecycle/
cache controls, native own-flow/Auth-wait/Codex, full K3–K7/Exact/Prefix/recovery/
platform/release/deployment remain open.


**Exact manual model setup HTTP/WebUI — scoped consumer acceptance (2026-10-30):**
[The sixteen-source successor](validation/2026-10-30-exact-model-setup-ui.md)
connects the existing V188 draft owner through authenticated GET/PUT and the
existing Model Designs details. Whole-revision CAS, original UUID/body retention,
exact successor/history checks and fresh current readback protect lost replies,
remounts and newer edits. Direct input events preserve same-value manual intent
and explicit null; draft saves neither change Provider policy nor approve designs.
Real native middleware/facade/PG and Chromium, 16 Node cases, route descriptors and
affected strict checks pass. V188/V189 and all eight native draft source hashes are
preserved; current worker remains ece271…. Earlier receipts below retain their
dated scopes. Use/discovery/native automatic Prefill, qualified Free child/review,
design generation/release, Prefix-Q1/Q2 and reference-safe GC remain open. Full
I1/I3/I4/Exact/platform/release completion is not claimed. Source/Cargo returns
explicitly to Root/Auth at the terminal handoff.


**DCR HTTP settings — scoped per-request acceptance:**
[The fourteen-source successor](validation/2026-10-29-dcr-http-settings.md)
adds V189 and two live execution/WebUI controls for discovery, registration and
cleanup. Each new HTTP request captures the acknowledged pair; the running GET
keeps its old pair and the later GET takes the new revision. Missing settings
cannot consume a new registration/cleanup claim. Token/DCR use one reader and
factory owner. Current compilation, 33 fresh DCR cases and connected/default
strict pass; 549 other R1 cases retain their exact source/image scopes, including
actual Chromium save/uptake/restoration and all 10 incoming Exact model cases.
The superseded grouped-discovery probe is excluded. All eight Exact setup-draft
afterhashes and V188 are unchanged. Setup/cache lifetimes, server/browser/setup/
ingress controls, native own-flow/Auth-wait/Codex/recovery and full K3/Exact remain
open. See the handoff for current Source/Cargo ownership.

**Exact model setup drafts — scoped native backend accepted:**
[The eight-source successor](validation/2026-10-29-exact-model-setup-draft.md)
adds V188 append-only unaccepted drafts in the existing model/design owner and
existing product service/facade. Whole-revision CAS, original operation identity,
exact historical reads, required nullable fields and grounded configuration origin
prevent stale writes/defaults. Drafts never publish Provider policy or approval.
Ten fresh actual cases, including seven real PG cases and the real product service
without model/VM, pass; connected/default Composition/Product strict and formatting
pass. No HTTP/UI/Use/discovery/native Prefill import, Free dispatch or design
activation is supplied; full I1/I3/Exact remains open. Source/Cargo returns explicitly
to Root/Auth. The current601b… worker is preserved; old91a… remains historical.

**OAuth token transport — scoped live-settings acceptance:**
[The twelve-path successor](validation/2026-10-29-oauth-token-transport.md)
replaces the generic token client's fixed timeout/body cap with two acknowledged
execution settings, V187 and actual EN/DE WebUI fields/help. Callback and refresh
capture one pair before dispatch; unbound configured settings block requests.
Native PG migration, in-flight retention, global execution, actual Chromium
save/uptake/restoration and affected strict checks pass. R3 contributes 35 fresh
cases; unchanged R1/R2 owners retain 505 distinct cases within their original
source/image scopes. Fifteen historical-default test fixtures are repaired
without weakening production constraints or editing old migrations. DCR
registration/cleanup, server/browser waits, setup/ingress guards, native own-flow/
Auth-wait and full K3/Exact/platform/release remain open. Source/Cargo ownership
is recorded in the handoff.

**Exact bootstrap prefill candidate — scoped pure component accepted:**
[The four-source successor](validation/2026-10-29-exact-bootstrap-prefill.md)
extends the existing bootstrap stage with deterministic unique-option selection
and checked B2 ID selection. It calls the retained model-budget export, preserves
protected zero/null values and returns all four field preconditions for the future
native CAS. Twenty-three Python cases also pass through93 actual Monty utility
evaluations; two Rust constructor cases and one real PG/IBS dependency-first
source inspection pass. Skills/Engine relevant strict and format/whitespace pass.
No native correlation/import/CAS/store/UI or active Recipe/Free qualification is
supplied; I1 and full Exact remain open. Source/Cargo returns explicitly to Root/Auth.

**Raw invocation preflight successor — scoped race repair:**
[The two-file successor](validation/2026-10-29-legacy-raw-preflight.md) retains
preflight-selected authenticated raw sources through actual audit/resource awaits
and injection. Bound and unbound legacy injection use conditional issuance; a
later replacement receives no lease, and partial handoffs are removed by abort.
Compiler/format/Host and connected/default Composition strict pass with 107 fresh
cases and no ignored cases. All 25 incoming Exact Free-capture paths are unchanged.
Legacy selection remains invocation-local; this is not original-family/own-flow/
refresh/native Auth-wait or full K3/Free support. Combined scoped inventory remains
160 inputs; current Source/Cargo ownership is in the handoff.

**Exact native prefix-free capture — scoped infrastructure accepted:**
[The 25-path successor](validation/2026-10-29-exact-prefix-free-capture.md)
derives an opaque capture-only declaration from the actual retained root Recipe,
reuses its original committed match and captures the original model family
without reading Prefix bindings. V186 adds immutable /3 observation metadata;
old /1-/2 selections and migrations stay unchanged. Sixteen actual cases cover
capture with the Prefix table unavailable, original family retention, rejected
Free dispatch and Default/Selected/global/child/embedding regressions. Ten native
results have exact import-only applicability; six Core results are fresh on the
final source. Connected/default strict and format/whitespace checks pass. Free
workflow qualification, technical context/preflight, occurrence evidence,
required child/review dispatch and the rest of Exact remain open. This is no
active Bootstrap or complete I2. Source/Cargo returns explicitly to Root/Auth.

**Raw-handle family/consumer successor — scoped acceptance:**
[The eleven-path successor](validation/2026-10-29-raw-source-family.md) captures
original raw generations/absence on the existing family and binds the exact
invocation's raw resolver/use fence. Bound obligation preflight/injection uses
original metadata and the shared conditional lease/consume/cleanup path; later
material cannot refill absence. Compiler/format/Host and connected/default
Composition strict pass with 111 fresh actual cases and no ignored cases.
Current native identity/policy, account sources and Default/Selected/global/child
consumers are preserved. Native raw/extension/ToolSkill workflow qualification,
unbound legacy callers, own-flow/refresh/native Auth-wait/UI and recovery remain
open; this is not full raw migration or K3 completion. The scoped incoming 154
inventory advances to 156 inputs; live Source/Cargo ownership is in the handoff.

**Exact retained Recipe/profile selection — scoped producer/consumer accepted:**
[The 22-path successor](validation/2026-10-29-exact-recipe-prefix-selection.md)
adds strict per-variant model-input metadata and pins it in the existing retained
selection. Native Selected-profile preflight/dispatch uses the original pair and
qualification, including a pair with no Base; custody/final guard agree and
ordinary Sempai keeps its own role contract. 21 actual selected cases/0ignored,
affected connected/default strict checks and formatting pass. Auth's 24 after-
hashes below remain unchanged. Free intent is recognized but native activation/
execution stays blocked pending complete qualification/context/child/review;
I2 and all other Exact I-packages remain incomplete. Recipe guide documents the
supported retained API; no new wire/provider selector or global prefix flag.
Source/Cargo returns explicitly to Root/Auth after this terminal block.

**Original Auth-flow generation proof — scoped owner acceptance:**
[The 24-path successor](validation/2026-10-29-original-auth-flow-source.md)
retains the authenticated encrypted generation with the existing original
selection at first native PG transaction/RecordFS decision CAS. Manual PG writes
its retained output before capturing proof in the same transaction. Original-only
readers verify exact selection/source binding; historical absence cannot refill
from later material or today's head. Actual memory/native-backed RecordFS and
native PG publication/recovery pass 73 fresh cases; unchanged 120 Auth cases and strict checks
remain applicable. Compiler, formatting and connected/default Composition strict
pass, with no ignored cases or warning suppression. This closes the original
access-generation storage/readback prerequisite, not task-family own-flow or
refresh progression, native Auth-wait production, aggregate readiness, activated
credential workflows or full K3. Preserve Exact's typed Prefix contracts below.
The current scoped Root/Exact/Auth inventory has 139 inputs; live Source/Cargo
ownership and exact artifacts are in the development handoff.

**Exact typed prefix-input successor — scoped contract accepted:**
[The ten-path receipt](validation/2026-10-29-exact-prefix-input-binding.md) records
seven actual selected cases/0ignored and affected Reborn/Turns/Composition
connected/default strict checks. Default/Selected/PrefixFreeWorkflow are distinct
from physical insertion; the original custodian and final gateway compare choice
and reference while preserving the guard, instructions and output reservation.
The existing `/1` writer still rejects Free. No qualified free-workflow capture,
persisted successor, Selected resolver or recursive child/review propagation is
activated; Exact I2 and all other I-packages remain incomplete. The prior
Root/Auth nineteen-path result below is consumed without changing its credential
carriers. Source/Cargo returns explicitly to Root/Auth after this terminal block;
the current handoff has exact artifacts and open gates.

**Original Tool-account source/use successor — scoped infrastructure accepted:**
[The current nineteen-path record](validation/2026-10-29-tool-source-capture-and-use.md)
captures original ProductAuth-account choices and actual encrypted generations
before ordinary runtime binding, carries the actual parent family into children,
and binds services to the original journalled invocation. Explicit v3 binding
requires the same owner's source-use guard around conditional issuance/consume;
fenced unused leases are revoked and late material is not staged. Original missing
ports/current kernel policy remain enforced. Current compiler/format/Host and
connected/default Composition strict checks pass with103 fresh distinct actual
cases/0ignored; older eleven-case capture results retain their separate source.
The complex tuple warning is resolved by named services, without suppression.
Native built-in declarations currently have no credential usages: this is no
manufactured GSuite registration, native auth-wait producer or activated workflow.
Raw handles, own-flow/refresh progression, complete native/global/UI/final-use
and durable recovery remain open in their existing owners. Root returns the
Source/Cargo window explicitly after the terminal handoff; the live owner and
current117-input scoped inventory are recorded in development-handoff.md.

**Retained credential services — scoped Host acceptance:**
[The five-path successor](validation/2026-10-29-retained-credential-services.md)
adds exact native declaration inspection and explicit credential service views
on the existing captured kernel/handler/substrate. Missing original ports remain
missing before preparation and during service resolution; no access is added by
the view. Compiler, formatting and affected Host library/tests strict pass with
97 actual cases/0ignored, including selected-material staging/abort and actual
native identity/current-policy enforcement. Do not sum overlapping older Host
receipts. Root/Auth owns Source/Cargo after Exact's explicit terminal return.
The later nineteen-path source/use successor above supplies original capture,
child-family carriage and explicit source-use fencing. Own-flow/refresh progression,
native wait production and complete final-use/UI integration remain open; these
scoped foundations do not activate a credential workflow or complete K3.

**Exact native preparation successor — scoped infrastructure accepted:**
[The new receipt](validation/2026-10-29-exact-native-preparation.md) records seven
actual native integration cases/0ignored and connected/default Composition plus
Auth lib/tests strict checks. Original catalogue/internal/child preparation now
precedes model capture and late runtime binding; cancellation acknowledgement
and actual dispatch fences stay with the existing owners. Bootstrap drafts compile
(two Rust tests), and the existing final counter has scoped prefix-free encoding
support (one native counter test), with no production workflow verifier or free
Kohai dispatch yet. All Exact I-packages remain incomplete. This successor closes
the pending affected Auth strict check mentioned in the older checkpoint below;
it does not broaden its four Reader cases or the earlier 258-case Auth receipt.
Source/Cargo returns explicitly to Root/Auth for the original Tool-source consumer;
Exact starts no competing shared executable patch before the next handoff.


**Current source successor — scoped metadata acceptance:** an eight-path apply-only handoff added
readonly exact-original-flow account-selection metadata to the existing Auth
owner/record source. Fake/PG/RecordFS readers validate protected original custody
without looking up today's selection head. The coordinated Exact K3-2 test image
compiles and Root executed4 distinct owner cases/0ignored without Cargo; see
[the exact metadata receipt](validation/2026-10-29-original-auth-flow-selection.md).
The single Auth Canceled test spelling is corrected and the failed history kept.
Final affected Auth strict/remaining combined checks stay with Exact. This is no
encrypted-generation, task-family, native-auth-wait or K3 acceptance. The scoped
258-case receipt below records its earlier exact source, not this newer
successor. Current handoff/inventory are in development-handoff.md.

[Native auth-wait lifecycle](validation/2026-10-29-native-auth-wait-lifecycle.md)
has scoped lower acceptance:258 distinct actual Rust cases/0ignored, including
real PostgreSQL reopen/resume/cancellation and filesystem record-owner/local
rejection cases. Current Turns/Composition test images, formatting and affected
Turns/LoopSupport/Reborn/Composition lib/tests strict checks pass. The native
runner port preserves original custody and resumes directly without queue or
checkpoint; cancellation failure/expiry cannot fabricate a VM acknowledgement.
An existing optional migration-test feature mismatch is repaired and qualified
both enabled (actual test) and disabled (strict checks). This is infrastructure,
not an activated ordinary native auth producer or original task-family/own-flow/
child/final-use acceptance; those remain open. The earlier pending higher
compilation/Root-harness checks passed in Prefix's scoped handoff below. Native
auth-specific workflow/terminal-answer acceptance and complete plans remain open.

[Native Prefix acceptance and actual callers](validation/2026-10-29-prefix-native-acceptance-and-callers.md)
now have scoped local acceptance:23 distinct relevant Rust cases/0ignored,
actual PostgreSQL/registry SDK/bounded tokenizer/global Monty, connected/default
strict checks and Root HTTP-harness test-target strict compilation. V182–V185 bind
immutable Pair/Profile/Generation approval and separate CAS activation to ordinary
Kohai/Sempai and post-turn review calls. Native no-submit SDK qualification catches
system-message joining; each real complete-request encoding verifies the selected
prefix bytes and leading token IDs. Canonical live model limits remain mandatory
with discretionary budgets disabled. Source originals and original reviewer wire
are immutable. The controlled model/HTTP peer is synthetic: physical Ornith/server
parity, cache qualification, ordinary Generate target-envelope production, explicit
Recipe/PrefixFree policy and full Prefix/v3 completion remain open. This supersedes
older counter-only observations for the stated native caller scope only.

[Independent obligation cleanup](validation/2026-10-29-obligation-cleanup.md)
has Host acceptance:91 actual Rust cases/0ignored, current test-image build,
formatting and strict affected lib/tests lint. Failed staged-material cleanup
no longer skips independent resource release; secondary/preparation errors remain
visible and poisoned state remains failed. The actual new fault-injection caller
checks released capacity and network cleanup. This is a prerequisite for safe
preflight waiting, not native auth continuation or K3 closure. The coordinated
combined Composition/Reborn checks below still remain pending.


[Authentication dispatch origin](validation/2026-10-29-auth-dispatch-origin.md)
now has lower caller acceptance:103 actual Rust cases/0ignored, four library and
two caller images, formatting and strict affected lower lib/tests lint pass.
CapabilityHost/HostRuntime retain preparation-versus-dispatch evidence; unknown
or dispatcher-entered auth cannot become resumable/retryable merely after login.
Existing cache, checkpoint, cancellation and custom recovery consumers are covered.
The21-path source is applied; current Composition/Reborn caller compilation and
native terminal-answer acceptance remain pending in the coordinated Prefix batch.
This is not native auth-wait, original family capture, K3 or complete-plan closure.


**29 October current source successor — partial validation:** the ordinary
credential-account resolver now consumes the protected complete requirement
selection, verifies current exact-account eligibility and captures an actual
encrypted source generation. Host account staging leases that generation
conditionally and retains the earlier issuer/store deadline. All known DTO
producers and affected fixtures are updated. The coordinated native Comp-lib
image has executed23 selector/material-producer cases, zero ignored:16 runtime
selector cases,4 existing filesystem selectors,2 new exact filesystem selection
cases and1 new direct native PostgreSQL case. Real encrypted native/record-FS
owners cover exact requirements, owner boundaries, revocation and source
replacement; these are prerequisites, not ordinary global authenticated Recipe
execution. Host validation adds90 actual cases/0ignored (10 staging unit,37
obligation and43 Host egress contract cases), including all3 new generation/
deadline staging regressions. Its affected lib/tests strict lint passes with
test-support. The113 distinct cases are scoped prerequisite evidence; the
root-harness primary compile and final connected/default strict checks remain
pending. The source successor has no complete
package acceptance yet. Original task-family/child/wait
capture, verified refresh lineage, GSuite/raw-source convergence and final-use
validation remain open. Three unbound task-port/host design files remain private
and are not part of this source. Historical acceptance below covers its recorded
source and scope; it does not certify this new successor.

[Raw OAuth refresh generation](validation/2026-10-29-oauth-refresh-generation.md)
has scoped acceptance:652 distinct actual Rust cases/0ignored, actual Chromium,
current compilation and connected/default strict checks. The two final native
PG/record-FS cases each cover six source replacement/admission/lease/consume/
acknowledgement scenarios;650 applicable positive cases are explicitly reused.
The durable action pins the authenticated encrypted input generation, actual Host
consumption is conditional, and legacy originals cannot acquire today's source
for a new dispatch. Failed fixture runs are preserved. The transport observer is
a fixture, not a live issuer. This supersedes the raw refresh-generation gap below;
general task-family/material/final-use consumers, Prefix roles, K3–K7 and complete
plans remain open.

[Google terminal setup and caller contracts](validation/2026-10-29-google-caller-contracts.md)
have scoped current-source acceptance: 113 distinct actual Rust cases, zero ignored,
actual Chromium, current image compilation and connected/default strict checks.
All 42 former caller cases remain enabled in default owning tests, with three new
terminal/elapsed cases and retained external public-facade coverage. Standalone
Google start returns original metadata without a usable link after completion,
cancellation or expiry; the Extension fixture consumes actual Product projection.
Both native Google backends and global Monty pass. This closes the terminal/caller
findings in the previous receipt below. Raw refresh generation selection, broader
custody consumers, upgrade/platform/Prefix and complete-plan gates remain open.


[Google private setup originals](validation/2026-10-28-google-private-original.md)
have scoped local acceptance:531 actual Rust tests/0ignored, actual Chromium,
fresh connected/default compilation and strict checks. Native PG/record-FS cover
one encrypted client+PKCE original, rejected first flow writes, recreated owners
with changed configuration, fresh public callbacks, exact-client refresh/rotation
and private bindings in ordinary account cleanup. Gate owners converge without a
process mutex; current metadata never substitutes historical client bytes. This
supersedes the earlier open producer/client-lineage boundary below. Standalone
terminal responses and older external caller fixtures still need alignment; that
external target was not executed by this receipt. Raw refresh-generation pinning,
broader original material consumers and upgrade/platform/Prefix gates remain open.

[Exact definition documentation delivery](validation/2026-10-28-exact-definition-update.md)
implements the bounded D01–D12 authoring/teaching corrections and open successors
in the seven architecture plans. This package leaves original sections of the five
protected plans unchanged; the Exact plan records individual dispositions for its
67-file inventory. Final Linux checks pass 29 teaching examples/132 behavior cases/
six rejected mutations; existing compiler evidence consistency also passes.
This is documentation source delivery, not Monty/Q1/Q2, installed package uptake,
K4–K7 closure or complete v3/v3.5 acceptance. Package/upgrade and future runtime
gates retain their existing owners; no executable source or activation was changed.

[OAuth setup original custody](validation/2026-10-28-oauth-setup-custody.md)
has scoped local acceptance:516 actual Rust tests/0ignored, actual Chromium,
fresh Auth/Product/Composition/worker images and strict connected/default-feature
checks. Existing Auth flow owners now persist/read the exact setup original with
authenticated encrypted-generation checks, absent-flow CAS and lost-acknowledgement
reconciliation. First/repeated callbacks retain the original snapshot and usage;
later-use markers, legacy rows and terminal states prevent unsafe reconstruction.
The native PG and record-FS cases include actual expiry and deleted bound accounts.
This supersedes the metadata foundation's open storage-port work below. Ordinary
Google private client/PKCE producers, material-source/refresh consumers and their
compatibility cutover remain open; opaque encrypted fixture bytes are not live
Google integration evidence. This is not full K3 or complete-plan acceptance.

[OAuth setup original metadata](validation/2026-10-28-oauth-setup-metadata.md)
adds the checked Auth contract foundation for first definition/snapshot/generation
and terminal projections:116 actual Auth tests, strict all-target checks and image
compilation pass. No raw private material enters these records. This is not a
connected Google persistence/callback/refresh or restart implementation; those
existing-owner consumers remain the next K3 step4 work. The older DCR acceptance
below does not qualify these newly added types or their future integration.

[DCR preparation recovery](validation/2026-10-28-dcr-preparation-recovery.md)
has scoped local acceptance:506 distinct Rust cases (187 fresh R2/319 applicable
Auth/Product results), actual Chromium and strict full-feature/default tests
checks. Missing first Auth-flow writes now recover from the exact encrypted
preparation; expiry/cancellation can retire it without a current account or new
provider registration. Later artifacts prevent reconstruction/replay. Both native
PG and record-FS cover real acknowledgement loss, original correlation and terminal
retirement. The PostgreSQL helper remains available under standard features; only
its actual filesystem branch requires skills-db. Exact feature applicability
preserves the successful full-feature image; default tests are checked fresh.
The interrupted pre-flow DCR finding below is superseded by this receipt. Durable
Google/provider originals and actual family/material/lease consumers remain open;
this is not full K3, Prefix, packaged-workflow, platform or deployment acceptance.

[Extension OAuth usage handoff](validation/2026-10-28-oauth-usage-handoff.md)
has scoped local acceptance:504 distinct Rust cases (317 fresh R6 /187 applicable
R5),45 fresh Extension JS cases, real Chromium and strict affected/default tests
checks. New Google/DCR originals retain the complete server-resolved requirement,
explicit account target and previous head. Native PG and record-FS callbacks
publish that retained usage without replay or overwriting a later selection.
Legacy originals keep their old absent-custody shape. WebUI version2 fixes the
target across reload/current-account changes. Default-feature scope/test-helper
boundaries are corrected; no warning suppression was added. The newer DCR receipt
above closes its pre-flow preparation/expiry finding. Durable Google PKCE,
generic Google originals and actual task-family/material/lease consumers remain
open. This is not K3/full-product,
live-issuer, platform or deployment acceptance.

**Skill/Recipe contract clarification — 2026-10-28:** a complete reusable Skill
needs no dedicated Recipe or command. Usage-combination approval, actual consuming/
test workflow qualification and optional MCP publication are distinct. Existing
revision/association readers have no required Recipe backlink; deployment still
publishes complete Recipe graphs and source export projects selected closures.
This documentation/seed-prose correction does not implement independent activation
of unused library components, expand export coverage or certify deployment. Existing
MCP discovery is opt-in via immutable Recipe `mcp_call`; malformed declarations and
stale pre-capture commands remain errors. Changed packaged text requires successor
source/integrity qualification; old task selections and historical receipts remain.
See [the owning Skill contract](../skills.md#independent-usage-qualification-and-deployment).
Local source checks for this correction: 43 actual `brassclaw_skills` tests, strict
affected-package Clippy, scoped Rust formatting, both development-skill validators,
introduced file links and changed-file whitespace pass. These checks do not qualify
new installed bootstrap bytes, production Prefix uptake or deployment/recovery.

[Google Extension OAuth original identity](validation/2026-10-28-google-original-identity.md)
has scoped acceptance: 151 distinct Rust cases (86 fresh R3 / 65 applicable earlier
cases) and 43 fresh Extension JS cases, strict affected/default checks. Google now
retains browser original IDs/expiry/state/PKCE through Start/Recover/Cancel and
uses the shared controller. Native insert-acknowledgement loss and concurrent
creators preserve the accepted verifier. The newer usage-handoff record above
supersedes that receipt's open setup/target/head and scoped-controller browser
findings. Durable Google PKCE, generic originals and actual family/material
consumers remain open; neither receipt completes K3 or full browser/deployment support.


[Extension requirement handoff](validation/2026-10-27-extension-requirement-handoff.md)
has scoped R4 acceptance:365 distinct cases (339 Rust/26 JS), strict tests/default
checks and exact unchanged-source/image evidence reuse. Full content-based usage
identities survive declaration order changes. Manual originals retain the exact
requirement across metadata recovery and current-manifest removal; readiness reads
completed selection custody and current exact account eligibility. The newer OAuth
successor above accepts its setup/callback usage binding. Actual original-family/
material/runtime consumers remain open. This is
not dispatch, browser, deployment or full K3–K7 acceptance.

[Auth fake OAuth/account-choice publication](validation/2026-10-27-fake-other-requirement-publication.md)
has scoped R2 acceptance:100 fresh cases, zero ignored, all-target strict lint.
Manual/OAuth/choice share one pure original/head precheck before mutex publication;
stale heads cannot leak account/completion/continuation changes, and completed
originals survive later choices/revocation. This is test-service fidelity only.
Deliberate sharing and actual original-family/material consumers remain open.

[Filesystem OAuth/account-choice publication](validation/2026-10-27-fs-other-requirement-publication.md)
has scoped acceptance:126 fresh cases (FS82/PG44), zero ignored, strict tests/default
checks. One existing head owner stages exact originals, publishes via CAS and
completes last. Cancellation cannot acknowledge an outstanding chosen publication;
OAuth head conflicts retain actual account effects with an explicit failed flow.
Old decisions do not backfill bindings. Non-manual fake fidelity, deliberate sharing
and actual original-family/material consumers remain open; K3–K7 are not complete.

[Native OAuth/account-choice publication](validation/2026-10-27-native-other-requirement-publication.md)
has scoped R2 acceptance:122 fresh cases (PG44/FS78),98 exact-reused Auth cases,
zero ignored, strict affected/default checks. Exact requirement heads commit with
account/original flow custody; competing/lost-COMMIT/rejected-final-write cases
preserve originals without replay. FS/fake non-manual publishers, deliberate
sharing and actual family/material consumers remain open. This is not full K3
or complete-plan acceptance.

[Auth fake manual-selection transitions](validation/2026-10-27-fake-requirement-publication.md)
have scoped acceptance:98 fresh Auth cases, zero ignored, strict all-target lint
and actual public contract cases. The existing mutex commits only after all
account/flow/head checks; stale choices leave accounts and continuations unchanged,
while original recovery retains selected metadata after later choices. This is
fake fidelity, not production credential-family acceptance. Native/FS non-manual
publishers, sharing and actual family/material consumers remain open.

[Prefix model-policy original binding](validation/2026-10-27-prefix-model-policy.md)
has the Prefix owner's scoped native3/shape2/operator-Pair2 acceptance on the
shared fresh R1 image. Historical recorded facts are checked against their exact
provider revision; missing historical limits remain Unknown. This does not supply
accepted designs, generation/serving or final request guards.

[Filesystem requirement publication](validation/2026-10-27-fs-requirement-publication.md)
has scoped acceptance:119 fresh Auth passes (FS78/PG41), strict affected/default
checks and actual encrypted Memory/PostgreSQL callers. The CAS-only owner stages
original custody, publishes the exact head and completes the flow last; incomplete
selections stay unusable, original recovery preserves later choices and a losing
publisher retains actual partial effects with an explicit failed disposition.
Versioned `/1` results remain readable. OAuth/account-choice/sharing/family
consumers and complete K3–K7 remain open; this is not whole-product acceptance.

[HTTP sensitive-save repair](validation/2026-10-27-http-sensitive-save.md) has
scoped acceptance:266 fresh passes, strict affected/default lints and native
memory/PostgreSQL callers. Sensitive visible/resolved and actual local targets/
parents reject before network and again before atomic opaque writes. Exact
write-only safe-file saves remain supported; mid-network changes preserve the
record/file and usage. Three existing Docker cases remain unexecuted. This is
not malicious local-FS race protection, platform acceptance or full K3 completion.

**Prompt contract — operator clarification confirmed 28 October 2026:** the
[four binding rules](reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
require a finished immutable prefix per provider/model pair and one saved accepted
design revision for generator, Kohai and same-pair Sempai. Kohai formats the dynamic
Orchestrator prompt first; optional connected/enabled Sempai corrects it with its
own generated reviewer prefix/target-pair design and returns the final dynamic
prompt. Kohai then inserts the intended prefix unchanged exactly once, without
formatting the return again. Documentary alignment does not establish runtime
serving acceptance; the Prefix plan retains that implementation/qualification work.
The [explicit handoff acceptance](reborn/model-token-limits.md#explicit-handoff-between-the-two-model-requests)
now distinguishes the original, formatted and returned final dynamic values.
The final join uses the returned value with review enabled and the initially
formatted value otherwise; Sempai's own prefix stays in its separate review call.

[Native manual requirement publication](validation/2026-10-27-native-requirement-publication.md)
has scoped acceptance:116 fresh passing Auth cases (PG41/FS75), actual native
COMMIT-loss/concurrency/rejection and encrypted legacy-original recovery, plus
strict tests/default checks. Native requirement heads commit with existing
credential/account/flow custody; original recovery cannot overwrite a later head.
This is a prerequisite only. Other OAuth/account-choice publishers, explicit
sharing and original task-family/material consumers remain open in K3; the FS
regression does not certify its new publication path.

[Root host/HTTP integration](validation/2026-10-27-root-host-handler-integration.md)
has scoped R6 acceptance:305 distinct passing cases on one frozen code tree,
including all23 Root programs/36 cases and four actual local/native settings and
record-boundary callers.301 executions are exactly qualified from R5, four are
fresh R6 executions. Strict affected/default checks pass. Three existing Docker
cases remain unexecuted. The four file/HTTP matrix failures are resolved; local
host-only defaults do not bypass PostgreSQL's acknowledged global settings.
Actual HTTP saves preserve structured records atomically. Persistent requirement/
account publication, family capture, K3–K7 and the complete plans remain open.

[Exact credential requirements and original setup custody](validation/2026-10-27-k3-requirement-custody.md)
have scoped R4 acceptance:682 distinct fresh Root Rust passes, strict affected/
default checks, real PostgreSQL/record-filesystem owners and native HTTP/Chromium.
The full usage reaches authorization/resolution/errors and separate exact gate
identity; first setup writes retain it and retries reject reinterpretation.
The unchanged architecture guard now accepts the browser test's physical test
placement. Three existing Docker cases remain unexecuted. Persistent requirement/
account publication, deliberate sharing, authenticated original material/family
selection and selected-generation issuance remain open in K3 step4. Metadata/read
seams are not grants; preserve this precursor instead of rebuilding it. The Prefix
owner's six Rust/three UI/one offline checks have separate scoped acceptance on
the same emitted images, not model-design activation or production serving.

[Conditional filesystem writes and internal result evidence](validation/2026-10-27-k3-conditional-writes.md)
have scoped R5 acceptance:291 applicable consumer passes through exact emitted
image reuse, three freshly executed native Prefix cases, strict affected-package
test lint, default-feature library lint and format checks. Six filesystem consumers preserve original CAS;
projection adaptation retains the same condition, unsupported local version
updates preserve originals and process creation uses atomic Absent. A trusted
internal result owner can require existing bounded step observations; PairCases
uses that support and now settles actual successful/mismatching/invalid-input
outcomes without replay. R1–R4 failures remain recorded. This closes those K3
step6 prerequisites, not requirement/account/family binding, other caps/protocols,
K3–K7, full plans, production Prefix serving or platform acceptance.

[Ambiguous credential-selection correction](validation/2026-10-26-k3-credential-selection.md)
has109 fresh passing Rust cases, strict test lint/build and actual native PG/
Chromium. Runtime no longer chooses same-provider accounts by recency/labels;
readiness preserves ambiguity. OAuth filters exact update scope before selection
and returns backend/ambiguity errors instead of silently admitting a new original.
Both bare-PKCE rejection and cache-miss retrieval from a real admitted DCR original
pass without post-setup HTTP. This is a necessary K3 step4 correction only:
persistent requirement/account binding, deliberate sharing and original task-family
grants remain open. No broader platform/global/serving or full-plan claim follows.

[Issuer-expiry acceptance](validation/2026-10-26-k3-issuer-expiry.md) now records
738 distinct fresh passing Rust cases, strict six-package test lint/build and
actual native PostgreSQL/Chromium for the captured 67-input package. Conservative
checked issuer deadlines survive callback/refresh, original-result recovery,
account projection/selection, authenticated Secret generations and final runtime
staging/cache/restore/injection. Unknown/zero/overflow and delayed acknowledgements
are qualified; expiry does not mutate raw account identity. The actual callback
new-account publisher now retains the original deadline after failed publication
and restart without another issuer exchange. Three existing Docker cases remain
unexecuted. Earlier open issuer-expiry statements below are historical for these
scoped consumers; complete original task-family grants, requirement-specific
selection, source reclamation, Codex sessions, caps, K3–K7 and the full plans stay
open. No global/platform/deployment or Prefix serving acceptance follows.

[Selected-generation Secret lease acceptance](validation/2026-10-26-k3-secret-lease-generation.md)
now covers authenticated PG/FS generation capture/consume, old unproven-lease
rejection, full FS lease-state proofs and version/body CAS through restored
revocation.395 distinct Rust cases, strict affected test lint and real
Chromium/native Auth HTTP pass for the captured seven-source package. The legacy
memory UUID/usage guard is exercised without claiming its split read releases
replacement material. A historical DELETE-fault fixture now tests actual retained
cancellation UPDATE and full original/revision/idempotence through PostgreSQL.
Three existing Docker cases remain unexecuted; frozen-image reuse is explicit.
Issuer expiry through final injection, original task-family grants and K3–K7/full
plans remain open. This closes the selected-generation prerequisite only.

[Original DCR/lifecycle and native filesystem acceptance](validation/2026-10-26-k3-dcr-originals.md)
now qualifies the common integrated Auth/DCR package:342 distinct Rust cases
(281 exact current-executable reuses and61 fresh connected cases), strict affected
test Clippy and actual Chromium/native Auth/Secret HTTP acceptance. Original
registration/cleanup custody, legacy retained inputs, exact local retirement,
stable lifecycle owner selection and complete large owner enumeration are covered.
Concurrent PostgreSQL index initialization is repaired transactionally; physical
index identities no longer alias sanitized prefixes. Browser reload/retry/cleanup
readback keeps the original IDs/first expiry with no repeated POST/DELETE. Issuer
egress is controlled and the popup-blocked component scenario is explicit; this
does not qualify the full Extensions page, live issuer, global startup or later
concurrent Prefix changes. Account-lifetime source reclamation, expiry/final use,
dedicated grant/session work, caps and K3–K7/full plans remain open. This supersedes
older private-only or blanket unimplemented DCR/retirement/lifecycle descriptions
only for the report's exact scopes.

[Elapsed-expiry and protected Extension Chromium](validation/2026-10-26-k3-extension-browser-expiry.md)
now qualify the remaining manual-transport checks: four actual-time Native/FS/Fake
cases, including missing FS projection repair after expiry and no secret/account
publication, plus the actual ConfigureModal/native PostgreSQL/protected Chromium
lost-ack/reload/changed-raw/reference/cancel path.47 common Node cases and strict
affected lint/build remain applicable under exact source/image checks; the browser
completes in the corrected sibling package layout without recompilation.
Requirement-specific account selection, DCR custody, issuer expiry/final use and
remaining K3/full-plan/platform/release acceptance remain open.

[Extension original credential handoff](validation/2026-10-26-k3-extension-originals.md)
now has scoped Product/Auth/protected native HTTP acceptance:175 fresh Rust cases,
46 applicable unchanged common Node cases and strict affected/default/Postgres/
format checks. Canonical Web invocation, per-requirement original pair, explicit
initial target, retained binding/first expiry, dedicated raw/reference/cancel and
redacted reload/context fences reach the actual caller. Generic lifecycle payloads
reject secrets; backend status failures remain visible. The elapsed-expiry and
real Chromium gaps have the scoped successor above; requirement-specific account
readiness/selection remains open. This does not close K3 or broaden Prefix serving.

[Manual HTTP/Chat and runtime Auth continuation](validation/2026-10-26-k3-manual-http-browser.md)
now have scoped acceptance:140 fresh Rust executions,34 applicable unchanged
common Node cases, protected real Chromium/PG/Auth/Turn continuation and strict
affected/PostgreSQL/default/fmt checks. All27 author files, captured source
membership and six actual images are independently verified; all20 accepted
Prefix pair-case authors remain unchanged. Original pair/invocation, first expiry,
raw/reference separation, terminal handling and late-response fences reach the
actual browser. Runtime binds the existing Auth facade to its final coordinator,
repairing successful credential publication followed by run-not-found; independent
native replay and all factory Auth tests pass. Old normal/Notion fixture digests
were corrected with real hash helpers without weakening production validation.
The browser uses an explicit stopped-worker state fixture; this is not proof of
a Recipe requesting credentials or executing after auth. Extension per-requirement
transport/binding, DCR, final use/expiry/caps and full K3–K7 remain open. This entry
supersedes older claims that the manual HTTP/Chat migration is still missing.

[Original manual setup owner/facade](validation/2026-10-26-k3-original-manual-setup-owner.md)
has scoped acceptance:281 freshly executed Rust cases including seven new real
native/Memory/PG-backed FS/public cases, strict full/PostgreSQL/default/fmt checks.
Original pair IDs, first expiry, exact scope/definition and actual terminal state
are retained. Strong raw checks the pair before effects; explicit reference
continuation requires confirmed original publication and does not select today's
account. This supersedes older statements that those backend/facade operations
are missing. Public HTTP/browser/extension identity and reference migration,
DCR/final grants/expiry, cap uptake and full K3–K7 remain open. The connected
extension path still trims raw input and reselects current accounts on retry;
its product port/controller must use the same original owner before K3 closes.


[Native manual cancellation/build prerequisite](validation/2026-10-26-k3-manual-cancel-disposition.md)
has scoped acceptance:274 fresh Rust cases, including three new real native
completed/canceled/lost-COMMIT-reply cases, actual public/global callers and strict
full/PostgreSQL/default/fmt. Consumed proof remains; pending cancellation has a
persistent timestamp and original body/version transaction. Completed is not
acknowledged as canceled. Fixed build stubs declare their input and preserve
identical output mtimes, with actual content/error probes. This supersedes the
native abandonment finding below; original setup/browser/reference-only/DCR/final
use and K3–K7/full plans remain open.


[Original-proof raw manual facade](validation/2026-10-26-k3-manual-facade-original.md)
has scoped acceptance:271 distinct Rust cases (107 fresh final and164 individually
qualified unchanged R1 results), strict affected/repaired/PostgreSQL/default/fmt
checks. Raw acceptance never substitutes completed metadata/current account.
The test adapter retains exact original result/input, rejects changed tokens and
captures prechallenge update snapshots; unbound creates a new account. Generic
abandon status is corrected. Native cancellation disposition, first HTTP/browser
setup identity/expiry, explicit reference-only continuation, DCR/final grants and
full K3–K7 remain open. This supersedes older blanket raw-facade/fake observations
below, not the remaining workflow or platform/release gates.


[Ordinary FS manual publication](validation/2026-10-26-k3-fs-manual-publication.md)
has scoped acceptance:308 fresh Rust cases, including12 new real encrypted
memory/native-PG write-boundary/race/restart/facade cases,53 other durable cases,
current public/global consumers and strict full/PostgreSQL/default/fmt checks.
Interaction decision CAS and same-account receipts retain original input/selection/
result without unknown-write cleanup or account reselection. Prechallenge bound
snapshots and unbound new-account semantics agree with native. Lower interaction
semantics remain distinct. Generic/fake/raw-facade/browser setup/reference-only
continuation, final grants/expiry and full K3–K7 remain open. This supersedes older
blanket statements that ordinary FS manual publication is unimplemented; it does
not close the remaining manual workflow or production/platform acceptance.


[AuthProduct original-entry snapshot acceptance](validation/2026-10-26-k3-auth-record-snapshots.md)
adds296 freshly executed Rust cases, including five real Memory/PostgreSQL/Local
recreate and legacy-decision cases, current strict profiles and formatting. Every
conditional FS AuthProduct record writer carries original version/body identity
to the existing atomic backend check; callback decisions retain account identity
and exact receipts. Unproven historical decisions stay preserved/conflicting for
controlled upgrade rather than publishing weakly. This closes that K3 step2
write-snapshot prerequisite. Full FS/generic manual publication, browser identity/
raw-result custody, final grants, complete upgrade recovery and K3–K7 stay open.

[Retained format-source observations](validation/2026-10-26-prefix-format-sources.md)
have the Prefix owner's scoped acceptance: eight Rust cases (including real
PostgreSQL with frozen original model files), six UI cases and current strict/
default/format checks. Complete template/tokenizer sources, alternatives and
declared dtype/token facts are retained/readable; this does not select a default,
execute a template, accept construction rules or qualify serving. Their existing
artifact guide is an intentional successor to the earlier20-document snapshot.

[Native manual original/atomic publication acceptance](validation/2026-10-26-k3-native-manual-publication.md)
adds 283 freshly executed Rust cases, including nine real native failures/races/
restart/facade cases, strict full/default/PostgreSQL-only checks and current
formatting. The ordinary native high-level flow commits encrypted original and
retained output, exact account, consumed interaction and completed flow together;
same-input recovery never selects the latest account and changed input conflicts.
The lower interaction port retains its separate non-flow-completing contract.
Generic/FS publication and legacy facade recovery, browser original setup/input,
final credential-use fencing and K3–K7 remain open. This supersedes earlier
blanket statements that native manual publication has not been connected; it
does not establish full manual-product or plan completion.

[Configured inactive model-target qualification](validation/2026-10-26-prefix-configured-model-targets.md)
covers the Prefix owner's unchanged five-file metadata/UI package: one real native
HTTP/PG/global/encoding case qualified from exact unchanged R3 source/artifacts,
six fresh UI cases and current default/fmt checks. Its canonical provider/model
limits and read-only capture do not activate roles. The current20-doc contract
package is captured too; accepted model designs, fitted content and final serving
remain open. Oversized source references retain fit=false and their real warning.

[Concrete native AuthProduct owner acceptance](validation/2026-10-26-k3-native-auth-owner.md)
adds 209 freshly executed Rust cases with strict full/default/PostgreSQL-only
checks. The native AuthProduct retains the factory's same concrete secret owner
independently of RootLLM; connected feature consumers and indexed backend guides
are corrected without suppressions. This enables the existing transaction ports;
atomic manual original-input/account/interaction/flow/result/browser publication
is still open in the existing K3 step2.

[Fixed-prefix/design ownership source audit](validation/2026-10-26-prefix-owner-contract-audit.md)
records the corrected required sequence: Kohai first formats only the dynamic
Orchestrator prompt using the same saved accepted target-pair design revision as
its prefix generator; optional connected Sempai corrects that formatted prompt
using its own generated reviewer prefix/design and returns the final dynamic
prompt; Kohai then inserts the intended finished prefix unchanged exactly once
before it, without reformatting the return, and checks the full request. Reviewer calls retain their own
design/prefix. The Prefix plan has been corrected accordingly. Current design
proposal storage, retained inline catalogue context and protected interceptor
recomposition do **not** implement or certify that complete serving pipeline.
The explicit caller migration remains owned by Prefix steps6/7 and§4.1/§5.3;
this is source analysis, not a new runtime/activation acceptance.

[Immutable secret custody acceptance](validation/2026-10-26-k3-secret-custody.md) adds348 fresh positive Rust cases with current strict/default/format checks. All actual owners/forwarders retain encrypted first inputs, issue immutable outputs only from exact original references and preserve retired identities; normal writers and conditional deletion cannot repurpose them. Native PG and persistent FS races/COMMIT-loss/read-only/tamper/owner/lock-order and existing public/global consumers pass. Full manual/browser/account/flow/interaction publication, ordinary mutable-generation lease fencing and K3–K7 remain open. This supersedes earlier descriptions of the storage foundation as private; its new ports are not yet the full manual product cutover.

**2026-10-24 conformity priority:** K0 baseline, [K1 exact prefix request identity](validation/2026-10-24-k1-prefix-request-identity.md) and [K2 atomic catalogue publication/invalidation](validation/2026-10-24-k2-catalogue-publication.md) are complete for their stated scopes. K2 closes with 39 fresh and nine qualified reused Rust results, 21 current UI cases, strict affected/default lints and final identities. K3–K7 remain open and precede regular plan continuation. General Pure-Q1 work remains deferred under this priority.

[K3 original manual provider publication](validation/2026-10-24-k3-provider-originals.md) has scoped acceptance: 57 Rust tests (38 fresh executions plus 19 exact reused results), 26 UI cases and strict affected/default lints. Atomic original receipts, encrypted request proof and reference-only read/reconcile are implemented. Exact client revision acknowledgement, desired/effective status and retained/cache/retry live fencing remain open; this does not close K3. [Captured client publication checks](validation/2026-10-24-k3-provider-publication.md) additionally record 67 passing Rust tests and exact primary/Sempai metadata, for that exact historical source. [Retained provider consumer acceptance](validation/2026-10-24-k3-provider-consumers.md) now covers the atomic Noop correction, async preparation, PostgreSQL/cache/retry compatibility checks and last-published UI: 123 actual Rust cases, 43 UI cases and strict affected/default checks. [Retained embedding consumer acceptance](validation/2026-10-24-k3-embedding-consumers.md) additionally covers 120 actual Rust cases and 43 UI cases: encrypted-store-only source credentials, live retained/cache/vector-contract checks, permanent cache retirement, immutable guarded model selection and UI error/loading refresh. [Native submission-fence acceptance](validation/2026-10-24-k3-submission-fence.md) additionally covers 55 executed Rust cases, strict lints and the actual original-claim cancellation barrier at the OpenAI Chat Completions transport. [Actual embedding publication acceptance](validation/2026-10-24-k3-embedding-publication.md) additionally covers 56 executed Rust cases, strict lints, same-source publication in the settings map and independent role uptake/reconciliation. [Displayed role-selection acceptance](validation/2026-10-24-k3-role-selection.md) now covers 63 Rust cases (57 fresh, six qualified reused), 43 applicable UI cases, real PostgreSQL concurrency/deactivation, actual primary/Sempai Ornith clients and the full HTTP/Prefix consumer. Ordinary role API CAS and canonical target cleanup are implemented. [Consistent desired/actual acceptance](validation/2026-10-24-k3-desired-publication.md) additionally covers 104 freshly executed Rust cases, 45 UI cases and strict lints: one read-only snapshot for desired definitions/roles/authenticated encrypted-key references, actual-client comparison, visible pending/unconfirmed/invalid states and repairable external selections. [Exact failed-target acceptance](validation/2026-10-24-k3-failed-publication.md) additionally covers 62 freshly executed Rust cases, 47 UI cases and strict lints: effective/failure metadata shares each existing publication owner, failed targets stay revision-bound across later edits and successful repair clears them atomically. [Task-selected model acceptance](validation/2026-10-24-k3-task-model-selection.md) now covers 78 freshly executed Rust cases, strict affected/root/default lints, actual native host/global/Child callers, V179 immutable admission and original Kohai/Sempai targets across edits and waits. Inherited model absence is explicit; side-effect-free metadata capture preserves real pre-submission cancellation acknowledgement. [Optional startup/current Prefix-input acceptance](validation/2026-10-24-k3-optional-startup-prefix.md) adds 48 fresh Rust cases, one exact reused startup case, 17 Python cases and strict root/Composition/Skills/default checks. Recoverable invalid role/credential/SDK startup preserves rejecting clients, exact failure status, ordinary deterministic routing and WebUI repair without a new VM. Snapshot/database errors remain startup failures. Peer Prefix derivation is a projection building block, not accepted Generate/serving. [Task-family embedding acceptance](validation/2026-10-25-k3-task-embedding.md) adds117 actual Rust results (final32 fresh architecture checks and85 qualified runtime/core results), strict current default/root lints and exact source applicability. Generic and retained native Memory entry points keep original provider/key/space across edits; task/private cache release, exact shared-cache survival, scope/cancellation/lifetime and unchanged full HTTP inputs are covered. [Codex SDK acceptance](validation/2026-10-25-k3-codex-sdk.md) adds55 freshly executed Rust cases and strict LLM/Composition/product checks: noninteractive static construction, no model replay for host auth repair, checked expiry, retained refresh credentials, private errors and redirect rejection. Return-only authorization exists; the actual WebUI controller and encrypted-session handoff are not yet migrated. [Original NEAR browser publication acceptance](validation/2026-10-25-k3-nearai-browser-publication.md) adds78 fresh Rust and19 UI cases with strict affected/product/default checks: atomic encrypted key/role/receipt, actual public callback and COMMIT-loss readback, stale-context rejection, shared settings owner and exact UI receipt/late-refresh fencing. It covers the existing browser route. [Original NEAR wallet publication acceptance](validation/2026-10-25-k3-nearai-wallet-publication.md) now covers113 distinct Rust cases (five final executions and108 qualified unchanged results),19 JS and five Python cases, with current strict checks: server one-use challenge, exact original context/proof, one exchange with selected HTTP policy, atomic encrypted key/role/receipt, actual model credential, rebuild readback and public API identity. No signer UI or real issuer is claimed. [Same-provider login model preservation](validation/2026-10-25-k3-login-model-preservation.md) now has18 fresh native/service/global cases and current strict checks: both login channels keep the explicit same-provider model, exclude a foreign model and prove the actual outgoing SDK model without extra DB reads or breaking old receipt proof. [Refresh artifact/client-binding acceptance](validation/2026-10-25-k3-oauth-refresh-artifacts.md) adds26 actual Rust results with exact syntax-only evidence reuse and current strict checks: completion-specific encrypted token destinations, original DCR registration handoff, omitted rotation and prepublication failure cleanup with a real DELETE rejection. [Final account snapshot-fence acceptance](validation/2026-10-25-k3-oauth-refresh-cas.md) adds96 fresh Rust cases and current strict checks: all three actual wrapper writes use existing atomic PG/filesystem metadata CAS; independent writers, final revocation and retained originals are covered. [Existing PG Auth boundary acceptance](validation/2026-10-25-k3-pg-auth-boundaries.md) adds34 fresh Rust cases and current strict checks: exact scope vs stable owner separation, preserved sessions, absent inserts, fenced interaction deletion, matching manual cancellation, propagated abandon failures and shared state/selection parity. Original refresh and OAuth callback custody have their separate bounded acceptance below; manual-token success, issuer expiry and complete artifact disposition remain open. Codex controller/rotating sessions, signer UI and full dedicated setup remain open. Dedicated auth/login/session and other protocol submission boundaries, applicable integrity/recovery support limits and K4–K7 remain open. Ordinary generic WebUI config excludes model-role keys; the standalone importer's active-instance fencing belongs to K6.

[Shared HTTP single-submission acceptance](validation/2026-10-25-k3-network-no-replay.md)
adds49 fresh Rust cases and current strict checks. The actual transport disables
Reqwest protocol-NACK resends; a native full-body HTTP/2 fixture includes the
default-resend positive control. Existing Network contracts and the ordinary
global packaged-content caller pass. Manual Auth custody, remaining protocol
submission boundaries and K3–K7 remain open.

[Original refresh custody acceptance](validation/2026-10-25-k3-auth-refresh-custody.md)
adds283 distinct Rust cases:83 fresh current Auth plus200 exactly qualified native/
consumer results, with all current strict checks. Existing PG/persistent FS owners
retain original intent/actual destinations before issuer and exact returned result
before final account/receipt publication. Original-only recovery, rejected writes,
lost acknowledgements, changed requests, cancellation, unknown/stale/foreign results,
public DTO and exact GSuite receipt/snapshot guards are covered within that report's
actual backend/transport limits. Manual-token success, expiry/final injection, complete artifact disposition,
DB-owned auth setup/original grant, Codex sessions and remaining K3–K7 stay open.

> **Priority changed — 2026-10-24:** complete `simplified_v3.md`'s opening
> conformity block K0–K7 before ordinary remaining-plan work. The K0 baseline is
> recorded in [the existing implementation journal](plans/simplified-v3-implementation.md).
> K1/K2 repairs have scoped acceptance; K3 and K4–K6 evidence/retention checks remain open. Earlier scoped
> acceptance is preserved. The already started private pure-source test batch may
> finish, but general pure approval/runtime work does not take priority over K1–K7.

> **Resumed by the operator — 2026-10-24:** [the restart handoff](development-handoff.md)
> records the preserved interruption and its completed successor. Source extraction
> now has scoped acceptance: five applicable earlier tests plus ten completed
> successor tests, strict affected/root lint and formatting. Full class Q1 and
> trusted durable review/approval remain open.

Reconciled on **2026-10-24** against the common `main` working tree. Consolidation
started from `580a7f73e`; the raw-source checkpoint is `c52bdc2e0`. This index
includes the successors through `747bcfff6`: pure-review prerequisites,
independent execution coverage, captured observation limits, original-subject/input
custody and retained global interpreter hosting, plus the subsequently validated
reusable preload-definition rule, complete native source-inventory primitive
and its canonical usage drafts
described below.
It describes the source delivered with this document.
It supersedes older present-tense implementation observations in the guides;
their binding component, authority and failure contracts remain authoritative.
The dated implementation journals retain historical evidence, including failed
attempts. Neither a journal entry nor this index establishes deployed support.

[Original refresh readback acceptance](validation/2026-10-25-k3-refresh-readback.md)
adds273 fresh Rust cases and current strict checks: missing/Submitted/final
originals avoid publication locks at the coordinator and PG/CAS-filesystem owners;
real held account/action rows, actual pending lock blocking and independent
finalizers preserve one account publication. Manual-token custody, complete
artifact disposition, expiry, dedicated setup/session and K3–K7 stay open.

[Original OAuth callback custody/PKCE acceptance](validation/2026-10-25-k3-auth-callback-custody.md)
adds 334 distinct passing Rust cases: 201 fresh at run5, 133 independent reused,
all qualified for the final equivalent Result-combinator correction, plus current
strict/default/fmt/integrity checks. Original request/prepared destinations,
durable outcome before PG atomic or CAS decision/account receipt publication,
restart/cancel/stale/unusable outcomes and actual Host/public/global consumers
are covered. Error cleanup authenticates terminal flow ownership before PKCE
eviction; malformed/foreign pending requests preserve the legitimate callback.
Manual success custody, partial-artifact disposition and remaining K3–K7 stay open.

[Manual setup original-flow acceptance](validation/2026-10-25-k3-manual-setup-custody.md)
adds 337 distinct passing Rust cases (204 fresh/133 exact independent reused) and
current strict/default/fmt/integrity checks, without an intermediate failure.
The known interaction's Flow ID is selected before writing and only its full
original definition is read after uncertain creation. Missing/unavailable reads
retain the interaction; foreign/canceled/expired state cannot become a waiting
challenge. Native PG and generic/filesystem callers share the same helper.
Full manual input/publication and HTTP failure/timeout custody remain K3 work.

## Common source and validation

The authoritative source is the repository root and its single Git index on
`main`. The prefix and runtime workstreams are combined there. Former mutable
validation/build mirrors are retired after their exact inventories are checked
against retained evidence. Historical immutable sources, binaries and receipts
are evidence archives, not alternative development trees or Cargo build inputs.

On the current development host Codex configuration supplies `CARGO_TARGET_DIR`
as `/Users/ollama/brassclaw-target`. Invoke Cargo directly, with serial executions
from the repository root and no inline target assignment or explicit shell wrapper.
`brassclaw-target2` is retired. Preserve relevant
executable evidence before cleanup and follow the disk rules in
[the development policy](development-policy.md) and [AGENTS.md](../AGENTS.md).
Other platforms use one appropriate local target directory; the macOS path is
not a product setting or a required installation path.

[The final reconciliation record](validation/2026-10-23-documentation-reconciliation.md)
maps these successor checkpoints and explains the canonical plan choices.
Its final repository audit confirms one registered worktree, both historical
local work branches fully contained in `main`, and the retired second target
absent. Historical branches and immutable evidence are not additional active
source or build trees. The audit does not imply that unrelated GitHub branches
have been merged or deleted.

The consolidation validation result is recorded in
[the validation record](validation/2026-10-22-consolidation.md). The subsequent
[pure-review prerequisite record](validation/2026-10-23-pure-review-foundation.md)
covers 67 focused tests, actual PostgreSQL acknowledgement cuts, global child
execution, unchanged architecture rules and strict affected/root lint. Older frozen
receipts establish their named scopes only. No full release/platform matrix,
fresh remote installation, general crash recovery or full-plan completion is
inferred from focused local checks.

[Validator observation acceptance](validation/2026-10-23-validator-observation-limits.md)
additionally covers seven focused actual-worker/PG consumer tests, including
complete 300,000-item negative diagnostics under captured per-step limits after
later live reductions. Independent report bounds and changed-result rejection
remain enforced. This is not successful class Q1, pure approval or activation.

[Original-subject/input acceptance](validation/2026-10-23-validator-subject-inputs.md)
covers 29 focused contract, actual-worker/PG and ordinary-global cases. A distinct
sealed observer now verifies the original stored submission/validator selection
and every actual input edge under captured limits; identical outputs from changed
subject data cannot be relabelled as its observations. Shared borrowed fingerprints
preserve historical bytes and aggregate limits without cloning result trees.
Trusted Q1 writers, approved mandatory rules, interpreter/run/attempt/settlement
correlation and general pure activation remain open.

[Retained global-worker acceptance](validation/2026-10-23-retained-global-worker.md)
covers 16 fresh ordinary/PG/global and separate-workspace lifecycle cases.
Source-Inspection, catalogue preparation and ordinary global hosting now share
the existing private worker image, retained with unknown singleton ownership.
Actual start after removal of a temporary installation source, image lifetime,
policy/limits, activation/restart, child selection and cancellation/settlement
callers pass, with strict lint/format in both workspaces. This repairs live hosting
identity; durable interpreter/Q1 records and generic crash recovery remain open.

[Preload-definition checking acceptance](validation/2026-10-23-validator-preload-definition.md)
covers nine fresh actual-worker/PG/ordinary-global and guide checks, plus strict
affected/root lint. A reusable pure rule compares native definition facts with
declared signatures/symbols/constants/imports. Installation retains its exact
unreviewed draft without replacing authored successors or active selections.
Original-subject/native-fact custody, complete class checks and durable review
scheduling/evidence remain open; a positive definition result is not Q1.

[Complete native source-inventory acceptance](validation/2026-10-23-source-inventory.md)
covers 25 final-source actual-worker/PG/global/offline-model and standalone cases,
with strict affected and separate-workspace lint. The existing source Tool now
inspects a complete map under one process/deadline and aggregate limits, returning
exact source/checksum/compiler facts with no candidate execution or partial success.
Both profiles retain live policy/settings and the shared image. Utility protocol
is 5; global protocol stays 20. Canonical inventory usage qualification, full subject/graph checks
and complete validator scheduling/evidence/activation remain open.

[Canonical inventory-usage acceptance](validation/2026-10-24-source-inventory-usage.md)
adds 14 fresh scoped tests and strict affected/root lint. Ordinary startup retains
the exact native Tool and both usage drafts with one shared ToolSkill, preserving
operator successors and live blocks. A pure decoder rejects duplicate or malformed
ID/source pairs before the native call; the canonical command Recipe posts the
unchanged report through the existing reply usage. Actual closed global-flow and
PG journal observations cover completion and failure. This is not public command/
MCP activation, complete class Q1 or original-subject/native-fact custody.

[Sealed-subject source-extraction acceptance](validation/2026-10-24-subject-source-inventory.md)
adds the pure `extract_review_python_sources` draft, retained at startup without
approval or activation. All class-22 candidate/dependency sources, including empty
text, reach the existing inventory usage as typed data; the comparison base and
non-Python metadata are excluded from that parser input. Actual closed Python-flow,
native hashes, PG arguments and captured input/result fingerprints cover original
selection, operator successors and six pre-effect failures. The 15-test scope
reuses five completed unchanged-source cases and completes ten after the deliberate
pause. The interruption is preserved. Mandatory rules, complete private class
routes and trusted run/attempt/settlement/evidence writers remain required.

[Recursive-contract rule acceptance](validation/2026-10-24-recursive-contract.md)
adds a retained pure checker for the current input/Tool-parameter/result grammar,
recursive defaults and exact numeric constraints. Actual Monty verdicts match
native readers across more than 140 cases; operator successors remain intact.
The 15-test scope retains 13 results after a verified unembedded policy change
and completes two remaining tests plus strict lint/format. This is grammar
coverage, not resource admission, computed arguments, handoff compatibility,
complete class Q1 or approval. See [the exact component scope](reborn/recursive-contract-check.md).

[Sealed-subject contract-Recipe acceptance](validation/2026-10-24-python-contract-review.md)
adds the retained `python_contracts` draft and composes its pinned recursive helper
with existing Collection/Aggregation on the real closed global Python flow.
Eight native worker/PG tests plus strict affected/root lint and format pass.
Invalid root/dependency contracts aggregate negative; swapped original subject
data fails the sealed receipt check even with identical positive output. There
are no Tool effects, model exchanges or new review evidence. The isolated scope
does not certify concurrent embedded-guide edits or a final regular app artifact;
full mandatory coverage and durable Q1/approval remain open.

[Private structure-Recipe acceptance](validation/2026-10-24-python-structure-review.md)
adds installation-retained source → native inventory → definitions/contracts →
Collection/Aggregation/receipt drafts. Actual ordinary startup preserves their
original bytes and operator successor; a closed global flow covers ten positive/
negative/substitution cases, exact native answers, worker identity and original
input/result edges with one journalled inventory dispatch each. Eleven fresh tests
and three applicable inventory results pass, with strict full-feature and
PostgreSQL-default lint, format and whitespace. The unsupported set operator is
repaired through an actual worker diagnostic; the bootstrap feature gate works
without `skills-db`. Complete mandatory rules, ordinary private Q1 and durable
review/approval remain open; edited guides/final regular app are outside this scope.

## Implemented boundaries

[Private pure-call draft acceptance](validation/2026-10-24-python-pure-calls-review.md)
records the batch already running when K0–K7 took priority. Forty-seven actual
tests and strict relevant lint/format pass: distinct ten-step/four-rule private
review, 17 general and 30 pure closed cases, no candidate execution, Q1 or
activation. The general structure draft stays separate. General pure-Q1 work is
deferred to its existing owner after K0–K7; current K1 repairs are not certified by
that preceding runtime-source snapshot.

[Pinned native profile/graph acceptance](validation/2026-10-24-python-graph-review.md)
extends the private structure draft to nine steps and three rules. Actual native
preflight name facts are pinned as typed constants; the new rule checks exact
dependencies, cycles, closure, symbol/export conflicts, positional interfaces and
original references after actual definition/contract results. Fifty native/PG/
Engine/architecture tests and strict full-feature/default lint pass, with 17 cases
in the closed Recipe. This remains partial structural review, not pure or complete
class Q1, Tool-call/binding/behavior approval, ordinary private admission, trusted
settled evidence or activation. Current embedded guide/final-app acceptance stays
separate from this isolated runtime-source batch.

| Area | Actual implementation and supported boundary | Remaining qualification or cutover |
| --- | --- | --- |
| Global Monty | `global_monty_startup.rs`, `GlobalMontyOwner`, `GlobalMontyDriver`, `global_task_factory.rs` start one supervised Monty 1.0 worker before ordinary workers/producers. Opaque input IDs, isolated tasks, typed retained Recipe execution and reply/history use that service. The per-chat lazy driver is removed; there is no Rust-loop fallback. | Supervised durable recovery/replacement, unresolved effects, broader wait/child/restart and platform acceptance. A completed task does not stop the instance VM. |
| Components and preload | `component_revision`, `revision_store`, retained instruction/inputs/tools/reviews and `retained_preload`/`retained_source` implement immutable revisions, recursive contracts, typed bindings, dependency-first function loading and pinned exports for selected graphs. Runtime values are data. Pure-review source prerequisites now expose exact complete closure observations, all-export checks, strict declarations and original sealed subject/selection identities; [their contract](reborn/pure-component-review.md) distinguishes these from approval. | Migrate remaining legacy seeds/callers; general standalone pure approval and qualified dynamic flow/occurrences remain separate gaps. Pure evidence writers/readers, Human Q2, seed qualification and activation are unfinished. Legacy composer bodies are not the ordinary retained execution contract. |
| Approval and deployment | Trusted structural/behavioral evidence, human association review, coherent authored catalogue activation and component deployment exist. Exact packaged bootstrap qualification exists for named first-party closures. | Eight class-specific validator Recipes and their ordinary durable scheduling are incomplete. Existing narrow package qualification is not a general pure-component exemption. Authored changes require human Q2; shipped seeds require controlled integrity/Q1/behavior, without extra installed-instance Q2. |
| Routing and MCP | Ordinary matching uses a coherent approved generation. Qualified command discovery, always-running MCP server and ordinary new-chat ingress/response correlation exist. Packaged public reply has scoped ordinary/MCP acceptance. | Populate and qualify the rest of the public library and its commands; migrate remaining compatibility surfaces. Draft/status labels are not advertised contracts. Only actual No-Match enters Tier 2. |
| Complete file source | The existing registered `builtin.read_file` has a strict `raw_utf8_preserved` profile returning the complete captured UTF-8 text, host invocation/content identity and explicit representation/completeness. Existing live read limits, current Tool policy and mount/structured-entry protection apply. [Scoped native acceptance](validation/2026-10-23-raw-file-snapshot.md) covers actual local and PostgreSQL callers and backend races. | Canonical raw Skill/export/ToolSkill/Recipe qualification, independently approved R10 pure checking, retained source/derived lineage and doc-sync/doc-convert consumer migration remain open. A numbered preview or caller flag is not raw provenance. |
| History and native custody | Exact submitted input and eligible history helpers exist; native Tool/model answers are retained before later validation or failed acknowledgement and redundant host payloads are freed after the original durable acknowledgement. | Finish remaining compatibility consumers and generic recovery/operator reconciliation. A journal acknowledgement cannot replay an effect or turn its original failure into success. |
| Child tasks | Explicit `workflow:{recipe_id,variant}` plus typed `inputs` reaches native admission with the original parent claim, invocation and captured catalogue. The actual child runs on the same global VM, uses dependent admission, returns its full correlated reply or classified terminal outcome and retains old selection. Readable ensure/input/run acknowledgement loss is reconciled by observation, without resubmission. | R09 installation-owned parent qualification/activation, all five historical subagent Recipe migrations, saturation, unreadable/delayed outcomes and restart. Legacy flavor/task authorization receipts are not child completion. Closed development review does not activate its parent Recipe. |
| Atomic transcripts/admission | Shared submitted-input binding rejects rebinding/redaction/wrong kinds; exact acknowledgement is read-only. PostgreSQL admission binds the existing input in the same transaction as the run/aliases/reservation. V176 adds canonical scoped inbound receipts with atomic message acceptance and checked legacy backfill. Memory/filesystem callers keep exact event identity and reject changed duplicate content. | Filesystem pending/partial-write reconciliation, legacy unkeyed reservation release and generic recovery remain open. Ambiguous/corrupt old receipts fail migration rather than selecting a winner. |
| Resources and live settings | Revisioned settings API/WebUI and acknowledged Rust/Monty publication cover heap, task compute, transport/hosting/queue limits, coding limits and native accounting. Default task compute is 600 executing seconds, excluding idle/queue/external waits. Startup/manual shared heap has a 512 MiB minimum; startup sizing is the default, optional automatic monitoring is separate. | Complete remaining cap inventory, effective-limit/platform/browser acceptance and general recovery. Fixed transport depth 64 and snapshot conflict-attempt policies (PG 12, filesystem 8) are still open; do not claim every value is adjustable. |
| Token policy | Discretionary token budgets default off, with accounting retained and eligible retrieval/history/task consumers uncapped by those quotas on the migrated path. Provider/model technical context/output limits remain independent. | Audit unmigrated consumers. Per-provider/model prefix/prompt policy is stored with provider revisions but unconditional final-dispatch enforcement is still required. |
| Prefix workstream | Six profiles, immutable originals/generations/maps, retained model-artifact inspection/tokenization, source-only compiler jobs and measurements, provider/model limit storage/UI, budget planning and source-check primitives exist. [Canonical dispatch-policy/count support](validation/2026-10-26-prefix-dispatch-policy.md) now reads exact provider/model caps and applicable declared context/output in the existing live revision result and supplies checked complete-input arithmetic, with real PG/cache/request/global acceptance. It adds no settings owner and is not authenticated request measurement or automatic final-dispatch enforcement. [Historical worker-reuse review](validation/2026-10-26-prefix-worker-evidence-review.md) limits the old incomplete 530-file applicability claims; observed receipts remain intact. The [leading-chat successor](validation/2026-10-26-prefix-leading-chat.md) now verifies expected leading bytes in actual complete rendering with context identity, real injection-adapter/HF/native/bootstrap/journal checks, and freshly reruns the exact affected older native Prefix cases with a newly built worker. Accepted artifact/provider-wire/policy binding and the final dispatch guard remain open. [Physical token partition measurement](validation/2026-10-26-prefix-token-partition.md) now extends the existing encoding Tool/usages with versioned whole-text byte-span attribution, native/HF/bootstrap/journal checks and strict affected/default acceptance. Caller-supplied physical spans are not authenticated semantic prefixes; the final actual-envelope guard remains open. The [source-export capacity consumer](validation/2026-10-25-prefix-export-capacity.md) now uses the acknowledged canonical catalogue byte limit instead of its fixed duplicate, with bounded size counting, complete-or-error output and fresh ordinary-runtime/PG/CAS exact-boundary/failure checks plus strict affected/default validation. A [budget-dependent reference-selection candidate](validation/2026-10-24-prefix-budget-selection.md#fixed-rulecurrent-content-resolution-successor) adds namespace/whole-unit rules, automatic current-content resolution, separate rule/unit-plan identities and exact complete-candidate measurements. [Wave-stable graph scaling](validation/2026-10-25-prefix-graph-scaling.md) adds efficient dependency/closure traversal with unchanged candidate bytes in differential checks, 15 Python cases and actual native 1,001-group coverage. A [captured-source candidate successor](validation/2026-10-25-prefix-captured-selection-and-sources.md) now composes profile-derived minimum coverage with the same measured selector, preserves complete snapshots and comparison evidence even when capacity-blocked, and qualifies both retained variants. Sources draft/profile race fixes have component and isolated real-browser evidence; strict affected/default checks pass. An [archived-original read candidate](validation/2026-10-25-prefix-archived-original.md) extends the existing generation-read Recipe with one pure full-source projection; four native integration cases and five exact-source-reused Python checks pass. A subsequent historical-index compatibility repair freshly passes seven Python checks and four native cases with strict affected/default lint; raw-by-hash originals work for both supported index formats, including baseline rows without input artifact names. A [registered-source candidate](validation/2026-10-25-prefix-registered-source-check.md) now connects the existing read/fetch/hash/classify/append usages as a 16-step variant, with seven fresh actual Monty/kernel-HTTP/PG cases and strict affected/default checks. Stored diagnostics preserve unknown content identity and remain unqualified. Admitted Sources execution, durable effect settlement and trusted availability projection remain open. Trusted Generate admission/approval/serving remain open. The existing model-design owner now retains [versioned unaccepted rule proposals](validation/2026-10-24-prefix-budget-selection.md#versioned-proposal-successor--24-october-2026), with actual declaration, HTTP/PostgreSQL, Monty/tokenizer and UI checks plus strict affected lints. | Named/composite target-aware Generate/serving, accepted model/profile/design bindings and trusted admission of complete authoritative profile coverage, Sources jobs/projection, Redesign/Create, final request guards, current cache/quality and installed acceptance. Qualify quarantined draft evaluation before acceptance; do not make normal Generate its own prerequisite. The new candidate is not bootstrap-qualified or activated. Source-only/quarantined generation is not activation. See the prefix plan's current delivery table. |
| Sempai and shared facilities | Completed-turn delivery/effect/accounting custody and immutable proposal/deployment infrastructure exist. Shared PostgreSQL endpoint/TLS/pool construction already exists. | Recipe-owned Sempai evidence/proposal/iteration semantics and connection/lifetime consolidation belong to the existing v3.5 work IDs. Do not build another reviewer, provider registry, VM, connection builder or universal recovery ledger. |

The original seven composition failures were resolved on their recorded global
service path. That limited result does not establish that all plans or all
integration suites pass on this combined tree. The validation record identifies
which combined-tree checks have actually run.

**Independent pure-step support:** authored preparation now checks each variant's
unbound executable steps separately from usage-closure UUID coverage; retained
selection/MCP verification repeats that check. Only the exact existing packaged
generation workflow has independent coverage on this path. [Scoped acceptance](validation/2026-10-23-independent-pure-execution.md)
passed for the reader hardening, actual operator/PG/global publication, old
selections, restart, deployment and child callers. Validator Steps 3/8–10
still own general root/export/closure approval, durable pins and compatibility/repair cutover,
as detailed in [the pure review contract](reborn/pure-component-review.md#exact-remaining-cutover-contract).
Neither this gate nor the source/declaration prerequisites establish general
independent pure activation. Invalid historical dependency-only selections fail
explicitly; they are not silently grandfathered, removed or replayed via Tier 2.

## One implementation owner per responsibility

This table is a dependency/navigation index, not another implementation plan.
Implement the linked owner's steps once. Other plans consume its contracts and
acceptance; they must not maintain a second implementation of that responsibility.

| Responsibility | Canonical implementation owner | Consumers / shared gate |
| --- | --- | --- |
| Instance authority, live resources, global lifecycle/admission/cancellation/recovery and runtime cap migration | [simplified v3 remaining work](../simplified_v3.md#remaining-implementation--consolidated-2026-10-22), Phases 0, 1–3, 3a, 4–8 and §§8–10 | Validator, composition, prefix; v3.5 V35-01/23/24/26/27/29. Reuse the existing service and acknowledged settings owners. |
| Complete review subjects, eight Q1 routes, behavior/semantic evidence, human Q2 and general pure-component approval | [validator remaining work](../validator_v3.md#remaining-implementation--consolidated-2026-10-22), Steps 1–12 | simplified Phase 0a consumes it; composition does not implement another validator. Coordinate V35-03/04/17–19 with this owner. |
| Canonical reusable components, typed layouts, bindings/exports, library migration and Skill execution commands/MCP population | [composition remaining work](plans/monty.composition.md#remaining-implementation--consolidated-2026-10-22), §§6, 20–25 and 35 | [component implementation instructions](plans/monty.composition.implementation.md), coverage, Recipe and Skill-Recipe appendices. simplified consumes qualified library results. |
| Actual child admission/recovery infrastructure | simplified Phase 3a and its current remaining-work section | Composition R09 consumes it and owns parent/usage/Recipe qualification and the five seed migrations; no second scheduler or Rust task workflow. |
| Prefix source/profile/design/generation/model budgets, target serving and final model-request size guards | [prefix remaining sequence](plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion), §§4–12 | simplified §10 consumes canonical provider storage/guards; v3.5 V35-20/28 consumes prefix policies and measurements. No duplicate editable budget store. |
| Recipe-owned Sempai conversion, dynamic flow/occurrences and shared connection/facility consolidation | [path to v3.5](../path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations), V35-09–16/21–22/28 and its dependency table | [Sempai](plans/sempai_v3.5.md) and [connections](plans/simplify_connections_v3.5.md) retain source requirements; v3 plans reuse prerequisites without duplicating future conversion. |
| Combined upgrade, installed browser/provider tests, platform matrix and release | simplified Phases 7–8 and §5 | Each owning plan supplies its acceptance results. A commit/push is a checkpoint, not a full-plan/release certificate. |

## Resource settings: what remains fixed

The [runtime limit audit](runtime-limits-audit.md) and actual
`MontyExecutionLimits`, revisioned settings DTOs, validators and frontend help
are the detailed inventory. Existing WebUI explanations must accompany every
new operator-adjustable policy. Distinguish desired and effective values.

- The 512 MiB startup/manual minimum is the requested product floor. A ceiling
  is not reserved memory. Safe larger values are live; unsafe reductions fail
  or remain visibly pending according to the selected mode.
- The 600-second default is per-task executing time, not global lifetime or a
  wall-clock deadline for external work. Removed allocation-count enforcement
  is retained as retired configuration evidence, not silently treated as active.
- Compiler nesting depth 30 (debug) / 200 (release), standard-library JSON
  recursion depth 200, transport depth 64 and the
  PG/filesystem optimistic conflict-attempt counts
  remain implementation policies to reconcile. Do not remove a parser guard
  before safe decoding/ownership/late-error cleanup and real deep-data checks.
  Conflict retries may repeat only pure snapshot transitions, never Tools.
- No active settings named `cachemax` or `max_shared_code_cache_bytes` were
  found in the audited runtime. Weak immutable code reuse is not a fixed
  task-data cache reservation. Keep auditing actual consumers rather than adding
  unused controls for absent names.
- Wire integer widths, checked sequence/version exhaustion, checksums and
  identity formats are representation constraints. Do not turn them into
  adjustable permissions or promise that an operator can exceed the protocol.

Every remaining implementation step must preserve exact old task selections,
live kernel policy, typed state, original effects and attempt fencing. Missing
support is implementation work; errors never become No-Match, empty success,
self-approval or a Rust/per-chat fallback.


## 2026-10-25 — K3 authenticated secret generation prerequisite

[Scoped acceptance](validation/2026-10-25-k3-secret-generation-ports.md) records
required generic reference/real CAS/owner-side comparison across all actual
stores/wrappers.429 run2 runtime cases are qualified solely for final unused-import
removals, with current strict/default/fmt checks. Actual PG/independent persistent
FS, readonly legacy preservation and post-COMMIT transport loss are covered.
This does not finish immutable manual input/account/interaction/flow custody,
DCR cleanup, original lease generation or K3–K7; keep the existing owner order.


## 2026-10-25 — K3 manual submit uncertainty

[Scoped acceptance](validation/2026-10-25-k3-manual-uncertainty.md) records the
shared native/generic completion coordinator and both HTTP routes preserving
original account/flow/interaction on unconfirmed completion/error/timeout/invalid
input.353 distinct cases (85 fresh final/268 exactly qualified), current strict/
default/fmt and actual persisted completion/public timeout/pending-input cases pass.
Full original manual input/publication/result, lower-store cleanup/latest-account
recovery/browser identity and DCR custody remain in existing K3 step2. The current
30-second server/browser constants remain K3 step6; K3–K7/full plans stay open.


### K3 filesystem secret CAS ABA — prerequisite closed, 2026-10-26

[Scoped acceptance](validation/2026-10-26-k3-secret-cas-aba.md) records atomic
version/encrypted-body checks in actual native memory/PG owners and controlled
secret delete/recreate-between-read-and-write. 904 passing Rust cases
(216 fresh final/688 exactly qualified), current strict/default/fmt and
actual public/global paths pass. Unsupported secret/resource CAS no longer
becomes Any; mount overclaims fail. The historical429-case receipt did not
cover this exact race and remains scoped to its own cases. Full immutable manual
input/publication, all-writer conditional deletion, lease generations and K3–K7
remain open. The next deletion prerequisite is private/source-only, not accepted.
The inherited worker-input list was subsequently found incomplete for normal/
transitive dependencies. The linked report narrows its global-worker claim;
The [next filesystem acceptance](validation/2026-10-26-k3-conditional-entry-delete.md)
now actually builds the worker and freshly runs both global callers; that closes
the current-worker equivalence gap. Direct native CAS results remain applicable.


## 2026-10-26 — K3 selected single-entry deletion

[Scoped acceptance](validation/2026-10-26-k3-conditional-entry-delete.md) records
native version/body conditional leaf deletion, unchanged scoped Delete permission,
composite/capability support and actual memory/PG/Local/concurrent/explicit-dir/
post-COMMIT-ack cases.116 fresh Rust cases and current strict/default/fmt pass.
The worker is genuinely rebuilt, with conservative complete-source provenance,
and actual retained Match/No-Match plus live-model global paths pass. All12 applied
and Prefix eight source hashes match. Full original/retained/manual/browser
custody and K3–K7 remain open; the larger11-file secret-owner draft is private,
not compiled or accepted. No component activation/platform/deployment claim.


## Exact K3-2 document contract and pure preparation — 29 October

[The bounded delivery record](validation/2026-10-29-exact-k3-2-doc-contract.md)
adds the future-consumer contract in open successor sections; all pre-existing
sections of the five protected plans are preserved by this Exact patch. Use
branches on accepted design, required construction child/review work is prefix-free,
and generation-internal Prefix-Q1 is followed by separate human Q2 activation.
The operator has confirmed retaining older live-referenced generations beyond
the two-predecessor archive until their references are released.

Versioned B1/B2 prompt artifacts and pure answer/evidence stages have 15 passing
Python cases and 65 contained Monty utility evaluations. The new class-22 draft
constructor remains outside the module tree, uncompiled and unseeded while the
Auth owner holds shared Source/Cargo. This is no ordinary provider dispatch,
Discovery/Design/UI/GC integration, runtime PrefixFreeWorkflow support, installed
uptake, K4–K7 closure or complete Exact/v3 acceptance. Existing native prefix
acceptance remains qualified by its separate owner receipt.
