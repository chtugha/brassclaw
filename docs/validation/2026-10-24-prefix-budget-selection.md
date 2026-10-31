# Budget-dependent reference selection — 2026-10-24

## Accepted scope

The [reference-selection contract](../reborn/prefix-budget-design.md) now has
working pure PythonCode and a six-step retained Recipe constructor. The Recipe
reuses existing native UTF-8 digest and original-template model-encoding usages.
No new Rust workflow Tool, provider request, mutable model-limit store or
approval exception was introduced.

Preparation validates all declared budget bands, complete unit ownership,
required groups, dependency closure/cycles and stable optional priority. It
preserves whole executable/fenced units and ancestor context. Optional additions
retain the smaller candidate's leading bytes. Selection verifies the complete
candidate bank, original digests and exact pinned-model observations, then checks
every full candidate without assuming monotonic token counts. Omitted units keep
their original source hash/range and a distinct design/size disposition. Sources
and original input bytes are not shortened or rewritten.

Set membership avoids quadratic source/unit ownership scans; buffered rendering
avoids repeated growing-text concatenation. There is no model-specific default
budget. The integer representation bound and SHA-256 format checks are technical
contracts, not editable token limits.

## Observed checks

Run from the common repository root using its configured canonical Cargo target
and serial Cargo lock. Toolchain: rustc 1.98.1 / cargo 1.98.1. The working tree
contained parallel catalogue changes; these checks do not certify those changes.

| Check | Actual result |
| --- | --- |
| `python3 scripts/prefix/test_prefix_design_selection.py` | Eight passed: fitting/exceeded boundaries, required dependencies, stable leading bytes, explicit bands including invalid unused bands, real Unigram nonmonotonic counts, source/measurement substitutions, fenced code/ancestor/control-token preservation and a complete 1,001-unit inventory. Actual WordLevel/Unigram encoders, no invented counts. |
| `cargo --config build.incremental=false --config profile.dev.debug=0 --config profile.test.debug=0 test -p brassclaw_reborn_composition --features skills-db,root-llm-provider --test prefix_design_selection --no-run` | Integration executable compiled successfully. |
| Retained integration executable with its adjacent trusted utility worker, `--nocapture` | Two actual tests passed, zero filtered: Monty preparation/selection plus Rust tokenizer agreement; exact retained Recipe, both native bindings and dependency/preload inspection. |
| Corresponding targeted `cargo clippy ... --test prefix_design_selection` | Completed with one unrelated existing `collapsible_if` warning at `catalogue_publication.rs:155`. No new-target warnings. |
| Earlier affected skills/composition strict library/test lint with `-D warnings` | Blocked by that same parallel catalogue warning; **not** recorded as a strict pass. No suppression or peer-code change. |
| Focused `rustfmt --check` and `git diff --check` | Passed. |

The raw executable/log/identity archive is
`target/accepted-evidence/prefix-design-selection-20261024/integration-successor/`.
`identities.json` identifies tested direct sources and executable hashes;
`build.log`, `tests.log`, `clippy.log` and `scope.json` retain the bounded results.
Source hashes were checked again after execution. Historical `first-build/` and
`final-build/` names denote earlier diagnostic attempts, not accepted terminal
results. They retain failures for diagnosis.

Disk-policy cleanup preserved needed executable evidence first. Disabling
incremental/debug output and moving these checks to their own ordinary Cargo
integration target reduced rebuild/cache pressure; no alternate target or mutable
source projection was used. The first whole-lib test feature selection lacked
the legacy migration feature required by unrelated unit tests. Subsequent
diagnostics exposed missing packaged-worker placement, an incomplete test snapshot
root inventory and an overly strong observation-string consumer constraint.
The final target retains all required roots, selects the exact Recipe closure,
uses the existing producer's string schema and validates canonical observation
content before selecting anything.

## Remaining boundaries

