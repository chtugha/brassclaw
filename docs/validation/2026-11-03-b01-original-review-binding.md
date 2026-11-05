# B-01 original review and queue binding — source handoff

Date: 2026-11-03. Branch: `codex/b01-original-review-binding`.
Base: `a4f308dc472b6f5b63e5825ad096f626895f6040`.
Owner/scope: bounded B1-02 original submission/candidate capture; migration V206.
The source handoff and review corrections below retain their original verification
scope. The final section records the subsequently authorized successful CLI
cross-build of 419a132e3. Native tests, component approval, activation and complete
B1/B2/B3 acceptance remain unclaimed.

## Implemented source boundary

The retained Q1 driver resolves the exact submission through the current queue's
immutable original binding. It no longer selects a submission by newest timestamp
for a component UUID. Capture and final Q1/Q2 mutation compare the same queue and
component bytes. Unbound historical queues stay visible but cannot acquire an
original through a guessed match or obtain retained Q1/Q2 graduation.

V206 retains the original request's target choice and per-queue binding separately
from mutable queue lifecycle. It verifies the real immutable subject, changed-root
reference, original base, full explicit legacy projection, scope, class and actual
locked candidate bytes. Deferred constraints require all changed roots in a
queued joint review. Bindings survive Q2 queue deletion; updates, deletion and
truncate are rejected. Historical pending rows retain their bytes/state/counters
and receive a false eligibility marker; new generations default true. No original
or review evidence is backfilled.

The existing operator submission endpoint accepts either explicit `queue_ids`,
`create_queue_project` for a new single-root class-21/22/23 legacy projection, or
neither for a standalone immutable submission. Existing required subject fields
and /1, /2, /3 subject meanings remain intact. Full candidate bytes and exact
references are required; omitted projection fields are rejected. Extra v3 fields
remain in the immutable revision. Creation preallocates component/queue identity,
locks queue before component/revision heads, and commits projection, queue,
subject, target choice and bindings together. An identical original retry reads
its original result, including after Q2 removes the queue. Changing bytes,
original ID/targets or scope cannot take over an earlier operation.

The immutable completed-turn producer uses the same transaction helper with an
explicit standalone target choice and its existing host-derived original ID.
Legacy create methods keep their existing draft behavior and use the same
queue-before-component ordering. This does not convert their incomplete inputs
into v3 subjects.

## Boundaries deliberately retained

* Joint /3 subjects retain their complete changed/context partner and dependency
  selection. Current /1 validator selection cannot execute these subjects, so
  both positive and negative partial queue writes are rejected. This package
  creates neither successor approval nor validator-selection semantics (B2).
* Target-choice records contain no Work claim, lease, scheduling state or wake;
  durable Work/admission/recovery remains B3.
* The wired legacy Sempai proposal sink still receives mutable constructor data
  without the immutable packet/original response identity. Its conversion belongs
  at the upstream producer boundary; it is not represented as completed here.
  Historical Docplan conversion is likewise not certified. Ordinary Docus
  class-17 editing remains outside the eight retained validator classes.
* Classes 0, 1–3 and 13 can bind explicit existing queue candidates only when the
  caller supplies a complete original revision and exact graph. They have no new
  legacy-create projection in this package.
* Existing Recipe lifecycle test fixtures remain explicitly test-only, with an
  unreviewed original and, for upgrades, a retained actual comparison base.
  They are not executable Q1/behavior evidence or a production reconstruction API.

## Verification performed

`git diff --check` and direct `rustfmt --edition 2024` formatting/parser checks
passed for the changed Rust source. Source self-review covered all new SQL
parameters/constructor arities, feature ownership, immutable retry/readback,
queue-before-component lock order and joint-review graduation guards.

No Cargo build, test-target compilation, Clippy, PostgreSQL migration execution,
native test or remote test was run in this package chat. No cloud build was
started; there is no build receipt to reuse. The supervisor must compile and run
the exact package before acceptance. Formatting is not type checking or SQL
execution. In particular, the migration and new native fault-transport test remain
unexecuted.

