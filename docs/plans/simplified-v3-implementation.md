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
