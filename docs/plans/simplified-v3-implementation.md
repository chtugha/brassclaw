# Simplified v3 implementation record

## Recipe architecture target clarification — 2026-10-06

[recipe.md](../../recipe.md) defines the binding authoring/input/version contract.
Recipes instruct the orchestrator using Tools, many ToolSkills, many Skills and
many small PythonCode components. Each Recipe component step has one reference;
internal PythonCode composition is allowed. IBS must pin the newest activated
approved versions and their transitive includes in BuildInstruction at task start.
Approved versions are immutable; Q1 plus human Q2 activate authored replacements
without invalidating prior versions or altering existing task snapshots. Typed
inputs/results remain data, and Monty preserves each task's context. No newest
lookup on resume; current Tool policy remains live. Schema support, the target
inputs mapping, single-component enforcement and complete version manifests
remain implementation work. This entry records documentation decisions only.

This record distinguishes implemented changes from the binding target in
`simplified_v3.md`. It is not a declaration that the migration is complete.

## Phase 0 evidence

Run `python3 scripts/simplified_v3_inventory.py --output /tmp/simplified-v3-inventory.json`.
The JSON contains source locations and symbols, route and placeholder candidates,
settings-source candidates, test/CI files, and every Cargo feature declaration.
It does not read databases, secret files, or running-instance configuration.
Reference counts change as implementation proceeds; regenerate before review.
Consumer classification is explicitly unresolved until a semantic review verifies
all applicable uses. Filename or identifier matches alone never authorize deletion.

Initial scan: 21,478 reference lines, 739 route candidates, 314 placeholder
candidates, 396 test/CI files and 65 manifests. Candidates include comments and
tests; these counts are not counts of public runtime routes or actual defects.

## Binding security decisions and migration dispositions

The single-operator target is already authorized by the plan. WebUI login proves
operator access to this instance. It does not prove that a remote channel sender,
model output, recipe or extension has authority to execute host effects.

| Existing dimension | Target disposition | Required evidence before removal |
| --- | --- | --- |
| WebUI account, role and deployment mode | One owner authenticated by instance token | Invalid/absent token, session revocation, rotation, CSRF/Origin and external-channel negative tests |
| Tenant/User/Project authorization | Instance-level capability policy | Enumerate every comparison and SQL filter; distinguish authorization from data ownership/FKs |
| Project/Thread/Run/Message IDs | Retain relational identity | Stable transcript order, exact message input, reply routing, parent/child references, replay/dedup tests |
| RuntimeProfile selectors | One fixed security policy | Preserve actual sandbox, process, filesystem, network and secret gates; reject obsolete config explicitly |
| Capability grants and approvals | Current instance-wide tool allow/block settings and technical rules; no per-invocation approval | Check current settings immediately before dispatch, including running recipes; reject stale/cancelled attempts separately from tool permission |
| Secret scope/AAD/path | Instance ID plus stable secret handle | Decrypt with original AAD before any key rewrite, re-encrypt and verify; ambiguity aborts migration |
| Actor/principal/channel identity | Operator identity or external provenance as appropriate | External OAuth identities and remote senders must never acquire operator rights implicitly |
| Monty conversation registry | One runtime-owned global service | Boot readiness, bounded admission, addressed cancellation, durable continuations and one-process ownership |

Token theft grants operator access, so token/session redaction, constant-time
verification, rotation and revocation remain mandatory. Browser XSS and CSRF must
not turn a logged-in browser into an unguarded host-effect path. Remote listening
remains explicit with loopback defaults; Origin/TLS/reverse-proxy requirements
survive removal of account management.

Prompt injection, untrusted Python/extension code and malicious MCP results stay
below the capability/kernel boundary. SSRF and subprocess environments remain
controlled by brokered network and secret access, sandboxing and resource limits.
Backups must preserve old keys until encrypted data and relational migrations
have been verified. Unknown external-effect outcomes after a crash require
reconciliation, not automatic replay.

## Implemented prerequisites

- Runtime component initialization moved from WebUI construction into
  `component_boot.rs`, called by `build_reborn_runtime` before worker/trigger
  startup. Migration proof is still `BootedDb`. Required seeding and queue
  recovery failures now abort boot; integrity failures remain fatal. All prompt
  bodies load before any process-wide prompt initialization. Each loaded body
  and its checksum are verified together against the compiled seed, so an edit
  after the earlier integrity scan cannot install an unverified prompt.
- The Monty driver resolves the exact `msg:<message_id>` accepted reference via
  `SessionThreadService::submitted_user_message`. The durable message must be
  User/Submitted, belong to the expected thread/turn/run, and have content.
  Missing/invalid references, lookup errors and linkage mismatches fail before
  effect-executor construction or VM driving. There is no latest-message or
  empty-string fallback.
- A test against the pinned Monty v0.0.16 proves that one VM can suspend A,
  complete B and then resume A without replaying either external wait.

## Remaining dependencies before production cutover

No phase has been declared complete. The global service is not yet connected to
production and the driver still owns per-conversation sessions. The owner/auth,
scope/policy, edition, settings/UI and persisted-data migrations remain open.
The inventory still requires per-consumer classification, actual route mappings
and a settings catalogue with defaults/ranges/restart semantics.

Monty's upstream `FunctionCall::resume_pending` and incremental
`ResolveFutures::resume` support async interleaving. `host.run_program` currently
fully awaits `execute_code`, so it needs a retained nested continuation keyed to
task/attempt. Durable blocked-run transitions must release the worker claim;
keeping a parent waiting on a worker-owned oneshot is insufficient.

The root `LimitedTracker` currently counts lifetime allocations and wall time.
Its `FunctionCall` offers `tracker_mut`, while `ResolveFutures` does not. A global
runtime needs bounded live-heap accounting and separately enforced task budgets,
excluding idle/approval time. The continuation test does not prove these limits.
Do not remove resource limits or reset global live-memory accounting to conceal
the mismatch.

The converted composition fixtures now start private native embedded PostgreSQL
instances and fail visibly on setup or cleanup errors. Embedded PostgreSQL tests,
component boot and the prefix-capability migration have run against real databases.
This does not establish backup/restore or product cutover acceptance. Other
unconverted fixtures still need inventory. Existing operator secret and instance
data have not been rewritten, migrated or deleted.

## Verification

- `cargo test -p brassclaw_reborn_config --all-targets`: 63 passed.
- `cargo test -p brassclaw_threads --all-targets`: all passed, including the
  eight new exact-input contract tests.
- `cargo test -p brassclaw_engine --test global_monty_continuation`: one passed
  against the pinned upstream VM; this is an API prerequisite test only.
- `cargo test -p brassclaw_architecture --all-targets`: 29 passed.
- `cargo check -p brassclaw_reborn_composition --features skills-db`: passed.
- Strict `cargo clippy` for composition, threads and engine with `--all-targets`,
  `skills-db` and `-D warnings`: passed. Two pre-existing expression-style
  warnings and one unused test-only helper were cleaned up during validation.
- Changed Rust files pass `rustfmt --check`; `git diff --check` passes.

Commands use the NVMe target directory required by the repository. These are
targeted checks, not the full workspace, PostgreSQL or browser acceptance suite.

The initially failing `--no-default-features` boundaries were corrected; strict
composition Clippy passed for that configuration before the second pending batch.
This is compatibility maintenance, not a decision to retain product editions.

## Revised-plan impact review — 2026-10-06

Reviewed the working-tree plan against the committed prerequisite milestone
`839c37f1` and the pending implementation diff before further implementation or
compilation. Sections 8 and 9 and the live Monty settings requirements are binding.
The latest revision also rewrites Phases 2 and 3 around global tool rules and
adds section 1.1: explicit Recipe failure outcomes, fixed component revisions,
fair execution, measured performance and a shared live-heap limit. Remaining
older approval wording must not guide the implementation.
The implementation queue was paused for this review. After removing the
incompatible addition and correcting the architecture guidance, the remaining
verification was restarted in one sequential background screen queue.

| Existing implementation | Disposition under the revised plan |
| --- | --- |
| Shared migration/seeding/integrity boot chain | Retain. Still required before the global VM and producers. Preserve integrity checking; editable intents require a distinct operator-edit contract, not disabling checks on executable system code. |
| Exact accepted message lookup and prior-context cutoff API | Retain. Message/thread/run linkage is data integrity, not operator authorization. The prior-context API is not yet wired into production prompting. |
| Mark-Submitted linkage and bounded pending-admission recovery | Retain, pending verification. Prevents workers executing an uncommitted input; does not introduce a tool approval. |
| Run/runner/claim attempt address and attempt-specific cancellation | Retain. The worker claim token fences execution and never enters Python or model context. It is not a capability approval lease. Dispatch/reply fencing is still incomplete. |
| Parent/child CAS aggregate and V089 thread links | Retain as execution-state persistence, pending verification. Independent transcripts remain separate. Tenant keys are transitional storage compatibility, not the final operator-access contract. |
| Durable run projection for external authentication | Retain the data projection, then adapt ownership checks to instance access and explicit flow/run identity. Third-party authentication remains required. User/project comparisons must not remain extra operator-role gates. |
| Newly wired durable operation-approval locator in runtime.rs | Removed from this pending batch; the original local-only locator and legacy approval composition remain. The durable run projection is now used only for external authentication. Full removal of existing operation-approval workflow still requires the coordinated global-tool-policy cutover. This rollback does not claim that tools no longer require legacy approvals. |
| Scope/actor filtering in run and interaction adapters | Transitional compatibility only. Convert operator authorization to instance authentication while preserving exact object/flow selection. Never remove every filter mechanically: wrong-run resolution is a separate correctness defect. |
| V088 prefix capability namespace and schema-key corrections | Retain. These restore registered tool identity and do not grant permission or alter intent examples, variants or learned entries. |
| Native PostgreSQL fixtures and reliable shutdown ownership | Retain. They supply real migration/concurrency evidence without silent skips. |
| No-default feature-boundary repairs | Retain while affected configurations exist. Phase 4 still decides and removes obsolete product features explicitly. |

### Required changes before completing the affected phases

1. Replace operation-approval policy, invocation grants/leases and approval UI
   together with authoritative global tool settings. Every actual dispatch must
   read the current setting, including dispatches from already running recipes.
   ToolSkill binding confers no permission. Keep attempt freshness, sandbox,
   network, secrets and resource enforcement as separate checks. Inventory
   blocked legacy approval runs and migrate/reconcile them explicitly; do not
   auto-approve pending effects or replay an in-flight tool call. Retain old audit
   evidence. Tests must prove allow/block changes without restart and absence of
   an additional operation-approval layer, rather than require new approval leases.
2. Keep external OAuth/service authentication, instance-token authentication and
   human Q2 component validation. Their waits are not tool permission approvals.
   Revise global Monty control events and tests accordingly; cancellation and
   child/auth continuation still require addressed, nonblocking delivery.
3. Implement the three Monty limits as a validated, versioned setting set with
   persisted/effective/pending/error state. The latest contract counts task VM
   time and task allocations separately from the shared live Monty heap. Reject
   an unsafe heap-limit reduction while retaining the previous effective
   revision; a global heap error may require supervised service recovery. The current engine uses fixed memory
   and allocation constants. The pinned tracker only exposes a duration setter
   that resets its clock; using it alone violates preservation of consumed time.
   Its time/memory/allocation errors become `RunError::UncatchableExc`, so Python
   exception handling cannot isolate a root-VM resource failure to one task.
   Resolve task time/allocation accounting, failure isolation and bounded
   execution/control response in the engine/Monty contract before live updates.
   A shared-heap fault must not be misreported as a safely isolated task fault.
   Preserve VM generation and counters; do not claim
   success merely because DB settings changed.
4. Add the intent-management work from section 8 before WebUI acceptance. The
   generic upsert passes `step_link = None`, and `seed_intent_input` overwrites it
   on conflict; the list uses `LIMIT 500`. Add stable row/variant addressing,
   pagination and transactional component-plus-matching edits. Preserve learned
   entries and review status. Define explicit system-component edit/tombstone
   ownership so boot/repair cannot silently undo operator intent edits. Matching
   preview must use the production resolver without learning, score mutation,
   tool execution or LLM calls. Persist the actual executed component, variant
   and tier with the turn; never reconstruct history from today's matching data.

### Verification boundary at the first review

At the first review the sequential queue had stopped at a missing
`DurableAuditLog` test import. Those statements described that review only.
Subsequent verification passed the composition runtime integration suite
(8 tests, including the new child-tree cases), then the focused external-auth
suite (2 tests) after correcting its fixture to use the runtime's PostgreSQL
turn store. The full composition library run before that fixture correction
reported 691 passed and 8 failed; there has been no full rerun since, and seven
other recorded failures remain unresolved. Strict all-target Clippy passed for
the tested composition/runtime crates, and strict composition Clippy passed
without default features. The worker (18), exact-input (11), host-tool (129)
and embedded PostgreSQL checks described above remain earlier results.

The later instance-policy draft and CapabilityHost hooks were added after those
successful checks and are uncompiled and unverified. They have no production
policy-source wiring. No global VM service, live-limit update, global-tool
authorization cutover or intent editor is complete.

### Adjustments applied before resuming plan steps

- Removed the added PostgreSQL operation-approval projection and its production
  wiring. Kept the durable external-authentication projection and shared native
  fixture improvements. Existing operation-approval fixtures describe legacy
  behavior only; they are not v3 acceptance requirements.
- Updated root agent/development architecture guidance with the current Recipe,
  global-tool-policy and resource-accounting contracts. The user-edited plan was
  preserved; a guarded edit detected its newer contents before writing anything.
- Keep V088 and V089: neither changes operator permissions, secret AAD or
  production tenant partitioning. V089 links execution-state aggregates only;
  the migration must not be represented as a full instance-data migration.
- The previous fixed-limit session implementation has not been extended. Live
  settings, fair VM execution and selected component revisions remain required
  work rather than fabricated successful settings/API behavior.
- The existing Python orchestrator still has prohibited Recipe-error/Tier-2
  fallback behavior. Section 1.1 now requires a typed task-error protocol and
  caller-level tests before this production execution path can be called ready.
  Do not replace it with an exception that destroys the future shared VM or a
  successful-looking error reply. The same migration must update system seed
  checksums and preserve already executed effects.

These adjustments remove incompatible pending work and correct the implementation
contract. They do not complete the global authorization or VM cutover. No further
plan phase should be marked complete on the basis of these changes.

## Latest plan impact review — adaptive memory and default-off token budgets

Re-read the revised plan before continuing implementation. This review compares
the committed prerequisite milestone and the pending changes; it does not alter
the user-edited plan or claim new test results. No Cargo execution was started.

### Compatible work to retain

Shared boot prerequisites, verified component loading, exact accepted-input
selection, Message/Turn/Run linkage, attempt addressing, native PostgreSQL
fixtures and parent/child aggregate persistence remain required. V088 and V089
are new migrations and preserve existing migration checksums. Their targeted
tests do not establish production backup/restore or the instance-data cutover.
The PostgreSQL build script makes newly added migrations invalidate the compiled
migration bundle; it does not change stored migration history.

### Pending work that must change before production wiring

1. **History retrieval:** `submitted_turn_input` currently applies
   `take(max_history_messages)`. This is a bounded context helper, not a complete
   prompt contract. When token budgets are disabled, callers must not silently
   substitute that suffix for the full eligible transcript. Separate bounded
   resident caches from paginated durable history access and prompt assembly;
   preserve the exact input cutoff and redaction checks. Technical model context
   limits still require an explicit handling policy. Do not solve this by an
   unbounded VM cache or by passing `usize::MAX` to every consumer.
2. **Cancellation:** the worker's new `stop_attempt(...).await` has no deadline.
   Add bounded acknowledgement and supervised failure handling before connecting
   a global service. A timeout must fence further effects/replies by that attempt
   rather than assume that dropping the wait stopped execution.
3. **Policy draft:** `InstanceToolAuthorizer` and the host preparation recheck
   follow the global permission direction but are not an accepted cutover.
   `InstanceToolRule.revision` is not propagated into prepared dispatch or audit;
   no revision-checked settings store exists. Define the dispatch admission point
   and coordinate it with settings publication, including changes during awaited
   preparation and the gap before effect execution. Obligation equality alone
   does not prove revision consistency or concurrent revocation. Legacy trait
   defaults still permit operation approvals and skip rechecks; production
   composition must prove all supported paths use the instance policy. Trace
   trust ceilings back to their source so old profiles, scoped grants and token
   ceilings cannot survive as hidden policy layers. Preserve technical isolation.
4. **Token switch:** the existing settings default is false, but V060's historical
   database default is true and retrieval paths still unconditionally pass
   `RETRIEVAL_TOKEN_BUDGET` and `PRIOR_KNOWLEDGE_TOKEN_BUDGET`. Inventory all task,
   history, retrieval, recipe-hint and repeated-retrieval consumers. Feed one live
   instance switch through those ports; preserve counting and actual provider
   limits. Change defaults with a new migration and explicitly preserve operator
   choices; do not edit V060 or indiscriminately strip resource ceilings.
5. **Adaptive memory:** replace the proposed fixed shared-heap settings design
   with automatic mode, reserve and optional manual cap. Separate desired and
   effective settings revisions from measured budget targets and adjustments.
   Verify platform/container measurement sources, finite fallback, freshness,
   growth, hysteresis, pressure relief and backpressure. Automatic reductions
   below the live heap remain pending; unsafe manual reductions are rejected.
   Track root plus nested Monty heap without double counting, separately from
   process RAM/PostgreSQL/system pressure. Preserve task time/allocation counters
   and the 600-second default for task compute time. The pinned tracker API gap
   still prevents claiming this is implemented.
6. **Components and intents:** keep the existing integrity checks, but design
   immutable selected revisions, transactional activation and consistent
   matching/component/prefix publication before editable intents are wired.
   System edits need explicit override copies with Q1/human Q2 and persistent
   deactivation rules; boot/repair must not overwrite them. Current seeding tests
   do not prove that new override contract. Root Monty code remains fixed for its
   service generation; ordinary validated intent changes need no service restart.
7. **Acceptance and documentation:** extend the settings catalog, API/UI and
   caller-level matrix for adaptive memory and disabled token budgets. Amend root
   guidance that describes only manual heap-limit rejection; automatic pending
   reductions are distinct. Before global production wiring, measure VM quantum
   and control bounds and establish the performance baseline required by §5.1.

These changes are prerequisites for the affected implementation packages. Keep
the verified data/boot work, correct the incompatible pending contracts first,
then resume the plan. The global orchestrator and policy cutover remain open.

## Intermediate publication verification — 2026-10-06

The operator explicitly requested publication of the current multi-agent working
tree to `main`, rather than waiting for complete plan implementation. The changelog
and root `CLAUDE.md`/`AGENTS.md` distinguish target requirements from current code.

Checks completed for this publication:

- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo test -p brassclaw_authorization -p brassclaw_capabilities -p brassclaw_threads --all-targets`: passed.
- `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo check -p brassclaw_reborn_composition --features skills-db --all-targets`: passed, including compilation of test targets.

The composition database/browser suites and the full workspace acceptance matrix
were not rerun for this publication. These checks do not establish completion of
the global Monty, live-budget, global-policy, intent-management or migration cutover.

Other agents continued editing during these checks. The publication also includes
the latest 600-second compiled defaults, bounded stop acknowledgement and optional
full-history lookup changes collected at staging. The checks above ran during
collection and do not certify every final concurrent edit in this checkpoint.
The final staged diff passed `git diff --cached --check`.

## Corrections and resource prerequisites implemented after the latest review

The user additionally requires a WebUI duration change to affect Rust and Monty
through one effective revision. Phase 3a now explicitly requires that shared
publication, preserved task consumption and runtime acknowledgement; an independent
Rust wall-clock copy is not an acceptable compute-time implementation.

Implemented and checked in this batch:

- Complete eligible submitted history can be requested explicitly with `None`;
  an explicit window remains available. A 131-message regression checks the full
  transcript exceeds the old 128-message window without changing the admitted
  input or cutoff. Redaction policy remains exercised. This API still needs the
  global production prompt adapter; it is not an unbounded resident VM cache.
- Stop acknowledgement is bounded to five seconds. Failed/unacknowledged stop
  prevents this worker from taking another claim and persists
  `monty_stop_unacknowledged`, rather than reporting a safely stopped task.
  Global-service fencing/supervision remains required before production cutover.
- The instance authorizer returns the admitted revision and rejects legacy
  operation-approval resumes. `LiveInstanceToolPolicy` publishes whole coherent
  generations using compare-and-publish. A production-host test publishes a
  block during awaited preparation, verifies cleanup and zero dispatch, and
  verifies stale publication cannot overwrite it. Technical changes, missing
  rules and unavailable policy also prevent dispatch. Legacy production factory
  wiring has not yet been replaced; no global permission cutover is claimed.
- Shared task settings publish time/allocation/token mode atomically via watch.
  Task consumption is retained across changes and accounting overflow fails
  closed. Rust and VM subscribers observe the same revision. These primitives
  require the durable settings/driver bridge, not separate startup snapshots.
- The pinned Monty custom `ResourceTracker` seam is implemented. Real parked-VM
  tests prove live allocation and duration enforcement without resetting recorded
  consumption. Time consumption in the duration test is injected as accumulated
  active segments: this is a tracker API proof, not actual production compute-time
  measurement or a fairness proof. Pure-Python quantum/yield and task-isolated
  resource failure still need their separate production prerequisites.
- Adaptive heap calculation accounts for additional capacity plus existing heap,
  then subtracts reserve once. It grows in configured increments on demand,
  rejects unsafe manual caps, leaves automatic reductions below live heap pending,
  applies backpressure, and retains a finite limit on absent/stale/future/overflowing
  measurements. Its parameters have no guessed RAM-percentage defaults. Platform
  measurement, reclamation, shared heap ownership and runtime/UI bridge remain
  explicit implementation requirements; calculator tests do not prove those.
- V090 changes only new-row defaults to 600 seconds and token budgets disabled.
  Native PostgreSQL tests apply it twice and verify stored 300-second/enabled
  choices survive. V034/V060 checksums remain untouched. Compiled duration defaults
  also use 600; existing explicitly stored values remain authoritative until the
  controlled instance-settings migration.

Verification ran in sequential background screen queues, never simultaneous
Cargo. Free NVMe space was checked before each Cargo execution. Passed: 12 exact
input tests, 19 worker tests, capability all-target tests, 4 focused instance-policy
caller tests, authorization library tests, 8 Monty resource-calculation/accounting
tests, 2 actual parked-VM tracker tests, and the native PostgreSQL V090 test.
Final strict all-target Clippy passed for engine, resources, authorization,
capabilities, Reborn, threads and composition with `skills-db`. No warning was
suppressed and no failing/ignored test was converted into a success.

During this batch another publication produced checkpoint `9469c496`, including
some concurrent history/default/cancellation edits. The tests above subsequently
checked the current files; the earlier checkpoint's checks were not treated as
certification of concurrent changes.

Remaining production work is material: the WebUI still uses legacy scoped settings
and Rust retains a startup wall-clock snapshot; Monty sessions still use the old
tracker. Connect one durable instance-settings revision to both consumers and
actual compute accounting, then complete measured fairness and the global VM
service. Retrieval/prior-knowledge fixed caps must consume the same token switch.
Component/intent revisions and safe override activation, migration backup/restore,
and the seven previously recorded composition failures remain open. These are
unfinished plan work, not resolved issues or evidence that the WebUI already
changes an active global service.

Upstream API review: current Monty has changed its tracker/API substantially; its
public [RunProgress source](https://github.com/pydantic/monty/blob/main/crates/monty/src/run_progress.rs)
shows host-call/name/future suspension states, without a general CPU-quantum yield
variant. Its [resource-limit documentation](https://pydantic.dev/docs/monty/limitations/resource_limits/)
still describes resource-error termination. Updating blindly cannot be counted as
proof of fair, resumable pure-Python execution. The tested custom tracker targets
the repository's pinned version; a dependency upgrade must undergo the same
continuation, failure-isolation and fairness acceptance before production wiring.


### Reconciliation with the revised plan (2026-10-06)

The plan now makes Monty 1.0.0 an upgrade gate before global production wiring
and adds §10, PostgreSQL-only provider definitions. The earlier passing tests
remain historical results for Monty 0.0.16; they do not certify either new gate.
The tagged 1.0.0 source was inspected separately from `main`.

| Already implemented area | Reconciliation and current disposition |
|---|---|
| Component boot prerequisites, integrity verification and `BootedDb` | Keep. These remain prerequisites before global VM and worker startup. Provider bootstrap is an additional prerequisite; current WebUI provider seeding does not satisfy it. |
| Exact submitted input/history cutoff and optional full history | Keep. Task input addressing and persistent history are still required; these changes do not depend on the Monty tracker API. |
| Attempt-addressed signals, bounded stop acknowledgement and honest terminal failure | Keep. Global hosting must additionally fence stale attempts and supervise all owners; stopping one worker is not instance-wide recovery. |
| Global tool policy snapshot/CAS and admission after awaited obligation preparation | Keep. §9 still requires current global rules at dispatch. Production factory/settings wiring remains absent; these tests are not a completed policy cutover. |
| V090 new-row defaults (600 seconds, token budgets off) | Keep. Applied migrations and explicit operator values remain untouched. V090 neither migrates scopes nor changes existing settings, and provides no live runtime acknowledgement. |
| Adaptive heap calculation | Keep as a pure calculator. Its `live_heap_bytes` input requires proven interpreter ownership; process-global allocator usage cannot simply be supplied as Monty heap usage. No allocator acceptance is claimed. |
| Custom tracker published in the engine | Corrected: moved to private `cfg(test)` `legacy_resource_tracker`. Monty 1.0 has a concrete tracker, so the old trait is no longer exposed as the proposed production seam. Both legacy compatibility tests still run. |
| Neutral task limits and consumption | Corrected: removed the mandatory allocation-count field from the new, unwired neutral API. Added `SharedMontyTaskBudget`: cloned readers use one mutex-protected account and the same live settings handle. Checking usage holds a coherent settings revision; exhaustion/overflow remains terminal even after a limit increase. |

Allocation-count retirement in the **legacy persisted/API contract is not yet
implemented**. Existing DB values and the old interpreter limit are unchanged.
The neutral API correction neither discards operator configuration nor pretends
that Monty 1.0 enforces an allocation-count limit. The explicit legacy-value
migration, UI/API change and acceptance must land together before that cutover.

The new shared-account tests prove its accounting contract: readers observe the
same revision and consumption, lowering a limit exhausts the same task, raising
it does not revive that task, a new task starts with fresh consumption, overflow
fails closed, and a simulated external wait adds no compute debit. They do not
prove actual interpreter timing, bounded CPU preemption or WebUI uptake. Hosting
must record each non-overlapping active segment once and include nested execution
without double counting. Rust and Monty must clone the same task account rather
than construct separate accounts from the same settings handle.

#### Tagged 1.0.0 constraints for the next migration batch

- `monty-types/src/resource.rs`: `elapsed()` is cumulative active execution time;
  feed/turn clocks are narrower scopes. The setters reset their accumulators.
  Use the cumulative clock to establish task deltas, retaining consumption outside
  feed/turn setters and across nested contexts. A remaining-time setter between
  host transitions alone does not implement live changes during a pure Python loop.
- `monty/src/run_progress.rs`: `RunProgress` and `FunctionCall` are no longer
  generic over the custom tracker; parked function calls expose `tracker()` and
  `tracker_mut()`. All used suspension states and completion/error ownership need
  compatibility checks, rather than mechanically replacing `LimitedTracker`.
- `monty-alloc/src/lib.rs`: live bytes, baseline and hard limit are process-global
  atomics; exceeding the hard limit aborts/exits the process, skipping destructors.
  Installing this allocator on the BrassClaw application process would include
  unrelated Rust allocations and expand the failure domain to the whole product.
  Do not install it there as an alleged isolated Monty heap backstop. Resolve a
  long-lived isolated host or a proven scoped allocator extension before wiring
  memory enforcement; neither a per-task VM/process nor an extra Rust Recipe loop
  is an acceptable shortcut.
- Build consumers found: root minimum 1.94, engine/composition minima 1.92,
  Docker builder 1.94, both Monty dependencies pinned to 0.0.16. These must change
  coherently with adapter/allocator/continuation tests and the rollback contract,
  rather than shipping a version-only bump.

#### Provider impact of §10

The existing provider path remains incompatible with the new target. Confirmed
consumers include `registry.rs` embedded JSON/file overlays, `llm_catalog.rs`
file boot resolution, WebUI `seed_builtin_providers`, `PgProviderRepo::upsert_builtin`,
LLM configuration saves, CLI resolution, migration step 4, Docker COPY and the
E2E workflow path filter. `upsert_builtin` currently clears `deleted_at`, while
`delete` rejects builtin rows. Removing only the UI guard would allow restart
reactivation. Provider cutover must therefore include DB-only bootstrap/readers,
removal of recurring seeding, soft-delete/reactivation with stable IDs, selection
transactions, preserved secret references and a providerless management boot.
No files/importers or existing definitions have been deleted in this correction
batch; the required backup/conflict migration is still outstanding.

Verification of this correction batch: sequential screen queue completed with
9 resource/accounting/calculator tests, 2 legacy interpreter compatibility tests,
and strict all-target Clippy for engine/resources. All passed with no ignored
cases or lint suppression. NVMe space was checked before each Cargo command.
The seven prior composition failures and the global service, 1.0 migration,
provider cutover and WebUI acknowledgement remain open, explicitly unverified.


### Upgrade evidence and Recipe failure contract (2026-10-06)

The production dependencies remain on 0.0.16 until the complete Phase 3a gate
passes. `tests/monty_v1_upgrade` is an independent, locked workspace for the
actual tagged 1.0.0 interpreter/types/allocator. Its tests cover graph arguments,
bound function values, dependent tool results, fresh snippet state, repeated
work waits in one VM, async A→B→A, host error/abort, capped stdout, cumulative
execution timing, and the actual orchestrator source with deterministic host
replies. Only the separate allocator probe executable installs `monty-alloc`.
Soft memory exhaustion returns an error; hard exhaustion exits that child with
code 65, after which another child runs successfully. Containment deadlines reap
children. This proves isolated allocator enforcement, not heap attribution or a
production long-lived hosting transport.

`MontyTaskClock` now attaches a hosting-owned cursor to the shared task account.
It debits cumulative-clock differences once per cursor, retains consumption over
settings/feed changes and nested interpreter executions, and fails the shared
task permanently on clock regression. Baselines exclude earlier execution.
The actual 1.0 root/nested clock test verifies the account and both readers:
parked waits add nothing, duplicate checkpoints add nothing, nested execution
adds its own elapsed time, and changing the feed limit cannot clear consumption.
This uses explicit final host checkpoints. Upstream completion/error clock
recovery, attribution between concurrent tasks, active-loop live updates and
resumable CPU scheduling are still missing; these tests are not production
WebUI-uptake or global-failure-isolation acceptance.

The actual `basic_mode.py` now separates genuine No-Match, ambiguity and
technical errors. A selected Recipe executes once; composition/step failure
never switches to Tier 2 or calls Kohai directly. Missing/failed No-Match
instructions fail explicitly. History persistence failure is visible after the
reply; it never retries completed effects. Fixed error markers map to sanitized
stage failures while raw tracebacks stay in debug detail. The legacy Tier-0
engine/loop bridge is also corrected: invalid/failed channels return
errors, the entire channel is validated before effects, and missing/failed
execution fails the selected Recipe without entering PromptStage/ModelStage.
Successful empty output is still success.

The No-Match/history **seed Recipes remain incomplete**: they assume cross-step
state despite fresh `{}` isolation, and their nested canonical host bindings
are not implemented. Removing the hidden direct-LLM fallback exposes this gap;
it does not repair those Recipes. Protected source checksum changes require the
verified repair/upgrade path on existing databases, with overrides preserved.
Do not bypass integrity checks or count mock sequencing as native execution.

The upgrade CI job runs the locked new and legacy workspaces and strict Clippy
on Rust 1.96. Benchmark workspaces are separate because old/new optional PyO3
dependencies conflict on Cargo's `links=python` uniqueness. Both use one helper,
10 warmups and 200 samples of compile/start plus 1,500 integer additions. Local
Rust 1.98.1 debug measurements: old p50/p95 510,625/857,750 ns, new
525,000/758,542 ns (+2.8%/-11.6%). This microbenchmark excludes kernel/IPC/DB/LLM
and does not certify end-to-end §5.1 performance or product speedup.

Initial batch evidence: 530 engine unit tests, 19 standalone 1.0 checks, 38
resource tests, native PostgreSQL boot/integrity regression, and affected strict
Clippy passed; no ignored tests or warning suppression. The earlier full
composition run passed 691 and failed eight. The WebUI facade test now checks
terminal state instead of concealing failure as a projection timeout; its focused
run confirms `Failed` / sanitized `driver_failed`. Source tracing identifies the
legacy driver dependence on a bare UUID engine Thread, whereas product
conversation IDs use `reborn-conv-...`; global exact task-context handoff must
replace that dependence. Do not strip prefixes, invent engine threads or add a
per-chat fallback to make these tests pass. The other seven runtime/model/gate
failures remain open. Subsequent failure-contract and minimum-toolchain results
will be recorded separately; earlier passing checks do not certify later edits.


Final evidence for this batch: 531 engine unit tests; 359 agent-loop unit/
integration tests; 47 composition orchestrator-adapter tests; 93 resource unit/
integration tests; 19 tagged 1.0 checks; the legacy benchmark; the native boot/
integrity regression; and all 29 architecture checks passed. Both isolated
Monty workspaces passed tests and strict Clippy on the installed Rust 1.96.0
minimum toolchain. Affected engine/loop/turns/composition/resources strict
all-target Clippy passed. The staged Monty CI selection changes passed their
25 shell cases and workflow/roll-up parsing. All Cargo commands ran sequentially
in screen with disk checks; no ignored cases or lint suppression were added.

The first architecture run exposed the boot test's duplicate socket reservation
inside composition. It was corrected by reusing the existing isolated native
PostgreSQL fixture and its supervised cleanup, rather than exempting the file
from the boundary scan. Native boot and the complete boundary suite then passed.
The legacy loop's retrieval backend errors and ambiguity now stop before prompt/
model requests as well; canonical caller tests enforce both. Ambiguity is an
explicit selection-required failure until the operator selection workflow is
implemented. Optional legacy hosts without a retrieval bridge and the Tier-1
optional prior-knowledge path still require the complete global cutover; these
checks do not certify a DB-only Recipe path for every old framework host.

The earlier full-composition 691-pass/eight-failure result remains an unresolved
integration baseline, not a passing full suite or evidence for later concurrent
changes. Production Monty 1.0 adoption, global Ready/inbox/lease/supervision,
bounded resumable CPU control, durable shared WebUI acknowledgement, supported
allocation/heap migration, native No-Match/history Instructions, provider DB-only
cutover, safe component revisions and backup/restore remain unfinished. No
runtime prerequisite, user data or existing operator setting was deleted.

### Complete eligible history and live retrieval token settings (2026-10-06)

`submitted_turn_input` applies a requested history suffix only after eligibility
filtering. `None` returns complete eligible history before the exact admitted
input; redacted/draft records cannot consume the suffix. PostgreSQL exact context
lookup verifies the entire thread scope and preserves requested ordering with a
linear index. The native PostgreSQL caller test covers 139 eligible messages,
a redacted suffix, a newer input, a bounded suffix and cross-agent rejection.

Retrieval and prior-knowledge APIs now accept `Option<usize>`: `None` explicitly
means no token ceiling. The fixed 4,096 ceiling was removed from Recipe and
orchestrator adapters. Production adapters read the existing scoped settings
store per call; disabling, enabling and disabling again is verified through
actual PostgreSQL settings and retrieval. Settings/source failures propagate;
they do not silently become empty or unlimited retrieval. Exact Recipe programs
are not truncated. This does not complete the instance-wide settings migration,
shared runtime acknowledgement, task accounting, or global history/prompt wiring.

Sequential screen verification passed: all-target thread/agent-loop/engine tests,
composition native PostgreSQL tests, affected all-target strict Clippy and all
architecture tests. No simultaneous Cargo executions or warning suppression.
The previously recorded eight composition integration failures remain open.

### Revision-checked Monty settings persistence (2026-10-06)

V091 preserves every existing settings value and adds a nonnegative desired
revision. GET returns that revision (zero for an absent row). Durable writes
require `expected_revision`; missing revisions are invalid and stale revisions
map to HTTP 409. Creation, field-wise patch and revision advance are atomic.
Rejected first-write claims roll back the provisional default row. Responses
return their own committed generation rather than re-reading a later edit.
Checked numeric conversions reject overflow and invalid values before mutation.
The WebUI sends its loaded revision and now sends/edits the token-budget switch.
This desired revision is explicitly not a runtime uptake acknowledgement; the
legacy scoped settings store and global Rust/Monty acknowledgement remain work.

The actual native PostgreSQL concurrent-first-write test passed, including
stale retry, preserving the winning patch, scope isolation, invalid boundaries
and absent-row rollback. All five native composition tests, 72 product unit
tests, four Monty WebUI handler contracts and affected strict all-target Clippy
passed sequentially in screen. JavaScript syntax checks passed. Existing mock
HTTP contracts do not prove production runtime status, restart or live uptake.

### Attempt stop receipts and final/error CPU clocks (2026-10-06)

The legacy driver no longer acknowledges a stop merely because its bounded
channel accepted the signal. Each exact claim registration owns a completion
receipt; stop waits for that receipt with a five-second bound. Duplicate live
registration is rejected without replacing the original sender. Missing/closed
receipts and full queues fail explicitly. The runner retains its independent
supervision deadline and admission quarantine. Actual channel tests cover
consumed-but-unacknowledged Stop, deadline, late claims, duplicate registration
and missing receipts. All 13 driver tests, selected runner tests, affected strict
all-target Clippy and all architecture checks passed sequentially in screen.
This is acknowledgement of the driving future's termination; detached external
effects, reply-transaction claim fencing and global-host quiescence/reconciliation
still require their separate production contracts.

Actual tagged Monty 1.0 tests establish an alternative to the `MontyRun`
final-clock gap: `MontyRepl` retains its tracker on successful completion and
inside `ReplStartError`, including timeout. Both final clocks feed the same
neutral task account without duplicate debit or reset during revision changes.
A real infinite Python loop wrapped in `except BaseException` times out rather
than reaching the subsequent host effect. Three actual task-accounting tests
and all-target strict upgrade-workspace Clippy passed on Rust 1.96.0. This proves
clock recovery with the REPL API, not production task attribution, resumable CPU
quanta, active native-operation control, global boot or live WebUI uptake. A
failed Recipe must still end; its mutated REPL must not be continued or replayed.

### Database writer cutover and current integration result (2026-10-06)

V092 rejects settings updates unless they advance the stored revision by exactly
one. This prevents legacy/unversioned SQL writers from changing values without
invalidating the WebUI's expected revision. Stop older writer binaries before
applying this migration; their writes now fail explicitly. A binary rollback
requires its compatible database state, not merely switching executables. The
native PostgreSQL test verifies rejection of unchanged, regressed and skipped
revisions while preserving both the accepted values and current revision.
No historical migration or stored operator value was rewritten.

The full current composition library run completed: **699 passed, eight failed,
zero ignored**. The eight failures are the previously identified runtime
system-prompt, model/tool/workspace, WebUI message/approval and yolo integration
cases. The legacy driver still depends on an engine UUID Thread, and canonical
No-Match/history programs still need the task-context/global-host cutover.
These tests were neither suppressed nor rewritten to accept failure. The full
run predates V092; the five actual native PostgreSQL tests and strict composition
all-target Clippy were re-run for that migration and its guard regression.

Remaining consumer risks include startup copies/scopes for model budgeting,
legacy message-count context ceilings, and the token switch disabling the whole
USD accountant instead of only token enforcement. The retrieval changes prove
complete eligible retrieval at its callers, not removal of all remaining task,
history and monetary-policy defects. Global production Monty hosting, allocator
integration/adaptive measurement, fair CPU control, claim-fenced effects/replies,
shared effective WebUI revisions and immutable component revision/override
management remain unimplemented. The REPL final-clock proof closes one upgrade
prerequisite; it does not authorize skipping the remaining Phase 3a gate.


### Durable approval composition failure (2026-10-06)

One of the eight composition failures is resolved. Approval interactions now
use the same scoped run-state reader as external-auth interactions, including
the PostgreSQL-backed reader. The empty PostgreSQL approval locator is removed.
Approval interaction scope preserves the full task-selection key, including
explicit thread ownership; reconstructing an actor-fallback scope previously
made parked WebUI runs disappear from listing. A mismatched actor cannot list
an explicitly owned task's approvals. These legacy selection fields do not
create new instance-wide tool policy or replace the pending policy cutover.

The existing native integration fixture now uses the durable approval and lease
stores selected by the runtime and starts its real capability invocation before
saving the approval, satisfying the database foreign key. It additionally
checks durable pending listing and actor rejection. Its original redacted audit
and SpawnProcess lease assertions remain intact. This fixes the fixture's
unused in-memory store writes without substituting a mock or changing its
successful-resolution expectation.

Sequential background screen validation passed the native regression, all
25 approval interaction contract tests and strict affected-package all-target
Clippy (composition with skills-db, product workflow). The full composition
library run now reports **700 passed, seven failed, zero ignored**. Log:
`/private/tmp/brassclaw-composition-after-approval.log`.

The seven remaining message-path failures are still unresolved. Independent
source review confirms no correct alternate production driver exists: opaque
product conversation IDs encounter a UUID-only engine Thread lookup before
admitted input resolution. Repair also requires canonical No-Match/history
Recipe variants and isolated-step data flow, nested host availability, and
prompt/model/capability/transcript operations through the neutral host ports
with actual run/turn identity. The current Kohai path bypasses those ports.
Their correct repair belongs to the global task-host cutover and its binding
Monty upgrade gate; this approval change does not complete that work. No IDs
were reformatted, engine Threads synthesized, legacy Rust loop restored, LLM
fallback introduced, or remaining failure expectations weakened.

### 2026-10-06 — Recipe compiler visibility and No-Match metadata repair

The binding Recipe-state contract was corrected with the operator: Monty owns
intermediate state within a Recipe execution, including required inputs/results
across child execution and waits. Mandatory fresh empty state per PythonCode
step is not the target. Earlier references here to isolated-step data flow are
superseded by that contract. AGENTS.md, CLAUDE.md and Phase 3a now describe it;
this documentation does not implement the missing task host.

IBS composition now follows tenant-anchored catalog visibility for validated
system Recipes/Actions and rejects pending components. Included components on
both channels must resolve, so a missing or unvalidated Rust ToolSkill cannot
silently disappear. The native regression exposed exact-scope-only name, UUID,
batch and registry retrieval too; those paths now admit validated system rows
consistently. Name resolution checks the requested class and deterministically
prefers an exact caller-scoped row over a system row with the same name. These
changes repair existing catalog readers, not the pending instance-policy cutover.

The seeded No-Match Recipe now carries canonical IBS step descriptions and a
variant with actual assembler, ToolSkill, tool and call-component UUIDs.
V093 upgrades the known legacy system metadata shape, retaining IDs and prose
annotations, leaving operator overrides and already authored IBS metadata
untouched. It does not modify PythonCode bodies or claim that this Recipe can
already execute through the production task host.

Sequential background screen validation: all eight focused composition/IBS
tests passed, including native PostgreSQL visibility, missing binding, malformed
metadata and idempotent upgrade/override checks. Strict engine/composition
all-target Clippy passed with composition/skills-db. Evidence:
`/private/tmp/brassclaw-composition-ibs-contract-v2.log` and
`/private/tmp/brassclaw-composition-ibs-lints-v2.log`.

Full composition library: **702 passed, seven failed, zero ignored** in
`/private/tmp/brassclaw-composition-after-ibs-repair.log`. The seven original
message-path failures remain. No opaque conversation ID was reformatted into
a UUID, engine Thread synthesized, per-chat/global-key fallback introduced,
legacy Rust loop restored, or failed Recipe replayed as direct LLM work.
Global hosting, Monty-owned Recipe state continuity, exact task handoff and
neutral prompt/model/capability/transcript wiring still need their implementation
and production-caller acceptance; Phase 3a and its Monty upgrade gate remain
incomplete. Do not mark any of the seven failures resolved from compiler tests.


### 2026-10-06 — Owned Monty task handoff and opaque input preflight

The turn-runner handoff now transfers an owned `Arc` host together with the
request and actual claim attempt. `MontyTaskHandoff` rejects mismatched request,
run, turn, resolved profile or context/scope conversation before dispatch. It
is deliberately not serializable or debug-printable; lease tokens remain in
Rust authority/transport. All five existing test adapters were updated to the
owned contract. This permits later global hosting to retain ports across waits;
it does not itself create a global service or establish cancellation quiescence.

The production driver now resolves the exact durable admitted input before
consulting the legacy engine store. The new thread-free reader keeps opaque
conversation IDs, asks the transcript boundary for complete eligible prior
history, and preserves pending admission as a retryable condition. A native
PostgreSQL regression uses an opaque `reborn-conv-...` ID, 142 real messages,
redaction and a newer message after the admitted input. It verifies 139 eligible
predecessors without truncation, pending admission, wrong-run rejection and
missing-reference rejection. The returned history is retained in the input
contract; the legacy executor still cannot consume it through neutral task ports.

The broader caller checks exposed a preexisting host-test boot omission: the
canonical compaction prompt was never initialized. Existing host/product
fixtures now initialize the actual canonical seed through the same initializer
used at production boot. Production readiness remains strict. The first compile
also exposed the distinction between `Arc<dyn Host>` and conversion from the
factory's `Box<dyn Host + Send + Sync>`; the owned type now preserves the exact
factory trait-object type. Strict all-target lint caught and removed imports
retired by the trait change. No diagnostic was suppressed.

Sequential screen checks passed: Turns/Reborn libraries (83 + 236), native
history/handoff regression (1), host integration (95), inbound integration
(14), shared product harness (5), and architecture checks (29). Logs are
`/private/tmp/brassclaw-monty-owned-handoff-{runner,history,host-v2,ingress-v2,harness,architecture}.log`.
Strict all-target Clippy passed for Turns, Reborn, composition and product
workflow with composition/skills-db in
`/private/tmp/brassclaw-monty-owned-handoff-lints-v3.log`.

**The seven original composition message-flow failures are not resolved.**
The full composition suite was not rerun: source review still shows its UUID-only
engine Thread requirement before execution. Next work must replace that executor
with the global task host under the Phase 3a Monty upgrade gate, preserve
Monty-owned Recipe step results/namespace and route model, tool and transcript
operations through exact task host ports. No ID coercion, synthesized Thread,
per-chat fallback or direct LLM bypass was introduced.

### 2026-10-06 — Explicit Recipe results and actual 1.0 host namespace

Monty now passes a Recipe-owned input/result dictionary to each
`host.run_program` call and records the preceding return value in that
dictionary. Rust forwards that explicit data to the nested interpreter instead
of replacing it with an empty dictionary. Each Recipe invocation creates its
own context; this does not provide arbitrary local-variable persistence or the
complete typed `inputs`/binding interface specified in `recipe.md`. Non-Match
receives the preceding history separately from the current query, and history
execution receives the actual answer. The seeded PythonCode bodies and global
production host still need their corresponding repair.

Tagged 1.0 checks now execute the unchanged orchestrator source with an actual
host object, rather than removing `host.` from the source. They verify the host
object ID and complete arguments: unlike 0.0.16, 1.0 does not include the receiver
in positional arguments. A real REPL check retains a prompt across Recipe
steps, preserves quotes/newlines/Unicode as data and rejects that prompt in an
unrelated Recipe context. These checks stop at the actual interpreter host-call
boundary; they do not establish production model dispatch or kernel enforcement.

Sequential screen validation passed: 93 engine orchestrator tests, 6 Recipe
failure-contract checks and 2 namespace/state checks. Strict all-target Clippy
passed for the engine and isolated upgrade workspace. Logs:
`/private/tmp/brassclaw-monty-recipe-handoff.log`,
`/private/tmp/brassclaw-monty-v1-host-state.log` and corresponding `-lints.log`
files. The source change also changes protected orchestrator seed content;
existing databases require the verified repair/upgrade path, never an integrity
bypass or replacement of operator overrides.

**The original seven failures remain unresolved and their tests are unchanged.**
No Rust-agent-loop or direct-model fallback was added. These are prerequisite
checks, not the global-host production acceptance required by Phase 3a.

### 2026-10-06 — Remove fabricated identity from the Monty model handoff

`KohaiCallCtx` now carries the actual admitted turn ID as well as the run ID.
The persistent driver installs this Rust-owned context before resuming a work
wait. The interpreter's host-call handler consumes it instead of deriving the
run from an engine Thread. Calls without that context fail before port dispatch.
Multiple model calls retain the IDs while advancing their iteration; returning
to the work wait clears the context. Installing a replacement during interrupted
execution is rejected, rather than attributing the old execution to a new task.

`PgKohaiPort` validates both IDs before prefix lookup, forensic persistence and
gateway dispatch. It no longer creates a replacement run or a fresh turn for
each model call. This fixes attribution in this existing model adapter; it does
not fix the adapter's missing prompt/capability host wiring or prove claim-loss
fencing, cancellation quiescence or the global lifecycle.

Sequential screen validation passed: 95 engine orchestrator tests and a native
PostgreSQL model-adapter regression. The latter reuses the existing recording
gateway and real prefix/interceptor stores, verifies exact run/turn IDs across
two calls, and verifies malformed IDs produce no gateway request or forensic
packet. Strict all-target Clippy passed for engine/composition with
composition/skills-db. Logs:
`/private/tmp/brassclaw-monty-kohai-identity-{engine,pg,lints}.log`.
The existing native boot/integrity regression also passed using the same
already-built composition test binary, without another Cargo compilation:
`/private/tmp/brassclaw-monty-kohai-boot.log`.

The original seven message-flow tests and their assertions remain unchanged.
They were not rerun for this downstream change: the production caller's legacy
UUID-only Thread lookup still precedes execution. They remain unresolved until
the thread-free task host and model/tool execution path are wired through global
Monty under the Phase 3a upgrade gate. No Rust-loop or direct-model fallback was
added.

### 2026-10-06 — Thread-free task host and prospective global Python lifecycle

`brassclaw_reborn::monty_task_host::MontyTaskHost` now owns the validated admitted
handoff and delegates individual prompt, model, capability and transcript calls
to that task's existing host ports. It preserves opaque conversation identity,
the resolved profile/route and claim address without constructing an engine
Thread. Structured model tool requests remain structured; Python decides the
next operation. Python-supplied pre-resolved messages and raw legacy Recipe
hints are rejected before port dispatch. Pinned Recipe/Skill context must be
resolved behind the host boundary rather than accepted as supplied component
bodies. The adapter checks existing cancellation observation before operations;
this is not proof of external quiescence or durable attempt fencing.

The persistent driver now retains this task host for its handoff, but still
enters the legacy UUID-only Thread path afterwards. **The original seven tests
remain unresolved and unchanged.** The adapter does not execute a Rust loop or
provide a direct-model fallback. Its successful caller checks use the existing
Reborn host fixture and gateway, including prompt grants, exact run/turn IDs,
cross-run rejection, structured tool output, capability denial, finalized reply
persistence and rejection after durable cancellation.

`engine/orchestrator/global_mode.py` supplies prospective global Python
sequencing with bounded async worker slots, task-local history and Recipe
results, explicit task routing tokens, ordinary host-error isolation, and an
explicit instance shutdown sentinel. It is not seeded, wired to boot, or an
alternative production fallback. It requires task-owned program references and
step IDs resolved from pinned manifests. Recipes post replies themselves and
return finalized message refs; the root does not re-post step output. History
inputs carry that reply ref. Task finish must verify the ref against actual
transcript evidence and retain the host's safe diagnostics/cancellation cause.

Activation still requires the full Phase 3a gate: the production global service
and framed hosting transport, Monty dependency migration/resource disposition,
bounded CPU/control and consumption-preserving live settings, neutral scoped
component/binding dispatch, pinned manifest context, Recipe/state child hosting,
No-Match/history seed repair, non-reply completion and verified exits. The new
Python source does not prove any of those caller behaviors. Do not replace the
protected orchestrator row or claim readiness from merely compiling this source.

Validation for this slice ran sequentially in screen: 98 Reborn host/runner
integration tests, the real Monty 1.0 global-lifecycle check, and architecture
tests passed. Strict all-target Clippy passed for Reborn/composition with
composition/skills-db and for the isolated Monty 1.0 workspace. Logs:
`/private/tmp/brassclaw-monty-task-host-full.log`,
`/private/tmp/brassclaw-monty-task-host-{lints,architecture}.log`, and
`/private/tmp/brassclaw-monty-global-lifecycle{,-lints}.log`.
The original seven were not rerun: this slice still reaches their known legacy
Thread blocker. The global source's successful Recipe/finish path remains
unverified until implemented against real component and transcript ports.


### 2026-10-06 — Typed No-Match/history components and history binding prerequisites

Fresh host seeds now load four PythonCode bodies from the component source files.
Prompt/history formatters consume `state["inputs"]`; Kohai and memory writing
consume `state["previous_result"]`. Every body assigns and returns `result`.
Runtime text is passed as values rather than interpolated into executable source.
The legacy Python orchestrator supplies this explicit nested input contract.
These seed changes do not replace stored PythonCode bodies or operator overrides.

Fresh `host-save-history` metadata now references actual formatter, memory
ToolSkill and writer UUIDs, with one component per IBS step and a default
`0:1-0:E` variant. The host seeder recovers the existing memory-write primitive
before referencing it; the later full capability pass recovers the same IDs.
Fresh system host Recipes declaring no LLM are inserted atomically with the
existing builtin Tier-0 maturity convention. Other insert callers retain their
original pending/seedling behavior; reseeding does not reset stored maturity,
metadata or overrides. Existing legacy history rows still require an explicit
revision-preserving upgrade; no automatic body or metadata rewrite was added.

The legacy text Kohai port preserves final assistant content, including empty
content, and rejects structured tool requests even when text deltas accompany
those requests. Their actual structured output is retained in the forensic
packet before the unsupported-output error propagates. Capitalized history
roles retain their assistant/system meaning. Monty's structured task-host port
remains the required production model/tool path.

The original seven integration failures remain unresolved: the driver still
enters its UUID-only engine Thread lookup before Recipe execution. These changes
neither activate the prospective global source nor satisfy Phase 3a. Child host
binding/dispatch, pinned revisions, full global hosting/resource/control gates,
verified replies and the history reply-ref protocol remain required. The new
legacy history formatter accepts typed answer text; it is not yet the global
protocol's verified reply-ref resolver.

Validation ran sequentially in screen: the native composition group passed
(10 tests), legacy Kohai output checks passed (13 tests), and actual Monty 1.0
Recipe failure-contract checks passed (6 tests). Strict all-target Clippy passed
for engine/composition with composition/skills-db, then passed again for the
completed history seeding changes. The already compiled full composition binary
reported **705 passed, 7 failed, 0 ignored**: precisely the original seven
message-flow failures, with their assertions unchanged. No additional failures
were introduced. Logs: `/private/tmp/brassclaw-monty-typed-components.log`,
`/private/tmp/brassclaw-monty-kohai-output.log`,
`/private/tmp/brassclaw-monty-recipe-contract.log`,
`/private/tmp/brassclaw-monty-{typed-components,history-binding}-lints.log`,
`/private/tmp/brassclaw-monty-history-binding.log`, and
`/private/tmp/brassclaw-monty-composition-final.log`.


### 2026-10-06 — Attempt-scoped host-call fencing for retained Monty tasks

`MontyTaskHost` now owns a Rust-only `MontyTaskFence`. A supervisor can retain
its handle independently of a dropped runner future. Every scoped host operation
registers before awaiting the real port, then checks cancellation/fencing before
returning its result. A closed fence is permanent. Cancellation addresses the
exact run/runner/lease tuple; a stale lease token cannot fence another attempt.
Admission and call bookkeeping use short locks with no lock held across an
external wait. Concurrent retained host calls have an explicit technical bound
of 64, separate from token budgets; excess admission fails visibly rather than
creating an unbounded queue.

`fence_and_wait` uses an explicit, checked deadline and returns pending evidence
when calls have not settled. Dropped call futures are retained as abandoned IDs
and permanently close admission. They require reconciliation, even when there
are no remaining Rust futures. Late successful values are rejected; actual host
errors and their diagnostic references are preserved. Multiple acknowledgement
waiters register before taking their snapshots so completion wakeups cannot be
lost. No claim token or host-call payload is serialized into Python or receipts.

This acknowledgement describes **Rust host-call state only**. It does not prove
external provider/tool quiescence, cancel a process, persist reconciliation, or
validate the current durable lease at dispatch. Kernel effect records remain
authoritative. The global supervisor must retain the fence, combine it with VM
continuation acknowledgement and durable effect/claim validation, and persist
unresolved evidence before releasing the task. The legacy signal broker's future
completion receipt is not upgraded into that production guarantee by this slice.
No global boot activation or per-chat/UUID workaround was added.

Focused validation passed: three real fence-state tests covering bounded waits,
late results/original errors, abandoned calls, wrong lease identity and registered
concurrent waiters; four existing/new Reborn host-boundary checks covering exact
context, structured model output, prompt grants, cancellation and provider/reply
rejection after fencing. Strict all-target Clippy passed for Reborn/composition
with composition/skills-db. All ran sequentially in screen; logs are
`/private/tmp/brassclaw-monty-attempt-fence{,-lints}.log`.
These are not simulated successful Recipe executions or acceptance of the global
production service. The original seven message-flow tests remain unresolved at
the legacy UUID-only Thread lookup and were not rerun for this downstream change.
Their latest full-suite result remains 705 passed, 7 failed, 0 ignored.

### 2026-10-06 — Versioned Monty resumable execution-control boundary

Added an isolated library extension in `vendor/monty-control`, version
`1.0.0-brassclaw.control.1`, based on the exact v1.0.0 commit already pinned by
the upgrade workspace. The four library crates retain upstream MIT notices;
original file SHA-256 values are checked against the local Git checkout and
recorded in `UPSTREAM-SHA256.json`. `CONTROL.patch` makes the extension diff
reviewable independently of unchanged vendored source. Explicit package
workspace paths prevent Cargo from inheriting the application workspace's
metadata. Stable rustfmt configuration omits unsupported nightly grouping
options rather than suppressing their warnings. Publishing is disabled.

`ExecutionControl` is a trusted, nonblocking hook over cumulative active VM
execution time. It is checked at entry, periodic resource checks and window exit,
including completion/error windows. Its errors latch and propagate uncatchably.
A live duration revision is read by the same `SharedMontyTaskBudget` used by Rust;
consumption is retained through resumes/settings changes. Authority stays in an
Arc omitted from serialization; a controlled dump requires explicit trusted
reattachment before any resumed opcode executes. Reattachment cannot erase an
observed terminal failure. The separate dump ABI `0xBC01` rejects upstream and
older continuation formats; it does not authenticate dumps or replace deployment
reconciliation.

`ControlYield` retains the actual VM stack, frames, locals and exception state
before the next opcode. Resume restores that state without injecting a synthetic
external return value. REPL control yields also preserve the Recipe's subsequent
feeds. Synchronous native reentry cannot serialize its Rust stack, so it keeps
yield requests pending until the native call has returned. Native operations that
already poll resource checks can observe cancellation; this does not claim that
all native operations are preemptible. Compilation, graph export, global async
CPU attribution and allocator/heap containment still require their own evidence.

Sequential screen validation passed seven actual-interpreter control regressions,
21 existing compatibility/lifecycle/Recipe-state/failure/accounting regressions
compiled against the extension, strict all-target Clippy for the isolated caller
workspace, and strict library Clippy for the modified interpreter/types crates.
Logs: `/private/tmp/brassclaw-monty-control.log`,
`/private/tmp/brassclaw-monty-control-{lints,engine-lints,compatibility,all-lints}.log`.
These checks establish the isolated VM primitive, not Phase 3a completion.
Production dependencies, boot readiness and the legacy driver are unchanged.
The original seven composition failures were not rerun: their UUID-only Thread
lookup remains unchanged, and their latest full-suite evidence remains
705 passed, 7 failed, 0 ignored. No Rust agent-loop/raw-model fallback, ID coercion
or synthetic Thread was introduced.

### 2026-10-06 — Task-owned child-VM hosting and typed continuation boundary

Added `crates/brassclaw_monty_host` as an isolated Monty 1.0 control-extension
candidate, consumed only by `tests/monty_control`. Per `recipe.md`, Recipes are
ordered instructions plus component inventories; IBS/composition assembles the
components and bindings, and Monty owns sequencing and result publication.
`RecipeVm` holds a child execution context only. It advances exactly one
caller-selected feed/continuation, never selects a Recipe/next step, dispatches
Tools, calls a provider, queries latest components or retries. The production
workspace, legacy driver and application dependencies remain unchanged.

Each feed receives a typed `inputs` mapping and a fresh result sentinel. Needed
locals survive subsequent steps and waits, but an absent assignment cannot
reuse the preceding result. Source checksum and syntax validation precede the
feed; the same immutable source handle and binding aliases survive suspension.
These checks do not establish catalogue selection, complete version manifests,
Skill-association approval, recursive component schemas or Q1/Q2. The trusted
caller must provide those and the global Python caller must publish results
under stable step/occurrence identities. No runtime data is interpolated into
Python source.

The adapter accepts JSON-shaped data with exact i64/u64 integers and finite
numbers. Unsupported Python values fail explicitly. It traverses the pinned
Monty graph without formatting BigInts or cloning whole intermediate containers.
Aggregate depth, node and UTF-8 payload bounds cover the entire host call, not
one independent allowance per argument. Only the selected aliases on the exact
host receiver produce host requests; aliases are not Tool grants. The trusted
supervisor still owns live kernel policy and attempt fencing before dispatch.

Continuation keys identify the exact context and boundary. Foreign/stale replies
cannot consume the live continuation; rejected replies remain private diagnostic
evidence. A completed child value is also retained when resumption fails due to
Python errors, output validation or a newly reduced live budget. That failure
never permits replay of an already completed effect. Catchable domain exceptions
and terminal technical failures remain separate port classifications. Stdout
capacity overflow is an explicit error, with produced output retained. Diagnostic
formatting exposes neither raw exceptions nor arguments/results. Large VM state
and error evidence are boxed instead of suppressing strict size lints.

Parked cancellation releases the VM and returns the pending host-call key; it
is VM-local acknowledgement, not external-operation quiescence. Busy Python
can yield with exact frames and cannot catch a supervisor cancellation. Parent
and child interpreters share the same live task account with separate cumulative
VM-clock cursors; a parked parent is not charged again while the child runs.
Compilation/export, unpolled native work, logical heap/adaptive memory and
allocator containment still need their documented Phase 3a contracts/evidence.

Sequential background screen validation passed ten actual-interpreter child-host
cases and strict library/caller all-target Clippy. The initial all-target run also
passed the unchanged 28 control/compatibility/lifecycle/failure/state/accounting
cases. After storage-layout and completed-result-evidence fixes, the affected
child cases and both strict lint checks passed again. Logs:
`/private/tmp/brassclaw-monty-child-host.log`,
`/private/tmp/brassclaw-monty-child-host-lints.log`, and
`/private/tmp/brassclaw-monty-child-caller-lints.log`.

**The original seven composition failures remain unresolved.** The legacy
`PersistentMontyDriver::drive_turn` still calls UUID-only `load_thread` after
loading the exact admitted input. A correct replacement requires the gated
instance-owned global service, boot readiness, child port dispatch, approved
component snapshots and real production-caller acceptance. No UUID conversion,
synthetic Thread, Rust agent-loop/direct-model fallback or changed original
assertion was introduced. Repeating the composition suite for this isolated
change would still hit that unchanged blocker; latest full-suite evidence remains
705 passed, 7 failed, 0 ignored. This slice supplies a required child boundary;
it does not activate global hosting or complete Phase 3a.

### 2026-10-06 — Actual root-VM boot/wait and future-correlation hosting

The isolated `brassclaw_monty_host::global::GlobalVm` now owns one actual root
interpreter. It receives integrity-checked source/checksum and aliases from its
caller, without an embedded script or per-chat fallback. Starting and VM-level
Ready are distinct: every configured worker must be parked at a unique work wait
before readiness. Early final, task-bound ports before readiness, incomplete or
duplicate waits and unsupported calls fail closed. Busy boot yields cooperatively.
This is not production facade readiness or proof of an exclusive instance lock.

Root requests use the same bounded typed adapter as child calls. Generation-bound
keys correlate exact Monty future IDs; wrong/foreign/stale replies retain evidence
without consuming other continuations. Fatal abandonment retains pending keys for
reconciliation. Admission accepts the exact seven-field current root envelope,
keeps opaque IDs/empty messages as data, rejects extra Rust-only claim fields and
leaves the live work wait intact after malformed/oversized input. The production
adapter still owes exact durable identity/history validation; this envelope is
not the full future transport protocol or an authorization record.

A generic callback cannot terminate a work wait. Explicit shutdown changes Ready
to Stopping and only that path supplies None to workers. Task errors/cancellation
use catchable root exceptions; a root Abort is instance-fatal supervision. No Rust
Recipe selection/sequencing, model fallback, automatic replay or fabricated finish
was added. The root execution-slice control does not incorrectly apply a single
task-duration account to the global VM lifetime/shared coroutine clock.

Five real root-host tests and the ten child-host regressions pass, including A
waiting while B reaches its host call and an actual child exception producing a
failed-task finish request. The durable finish remains unresolved in that test;
no successful provider/Recipe/DB operation is simulated. Strict host-library and
isolated caller all-target Clippy pass. All ran in the sequential screen queue;
logs remain `/private/tmp/brassclaw-monty-child-host{,-lints}.log` and
`/private/tmp/brassclaw-monty-child-caller-lints.log`.

The application/production driver is unchanged. **All original seven composition
failures remain unresolved at UUID-only Thread loading; they were not rerun for
this isolated change.** Production adoption still requires the Phase 3a upgrade
resource/allocator/heap/root-CPU gates, exclusive boot ownership, durable ports and
wait/resume supervision, selected component manifests and production-caller
acceptance. Do not activate this candidate or mark the seven resolved solely from
its VM-level handshake and interpreter tests.

### 2026-10-06 — Task-owned typed boundary accounting

The isolated child host now debits typed input, host-return, host-argument and
result conversion to the same `SharedMontyTaskBudget` as its interpreter. Each
interval wraps only synchronous data conversion, never interpreter execution,
nested work or external waits. Rejected conversions retain their compute charge.
The live account and cancellation are checked before and after conversion;
resource failure terminates the child context before publishing a dispatch or
result. A completed host return and prior stdout remain private error evidence,
including when the live budget rejects conversion on parent resumption.

Validation against `aa4dd3df` plus this host/caller diff: eleven real child-host
cases and five root-host cases pass. Strict host-library and isolated-caller
all-target Clippy also pass, sequentially in the background screen queue. Logs:
`/private/tmp/brassclaw-monty-child-host.log`,
`/private/tmp/brassclaw-monty-child-host-lints.log` and
`/private/tmp/brassclaw-monty-child-caller-lints.log`. Existing production-suite
evidence is unchanged; the original seven failures were not rerun or resolved.

This closes only the host-side typed-adapter accounting gap. Monty's internal
compilation, snapshot and graph import/export still need separate accounting;
unpolled native work, root coroutine attribution and logical shared heap/allocator
containment remain prerequisite implementation and acceptance work. Measuring a
whole interpreter hosting call by elapsed wall time would wrongly charge waits
or nested work, so it cannot substitute for those ownership boundaries. No
production dependency, global service wiring or component activation changed.

### 2026-10-06 — REPL scanning/compilation task accounting

The isolated extension is now `1.0.0-brassclaw.control.2`, dump ABI `0xBC02`.
REPL source scanning and synchronous compilation have preparation windows that
retain actual active time on success, rejected syntax and unwinding. Trusted
control checks before and after those windows include preparation plus VM time
in the same cumulative task cursor. Static Monty feed/turn limits and `elapsed()`
remain VM-only; `preparation_elapsed()` reports the separate retained component.
Snapshot adapters must preserve both clocks and the account cursor without
charging earlier consumption again. Old extension/upstream dumps are rejected;
no migration or resumption compatibility is claimed.

A live cancellation/budget failure after successful compilation restores compiler
tables to the REPL before returning failure and executes no opcode. Preparation
does not wrap VM execution or external waits. Three actual-interpreter regressions
cover charged syntax failure and retained tables on compiler-exit cancellation,
dump persistence/old ABI rejection, and a shared 600→30-second revision published
at compiler exit before Python effects. Raising the budget cannot revive the
failed task account. This is accounting and boundary enforcement, not compiler
preemption or a measured bound on native/compiler response time.

Validation: all 47 isolated interpreter checks pass against `aa4dd3df` plus the
host/preparation diff. Strict extension (`monty`, `monty-types` libraries), host
library and isolated caller all-target Clippy pass. Cargo ran sequentially in
background screen. Evidence: `/private/tmp/brassclaw-monty-preparation.log`,
`/private/tmp/brassclaw-monty-preparation-lints.log`,
`/private/tmp/brassclaw-monty-child-host-lints.log` and
`/private/tmp/brassclaw-monty-child-caller-lints.log`. The extension patch is
regenerated from the hash-verified upstream baseline; production pins remain
unchanged. The original seven failures remain unresolved and were not rerun.

Remaining resource gates include one-shot artifact/root construction, graph
import/export and snapshot accounting, compiler/native response bounds, root
coroutine ownership and shared logical heap/allocator containment. Production
boot, durable ports/continuations, selected catalogue manifests and WebUI effective
revision acceptance still require implementation; this isolated evidence does
not authorize activating the candidate or declaring Phase 3a complete.

### 2026-10-06 — Interpreter graph import/export task accounting

The isolated extension is `1.0.0-brassclaw.control.3`, dump ABI `0xBC03`.
Preparation guards now own a bounded accounting record rather than borrowing
the resource tracker while the interpreter mutates its heap. Graph input and
host-return import, host-call argument export, and returned-value export debit
the same cumulative control clock as REPL compilation. Nested conversion shares
an existing window; native VM reentry transfers the active clock, and native
sleep pauses both clocks. No account lock spans interpreter work or a trusted
control callback. Poison/overflow telemetry returns an error instead of zero.

Before and after conversion, resource failure prevents an exported dispatch or
result from being published. Imported VM references are released on a closing
failure. Owned guards record unwinding without invoking callbacks in destructors.
Foreign completion cannot charge another tracker. An active preparation without
its live guard is not an executable idle continuation after dump restoration;
both earlier extension ABIs are rejected. This remains trusted-producer snapshot
handling, not snapshot authentication or a durable production resume contract.

Four added actual-interpreter/accounting cases cover input and actual child-return
import cancellation before Python effects/handlers, native sleep exclusion plus
unwinding/foreign ownership, active-preparation dump rejection, and a real child
list imported/exported by its suspended parent without another compile. The
child result remains data; no provider/Tool/durable-completion success is faked.

Validation against `36507b3d` plus this graph/guard diff: 50 affected interpreter
tests passed, followed by 14 control tests including the final added case, for
51 distinct passing checks. Strict extension library, host library and caller
all-target lints pass. Cargo ran sequentially in background screen. Evidence:
`/private/tmp/brassclaw-monty-control3-suite.log`,
`/private/tmp/brassclaw-monty-control3-extension-lints.log`,
`/private/tmp/brassclaw-monty-control3-host-lints.log`,
`/private/tmp/brassclaw-monty-graph-control.log` and
`/private/tmp/brassclaw-monty-graph-caller-lints.log`. Existing source checks were
reused after the final test-only addition; no broad production suite was rerun.

Snapshot/cleanup accounting, one-shot artifact/root construction, native/compiler/
graph response bounds, root task attribution and logical shared heap/allocator
containment remain open. Production boot/ports/continuations, exact component
manifests and WebUI effective-revision uptake still require implementation and
caller acceptance. **The original seven composition failures remain unresolved
at legacy UUID-only Thread loading.** This change neither activates global
production hosting nor bypasses the Recipe/IBS/Monty path to make them green.


### 2026-10-06 Ordered IBS assembly and admitted reply handoff

`build_ordered_instruction` shares the existing compiler but retains the exact
cross-channel `step_link` order. Its private selection rejects overlapping
execution ranges, duplicate description indices, zero step numbers and empty or
multi-component steps. `compose_typed_program` pairs a separate Rust ToolSkill
binding with its immediately following PythonCode, without source substitution
or a concatenated-source execution alternative. Missing/wrong-class components,
prose execution, dangling bindings and conflicting canonical host aliases fail
before any program is returned. This is structural assembly infrastructure;
recursive schemas, internal include expansion, exact association approvals and
immutable transitive version manifests still require their own enforcement.
The legacy production caller is deliberately not switched to partial enforcement.

The focused engine caller checks pass: 5 composition tests (including selected
cross-description order, binding pairing, untouched source, rejection cases and
`echo`/`host.echo` collisions), 48 instruction tests, and affected strict lints
with `skills-db`. Evidence: `/private/tmp/brassclaw-typed-assembly-alias.log`,
`/private/tmp/brassclaw-ordered-instruction.log` and
`/private/tmp/brassclaw-typed-assembly-final-lints.log`, against HEAD `36507b3d`
plus this working diff. No additional provider/database success was simulated.

`MontyTaskHost` retains the actual finalized reply ref/content after its transcript
port returns success. Content lookup requires that exact ref and passes the
attempt fence; a foreign ref or fenced task cannot use it. The trusted supervisor
can still recover the actual ref after fencing for reconciliation. This cache
retains one current reply; durable transcript records remain authoritative after
restart. Four existing actual task-host caller tests and affected strict lints
pass (`/private/tmp/brassclaw-monty-reply-handoff.log` and
`/private/tmp/brassclaw-monty-reply-handoff-lints.log`).

The unactivated global class-10 source calls `host.resolve_reply(task_token,
reply_ref)` before the history Recipe and hands over the real answer as data.
The global service must bind this primitive to the exact admitted task host's
lookup; neither a caller-supplied string nor a new UUID Thread establishes reply
ownership. Complete the non-match Recipe's separate reply step and typed local
binding contracts before exposing it through this path. Do not replay model or
Tool effects because publication/history fails.

`GlobalVm::start_ready` now performs the actual configured-worker work-wait
handshake with an explicit startup deadline. It never returns a late Ready
result. The deadline includes synchronous construction but does not preempt
compiler/native work; bounded process supervision remains required. Its worker
inventory exposes only real work waits, not generic port-call keys.

The PostgreSQL exclusive-owner candidate detaches its connection from the pool
before requesting the two-int database-instance advisory lock. Cancellation or
abandonment closes that session instead of returning a reentrantly locked
connection to the pool. Repeated ownership checks never reacquire the lock.
Actual native PostgreSQL acceptance proves second-owner refusal, single-unlock
release, abandoned-session recovery and real lock-loss detection
(`/private/tmp/brassclaw-monty-instance-owner.log`). This candidate is test-gated
until the global supervisor can retain ownership through VM/host-call quiescence,
bound connection checks and fence dispatch on ownership loss. Do not acquire and
release this guard merely around seeding or claim it is facade readiness.

The original seven failures still reach the legacy UUID-only Thread loader.
These gates are prerequisites, not passing evidence for those seven or permission
to install a Rust agent-loop/model fallback. Production Monty upgrade/resource
containment, full selected manifests, durable port/admission hosting and live
settings uptake remain on the required cutover path.


Startup caller acceptance passes all six actual root-VM boundary cases, including
the new real handshake and busy-boot deadline. Strict host/caller lints pass
(`/private/tmp/brassclaw-monty-startup-handshake.log`,
`/private/tmp/brassclaw-monty-startup-host-lints.log`,
`/private/tmp/brassclaw-monty-startup-caller-lints.log`). PostgreSQL ownership
strict composition lints pass as well
(`/private/tmp/brassclaw-monty-instance-owner-lints.log`). The owner remains
isolated: connection-loss supervision and retained ownership across actual global
shutdown must be implemented before enabling it in production startup.


### 2026-10-06 Physical worker allocator overflow repair

The unreleased Monty `control.3` extension now rejects an unrepresentable finite
baseline-plus-budget ceiling instead of saturating it to an uncapped worker.
Invalid replacement preserves the armed limit. Live allocation/refund accounting
uses checked atomic arithmetic and terminates on overflow/underflow instead of
wrapping the counter. This does not change the serialized dump ABI.

An actual isolated worker installs `monty-alloc::LimitedAllocator`, arms a finite
ceiling, rejects the overflowing replacement, then executes a real Monty native
64-MiB string allocation. The allocator ends the process with the documented
OOM exit code and diagnostic. The parent test verifies the actual exit/output;
no fake memory probe or simulated successful primitive was added. Regression and
strict allocator/caller lints pass (`/private/tmp/brassclaw-allocator-worker.log`,
`/private/tmp/brassclaw-allocator-extension-lints.log`,
`/private/tmp/brassclaw-allocator-caller-lints.log`). All 228 upstream file hashes
were verified and the regenerated `CONTROL.patch` applies to the pinned upstream
checkout. Existing registry dependency pins are retained; only the generated
local allocator dependency is added to the isolated caller lockfile.

This closes the finite-ceiling overflow defect and supplies real physical
backstop evidence. It does not close shared logical heap attribution, adaptive
measurement/reclamation, native/compilation response bounds or production worker
supervision/reconciliation. The seven composition failures remain unaccepted
until the coordinated global production cutover exercises their actual callers.


Final relevant-diff verification: the legacy compiler avoids allocating a cross-
channel order it does not consume; its 48 existing instruction regressions and
strict affected lints pass (`/private/tmp/brassclaw-ordered-compiler-final.log`,
`/private/tmp/brassclaw-ordered-compiler-final-lints.log`). The preserved isolated
allocator lockfile is accepted by `cargo --locked`; its actual worker test and
strict caller lints pass (`/private/tmp/brassclaw-allocator-pinned-caller.log`,
`/private/tmp/brassclaw-allocator-pinned-lints.log`). No competing Cargo executions
were started. Concurrent prefix-authoring files are left untouched.


### 2026-10-06 Isolated root worker transport and reply contract repair

The Monty 1.0 hosting candidate now has a real `monty_worker` executable and
`process::GlobalProcess` supervisor transport. This follows Phase 3a's permitted
isolated hosting option; a process boundary is an implementation choice, not
an additional Recipe execution engine. One worker owns one long-lived root VM.
Rust commands advance only explicit Monty boundaries; Python retains sequencing.
No process per task and no Rust-loop/model fallback is introduced.

The worker installs the actual finite allocator before source ingress/compilation.
The parent uses an absolute executable path, a cleared environment and private
length-prefixed JSON pipes. Bounded serialization and pre-allocation header checks
fail rather than truncate. Protocol/sequence and generation-specific continuation
checks retain exact correlation; a second Boot cannot replace a live root.
Independent response deadlines contain busy compilation/native execution through
worker termination. Dropped instance transport futures fence the process and
retain their interrupted command. Unknown/oversized input preserves the admission
wait. OS kill errors, missing exit acknowledgement and a real reaped exit remain
distinct. Graceful shutdown waits for the actual Stopped boundary and normal exit.

The transport future belongs to the instance actor. Cancelling a user turn must
not drop that future or kill the global VM: task/child fencing and unrelated-task
progress require the still-unfinished production actor/registry. Fatal worker
containment cannot acknowledge external effects or justify replay. Keep the DB
owner until that actor reconciles pending calls and the worker is quiescent.
Child hosting inside this contained worker, root CPU attribution, live desired/
effective revision coordination, logical adaptive heap measurement/enforcement,
full pinned immutable manifests, durable inbox/effects and production startup/
shutdown remain binding gates. The candidate remains outside the application
runtime graph; the original seven failures still lack production cutover evidence.

Actual sequential screen acceptance: six process tests and six root tests pass
with preserved registry pins and `--locked`; affected host/caller strict lints
pass. The final cleared-environment process run and lints pass as well
(`/private/tmp/brassclaw-process-host-final.log`,
`/private/tmp/brassclaw-process-host-contained.log`,
`/private/tmp/brassclaw-process-host-contained-lints.log`,
`/private/tmp/brassclaw-process-host-contained-caller-lints.log`). Tests verify
real subprocess exits, actual native OOM, independent busy-startup interruption,
opaque task delivery, explicit shutdown, dropped-transport containment and actual
malformed-frame rejection. They fabricate no successful Recipe/provider/effect.
Only Tokio process support's `errno`/`signal-hook-registry` dependencies were added;
unrelated platform version changes from offline resolution were restored.

A separate downstream defect was repaired in the existing reply primitive:
seeded PythonCode calls `host.post_reply(answer=...)`, but the legacy handler
read only `text` and silently discarded it. It now accepts exactly one nonempty
string using canonical `answer`, retained legacy `text` or the existing positional
form. Missing/unknown/duplicate/conflicting/non-string/empty arguments fail before
transcript/event mutation; values are never coerced. No seeded component is
silently replaced. The actual Monty host-call regression and affected strict engine
lints pass (`/private/tmp/brassclaw-reply-argument-contract.log`,
`/private/tmp/brassclaw-reply-argument-lints.log`). This legacy in-memory append
still does not provide the admitted task host's durable reply reference or remove
the UUID-only production driver boundary blocking the seven original tests.


Final transport review also bounds parent-side JSON traversal before encoding.
An iterative borrowed preflight checks depth/nodes/bytes and pending stack entries;
excessively nested completed host returns remain retained command evidence, and
neither the VM continuation nor IPC sequence is consumed. The actual caller tests
now cover this rejection and an independent encoder frame overflow before valid
opaque admission. The final six worker regressions and strict host/caller lints
pass (`/private/tmp/brassclaw-process-host-bounded-data.log`,
`/private/tmp/brassclaw-process-host-bounded-data-lints.log`,
`/private/tmp/brassclaw-process-host-bounded-data-caller-lints.log`). These explicit
transport limits never justify silently truncating history or reintroducing token
budgets: production must handle larger eligible data with bounded transfers or
run-scoped references. No application dependency/driver cutover is claimed.


### 2026-10-06 Contained Recipe contexts and explicit child handoff

The isolated Monty 1.0 worker now owns bounded task/context registries as well
as the single instance root. Private protocol 2 deliberately replaces protocol
1: boot requires validated task settings and a context bound; admission accepts
exactly six data fields and returns a fresh worker-issued task handle. Conversation,
message, turn and run IDs remain opaque and unchanged. The owning Rust actor must
associate that handle with the exact admitted attempt host; it is never a claim,
credential, permission or substitute for durable fencing.

Contexts execute only caller-selected, integrity-checked Python artifacts. They
retain locals between feeds and bind inputs separately from source. Children
require an explicit same-task, live parent; actual computed return values are
resumed into the parent through its exact continuation. Cancelling a parent
releases its descendants, retaining every outstanding child host identity.
Descendant traversal visits the graph once rather than repeatedly scanning a
chain. Released root contexts cannot be recreated for the old handle; stale
answers fail with retained command evidence. A running child yields at actual
bytecode checkpoints, and its cancellation leaves the instance root and other
tasks alive. Classified domain exceptions are catchable; terminal child errors
are uncatchable. Root port replies reject child-abort requests.

Every context for one task shares the same live compute account. Context/artifact
construction and existing conversion/execution clocks debit non-overlapping
segments. Settings edits compare exact revisions and retain consumption; stale
settings fail distinctly. Snapshots report effective settings and task accounting
errors explicitly. Graceful root-worker closure refuses remaining task records.
Production must fence and reconcile attempts/effects before releasing them;
local release cannot establish durable completion or external quiescence.

Actual screen checks run sequentially with no simultaneous Cargo executions:
11 child-host, six root-host and eight subprocess cases pass, plus strict host
and isolated caller lints. Evidence is in
`/private/tmp/brassclaw-process-recipe-contexts.log`,
`/private/tmp/brassclaw-process-recipe-context-lints.log` and
`/private/tmp/brassclaw-process-recipe-caller-lints.log`.
The subprocess cases use actual Monty computation, host suspensions, native
allocation failure and process lifecycle; no successful provider/Recipe/finish
port result is fabricated.

This supplies contained child hosting, not the production cutover. Root CPU
attribution, shared adaptive logical heap enforcement, desired/effective WebUI
coordination, immutable complete manifests/recursive binding contracts, durable
effect accounting, instance ownership/startup/shutdown and production host-port
integration remain required. The legacy UUID-only driver remains active, so the
original seven composition failures have not been accepted as resolved. No
Rust-loop/model fallback or per-task process is introduced.

The final explicit settings-conflict assertion and changed caller lint also pass
(`/private/tmp/brassclaw-process-recipe-settings-final.log` and
`/private/tmp/brassclaw-process-recipe-caller-final-lints.log`). No application
Cargo build was run for this isolated hosting change.


### 2026-10-06 Instance-owned transport actor

`brassclaw_monty_host::transport_actor` now owns worker IPC independently of a
cancellable turn future. Accepted commands have bounded, privately retained
receipts; abandoning a waiter leaves the command/result recoverable through the
supervisor inbox. Collection releases count/frame credits. Overflow and closed
queues return the complete unaccepted command. Command validation uses the same
bounded data traversal as direct IPC; retained encoding reserves the largest
sequence representation before acceptance. This evidence encoding is never sent
or replayed as a request.

A separate bounded control/completion lane prevents ordinary abandoned receipts
from occupying all cancellation/settings/continuation capacity. Priority is
bounded to eight consecutive controls before available ordinary work is preferred;
instance termination remains independently addressable. The actor performs only
explicit transport operations. Recipe order, child result handoff and Tools are
still selected by Monty, not this Rust loop.

Normal shutdown joins after the actual Stopped/exit handshake. Fatal transport
failure retains the completed failure and explicit not-started queued receipts.
An interrupted join retains its handle. OS kill/reap failures remain accessible
in ProcessError/ActorExit, including failed startup where the process object would
otherwise be dropped. The instance owner must retain PostgreSQL ownership until
actual local quiescence and durable effect reconciliation; neither a local receipt
nor actor exit proves external settlement. The actor inbox is RAM infrastructure,
not durable admission or a replay permission. Frame credits bound retained transport
payloads; they do not claim exact physical memory accounting for decoded values.

Four actual actor cases and eight affected transport regressions pass, along with
strict host/caller lints, in sequential screen execution with `--locked`:
`/private/tmp/brassclaw-instance-actor-host.log`,
`/private/tmp/brassclaw-instance-actor-host-lints.log`, and
`/private/tmp/brassclaw-instance-actor-caller-lints.log`.
The first native-failure test incorrectly used unsupported bytearray and stalled
waiting for an instance failure; the owned run was interrupted, process cleanup
was verified, and the case now uses actual supported string allocation with
bounded waits. A separate real Python-error case verifies the deliberate task
failure/instance failure distinction. No native failure, model, Tool or durable
finish success is fabricated. Unchanged root/child evidence remains applicable.

The next root-accounting change must use actual scheduler identity and switches
(`bytecode/vm/async_exec.rs::activate_task`, task cleanup and host/control boundaries).
The current ExecutionControl::checkpoint receives only elapsed time, and its
control-error latch terminates that interpreter. It cannot associate root CPU
with a BrassClaw task or safely impose a task-local failure on the global VM.
Do not attribute the shared cumulative clock to the last Rust-handled task.
A persistent worker coroutine is reused for successive admissions: task accounting
must bind the current admitted handle, retain its usage through switches/waits,
and clear that association on completion. Service/idle time stays separate.
Any interpreter extension must preserve task-local failure handling in the trusted
root, keep unrelated workers alive, version its ABI/patch/integrity record, and
prove A→B→A plus short/error/native/preparation boundaries before production use.

Production dependencies/driver remain unchanged. Monty 1.0 integration, root CPU
attribution, shared adaptive logical heap, immutable manifests and typed component
contracts, durable effect accounting, retained instance ownership and production
port/startup/shutdown coordination remain gates. The original seven composition
failures have not been rerun or accepted as resolved by this isolated actor proof.

### 2026-10-06 Actual scheduler accounting prerequisite

The isolated interpreter is now `1.0.0-brassclaw.control.4`, dump ABI `0xBC04`.
`ExecutionObservation` exposes separate cumulative execution/preparation clocks
and the actual loaded coroutine. Scheduler activation samples the outgoing
context before switching; actual task exit clears it. Mechanical VM cleanup
during snapshot creation preserves the suspended identity. New REPL feeds select
main context 0. A transition-latched fatal control error is checked before loading
or executing the next coroutine, including a short feed that would otherwise
reach a host call before its first periodic checkpoint. Existing child controls
retain their combined-clock API, terminal latch and shared task account.

`GlobalVm::execution_accounting` now partitions executing time into a bounded
main/worker/discarded-context map and reports preparation separately.
`pending_context` identifies the coroutine that issued an actual pending call.
Preparation is not charged to that coroutine: importing A's host result while B
was last loaded must not charge B. Persistent coroutine identity is not admitted
task identity or authority. The next integration must bind each admitted handle
to its actual worker context, debit the same root/child task account, retain usage
across waits/settings revisions, and clear the association on completion. The
interpreter-terminal control error must not be used to kill the root for a single
task; scoped interruption and trusted failure/finish handling remain required.
That interruption must retain or explicitly settle the interrupted task's actual
pending futures, with matching continuation/context and attempt fencing. Raising
into a coroutine after creating an external future but before its `await` can
leave an orphan in the scheduler's pending index; removing only the host map would
break the exact pending-set invariant. Prove this boundary and permit trusted
failure/finish service work after a task budget latches, without allowing further
task Tools or affecting another worker's pending calls.

Coroutine and host-call counters now reject exhaustion before mutating counters,
taking references or dispatching. Release builds cannot wrap and reuse an old
correlation identity. Refused external/method/OS calls and settled awaitables
release arguments, values and pending effects. This is instance-fatal identity
exhaustion, not permission to retry/replay a task. Old dump ABIs are explicitly
rejected; reconcile their continuations/effects before an upgrade.

All 228 recorded upstream hashes and the exact zero-context patch round trip were
verified against the retained upstream checkout. The final changed interpreter,
host and callers pass 47 real regressions plus two counter-exhaustion unit cases,
and strict extension/host/caller Clippy checks in sequential background screen
execution. Evidence:
`/private/tmp/brassclaw-monty-context-final.log`,
`/private/tmp/brassclaw-monty-context-identities.log`,
`/private/tmp/brassclaw-monty-context-extension-lints.log`,
`/private/tmp/brassclaw-monty-context-host-lints.log`, and
`/private/tmp/brassclaw-monty-context-caller-lints.log`.

The new cases cover A→B→A real coroutine execution, unchanged clocks during a real
host wait, native callback reentry with an error and deferred safe-boundary yield,
exact identity/clocks after dump/restore, short completion/error exits, and fatal
transition failure before dispatch. Initial build errors (one missed asyncio
caller and old value constructors in the new test) and the native-test setup
failure are preserved in the initial logs. The setup now supplies a top-level
periodic checkpoint after native reentry; a pending yield need not supersede an
earlier host boundary. No warning is suppressed or product success fabricated.

This is a root accounting prerequisite, not completed task budget enforcement or
production/global hosting. Production dependencies/driver are unchanged; original
seven composition failures remain unaccepted. Root admission/account association,
scoped failure and preparation accounting, adaptive logical heap, complete
immutable component manifests/bindings, durable effects/ownership and production
ports/startup/shutdown/WebUI coordination still require their documented cutover
acceptance. No Rust-loop fallback is added.

The final context-map bound uses checked arithmetic; the affected seven root
cases and host lint also pass in
`/private/tmp/brassclaw-monty-context-bound-final.log` and
`/private/tmp/brassclaw-monty-context-bound-lints.log`. These checks used
Rust 1.98.1 and the unchanged NVMe target/profile settings; they do not replace
the minimum-version/production cutover gates.

### 2026-10-06 Failed-root transport fencing

Review of instance-fatal counter exhaustion exposed an adjacent actor error:
all framed VM failures were treated as task-local, even when the actual root
lifecycle was Failed. `GlobalProcess` now immediately fences/kills transport
for that state while preserving the failure diagnostic and snapshot. The actor
recognizes Failed snapshots in both result branches, reports `InstanceFailed`,
joins actual termination/reaping and retains every queued command as not started.
Child Python failures whose root remains Ready continue to leave the instance
available. A frozen pending set is reconciliation evidence, not task completion
or permission to replay; instance ownership must still outlive quiescence and
durable effect reconciliation.

The new real worker case raises from a worker after accepted admission, proves
the other actual work wait remains in the failure snapshot, joins local exit and
collects the undispatched queued command with its exact original contents.
Five actor and eight process cases pass along with strict host/caller checks:
`/private/tmp/brassclaw-monty-root-fatal-actor.log`,
`/private/tmp/brassclaw-monty-root-fatal-host-lints.log`, and
`/private/tmp/brassclaw-monty-root-fatal-caller-lints.log`.
Together with unchanged passing interpreter/root evidence, 50 unique focused
cases pass. This remains isolated infrastructure; the seven production composition
failures are not claimed resolved, and no production dependency is upgraded here.


### 2026-10-07 Mandatory shared root/child task account

The isolated root admission API now requires an existing shared task account.
There is no unmetered admission overload. The contained worker creates that
account before admitting Python and retains the same account in its child
registry. Actual coroutine execution deltas debit it; boot, another task's CPU,
idle/external waits and the separate root preparation clock are not assigned to
this task. Child execution adds to the same consumption while root execution is
parked. Root clock cursors and lifetime telemetry never reset when workers are
reused or live settings change.

Input/continuation/conversion rejection occurs before root binding and rolls
back the registry's task/run reservation. Duplicate live routing tokens and
already-terminal supplied accounts reject without consuming the work wait.
Fatal admitted execution preserves the issued task handle and account in the
failure snapshot, including failures before the first port. The actor's existing
instance-failure fencing/reaping retains that snapshot and undispatched receipts.

Returning to the trusted work wait releases the coroutine/task association only
after its actual pending host calls have been settled. Closing a task account
while the root still owns it now rejects without deleting its child state or
consumption; child cancellation alone is not root cancellation, durable finish
or external-effect acknowledgement. Root compute/accounting failures remain
explicit in the shared account and `task_compute_failure`; they never become a
whole-instance control-latch failure.

The final changed path passes 22 affected regressions (nine root, eight actual
subprocess and five actor cases) and strict host/caller Clippy checks, queued
sequentially in screen with the required NVMe target/profile settings. Evidence:
`/private/tmp/brassclaw-monty-required-root-account.log`,
`/private/tmp/brassclaw-monty-required-root-host-lints.log`, and
`/private/tmp/brassclaw-monty-required-root-caller-lints.log`. No build errors or
warnings were suppressed. The terminal-account fixture explicitly injects an
account counter to check failure handling; it does not claim 601 seconds of
measured VM work or successful external execution. Actual root/child execution,
A/B isolation, exact root telemetry deltas, real waits, admission rejection,
worker reuse and fatal evidence are exercised through real interpreters.

This closes the account-association prerequisite, not Phase 3a enforcement or
production acceptance. Root preparation still needs explicit owner attribution.
A terminal account is reported, but expired root bytecode is not yet safely
interrupted inside the trusted task handler: the existing uncatchable global
control latch would destroy the instance, and injecting while an unresolved
future exists can orphan its correlation. Task-scoped interruption, bounded
failure/finishing, pending-future/effect reconciliation and dispatch fencing must
be completed before production wiring. The original seven composition failures
remain unaccepted; application dependencies and the legacy driver are unchanged.
No Rust agent-loop fallback or fabricated provider/Recipe completion is added.


### 2026-10-07 Protected root interruption and typed task ports

The isolated extension is now `1.0.0-brassclaw.control.5`, dump ABI `0xBC05`.
`ControlYield::raise` injects an ordinary exception at the retained opcode,
without a fabricated external return or clearing a terminal control latch.
The candidate class-10 root establishes its protected handler through synchronous
private `host.enter_task`; eager-await eligibility is a scheduling hint, not a
validity requirement. Task compute failure and cancellation reach that handler
while unrelated workers remain alive. The host refuses a second root port in
one scope while its first future is unsettled, before dispatching any effect.

`RecipeCommand::CancelTask` accepts only an issued handle in the current worker
registry. It requests root interruption, fences existing child contexts and
rejects further context creation/source feeds. Contexts, accounts and outstanding
keys remain retained. `CancellationRequested` is not acknowledgement of task
completion, external-effect settlement or Rust attempt-fence quiescence.
An outstanding root result settles its actual future before interruption. A
withheld real answer is retained with its exact continuation in private protocol
3 snapshots for trusted reconciliation. Taking this evidence authorizes no replay.

All 228 upstream hashes and the exact extension patch round trip were verified.
The full isolated interpreter/host queue passed, followed by 24 refreshed root,
process and actor regressions and strict host/caller Clippy checks:
`/private/tmp/brassclaw-monty-task-interruption.log`,
`/private/tmp/brassclaw-monty-scoped-cancel.log`,
`/private/tmp/brassclaw-monty-scoped-cancel-host-lints.log`,
`/private/tmp/brassclaw-monty-scoped-cancel-caller-lints.log`.
Pure-bytecode interruption uses a clearly identified terminal-account fixture;
cancellation uses an actual private command and actual unresolved host error,
without supplying a provider, Recipe, Tool or durable finish success.

`MontyTaskHost::dispatch_port` now maps typed Monty data to one existing prompt,
model, capability or transcript port. It preserves structured model output and
host-issued result/message refs; it does not iterate a model or sequence a
workflow. Unknown fields, including nested fields, and raw prompt bypasses are
rejected before dispatch. The unchanged task host still checks cancellation and
attempt fencing. Its late successful results are now retained privately instead
of discarded after fencing; recording completes before local call-settlement
waiters wake. The permanent fence and its bounded in-flight capacity bound these
receipts. Taking a receipt does not settle a durable effect. Four fence/data
regressions, four existing task-host caller cases through the typed boundary and
strict affected Reborn linting pass:
`/private/tmp/brassclaw-monty-retained-task-results.log`,
`/private/tmp/brassclaw-monty-retained-task-ports.log`,
`/private/tmp/brassclaw-monty-retained-task-results-lints.log`.
Those existing caller fixtures do not establish native production composition
acceptance; no replacement/mock production path was introduced.

Production remains unwired: the old driver still performs its UUID engine Thread
lookup. The seven original composition failures therefore remain outstanding.
Next acceptance work must connect verified/pinned IBS programs and these scoped
ports to the single startup-owned transport, complete the two production Monty
1.0 API/dependency/resource migrations, and retain the owner through VM/effect
quiescence. Root preparation ownership, bounded finishing/reconciliation and
live desired/effective settings acknowledgement remain open. Do not activate the
candidate or claim the seven failures resolved from these isolated checks.

### 2026-10-07 Correlated root preparation accounting

The isolated Monty 1.0 host now charges interpreter preparation to the shared
admitted task account as well as actual coroutine execution. A serialized
single-future resume explicitly owns import preparation; the scheduler still
owns bytecode attribution. A parked `ResolveFutures` has no runnable coroutine,
and only the addressed future is resolved. This prevents importing A's actual
host answer from charging B merely because B was the last worker to park.
Preparation scope cleanup is guarded across failures and unwinding; account
poison, overflow or ownership mismatch stays a failure. Boot/service telemetry
and execution/preparation totals remain separate, and external waits stay free.

Evidence: `/private/tmp/brassclaw-monty-owned-root-preparation.log` passed the
24 affected root/process/actor tests. The real A → B → A regression checks both
preparation ownership and the combined shared account, including child work,
worker reuse and unchanged consumption during an external wait. Affected host
and caller Clippy logs with the same prefix passed with `-D warnings` after
resolving the reported collapsible conditional. These isolated checks do not
resolve the original seven production composition failures or activate the
global service; production dependency, catalogue/Recipe and lifecycle cutover
remain required.

### 2026-10-07 Typed adapters, contained utilities and prepared flow

The root's synchronous typed-data adapters now debit the same admitted compute
account as root preparation/bytecode and child execution, with separate bounded
per-coroutine telemetry. They preserve the actual original host answer when a
conversion exhausts the task budget. The A → B → A regression includes all three
clocks; another actual error-return case checks withheld-answer preservation.
`/private/tmp/brassclaw-monty-root-adapter-accounting.log` passed 25 affected
actor/root/process tests; strict affected host/caller linting passed.

The isolated allocator worker now supports one-operation parse/evaluate utilities.
It receives separately typed inputs through private pipes, rejects host/file/sleep
authority, exports cycle-checked finite typed results and retains actual diagnostics
and stdout on failure. Parent deadlines cover transport, response and reaped exit;
success requires the worker's actual successful exit, not merely a reply frame.
Input aggregates are checked before serialization/spawn. Four real-process tests
include a busy timeout, actual allocator exhaustion contained to the disposable
worker, invalid outputs and subsequent successful reuse of the utility service.
Utility/process/actor checks passed in
`/private/tmp/brassclaw-monty-contained-utilities.log`; final utility and strict
caller checks passed in `...-contained-utility-final.log` and
`...-contained-utility-final-lints.log`. Strict host linting passed in the first
batch. Application utility callers still use the legacy Monty ABI; this candidate
does not establish their cutover or dynamic compilation accounting.

The actual composition Q1 adapter now supplies candidate name/description/content
as typed `inputs` values. Only fixed legacy placeholder spellings are adapted in
the trusted validator source; candidate values never become Python source. Its
successful result requires exactly a boolean `pass` and a string-list `errors`,
with consistent success/failure semantics. Twelve Q1 checks and strict affected
composition linting passed in `/private/tmp/brassclaw-monty-q1-typed-data.log` and
`...-lints.log`. This is the actual adapter and pure result contract, not completed
native approval-store or Q2 acceptance.

The candidate global Python source now validates and executes generic prepared
`recipe-flow/1` control trees. Step, branch, foreach, bounded repeat with explicit
carry and return nodes use typed references and distinct occurrence addresses.
Preflight rejects forward/unavailable results, unreachable/duplicate steps,
invalid scope and excessive expanded dispatch counts. Iteration-local results
cannot implicitly leak into a later iteration. Runtime errors never enter Tier 2.
Actual Reborn model output is externally tagged (`assistant_reply` or
`capability_calls`) under `LoopModelResponse.output`; do not invent a `type` tag
or flatten capability calls into text. Thirty actor/flow/root/global-lifecycle/
process checks and strict caller linting passed in
`/private/tmp/brassclaw-monty-recipe-flow.log` and `...-lints.log`. The four pure
flow cases execute actual interpreter helpers without simulated component/Tool/
provider success. This format is transient prepared metadata, **not an accepted
new field in persisted Recipe JSONB**. Authoring/compiler support, recursive schema
proofs, complete approved snapshot manifests and durable occurrence/effect records
must precede activation. The source bound is now 32 KiB to accommodate the actual
verified root; no data truncation was introduced.

Production turn-runner configuration now distinguishes an optional independent
driver wall-time liveness deadline from Monty task compute duration. Composition
does not feed `max_duration_secs` into the wall-time timer, which would charge
provider/queue waits. Explicit driver deadlines still stop the exact attempt before
claiming more work. A failed actual startup settings DB read now fails startup
instead of silently disabling limits. Twenty turn-runner checks and one native
PostgreSQL settings failure/default case passed in
`/private/tmp/brassclaw-monty-compute-not-wall-time.log` and
`/private/tmp/brassclaw-monty-settings-startup-failure.log`. Strict Reborn and
composition all-target checks passed in `...-compute-not-wall-time-reborn-lints.log`
and `/private/tmp/brassclaw-monty-startup-proof-final-lints.log`. The migration proof
is used by the native fixture; its must-use warning was fixed without suppression.
Static legacy duration uptake is not the shared live effective-revision cutover.

**The seven original composition failures remain unresolved.** Their production
driver still reaches the UUID-only Thread load. None of these checks changes
their assertions, substitutes a model reply, activates global hosting or completes
Phase 3a. Continue with actual recursive contracts, immutable catalogue/association
selection, the two-dependency Monty migration and startup-owned service integration.

### 2026-10-07 Recursive contracts, typed IBS preparation and graduation evidence

`brassclaw_skills::value_contract` now validates the recursive contracts in
`skills.md`: exact fields/duplicate keys, nullable versus missing values,
consumer-only defaults, object/list children and extras, Unicode lengths,
exact numeric bounds/cross-input checks and declared computed arguments.
Binding checks aggregate default expansion before copying; output validation
never fills missing data. Producer/consumer types must match exactly, including
integer versus number. Error paths remain private because dynamic keys may
contain input data. Nine focused contract checks passed in
`/private/tmp/brassclaw-monty-exact-binding-types.log`; strict default-feature
checks passed in `...-lints.log`. The existing legacy credential functions and
tests now correctly require `v1-types`; their relevant enabled-feature checks
remain passing (`...-credential-feature-boundary.log`, `...-credential-feature-lints.log`).

`prepare_typed_program` combines the actual ordered typed composer with
`typed_bindings::prepare_input_layout`. It rejects incomplete/duplicate local
bindings, undeclared task references, forward results, unsafe optional/null
field access, wrong declared types and invalid constants/default requests.
Prepared metadata contains data references, not substituted code. Four actual
IBS checks passed in `/private/tmp/brassclaw-monty-typed-input-preparation.log`
and again through the affected caller in `...-association-ibs-caller.log`.
This is straight ordered-workflow preparation; it neither establishes trusted
catalogue/association approval nor proves conditional graph guards.

The actual legacy validation queue now locks the review row inside its
graduation transaction and shares queue-before-component lock order with
invalidation/purge. Rejection and threshold promotion use one guarded update.
Upgrade fields/types fail closed at submission and graduation; class checks
prevent a class-1 usage ticket from modifying/deleting a class-10 Orchestrator
in the shared table. Invalid Q1 pass-with-errors is rejected before SQL.
V094 retains exact component/queue bytes, SHA-256 checksums and reviewer in
immutable graduation receipts before queue deletion. Receipt insertion failure
rolls back the whole graduation. These legacy receipts are **not**
`skill-association-approval/1` evidence and do not fix mutable component rows.

The repeated real graduation regression exposed a V077/V092 interaction:
updating `last_graduation_at` incorrectly required a new resource settings
revision. V095 permits only that cursor to differ at the existing revision;
every other column remains revision-checked, including future settings columns.
Actual resource changes still fail without the next revision. Cursor updates
also work at exhausted revisions; settings changes remain rejected. Sixteen
native queue cases and the native settings CAS/exhaustion case passed in
`/private/tmp/brassclaw-monty-graduation-evidence.log` and
`...-graduation-settings-revision.log`; strict composition all-target linting
passed in `...-graduation-evidence-lints.log`.

Production remains the legacy UUID-only per-chat driver. The seven original
composition cases have not been made green by these prerequisite changes.
The model trace also identifies missing Sempai/Kohai hooks at the new task-host
port; integrating them must capture host-resolved prompts and preserve issued
prompt authority/structured Tool replay, rather than forwarding raw reference
strings as a Sempai-adjusted prompt. The resolved-prompt/accounting repair is
covered by the 2026-10-07 section below; global hosting remains open.

### 2026-10-07 Exact association declarations

`brassclaw_skills::association_contract` parses the exact `skill-association/1`
and `skill-association-approval/1` shapes from skills.md. It rejects duplicate
and unknown fields, malformed UUID/class/version/checksum references and
incomplete argument maps. Actual arguments must retain direct input values
and satisfy computed contracts; the registered Tool contract is checked
separately. Combination selection compares exact association bytes and the
entire supplied dependency graph. Failure declarations count the initial
dispatch and require the declared retry/idempotency evidence. Parsing never
resolves evidence, grants permission or performs a retry.

Five new association checks and the existing eleven unit/nine recursive
contract checks passed in `/private/tmp/brassclaw-monty-association-contracts.log`.
Four actual IBS caller checks passed in `...-association-ibs-caller.log`.
Strict all-target checks passed for Skills with db-store/v2-compat and Engine
with skills-db (`...-association-contracts-lints.log`,
`...-association-ibs-caller-lints.log`). Parsed approval declarations remain
untrusted: immutable catalogue storage, evidence provenance, activation and
production manifest consumers are still required. The seven original failures
remain open; this contract work does not activate the candidate global service.

### 2026-10-07 Resolved model interception and shared review accounting

The actual task-host model port now captures host-authorized resolved messages,
not executor reference strings. Native PostgreSQL cases preserve the selected
System prefix, opaque conversation identity, structured capability requests and
Tool replay metadata. Sempai uses that selected prefix and valid message refs;
invalid review output stops dispatch. Public raw resolved-prompt injection is
rejected before consuming a prompt grant. The reference-only executor hook no
longer creates a duplicate forensic packet.

Sempai review checks the same model policy and resource governor as its enclosing
call, with an isolated reservation namespace retaining the exact run identity.
Reported usage reconciles at the selected review-model price. Policy/budget denial
calls neither provider and releases the enclosing reservation. Concurrent
reservation admission now claims before creating a governor hold. V096 supplies
the JSONB snapshot column omitted by the existing PostgreSQL budget store;
initialization and update use distinct CAS writes, and exhausted versions fail
without overwriting data or recreating a deleted snapshot.

Four native model-port cases, two native governor CAS cases, fifteen accountant
checks, all 103 Reborn host cases and 55 model-port cases passed in
`/private/tmp/brassclaw-monty-review-budget-{native,cas,accountant,host,support}.log`.
Strict all-target checks passed for Reborn with and without root-llm-provider,
LoopSupport, Resources and the skills-db composition consumer in
`...-review-budget-{lints,default-lints,consumer-lints}.log`. These results cover
real host ports and PostgreSQL with recording provider fixtures. They do not
claim live provider coverage, global startup wiring or resolution of the seven
original composition failures. The production driver still requires an engine
Thread; the Monty 1.0/resource/catalogue cutover remains open.

### 2026-10-07 Exact Q1 candidate and Q2 review seals

Q1 now reviews the actual proposed upgrade, while retaining native PostgreSQL
canonical bytes for both the current component and its queued submission.
Recording a pass or failure compares those exact bytes again under queue-then-
component locks. Q2 validates the proposed fields and refuses graduation if the
reviewed content, metadata or submission changed. Accounting/lifecycle updates
do not invalidate an authoring review. V097 retains existing proposals and
returns previously unsealed Q2-pending rows to Q1 rather than inventing evidence.

Validator Recipe/PythonCode selection uses one read-only repeatable-read
transaction. Unapproved code, multiple entry points, extra bindings, malformed
steps and ambiguous validators fail explicitly. Trusted checked-in seed auditing
retains its existing bootstrap exemption with exact candidate seals; it does
not constitute ordinary authored Q1, human Q2 or combination approval.

Twenty-four actual PostgreSQL queue cases, thirteen Q1 cases (including native
proposed-content execution), the idempotent boot/integrity case and strict
composition all-target linting passed in
`/private/tmp/brassclaw-monty-q1-review-{queue,caller,boot,lints}.log`.
These seals do not implement immutable component versions, persisted validator
dependency manifests, system-validator visibility across legacy scopes or the
full recursive/semantic Q1 gates. Those remain explicit cutover work; the seven
original message-flow failures remain open until the global driver is wired.

### 2026-10-07 Global service, model/Tool caller and durable handoff

The isolated Monty 1.0 service retains admitted task ports, started host futures,
actual late answers and failed IPC exchanges independently of turn waiters.
Dropping a waiter fences only its own attempt. Root/child settlement and return
to the real work wait precede a successful local cancellation acknowledgement.
Fatal transport failures retain evidence and wait for started host futures;
they do not authorize task replay or claim external-effect quiescence.

Service-mediated settings publication obtains the VM's exact revision receipt
before publishing the same effective task limits to Rust. It preserves compute
consumption, including while a Tool waits, and preserves the root boundary
still awaiting service delivery. Stale edits fail without changing either side.
All five service cases passed in
`/private/tmp/brassclaw-monty-live-settings-service.log`; the service strict check
passed in `/private/tmp/brassclaw-monty-owned-service-lints.log`. This is not yet
a WebUI desired/effective store subscription or adaptive live-heap uptake.

The real model caller now resolves authorized complete eligible transcript and
retained Tool payload bytes before model policy/accounting. Both Reborn model
gateways retain that prepared request for provider dispatch. Disabled token
budgets preserve all eligible history; the native caller verifies 140 historical
messages, exact current input, exclusion of later input and joint live budget
revision changes. The selected system bundle stays fixed across model calls.
All 103 affected Reborn host cases, six model-gateway cases and strict affected
consumer checks passed in
`/private/tmp/brassclaw-monty-prepared-{host,model-gateway,consumer-lints}.log`.

V098 retains actual capability inputs/results with exact identity checks and
immutable conflict rejection. The native global Python caller invokes the real
filesystem Tool through the instance-policy kernel, reads its persisted result
and supplies its complete output and provider replay metadata to the next model
call. PostgreSQL transcript persistence now uses a private replay-preserving
serializer; public history still omits that metadata. Repeated result appends
retain their original message identity and reject conflicting provider metadata.
Recording gateways remain provider fixtures; no live provider claim is made.

`global_monty_driver.rs` is the owned-handoff cutover candidate, compiled through
the isolated caller rather than enabled in application startup. It reads the
exact admitted input, supplies complete typed history, addresses cancellation by
the Rust-only attempt and returns the actual published final-reply reference.
Two opaque conversations complete on one already-started service. Its capacity
is bounded; unresolved attempts and receipts require supervisor reconciliation.

V099 and `pg_monty_admission.rs` retain durable no-replay admission receipts.
Reservation and each host dispatch check the real locked turn snapshot, exact
scope/message/run/claim and actual database-time lease validity. Settlement is
written after the actual service receipt, not merely Python's finish request.
Repeated fresh execution and stale claims fail explicitly. These records are
neither Tool grants nor effect deduplication evidence; uncertain admissions and
continuation recovery cannot be reset or replayed automatically. Three native
caller cases, including durable settlement/replay/stale-claim checks, and strict
caller linting passed in
`/private/tmp/brassclaw-monty-durable-driver-{caller,lints}.log`.

The original seven composition failures remain open: production startup still
wires the legacy UUID-dependent driver. Before replacing it, complete the
coordinated application Monty dependency/API/resource migration, exclusive
instance-owner supervision through settlement, approved catalogue/Recipe/IBS
snapshot adapters, durable waits/children/effects and live settings wiring.
These caller proofs must be reused by that cutover; they do not weaken or
replace the seven original acceptance assertions.


### 2026-10-07 Contained application utilities and packaged worker

Engine and Composition now share the versioned vendored Monty 1.0 dependency.
Pure Python syntax/formatter and Q1 validator execution use the separately
packaged disposable worker. Caller inputs are bounded before copying, runtime
values stay separate from source, and actual interpreter/containment diagnostics
remain visible. Source, recursive value, frame, output, compute and wall bounds
are distinct from disabled token budgets. There is no in-process fallback.

The worker is a root workspace member and is packaged beside both supported
product binaries. Source-build instructions, installer checksum verification,
release assets, CI test prerequisites and Docker copies now include it. Shell,
JavaScript and workflow syntax were checked; Docker/release builds have not been
executed. The actual Engine formatter cases, syntax caller, all thirteen Q1
cases and affected strict consumer checks passed in the
`/private/tmp/brassclaw-monty-v1-contained-*.log` and
`...-monty-boot-{utilities,utility-lints}.log` batches. Utility waiter Drop still
uses child kill-on-drop; it does not provide an owned reap receipt to that caller.
Other legacy Engine execution paths remain in-process and must be retired or
contained before production upgrade acceptance.

### 2026-10-07 Startup ownership and retained quarantine

Instance ownership is registered before awaiting worker boot and retained by a
process-owned registry through actual worker and host-future settlement. Dropping
a startup waiter, quarantine evidence or a supervisor no longer unlocks a live
or uncertain instance. Quarantine capacity is bounded and fails before spawning
another worker. Only proven quiescence releases the guard; failed unlocks remain
visible. Real PostgreSQL cancellation-during-boot and evidence-drop cases pass
alongside the existing global/model/ownership caller cases: all seven native
cases and strict caller checks passed in
`/private/tmp/brassclaw-monty-quarantine-{caller,lints}.log`. These are isolated
ownership/model proofs, not the original seven composition acceptance failures.

### 2026-10-07 Removed allocation count and honest lifecycle facade

V100 retains old allocation-count values in immutable historical storage without
changing operator revisions. Active DTOs explicitly report that Monty 1.0 does
not support that limit; requests to set it fail before store mutation. The WebUI
shows retirement and the retained historical value. Defaults, custom historical
values, revision exhaustion, old writers and atomic settings edits were checked
against real PostgreSQL in
`/private/tmp/brassclaw-monty-allocation-{migration,http,consumer-lints}.log`.

An unwired facade now returns unavailable for VM status/restart instead of
fabricating Running, an applied settings revision or Restarting. The regression
uses real workflow/turn services in
`/private/tmp/brassclaw-monty-lifecycle-facade.log`; default-feature and skills-db
consumer linting and Q1 checks passed in `...-lifecycle-*.log`. This deliberately
does not claim that the actual global lifecycle port or WebUI publication exists.

### 2026-10-07 Shared VM allocation domain and live heap receipts

The versioned `control.6` allocator records ownership on each allocation, including
headers/alignment padding. Tags survive nested scopes, scope exit, grow/shrink and
cross-thread deallocation. Synchronous worker VM hosting includes root/child
interpreter state, compilation and controlled adaptation; frame decoding and
encoding remain outside. Real shared bytes, rather than baseline subtraction or
serialized length estimates, travel in private worker protocol 4. Physical
worker headroom remains separately finite; logical growth currently cannot
exceed that configured physical reserve.

The instance service requires an initial logical limit before its real Ready
handshake. Revisioned manual reductions below live usage are rejected without
changing effective limits. Automatic reductions remain pending with admission
backpressure until actual state reclamation permits uptake. Existing scopes,
other tasks and compute charges are retained. The actual root-plus-child caller
checks reclamation, stale edits and a native allocation rejected by the new soft
limit while the parent/root remain usable. Standalone physical-backstop probes
explicitly omit soft preflight; the instance service refuses that configuration.

Task and heap edits share a bounded FIFO service control lane. Actual worker
acknowledgement precedes publication. Expected infeasible edits do not kill the
service; uncertain transport/protocol failures retain actual receipts and require
containment. A dropped edit waiter cannot withdraw an accepted publication.
Allocator ownership, ten process cases, five initial service cases and seven
native owner/model cases passed in
`/private/tmp/brassclaw-monty-{vm-ownership,live-heap}-*.log`. The expanded six
service cases, real soft-limit caller and strict control consumers passed in
`/private/tmp/brassclaw-monty-owned-heap-{process,service,lints}.log`.

The pending-reclamation timer and syntax/transport diagnostic distinction passed
the relevant service/utility callers and strict consumers in
`/private/tmp/brassclaw-monty-heap-pending-*.log`. The correctly named actual
syntax caller and architecture checks passed in `...-correct-syntax-caller.log`
and `...-host-architecture.log`; the earlier zero-case syntax filter is not
acceptance evidence. Rust 1.96 Host checks passed in `...-heap-msrv.log`. OS/cgroup pressure sampling, physical
backstop resizing, full adaptive publication, immutable catalogue/association
selection, durable effects/waits and actual application startup wiring remain
open. None of these proofs substitutes a No-Match-only production adapter for
ordinary Recipe support. The original seven remain open until that conforming
cutover and their unchanged production assertions pass.


### 2026-10-07 Immutable revision retention and retained Recipe IBS

V101 introduces instance-wide append-only component revision storage separate
from legacy mutable component rows. The strict `component-revision/1` retention
envelope preserves exact document, stable dependency and Skill-association
bytes under one checksum; it is not a new Recipe `step_descriptions` syntax.
Monotonic per-identity CAS rejects concurrent losing edits and class changes.
Failed transactions do not consume a version. PostgreSQL rejects updates,
deletes and truncation of retained revisions. No legacy validated label or
`source=system` row is automatically imported as approved.

Exact graph loading uses a read-only repeatable-read transaction, checks every
UUID/class/version/checksum, rejects missing/unrelated/cyclic dependencies and
keeps exact Skill association bytes with their owner revision. Bounds reject
an entire oversized graph rather than truncate it. The snapshot deliberately
has no approved/active flag: full graph retention is not trusted review evidence.
Four real native PostgreSQL cases and strict Skills/control consumers passed
in `/private/tmp/brassclaw-monty-revisions-{native,skills-lints,control-lints}.log`.

The Engine's retained Recipe IBS compiler selects an embedded variant only
from that exact retained Recipe document. It retains the complete graph with
Recipe revision, variant, step_link, variable patterns and ordered instruction;
unsupported fields, ambiguous variants, wrong executable classes and invalid
single-component references fail before effects. It never fetches latest or
executes prose. Workflow classification is supplied separately by the trusted
review/caller; this compiler does not infer approval/tier from mutable metrics.
Two actual PostgreSQL-to-IBS cases passed in
`/private/tmp/brassclaw-monty-retained-ibs-native.log`. The first lint run identified
an explicit test loop counter; it was changed to an iterator zip. Strict Engine
consumers then passed in `...-retained-ibs-engine-lints-fixed.log`.

Remaining catalogue work includes supported authoring/review adapters, exact
trusted Q1/human Q2/behavioral evidence resolution, immutable Tool artifacts,
atomic active generations, typed binding/contract metadata and durable task
snapshot references. These retention/IBS callers do not activate components or
wire the production global driver. The seven original composition assertions
remain unchanged and unresolved until the conforming production cutover passes.


### 2026-10-07 Unsolicited worker exit supervision

The transport owner now awaits actual native child termination while IPC is
idle, using cancellation-safe `Child::wait`. Incoming commands cancel only
that wait, never the worker. An unsolicited exit closes its private command
ledger, retains the real exit status and publishes a termination notification.
The instance service observes that notification without an Inspect RPC, work
polling or a new task. It closes admission, fences task ports and retains late
actual host results until its owned futures settle. A terminated worker never
becomes a successful global shutdown or a successful external-effect receipt.

Six actor cases, eight service cases, seven native ownership/model cases and
strict control/Engine consumers passed sequentially in
`/private/tmp/brassclaw-monty-idle-exit-{actor,service,native-owner,control-lints}.log`
and `...-retained-ibs-engine-lints-fixed.log`. New real SIGKILL cases cover an
idle worker and a worker with paused real file I/O; the latter fences immediately,
delays settlement until the actual read returns, retains its value and never
posts a reply. The standalone fatal physical-allocation probe now explicitly
omits logical soft preflight; its fatal-exit assertions remain unchanged and a
ServiceOwner rejects that probe configuration. The first mismatch is retained
in `...-idle-exit-actor-soft-preflight-mismatch.log`.

These are contained service/owner prerequisites. Production runtime startup
still uses the legacy driver, and the original seven failures remain unresolved.
No process death, retained manifest, component label or passing isolated model
case authorizes a Rust-loop or No-Match-only production cutover.

### 2026-10-07 Retained typed inputs and actual Monty Recipe handoff

`memory/retained_inputs.rs` now reads recursive PythonCode input/result
contracts and selected variant bindings from the same retained Recipe graph.
The new retention-document metadata is documented in
`docs/reborn/contracts/retained-component-inputs.md`; it is not silently added
to the legacy Recipe/PythonCode INSERT constructors. Preparation requires exact
selected step/local coverage, typed constants/defaults and compatible earlier
result fields. Concrete values remain data. No failed refinement becomes a raw
positional slot, and no source interpolation or output invention is introduced.

Retained variant example capture requires complete anchors/separators and the
declared semantic names. Regexes compile once during preparation, with finite
technical capture/compiled-regex/cache bounds. Full-slot refinement and named
group participation are checked on concrete data. This is not active intent
selection or a proof that the example passed semantic review. Numeric/boolean
conversion remains explicit reviewed component logic, rather than text coercion.

The existing typed composer now has an exact retained PythonCode resolver for
workflows without Tool bindings or nested source assembly. Those two unsupported
cases fail explicitly; this constructor is draft validation, not a restricted
production fallback. Prepared layouts emit the existing `recipe-flow/1` control
data. The global Python helper consumes the bound `inputs` from composition for
that path and owns ordering, result state and the return handoff. Admission's
message layout need not equal the Recipe's captured/local input layout.

Actual PostgreSQL-to-IBS cases passed: two retained-instruction cases and three
retained-input cases. The latter includes the actual global `_execute_recipe`
helper requesting both selected steps, executing their actual retained bodies
in one task-owned child context, and carrying the first result through Monty to
the second. Rust transports and validates those requests/results; it does not
select the next step or invent Tool/provider success. The validation entry does
no intent matching, publication or component activation and is not advertised
as the ordinary runtime caller. Its final probe is not acknowledged as a product
task finish or external effect; the actual worker is reaped.

Evidence: `/private/tmp/brassclaw-monty-retained-flow-native.log`,
`...-retained-root-child-native-final.log` and strict affected Engine checks in
`...-retained-root-child-lints.log`. The same global source passed 32 affected
flow/global/process/native model callers on Rust 1.96 in
`...-retained-flow-callers.log`. The preceding complete contained caller set
passed 84 cases on Rust 1.96 in `...-ci-msrv-callers.log`. Architecture dependency
and composition checks passed 29 cases after the new storage/test dependency
edges; test binaries were run serially after Cargo finished, so their internal
Cargo metadata calls did not overlap another Cargo execution. CI includes the
retained PostgreSQL callers and builds the actual companion worker first.

The first authoring fixtures omitted required IBS/check fields, then the stronger
transport fixture used an unsupported task duration and failed to defer an
async host call. These were corrected without relaxing validators, budget bounds
or continuation checks. Their failed diagnostics remain in the corresponding
`...-retained-inputs-*-fixture-failure.log`,
`...-retained-root-child-invalid-fixture-budget.log` and
`...-retained-root-child-undeferred-fixture.log` files.

Next production work remains exact Tool implementation retention, trusted
component/combination review and coherent active-catalogue generations, complete
nested assembly and ordinary task-port/boot/shutdown wiring. Durable effect,
wait/reclaim and adaptive resource acceptance remain required. This evidence
does not close those gates: the original seven runtime composition failures
remain unresolved and their assertions are unchanged.

### 2026-10-07 Retained Tool binding preparation and actual handler retention

`memory/retained_tools.rs` prepares a ToolSkill/PythonCode pair from the same
exact graph used by IBS and typed inputs. The new retention-document binding
contract is documented in `docs/reborn/contracts/retained-component-inputs.md`.
Preparation verifies the explicit Tool UUID/callable/capability mapping,
descriptor dependencies, unique matching Skill association, associated code
contracts and compatible primitive parameters. Runtime arguments are checked as
data. Legacy interpolation, Ignore/Fallback and implicit retry policies fail
before a binding is exposed. Mutable path-based load directives are not emitted.
Nested source assembly, trusted review and actual implementation registration
remain separate gates; this preparation is not approved execution.

The real PostgreSQL-to-IBS binding regression passed, including incompatible
replacement contracts, changed mappings, missing/extra descriptor metadata and
argument injection rejection. The previous retained instruction/input cases also
passed: six cases total in `/private/tmp/brassclaw-monty-retained-tools-native.log`.
Strict Engine checks passed in `...-retained-tools-lints.log`. CI includes the new
caller with the retained PostgreSQL cases.

Host Runtime now exposes `retain_binding` for an actual registered first-party
handler. The returned handle owns that concrete implementation independently
of registry replacement or destruction and refuses a request for another
capability on a multi-primitive handler. A real JSON primitive regression proved
retention and release. A second regression passed through the actual
CapabilityHost/InstanceToolAuthorizer path: the same retained implementation
succeeded at revision 1, was denied at revision 2 and succeeded at revision 3,
without legacy scoped invocation grants. This does not establish registration
provenance, artifact checksum/ABI compatibility, cross-restart retention or a
stable Tool-UUID policy mapping. Those must be supplied by the catalogue and
implementation adapter before production cutover.

Both host regressions and strict Host Runtime checks passed in
`/private/tmp/brassclaw-monty-retained-handles-{native,kernel,lints}.log`.
The original seven were rerun individually and serially against the current
composition binary: all seven still failed. Evidence is in
`/private/tmp/brassclaw-monty-original-seven-current-{build,cases}.log` and the
individual `brassclaw-original-seven-current-{1..7}.log` files. The runtime
still constructs the UUID-only legacy driver; these focused proofs do not
resolve those failures or authorize fallback.

### 2026-10-07 Retained step execution and caller-owned selection snapshots

`executor/retained_recipe.rs` now executes exactly the PythonCode step requested
by Monty, using the retained IBS program and concrete typed inputs. One admitted
task owns one child context across these feeds. Rust advances only mechanical
control/host boundaries; the global Python helper still chooses each next step,
retains result state and resolves the Recipe's input references. Unknown steps,
missing implementations, repeated feeds and unsupported implicit retry policies
fail explicitly. An actual execution/output failure fences subsequent feeds;
dropping a feed future leaves the execution fenced, with actor receipts still
owned by the instance. This does not implement durable invocation retry/reclaim.

A Tool boundary must match the exact associated callable, accepts validated
keyword data, and dispatches through the supplied actual retained kernel adapter.
A completed host answer is retained before resume or result validation. Worker
failures preserve their actual error/snapshot and the submitted command recovered
from the actor ledger; command recovery failure is retained explicitly too.
This private evidence remains available after conversion to a classified port
failure. It is not a durable effect journal, cancellation acknowledgement or
permission to retry. Supervisor settlement still owns actual context release.

Binding preparation now indexes selected steps/usages once and caches each
usage's complete dependency closure. Repeated uses share that retained closure.
Association combination references include the Skill owner and its transitive
selected dependencies, without unrelated Recipe steps. Aggregate combination
capacity rejects the entire assembly rather than truncate approval references.
These references establish selection only; no parsed declaration becomes trusted
Q1/Q2/behavior evidence.

`PgComponentRevisionStore::read_exact_in_transaction` supports exact graph reads
inside the caller's repeatable-read/serializable selection transaction. It refuses
read-committed isolation, does not open a second database view and never commits
the caller's transaction. The native regression advances a real revision from a
second connection: the original selecting view retains its old head/content,
rejects the not-yet-visible replacement and leaves both immutable versions usable
after commit. This is a selection primitive, not an active catalogue or approval.

The real isolated caller now covers PostgreSQL retention/IBS, the actual global
`_execute_recipe` helper, actor transport, child VM and real CapabilityHost with
InstanceToolAuthorizer. It invokes the retained JSON implementation after its
source registry is destroyed. A live policy edit denies the next explicitly
requested step. A separate invalid-output draft retains the actual successful
Tool answer before output validation fails, and neither case permits replay.
No provider response, external effect, product completion or component approval
is fabricated. These are unapproved behavioral-validation drafts, not ordinary
application startup. CI includes this caller; the isolated lockfile gained only
the Engine/Skills dependency edges and required existing YAML bridge packages.

Five storage cases and the real kernel caller passed in
`/private/tmp/brassclaw-monty-retained-snapshot-kernel-native.log`; strict isolated
consumers passed in `...-retained-snapshot-control-lints.log`. Earlier root cases
passed in `...-retained-executor-native-final.log`. All seven root retained IBS/input/Tool cases and strict root Engine consumers
passed in `...-retained-snapshot-root-{native,lints}.log`. The serial minimum-
Rust-1.96 run passed all 117 isolated caller cases and strict all-target linting
in `...-retained-msrv-{callers,lints}.log`, with actual companion binaries built
in `...-retained-msrv-worker.log`. This is local evidence; GitHub CI has not been
run against this uncommitted change.
Initial fixture schema/worker-count mismatches and the missing ledger-origin
assertion are retained in `...-retained-kernel-native*.log`. The root lint's large
failure-record diagnostic was fixed by boxing the retained command, not suppressed.

The next ordinary task factory must load a consistent activated catalogue and
resolve actual immutable Q1, human Q2 (or the controlled system-seed path),
behavioral and association-combination records. It must retain exact Tool
implementation/ABI/artifact identities, persist the selected workflow and
invocation/effect evidence, and issue a private program reference. Composition
returns typed inputs/flow/step identities; `run_program` validates that task's
program reference and calls this retained execution primitive. Recheck actual
admission/ownership and live kernel policy at their dispatch boundaries. A failed
Recipe, approval lookup, SQL operation or ambiguity must never become No-Match.
Only after the remaining catalogue, nested assembly, durable lifecycle and live
resource gates pass may runtime.rs replace the legacy driver and start the global
owner before workers/ingress. The original seven remain unresolved; their tests
and assertions are unchanged.

### 2026-10-07 — Fail-closed review callers and retained usage records

The Recipe facade no longer falls back to a direct `validated` update when
Q2 queue graduation fails. Missing/nonpassed/stale Q1 and SQL errors preserve
the queue and prevent graduation. Recipe workflow-tag cleanup, the immutable
legacy receipt and attached prefix invalidation commit or roll back together.
Validator Recipes retain their intentional `05:validator` routing tag, so Q1
continues discovering them. The public queue path requires the legacy human
marker; checked-in bootstrap audits use a crate-private method. Neither marker
nor this legacy receipt becomes an exact-combination approval or Tool grant.

All three real facade regressions and 13 actual Q1 cases passed in
`/private/tmp/brassclaw-monty-review-final-{facade,q1}.log`; all 24 queue cases
passed in `...-review-queue-native.log`, and affected composition linting passed
in `...-review-final-lints.log`. These cases include a real PostgreSQL cache
constraint failure: graduation, tag cleanup, receipt and queue deletion all
roll back, then commit coherently once the fault is removed. Explicit Q1 queue
fixtures test lifecycle only, without claiming behavioral or semantic acceptance.

`V102` and the read-only Skills review store retain exact authored association
approval/evidence bytes in the selecting transaction. They verify the actual
immutable usage graph, stage identity and successful same-combination records;
human Q2 must cover precisely the chosen Q1 and behavioral records. Missing,
failed, duplicated, overlapping or swapped evidence fails explicitly. Records
are append-only; replacement leaves old usage versions and evidence available.
The actual native storage/resolution fixtures passed in
`...-association-records-native.log`, and strict consumers passed in
`...-association-records-lints.log`. These fixture records are deliberately
inactive and are not actual Q1/behavior/human-Q2 evidence. Trusted record writers,
controlled system-seed provenance, Recipe approvals and catalogue activation
remain unimplemented; the reader explicitly refuses system-seed mode.

The IBS review consumer binds precisely the selected Skill usages to retained
approval IDs/records, reusing each complete usage closure for repeated calls.
Missing/extra IDs or a nonexistent database record cannot be accepted merely
because binding/schema preparation succeeded. The actual PostgreSQL binding
caller and strict Engine/control consumers passed in
`...-ibs-review-{native,lints,control-lints}.log`. Workflow YAML parsing and all
29 classifier cases also passed after the CI updates.

`LiveStableToolPolicy` maps registered capability identities to stable Tool
UUIDs and publishes that mapping with the current technical rules. Old version
and alias mappings cannot be removed or reassigned by a settings publication.
The actual retained PostgreSQL/IBS/global/child/kernel caller now checks its
selected Tool UUID against this source before dispatch, then observes live
revocation on the next step. Pure identity/publication cases and the actual
kernel/global callers passed in `...-stable-policy-{native,kernel,retained}.log`;
strict authorization/host/control consumers passed in
`...-stable-policy-{lints,control-lints}.log`. Durable UUID policy persistence,
verified registration/artifacts and the ordinary runtime cutover remain work.

No original-seven assertion, Rust-loop fallback or empty-catalogue-only
production adapter was introduced. Their ordinary runtime path still uses the
legacy per-scope driver. Continue with trusted record producers, coherent active
catalogue selection and the full ordinary task factory, then complete the
resource/durable-lifecycle gates and wire startup/shutdown before rerunning all
seven. These prerequisites and draft callers do not resolve the original seven.


### 2026-10-07 — Preserve real intent/workflow selection through IBS

Fixed the legacy choice caller's cross-scope score update and caller-supplied
component identity. Choices retain their actual template and step link after a
locked identity check and commit. Scope score buckets use exact tuple keys,
reclaim expired entries and bound process metadata without limiting task/token
consumption. Template `_`, backslash and SQL escape text remain literal. SQL
ranks distinct component/class/link workflows before its candidate limit;
matching template duplicates and an intervening low-score class no longer hide
another eligible workflow. Host/PKR candidate mappings preserve row/link IDs.

Added caller-owned repeatable-read/serializable matching and a retained match
adapter into actual IBS. The adapter requires exactly one embedded Recipe
variant for the exact intent expression/link. It rejects identity loss,
ambiguity and mismatched old/new selection instead of choosing a variant or
falling back. Actual PostgreSQL replacement commits demonstrate that matching
and retained assembly preserve the old view together; later tasks use the new
view. These fixtures use draft revisions, not activated/approved components.

Evidence: `/private/tmp/brassclaw-monty-match-ibs-native.log` (3 actual PG/IBS
cases), `...-match-snapshot-native.log` (10 actual PG matching/choice cases),
`...-match-engine-lints.log` (strict Engine all-targets, skills-db) and
`...-match-composition-lints.log` (strict affected Composition library/test).
All passed with Rust 1.98; checks ran serially in background screen. Native
fixture initialization failures now fail the former Docker-skip intent tests.
The CI gate includes that actual native target. No original-seven assertion
changed; those seven runtime failures remain unresolved pending ordinary global
owner/approved-catalogue/dispatch-resource wiring. Immutable active generation,
trusted review producers, actual artifacts and durable invocation/reconciliation
remain prerequisites; these selection APIs do not establish them.


### 2026-10-07 — Durable stop-only dispatch and workflow selection

V103 and the private `PgMontyAdmission` invocation adapter retain an immutable
run/Recipe selection and initial Tool dispatch intent. The selection pins the
exact Recipe, embedded variant/layout, order/classification and complete retained
revision graph before execution. Later unexecuted steps cannot adopt a replacement
revision. Recipe UUID scopes local step IDs, so a follow-on Recipe does not collide
with the original workflow. Actual claim/admission checks precede insertion; a
repeated run/Recipe/step requires reconciliation instead of effect replay.

Actual host answers retain Return/domain/terminal classification and persist
before child resume/output validation. The private original handle can store a
late real answer after cancellation; exact repetition is idempotent and conflicting
answers fail. Unknown/storage-failed dispatches remain uncertain. SQL rejects
identity mutation, replay resets, deletes and truncation. These records are not
combination approval or Tool permission; live kernel policy remains independent.

The real PostgreSQL/IBS/global-Monty/child/kernel case retains success before output
failure and checks live denial on the next call. Its actual uncaught root failure
is acknowledged, the worker is reaped, and the real failed outcome is settled.
The native journal case uses a real JSON Tool result, real answer-storage failure,
late cancellation/result retention, a real immutable replacement, and a separate
follow-on Recipe with the same local step ID. No effect/provider completion or
human approval is fabricated.

Eight native model/ownership/journal cases and the retained Recipe/kernel case
pass in `/private/tmp/brassclaw-monty-invocations-selection-native.log`; strict
isolated all-target lints pass in `...-invocations-selection-lints.log`. Strict
root Engine all-target and Composition skills-db library lints passed in
`...-invocations-{engine,composition}-lints.log` for the unchanged shared trait and
dependency edge. This is local Rust 1.98 evidence, not GitHub CI or MSRV acceptance.

The ordinary runtime still uses the legacy driver, so the original seven remain
unresolved. Next work must supply trusted catalogue/review/artifact producers and
the normal global task factory, then connect these records before ordinary feeds.
Guarded repeated occurrences/retries, durable waits/recovery and adaptive/live
resource uptake remain explicit cutover requirements. Stop-only infrastructure
and unapproved draft callers do not satisfy that complete gate.


### 2026-10-07 — Strict audit verdicts and contained syntax observations

The existing Engine code-audit helper now accepts exactly five explicit ordered
PASS/FAIL verdicts. Empty, incomplete, extra, misleading or malformed responses
and non-text model outputs cannot pass. Its obsolete artificial 512-token cap
is removed; the verdict parser has an explicit byte-capacity error instead of
silent truncation. Source inspection found no live caller of this helper; this
fix does not activate a new ordinary Q1 or certify any component.

The contained utility now supports syntax observations using the same Ruff parser
version as Monty's compiler. It compiles first and executes no Python. Reports
retain exact-source checksum and syntactic host calls/ranges, imports, host-value
references, retired/dangerous name references and result stores. Strings/comments
are ignored. Bounded traversal/report failure is explicit. These observations do
not prove call reachability, approved dependent chains, semantic consistency or
producer provenance. The worker utility protocol is 2; global transport remains
unchanged. No new host Tool or ordinary runtime fallback was introduced.

Seven verdict cases and strict Engine lints pass in
`/private/tmp/brassclaw-monty-audit-{verdicts,lints}.log`. Five actual utility and
four existing flow-host cases pass in `...-structure-utility.log`, including
real parser errors and contained allocator/deadline failures. Strict isolated
all-target lints and all workspace architecture cases pass in
`...-structure-{lints,boundaries}.log`. Checks ran serially in background screen
with Rust 1.98. The original seven are still unresolved: no ordinary driver
replacement or catalogue activation is claimed by these checks.


### 2026-10-07 — Mandatory retained-source preflight

The retained executor now accepts only the exact program returned by contained
source inspection. The immutable result can be shared across task hosts and
repeated component uses. Every selected use is checked against its actual
prepared Tool binding before executable feeds. The current adapter supports
zero-call pure logic and one-call Tool usage; dynamic host access, unbound or
multiple call sites, forbidden/unsupported imports and intrinsics, and missing
result stores fail explicitly. No authoring rule forbidding supported dependent
chains is introduced; that broader binding adapter remains required.

Actual PostgreSQL drafts show misleading literals/comments are ignored and
wrong bindings/dynamic dispatch/intrinsics/imports/missing results are rejected
before execution. Actual compiler failures retain original source and worker
receipt. The existing real global-Monty/child/kernel success-before-output-error
and live-policy case still passes. Two native cases pass in
`/private/tmp/brassclaw-monty-source-retained-native.log`; four native typed-flow
cases pass in `...-source-retained-inputs.log`; strict isolated all-target and
root Engine skills-db all-target lints pass in
`...-source-{isolated,engine}-lints.log`. Checks ran serially in screen, Rust 1.98.

This supplies structural preflight, not semantic approval or catalogue
activation. The ordinary runtime/approved catalogue/production factory cutover
still remains, so no original-seven success is claimed.

### 2026-10-07 — Actual structural Q1 record producer

The private `pg_retained_q1.rs` candidate derives structural-only Q1 evidence
from the actual contained inspection of the exact retained Tool program. It
records the association checksum, complete selected usage closure and observed
source facts. It accepts no caller-supplied passed flag. Repeated usages share
one record; exact persistence is idempotent and conflicting bytes fail. The
original evidence IDs and bytes survive storage/unknown-commit failure, so
recovery persists the same observation rather than manufacturing a new verdict.

Two native PostgreSQL cases pass in
`/private/tmp/brassclaw-monty-source-q1-producer-native.log`; strict isolated
all-target checks pass in `...-source-q1-producer-lints.log` with Rust 1.98.
They cover exact old revisions after replacement, conflicting evidence and an
actual deferred commit failure followed by idempotent recovery. No component
activation, combination approval, behavioral approval or human Q2 is produced.
This is a private producer prerequisite; ordinary runtime integration and the
original seven composition failures remain unresolved.

### 2026-10-07 — Exact task-owned composition ports and routing eligibility

The private `GlobalRecipePorts` candidate now carries catalogue-owned inspected
programs through composition into opaque program references, typed task inputs
and actual compiler flow. Monty selects each next step; the adapter executes only
that requested retained PythonCode, keeps its child/Tool evidence, and persists
selection before any feed. Flat stop-only execution is supported; repetition,
retry and durable continuation require their separate addressing contract.
Provider waits do not hold Recipe lookup locks. The driver builds ports from
the exact admitted input and transfers the actual TaskControl during failed
settlement so reconciliation does not lose the retained execution.

The Engine catalogue matcher filters exact selected Recipe/link/example
membership before ranking. Mixed revisions, review-pending rows and excluded
drafts cannot hide an eligible workflow; SQL errors remain technical failures.
The filter itself establishes no approval or activation. Root guides clarify
Recipe -> IBS intermediate BuildInstruction -> composition -> executable Python
steps, with runtime inputs separate from source.

Actual native PostgreSQL/global/child/kernel task-port cases: **9 passed**, strict
isolated all-target lint passed in
`/private/tmp/brassclaw-monty-task-recipe-ports-{native,lints}-final.log`.
Actual Engine retained-instruction cases: **4 passed**, affected all-target strict
lint passed in `/private/tmp/brassclaw-monty-catalogue-eligibility-{native,lints}-final.log`.
These use Rust 1.98 and the real packaged worker, with serial Cargo execution.
The new negative completion case is an unapproved JSON draft without a reply
step; both Tool outputs satisfy its schema. It must not be described as an
approved Recipe or IBS producing an invalid reply.

Normal runtime still uses PersistentMontyDriver; the original seven are not
resolved by these cases. Trusted semantic/behavior/Q2 provenance, coherent
activation, actual implementation identity and full ordinary factory/startup,
resource and continuation wiring remain required before cutover.

### 2026-10-07 — Retain actual usage behavior for immutable review

The actual retained executor now provides opt-in bounded review observations on
its existing child/Tool path: bound typed inputs, dispatch arguments, actual
answer variant and validated result/classified failure. Fields are executor-owned;
unsettled/dropped execution cannot claim completion. Normal execution incurs no
additional value fingerprinting. Full answers and transport errors remain retained
separately, including a successful Tool followed by failed Python result validation.

`BehavioralReviewSet` compares declared expected values/outcomes with those actual
observations. Repeated calls of a usage retain one exact association closure;
mismatches are persisted as failed cases. A passing negative test explicitly
means the observed failure matched its expectation, not that the Tool/task
completed successfully. Reports state semantic_approval=false and
workflow_completion=false. Their producer owns no approval, activation or Tool
grant. The common actual PostgreSQL persistence path verifies the exact graph,
retains original IDs/bytes on failed commit and supports exact idempotent recovery.

Focused serial native execution: **15 passed** (9 model/task-port, 4 Q1/behavior,
2 retained execution) in `/private/tmp/brassclaw-monty-behavior-producer-native.log`.
Strict isolated all-target and Engine skills-db all-target lint passed in
`/private/tmp/brassclaw-monty-behavior-producer-{control,engine}-lints.log`.
This includes real JSON kernel effects, actual failed output validation, live
policy denial, cancellation, actual deferred PostgreSQL commit failures and
no replay. Typed fingerprint checks preserve presence/types and reject excessive
depth before traversal. The tested code uses Rust 1.98 and the packaged worker.

The original seven remain unresolved; ordinary runtime still constructs the
legacy driver. Human review must bind the complete actual record set; mutable
human/builtin markers do not supply v3 provenance. Recipe and pure-logic review,
system-seed evidence, consistent active generations, implementation identity and
the ordinary global factory/resource/continuation cutover remain implementation
work. The legacy builtin audit also still treats failures as nonfatal; its recovery
and trusted-seed writer require coordinated repair rather than relabeling existing
validated rows as approved v3 components.

### 2026-10-07 — Exact human association review through the operator WebUI

The existing RecipeStore/RebornServices port now supplies an exact immutable
usage review and explicit human decision through the normal WebUI composition.
The view retains the complete actual Q1/behavior record set and all selected
revision/association bytes. A domain-separated checksum binds the decision to
that view; replacement draft heads do not alter the reviewed combination.
Authenticated actor identity never comes from the request body. Stable decision
IDs, exact collision checks and one PostgreSQL transaction preserve atomicity
and idempotent recovery, including an actual deferred commit failure.

The panel displays actual source/evidence and keeps the original decision for
retry while mounted. No arbitrary evidence-write API, automatic semantic verdict
or catalogue activation was added; receipts explicitly state activation=false.
The existing composite-auth path also needed repair: ordinary signed sessions
could inherit operator route access from the mounted instance bearer branch.
Operator routes now authenticate each request against eligible branches, including
implicit HEAD reads. This adds no tenant/project/feature role checks. UUID fallback
creation uses actual cryptographic entropy and canonical v4 formatting.

Serial focused evidence: **15** actual Monty/kernel/PostgreSQL cases, **1** actual
HTTP/PostgreSQL review case, **47** gateway cases, **2** descriptor cases and **1**
real signed-session/instance-bearer case passed. Logs are
`/private/tmp/brassclaw-monty-human-q2-{native,caller,gateway,descriptors,real-auth}.log`.
Strict isolated and affected-consumer all-target checks passed in
`...-control-lints.log` and `...-consumer-lints.log` on Rust 1.98 after correcting
shared-fixture visibility. Changed JavaScript syntax and actual crypto UUID
fallback checks passed. The actual Reborn dependency-boundary case passed in
`...-architecture.log`; its binary ran after the Cargo build exited so metadata
lookup did not overlap another Cargo process. Browser interaction was not exercised.

This covers human Q2 for one Tool usage only. Trusted ordinary validation
provenance, whole-Recipe/pure-logic/protected-root review, controlled seed evidence,
coherent activation, actual implementation identity and full global startup,
resource and continuation wiring remain required. Nested PythonCode assembly
remains unsupported; a stored includes column alone does not implement it.
The original seven composition failures remain unresolved; ordinary runtime
still constructs PersistentMontyDriver. These prerequisite cases do not certify
the production global cutover or full v3 plan.

### 2026-10-07 — Accept actual empty leaf-code include metadata

Both retained preparation paths previously rejected every document carrying an
`includes` field, including the persisted V069 default `[]`. They now share one
leaf check that accepts an absent or explicitly empty array and a null dependency
registry. A malformed include value, nonempty include list, declared dependency
or non-null registry still fails explicitly before execution. This is schema
compatibility for leaf code; it does not implement recursive code assembly or
permit dropping nested dependencies. Exact source and typed data stay separate.

The real PostgreSQL/IBS tests use the actual empty-list/null defaults for pure
logic and Tool-calling code. Five cases passed, including actual global/child
result handoff, invalid output fencing, rejected malformed/nonempty replacements
and retained originals. Engine skills-db strict all-target lint passed. Evidence:
`/private/tmp/brassclaw-monty-leaf-includes-{native,lints}.log`, Rust 1.98, serial
Cargo in screen. The ordinary global cutover and original seven remain open.

### 2026-10-07 — Immutable operator review submissions

The existing RecipeStore/RebornServices facade now exposes operator-only staging
and exact-subject reads, wired through the actual PgRecipeStoreFacade. V104 and
PgReviewSubmissionStore reuse V101's CAS revision allocation and retain the
candidate's exact bytes, base reference, authenticated actor and complete declared
transitive dependency selection in one repeatable-read transaction. Stable IDs
recover identical requests before any new allocation, even after heads advance;
changed subjects/actors conflict. Incomplete graphs and actual commit failures
roll back the revision head and subject together. The records reject mutation,
deletion and truncation. No live legacy component, review evidence or activation
record is changed by the new staging path.

The advanced WebUI editor renders sources as text and retains the pending request
while mounted. First explicit rejections permit correction; uncertain requests
keep their original identity/bytes. Loading by ID does not automatically settle a
pending mutation from a potentially different actor. Browser reload is not a
durable outbox. Ingress/extractor bounds agree at 14 MiB; a real payload above
Axum's former 2 MiB default reaches the normal HTTP caller. The receipt explicitly
reports unreviewed/activation=false. Declared graph retention is not proof of
class-specific semantics, trusted validation, override enforcement or activation.
See [the submission contract](../reborn/contracts/component-review-submissions.md).

Tracing the legacy Sempai writer also exposed a byte-slicing panic and invalid
Recipe names. Intent proposal identifiers now use a schema-compatible UUID slug;
the full Unicode/long example remains in intent_examples instead of a truncated
name or oversized description. The real sink persists it and queues it pending.
This repair does not migrate Sempai to immutable submission identities.

Serial screen evidence on Rust 1.98: three actual PostgreSQL/HTTP/Sempai cases,
two locked-descriptor cases and ten gateway helper cases passed; affected
composition/product-workflow/WebUI/skills all-target strict lint passed. Logs:
`/private/tmp/brassclaw-monty-submission-{native,descriptors,gateway,lints}.log`.
The native queue first caught the legacy name constraint; strict lint then caught
the fixed-size chunk API update. Both were repaired, with no suppression. Final
native regressions and strict lint passed after those repairs; unchanged route
checks were reused. Changed JS syntax checks passed; browser interaction, CI and
performance measurements were not exercised.

This implements the operator submission prerequisite in validator_v3 Step 2,
not the complete step or plan. Legacy authoring/proposal migration, durable Monty
review admission, actual trusted validators, whole-workflow/protected-root and
controlled-seed evidence, coherent activation and the normal global factory,
resource and continuation cutover remain required. The original seven composition
failures remain unresolved; normal runtime still constructs PersistentMontyDriver.

### 2026-10-07 — Reconcile validator/prefix plans and retain Sempai draft fields

Read validator_v3.md and docs/plans/prefix_v3_upgrade.md completely. Their review,
bootstrap, catalogue, provider and prefix gates remain prerequisites for the
production global cutover, rather than approval that can be inferred from draft
storage. The next repair addresses the prefix plan's observed proposal loss.

The actual PgSempaiProposalSink now decodes the supported class-21/22 constructor
fields without discarding IBS descriptions, variants, dependency registries,
PythonCode includes, prior knowledge, override settings or routing metadata.
Unknown top-level fields and malformed typed fields are reported and rejected;
host scope, identity, provenance, approval and tier cannot come from the model.
Optional JSON values preserve absence versus JSON null. Candidate nested contracts
remain pending data for Q1, not proven valid or executable components.

Recipe/PythonCode creation and queue submission now share the author's transaction.
The prior two-transaction path could leave an orphan row after queue failure.
V105 repairs another actual blocker: V052's PythonCode source constraint rejected
every `sempai_proposal` write. The new migration admits that origin without
changing pending status or the existing system-seed rules. Store diagnostics keep
SQLSTATE without private candidate rows. See
[the supported persistence contract](../reborn/contracts/sempai-draft-persistence.md).

Five real PostgreSQL caller regressions passed on Rust 1.98 with skills-db: three
Sempai cases and two shared queue cases. They cover both proposal transports,
complete supported fields/source, pending delivery exclusion, unsupported fields,
malformed booleans/includes, repeatable provenance migration, queue/commit failure
rollback, known-failure retry and duplicate queue rejection. Checks ran serially
in screen with the required disk checks/cleanups. Composition skills-db strict
all-target lint passed without warnings. Changed documentation links and
`git diff --check` passed; no dependency/API edge or frontend changed.
Logs: `/private/tmp/brassclaw-sempai-v3-{native,queue,lints}.log`.

This is a repair of the legacy proposal transport, not validator_v3 Step 2's
complete migration. Sempai still needs immutable review identities, complete
proposed dependency selection, unknown-commit reconciliation, raw-byte duplicate
key rejection and durable review admission. No validator, Recipe, provider or
prefix was approved/activated. The original seven failures, normal global factory,
resource/settings and continuation cutover remain open; their unchanged tests
were not rerun for this storage repair.

### 2026-10-08 — Own global task preparation and settlement

Added OwnedGlobalTaskFactory for the existing global-driver candidate. It checks
the exact opaque conversation/message/turn/run handoff before admission and
retains the private admission address before any database I/O. Previously the
convenience reservation function returned its owner only after COMMIT, discarding
the address on an uncertain acknowledgement. That convenience now exists only
for native fixtures; application ownership uses prepare/persist separately.

Catalogue capture, failed/dropped preparation, child/Tool state and actual service
receipts stay owned under bounded capacity. A failed capture cannot replace its
reservation or turn into No-Match. Successful completion frees its slot only
after a real published reply and durable acknowledgement. Failed settlement
transfers the host, admission, ports and receipt together after acknowledgement,
without declaring effects reconciled or authorizing replay. Registry locks do
not span database/provider/VM waits. Concurrent acknowledgement cannot release a
replacement reservation at the same attempt address. See
[the task-factory contract](../reborn/contracts/global-monty-task-factory.md).

Seventeen actual worker/kernel/native-PostgreSQL cases passed serially in screen:
11 model-host/ownership/factory cases, four review-host cases and two retained
execution cases. New checks cover identity rejection before database writes,
failed and cancelled catalogue preparation, stable retained admission keys,
capacity backpressure, actual failed-Recipe settlement transfer, and subsequent
No-Match work on the same global root. The existing model/tool/provider replay,
opaque identity, cancellation, database ownership and no-Recipe-replay assertions
remain intact. Isolated all-target strict lint passed without warnings. Logs:
`/private/tmp/brassclaw-owned-task-factory{,-lints}.log`, Rust 1.98, with mandatory
disk checks/cleanups before each Cargo execution.

This uses the explicit draft-validation catalogue for its constrained caller
tests. It does not supply or certify a production approved-catalogue owner. The
ordinary runtime still constructs PersistentMontyDriver, and the original seven
composition failures remain unresolved. Protected-root/seed provenance, coherent
approved activation, retained actual implementations, live resource/settings and
durable continuation/recovery wiring still gate the ordinary factory cutover.
