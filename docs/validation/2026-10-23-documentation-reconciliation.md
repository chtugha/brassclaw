# Common-tree documentation reconciliation — 2026-10-23

This records source consolidation and documentation maintenance, not completion
of the implementation plans or a new release. The original combined-tree audit
and scoped checks are in [the consolidation record](2026-10-22-consolidation.md).

## Source and build ownership

One active checkout and index belong to repository-root `main`. The two retained
historical local branches are ancestors, not unmerged agent work. The mutable
build mirrors and second Cargo target were retired after source/evidence checks.
Use only `/Users/ollama/brassclaw-target` on this development host, serially under
the shared Cargo lock. Immutable historical source/binary/receipt archives remain
evidence; they are not competing build trees and must not be deleted as caches.
Platform installations use their own appropriate paths.

The successors included in this reconciliation are:

| Checkpoint | Source and evidence boundary |
| --- | --- |
| `19cf6382f` | Combined prefix/runtime source, native child handoff, atomic inbound V176 and active plan/guide reconciliation; [503 scoped checks](2026-10-22-consolidation.md). |
| `c52bdc2e0` | Complete native UTF-8 capture; [75 scoped checks](2026-10-23-raw-file-snapshot.md). Canonical usage/R10 pure qualification and consumers remain open. |
| `887e0ea14` | Pure-source/declaration/sealed-subject prerequisites and real PostgreSQL test transport ownership; [67 fresh scoped checks](2026-10-23-pure-review-foundation.md), separately identified reused global-root evidence. |
| `172f8c085` | Per-variant and retained-reader independent pure-execution gate; [39 scoped checks](2026-10-23-independent-pure-execution.md). General pure approval remains open. |
| `c5ee5ecd0` | Captured per-step validator observation limits; [seven actual-worker/PG checks](2026-10-23-validator-observation-limits.md), complete negative diagnostics and strict affected/root lint. Not successful class Q1 or activation. |

Counts describe each named receipt and may overlap. They are not a newly executed
combined-suite total. Frozen receipts preserve actual source/artifact inventories,
commands, failures and limitations; a prose update does not broaden them.

## Duplicate implementation disposition

The existing detailed steps and acceptance matrices remain binding. The current
remaining-work sections are their ordered disposition; no completed foundation
is restarted. Each consumer references its owner instead of maintaining another
implementation of that responsibility.

