# Incoming Bob package review — 2026-11-13

This is a supervisor source-review checkpoint. None of the four incoming
packages is merged or accepted as complete. The four component guides and the
three Bob plans remain binding; reports and structural parsing grant no approval.
Main's reviewed compatibility checkpoint is `d24a87508`. Review corrections go
back to the existing package owners, not a second implementation service.

## Source identities and ownership

| Package | Branch | Latest inspected commit |
| --- | --- | --- |
| B1-06 | `codex/b1-06-approval-writer` | `266bea0d7a3ea577e7cfe43d241dd908b477ebf6` |
| B2-02 | `codex/b2-02-retained-selection-v2` | `27267af66e0c2a5118442d18c4109fb898f04c37` |
| B3-06 | `codex/b3-06-durable-q1-work-wt` | `f5493ff046db0cb689966bd5e9604a7d4fde3d74` |
| B3-09 | `codex/b3-09-webui-review-panels` | `e683aa024702ac3849a0790987ed866c81760549` |

V214 belongs to B1. B2's unpublished migration is renamed to V215 in
`abd55c2c3`, without changing the SQL. Applied migrations are untouched.
Package work stays in isolated worktrees. Root alone integrates/releases and
runs local Cargo. Lock ownership ends with `flock(LOCK_UN)`/descriptor close,
never unlinking `/private/tmp/brassclaw-target-cargo.lock`.

## B1-06 — stage and subject binding

The earlier `e62a4db6c` incorrectly looked for Q1 run rows for every authored
evidence stage. `266bea0d7` corrects that routing: the authored Q1 evidence is
joined to its run and original submission, including root/version, subject,
selection, class and task identity. Behavior and human Q2 no longer pretend to
be Q1 runs.

A matching successful run is still insufficient qualification: both
`persist_validator_q1_evidence` and the explicitly Source-only
`persist_validator_source_evidence` use the same internal persistence path,
`kind: q1` and run table. The existing verdict reader requires
`report.q1_qualification == false` for its structural observation. The approval
writer must verify the actual qualified check scope through its review owner;
it must not promote successful prerequisite/structural observations to full Q1.
Require an actual Source-only successful run as a negative approval case.

This is not complete own Pure approval. The new positive writer test directly
inserts behavior and human-Q2 evidence in SQL. The cited existing
`pg_retained_q1::persist_behavioral_reviews` produces selected Tool-usage
evidence, not general standalone Pure behavior. A typed row with a successful
flag is not a qualified producer or an authenticated human decision. Preserve
historical system-seed contracts; keep unsupported authored Pure approval
closed until its actual producers and exact decision binding are connected.
No additional installed human Q2 is needed for genuine shipped seeds.

B1-06 still owns the class/profile decision dispatcher, displayed-subject and
evidence CAS, trustworthy producer linkage and the atomic approval/link write.
The package must not be marked complete from these narrower writer changes.

## B2-02 — `/2` reader and retention

`0c6ef93ec` improves duplicate-key/resource rejection, closed objects,
functional-step membership and immutable parsed fields. These fixes are useful,
but the following gaps remain in `27267af66`:

- `parse_component_ref` only delegates to the existing
  `association_contract::ComponentRevisionRef` Serde visitor. That visitor checks
  object shape/types but does not reject nil UUIDs or zero versions. The package
  documentation and new zero-version test assume checks that are absent.
- The new parser leaves `recipe`, `variant` and `components` opaque. Builder
  roundtrip success does not establish a fully checked retained selection.
- `review_selection_store::verify_program_selection` accepts `/2` while checking
  only that `functional_approvals` is a nonempty object. Its comment defers deeper
  checks to a later Engine consumer that is not connected by this package.

The owner must expose one reusable structural contract at an appropriate existing
dependency boundary and use it before retention and reconstruction. Avoid a
Skills-to-Engine cycle or two divergent validators. Retained legacy base
references use their existing hex-checksum codec; the shared native reference
type uses a byte-array checksum. Preserve each documented wire representation
rather than conflating them. Neither structural parsing nor a profile label
establishes approval or compatibility.

