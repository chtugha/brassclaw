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
| A-01 — native Codex/Auth custody | `01a1a022-90b1-7932-8dcf-c0ef34b79fee`, host `local`, title “Bob 3 — Native Codex/Auth via Modal”; `/Users/ollama/.codex/worktrees/e8ba/brassclaw`, branch `codex/bob3-auth-custody` | First commit `77b31f122` integrated as `e92b66b5d`. Modal CLI compiled; final encoder/test-source delta and native behavior still require local acceptance. Review corrections remain in this same package: recover original unfinished filesystem custody without blocking its only startup/recovery path; compare all imported native identity/index columns, not JSON alone. No further independent Auth task is assigned in this chat. |
| B-00 — review inherited Q1/Oracle repair and finish its acceptance fixes | Fresh package chat to be created | Inherit source commits `548254e43`, `a534c4654`, `626e9fddc`, `fdc1f0bc2`. Review exact pure-link restrictions, shared driver settlement proof and protocol oracle classification; repair attributable implementation/test defects. Keep within this existing connected package, with no new approval architecture, queue or VM. Root's already written code is not automatically accepted. |

Migration ownership: V203 (exact pure links) is integrated; V204 belongs to A-01
(filesystem Auth cutover) and is integrated. Coordinate any next migration with
the supervisor before using its number. B-00 requires no migration by default;
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
- The prepared combined native runner is
  `/private/tmp/brassclaw-bob-native-20261103/run-3.py`; use a new evidence
  directory for a rerun. It compiles Composition/LLM libraries together, then the
  Auth contract target serially, retains actual binaries/worker and runs exact
  selected tests locally. Check source manifests and actual outcomes; do not
  reuse an old binary to claim a new interface was tested.
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
