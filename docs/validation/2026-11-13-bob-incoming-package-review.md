# Incoming Bob package review — 2026-11-13

This is a supervisor source-review and bounded acceptance checkpoint. The
inspection-only B3-09 slice is accepted into Main; the other packages remain
unaccepted and no complete Bob step is claimed. The four component guides and the
three Bob plans remain binding; reports and structural parsing grant no approval.
Main's reviewed compatibility checkpoint is `d24a87508`. Review corrections go
back to the existing package owners, not a second implementation service.

## Source identities and ownership

| Package | Branch | Latest inspected commit |
| --- | --- | --- |
| B1-06 | `codex/b1-06-approval-writer` | `01cc4499a`, report `8b6f96a63`; restored guard, remaining lint/DB fixture findings |
| B2-02 | `codex/b2-02-retained-selection-v2` | `e0be06d8fbe9c08abd551ac522034255a2e90826` |
| B3-06 | `codex/b3-06-durable-q1-work-wt` | `d94aaa8e1`, report `07e0a4656` |
| B3-09 | `codex/b3-09-webui-review-panels` | `f857421e3`, accepted inspection slice |

### Latest follow-up and actual candidate check

Earlier per-commit findings below remain historical where superseded here.
The five B1 corrections are applied in `b48d45b7c`: real table/positive
qualification requirement, feature-correct selectors, helper removal and bounded
checkpoint ownership. Root prepares only that source on
`codex/bob-reviewed-integration-20261113`, candidate merge `8cdbd5f7`;
At that gate Main remains at `d722c92d4`. B1 is not accepted into Main.

Actual Run-58 strict Rust 1.100 Composition all-targets Clippy, explicitly
`postgres,skills-db`, fails on `clippy::collapsible_if` at
`pg_pure_component_approval.rs:687`. Inputs remain unchanged; elapsed time is
103.969 seconds. Upper/test lint may be blocked; this is not complete lint or
native acceptance. The receipt runner also incorrectly required successful
feature artifacts before recording a failed command. Original receipts and full
Cargo JSON/stderr are preserved. Separate `post-review-diagnostics.json` confirms
Cargo `build-finished: false`, the diagnostic and unchanged source; the future
runner records failures before success-only artifact assertions. No unchanged
build is rerun for a reporting error. The source freeze is released.

B1 also needs a fixture correction before native execution: its subject-binding
test stages revision 1 and then calls `submit(base=None)` on the same UUID,
attempting another allocation from head zero. Use `submit_retained` or allocate
once through submission, preserving assertions. V214 requires actual missing-run,
failed-run and seed-exemption constraint cases; closed authored writer tests
return before that trigger. This is low-level DB integrity, not full approval.

### B1-06 `3f802f79c` — rejected cross-package correction and rollback

The new commit changes only the B1 approval writer/tests, not B2's retained
selection contract. Its four-item summary does not resolve B2's `/1` selection
strictness, captured original limits, Recipe/variant/step structure or formatting.
It deletes the authored-path guard, Q1 subject-binding join and authored evidence
format handling from the previously reviewed `b48d45b7c`, removing their tests
without connecting qualified replacements (928 deleted lines across two files).
V214's succeeded-run check cannot substitute for positive full-Q1 qualification,
exact submission binding, trusted behavior or authenticated human Q2. Existing
production callers remain installation-owned; no new public exploit or activated
authored support is claimed. Reopening this internal writer is nevertheless not
an accepted repair. Restore the prior protected B1 checkpoint and apply the
actual B1 lint/fixture/DB-constraint corrections without deleting the guards or
tests to avoid their failures. Keep B1 qualified producers as explicit open work.

The local evidence limit value still copies every field from REVISION_LIMITS;
passing it to an unused `_limits` parameter changes neither the accepted limit
nor the caller contract. It is not captured-original limit propagation. The extra
unknown-field/non-object/duplicate-evidence-ID cases are useful only within their
B1 evidence/declaration scope, not duplicate Recipe-step or retained-selection
coverage. The unchanged package report still describes the removed guards and
binding as present and needs correction. No Cargo run or integration is justified
for this known-regressed source. The existing candidate retains `b48d45b7c`.