## Written tests for supervisor execution

The original eight tests in `pg_review_submission::queue_tests` use the real native
PostgreSQL harness and, where applicable, mounted operator HTTP:

* `original_queue_operator_create_retry_and_q2_historical_readback`
* `original_queue_a_cannot_certify_b_or_changed_cas`
* `original_queue_competing_authors_keep_one_atomic_original`
* `original_queue_lost_commit_ack_reads_the_original_without_reinsert`
* `original_queue_joint_context_is_retained_and_never_partially_graduated`
* `original_queue_recipe_and_catalogue_create_preserve_complete_documents`
* `original_queue_foreign_payload_and_historical_rows_fail_without_subject_allocation`
* `original_queue_upgrade_preserves_old_pending_bytes_without_backfill`

The existing actual retained Monty Q1 positive/negative fixtures in
`runtime/tests/validator_retained_q1_tests.rs` now capture their explicit original
queue binding before real validator execution. Re-run that module as the B00/B01
integration regression, plus affected Recipe manual Q2/rejection/upgrade and
validation-queue lifecycle tests. These are requested checks, not passing results.
Full changed-partner /3 runner acceptance remains unavailable and unclaimed.


## Supervisor corrections after 957b493c5

This correction remains source-only and unaccepted. No local Cargo, migration,
native test, browser runtime or Modal operation was executed. Direct rustfmt,
JavaScript parser checks (`node --check`) and `git diff --check` are the only
verification performed here.

### Visible original diagnosis, without Work state

`QueueRow` and the actual `ValidationQueueItem` API projection now expose an
additive `original_review` view: `bound`, `unresolved`, `legacy_unresolved`,
`missing_queue` or `joint_unsupported`, with original queue/submission IDs.
One read-only scoped relation carries the view for all eight classes through
facade/services/HTTP, replacing the old Recipe-only list and intermediate
class-wise append. Counts use the same relation, including missing-queue rows. No queue payload, old evidence, error list,
state or review counter is rewritten by diagnosis.

The existing Settings queue fetch includes both auto and manual buckets, so an
new unbound pending original is actually visible. New drafts explain explicit
binding needs; historical rows show unresolved custody and the missing safe
disposition/fencing owner without promising a resubmission path;
unsupported manual class actions and unresolved/joint partial approval stay
blocked. The existing submission panel now accepts explicit existing queue IDs
or the supported new-project projection and retains those selectors with the
same pending original across uncertain responses. It displays whether the
receipt is standalone or bound. These are binding facts, not B3 Work status.

### Failure classification and preparation order

One composition mapper preserves immutable-store `Database` and nested revision
`Database` errors as storage-unavailable; actual identity/CAS conflicts remain
`ReviewChanged`, and invalid input remains invalid. Capture, original readback,
final verification, Q2 and create wrappers use that classification. Pool/commit
failures remain unavailable. No such failure is written as a Q1 verdict.

The Q1 compatibility entry keeps bridge readiness and supported-class checks,
then resolves/captures the original and authenticates its exact subject before
reading/preparing/inspecting validator programs. A new unbound queue explicitly
defers as unresolved with resubmission required. A historical unbound queue
instead defers as unresolved legacy custody, with safe disposition/fencing not
connected and replay forbidden. A missing queue remains
`NotFound`; an inconsistent existing seal is an error. Both existing unbound
runtime deferral tests now assert the exact unresolved/action message.

### Transport size ownership

The former binding-specific 64-MiB quota was unjustified and is removed from
V206 and the reader. It was neither the 8-MiB revision-envelope contract
(`component_revision::REVISION_LIMITS`) nor the 16-MiB subject contract
(`review_submission_store::MAX_SUBJECT_BYTES`). The separate 64-MiB retained
snapshot bound (`revision_store::MAX_SNAPSHOT_BYTES`) covers a different object
and cannot be applied to a seal with escaped repeated strings. The operator
route/UI still owns its existing 14-MiB request-body limit. None was expanded.