| Responsibility | Selected plan and reason | Other plans consume |
| --- | --- | --- |
| Global VM, authority, resources, admission/cancellation/recovery and release | [Simplified v3](../../simplified_v3.md#remaining-implementation--consolidated-2026-10-22) owns actual product/runtime cutover and its production/platform acceptance. | Validator, composition and v3.5 use the existing service, acknowledged settings, effect custody and recovery contract. |
| Complete Q1 subjects/class routes, observed behavior, human Q2 and standalone pure approval | [Validator](../../validator_v3.md#remaining-implementation--consolidated-2026-10-22) owns trusted review provenance, evidence and activation prerequisites. Its [exact pure cutover](../reborn/pure-component-review.md#exact-remaining-cutover-contract) names writer/reader/schema, execution correlation, Q2/seed, pin and compatibility requirements. | Simplified Phase 0a, composition R10 and V35-03/04 consume that implementation; no second writer, approval exemption or zero-Tool Skill. |
| Reusable component library, R09/R10 usages and commands | [Composition](../plans/monty.composition.md#remaining-implementation--consolidated-2026-10-22) has the per-component/Recipe/usage ledgers and [detailed appendix](../plans/monty.composition.implementation.md). | Runtime supplies child/file primitives and validator supplies qualification; composition owns the parent/raw usage and all consuming Recipe migrations. |
| Prefix sources/design/generation, model-specific budgets, serving and final request guards | [Prefix](../plans/prefix_v3_upgrade.md#remaining-implementation-sequence--full-completion) owns the complete source/design/provider lifecycle and size acceptance. | Simplified and v3.5 consume its guards/policies through the existing provider store; no second compiler or editable limit owner. |
| Dynamic occurrences, Recipe-owned Sempai and shared connection/facility conversion | [Path to v3.5](../../path_to_v3.5.md#ongoing-plan-handoffs-not-parallel-implementations) has stable work IDs and dependency handoffs. | Current v3 work preserves the required provenance/fencing; it does not preempt V35-09–16/21–22/28 with another scheduler/reviewer/connection builder. |

The four component guides distinguish the implemented retained runner from the
legacy substitution composer. Root architecture guides and all four requested
plans now include the subsequent pure-review/observation boundaries. Active
subsystem guidance, resource audit and Prefix/Sempai/connection references retain
their existing current-status routing. Historical migrations, validation journals,
frozen references and deprecated examples retain their original content/evidence.

## Validation of the final documentation

The first batch passed compilation and the actual complete-guide baseline, then
found a stale renderer regression expecting five helpers after the current source
had grown to twelve. The successor checks the actual parsed helper-name/signature
map against the component's declared private-function interface. It retains the
effect-free and actual output/original preservation checks; no production renderer
or approval exception is changed. The failed receipt is retained separately.

The corrected batch passed from the common root, Rust 1.96.0, the single target
and shared lock, with `--locked`, incremental/debug symbols disabled and the
existing composition `skills-db,test-support,migrate-from-libsql,bedrock` plus
`brassclaw_llm/testing` configuration. The application build also enables its
`skills-db,migrate-from-libsql,bedrock` features. Exact commands and source/artifact
inventories are retained in
`target/accepted-evidence/documentation-reconciliation-20261023/run-2/accepted.json`.

| Executed check | Result / command seconds |
| --- | --- |
| Common application/worker build and application version | Passed; 44.535 / 1.498. |
| Composition test/worker build | Passed; 108.785. |
| Complete actual embedded guide baseline | 1 passed, none failed/ignored; 8.938. |
| Actual pure renderer and helper declaration map | 1 passed, none failed/ignored; 0.091. |
| Installation profiles, repeat seeding and preserved overrides | 1 passed, none failed/ignored; 2.888. |
| Ordinary global-root matched/unmatched execution | 1 passed, none failed/ignored; 68.693. |
| Affected composition Clippy, `--tests -- -D warnings` | Passed; 77.586. |
| Repository formatting and whitespace | Passed; 4.466 / 0.044. |
| Local links/anchors in 88 touched active Markdown documents | Passed. |

These are **four newly executed tests**, not a complete test suite or benchmark.
The complete current guide bytes are checked by the actual contained worker;
older binary receipts are not relabelled for new embedded references. All 39
reported executable artifacts and the adjacent actually selected worker are
integrity checked. The final canonical app/worker pair uses the normal application
build and the exact worker used by these tests:

| Artifact | SHA-256 |
| --- | --- |
| Application | `65b1d82ee89186487e897e0e9cecc6a0818c4482711db63de26cfa8fa6e93602` |
| Adjacent normal-build worker actually selected by `installed_worker()` | `b1fefa0f6e0742611100cca391a3a63fcaa1d6ec61663400879ed11ae7185fa9` |
| Additional worker artifact emitted by the test-feature build, retained separately | `861223cad68d9e964fadb3e1efa74c56f55b44949b078b1a710de79ce9db1e39` |
| Composition tests | `6139bd294649a52094493a2b6417fc327ccb8942864f51010861dd86353cd019` |

The acceptance receipt SHA-256 is
`451cbf4ba4472e06d536f66b0c5d00df789f32f7c0a4917121a9063f64e600dd`;
source inventory SHA-256 is
`8e4d7448251517b74e824decd34b27ea6993212225613c8a360a85ee8a4aaf32`.
`final-source-applicability.json` records final source/worker custody and canonical
restoration. Only this unembedded report is completed after acceptance. No full
model/platform/installed, release or full-plan acceptance is implied. All remaining
implementation steps retain their owning plan and acceptance requirements.