### B1-06 `01cc4499a`, report `8b6f96a63` — restored, still not accepted

The net comparison against `b48d45b7c` confirms the authored guard, authored
format handling and Q1 subject-binding checks are restored. The subject-binding
fixture now obtains its sole revision from `submit()` and uses `sub.candidate`;
the earlier duplicate allocation is corrected. The new unknown-field and
duplicate-stage-UUID cases remain evidence/declaration checks, not B2 selection
coverage. No ineffective `_limits` parameter remains.

Two connected corrections are still necessary before spending another build:

- Run-58's actual `clippy::collapsible_if` was the nested legacy-checksum check
  in `validate_evidence_declaration()` at old line 687, now line 679. That block
  is unchanged. The modified `detect_evidence_format()` conditions are different
  code and do not resolve the observed diagnostic. Collapse the actual reported
  condition while preserving checksum validation and both format branches.
- The new direct V214 test references a random root UUID without staging a
  corresponding revision. Its seed-success INSERT therefore fails V200's
  `(root_id, root_version)` foreign key. Its authored-negative INSERT also passes
  SQL NULL for q2_ref despite a non-null UUID in the JSON: V200 independently
  raises CHECK_VIOLATION for that row, so checking only SQLSTATE does not isolate
  V214. Stage one real class-22 revision, use its exact identity/checksum and
  a non-null matching authored q2_ref; assert the specific V214 error as well as
  its SQLSTATE. Add the missing genuine `succeeded=false` run fixture with valid
  submission/evidence references. Keep seed exemption as a successful actual
  INSERT. These controlled low-level constraint fixtures do not qualify authored
  approval or reopen the writer.

No new Cargo run is justified for these source-visible failures. Main and the
previous B1 candidate remain unchanged. General B1 producer qualification stays
open; restoration of the guard is not activation.

B3-06 `ad1ea1b90` removes the production draft selector and incompatible checksum
read, and explicitly defers absent selections. Its write now has a deadline and
custody. Remaining: bounded readback starts with `write_result?` and skips its
read on write failure; the outer original reader remains unbounded/raw-pooled.
Guard and bound actual SQL resolution separately from captured native checks.
The test-only entry duplicates production preparation, and the ACK-cut test
permits arbitrary first `Ok`/`Err`. Use one actual path, preferably the existing
transport-backed runtime with a capacity-one pool, and require exact successful
recovery when readback is healthy. Test intentionally blocked/unresolved readback
separately. The controlled draft fixture does not prove general qualification.

B3-09 `63d4a80c6` fully reverts the flawed class-22 Rust helper; the net Rust
change is a boundary comment. Class 22 is inspection-only and the corrected
React boundary remains. Root's changed Node suites pass 35/35, zero ignored.
Class 21 still labels legacy graduation as a connected Q2 writer without a
displayed-view expectation. Keep the new panel inspection-only for both classes
until B1 connects typed Pure/workflow decisions. Existing legacy QueueRow behavior
is separate. Correct stale dispatcher/409 header claims and replace plan-ID/
internal-writer product text with a plain unavailable-approval explanation.

### Accepted B3-09 inspection slice `f857421e3`

The new panel is now read-only for both classes, removes its mutations and stale
affordance texts, and uses plain unavailable-decision wording. Existing legacy
QueueRow actions remain unchanged. Root reviews the complete final change,
confirms removed translation keys have no production consumers and runs 33 panel
plus eight queue Node cases: 41 passed, zero failed/skipped. This is static
render/contract evidence, not real React lifecycle or backend approval acceptance.

Candidate `04dfb7c7` includes this reviewed slice for Run-59. Actual Rust 1.98.1
static-library test compilation passes without warnings in 13.28 seconds;
all 28 existing Rust asset/router tests pass, zero ignored. Generated asset-table
inspection and executable bytes establish exact inclusion of the new JS module.
Total recorded batch time is 16.912 seconds, source/artifact hashes unchanged.
Logs, captured source, artifact SHA and the exact asset proof are under
`target/validation/bob-native-repair-20261105/run-59-b309-static/`.