The utility worker was retained from the existing trusted packaged image, SHA-256
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.
It was placed beside the archived test executable; it was not substituted into
the active build target or claimed as a newly built application/worker pair.
The tokenizer test's observation metadata wraps actual algorithm results solely
for pure selection checks; it is not a native effect-custody/approval receipt.

The new graph is a candidate definition, without startup qualification, accepted
design/profile/consumer bindings or public activation. Canonical policy/source/
model-artifact resolution, required knowledge coverage and quality review remain
caller requirements. Native transport limits still apply; no silent truncation
or batching shortcut was added. Final semantic prefix/dynamic partition,
composite/chat fit, unconditional pre-dispatch limits, Sources jobs, Redesign,
serving/cache and deployed-model quality remain open in the prefix plan. Neither
these tests nor an oversized source-only diagnostic close those gates.

## Independent coverage-floor comparison successor

The existing pure `prefix_design_stage` library additionally accepts `coverage`.
It compares all declared size bands with a separately supplied profile floor,
including dependency-only required groups and newly captured namespace members.
The design cannot satisfy this comparison by weakening its own `required_coverage`.
The report retains all exact inputs; it authenticates neither their origin nor
their source hashes. A trusted floor resolver and the accepted Generate/evaluation
caller remain open; the six-step candidate Recipe is unchanged.

Observed on the successor source:

- All **13** standalone real-tokenizer tests pass, including two new coverage
  regression cases with profile, missing-source, empty-floor, hidden-band,
  overlapping-band, weakened-requirement and new-component variants.
- **Three actual native utility-worker operations pass**: the exported coverage
  phase agrees with the CPython fixture; weakened proposed requirements and a
  missing obligation in a later band both raise the intended error in Monty.
  Requests, replies, stderr and source/worker hashes are retained in
  `target/accepted-evidence/prefix-coverage-20261024/native-utility-run-1/`.
  This uses the previously archived worker with the SHA recorded above, through
  its existing framed utility protocol. It does not claim a newly compiled
  worker/application pair or component admission/approval.
- Python syntax, focused Rust formatting and `git diff --check` pass.

The successor Cargo integration and strict affected lints now **pass** under
`target/accepted-evidence/prefix-coverage-20261024/assembly-run-2/`:

- Both actual integration cases execute and pass with zero ignored/filtered:
  exported-stage Monty/CPython agreement plus native tokenizer selection, and
  retained candidate Recipe/native-binding/dependency/preload assembly.
- Strict Clippy passes for the Composition integration target with its captured
  features and separately for the default Skills library, both `-D warnings`.
- All 13 standalone tests, focused formatting, final source-integrity checks and
  whitespace checks pass. The immutable `accepted.json` records actual commands,
  results, executable hashes and worker provenance. No live model/serving
  qualification or new worker/application pair is claimed.

Assembly run 1 stopped before compilation at the host volume's mandatory 93%
disk-space gate. Run 2 verified previously archived executable evidence and saved
the remaining cache test executables before a capacity-triggered cleanup of the
disposable canonical NVMe dev cache. The repository evidence tree was untouched.
After successful integration and Composition lint, the disk gate stopped the
last Skills lint; the retained resume runner cleaned only the now-unneeded
Composition cache, then completed the remaining checks against unchanged source.
The successful integration was not replayed. Both original runners, source
inventory, raw logs, disk observations and native executable remain retained.


## Profile-derived design inputs — shared successor acceptance

The existing source-compiler library now derives a coverage floor and included
model-design inventory from its own per-profile requirements through the pure
`design-inputs` stage. It recomputes `prefix_prepare` from the exact original
snapshot, consumes the existing complete ordered digest batch, preserves the
unchanged snapshot with excluded originals, and accepts no proposed-design
requirements or caller-edited intermediate state. The typed stage constructor
is available alongside the existing compiler stages; no policy database or
provider workflow was added.

Shared acceptance is retained in
`target/accepted-evidence/k3-optional-startup-prefix-20261024/run-4/`.
The accepted receipt SHA-256 is
`c8886ce7ec728626791a85e2951106e7f122dca6c8169934a9effb1778842f75`.