The binding serializes three complete JSON text strings C, Q and R. JSON
encoding of any UTF-8 string requires at most six output bytes per input byte
(control characters), plus quotes; existing valid JSON text represents those
controls as escapes, so for C/Q/R the additional string encoding is at most
two bytes per text byte, plus quotes. The remainder is the small typed envelope
and exact references. These are expansion relationships, not a new quota:
legacy row/feedback/proposal TEXT fields do not share a bounded combined input
contract, so no new fixed seal ceiling can be derived from the revision limit.
The actual serialized length and existing PostgreSQL TEXT/JSONB/wire capacity
therefore determine storage feasibility. A technical DB/transport failure is
unavailable and preserves the exact original request for readback; nothing is
truncated. The reader uses strict typed Serde fields (duplicate/unknown fields
rejected), checks canonical reserialization/checksum and rereads the real
retained subject/projection, without applying an unrelated byte/node quota to
the expanded host-generated wrapper.

### Actual caller meanings and remaining cutover

| Caller | Actual input and source behavior | Queue/Q1 meaning |
| --- | --- | --- |
| Operator complete submission without target | Full immutable candidate, exact graph/base/partner and original request ID through the common transaction helper | `standalone`: genuine retained original subject; no legacy queue seal or automatic graduation |
| Operator explicit `queue_ids` | Same full original plus exact queue selectors through common capture | `bound`: all changed roots and actual payload/CAS are captured atomically; no B3 scheduling claim |
| Operator `create_queue_project` | Same full original plus explicit project, classes 21/22/23 through actual row constructors and common capture | `bound`: pending projection, queue and original commit together; the bare store does not create a B3 wake |
| Completed-turn `submit_candidates` | Host-derived original IDs, original candidate bytes, exact base/graph through the same helper | Explicitly `standalone`, `queue_review_ready=false`; original retention, not legacy queue submission or B3 admission |
| Legacy Sempai constructor sink | Constructor fields only; lacks exact original revision/graph/response identity | Saves unresolved drafts and queue tracking only. Result fields/logs now say `*_drafts_saved`, never successful immutable-review counts. Real draft tests assert absent original IDs and `unresolved` |
| Legacy Docplan upsert | Historical constructor draft, not a full immutable revision | Still a legacy draft; no fabricated original or v3 submission claim |
| Ordinary Docus edit | Existing class-17 document edit path | Unchanged; not one of the eight retained validator classes |

No valid original is reconstructed from incomplete constructor rows, and no
legitimate draft/document save is disabled. The legacy Sempai/Docplan producers
are not upgraded into full immutable authoring by relabeling their output. Their
upstream conversion, general class creation, joint successor validator/approval
and B3 durable admission remain unaccepted work. Therefore this source package
still does **not** satisfy the complete B1-02 scheduler/Q1 exit or any overall
plan completion label.

Additional native tests written, not executed:

* `original_queue_escaped_source_near_revision_limit_roundtrips_without_seal_quota`
  uses source-like quotes/backslashes/newlines within two encoded lines of the
  existing revision maximum, through real operator capture and original retry.
  The actual queued row also carries server-side quoted/backslashed formatter
  output; the resulting seal is asserted larger than the removed 64-MiB ceiling,
  while the original revision and HTTP body stay inside their existing limits.
* `original_queue_http_projects_unresolved_historical_and_bound_states_without_mutation`
  checks the real mounted queue API, both Recipe and PythonCode projections,
  pending counts, historical diagnosis and unchanged original queue bytes.
* `original_queue_broken_transport_preserves_unavailable_and_original_readback`
  closes the real PostgreSQL proxy during an original-read transaction and checks
  unavailable classification and subsequent original-ID HTTP readback.