Main merges only the UI branch as `52d74c2c3`. A checked comparison verifies all
362 static-package/root-manifest inputs identical to the tested candidate;
`main-scope-reuse.json` records that applicability. The unaccepted B1 writer and
V214 are absent from Main. No repeated native build is needed for unchanged
static inputs. This accepts inspection packaging/routing, not typed Q2, full
B3-09/Bob, browser/issuer/platform or deployment completion. The pre-existing
eleven provider fixture failures remain explicit B3-12 work.

### B3-06 `ec039b38e` remains unaccepted

The read setup is factored and the ACK test now requires success. Source review
still finds a closure error-type mismatch: `bounded_for` requires
`RebornBuildError`, while new `?` paths return local `WriteOutcome`. Carry the
classified outcome through a correctly typed data result. The ACK test also
consumes `result` with `unwrap()` and later uses it in a diagnostic; borrow or
retain the recovered state once. These are source-visible issues, not observed
compiler results for this branch.

Both metadata-read helpers remain raw-pooled/unbounded, and production/test
recovery flows remain separate after sharing their setup. Complete actual guarded
SQL resolution and one real path, then verify same-pool capacity-one ACK recovery
and immutable original checksums. Pre-COMMIT failure means no committed write,
not necessarily that no INSERT statement executed. Corrections return to B3-06;
no compilation or acceptance of `ec039b38e` is claimed.

### B3-06 `6c24e5e33` — further source corrections, not accepted

The custom closure error mismatch and consumed test result are corrected.
Metadata SQL now has an acknowledged deadline while native original inspection
remains outside that SQL block. The two recovered request checksums are compared.
Remaining owner corrections:

- The closure borrows `Cell<bool>` across await, conflicting with the actual
  `async_trait` Send-future requirement because Cell is not Sync. A status value
  mutably borrowed by this one future needs no additional synchronization.
- Both timed SQL readers still acquire raw pooled clients. Use existing
  transaction custody so timeout cannot recycle an uncertain read transaction;
  recycle only after acknowledged commit, including the absent-original branch.
- The new zero-timeout checkout does not prove saturation. In the pinned
  Deadpool 0.13.1, Object::take calls detach_object, which decrements pool size
  and restores a semaphore permit. Recovery should permit a fresh bounded
  checkout with a distinct connection identity, not require permanent capacity
  loss. Exercise the same capacity-one pool rather than a separate healthy reader.
- A timeout after the commit-attempt flag is set is possible during commit await;
  correct the contrary comment. The fixed-size request checksum is always
  nonempty, so that assertion proves nothing; use actual original-byte digest
  and identity comparisons. No replay of a completed effect is authorized.

These are source findings. No Cargo/native acceptance of this branch is claimed.

### B3-06 `e4b6adedb` — custody corrected; acceptance still pending

The commit replaces Cell with AtomicBool and gives both timed transactional
readers the existing EvidenceClient custody, including acknowledged recycling
on the absent-original branch. The impossible timeout claim and vacuous array
nonempty assertion are removed. The test compares the returned checksum with
the persisted row. Direct Rust 1.100 Rustfmt checks of both changed Rust files
pass; no Cargo, PostgreSQL or runtime acceptance is claimed.

Remaining owner corrections before the combined build:

- The ACK-loss test still injects only the write pool and resolves the write
  through the separate healthy `self.pool`. A later successful checkout with a
  different backend PID establishes fresh-connection availability, not the
  required full recovery through one capacity-one pool. Construct the actual
  runtime/catalogue with that shared proxy pool, prepare fixtures before arming
  the exact fault, and invoke the production preparation entry point. Reuse the
  existing proxy-runtime and state-fixture worker-stop patterns where needed.
  Require successful original readback, one intercepted ACK and one original
  row without replay; do not retain a separate healthy recovery route.
- The new ten-second timeout bounds only checkout, leaving its PID query outside
  the bound. Use the applicable existing acknowledged deadline for the complete
  recovery observation. Describe the PID assertion as a new-session check, not
  proof that the old server process has already terminated.
- The nested successful-write/wake `if` remains in the changed production method;
  correct the source-visible collapsible-if issue before strict Clippy. This is
  not a new compiler observation. The package report also retains trailing
  whitespace and a stale WriteOutcome test description.

The package remains outside Main. The read-custody and Send findings from the
previous commit are superseded by these source corrections.