- **Three actual Prefix integration cases pass**, zero ignored/filtered:
  current captured-guide floor/source projection through Monty, whole-unit
  selection/native Rust tokenizer agreement, and retained candidate Recipe/
  native-binding/dependency/preload assembly. The new stage's typed declaration
  is constructed; this is not execution of a newly activated Generate graph.
- **Four compiler tests and 13 selection tests pass.** The compiler case covers
  all three supported core profiles, exact native-batch hash positions, a new
  included component, archived/excluded originals, prohibited requirement
  overrides and incomplete digest batches. The stage does not itself
  authenticate native-result custody or captured-source provenance.
- The **actual complete HTTP/PostgreSQL Prefix consumer passes in 345.30s**:
  319 current sources, 8,970,577 snapshot bytes, correlated native calls and
  reconnect without replay. The independent Sempai diagnostic is 372,043 tokens
  against the retained 262,144-position configuration, remains `fit=false`, and
  produces no design acceptance or serving activation. Neither number changes
  the canonical operator prefix/prompt limits or proves effective server capacity.
- Strict affected Root/Composition/Skills and default Composition lints,
  formatting and source integrity pass. The shared package records **48 freshly
  executed Rust cases plus one explicitly reused Startup result**. That reuse
  is a separately qualified 193.83s run-2 startup result; it is not a newly run
  run-4 test. The actual 96.47s routing/restart case demonstrates provider-free
  ordinary Match with both optional model roles unavailable.

An earlier direct archived-worker check also agrees with CPython for the new
stage; its raw request/reply/source identity is retained under
`target/accepted-evidence/prefix-design-inputs-20261024/native-utility-run-1/`.
It remains scoped to that historical worker and does not claim a rebuilt pair.
Shared runs 1–3 retain their failed build/test-fixture diagnostics. Only run 4
is the completed package receipt.

Remaining work: bind the authenticated source capture, exact native digest
results, derived floor and every-band comparison into the target-aware private
draft evaluation and accepted Generate workflow. Current source-only jobs and
constructor/projection checks do not supply that wiring or a trusted design
acceptance. Profile parity outside the three core profiles, quality review,
composite/final-envelope checks, serving/cache qualification and deployment
remain open. No new compiler/model call needs to be rerun solely to update this
nonembedded status report.

## Fixed-rule/current-content resolution successor

The final source additionally implements
`brassclaw-prefix-generation-rules/1`. Exact file/namespace/complete-unit selectors
resolve the current captured inventory into the existing unit-design contract.
Rules and concrete resolutions retain separate hashes/bytes. Every declared
selector must match; required coverage must belong to each band's mandatory
dependency closure. There is no regex execution, body slicing, inferred rule
change or implicit latest catalogue read. Authoritative source/profile coverage
and accepted-design binding remain separate required caller gates, not facts
established by a proposal's own declarations.

The successor archive is
`target/accepted-evidence/prefix-design-selection-20261024/rule-successor/`.
Its `identities.json`, exact `sources/` copies, executable/worker, `build.log`,
`python-tests.log`, `tests.log`, `clippy.log` and `scope.json` identify the actual
checked successor. Direct source hashes were rechecked after execution.

- **Eleven CPython tests passed**, including the earlier eight plus fixed-rule
  component growth/identity/input-order/namespace isolation, newly uncovered
  mandatory sources and malformed/missing source selectors. Do not add the old
  eight again as separate final-source coverage.
- **Two integration tests passed**, zero filtered, using the same actual Monty,
  Rust tokenizer and retained graph/preload interfaces. The measurement case now
  uses generation rules and additionally resolves a new mandatory component in
  Monty while retaining the exact original rule bytes.
- The same Cargo integration build command/features/profile passed. Targeted
  Clippy completed with one unrelated `catalogue_publication.rs:196`
  `collapsible_if` warning in the parallel catalogue work. The earlier strict
  batch failed on that warning class; no strict-success claim or lint suppression
  is introduced. Source changes in that parallel owner are outside this receipt.