The existing lost-commit-ACK test now requires HTTP 503 rather than accepting any
error status. The existing real Sempai test asserts draft-only unresolved queue
views. The existing retained Q1 deferral module keeps its runtime and queue setup
and checks the explicit unresolved original contract; assertions were strengthened,
not removed. All these cases, including new SQL projections and JS runtime UI,
remain for supervisor compilation/execution and acceptance.


### Complete v1 field inventory correction

The supervisor found that the original Tool branch omitted V055 dependency
metadata. Inspection also found Tool prior-knowledge/override fields from V082
and the Skill legacy compatibility/activation/prerequisite fields still read and
written by `DbSkillStore`, plus authored lineage and Recipe processing status.
Constructor structs are therefore not treated as the full semantic inventory.

V206 now defines the fixed v1 field set once in `review_queue_fields`, with
`review_queue_projection`, `review_queue_agrees` and `review_queue_proposed` as
the shared SQL owner. Retained Rust capture uses the same overlay, checks full
original root documents with that agreement function before allocating revisions,
and then retains exact bytes. The immutable reader and independent insert
trigger use that same agreement definition. The base comparison uses it too.
No second Rust field whitelist is introduced. The old non-retained document/note
overlay remains unchanged for its existing lifecycle.

The inventory was checked against the actual CREATE TABLE migrations V027,
V030, V033, V037, V052 and V053; additions V046, V050, V055, V069, V070, V071,
V079, V082 and V085; and removals V064, V071, V072 and V077. Later migrations
through this source's V205 do not add component-table columns. Relevant current
consumers include `DbSkillStore::{SkillWriteInput,DbSkillRow}`, class stores,
retrieval/effective-content assembly, Recipe metadata/status readers and the
existing formatter/bootstrap integrity owners.

| Classification | Treatment |
| --- | --- |
| Authoring/content/binding/dependencies | Required explicitly, including Tool dependency registry/prior knowledge, Skill compatibility/license/allowed tools/version/activation arrays/token field/setup/prerequisites, ToolSkill parameters/includes, Recipe processing status and class-specific payloads |
| Lineage | `similarity_parent_id`, `replaces_id`, `parent_version` required explicitly; missing is not silently replaced with null |
| Identity/scope/class | Verified independently against the exact root and locked queue/component row |
| Sequence/provenance/derived output | `prompt_uid`, `source`, `content_hash`, `content_checksum`, `formatted_content` belong to their existing host/sequence/similarity/bootstrap/formatter owners; they are not missing-author-field defaults or approval evidence |
| Lifecycle/accounting | The existing exclusions in `review_component_bytes` are reused rather than duplicated |

All non-lifecycle row bytes remain CAS-protected, including the host/derived
fields. If an original explicitly supplies a key that also exists on the actual
row, the agreement function requires equality even when that key was not
mandatory. V3-only metadata remains in the immutable original and exact graph.
The projection is fixed for `/1`, not recomputed from the newest schema at
runtime: otherwise a later column addition could invalidate old readback. A new
semantic column needs an explicit successor contract/reader migration; the native
schema-inventory test detects an unclassified addition rather than silently
changing the meaning of old seals.

Two additional real-PG tests written, not executed:

* `original_queue_v1_projection_inventories_every_current_class_column` compares
  the SQL contract against the actual migrated PostgreSQL attributes for all
  eight classes, reusing the same SQL classifications (no Rust field list).
* `original_queue_tool_skill_and_binding_capture_rejects_missing_and_mismatched_metadata`
  exercises classes 0, 1, 2, 3 and 13. It supplies real full original graphs,
  preserves an exact unchanged code partner for Skill function subjects, rejects
  omitted dependency metadata and mismatched binding/prerequisite/activation/
  lineage values, checks no allocation/queue mutation, then captures/readbacks the
  exact matching original. A second path deliberately bypasses the caller's
  precheck using the real immutable writer and a correctly checksummed seal;
  the actual DB trigger must reject its different dependency metadata and roll
  the whole transaction back. These are capture/integrity tests, not fake Q1 or
  executable prerequisite qualification.

