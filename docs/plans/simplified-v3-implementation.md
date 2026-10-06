# Simplified v3 implementation record

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