This supersedes the older source identities for the current selection/resolution
prerequisite. It does not close accepted-design storage/bindings, source capture
completeness, bootstrap/general pure approval, ordinary target-aware Generate,
final request guards, WebUI actions, serving or deployed-model/cache quality.

## Versioned proposal successor — 24 October 2026

The existing model-design store/API now retains
`brassclaw-prefix-design-proposal/2`: an unaccepted `whole-unit-reference/1`
declaration containing the reusable generation rules. Historical v1 reads/write
checks remain effective. The new source-independent Rust checker validates all
bands, canonical selectors, required groups and dependency graphs; adjacency/
in-degree traversal handles a reverse-ordered 1,001-group graph without a cutoff.
The current full-reference diagnostic explicitly rejects v2 before durable job
admission. Its WebUI explains why those proposals remain listed under model
designs but cannot be selected there. No table, settings owner, provider call,
acceptance writer or new approval exception is introduced.

The combined K2 run reused its earlier unaffected catalogue/child cases and
freshly compiled/executed this successor's actual affected consumers. Reuse
these checks rather than rebuild them:

- Two rule-declaration tests and two versioned-proposal tests passed.
- Both actual Monty/tokenizer/retained-preload integration tests passed, including
  the native declaration check against the same reusable rule fixture.
- The existing complete HTTP/PostgreSQL deployment/prefix caller passed in
  344.65 seconds. It now checks v1/v2 immutable read/retry, rejected cycles and
  absence of a definition/admission for v2 in the legacy measurement workflow.
- Ten affected measurement-panel tests pass within the 21 fresh shared UI tests;
  the other eleven belong to the parallel catalogue/deployment workstream.
- Strict affected Root/PG/Skills/Composition/Product/WebUI test-feature Clippy,
  default Composition Clippy, formatting and whitespace checks pass. The earlier
  unrelated lint failures remain genuine historical failures, not retroactive
  successes or suppressed warnings.

Evidence: `target/accepted-evidence/k2-catalogue-publication-20261024/run-6/`.
`accepted.json` SHA-256:
`45762253e1c57e66adfc66365ced0f25e9b491f1e9427c1640c1052e47525326`.
The source inventory and all ten integrated Prefix runtime/UI file hashes were
independently compared with the current files after completion. Original
executables/worker, retained commands/results and exact reused-evidence mapping
are in that archive. This receipt uses Rust 1.96.0; the earlier standalone
selection receipt used 1.98.1. It is not a final regular application/worker release
or a testmachine deployment.

**Raw-log limitation:** this actual runner used `prefix-design-integration` as
both build and test labels and opened their shared log with replacement mode.
The surviving log contains the two real passing tests; the preceding build's
raw diagnostics were overwritten. Its build command/zero return code and exact
executable hash remain recorded. Do not claim that every original build diagnostic
or linker warning was preserved, reconstruct lost output, mutate the historical
runner or rerun unchanged tests merely to disguise this loss. Future runners must
use distinct build/test log identities and exclusive creation. Strict lint results
are retained separately and remain actual passes.

The fresh complete Sempai diagnostic measured 371,505 tokens and remained
capacity-blocked/not activated. This is an observation for that exact captured
source/catalogue, not a new limit or a successful target-aware prefix. Independent
authoritative source/profile floors, accepted model/consumer bindings, qualified
draft evaluation/bootstrap, ordinary Generate, final envelope guards and serving
remain unfinished. The plan now explicitly orders quarantined draft generation
before acceptance: V145 requires an actual candidate generation, so normal Generate
must not become its own acceptance prerequisite.

## K3 Prefix UI handoff — scoped package passed

The shared Provider/Prefix consumers now refresh the existing canonical provider
query instead of publishing an unversioned mutation response into its cache.
Comparing only the edited provider's revision cannot order another provider or
role selection. The captured earlier UI reproduced both a revision-10-to-9
rollback and an old Sempai selection replacing a newer selection at the same
provider revision. The successor was tested with the actual shipped
QueryClient/QueryObserver; only transport replies and React-hook scheduling were
controlled. This is UI behavior evidence, not PostgreSQL or model-uptake proof.

