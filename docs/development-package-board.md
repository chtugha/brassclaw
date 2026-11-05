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

| Package | Chat/worktree | Scope and state |
| --- | --- | --- |
| A-01 — native Codex/Auth custody | `01a1a022-90b1-7932-8dcf-c0ef34b79fee`, host `local`, title “Bob 3 — Native Codex/Auth via Modal”; `/Users/ollama/.codex/worktrees/e8ba/brassclaw`, branch `codex/bob3-auth-custody` | Accepted within the recorded source/native scope, integrated through `a63100515`. Run-6's 71 passing cases plus run-7's two corrected cases cover the selected 73-case matrix; eight-package strict Clippy passed. Actual OpenAI issuer, full product/platform/release and broader B3 gates remain open. Full mandatory reading confirmed. Package closed; no independent new task in this chat. |
| B-00 — review inherited Q1/Oracle repair and finish its acceptance fixes | `01a1a062-b1c4-7ba0-8506-b7c8b3ee4b33`, host `local`, title “Paket B-00 — Q1- und Oracle-Reparatur”; `/Users/ollama/.codex/worktrees/ac66/brassclaw` | Accepted in its bounded inherited repair scope: exact-source Modal CLI/native smoke, actual incomplete-execution regression and all six retained Q1 callers passed, along with Oracle/pure-link/shared-driver cases. Eight-package strict Clippy passed. General pure approval, canonical genuine qualification, durable scheduling and B2 cutover remain open. Package closed; no new task here. |
| B-01 — original immutable review request | `01a1a0d0-de89-7a73-9fd2-af06d5d1ca81`, host `local`, title “Paket B-01 — Originale Review-Anforderung”; `/Users/ollama/.codex/worktrees/2f1e/brassclaw` | Accepted in its bounded scope through `5c5e84bf3`: exact original/CAS/fault/readback, complete queue projections/paging, actual Q1/Sempai/lifecycle cases, 24 UI tests and strict twelve-package lint. Modal CLI/native smoke also passed. V206 integrated. Full `/3`, legacy custody disposition, complete Sempai/Docplan conversion and durable Work remain open. Package closed, no next task here. |
| A-02 — exact Pure-approval readers | `01a1a139-eac1-7e32-9ac9-5ddfff64994a`, host `local`, title “Paket A-02 — Exakte Pure-Approval-Reader”; `/Users/ollama/.codex/worktrees/27f9/brassclaw` | Narrow A-02a `f62e17a12` integrated as `9c8d3c12a`, scoped acceptance complete: exact canonical `/1` prepare/recheck/reload, occurrence isolation, ordinary publication, retained child, complete deployment caller and strict affected lint. No new general Pure approval, migration or selection format. Package closed; successor gets a fresh chat. |
| A-03 — genuine Pure approval and exact retained successor contract | `01a1ab08-ac3e-7812-bf0d-ba6afcc18fb0`, host `local`, title “Paket A-03 — Pure-Approval und Retained-Vertrag”; `/Users/ollama/.codex/worktrees/a7f2/brassclaw` | Mandatory reading and actual source/contract review complete. Only A-03a source slice authorized: existing pure/structure graph receipts, exact native-inventory/subject observations and genuine narrow parser prerequisite/usage qualification. General P22 approval, `/2` emission, Q2/activation and Work remain separate open gates. No paid compile until coherent diff/self-review. B-02 owns shared private admission mechanics. |
| B-02 — durable private Monty Work | `01a1ab4e-ae8c-7342-98a3-7b6b8ab50650`, host `local`, title “Paket B-02 — Dauerhafte private Monty-Aufträge”; `/Users/ollama/.codex/worktrees/d230/brassclaw` | Mandatory reading, actual source inventory and PRECISION-1 reviewed. Connected first source slice authorized: genuine private Origin/PG claim/admission/cancellation/live settlement for existing structural validation and quarantined source-only Prefix, through the one driver/worker and existing configured fallback. V207 reserved exclusively here. No general Pure approval or durable-proof restore claim; full diff/schema review before compilation. A-03 owns review semantics and parser observations. |

Migration ownership: V203 (exact pure links) is integrated; V204 belongs to A-01
(filesystem Auth cutover) and is integrated. V205 is reserved exclusively for
A-01's snapshot-safe custody successor; do not modify applied V203/V204. Coordinate
any later migration with the supervisor before using its number. V206 is reserved
for B-01 if its reviewed original-request contract needs a migration. B-00 requires no migration by default;
it must explain any actual schema need before implementation.
A-02a uses no migration. V207 is reserved exclusively for B-02's reviewed
connected private-Work/Origin/capture/guard successor. A-03a has no migration
allocation. Further schema changes require a distinct reviewed reservation;
no allocation permits editing or reinterpreting prior migration bytes.

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

Main checkpoint `dd8307eb5` is pushed. B-01 and A-02a retain their scoped
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

Final warning acceptance also remains open: the retained native test executable
has a measured 19.12 MiB `__eh_frame` section, exceeding the linker's reported
16 MiB compact-unwind offset limit. Metadata-only lint is not linked acceptance.
The inherited composition-wide deprecated-type suppression requires the actual
legacy-adapter/type migration audit in B2-09, consumed by B3-12. Neither warning
suppression nor untested removal of retained compatibility closes these gates.