The previously written 21/22/23 and actual retained-Monty Q1 fixtures now include
explicit lineage fields; new Recipe projection fixtures declare processing status.
All new cases and SQL remain unexecuted and unaccepted pending supervisor checks.


### Global queue paging, rejection threshold and honest legacy custody

The intermediate append projection was removed. The existing list owner now
builds one scoped relation of all eight classes and their queue/missing-queue
state. Both list and count read this relation. One query globally orders by
queue submission time (component creation time when no queue exists), then
class and stable component UUID. The existing `MAX_RECIPE_LIST_ROWS` value is
reused unchanged as a **visible per-page size**, not a class quota or total
ceiling. The API accepts `after` and returns `page_size` and `next_cursor`;
lookahead determines whether another page exists. Typed cursors bind the scope,
project and bucket and carry only the last sort key. Cursor data grants no
authority; actual scope filtering is mandatory on every query.

This is an explicitly live keyset listing, not a retained catalogue/task
snapshot: concurrent lifecycle changes can change page membership, and refresh
starts another traversal. Stable keys and class-independent ordering prevent a
full Recipe prefix from permanently hiding another class. The Settings client
follows every page in both auto/manual buckets, deduplicates by class/UUID and
merges them in the same ordering. A page/transport failure remains an error,
not a success containing a silently incomplete prefix.

Recipe and non-Recipe Revision/Rejection filtering and displayed buckets now
use `queue_store.reject_threshold()`, the same configured owner value used by
actual rejection writes. The copied `<3`/`>=3` Recipe/append classifications
were removed. No new Settings service, threshold value or higher row cap was
introduced.

Legacy `required=false` rows represent **unresolved custody**, potentially with
unknown execution. There is no currently connected safe operator transition
that preserves the stable UUID/evidence, fences/reconciles unknown execution
and creates a new queue generation. UNIQUE(scope,component_id) and the existing
state-4 purge are not that transition. The UI therefore states this missing
B1/B3 disposition/fencing boundary and forbids replay, rather than promising an
already executable resubmit action. The Q1 compatibility entry emits the same
custody diagnosis before validator work; capture and Q2 reject it through a
distinct typed error. Fresh capture rejects historical eligibility before
revision allocation. No product path flips `required`, deletes the old pending
entry, creates a replacement generation or replays unknown work. New
`required=true` unbound drafts retain the explicit complete-original binding
path. Tests represent historical rows at insertion/migration time, not by
turning a new row into an eligible historical replacement.

Additional native tests written, not executed:

* `original_queue_http_pages_full_recipe_stock_without_class_starvation`: fills
  the actual existing Recipe page size, adds earlier and later PythonCode queue
  entries, and follows the real mounted HTTP cursor. It checks page bounds,
  next-page reachability, strict global ordering, no duplicate/lost UUID and
  rejection of a cursor reused for another bucket.
* `pg_recipe_store::tests::native_queue_projection_uses_the_reject_writer_threshold_for_every_class`:
  actual rejection writer at threshold 5 produces counter-4 Revision rows for
  Recipe and PythonCode; the real facade projects both using that owner. An
  existing per-store threshold of 2 classifies both retained rows consistently
  as Rejection. The state-2 setup is explicitly a lifecycle fixture, not Q1
  execution/evidence.

There are now fourteen `original_queue_*` cases plus this threshold case, the
modified genuine retained Q1 modules and Sempai lifecycle coverage. The existing
runtime deferral test also asserts historical custody with no resubmit promise.
Source-only checks remain the only executed checks; the newly connected query,
ports, pagination, migration and UI still require supervisor compilation and
native/browser verification. The escaped-source test deliberately exceeds the
removed seal ceiling using real server-side formatter data and should be run
with its memory footprint in mind; no smaller input/transport policy is relaxed.


### Persisted state-4 rejection visibility correction