## B3-06 — original preparation and cancellation

The final report correctly acknowledges that package-default Clippy does not
compile the changed production modules. Required Composition lint explicitly
enables `postgres,skills-db`.

The new production selection producer in `private_task_catalogue` calls
`read_allocation_head` for five validator components through separate reads.
The revision store explicitly documents allocation heads as neither approval nor
activation. This does not select one coherent qualified validator generation.
Use the existing qualified catalogue/bootstrap selection owner and retain its
exact immutable selection; preserve the already atomic qualified Source path.

Moving write operations outside the reconciler's timeout does not establish
commit custody. `TurnRunner::claim_run` still selects cancellation against the
entire reconciliation future. `prepare_fresh_private_validator` uses a raw
pooled transaction; cancellation can drop it before its post-error readback.
The existing transaction/evidence guard and bounded acknowledged resolution must
cover that boundary, with original-ID readback and no replay.

The named lost-commit-ACK test calls successful preparation twice sequentially.
It tests idempotency; it does not cut an ACK or cancel an in-flight commit.
Retain that useful case under its actual scope and add genuine PostgreSQL fault
acceptance through the existing ACK-cut/transaction test support.

## B3-09 — displayed review and actual mutation path

The panel reads the original immutable submission, which is the correct
inspection source. Its mutation path remains incomplete:

- `validateComponent`/`rejectComponent` send class/component ID and optional
  feedback. They do not send the viewed immutable submission/subject/evidence
  selection. The request DTO likewise contains only feedback. Rendering a 409
  message does not provide transactional stale-view protection.
- The actual `PgRecipeStoreFacade::update_component_validation_status` handles
  classes 21 and 13; class 22 returns unsupported. Calling that endpoint does not
  connect the new panel to a pure-approval writer.
- Calling `ValidatorReviewPanelInner(...)` as a normal function does not create
  a React component boundary; the wrapper's conditional return does not itself
  guarantee hook order across class changes.

Coordinate the typed immutable decision request and real class/profile dispatcher
with B1. Reuse its trusted writers rather than creating a generic status-only
approval shortcut. Test real HTTP/store behavior for class 21/22, changed views,
rejection and unknown acknowledgement as applicable. Static render fixtures do
not establish those backend properties.

## Actual checks and next acceptance

Root runs these inexpensive Node checks directly, without Cargo or cloud work:

| Source / selected suite | Observed result |
| --- | --- |
| B3-09 panel and queue suites | 35 passed, zero failed/skipped |
| Main provider-components suite | 4 passed, 11 failed, zero skipped |
| B3-09 provider-components suite | 4 passed, same 11 failed, zero skipped |

The provider failures are missing `ProviderBootstrapStatus` or
`ProviderAuthStatus` in the VM fixture's render context. They precede B3-09;
this baseline comparison is not a waiver. Fix them through the existing UI/test
owner, preserving real provider behavior and assertions.

Separately, Run-57-r1's strict Rust 1.100 workspace/all-targets check passes on
Main `d24a87508` in 62.104 seconds with zero Rust diagnostics, zero Cargo warning
lines and unchanged source. The next compatibility phase stops at the disk gate
before Cargo; the whole batch and native acceptance remain incomplete. That
receipt does not compile or validate any of these unmerged branches.

Bundle source-accepted corrections, then compile the affected combined artifacts
once. Required acceptance includes strict full-feature Composition, relevant
compatibility features, actual `/2` parser negatives and PostgreSQL retention/
migration cases, real stage/subject rejection and decision cases, durable fresh/
restart/cancelled-commit originals, and the connected UI/API cases. Preserve
applicable prior evidence; do not relabel source fixtures as genuine executions
or repeat an unchanged passing compiler phase without relevant changes.

No new release/tag, normal remote-test activation, complete Bob plan, general
Pure approval or full private N/R/Q/T acceptance is claimed by this checkpoint.
