# 2026-11-12 — Runnable intermediate checkpoint, incomplete acceptance

The user reprioritized integrating validated, ready work and publishing Main
while credits are limited. This is a development checkpoint: the application
and worker build, and version/help smokes pass. It is **not** release, deployed
support, general Pure approval, full private-stack acceptance or completion of
any of the three Bob plans. No version bump, tag or installation is claimed.

## Source and integration

Compiled runtime source is `9bb027f368f2380d411f788aecf9503b677591de`.
Subsequent `6df1342e1`/`a139d1c6f` publication and preserving merge `f520ee396`
change only mirrored host development Skill instructions; the merge has no tree
delta from its first parent. Final checkpoint docs and Modal helpers are outside
runtime/compiler inputs. Their Python tests are separate from native acceptance.

The independently reviewed T slice `82edcdb57`, integrated as `2b5ed2f2c`, adds
bounded filesystem record projection through the existing PostgreSQL/Composite
owner. Exact kind/version and byte limits are checked in one atomic read;
unsupported backends do not fall back to an unbounded read. It does not activate
full History, private Memory, qualification or cold restore.

Reviewed checkpoint corrections are integrated through `9bb027f36`: real inner
file-level test gates and conservative architecture scanners; the existing PG
TransactionClient Drop let-chain; Memory hex decoder using checked `as_chunks`;
Codex side-effect `inspect_err`; boxed optional Python exceptions with actual
Engine unboxing; and private selection type alias/boxed retained preparation.
No Rust-loop fallback, new approval grant or suppression is introduced.

Final inventory records 47 local branches: 44 `codex/*`, Main and two other
branches. Of 46 side histories, 34 are ancestors or patch-equivalent and 12 have
non-equivalent histories. These include superseded variants and the connected
N/R/Q/T stack, not twelve independently missing features. There are 18 worktrees.
No branch/worktree is deleted. Full T remains at `92fc13a61`, and Q's preserving
combined branch at `8ed62ca7b`; those stacks remain unaccepted as a whole.

## Actual compilation and retained programs

All three commands run locally and serially under the canonical native Cargo
lock, via `python3 scripts/local_build.py --jobs 2 -- cargo ...`, inheriting the
configured `/Users/ollama/brassclaw-target`. There is no inline target override
or explicit shell wrapper. Rust is `1.98.1 (48a229cea 2026-09-01)`.

| Program | Cargo arguments after `cargo` | Seconds | Result |
| --- | --- | ---: | --- |
| Application | `build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn --profile test --message-format=json` | 497.950 | PASS, zero compiler warnings |
| Normal worker | `build --locked -p brassclaw_monty_host --bin monty_worker --message-format=json` | 12.157 | PASS, zero compiler warnings |
| Composition test harness | `test --locked -p brassclaw_reborn_composition --features postgres,skills-db,migrate-from-libsql,brassclaw_pg/test-support --lib --no-run --message-format=json` | 1260.866 | PASS, zero compiler warnings |

The actual application uses its default `postgres`, `skills-db`,
`migrate-from-libsql` and `root-llm-provider` features. Its `--version` reports
`brassclaw-reborn 1.7.0-rc.4`; `--help` also succeeds. The application uses the
existing test profile with opt-level 1, debug level 1, debug assertions and
overflow checks enabled; it is a product executable, not a test harness. The
worker is its normal dev opt-level 0/debug-level 1 build with no features. The
Composition harness uses test opt-level 1/debug-level 1 and actual test-support.

The genuine harness link is warning-free: `__eh_frame` is 13,760,932 bytes and
`__unwind_info` 2,639,512 bytes. The real selected package profile gates pass,
including the existing `sha2` opt-level 3 override. No unwind section, overflow
check or assertion is disabled. This does not establish another platform's link.

Retained programs are under local, ignored
`target/validation/bob-native-repair-20261105/run-52-checkpoint/binaries/`:

| Artifact | SHA-256 |
| --- | --- |
| `brassclaw-reborn` | `f047b13ed159fc29c072961920994018fb8cc8e0132c363a2da3ca90fc5271e3` |
| `monty_worker` | `b460d2d828d32ffe6a2b5da44a000c0eea643eb14ceacaefcac4b9b3c0b904bd` |
| `brassclaw_reborn_composition` | `6334a412a3dc015f255e1a8fcae15653948a9ec34215d2f0d6919ff932a2d7b9` |

## Five genuine native cases

These are execution-only uses of those retained images, with actual PostgreSQL
and original global-worker paths. Each selector executes one real test, zero
ignored, with 1441 filtered out. No Cargo rebuild is performed for execution.