Rejection pages now include persisted state 4 independently of the current
threshold, as well as state-3 rows at/above that threshold. A later higher
threshold cannot revive or hide an actual deletion candidate. The wire preserves
its existing distinct `queue_code="garbage"`, `validation_status="garbage"`;
state-3 threshold rejections keep `q4_rejection`/`rejected`. Counts reuse the
same projected lifecycle status, so state-4 rows count under garbage and are
not silently relabeled as rejected/pending. Legacy component-only garbage also
projects state 4. The queue status badge uses the existing negative tone for
both rejected and garbage. Approval gates and cleanup behavior are unchanged.

Added native test (written, not executed):
`original_queue_rejection_api_retains_writer_deletion_candidates_and_counts`.
Actual Recipe and PythonCode rejection writes cross the configured owner
threshold into state 4; the mounted Rejection API must return both, Revision
must exclude them, garbage/rejected/pending counts must agree, and attempted
Recipe validation must remain rejected with both terminal queue states intact.
Q2 state setup is explicitly a lifecycle fixture, not Q1/evidence qualification.
The existing facade threshold test now also crosses the threshold through the
real writer, raises the read-owner threshold above the persisted counters and
checks that both classes remain terminal/visible/countable under garbage.

There are now fifteen `original_queue_*` tests plus the facade threshold test.
This correction has only parser/format/diff verification; no Cargo, SQL migration,
native test or Modal compilation was run. Full B1-02/plan acceptance remains open.


## Authorized frozen-source Modal CLI build — 2026-11-03

The supervisor authorized exactly one bundled CLI compilation after reviewing
`419a132e3d18da259392041fcdd0d21a9e3f84f6`. Compiler inputs stayed unchanged for
capture, synchronization, worker execution and receipt collection. The worktree
was clean at that HEAD before launch and after completion. This report-only
update is subsequent to the captured source and is not another compiler run.

The Modal skill was read; its version and the existing SDK both report 1.6.3.
Existing Apple activation pins `im-S1cpT9G1fsMPpwjISvBSlK`, qualified by job
`7da58631-e7e7-4faa-a144-025eb4f1845f`. No image/setup/activation change was made.
Actual process/lock and persisted unsettled-job checks found no competing remote
run. The existing controller held `/private/tmp/brassclaw-modal-build.lock`
continuously across sync, upload/readback verification, submission and terminal
receipt/artifact collection. Local native work remained independent.

One initial **local launcher** used Homebrew Python without the installed Modal
SDK and failed at `import modal`, before entering the lock, synchronizing source
or starting an App/function. That terminal log is retained. The launcher was
corrected to the existing Miniconda SDK interpreter; no source/profile changed.
There was exactly **one paid compiler job**, with no compiler retry.

| Receipt field | Actual value |
| --- | --- |
| Source HEAD | `419a132e3d18da259392041fcdd0d21a9e3f84f6` |
| Source Git tree | `f0e37f946917572172bc1b78fba3658ecf16cb75` |
| Source manifest SHA-256 | `636b2f8924c766de45acf22c771f64927db1af9f8f25876f8653d15c49015fda` |
| Source entries | 10,160, with exact file identities in `source.json` |
| Job | `c44d9601-93af-4156-81c8-35c87d24918d` |
| App | `ap-gCNsQzDuXnCxFSUwMnMvz6` |
| Function call | `fc-01M6GNJFFC9GYEWTYP249APX16` |
| Environment / Volume | `brassclaw` / `brassclaw-build` |
| Profile | 8 CPU reserved/maximum, 32,768 MiB reserved/maximum, 8 Cargo jobs, 900-second timeout, 30-second shutdown grace, non-preemptible |
| Expected local duration | 480 seconds, authorized from comparable 6–8-minute CLI batches |
| Worker return code | **0** |
| Worker duration | **353.8550000190735 seconds** |
| Cargo completion | `Finished dev profile [unoptimized + debuginfo] target(s) in 5m 03s` |
| Final remote state | Actual App listing: `stopped`, `tasks=0`; controller `state.json=completed`, `returncode=0` |