### B3-06 `d94aaa8e1`, report `07e0a4656` — shared recovery, test lifecycle open

The actual runtime now receives the capacity-one proxy pool, and the test uses
the production `prepare_private_validator()` entry point. Write and readback
therefore share that pool. Checkout and PID query have one configured timeout;
the nested production wake condition is flattened. These supersede the previous
split-pool and wake-condition findings. No Cargo/native pass is claimed.

Before building, correct the connected fixture lifecycle in the package owner:

- Stop and await the existing turn worker through
  `test_pg::stop_worker_for_state_fixture()` immediately after runtime startup,
  before creating the controlled queued selection. The ordinary worker invokes
  `reconcile_private_originals()` before claiming work and can prepare this same
  original before the manual faulted call (or consume the armed fault itself).
  The test's rig mutex does not lock the runtime worker. Apply this to the other
  new manually controlled preparation/rollback cases as appropriate; do not
  introduce a fake production success flag or replay. Preserve separate live
  Worker execution acceptance. Assert no private original exists before arming.
- Keep the proxy and pool available until `runtime.shutdown().await` completes;
  close the pool and shut down the transport afterwards. The former split-pool
  teardown order cannot be reused after making this the runtime's shared pool.
  This avoids races between active lifecycle owners and closed infrastructure.

The earlier report whitespace and PID wording remain presentation corrections;
a distinct PID proves a different observed session, not completed old-backend
termination. Production connection custody stays with the existing PG owner.

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

### B1 follow-up `d70f23958` / `825591821`, report `c0cdb7ccd`

The writer now explicitly rejects authored declarations before database access.
This closes the unsupported authored creation path while preserving the controlled
seed path and existing public read APIs. It is a bounded repair checkpoint, not
completion of B1-06 or general Pure approval. The dormant subject-binding code
below the guard is not evidence of supported approval execution.

Remaining owner corrections concern an unused `legacy_evidence_json` test helper
and the future activation instructions. The actual selection table is
`reborn_component_validator_selections`, not
`reborn_retained_validator_selections`. Rejecting only `preparation-only` would
still admit the Source-only `qualified-pure-source-prerequisite` profile;
`source_qualification() == None` also does not prove complete Q1. Reopening needs
a positively authenticated complete qualification from the existing owner.
Trusted pure/workflow Q2 writing remains B1-06, with B3-09 consuming that interface.

All eight listed tests, including the three ordinary `#[test]` functions, are
inside the `postgres,skills-db`-gated module. The handoff's unit selectors without
features may select zero tests. Root's direct Rust 1.100 formatter check passes
on the writer and test file with unchanged source. No Cargo, PostgreSQL run or
full B1 acceptance is claimed.

## B2-02 — `/2` reader and retention, initial `27267af66` review

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

### Follow-up `78147448b`, report `e0be06d8f`

The owner moves the shared structural contract into `brassclaw_skills`, reused
by the Engine adapter and review-selection store without a crate cycle.
Explicit post-deserialization checks now reject nil UUIDs and zero versions in
implementation/own-root references. Parsed records retain private fields and
binary checksum values. These source corrections close the corresponding initial
findings; no unit/native pass is claimed.

The complete four-file follow-up and its test sources are inspected. Integration
remains blocked by the following connected findings:

1. The shared `/1` branch returns after checking the tag, while the store removes
   its previous unknown-top-level-field rejection. Preserve the strict historical
   `/1` schema, including rejection of `/2` extensions under a `/1` tag, and test
   the actual retention path.
2. The first parse and builder hardcode `REVISION_LIMITS`. The store applies its
   acknowledged/original limits only during a second complete parse. Pass the
   applicable limits into the shared contract from the start and reuse the parsed
   result rather than rejecting eligible originals or decoding twice. Existing
   bounded historical behavior and explicit capacity classification remain.
3. Base Recipe/variant/component and invocation-layout values remain opaque;
   duplicate step-order entries are not rejected. Keep database graph/evidence
   authentication in its actual owner, but validate the full wire shape and
   static reference/step invariants in the structural owner. Unit positive
   fixtures currently use native byte-array Recipe checksums and variants without
   the actual retained step link, rather than the existing serializer's wire
   shape. Use genuine supported serialized examples, including hex checksums for
   base references, and negative cases isolating each invariant.