| Result | Seconds | Exact selector |
| --- | ---: | --- |
| PASS | 4.084 | `native_bounded_record_tests::native_pg_bounded_record_projection_preserves_original_versions_and_rejects_oversize` |
| FAIL | 441.261 | `runtime::tests::qualified_source_review_tests::native_qualified_pure_source_original_reaches_existing_worker` |
| FAIL | 977.345 | `runtime::tests::private_work_admission_tests::intent_custody::exhausted_pool_and_original_claim_lock_share_one_captured_deadline` |
| PASS | 24.038 | `pg_review_submission::function_tests::native_function_subject_atomically_retains_both_edits_and_exact_unknown_commit_retry` |
| PASS | 450.980 | `runtime::tests::original_pure_case_tests::native_original_pure_case_records_real_return_exception_and_contract_defect_without_approval` |

The bounded projection case uses a pool **constructed with capacity one**, exact
and absent/wrong-kind/opaque/directory/version/oversize cases, metadata isolation
and fail-closed selected-mount behavior. The function-subject case checks both
retained edits and original unknown-COMMIT reconciliation. The original Pure
case exercises actual return, Python exception and contract-defect observations
without granting candidate approval. Those three passes have only that scope.

### Open Source1 failure

The Source1 case fails during fresh `build_reborn_runtime`, before its source
assertions. The closed error says `checker_fixture_admission`, settings revision
1, `timeout_millis=2000`, outcome unresolved. The existing bounded owner wraps
the entire original admission flow, including thread/inbound/run submission,
claim, host handoff and cancellation. The exact delayed substage is not proven;
this is not evidence that one particular SQL statement is slow. Earlier Run-43
Source1 passed, and the current Pure case completes fresh startup. Do not
reclassify the failure as universal startup failure or retry unchanged for green.

Next bounded diagnosis belongs in the existing bootstrap admission/IO owners:
observe actual phases of this original exchange, retain unresolved-effect
custody and identify the owner of elapsed work before changing behavior. A
larger timeout or synthetic replacement fixture is not a demonstrated repair.

### Open physical backend cancellation acknowledgement

Both genuinely pool-one fresh bootstraps now finish. The exhausted-pool branch
returns bounded `Unavailable` and advances through its no-effect checks. The
original-claim-lock branch reaches the expected original PostgreSQL backend
blocked by the actual held row lock. It records acquire/begin acknowledgement,
`claim_started`, `client_scope_ended`, an empty retired pool and `Unavailable`
under revision 2's 2000 ms deadline.

The test then fails its actual backend-termination check after 30 seconds,
**before releasing the original admin lock**. A client future ending or pool
connection retiring does not prove PostgreSQL stopped the in-flight command.
The driver/server cancellation cause remains unproven; no product repair is
claimed. Follow-up must establish original-command cancellation/reaping and
server acknowledgement through the existing IO custody/PG owner. Do not move
unlock earlier, weaken the assertion, increase the pool or assume socket close
is acknowledged cancellation. Existing attempt/effect fences remain mandatory.

## Other checks and preserved failures

- All **33 architecture cases pass** in Run-51 after the explicit real test gates
  and positive/negative scanner regressions.
- Reuse **68 default filesystem tests** from Run-46: exact unchanged inputs,
  dependency/toolchain/environment and retained evidence are checked.
- Reuse warning-free PostgreSQL filesystem all-targets strict Clippy and **two
  Memory codec tests** from Run-49. The intervening Engine/LLM/Monty/Skills edits
  are outside those unchanged dependency paths. These are not full FS/Memory
  workflow acceptance.
- `python3 scripts/test_build_environments.py` passes **16 tests** locally in
  0.324 seconds during finalization. They use controlled transport/fixtures and
  real local lock processes, not actual cloud compilation or product acceptance.
- Composition library strict Clippy with the exact test features above and
  `--keep-going -- -D warnings` **fails**. All 20 remaining diagnostics occur in
  13 source files unchanged from checkpoint preparation `b48f43cc`.

### Remaining strict-Clippy diagnostics

Locations refer to the compiled source, under
`crates/brassclaw_reborn_composition/src/`:

| Source location | Diagnostic |
| --- | --- |
| `pg_review_submission.rs:344` | `clippy::redundant_field_names` |
| `pg_review_submission.rs:345` | `clippy::redundant_field_names` |
| `authored_monty_catalogue.rs:945` | `clippy::redundant_field_names` |
| `pg_review_admission_source.rs:448` | `clippy::collapsible_if` |
| `pg_review_submission.rs:205` | `clippy::too_many_arguments` |
| `pg_review_submission.rs:493` | `clippy::collapsible_if` |
| `pg_review_submission.rs:579` | `clippy::collapsible_if` |
| `provider_codex_credentials.rs:268` | `clippy::large_enum_variant` |
| `webui_body_limit.rs:111` | `clippy::result_large_err` |
| `bootstrap_checker_execution.rs:427` | `clippy::collapsible_if` |
| `bootstrap_checker_execution.rs:463` | `clippy::collapsible_if` |
| `bootstrap_pure_checker_fixtures.rs:639` | `clippy::type_complexity` |
| `private_work_terminal.rs:98` | `clippy::large_enum_variant` |
| `pure_case_qualification.rs:952` | `clippy::needless_borrow` |
| `pure_case_source.rs:38` | `clippy::too_many_arguments` |
| `installation_inventory_observation.rs:210` | `clippy::too_many_arguments` |
| `installation_inventory_observation.rs:240` | `clippy::too_many_arguments` |
| `bootstrap_inventory_qualification.rs:135` | `clippy::too_many_arguments` |
| `bootstrap_inventory_qualification.rs:185` | `clippy::too_many_arguments` |
| `private_task_catalogue.rs:868` | `clippy::cmp_owned` |

B3-12 owns final applicable validation; B2-09 owns connected legacy reader/API
migration. These findings remain an explicit open gate under those existing
owners, with exact JSON source hashes. No allow attribute, warning filter or
false green baseline is added. Existing legacy deprecation migration is still
open; a warning-free build is not a successful strict-Clippy run.

### Original source guards are not rewritten

Run-52's strict full-tree guard fails during compilation because separately
human-authorized Modal helpers and the new unembedded plan change. An external
five-line Modal CHANGELOG addition is parked and the compiled original restored.
A separate terminal review checks all **11,421** actual compiler dep-info files,
no references to the changed non-input paths, unchanged Rust/manifests/runtime
inputs, actual original CHANGELOG bytes embedded in both executable images,
and retained artifact hashes. It establishes compilation applicability; the
original `completed.json` remains failed.

During native execution the separately authorized mirrored host Skills and
unembedded plan change too. A second terminal review preserves that failed
manifest guard, verifies the development-doc-only commit/merge and all three
actual retained/canonical images. It records the three passes and two failures
without claiming a green aggregate. Later finalization changes only unembedded
development documentation and already reviewed Python build helpers. The
embedded CHANGELOG checkpoint entry is unchanged from the actual build.

## Modal and cost scope

The compiler helper selects Apple Rust 1.98.1 independently of the project's
MSRV, preserves the pinned SDK/bootstrap layers and serializes image setup
against unsettled remote jobs. Prepared image `im-Wxm74Dwhp6v20AWvy6T6aj`, App
`ap-cWU0HDza3xIJQkrZoOeft0`, has matching compiler version and an actual arm64
Mach-O compile/link probe. The first missing-cross-linker probe failure is
preserved. The image is **not activated**, with no BrassClaw project build or
native acceptance on that revision. Earlier image qualification does not transfer.

The user now authorizes useful Modal test builds, tests and other necessary
work, serially or alongside local work, when it reduces total turnaround.
The development policy records this instead of the old blanket local-only rule.
The current Apple controller still only exports CLI dev/release builds. The
combined-build document remains a plan. No new Root paid cloud job or duplicate
project build was launched for this checkpoint. Matching Rust does not make
Linux-host and macOS-host Cargo targets interchangeable; no target tree is copied.

## Evidence retention and next ownership

Original local records remain under
`target/validation/bob-native-repair-20261105/`: Run-46 default FS, Run-49 FS lint/
Memory, Run-51 architecture and strict lint, and Run-52 compile/native outcomes.
Run-52 retains `source.json`, `applied.json`, `artifacts.json`, link receipts,
`terminal-disposition.json`, `native-terminal-disposition.json` and all five
`selected-native/test-00N.log` files. Original failures and original runners are
preserved. Raw records/binaries are local ignored evidence; this report records
portable facts without committing binaries, databases or credentials.

Only audited obsolete compiler cache entries are cleaned when actual disk gates
fail; retained artifacts, active/current caches, source, DB and evidence remain.
All owned Run-52 compilation, worker and PostgreSQL processes are terminal and
the freeze is released. No more compile is needed for final unembedded docs.

The next bounded runtime work is diagnosis/repair of the two failed original
boundaries, followed by their necessary native acceptance. The complete private
N/R/Q/T stack, actual private Memory consumer, general Pure approval, remaining
validator/seed migrations, policy/cap migration and durable recovery remain open
in their existing owners. Do not merge the private stack or mark Bob completion
on the strength of this intermediate checkpoint.