The exact compilation command was:

```text
cargo build --locked -p brassclaw_reborn_cli --bin brassclaw-reborn --target aarch64-apple-darwin
```

The existing controller, qualified image, resource settings and compiler/cache
owners were reused. Upload/readback verification reported archive SHA-256
`e025c68bad5b845a9e8d65ce562486742493e58f81cd788018fccec31f629047`,
143 changed files relative to the retained remote baseline, zero deletions and
1,168,648 compressed bytes. The worker verified the admitted complete source
before compiling. Request, completion and actual source-manifest hashes agree.

Artifacts:

* Downloaded `brassclaw-reborn`: **337,439,792 bytes**, actual local `file` reports
  **Mach-O 64-bit executable arm64**. Local SHA-256 was recomputed and matches
  the worker/controller receipt:
  `02e1c110a90d738946b8460c2d40119d6c85c9fedd46c847d0ad193dd33639ff`.
* Worker-verified pgvector archive:
  `aarch64-apple-darwin/debug/build/brassclaw_embedded_postgres-ca1c2a1628a504f0/out/pgvector.tar.gz`,
  **73,888 bytes**, SHA-256
  `e4e03c5defc87c5540c2b294cd32b6b7010d54afa54dd37376ab89e8520f3b1f`.
  The exact path/hash is in the completion receipt; the controller downloads the
  CLI binary but does not separately download path-bearing artifact keys.

Full actual evidence directory:
`/Users/ollama/brassclaw-target/modal-build/jobs/c44d9601-93af-4156-81c8-35c87d24918d/`.
It contains `request.json`, `source.json`, `applied.json`, `completed.json`,
`state.json`, `cargo.log`, full `controller.log`, downloaded binary,
`image-provenance.json`, actual `app-list.json`/`app-final.json`, launcher
`launch.json`/`terminal.json`, initial local-preflight failure record, original
worker/controller/source-sync/settings inputs, `verification.json` and
`evidence-index.json` with file identities. Original launcher logs are also kept
under `/private/tmp/brassclaw-b01-modal-419a132e3-20261103*`.
App URL: <https://modal.com/apps/chtugha/brassclaw/ap-gCNsQzDuXnCxFSUwMnMvz6>.

This establishes the exact default-feature Apple CLI compilation and recorded
artifact identities, **not** test-target compilation, SQL/migration execution,
Clippy, native binary execution, native/runtime/browser testing, activation or
B1-02/whole-plan acceptance. No local Cargo, remote tests, native artifact startup
or additional paid job was run. Native acceptance remains supervisor-owned.


## UI regression test-source correction — 2026-11-04

Supervisor review requested test-source coverage for the newly connected UI
contracts while the integrated main remains frozen for native compilation.
Only test sources and this report change in the package worktree; production
JavaScript, Rust, SQL, DTOs and API behavior remain unchanged. No Cargo, Modal
job or paid retry was started.

The existing controlled VM/React-hook style is retained. Test transports produce
controlled JS values/errors only: these cases do not certify a PostgreSQL seal,
Q1, human Q2, activation, durable Work or actual browser acceptance.

* `component-submission-panel.test.mjs`: preserves the original seven cases and
  adds five. Existing-queue and create-project targets keep their exact original
  selector values, order, candidate, ID and observed counter after an unknown
  commit; controls and new-request action stay locked. An already queued stale
  selector event cannot retarget the pending retry. For both target kinds,
  inspection of a retained subject with the same ID but different selectors is
  visible only as readback, never substituted for the pending POST acknowledgement
  or original request. Standalone retains the old field absence after editing
  both hidden selector types.
