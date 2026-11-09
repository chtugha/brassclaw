# Development package board

Latest user direction, reiterated 2026-11-04: a supervisor and at most two implementing
package chats, a fresh chat for each new package, one shared Modal compiler.
The [development policy](development-policy.md#shared-source-and-central-build-ownership-2026-10-30)
defines the workflow. This board is development coordination, not a new product
queue, component approval or evidence of completed Bob plans.

## Responsibilities

- Supervisor: `/Volumes/SSDE/brassclaw` on `main`; package definition, architecture
  review, native tests/final acceptance, integration and release. No new source
  implementation. Defects return to their package chat. Existing source written
  before this direction retains its real tested/untested scope.
- Package chats: their own isolated worktree/branch; mandatory architecture and
  owner-plan reading, implementation, self-review, tests as source, eligible
  Modal compilation and exact commit/receipt handoff. No local Cargo, remote
  tests, main edits/push or nested delegation. Each handles only its package and
  its review corrections; the next package receives a new chat.
- Modal: existing activated Apple image `im-S1cpT9G1fsMPpwjISvBSlK`, environment
  `brassclaw`, volume `brassclaw-build`. Use the existing controller and shared
  `/private/tmp/brassclaw-modal-build.lock` for the entire synchronization/build/
  receipt lifetime. No parallel remote job or standalone competing upload.
  Keep existing cost/activation policies; no provisioning or speculative runs.
- Local validation: supervisor only, one Cargo execution at a time under
  `/private/tmp/brassclaw-target-cargo.lock`, configured canonical target, Screen
  for long jobs. Immutable compiled artifacts can serve multiple selected cases.
  No package integration into captured inputs during compilation.

Every package must read `AGENTS.md`, `CLAUDE.md`, `recipe.md`, `skills.md`,
`tools.md`, `toolskills.md`, `path_to_v3.5.md`, the development policy, applicable
crate guides and its relevant Bob/owning plan sections before implementation.
Its first report states the relevant invariants and actual reused owners.
Its handoff names base/final commits, changed contracts, genuine compilation
receipts, unexecuted tests, known gaps and proposed exact native test selectors.
The supervisor accepts the connected behavior, not a successful build alone.

## Packages and exact acceptance

### Current integration checkpoint — 2026-11-09

**Run-24 — 2026-11-09:** exact Main `b9620a1f5` compiles the current worker
and complete six-library/four-integration graph without warnings (7.136/644.104 s).
Actual source, profiles, eleven retained artifacts and link checks pass;
`__eh_frame` is 12,956,976 bytes and all 1,388 Composition names are accounted for.
The F-01 strict receipt-consumer regression and both I-01 real operator HTTP/PG
original-custody cases pass, zero ignored. Common global startup fails after
190.685 s with `original observation I/O deadline elapsed; outcome unresolved`.
No dependent Source/runtime cases start. The exact bounded operation is not
established by this generic error; F-01 diagnoses the connected original-proof
path before correction. No larger timeout or weakened verification is accepted.
Source/HEAD and every retained executable are verified after the run; the freeze
is released. See `run-24-composition/native-batch-disposition.json` in the existing
validation evidence directory.

J-01 continues its isolated V211 single-case runtime package. K-01, fresh chat
`01a1c10c-3ba1-7a22-be23-4e8885c3f645`, worktree `24b2`, is **read-only proposal**
for B2-06 snapshot/dependency quotas through existing acknowledged preparation
settings and historical readers. No K-01 implementation or migration is reserved
yet. I-01 remains available for its own findings; F-01 correction and J-01 are
the two implementation slots when the concrete correction is authorized.

**Run-23 and connected source checkpoint — 2026-11-09:** worker and complete
six-library/four-integration test graph compile without warnings (0.823/623.518 s).
Actual profiles/source/artifact/link checks pass: `__eh_frame` 12,842,716 bytes,
`__unwind_info` 2,468,088 bytes; all 1,376 Composition names are accounted for.
The first genuine common startup fails after 103.884 s with bootstrap
`InvalidPortResult`; no dependent native cases run. Its correctly closed private
PG rig leaves no claim available for later attribution. A deterministic F-01
factory defect on this rejection path is source-proven: later variant result
edges were renumbered while the associated receipt key was not, although the
expected manifest renumbered both. Reviewed repair `5922cba87` shares their
normalization and adds a real-consumer transport regression without relaxing
oracles or the strict reader. Precise failed-run claim attribution and native
success are not inferred from this source finding.

I-01's complete Source slice and review corrections are source-reviewed,
hash-verified and integrated through `7152f0e0a` (25 files). The reviewed common class-21/23
authoring follow-up is integrated as `d30a5126b`/`8b83d551d`; the complete
I-01 package now covers 28 files and eleven new native test sources. V210/outer selection
`/2` carries the exact qualified W/checker/parser/worker/package pins. Authoring,
original typed capture and Ready share one transaction; actual parser observations
and H-01 structural confirmation are connected. Uncertain connection disposal,
bounded one-write/fresh-readback with all run-index fields, same-snapshot proof
reuse and genuine fault/smaller-limit/upgrade test sources are reviewed. This
source remains uncompiled/unrun. All three queued-authoring classes use the same uncertain-connection custody
owner, with actual mounted HTTP ACK-loss/caller-abort regression sources. Cross-package historical qualification/image
loading remains B2-06/07/09 work, not a schema-upgrade acceptance claim.

J-01 now implements the reviewed registered single-Pure-invocation primitive
and normal ToolSkill/Skill-code/independent encoder/private Harness Recipe under
reserved V211. Same-global-worker/budget, native exception/context release and
normal H-01 Harnesssuccess delivery lead only to a technical settled Case.
No new non-UUID slot, Rust case loop, whole-task failure-delivery, general
behavior/Q1/Q2/approval/publication is authorized by that package. Its source and
acceptance remain pending; I-01's positive reader stays test-only until that
real consumer is connected. All three full Bob plans remain open.

Earlier scoped checkpoints follow; their dates and source/artifact scopes remain
historical where superseded by this checkpoint.

**Run-22 follow-up — 2026-11-09:** worker compilation passes in 0.881 s.
The joint graph reaches production and test branches, then stops after 247.24 s:
four E0425 sites share one missing H-01 test-runtime import; production reports
five unused historical waiter APIs. The two actual ProductWorkflow/inbound
harnesses pass **93/93, zero ignored**. This complements Run-21's independent
453-case acceptance, without claiming a passing Composition build. Source/HEAD
and artifact identities are verified. Reviewed H-01 corrections `56a49ea22` and
`aebdd70ea` scope only genuinely test-only consumers and add explicit imports;
productive post-Factory proof creation/delivery and producer custody remain.
The corrected baseline needs actual shared startup/native acceptance before new
Source/Behavior consumers can claim qualification. I-01's delivered Source slice
is under connected review: unknown-COMMIT connection custody, exact run-index
readback and repeated same-snapshot proof checks have concrete corrections.
J-01's revised read-only design uses typed native case outcomes with a normally
completing qualified Harness, preserving ordinary fail-stop and H-01 delivery.

**Run-21 follow-up — 2026-11-09:** the worker builds in 13.44 seconds.
The joint test graph stops after 132.63 seconds with two API errors and one
unused production import. Its independently produced Turns, Reborn and
loop-driver caller executables pass **453/453 actual tests, zero ignored**;
this is not Composition acceptance. Source/HEAD and artifact hashes are verified.
The H-01 import/visibility corrections are reviewed and integrated as
`515035298`/`a474c1740`. F-01's complete original-proof transaction correction
is reviewed and integrated as `9d807f6fc`: preserve the genuine two-phase Turn
custody, verify before acquiring W's client, then recheck all original facts in
its RR snapshot without a nested pool checkout. Native/primitive/worker/admission
verifiers are unchanged. A single corrected joint build is next; no F-01/H-01/
V209 runtime or full Bob completion follows yet.

I-01 is implementing qualified Pure Source selection/original admission under
reserved V210 and `component-validator-selection/2`, without behavior/Q1/Q2
approval. J-01 is a read-only Behavior/Human-Q2 design package. These reuse the
existing owners; new source is isolated from the supervisor's build inputs.

**Latest reviewed integration:** E-01's supported-settings test correction is
integrated as `0039b365e`. F-01's complete 15-file checking-function/support/Source
workflow package and real Store boundary tests are reviewed, hash-verified and
integrated as `48eec00f6`/`7b4c29596`. F-01 has no compilation or native acceptance
yet. Its scope is independent shipped checking helpers and their exact workflow,
not general candidate Pure approval, authored Q2 or successor publication.

**Run-20 native receipts:** the current worker and joint Composition/Memory/PG
harness build on `377ad1bec` pass without warnings (0.861/654.546 seconds).
Profile/source/link checks pass; `__eh_frame` is 12,566,888 bytes and all 1,368
Composition test names are accounted for. The real common PG/Monty startup passes
in 106.64 seconds. Five E-01 ACK/cancellation/worker-death cases, early zero-approval
boot, both A-03 generation cases, both D-01 PG codec cases, the opaque global-driver
case and PG transport validation pass. One E-01 test incorrectly requests depth
65 above the supported 64; its reviewed correction is integrated, awaiting rerun.
Six Prefix/G-01 cases fail at the private-Work SQL boundary. PostgreSQL reports
`operator is not unique: unknown - unknown`; six unparenthesized nested JSON
extraction/minus expressions in three V207 functions are the confirmed cause.
The rollback test also returns green, but its generic error assertion cannot
establish all four intended fault points. Its real trigger reachability is being
strengthened. All failed and limited receipts are preserved; no full package
acceptance follows from those green results.

B-02's exact six-expression V209 repair and genuine upgrade/trigger-reachability
regression sources are reviewed and integrated as `e9f9e1e8c`. V207 bytes are
unchanged. H-01's complete 39-file delivery/confirmation/discovery package is
reviewed, hash-verified and integrated through `103b3a444`. V208 retains actual
post-Factory delivery and structural original confirmation. Connected review
corrected strict repeatable-read transactions, one-write/fresh-readback handling
of unknown acknowledgement, and the distinction between an original actor and
the storage operator identity. The old unqualified five-head Q1 producer is
restricted across its connected graph to tests; positive structural observation
stays State 1. No general Q1-ready producer or approval is asserted.

The next joint native build covers F-01, E-01's supported-settings correction,
H-01, V209 and affected Turn/Reborn/ProductWorkflow callers. All new native
cases remain unexecuted. I-01 has supplied a read-only proposal for qualified
Pure Source selection and the missing general Behavior/Q2 contract; supervisor
review is pending. H-01/F-01/B-02 remain idle for concrete own corrections.
V208/V209 preserve old migration bytes and native Terminal `/1` semantics.

Earlier Run-19 checkpoint (retained historical receipt):

**Native follow-up, 2026-11-08:** Run-19 on `381706153` builds the current worker
and Composition/Memory tests successfully in 0.569/588.585 seconds, without
compiler or linker warnings. Actual source/profile/link/inventory checks pass:
`__eh_frame` is 12,552,864 bytes; 1,364 test names have no unexplained losses.
The exact 47 D-01 retirements and one E-01 early-boot replacement are recorded.
The first actual startup prerequisite fails after 48.87 seconds at E-01's
expected Python exception check. No further dependent cases are started.
The source-proven pinned Monty KeyError-format correction `4b4ecb0ac` and
G-01's held-ACK test transport `6ad115a0b` are integrated, with native acceptance
pending. Earlier failed compile receipts remain preserved.

Run-17's three retained Memory executables pass 67 actual cases: 52 unit,
13 VFS and two new InMemory codec tests, with zero skips. Their separate
post-run acceptance verifies source and executable hashes. This evidence applies
to those exact executables and unchanged Memory code; freshly relinked Run-18
images are not claimed executed. PostgreSQL codec/global-driver acceptance is
still open. G-01 is idle for its own corrections. F-01 subsequently passed connected source review, including its private fixture
subject scope and real-reader negative tests, and was integrated as recorded above. No general Pure
approval, authored Q2, successor selection or full Bob completion is claimed.

Local `main` contains the source-reviewed A-03/B-02/D-01 chain, the bounded
B03a atomic Prefix producer through `576372fe2`, E-01 through `0e2b53c30`,
D-01's Memory correction `18183a653` and A-03's generation correction `bea5efa8a`;
the latest pushed accepted
checkpoint remains `68f8b798e`. Source integration is not native acceptance.
Run-14's combined test compilation failed on test-only consumers; their reviewed
corrections are integrated. Its six unchanged retained executables subsequently
passed 11 selected host/WebUI/HTTP/real SQLite-to-PostgreSQL migration cases.

Run-15 compiled its current worker and Composition harness successfully, with no
compiler/link warnings. Its immutable 70-case native batch passed 25 and failed
45; neither ignored nor empty selection counted as passing. Both explicit actual
Ornith model calls passed. The source/profile/unwind/test-inventory gate passed:
`__eh_frame` is 12,341,644 bytes; all 1,353 baseline tests remain except the exact
47 reviewed D-01 retirements, with 45 new tests. Failed receipts are retained.
Most failures stop at the shared installation readback; others expose the missing
contract-test receipt declaration, the old generation qualifier's artificial
frame reduction, and colon-bearing opaque memory attribution. A-03's first two
runtime corrections are integrated as `0cba4f04b`, still awaiting native rerun.

B03a `f686cfd226`/`83efab192` is reviewed and integrated as
`a9021aba9`/`576372fe2`: the actual private source-only Prefix producer commits
original, Work, complete preparation and Ready together. Existing originals get
strict readback only; historical absent/unsealed Work is not completed using
current data. It reuses the existing transaction-client owner and Worker, without
new schema, queue or preparation service. Its real ACK/rollback/historical native
cases are source-only pending acceptance. Validator original-to-Work discovery
and post-native domain acknowledgement remain open.

E-01's complete 19-file source package `8ff7abc478` is reviewed, independently
hash-verified and integrated as `0e2b53c30`, with native acceptance pending. It
replaces manufactured early checker approvals with actual contained own-export
qualification and mandatory original startup readback. Its small receipt helper
qualifies independently first; complete candidate-class Q1 remains open. The old
early eight-approval test is deliberately replaced by a zero-approval draft-boot
test, with actual behavior covered by the new native export tests.

A-03's final seven-file generation package is reviewed and integrated as one
corrected commit `bea5efa8a`. It uses the actual captured executor, binds original
fixture inputs before effects and reads its exact immutable subject before fresh
fixture creation. The rejected global history lock/scan is absent. D-01's full
Memory codec/attribution correction is reviewed and integrated as `18183a653`,
preserving original Host IDs, legacy address bytes and the existing short-path
compatibility alias. A-03, D-01, E-01 and B03a are idle for their own acceptance
corrections. Their later changes have no Run-15 runtime acceptance. The next
coherent native batch compiles the current worker plus Composition/Memory tests,
then checks common startup before dependent cases. No new paid Modal job is
running; no package chat runs local Cargo or tests.

Before Run-15, worker and required executables were hash-verified and retained
on the external evidence volume. Required four-package cleanup removed 11.2 GiB.
After Run-15, its current worker and executable were independently hash-verified
outside the canonical cache. Necessary Composition/Skills package cleanup removed
8.0 GiB, but still exceeded 90% disk use. Subsequent Engine-only cleanup removed
3.0 GiB, leaving 25.14 GiB free at 88.99% use, within both required thresholds.
Additional needed cached executables were preserved with hashes and explicitly
unattributed status. No whole target, source, PG data or retained evidence was
removed. Recheck actual space before the next compilation; cleanup is not routine.

| Package | Chat/worktree | Scope and state |
| --- | --- | --- |
| A-01 — native Codex/Auth custody | `01a1a022-90b1-7932-8dcf-c0ef34b79fee`, host `local`, title “Bob 3 — Native Codex/Auth via Modal”; `/Users/ollama/.codex/worktrees/e8ba/brassclaw`, branch `codex/bob3-auth-custody` | Accepted within the recorded source/native scope, integrated through `a63100515`. Run-6's 71 passing cases plus run-7's two corrected cases cover the selected 73-case matrix; eight-package strict Clippy passed. Actual OpenAI issuer, full product/platform/release and broader B3 gates remain open. Full mandatory reading confirmed. Package closed; no independent new task in this chat. |
| B-00 — review inherited Q1/Oracle repair and finish its acceptance fixes | `01a1a062-b1c4-7ba0-8506-b7c8b3ee4b33`, host `local`, title “Paket B-00 — Q1- und Oracle-Reparatur”; `/Users/ollama/.codex/worktrees/ac66/brassclaw` | Accepted in its bounded inherited repair scope: exact-source Modal CLI/native smoke, actual incomplete-execution regression and all six retained Q1 callers passed, along with Oracle/pure-link/shared-driver cases. Eight-package strict Clippy passed. General pure approval, canonical genuine qualification, durable scheduling and B2 cutover remain open. Package closed; no new task here. |
| B-01 — original immutable review request | `01a1a0d0-de89-7a73-9fd2-af06d5d1ca81`, host `local`, title “Paket B-01 — Originale Review-Anforderung”; `/Users/ollama/.codex/worktrees/2f1e/brassclaw` | Accepted in its bounded scope through `5c5e84bf3`: exact original/CAS/fault/readback, complete queue projections/paging, actual Q1/Sempai/lifecycle cases, 24 UI tests and strict twelve-package lint. Modal CLI/native smoke also passed. V206 integrated. Full `/3`, legacy custody disposition, complete Sempai/Docplan conversion and durable Work remain open. Package closed, no next task here. |
| A-02 — exact Pure-approval readers | `01a1a139-eac1-7e32-9ac9-5ddfff64994a`, host `local`, title “Paket A-02 — Exakte Pure-Approval-Reader”; `/Users/ollama/.codex/worktrees/27f9/brassclaw` | Narrow A-02a `f62e17a12` integrated as `9c8d3c12a`, scoped acceptance complete: exact canonical `/1` prepare/recheck/reload, occurrence isolation, ordinary publication, retained child, complete deployment caller and strict affected lint. No new general Pure approval, migration or selection format. Package closed; successor gets a fresh chat. |
| A-03 — genuine Pure approval and exact retained successor contract | `01a1ab08-ac3e-7812-bf0d-ba6afcc18fb0`, host `local`; `/Users/ollama/.codex/worktrees/a7f2/brassclaw` | Narrow A-03a source and corrections through `a90363b432` are reviewed and integrated. Run-15 compiled but its selected runtime acceptance failed. The writer-spelling/receipt fixes await rerun; the final generation-frame/original-configuration package is source-reviewed and integrated as `bea5efa8a`. Idle for its own native corrections. Real parser/Structure/Pure facts remain prerequisites, not general Pure/Q1/Q2/activation or `/2` publication. Mandatory reading complete. |
| B-02 — durable private Monty Work | `01a1ab4e-ae8c-7342-98a3-7b6b8ab50650`, host `local`; `/Users/ollama/.codex/worktrees/d230/brassclaw`, branch `codex/b02-private-work` | Connected source and corrections through `aacf4f291` are reviewed and integrated: V207, genuine private structural/Q1 and source-only Prefix consumers, shared driver/Worker, exact captures, original readback, bounded cancellation/attempt custody and live completion delivery. Native acceptance pending; idle for own concrete corrections. No absent-Work discovery, old-image proof restoration or general Pure/serving claim. |
| C-00 — native test unwind/linker repair | `01a1acab-875c-7073-9ddc-b84b4cd73443`, host `local`, title “Paket C-00 — Native Unwind- und Testlinker-Reparatur”; `/Users/ollama/.codex/worktrees/4d31/brassclaw`, branch `codex/c00-test-unwind-profile` | Accepted in its bounded profile/link/caller scope. Source `36ce1c4c5` integrated as `e212e0ba2`; actual native Run-11 links without warnings, retains unwind/debug/overflow checks, has 11,978,176-byte `__eh_frame`, preserves all 1,353 prior test names and passes both selected real PG callers. Package closed; no independent new task here. A changed A-03/B-02 graph needs its own checks. Full B3-12 remains open. |
| D-01 — retire the private legacy Engine/Store bridge | `01a1ad6e-f507-78b2-883f-c3078c5c3678`, host `local`; `/Users/ollama/.codex/worktrees/9920/brassclaw`, branch `codex/d01-private-engine-bridge-retirement` | Source `f30dbcf6a` is reviewed and integrated. Removes the unreachable private bridge and solely dependent adapters/builders/guard, unused Store argument and unregistered LibSQL adapter. Preserves public compatibility, ordinary Monty and actual migration/import readers. Exact 47 retirements verified. Run-15 passes the real no-Monty Worker case; startup is blocked by shared bootstrap and opaque scope fails the real memory attribution codec. The diagnosed Memory-owned codec/attribution correction `b16976c41` is source-reviewed and integrated as `18183a653`, with real backend test sources and legacy compatibility retained; native acceptance pending. Idle for own corrections. Broader B2-09, private composer/seed alignment and Engine codec/suppression migration stay open. |
| E-01 — genuine shipped seed qualification | `01a1b0d3-5a16-7932-86b1-786b2906224f`, host `local`, title “Paket E-01 — Echte Seed-Qualifikation”; `/Users/ollama/.codex/worktrees/ac2c/brassclaw` | Complete 19-file source `8ff7abc478` reviewed, independently hash-verified and integrated as `0e2b53c30`: exact canonical eight checker roots plus independently qualified receipt support, real contained fixtures/settlement, strict original proof and mandatory startup readback. Native acceptance pending; idle for own corrections. No general class-Q1, remaining 24-root approval, successor publication or migration claim. |
| B03a — atomic private Prefix original/Ready publication | `01a1b10a-5a89-7712-a37c-f77f5a66c70f`, host `local`, title “Paket B-03 — Atomare Review-Arbeit”; `/Users/ollama/.codex/worktrees/dddf/brassclaw`, branch `codex/b03a-prefix-original-ready` | Four-file source package and existing-client lifecycle correction through `83efab192` reviewed and integrated. No schema/Worker/validator change. Three new real native cases and updated existing fallback/claim/readback cases await execution. Genuine withheld-BEGIN cancellation requires the existing test transport's hold/release extension before full B3-11 closure; its current cut-connection cases do not prove that boundary. Idle for own corrections. |
| F-01 — closed Pure-checker/support qualification | `01a1babb-0ccd-7ed2-8211-f5a6a9f29976`, host `local`, title “Paket F-01 — Vollständige Pure-Eigenprüfung”; `/Users/ollama/.codex/worktrees/1a9e/brassclaw` | Full architecture/source proposals reviewed. Authorized connected implementation of ten existing public checking functions, separate private reader support and their Source workflow, reusing existing original-execution custody. Two private fixture identities reserved. The genuine E-01 common prerequisite passed in Run-20; source `0dad53132`/`ce00c3dec` is integrated as `48eec00f6`/`7b4c29596`. Compilation/native/lint acceptance remains pending. No general candidate Pure approval, authored Q2, `/2` publication or migration authorized. |
| G-01 — actual held transaction acknowledgements | `01a1bac3-1f9a-77e3-86ac-e2ee132fb953`, host `local`, title “Paket G-01 — Echte Transaktionsabbruch-Abnahme”; `/Users/ollama/.codex/worktrees/d159/brassclaw`, branch `codex/g01-held-transaction-acks` | Complete two-file source `dc19b6397` reviewed, independently hash-verified and integrated as `6ad115a0b`. Extends existing Cut transport with real Hold/Release and adds actual producer cancellation/reuse/shutdown/native-completion test sources. No production policy or schema change. Native and strict affected lint acceptance pending; idle for own corrections. |
| H-01 — durable original review discovery and native delivery | `01a1bbf8-4da9-7e50-b16e-22adaea2a83a`, host `local`; `/Users/ollama/.codex/worktrees/527f/brassclaw`, branch `codex/h01-original-work-custody` | Complete connected source and transaction/actor review corrections integrated through `103b3a444`. V208 delivery/confirmation, historical readers, Prefix/validator consumers and existing Worker reconciliation are source-reviewed. Native compilation/acceptance pending. Positive structural result remains State 1; no full qualified Q1 producer. Idle for own concrete corrections. |
| I-01 — qualified original Pure Source review | `01a1bef8-0c72-74f1-899c-a7e4b84b0633`, host `local`; `/Users/ollama/.codex/worktrees/5b8c/brassclaw` | Connected 25-file source reviewed and integrated through `7152f0e0a`; V210/outer /2 and real Source observer/confirmation, metadata custody corrections and nine native test sources. Compile/native acceptance pending. Common class-21/23 authoring custody correction reviewed and integrated as `d30a5126b`/`8b83d551d`; full 28-file package awaits compilation/native acceptance. Positive future-consumer view test-only until J-01. No general Pure approval or cross-package loader. |
| J-01 — Pure behavior and human Q2 contract | `01a1bfbb-8d29-75b1-9946-9c2cb8ad14ac`, host `local`; `/Users/ollama/.codex/worktrees/8147/brassclaw` | Reviewed registered single-invocation Tool/ToolSkill/Skill-code/encoder/private Recipe package authorized, V211 reserved. Implementing sealed same-task native context/outcome/release, real qualification and Harness delivery to technical settled Case. No general case iterator/behavior/Q1/Q2/approval/publication. |

Migration ownership: V203 (exact pure links) is integrated; V204 belongs to A-01
(filesystem Auth cutover) and is integrated. V205 is reserved exclusively for
A-01's snapshot-safe custody successor; do not modify applied V203/V204. Coordinate
any later migration with the supervisor before using its number. V206 is reserved
for B-01 if its reviewed original-request contract needs a migration. B-00 requires no migration by default;
it must explain any actual schema need before implementation.
A-02a uses no migration. V207 is reserved exclusively for B-02's reviewed
connected private-Work/Origin/capture/guard successor. A-03a has no migration
allocation. Further schema changes require a distinct reviewed reservation;
no allocation permits editing or reinterpreting prior migration bytes. V208 is
reserved exclusively for H-01 delivery/confirmation; V209 exclusively for B-02
parenthesized JSON guard repair. H-01 does not replace V207 function bodies;
B-02 preserves their exact meaning while repairing the six precedence defects.

## Current native evidence and handoff

- First batch `target/validation/bob-native-repair-20261103/run-1/`: one compile,
  17/17 selected native PG/HTTP/Q1/driver/Auth tests passed. Commit `548254e43`.
- Second batch `.../run-2/`: compile succeeded; 18/19 selected cases passed.
  The sole failure was the new foreign-result fixture missing V102's required
  format/ID before task execution. It is corrected in `a534c4654`; the corrected
  positive case and later Oracle changes await native validation.
- Third attempt `.../run-3/` stopped at the disk guard **before Cargo**; it is
  not compilation evidence. Needed test executables/worker were already retained.
  Serial package-only cleanup removed 9.1 GiB. Afterwards: 29 GiB free, 86% shown
  by `df`; no repository evidence or whole target was cleaned.
- Fourth batch `.../run-4/`, source `4cebad6b6`: Composition/LLM compilation
  succeeded in 479.73 seconds. Disk guard stopped the following Auth compilation
  before Cargo. After retaining both actual executables and worker, package-only
  cleanup removed 4.8 GiB; the Auth contract executable compiled in 6.15 seconds.
  `continuation-1/` rejected an ignored private cross-process worker in its test
  selection before execution. This was a harness rejection, not product evidence.
- `.../run-4/native-tests-1/` reused those exact hashed executables without Cargo:
  **59/69 selected native cases passed**, ten failed. All six retained Q1 cases,
  Oracle, exact pure links/public-write restrictions, driver lifecycle, LLM host
  custody and Auth no-send contract cases passed. Two Auth migration tests failed
  on invalid fixture master keys, five new Codex-refresh cases on publication
  `Unavailable`, and three existing Device cases on missing original/conflicting
  readback. Exact names and logs are in `selection.json`, `test-progress.json`
  and `completed.json`; all ten belong to A-01 for diagnosis. No ignored or
  zero-selected test counted as passed. No full-package acceptance is claimed.
- B-00 Modal job `cf5e67e0-597d-4cc3-ae4a-0854fccb0f5c` compiled `f62e911cf`;
  its retained CLI SHA-256 is
  `809e6ff0648b2aa701e806d1cc9ce4215d0229f83962a499a83b1353e8573d76`.
  Supervisor `accept-native` passed actual Mach-O/dependency inspection and
  `--version`/`--help`; it ran no Cargo or cloud job. The existing job directory
  retains `native-accepted.json`. This does not compile its new test source or
  establish full product acceptance. See the exact-source
  [B-00 report](validation/2026-11-03-b00-package-review.md).
- `.../v205-sql-review-2/` independently executed captured V205 function SQL on
  isolated local PostgreSQL. Three fixed-snapshot insert/transfer cases failed
  with required `40001`; matching owners succeeded and a visible competitor
  failed `55000`. Exact V205 SHA-256:
  `c36c5c7c16b067bc0dbf2b56467fa6dc260670ad5cd54500cdda819e96a35ce0`.
  This verifies row-lock semantics, not Rust import/trigger attachment/upgrade.
  The preceding sandbox attempt failed on shared-memory creation and remains a
  separate failed setup record; the authorized rerun stopped its cluster cleanly.
- Fifth batch `.../run-5/` compiled source `a4f308dc4` for 207.08 seconds and
  stopped at missing test-local `AuthProductError` qualification. No selected
  tests ran; no successful Composition artifact is claimed. A-01 supplied the
  reviewed one-line correction, integrated as `74fe7b1cc`; the next run reuses
  the canonical Cargo cache without cleaning it.
- Before run-6 the disk guard required another package-only cleanup (1.8 GiB).
  Run-6 captured `a64d9d93e`, compiled Composition/LLM in 473.36 seconds and
  executed 73 exact native Composition cases: **71 passed, two failed**.
  Total batch 961.51 seconds. New Q1 incomplete-execution classification and all
  six retained Q1 callers passed. Auth's pending-factory fixture expected
  `Completing` instead of actual `CallbackReceived`/`Submitted`; its Device
  custody fixture incorrectly deserialized privileged host `RuntimeKind::System`.
  The supplied test corrections preserve production custody/identity rules.
  No zero-selected/ignored case counted as passing. Actual Composition SHA-256:
  `e617d6ab5435ea40b5195113772f3d7f8e86bca423e4e6c20257d611f8d8979f`.
  Existing `completed.json` records the failure and must not be overwritten.
- After user-requested inspection and explicit deletion authorization, the
  supervisor removed exactly 28 unchanged, unreferenced temporary native-image
  files and their empty parent directories: 9.91 GiB, about 29 GiB free afterwards.
  No wildcard deletion, source/PG data or Cargo-cache cleanup was involved.
  Read-only audit and exact deletion receipts remain at
  `/private/tmp/brassclaw-native-deletion-audit.json` and
  `/private/tmp/brassclaw-native-deletion-result.json`.
- Run-7 is prepared to compile the two corrected test sources once, execute only
  those two failed cases and then run affected-package Clippy serially if both
  pass. **Terminal result:** source `82a8fd1eb`, compile 157.93 seconds, both
  real native cases passed (6.44/1.96 seconds). Actual Composition SHA-256:
  `09d75e1d29dcfb0b7662f48a27433ad15288f443ff968bc83b123ca1a4e5eb29`.
  The unchanged 71 passing run-6 cases and LLM/Auth run-4 evidence are reused;
  this is not a new single-executable 73-test result. First lint stopped at a
  redundant field initializer, corrected by A-01 and integrated as `a63100515`.
- `.../lint-2/` captured `a63100515`; actual eight-package Clippy with
  `postgres,skills-db`, `--all-targets` and `-D warnings` passed in 124.17 seconds
  on native Rust 1.98.1, aarch64 Apple, LLVM 22.1.8. It did not repeat runtime
  tests or link a new product executable. No warning suppression. Native
  Apple unwind-size warnings from linked test executables remain open and are
  not repaired merely because metadata-only Clippy passes.
- The Apple linker unwind-size warning remains visible and unresolved. No
  panic/exception behavior change or warning suppression is accepted as a fix.
- Private canonical-factory draft:
  `/private/tmp/brassclaw-bob-seed-20261103/canonical.patch`. It is **not applied**;
  applying it alone can advance revisions while old qualification skips them.
  It is a development proposal, not approved implementation or qualification.

## Remaining work and next dispatch

All three Bob plans remain the implementation objective. B1-01/B2-01/B3-01
inventories and prior scoped B3-10 work are recorded; broader steps stay open.
After current acceptance/review, define the next bounded packages from the real
remaining steps: original queue/submission binding; canonical complete check
sets and genuine noncircular seed qualification; trusted own-function/Q2
production with exact versioned carriage; shared durable private work without
synthetic chat; coherent publication/all retained readers; product projections
and final interruption/platform acceptance. These are backlog areas, not new
parallel owners or claims of completion.

Assign each shared contract once, record dependencies before splitting packages,
and keep at most two implementing chats. Archive/finish an accepted package chat;
create a fresh chat for its successor. Commits and pushes do not end the overall
implementation objective.

### Current source-review findings — 2026-11-05

Main checkpoint `6a440d715` is pushed. B-01 and A-02a retain their scoped
acceptance above; all three complete Bob plans remain open.

A-03a's first source checkpoint `e72119fb5` is not integrated or compiled.
Its connected review requires installation-owner reachability beyond test-only
callers, exact terminal qualification readback before any new admission/effect,
original preparation capture and actual shared-host negative fixtures. A generic
resource/input failure cannot stand in for the intended native guard rejection.
Configuration-sized qualification buffers must also respect actual resource
feasibility without guessed limits. Lost acknowledgement, cancellation and later
cleanup failure need their real native cases. No paid compilation is approved
until the corrected coherent diff is reviewed.

The bounded scoped-fixture adapter design is reviewed for implementation in
A-03a: the existing native registration and invocation-service owner retain the
original handler/artifact and pass an exact private fixture preparation to the
actual SourceInspection backend. Fixed package input determines the narrowed
fixture bound; base and derived settings identities stay distinct. It must retain
each actual primitive result before advancing, preserve current policy/custody
and support completed-original readback without replay. This is source work to
finish, not an implemented adapter or successful qualification. A temporary
unavailable-only path for normal valid settings cannot close the package.

B-02 owns the common acknowledged preparation capture at the existing settings/
catalogue owner, pairing its actual revision with the exact limits. A-03 owns the
validator input/selection capture and its checked original-byte restoration;
B-02 persists and consumes it without another serializer. Later native-dispatch
revision and original task-preparation revision remain separate. V207 stays
exclusive to B-02. Existing work must not recapture current settings or rematch
on restart. These assignments describe work in progress, not implemented APIs.

A-03 also owns the necessary correction of the existing contained-bootstrap
executor configuration. Extract the current pure startup constructor once in
its existing owner, using the actual settings already read before catalogue
boot; both controlled qualification and the real global start consume it.
The current helper's fixed frame/capacity/deadline values and utility-memory
derivation cannot certify the new path. Utility parser physical limits and VM
executor heap/backstop have different purposes. This executor-creation profile
is distinct from B-02's original task-preparation capture: ordinary private work
uses the running global VM and keeps current shared memory/policy controls live.
It does not create a per-task retained heap policy or another settings reader.

A-03a's subsequent source work now connects a queue-free installation subject/
selection adapter and the existing Structure/Pure checking graphs to catalogue
boot. These remain uncompiled, unaccepted worktree changes. The installation
records carry factual observations without authored submissions, Q1/Q2 grants
or activation. The shared Engine serializer, receipt/dataflow checks and native
fingerprint checks serve both actual observation and historical readback. Source
review also requires original completion after normal Turn pruning, pool-size-one
readback, unchanged execution counters and later smaller task limits. Historical
metadata reads must not apply today's task limits; actual new execution uses its
captured preparation. The consumer receives the actual startup root rather than
selecting another root after admission. Finish review, reconcile accepted main
changes and freeze exact inputs before compilation; source edits are not receipts.

B-02's earlier pending tool approval is resolved. Its first shared
`PreparationCapture` source handoff (`8bc93bd29`, not integrated) retains actual startup/effective revision,
complete limits and original bytes through the existing catalogue/settings
owner. Source review checked the actual acknowledged `effective_view`, getter
delegation and original-byte restoration; no compilation or runtime acceptance
is claimed. The owner commit is to be shared with A-03a for its one validator
input/selection capture implementation. Connected LIVE Work/Origin/claim/admission
integration and V207 remain B-02 work, not delivered functionality.

Latest source-only successor: A-03a `71ad80547` incorporates accepted main
`6a440d715`, B-02's actual preparation API and the domain
`PreparedValidatorReview`/`ValidatorInputCapture` original-byte API. Its sealed
installation adapter, boot consumer, parser qualification and original readback
are now source implementations, superseding the earlier design-only observations
above. The supervisor reviewed the contained cancellation/close correction:
proven quiescence permits the original fenced Turn cancellation, lost close ACKs
read back the original, and operations retain additional cleanup errors rather
than discarding them. None of these source reviews is native acceptance or
general Pure approval. The immutable STATUS7 manifest has SHA-256
`4bd1a9bbbca5e202b215c1fd30ba79e82915dbf964a0509ef27df4b74428d094`;
its handoff is under `/private/tmp/brassclaw-a03a-20261105/`.

B-02 has checkpointed its connected private-origin work as `42ecb6afa`.
The app approval for its Git integration is now resolved; the owner is merging
the A-03 dependency chain in its own worktree. The supervisor did not bypass
or reassign that pending action. B-02 received
the complete A-03 successor and the read-only actual-origin compatibility report.
It must reconcile the genuine optional Chat context with private claims, preserve
the distinct contained-foundation and live-private settlement owners, finish both
production consumers and deliver the combined source review before compilation.
Neither branch is integrated into main or runtime-qualified. The preparation and
validator capture APIs are shared owners, not duplicate implementations.

The supervisor also returned a connected delivery finding from `42ecb6afa`:
the worker may drop the private driver future on heartbeat/cancellation while
the original is subsequently settled by `stop_attempt`; its waiting caller
must still receive a correlated outcome or explicit unresolved error. Late
waiter registration must not lose the original delivery. These are source-review
findings for B-02 to resolve, not observed native failures. A-03's acceptance
inventory names 32 existing primary cases. Its capture-restore helper runs in
one process; it does not establish process-cold or durable Work recovery.

The combined branch now includes A-03 through merge `2831a1bbf`; production
consumer/caller corrections and native test sources are still being completed.
Further source review identified two success windows: factory retention can be
released before completion delivery, and a heartbeat must not mistake the same
claim's already settled original for lost ownership. A delayed successful
delivery must survive a heartbeat period without renewed dispatch authority.
These fixes and their genuine negative cases remain B-02 acceptance work. The
submission-to-Work transaction/startup gap also needs explicit coverage or a
precise remaining B3-06/B3-07 handoff; durable rows alone do not prove dispatch.

The independent C-00 source boundary is locally integrated as `e212e0ba2`.
Its first native profile/link check completed in detached Screen while B-02
continued in its isolated worktree. This used the development policy's
targeted intermediate check of an unresolved shared build boundary: it can
resolve a remaining unwind blocker before the larger Work integration, and
Cargo can reuse unchanged optimized dependencies afterwards. It is not B-02 or
A-03 runtime acceptance. Run-11 passed: 779.70-second graph compile, no warnings,
11,978,176-byte `__eh_frame`, retained unwind sections and all 1,353 old test
names, with both selected native PG callers passing. Source/HEAD stayed frozen
throughout; the source freeze is now released. Exact artifacts and scope are in
the [C-00 receipt](validation/2026-11-05-c00-test-unwind-profile.md).

Final warning acceptance also remains open: the retained native test executable
has a measured 19.12 MiB `__eh_frame` section, exceeding the linker's reported
16 MiB compact-unwind offset limit. Metadata-only lint is not linked acceptance.
Read-only DWARF inspection of that same hash-verified run-10 executable found
348,854 FDE records totalling 19,750,896 bytes; many resolved symbols are generic
`core`/`alloc` helpers. Symbol namespaces do not identify Rust compilation-unit
ownership, so this is diagnostic evidence, not proof that optimizing one named
crate fixes the limit. Detailed current-archive and actual-binary analysis is in
`/private/tmp/brassclaw-bob-unwind-archives-review-20261105.json` and
`/private/tmp/brassclaw-bob-unwind-fde-review-20261105.json`. No new build or
optimizer/linker change was made for this inspection.
The inherited composition-wide deprecated-type suppression requires the actual
legacy-adapter/type migration audit in B2-09, consumed by B3-12. Neither warning
suppression nor untested removal of retained compatibility closes these gates.
