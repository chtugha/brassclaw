# B-00 inherited Q1/Oracle package review

Date: 2026-11-03. Package base: `57364c351` (clean detached worktree before
creating `codex/b00-q1-oracle-review`). Only this package worktree is changed.
Inherited review subjects: `548254e43`, `a534c4654`, `626e9fddc`, `fdc1f0bc2`.
No main integration, push, release, provider change, local Cargo or test execution.

## Architecture and source reconciliation

The root AGENTS/CLAUDE, development policy, four component guides and
`path_to_v3.5.md` govern this review. Applicable crate guides are Composition
AGENTS/CLAUDE, Skills AGENTS and Engine AGENTS/CLAUDE/MONTY. Relevant owning work
is Bob 1 sections 3–8 (especially B1-04/06/08), Bob 2 sections 3–4 and Bob 3
sections 5–6. The existing owners are reused:

- Recipes/pinned PythonCode select and aggregate validator checks. Rust verifies
  immutable subjects, observations, claims and native settlement. No validator
  workflow moves into Rust and no Tool, scheduler, VM or fallback is added.
- `RetainedValidatorCoverage` and `ReviewAdmissionSource::verify_receipt` verify
  every observed input/result fingerprint before retaining provisional evidence.
  They cannot wait for their own terminal receipt. `GlobalMontyDriver::drive_host`
  owns shared native delivery; the factory owns durable acknowledgement.
- `SettledInternalCompletion` has private fields and is minted after
  `settle_attempt`. `read_settled_validator_q1` consumes this proof and the exact
  subject/selection, authenticating task/run/runner/claim/root/accounting and
  durable settlement before reading the corresponding diagnostic verdict.
- `pg_pure_component_approval` verifies strict evidence stage/identity/checksum
  and complete immutable closure. Public `PureApprovalStore` resolves existing
  exact bytes only. V203 binds new link candidates to both submission and
  approved root, preserving historical rows rather than rewriting them.
- The B1/B2 protocol oracle binds independent case inputs/expected results.
  Only the exact caught ValueError/message is a negative protocol observation;
  transport/containment/syntax/runtime errors remain errors. Producer/oracle v2
  prevents reuse of the earlier permissive evidence identity. Startup supplies
  its actual compute settings through the existing live-settings conversion.

Each task keeps its original exact graph; live Tool policy remains independent.
ResultOnly, an observation row or a public declaration establishes no approval.
Auth/Codex and V204 were inspected only as baseline context and are unchanged.

## Concrete correction

The final Q1 match still wrote `gate1_fail_reviewed` for cancellation and
unexpected reply/blocked/failed completion. Those are incomplete execution,
not completed negative candidate checks. B-00 moves completion authentication
into a private gate used by the actual orchestrator. All such exits and missing
settlement proof return existing `Q1Error::Admission` without a queue decision.
Only `read_settled_validator_q1` can supply the verdict subsequently applied to
the queue. Actual observed negative diagnostics retain their existing path.

The new regression source covers cancellation, unresolved-effect failure and
all completion kinds without a trusted proof. These are negative classification
fixtures, not simulated successful execution or native-settlement evidence.
The existing real PG/global-worker positive and negative Q1 cases remain the
caller-level acceptance selectors. No public DTO, Driver/Auth port, database
schema, approval format or migration changes in this package.

## Review outcome and explicitly open interfaces

No further defect was found in the bounded inherited pure-link/public-write,
shared-drive/proof, exact-verdict or Oracle changes during source review. This
is not acceptance of the larger surrounding implementation:

1. B1-02/B3-06 must atomically bind the original queued proposal, immutable
   submission, selected validator/check set and original work/run/attempt before
   effects. Current latest-submission/latest-draft selection and synthetic
   bootstrap chat admission remain open. Graduation still needs that original
   binding and durable idempotent outcome reconciliation; this package adds none.
2. B1-05/06 must produce actual independently observed pure/own-function evidence
   and authenticated Q2, with noncircular S qualification and exact execution
   identities. The internal L013/validator seed producers still contain check-list
   success reports and UUID-only skip identities. Mechanical writer checks and
   finite protocol cases do not repair or certify these producers.
3. B2 must carry checked exact approval digests, export/interface and actual
   selected dependency coverage through every prepare/restore/task/private/child/
   prefix/MCP reader before successor publication. UUID-only independent-pure
   membership remains an open reader issue. No partial cutover is enabled here.
4. Full interruption/native custody, durable work recovery, eight-class checks,
   genuine complete seed qualification and installed/platform acceptance remain
   with their original owners. The Apple `__eh_frame` warning is unresolved;
   no warning suppression or unwind/panic change was made.

The private canonical.patch was not applied. Root board/status/plan files were
not changed; the supervisor owns their integration updates.

## Evidence and native handoff

Prior evidence is attributed only: run-1 passed 17/17 on its source; run-2 compiled
and passed 18/19, its decoy fixture failed V102 before execution and was corrected;
run-3 stopped before Cargo at the disk guard. None validates later changed bytes.

Local source checks: targeted `rustfmt --edition 2024 --check` and
`git diff --check`; no Cargo/test processes started. New tests are uncompiled and
unexecuted in this package. Native test compilation, execution and affected-package
lint remain supervisor-owned.