* `validation-queue-paging.test.mjs`: four cases cover full traversal of both
  buckets, opaque cursor encoding, an empty intermediate page, class-qualified
  merging/deduplication, a live auto-to-manual transition, global time/class/ID
  ordering and old-wire timestamp fallback. A later-page failure in either
  bucket rejects the complete fetch, preserving the original error instead of
  returning a partial success. A non-advancing cursor fails explicitly.
* `validation-queue-tab.test.mjs`: eight cases load the actual English/German
  message packs and render the actual QueueRow with controlled hooks. Both
  legacy custody and joint-unsupported diagnoses/queue IDs must be visible;
  Recipe Q2 approval is disabled. Joint Skill/PythonCode rows expose no partial
  Q2 actions and show the missing-action message. Positive bound-Recipe and
  new-unbound controls ensure the fixture is not simply blocking every row.
  No mutation success callback or backend approval receipt is synthesized.

Total: 24 UI cases across these three files, 17 newly added. Direct `node --check`
parser checks and `git diff --check` passed. **The tests themselves were not run
in this package chat**; local execution/acceptance remains supervisor-owned.
Suggested bundled local command:

```text
node --test crates/brassclaw_webui_v2_static/static/js/pages/settings/components/component-submission-panel.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/validation-queue-paging.test.mjs crates/brassclaw_webui_v2_static/static/js/pages/settings/components/validation-queue-tab.test.mjs
```

The successful Modal CLI receipt above still applies only to its frozen
419a132e3 inputs. These test-source additions neither broaden that receipt nor
claim completion of the open B1-02/whole-plan/native acceptance scope.

## Native run-8 lifecycle fixture correction — 2026-11-05

Supervisor run-8 reported a successful native test compilation (463.38 seconds)
and 33/36 passing tests. The two new rejection fixtures failed at their direct
`state=2` setup with `q1_review_seal_complete`, before reaching the rejection
writer. This section records their source correction, not a successful rerun.

Both fixtures now seed only their counter while pending, then use the existing
test-only `ValidationQueueStore::gate1_pass` lifecycle owner. That owner captures
actual candidate/queue bytes and seals through the existing reviewed-state writer.
Its existing Recipe fixture support now also permits PythonCode with explicitly
empty `includes` and null `dependency_registry`; it does not infer dependencies
or Skill associations. The resulting originals remain unreviewed lifecycle
fixtures, never executable Q1/behavior/approval evidence.

The threshold projection test uses fresh terminal candidates in a separate
project scope after checking the nonterminal threshold projections. It does not
reopen rejected originals or reuse their stale seals. Both classes still reach
terminal state through the actual rejection writer, and retain garbage labels,
counts and rejection-bucket placement after a higher threshold is configured.

Production paths, database constraints and approval contracts are unchanged.
Direct rustfmt parser/format checks and `git diff --check` pass. No Cargo command,
test execution or Modal job was started for this correction; supervisor-owned
native reruns remain required. The previous build receipt does not cover these
changed test inputs, and general B1/B2/B3 acceptance remains open.

## UI translation fixture correction — 2026-11-05

The supervisor executed all 24 UI cases after integration and reported 22 passes
and two failures in the new German queue-row cases. Their fixture loaded only
the German message pack and substituted raw keys for missing translations;
its button lookup then used an absent raw German value. The real `lib/i18n.js`
owner instead falls back to the English pack for those button labels.

The queue-row fixture now executes the actual i18n provider, language detection,
translation function and `useT` hook in its controlled VM, with actual English
and selected-language packs. Only React context/hook storage is controlled;
fallback and interpolation are not copied into the fixture. It asserts the
selected provider/document language and looks up the real rendered button by
the actual translated/fallback label. The visible localized diagnosis, queue
ID, disabled approval and absence of mutation calls remain required. Joint
non-Recipe rows still require the absence of both actions, using valid labels.

Only the test source and this report change. `node --check` and
`git diff --check` pass; the corrected cases were not executed in this package
chat. This correction accompanies `75d237b7c` for the supervisor's focused
JS/native reruns, with no extra Cargo compilation or Modal job here.
