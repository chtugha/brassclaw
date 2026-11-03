# Development package board

Latest user direction, 2026-11-03: a supervisor and at most two implementing
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

## Active packages

| Package | Chat/worktree | Scope and state |
| --- | --- | --- |
| A-01 — native Codex/Auth custody | `01a1a022-90b1-7932-8dcf-c0ef34b79fee`, host `local`, title “Bob 3 — Native Codex/Auth via Modal”; `/Users/ollama/.codex/worktrees/e8ba/brassclaw`, branch `codex/bob3-auth-custody` | Initial and review follow-ups integrated through `7936dc29e`. Run-6 passed all five new Codex-refresh cases, migration/index/strict-JSON/fence cases and original Device failures except one later fixture assertion. The two remaining fixture corrections `39d4c0c8d`/`c7ec0ef89` are integrated; their actual connected execution and relevant lint remain pending. Full mandatory architecture reading confirmed; no independent new Auth task. |
| B-00 — review inherited Q1/Oracle repair and finish its acceptance fixes | `01a1a062-b1c4-7ba0-8506-b7c8b3ee4b33`, host `local`, title “Paket B-00 — Q1- und Oracle-Reparatur”; `/Users/ollama/.codex/worktrees/ac66/brassclaw` | Inherited source commits `548254e43`, `a534c4654`, `626e9fddc`, `fdc1f0bc2` reviewed. Correction `f62e911cf` and report `25a9536bd` integrated as `3223415ff`/`6c2041ab6`: incomplete execution no longer writes a negative candidate verdict. Exact-source Modal CLI and local native CLI smoke checks passed; new regression compilation/execution and affected lint remain pending. Chat is idle awaiting supervisor acceptance, not assigned another package. |
| B-01 — original immutable review request | `01a1a0d0-de89-7a73-9fd2-af06d5d1ca81`, host `local`, title “Paket B-01 — Originale Review-Anforderung”; `/Users/ollama/.codex/worktrees/2f1e/brassclaw` | Complete mandatory reading and revised contract reviewed. Bounded implementation authorized after the terminal run-6 and passing B-00 native cases; final predecessor lint remains supervisor-owned. Implement exact immutable request/queue binding, coherent supported caller capture and final CAS in the existing owners. V206 reserved. Keep actual legacy and joint-runner gaps explicit; no replacement scheduler/approval owner or blanket disabling of authors. |

Migration ownership: V203 (exact pure links) is integrated; V204 belongs to A-01
(filesystem Auth cutover) and is integrated. V205 is reserved exclusively for
A-01's snapshot-safe custody successor; do not modify applied V203/V204. Coordinate
any later migration with the supervisor before using its number. V206 is reserved
for B-01 if its reviewed original-request contract needs a migration. B-00 requires no migration by default;
it must explain any actual schema need before implementation.

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
  pass. The unchanged 71 passing run-6 cases and LLM/Auth run-4 evidence are
  reusable; final lint/product/platform claims remain open.
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