Read and Reconcile preserve original commit evidence separately from canonical
read failures. The model-limit editor also displays a known committed receipt
when runtime reload fails, refreshes desired limits through the same query and
does not replay the write. Confirmed persistence signals dependent model-design
views through the existing notification owner. The readonly Prefix Model Designs
consumer fences superseded catalogue successes/errors so a late response cannot
put an older provider revision into the next Capture request.

The shared owner integrated the minimal patches into its newer committed/unknown
and selected-role work; older staged complete files were not copied over that
work. All **26 current UI cases passed**: seven model-limit editor cases, four
receipt-status cases, five model-design catalogue cases and ten original-mutation
transport cases. The earlier captured comparison diagnostics and exact source
identities remain under `/private/tmp/prefix-provider-cache-successor/`; their
red/green outcomes do not stand in for the integrated package's native acceptance.

The integrated native/strict-lint package passed in
`target/accepted-evidence/k3-provider-originals-20261024/run-7/`: 38 fresh Rust
results and 19 exactly qualified run-6 results, the 26 current UI cases, seven
syntax checks, strict affected/default lints, formatting and final identities.
The complete actual HTTP/PostgreSQL Prefix caller passed in 343.28 seconds.
`accepted.json` SHA-256 is
`bdf40f057010bc4be4861521bfe5ed6b88b6cf896f258e80d3418b7dd35e673f`.
All four affected UI runtime source hashes were independently compared with
that source inventory after release. Earlier failures, source snapshots and
original executables remain preserved; reuse is not presented as fresh execution.

The shared owner explicitly released this package's source/Cargo freeze. The
earlier K2 receipts remain immutable and apply to their own snapshots. This K3
receipt qualifies original manual publication/readback/reconciliation and its
current consumers, not actual provider-client revision acknowledgement or live
dispatch fencing. Those and the complete target-aware prefix pipeline remain
separate unfinished work. This does not close K3 as a whole or the Prefix plan.

## Shared provider-consumer successor — scoped prerequisites passed

The [K3 consumer report](2026-10-24-k3-provider-consumers.md) now qualifies the
atomic client/publication snapshot correction, async retention preparation,
factory compatibility checks before cache and raw retry attempts, live cache
policy and late-fill fencing. These are existing shared owners to consume,
not Prefix infrastructure to duplicate. The initial 120 passing Rust cases are
separate from three initially ignored model cases: their zero-execution result
is not counted as a pass. All three were subsequently executed explicitly with
`--ignored` against the verified Ornith endpoint and passed in 91.807 seconds.
Thus 123 Rust cases, 43 UI cases and ten syntax checks have actual scoped
acceptance, alongside strict affected/default lints and final identities.

Evidence is `target/accepted-evidence/k3-provider-consumers-20261024/run-1/`.
The independently checked `accepted.json` SHA-256 is
`6cabe2e4b990a735f69b8c1de1a11168abc21c1963f07d6022a5e52b345efbea`;
the checked supplemental `live-model-consumer-results.json` SHA-256 is
`7e203d4c4df039485932148738644819764b2a5f12c26986fcc11d0e1040d67a`.
The actual complete HTTP/PostgreSQL Prefix caller passed in 342.701 seconds.
Its complete Sempai diagnostic is 371,747 tokens with fit=false and no
truncation/activation; model discovery confirmed 131,072 tokens. These are
observations for their exact target/source snapshot, not new editable limits.

The shared owner released this compiler-input/Cargo freeze. K3's remaining
embedding/session/late-claim/mutable-model and desired/effective boundaries stay
open, as does Prefix's distinct final semantic envelope/token-size guard. The
successful short native model cases do not establish full-prefix capacity,
design acceptance, cache-state correctness or model-quality qualification.