4. Root's direct `rustup run 1.100.0 rustfmt --check --edition 2024` over the new
   contract, Engine adapter and changed store exits 1. The full 699-line output
   is retained at `/private/tmp/brassclaw-b2-02-78147448b-rustfmt-review.log`.
   This is a formatting check only; no Cargo or product execution occurred.

`cargo test -p brassclaw_skills selection_contract_tests` selects the new unit
tests. PostgreSQL retention/migration acceptance still needs `db-store` consumers
and the real backend. Neither a default-feature unit run nor a roundtrip fixture
can replace those cases. V215 remains reserved and unexecuted by this review.

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

### B3-06 follow-up `8f46752d6`, report `6655c63e0`

The five allocation-head reads now share RepeatableRead, and the write uses
the existing EvidenceClient guard with recycling only after acknowledged commit.
These source changes address mixed-read snapshots and uncertain connection
recycling. They do not establish qualified selection or bounded reconciliation.

Further source findings returned to the owner:

- The new checksum query reads `reborn_component_revisions.checksum` into
  `Vec<u8>`. V101 and the current revision-store reader define that column as
  hex `TEXT`; this typed `Row::get` is incompatible and can panic. Reuse the
  checked revision reader rather than duplicating a wrong wire conversion.
- The same transaction still selects `last_version` allocation heads. Coherent
  drafts are not qualified packaged references. No check establishes the
  provenance claimed in the method comment. B1-07/B2-04 own the actual qualified
  selector; absent support must remain explicit rather than use draft heads.
- `bootstrap_evidence::io::acquire` supplies custody only. It does not apply the
  acknowledged Store deadline. The write/readback calls remain outside the
  reconciler timeout without a replacement bounded original-resolution contract.
- The new BEFORE-INSERT trigger exercises a real statement failure/rollback,
  not lost COMMIT acknowledgement. The remaining comment claiming Case 3b
  covers real lost ACK is incorrect. Keep the rollback test, restrict its fault
  to the exact submission, and add genuine held/lost-ACK acceptance through the
  existing `AckCutTransport` support.

Root's direct Rust 1.100 formatter check passes on both changed Rust files with
unchanged source. No Rust compilation or PostgreSQL execution occurs here.

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

### B3-09 follow-up `ff186d9b8`

The wrapper now constructs a React element for the inner component, correcting
the missing component boundary. Root reruns the changed panel/queue fixtures:
35 pass, zero fail/skip. They exercise static rendering, not actual React
rerender lifecycle or backend decisions. The unchanged provider baseline is
reused rather than rerun.

The added Rust class-22 helper is not accepted:

1. `update_python_code_validation_status` is placed inside `impl RecipeStore for
   PgRecipeStoreFacade`, but the trait declares no such method. This is a
   source-visible compilation error; use an appropriate existing implementation
   boundary, without inventing an unnecessary trait requirement.
2. `ValidationQueueStore::approve` performs legacy mutable graduation and writes
   its receipt. It does not supply B1's qualified immutable Pure approval and
   submission link. A `human` string or queue status cannot substitute for the
   trusted typed decision writer. Keep unsupported profiles closed.
3. Requests still carry no expected original view. After one review is completed
   and a successor review of the same UUID is prepared, an old browser can approve
   the new internally consistent review. Server-side internal consistency alone
   cannot detect that the operator viewed the earlier subject. Bind the actual
   displayed original/evidence/base identity through the existing decision surface.
4. The new helper maps `ReviewChanged` to `RecipeStoreError::Invalid`; the actual
   services mapper converts that to HTTP 400. The claimed 409 path is not preserved.
5. The rejection branch still writes `rejected` without a matching Q2 row. Reject
   absent/wrong review state rather than mutate an unrelated approved projection.
   Return actual decision metadata: fixed previous status/attempt counts and the
   missing-readback fallback to `validated` do not prove success.

B1-06 owns the trusted backend dispatcher and immutable decision transaction;
B3-09 owns presentation/client wiring. Return these corrections to their owners
before full-feature compilation; the UI pass does not qualify the Rust change.

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