Necessary Modal build: current standalone CLI on the activated Apple image,
8 CPU/32 GiB/8 jobs, 900 seconds, nonpreemptible; existing shared controller/lock,
no new image or remote test. Local timing basis: the inherited shared Composition/
startup delta needs a multi-minute CLI compile (prior compatible remote Cargo
4m47s / controller 404.52s; native test batch is separately much longer).
Use estimated local 600 seconds. Billing estimate is below USD 1 for the single
900-second bounded build, reusing the existing qualified profile; this is an
estimate, not an actual billing measurement. The actual receipt is recorded below.

Recommended Composition native selectors (`--features postgres,skills-db --lib`;
real installed worker and native PG required, zero-selected/ignored is failure):

- `q1_orchestrator::tests::incomplete_execution_cannot_become_a_negative_candidate_verdict`
- `runtime::tests::validator_retained_q1_tests::run_q1_validation_retained_passes_and_persists_evidence`
- `runtime::tests::validator_retained_q1_tests::run_q1_validation_retained_fails_for_empty_content`
- `bootstrap_free_workflow_approval::fixture::tests::native_packaged_protocol_oracle_observes_every_declared_case`
- `bootstrap_free_workflow_approval::fixture::tests::native_oracle_only_classifies_the_expected_protocol_rejection`
- `bootstrap_free_workflow_approval::fixture::tests::stored_oracle_requires_the_exact_independent_answer_and_rejection`
- `pg_validator_q1_evidence::verdict::tests::`
- `pg_pure_component_approval::tests::`
- `native_http_pure_declaration_cannot_mint_approval_even_with_positive_rows`
- `global_driver_hands_opaque_admitted_tasks_to_one_existing_service`
- `completed_reply_survives_waiter_fencing_before_durable_settlement`
- `dropped_turn_waiter_requires_worker_and_durable_cancellation_acknowledgement`
- `global_driver_repeated_rejections_release_attempt_capacity_without_cancellation_or_replay`
- `unsubmitted_cancellation_retains_exact_proof_until_database_acknowledges`

Resolve substring selectors against the freshly compiled executable's `--list`
before exact execution. The two Q1 module paths above must likewise be confirmed
against that list; names are source selectors, not claims of observed execution.
Actual in-flight cancellation between provisional observation and settlement,
lost settlement ACK and stale-claim graduation need native interruption acceptance;
the new pure classification regression does not establish these stronger claims.


## Terminal Modal receipt

Build source commit: `f62e911cf2b994298d5a16a026b9da14b1352779`.
Job: `cf5e67e0-597d-4cc3-ae4a-0854fccb0f5c`.
[Completed Modal App](https://modal.com/apps/chtugha/brassclaw/ap-DCukHBS0sru814J6FKViwj).
The controller returned zero and reports completed, after exact remote/local
receipt equality and artifact checksum verification. The shared lock was held
through upload/readback, execution and receipt/artifact collection.

```text
python scripts/modal_build.py --platform apple run --confirmed-modal --estimated-local-secs 600 -- cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn --target aarch64-apple-darwin
```

- Source manifest SHA-256:
  `27e8526e699dfcebd2a0b6f4bc1053e632d00f0c1b2ad840b8576a034d5f8737`.
- `q1_orchestrator.rs` manifest/current worktree SHA-256:
  `cf06578f0bde92ee7ed25e432fabe1b09574f1075d21a2373bdf0e4238d9e57b`.
- Exact qualified image: `im-S1cpT9G1fsMPpwjISvBSlK`, Rust 1.96.0,
  Apple arm64 dev profile. CLI defaults propagate `postgres,skills-db` and
  root provider support to Composition; no test target was compiled remotely.
- Resources: 8 reserved/maximum cores, 32768 MiB reserved/maximum RAM, 8 Cargo
  jobs, nonpreemptible, 900-second deadline. Actual worker duration 543.590205s;
  Cargo duration 7m14s. No compiler warnings/errors in the retained complete log.
- Mach-O arm64 CLI size: 336727712 bytes; SHA-256:
  `809e6ff0648b2aa701e806d1cc9ce4215d0229f83962a499a83b1353e8573d76`.
- Completion receipt SHA-256:
  `3ce79ebdc9f5a59545285bc43201c251b206e3ec22dabe843a00db494333ed17`.
- Runtime cost estimate using the repository's earlier same-profile estimate
  (USD 0.43 / 808.41s): about USD 0.29. This extrapolation is not a current
  account billing measurement and excludes storage/transfer/setup.

Full request/source/applied/original-runner/provenance/log/completion/artifact
records remain in the existing owner directory:
`/Users/ollama/brassclaw-target/modal-build/jobs/cf5e67e0-597d-4cc3-ae4a-0854fccb0f5c/`.
No second job, image creation, remote test, local Cargo, native executable run
or Clippy invocation occurred. Test sources remain uncompiled/unexecuted.
The final report-only commit does not change compiled Rust inputs. The observed
native linker unwind warning from earlier acceptance is still unresolved;
its absence from this cross-build log does not establish a native-linker fix.
