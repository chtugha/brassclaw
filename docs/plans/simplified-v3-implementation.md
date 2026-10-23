# Simplified v3 implementation record

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
Each real Skill has a canonical execution Recipe with a matching command.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../skills.md#preloadable-function-interface-binding-v3-target).
Selected retained loader/store/runner support is implemented; this contract
does not certify every workflow or the complete migration. See
[the current implementation status](../implementation-status.md).
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


## Startup/manual heap floor — operator revision, 2026-10-09

The existing 512 MiB default is now the minimum configured heap budget. This
supersedes the earlier startup downsizing requirement. It is an allocation
ceiling, not an eagerly allocated reservation. Startup sizing may select between
this floor and a larger configured ceiling; low headroom/reserve readings retain
the floor with explicit status and do not fail startup solely for that reading.
Manual mode retains its configured value without measurements. Optional automatic
mode keeps its separately requested safe pressure/capacity behavior.

Implement one public default/floor constant, reject below-floor requests before
live publication and durable CAS, and enforce the configured floor in PostgreSQL.
Upgrade existing smaller values by one revision; preserve larger values, unrelated
settings and modes, and fail explicitly on exhausted revision arithmetic. Keep
worker live-allocation feasibility checks for reductions above the floor. WebUI
must display 512 MiB as its minimum and explain the budget/reservation distinction.
Verify actual migration, invalid CAS without mutation, startup low-capacity floor,
manual uptake and retained global VM identity through the existing production-path
checks. Earlier acceptance predates this new operator requirement.

V114 and the settings reader/writers now enforce that floor. Reconciliation
also restores the manual value or startup floor when switching out of optional
automatic mode; configured ceilings stay at or above the floor. The English and
German WebUI labels explain the retained startup minimum.

The serial local Rust 1.96 queue passes the native migration and atomic CAS
regressions, including unchanged larger budgets/revisions and actual SQL CHECK
rejection. The ordinary WebUI/startup/restart regression passes in 116.98 seconds
with the real retained worker and native PostgreSQL: an oversized reserve forces
below-floor headroom, the acknowledged startup budget remains 512 MiB, invalid
edits do not mutate the DB, valid manual edits retain VM identity, and periodic
measurements remain confined to optional automatic mode. Composition all-target
strict linting passes with `test-support,libsql,skills-db`. JavaScript syntax
checks pass.
This is acceptance of the operator floor, not the complete hosting/control or
component cutover.

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

### GitHub fixture follow-up — 2026-10-09

Run 37864642005 compiled the owned handoff fixture and exposed its missing
compaction prompt initialization in root shard 2. The filesystem fixture now
loads the identical first-party seed consumed by production component boot;
no synthetic prompt or production execution fallback was introduced. The unused
`AgentLoopDriverHost` import was removed. The behavioral rerun remains pending.

The subsequent run 37865174881 exposed the same omitted boot prerequisite in
hook/milestone tests and subagent direction prompts in the root filesystem
fixture. The three host-port targets now pass locally: hooks 38, loop host 103,
milestone projection 9, with strict target linting. All fixture constructors,
including unsubmitted messages, initialize the canonical compaction seed.
Root subagent fixtures initialize the canonical direction and failure prompts;
their GitHub behavioral rerun remains pending. These are retained historical
substrate fixtures, not additional production Monty acceptance.

### Portable memory measurements — 2026-10-09

`brassclaw_host_runtime::sample_process_memory_capacity` takes the supervised
worker PID. It performs read-only native measurements without subprocesses,
page walks or forced reclamation. Additional capacity excludes existing VM
allocations; the calculator adds the actual live heap once and applies the
operator reserve separately. A controller must fence the worker generation.

- macOS: free plus purgeable pages, bounded by physical RAM, with XNU dispatch
  pressure levels. Speculative pages are already included in free pages and
  are never added twice; inactive/compressed pages are not assumed available.
- Linux: MemAvailable plus host/cgroup PSI; each visible ancestor's memory.max
  and memory.high minus its current usage clamps growth. The actual unrestricted
  hierarchy root must be established from the memory-controller interfaces.
  Hidden or legacy v1 hierarchies, incomplete counters and movement during a
  sample return explicit unavailable/inconsistent measurements.
- Windows: native physical/commit headroom and nonblocking low-memory notification.
  A worker inside any job returns unknown containment: querying only its
  immediate job or its peak consumption cannot establish nested current limits.
  The native-counter test exercises the real APIs even inside a CI job.

Actual local macOS counters pass, with strict host-runtime/PostgreSQL-feature
linting. The architecture guard now permits only the two inspected private
Linux counter readers and still rejects unrelated file/application-state reads;
27 boundary tests pass. Linux/Windows execution is assigned to the new GitHub
OS matrix and is not yet claimed as passed. Calculator finite-sentinel checks
pass locally (8 adaptive-calculator tests and strict PostgreSQL-feature library
linting). The full local batch used Rust 1.98 with incremental/debug info off
and the NVMe target, serial `screen` queues; the first boundary run failed on
the missing kernel-counter classification and the repaired run passed all 27.

Remaining Phase 3a work includes production measurement ownership, bounded
sampling cadence, admission backpressure, all heap/settings WebUI controls and
complete container/job measurements. Manual reductions require serialized
worker feasibility and persistence coordination: inspecting a stale heap then
writing the DB is insufficient, and raising a runtime cap before a DB CAS can
permit allocations that prevent rollback. Preserve the acknowledged effective
limit and distinguish automatic budget revisions from desired settings revisions.
These prerequisites do not establish the completed adaptive-memory cutover.

### Revised memory requirement — 2026-10-09 (supersedes adaptive-default entries)

The operator requested the simpler default. Phase 3a now requires `startup`
(one conservative sizing, then constant), `manual` (live operator value), and
optional `automatic` (slow measurements; default interval 60 seconds). Historical
adaptive-default entries below record earlier requirements and do not govern
new implementation. Preserve existing stored limits as manual during migration.

Implement in this order: persisted settings/validated DTOs and explicit upgrade;
startup sizing with finite fallback and visible measurement status; serialized
manual worker feasibility plus durable CAS/commit coordination; WebUI controls
and exact desired/effective heap observations; optional controller sharing that
publication owner. No periodic pressure measurements in startup/manual modes.
Elevated pressure alone must not stop admission. Missing samples forbid growth,
but retain finite-budget operation while configured headroom exists. Critical
pressure or proved insufficient capacity can pause new work. Whole task cleanup
and bounded queue waits remain independent requirements. Retain typed results
between dependent steps; never discard unresolved effect evidence as cleanup.

Validation must cover actual startup and WebUI edits, preserved operator values,
no sampling in startup/manual modes, minor pressure without stalled admission,
missing-sample fallback, critical-pressure bounded admission, unsafe manual
reduction rejected without DB mutation, dropped waiter/commit readback, settings
conflicts and retained VM identity. These changes are not yet accepted merely
because the plan was revised.

### Memory-mode implementation and local acceptance — 2026-10-09

V111 adds the validated memory policy without changing existing byte limits;
existing rows become manual with a revision advance. New rows default to startup
mode with a 512 MiB ceiling/fallback. Startup sizing uses the real supervised
worker's capacity once; missing measurements retain the finite configured value.
Startup/manual mode never performs periodic pressure reads. Optional automatic
mode measures no faster than its configured interval, owns timed-out probes,
and keeps minor pressure from pausing otherwise available finite-budget work.

The existing WebUI/settings path now exposes mode, cap, reserve, sampling and
growth parameters, together with actual heap use, acknowledged revisions,
measurement status and admission state. A manual heap edit freezes the sole
transport owner, including direct child commands, until durable CAS/commit
settles. Known rejection restores the previous limit and any pending automatic
target; an unknown outcome contains the worker rather than permitting more work.
Accepted edits survive HTTP cancellation. Memory backpressure leaves bounded
admissions waiting under their existing attempt identities while controls and
active results remain serviceable.

Local evidence uses Rust 1.98, the NVMe target, incremental/debug info disabled
and one serial `screen` queue. All original seven composition callers and the
matched/unmatched regression pass with the actual retained worker and native
PostgreSQL. The WebUI regression additionally proves manual uptake, unsafe
reduction without DB mutation, no manual measurements, one optional measurement
per 60-second interval, return to startup sizing and unchanged live VM identity.
The native migration/CAS/startup-failure cases pass. Nine budget regressions,
16 real service tests, 58 HTTP contracts, 59 model-gateway tests, two repaired
trace tests and 27 architecture checks pass; affected library, host and control
workspace strict lints pass. Source review and JS syntax checks cover the form.

The first local queue lost its worker during mandatory cleanup; the corrected
queue retains the real built binary outside the target and restores it beside
the actual test executable. Feature-gate lint failures and the missed filesystem
fixture policy were corrected and rerun. The root macOS unoptimized binary still
emits the linker warning about the 16 MiB compact-unwind offset range. It remains
visible; panic recovery has not been disabled. Linux/Windows native CI, complete
job/container measurements, remaining hosting-limit controls, durable fatal
recovery and the full catalogue/authorization/prefix cutovers remain open.

### Duration range and GitHub follow-up — 2026-10-09

The plan now removes the arbitrary 30–3600 second duration range. V112 changes
only the database check to positive INT seconds; stored values and revisions
remain unchanged. The WebUI, PostgreSQL writer and shared Rust/worker budget
validator accept 1 through 2147483647 whole seconds. The 600-second default and
executing-time accounting remain unchanged; missing/null PATCH fields retain
their prior values. Zero, negative, fractional and out-of-representation edits
are rejected at the relevant input boundary, before durable publication.

The serial local queue passed seven resource-budget regressions, the real worker
active-account test with live 7200/1-second revisions, native V112 migration and
CAS tests, and the ordinary WebUI/startup/restart regression with a 7200-second
edit (126.21 seconds). Resource, composition and isolated control-workspace
strict lints pass. Rust 1.98 used the NVMe target without incremental/debug info;
the actual newly built worker was retained outside mandatory cleanup and restored
beside the actual test executable. No remote machine was used for compilation.

GitHub run 37872977914 exposed an obsolete coding-decoder lint allowance,
two CLI fixtures lacking explicit runtime policy, absent positive coverage for
the registered prefix primitives and two validation expressions rejected by
Rust 1.96 Clippy. The decoder now uses stable array chunks and rejects a trailing
incomplete UTF-16 unit instead of silently discarding it. All 16 coding caller
tests, including real Unicode edit/BOM/newline preservation and malformed-file
rejection, pass; both CLI DCR fixtures pass with explicit policy. Their affected
strict lints pass.

`factory::tests::postgres_prefix_tools_dispatch_and_publish_exact_bundle_once`
passes through the actual factory registry, real PostgreSQL backends and durable
publication. It verifies eligible content, pending/unrelated exclusion, exact
stored bytes/fingerprint/generation/time, wrong-conversation rejection, ticket
rollback and rejected replay without replacement. Composition strict lints pass.
The root capability inventory now names these two primitives and their actual
test. This is primitive-level acceptance of the existing backend, not qualified
library activation, a global Monty prefix Recipe or complete prefix-v3 assembly.
Rust 1.96 passes all 14 existing association/value-contract cases, the root
capability inventory and strict engine/all-target and coding-caller lints. This
checks the minimum-version diagnostics directly without suppressing them. No
GitHub rerun of this follow-up diff is yet accepted.

Release run 37872337895 passed Linux x86_64 and both macOS architecture builds
for commit 17a8920d5a2862ab5be39ab432a010e7d222c62b (`v1.7.0-rc.2`). That
release contains the revised memory modes; it does not include this subsequent
duration/CI follow-up diff or certify Windows runtime support or the full plan.

### Next hosting control: task Recipe selections — implementation contract

Replace both hidden eight-Recipe checks in `GlobalRecipePorts` with the existing
instance settings publication owner. Add `max_recipes_per_task`, default 8 to
preserve current behavior, with positive PostgreSQL INT representation. Persist
and patch it with the same CAS revision; do not create per-chat settings or a
second control service. Expose desired/effective value and revision in the WebUI.

The factory must supply a required, read-only capacity source backed by the
acknowledged complete settings view. New intent/named selections check the
current capacity under the task's selection lock immediately before retaining
them. A slow catalogue lookup must not retain against an obsolete preflight
value. Duplicate selections remain distinct composition errors. A reduction
retains every existing selection and its execution/results; only further
selections above capacity fail with an explicit resource reason. No truncation,
eviction, newest-version lookup, retry reset, Tier-2 fallback or VM restart.
Report the capacity/revision actually checked in retained settlement evidence.
The limit belongs to Rust's retained-selection owner; worker heap/context limits
remain independently enforced and must not be falsely reported as enlarged.

Acceptance needs real migration/CAS validation and ordinary global runtime/WebUI
edits, including a bound violation, a successful later task after raising the
limit, unchanged VM identity and no repeated completed effect. Retain isolated
control-caller compatibility; run checks serially after the coherent change.
This entry is a plan, not implementation or acceptance evidence.

### GitHub rc.3 diagnostics — 2026-10-09

Release run 37874982504 passed Linux x86_64 and both macOS architecture builds
for 351568dd3bce811bb82802a6ea8f8fcee031d9fd (`v1.7.0-rc.3`). Test run
37874986009 exposed two further failures: the Linux memory probe's unnecessary
borrow under Rust 1.96 Clippy, and seven composition turn callers compiled
without `skills-db`. The CI feature selector now includes the production
global-driver feature rather than substituting a Rust execution fallback.
The Linux borrow is removed. Deprecated Monty atomic updates use `try_update`
with identical checked arithmetic and ordering; all 228 upstream hashes and
the updated control patch's complete reconstruction are verified. The CI
rerun and current Recipe-capacity runtime acceptance remain pending.

### Recipe capacity, fixture repairs and floor acceptance — 2026-10-09

V113, the existing CAS settings path and the WebUI now expose live Recipe
selection capacity. The task checks the acknowledged complete revision after
catalogue lookup under its selection lock. Capacity reductions preserve selected
workflows, results and completed effects; settlement records the checked policy.
The ordinary native runtime regression passes a capacity-one violation without
Tier-2 replay or repeated reply, then a successful later task after raising to
16 in the same VM. Native migration/CAS and the original seven callers pass.
The isolated real model/worker target passes 18 tests; all 58 HTTP contracts pass.

The broader composition run passed 705 of 735 tests and exposed 30 fixture
failures: omitted explicit runtime policy in successful builders and three
startup-reserve dependencies on machine headroom. Their constructors now supply
the supported policy and durable test settings. All 30 formerly failing callers
pass through the actual compiled library/worker/native PostgreSQL after repair;
the production missing-policy guard remains enforced. The subsequent floor
queue passes the three affected migration/CAS/live-WebUI cases described above.

Rust 1.96 all-target composition/control lints pass, together with three actual
allocator cases. Shared Rust-1.92 crates use checked compare-and-exchange loops;
their library regressions and strict all-target lints pass. Four real native
PostgreSQL model-capture tests pass, preserving iteration capture and budget
holds. Cargo commands ran serially in local screen queues with the NVMe target,
incremental/debug info disabled and disk cleanup before each required command.

Current teaching-source integrity was reverified with the bounded Linux Python
checker: 29 examples, 132 behavior cases and six rejected defect mutations.
Historical receipts are preserved alongside the new exact-source receipt;
the local prefix source-preservation regression passes. These checks are draft
teaching evidence, not Monty/Q1/Q2 activation or the whole prefix migration.
GitHub acceptance of this new checkpoint remains pending. The complete retained
function loader, authored activation, MCP chat relay, other hosting controls,
durable recovery and full authorization/catalogue/prefix cutovers remain open.

## Phase 0 evidence

### Deferred compilation: settings publication capacity and HTTP repair — 2026-10-09

The current operator instruction is to leave Cargo to the other agent and
continue implementation. No Cargo command was started in this source batch.
The previously finished serial batch reported all 18 actual `service_host`
cases and isolated control-workspace strict lints passing. Its native selectors
1–7 also passed (component database nullable/concurrent creation, invalidation
failure, backlog migration, child-context migration, memory migration, settings
CAS and global Match/No-Match). Selector 8 stopped in the live WebUI test because
the response helper received a non-JSON body. Logs remain under
`/private/tmp/brassclaw-child-context-*`; subsequent selectors/lints did not run.
These are historical results for that batch, not acceptance of the changes below.

Source inspection traced selector 8 to Axum's `JsonDataError`: a value above
u32 range failed extraction before the facade and returned text/422. The Monty
settings handler now accepts extractor rejection explicitly and uses
`WebUiV2HttpError` for the stable, redacted JSON error vocabulary. Invalid typed
JSON receives 400, matching settings validation; body-limit/content-type
statuses remain intact. Extractor text and submitted values are never copied
to the wire error. The actual native WebUI regression retains strict JSON
decoding and now covers negative/fractional/null/string and overflowing values
plus private-value redaction. It must pass after the next permitted compile.

The next hosting control is `execution_limits.max_pending_settings`, default
8, positive u32. V117 materializes only missing values, advances the revision
once and preserves explicit operator values. It rejects invalid SQL values and
exhausted revisions transactionally. Ordinary startup supplies the configured
value; complete worker-acknowledged hosting publication applies its Rust-owned
capacity before publishing the shared effective revision. Legacy publication
wrappers preserve it. The fixed physical channel of eight is replaced by a
private channel protected by atomic ownership credits. Credits include the
currently executing operation and survive dropped callers, queue movement and
reductions below usage; further edits fail with explicit backpressure until
capacity is free. Shutdown rejects queued edits and releases their credits;
an already executing durable heap transaction remains owned through settlement.
Count capacity is independent of task backlog; transport frame limits remain
separate technical checks. Actual pending count, limit and retained excess are
visible in GET/status and the EN/DE WebUI.

Control processing alternates with ordinary service turns so a continuously
replenished settings inbox cannot consume every root/cancellation turn. Ready
owned publications schedule another turn without depending on coalesced Notify
permits; an idle empty inbox introduces no polling.

Required acceptance is added to the existing real worker/native tests: hold
actual durable file commits while eight accepted edits queue, drop waiters,
reduce to two without deleting accepted operations, grow to twenty beyond the
old channel ceiling, verify stale-CAS outcomes and explicit shutdown rejection,
and retain the same root identity. Native V117 checks legacy/default/explicit
values, invalid numbers/missing fields and revision exhaustion. The existing
live WebUI regression changes the value alongside child/backlog limits and
checks the actual owner. Source formatting/parsing, changed JavaScript syntax
and authored whitespace checks pass. Actual worker/native cases and affected
strict lints remain deferred; do not mark this control or the whole plan accepted.

### Running validation and newly traced retention gap — 2026-10-09

After explicit authorization to compile, the serial detached screen
`66824.brassclaw-settings-capacity` started the prepared batch. No competing
Cargo/rustc process existed at launch. Mandatory disk cleanup removed 2.1 GiB
before compilation. The isolated control workspace compiled and its 19 actual
`service_host` cases passed (zero ignored, 1.26 seconds), including the new
settings-capacity ownership/shutdown regression. Its strict all-target Clippy
also passed. Native composition compilation/acceptance is still running; these
results do not establish ordinary WebUI/V117 uptake or the full v3 plan.

The same serial batch subsequently passed all 16 selected native composition
cases, each with one actual test executed. These include V115/V116/V117 upgrades,
the real WebUI settings/HTTP rejection regression (116.36 seconds), component
database failure/concurrency cases, global Match/No-Match and the original seven
composition callers. Native logs and the exact selector script are preserved in
`/private/tmp/brassclaw-settings-capacity-native-accepted-lint-failed/`.
The following composition lint stopped on two `collapsible_if` diagnostics in
the service settings publication block. They were repaired with equivalent
short-circuit let chains, without suppression. Later host-runtime/HTTP lint
steps did not execute in that stopped batch.

### Recipe settlement capacity repair and serial follow-up — 2026-10-09

`pg_monty_admission::validate_outcome` no longer applies a second fixed
eight-Recipe limit. It still requires an array and preserves bounded typed-data,
outcome/status and actual-completion validation. Live capacity remains checked
before retaining a new distinct selection. Settlement must retain the original
execution evidence after a policy reduction; it must neither truncate the report
nor replay completed effects to satisfy a later capacity value.

The new `model_host::recipe_capacity` regression stages eight distinct pure-logic
Recipe revisions sharing one typed PythonCode component and the packaged reply
workflow. One repeatable-read catalogue capture pins all nine roots/references.
An explicit validation root uses the actual Monty dispatch/worker/accounting code
to execute these Recipes and pass each typed result to the next. The actual
owned settlement is held after publication; the selection policy is reduced
from nine to one before its PostgreSQL acknowledgement. Required assertions are
all nine completed step reports, their original checked revision, one answered
reply invocation with attempt count one, exactly one transcript reply, no model
call and the unchanged global root. This is a candidate settlement regression,
not catalogue activation or preloadable-Skill migration acceptance.

After the prior screen ended with lint status 101, detached screen
`80813.brassclaw-recipe-capacity` started the reviewed follow-up serial queue.
It builds/runs that actual regression, then executes isolated control, composition,
host-runtime and HTTP strict checks. Process inspection found no other Cargo or
rustc; mandatory cleanup removed 2.1 GiB before compiling. Logs use the new
`brassclaw-recipe-capacity-*` and `settings-capacity-composition-lints` names;
the previous native results remain intact. The follow-up subsequently passed
isolated control and composition all-target strict Clippy, the actual
component-database diagnostic regression, host-runtime strict Clippy with
`postgres,test-support`, and HTTP all-feature/all-target strict Clippy. Its screen
waited for another live Cargo process before the last two steps; it did not
start a competing execution.

The new actual nine-Recipe worker/PostgreSQL regression passed: one test ran,
zero failed/ignored, 3.54 seconds. Its log is
`/private/tmp/brassclaw-recipe-capacity-case.log`; the isolated control-workspace
all-target strict Clippy also passed. The original native capacity edit to 16
ran only reply/history (two selections), so it did not prove this requirement.
The new result establishes retained execution handoff and settlement after a
later selection-policy reduction. The subsequent expanded regression also
passed (one actual test, zero failed/ignored, 3.58 seconds) and its isolated
all-target strict Clippy passed. It corrupts copies of the actual persisted
outcome with null/object/missing Recipe arrays and false root/aggregate
completion flags, verifies classified validation rejection before any private
address lookup, and confirms the settled outcome and single reply remain
unchanged. Logs are `/private/tmp/brassclaw-recipe-capacity-negative-{case,lints}.log`;
screen `99355.brassclaw-recipe-capacity-negative` is now complete. Complete
catalogue/preload activation remains separate work. Fixed
factory/driver retention acceptance is still separate work: the new shared
credit wiring below needs real ownership and live-settings verification.

### Shared attempt retention — implementation and acceptance contract

Source wiring now replaces the separate factory/driver 256-slot checks with
one instance `execution_limits.max_retained_attempts` (default 256, positive
u32), V118 persistence and shared ownership accounting. Ordinary startup reads
the configured value; complete settings publication updates the Rust ledger
after worker acknowledgement, and WebUI shows actual retention/excess. The
driver verifies its active private credit after the history await and before
preparing ports. The corrected local acceptance batch passed on 2026-10-09:
19 actual model-host tests, 20 service tests, and 13 native composition cases,
including V118 plus V115–V117 upgrade regressions, ordinary global Match/No-Match,
live WebUI growth to 257/258, and the original seven composition regressions.
Both control and composition all-target strict Clippy passed. The service test
also reserves 257 real credits, reduces capacity to one without deleting any,
and verifies closure/refund under the original root. Actual driver/factory
acceptance verifies one shared credit, failed-effect transfer, dropped
preparation retention, and durable completion refund even with observed receipts.
Evidence is `/private/tmp/brassclaw-retention-accepted-20261009/`; settings and
migration source are V118. This is local ownership/settings acceptance, not full
platform, authored/preload catalogue or crash-recovery acceptance. The separate
application/worker binary build remains in the background. Existing local-dev
`InMemoryBackend` warnings remain visible until the persistence cutover. This is infrastructure capacity, not a Tool grant,
a task execution count, or a new Recipe. Do not implement two settings reads
that independently permit the full limit: one attempt can be held by both owners
and must consume one credit in total.

Reserve before the driver's first history/preparation await. Transfer/share the
same credit with the factory before it retains the private admission address
and begins database I/O. A factory call used directly by a behavioral validator
must reserve through that same owner. Identity is the existing private
`MontyTaskAttempt`, including its lease token; it must never be formatted,
serialized into settings/status, or handed to Python. Same-attempt sharing must
not permit replacing another host/address or bypass duplicate/replay fencing.
Keep registry locks out of async I/O. If weak references are used to deduplicate
owners, fence expiry/removal so an old destructor cannot remove a newly reserved
generation under the same key.

Normal completion may release the credit only after the actual service receipt,
reply verification and durable admission acknowledgement. Observers retaining
a completed receipt must not keep VM memory or an operational slot allocated.
Preparation failures after possible reservation commit and failed/uncertain
effects retain their address, exact selected graph, answers and credit under
a trusted reconciliation owner. Releasing the driver's map alone does not
establish release of the factory's evidence. Existing `take_settlement` and
`take_failed_settlement` seams transfer reconciliation responsibility; changing
the limit must not turn those transfers into implicit evidence destruction.
Do not delete reserved/started rows, automatically replay effects or release
unknown effects merely to regain capacity.

Publish the shared Rust-owned limit in the existing complete settings operation
after the worker acknowledgement and before the common effective revision is
visible. Validate/store/migrate via the existing Monty settings path and expose
actual retained count, limit and excess in WebUI status. A reduction below count
retains all owners and blocks only new reservations; growth permits new work
without changing the global root. Keep this ledger separate from serialized
backlog count/bytes and settings publication credits.

Required acceptance uses actual driver/factory/PostgreSQL/service ownership:
one admitted attempt held by both owners counts once; a dropped preparation
waiter with uncertain commit remains counted; completion frees capacity even
with retained public receipts; failed-effect transfer preserves evidence/count;
live reductions drain without cancellation/deletion; growth exceeds the old
256 ceiling; stale attempts and duplicate admission remain fenced. Verify
shutdown and lost-ack behavior, desired/effective revisions and unchanged root.
This contract is not implemented by the current fixed arguments.

### Next hosting control: admitted backlog — implementation contract

Expose instance-wide `execution_limits.max_queued_tasks` (default 64, positive
u32) and `max_queued_bytes` (default 64 MiB, positive native-size representation)
through the same settings/CAS/control publication. The byte limit counts the
validated serialized work envelope, including reserved protocol/identity
overhead; it is not an RSS or actual allocator measurement. Replace the fixed
bounded work channel and independent 1024 maximum with a logical count/byte
credit owner. An unbounded transport channel is permitted only behind those
atomic bounded credits; draining into a local FIFO cannot create fresh capacity.
Each accepted work item retains its credit through enqueue, queued wait and
uncertain Admit; release only after actual admission or explicit rejection.

Capacity updates happen after the worker's complete settings acknowledgement
and before the shared Rust revision/receipt publication. Lowering limits keeps
existing work and bytes, blocks new reservations while over capacity and never
forgets debt when a held credit is returned. Growing limits wakes bounded
admission waiters. Dropped waiters return only their own reservation; instance
closure wakes waiters and rejects admissions without discarding durable input.
Memory backpressure and heap reductions remain independent admission gates.
Counts/bytes and over-capacity state in GET/status come from the credit owner.
No second durable queue, input/history truncation, rematch or workflow retry.

Verify saturation by both count and serialized bytes, growth above the old
physical channel capacity, reductions with owned queued inputs, actual release
after admission/rejection, cancellation without leaks, close/wakeup races and
unchanged global root/effect identities. Extend existing native WebUI/CAS and
real service tests. Keep all Cargo checks deferred while the other agent uses
the toolchain; source implementation precedes compilation.

**Source status (2026-10-09; acceptance pending):** the neutral DTO and V116
materialize absent queue fields with one revision advance and retain explicit
values. The service now holds atomic count/serialized-envelope-byte credits
through channel, FIFO and uncertain admission ownership. Complete hosting
publication applies the acknowledged queue policy before shared Rust revision
publication; legacy settings wrappers preserve queue policy. Ordinary startup
uses configured limits; WebUI forms/status expose both actual owned usage and
retained excess. The new actual-worker regression saturates count/bytes, raises
capacity from 2 to 70, checks byte debt, cancellation, closed waiters and original
root/effect identity. Native migration and WebUI regressions cover missing and
partial legacy fields, explicit values, invalid SQL/HTTP edits and same-VM uptake.
Rustfmt parsing, changed JavaScript syntax and authored diff whitespace checks
pass. Cheap capacity preflight avoids serialization when already full; the atomic
reservation remains authoritative and envelope preparation moves the payload
instead of cloning it. No Cargo or new behavioral acceptance was run while the other agent uses
the toolchain. Await the repaired child/backlog service, native integration and
strict lint batch before claiming these controls accepted.

### rc.4 CI finding — settings-lock race

Release run `37880720548` succeeded for Linux x86_64 and macOS x86_64/aarch64.
Tests run `37881536552` reports 734 composition tests passing and one failure:
`native_webui_monty_task_settings_reach_one_running_instance` expected HTTP 400
for a 1-byte manual limit, but got 503. Raw job log retained at
`/private/tmp/brassclaw-rc4-composition-ci.log` (job `113662218348`). Inspection
shows intrinsic memory validation occurred after an immediate try-lock against
background reconciliation. The pending source validates the floor/native/i64
representation before lock acquisition and uses the existing source deadline
for an owned lock wait. Invalid input cannot become busy solely because the
controller owns the mutex; valid changes await transient reconciliation while
preserving bounded wait, CAS, retained operation ownership and worker feasibility.
The existing native WebUI regression must verify this repair at the next local
compile window. The rc.4 CI result does not accept uncommitted protocol-7 source.

### Component-cache failure propagation — pending repair

Review found `PgComponentDbBackend::mark_stale` logged database failures and
returned success. The existing component database usage must acknowledge only
committed invalidation; its Recipe owns handling the actual failure. Pending
source now propagates the existing backend error through the existing Tool
port, removes the unnecessary module-level dead-code suppression and retains
prefix bytes after rejected writes. No new workflow Tool or Rust sequencing was
introduced. `ComponentDbError` also retains its internal diagnostic while
Display/Debug and the runtime safe summary expose only classification; backend
SQL/pool text must not become a public answer or log payload.

A native PostgreSQL regression invokes the actual backend/first-party Tool,
uses an actual rejecting database trigger, checks Backend failure without public
private-row text or changed prefix bytes, then removes the restriction and
checks committed invalidation. A focused diagnostic-boundary regression covers
explicit secret-shaped text. Rustfmt and authored whitespace checks pass;
behavioral tests and affected host-runtime/composition strict lints are queued
for the later compile window, not run or accepted yet. This repairs an existing
primitive failure contract, not complete catalogue activation or prefix-v3 work.

The adjacent caller audit also found nullable legacy `content_hash` reads using
nonnull decoding, racy `is_new` from a preliminary SELECT, and optional metadata
silently ignored/defaulted when malformed. Pending source represents absent
hashes explicitly (read_hash None / read_row null), establishes creation from an
actual transactional INSERT outcome and updates only after a known conflict,
commits before returning success, and validates nonnil UUID links/string fields
and complete consumer-tag arrays before effects. Native coverage checks an
unhashed real row, parallel real creators returning one new/one existing receipt
for one UUID, last committed content/hash agreement, invalid links without
mutation and malformed Tool fields without inserted rows. It retains the current
legacy pending-row authoring behavior; this is **not** immutable authored
catalogue/override activation acceptance, which remains required by the full
cutover. No output/hash default or implicit retry was added.


### Next hosting control: retained child contexts — implementation contract

Add positive `execution_limits.max_recipe_contexts`, default 64, to the existing
neutral settings DTO, durable JSON/CAS, form and complete settings publication.
Legacy JSON without the field decodes to the existing default; explicitly
materialize it in a migration with one revision advance, retaining explicit
operator values and all other limits. This is instance-wide context capacity,
distinct from per-task Recipe selection and root worker concurrency.

The contained worker owns usage. Include actual configured capacity and active
count in every worker snapshot; transport/service observations must come from
that reply, not from the desired settings row. Publish capacity and VM bounds
in the same acknowledged settings generation. Use a new paired worker protocol
for the new required command/snapshot fields; reject a mismatched older worker.
Validate positivity/native representation before persistence and publication.
Reductions never destroy existing contexts or their results/feed/compute state;
they block only new Open requests while usage is at or above the new capacity.
Expose excess retained usage separately from acknowledgement of the new limit.
No fresh root, TaskSettings-only false acknowledgement, Recipe replay or Tier-2
fallback. CloseTask releases actual usage normally after effect settlement.

Verify real two-task retained contexts across a reduction below usage, rejected
new opens, original child resumption, growth permitting a later open, and actual
zero usage after task settlement with unchanged root identity and compute/feed
accounting. Extend ordinary WebUI/native PostgreSQL acceptance for live capacity
uptake, invalid requests and old JSON/migration preservation. Implement the
coherent source/worker/consumer change before serial local compilation.

Source implementation now supplies the field, V115 upgrade, acknowledged worker
capacity/count snapshots, live service publication and WebUI draining status.
The first actual protocol-7 worker and isolated service target compile. Sixteen
existing service cases pass; the new capacity case initially attempted a second
root context for one task and correctly received WrongBoundary after growth.
Its fixture now retains the actual parent context and opens a same-task child;
the production parent/isolation check is unchanged. The repaired queue has been
stopped at the operator's request while another agent uses Cargo. Its behavioral
rerun, native migration/WebUI/original-seven acceptance and strict lints remain
pending; do not treat the child-capacity feature as accepted yet. Resume the
serial local queue only in an available compilation window.

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

## Initial prerequisites — historical evidence, 2026-10-06

This subsection records the first implementation batch. It is not the current
runtime inventory. In particular, the v0.0.16 experiment below is historical;
ordinary production now uses the supervised global Monty service described in
the dated implementation and acceptance entries that follow.

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

Current overview, 2026-10-17: ordinary `skills-db` startup owns one supervised
global Monty service before turn workers and trigger producers. Admitted work
reaches that existing VM through `GlobalMontyDriver`; there is no per-chat lazy
driver or Rust-loop fallback. Actual native multi-turn Match/No-Match, retained
child execution, live settings and kernel/PostgreSQL callers have scoped passing
evidence in the dated entries below. The original seven composition failures are
resolved in that documented scope. Their resolution does not complete the plan.

The controlled Monty fork supplies task-owned coroutine accounting and retained
contexts. Shared heap and task executing-time accounts are separate; startup
sizing is the default and periodic pressure adjustment is optional. Configurable
limits have acknowledged Rust/worker revisions and EN/DE help where recorded in
the runtime limits audit. Do not reintroduce the historical lifetime allocation
counter, per-chat VM or pressure-based default to implement later phases.

Remaining work includes all eight concrete validator Recipes and their ordinary
Q1 ingress, full qualified preload/activation, flow/branch/effect-occurrence and
durable retry/recovery contracts, retained native implementations and immutable
override acceptance, the remaining legacy authorization/data cutover, and the
unresolved store/parser/transport and other limits in
`docs/runtime-limits-audit.md`. Full prefix work has its own plan and acceptance;
do not infer completion from the installed bootstrap workflows. Fatal VM recovery
must fence the old generation and reconcile effects before replacement.

Private native PostgreSQL acceptance covers the tested callers and migrations.
It does not establish complete backup/restore, installed upgrades, Windows/Linux
execution or test-machine acceptance. The final release, GitHub builds and fresh
remote installation/real-model QA remain required after implementation. Use the
dated receipts and exact source inventories below to distinguish accepted slices
from candidates and later concurrent edits. The whole plan remains unfinished.

## Initial verification — historical evidence, 2026-10-06

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

### 2026-10-08 — Retain exact workflow observations, including pure logic

V106 adds immutable selected-variant structural and step-execution evidence,
separate from one-Tool association records. The actual IBS selection now has
one sealed representation shared by review evidence and the durable task
journal. Behavioral observation retains execution order as well as typed
value fingerprints, so reordered steps cannot pass solely because their
individual results match. Pure-logic workflows need no fabricated Tool usage.
Missing/unsettled steps, wrong termination and uncertain transport are rejected;
failed expectations remain failed evidence. Commit recovery retains the same
review object, ID and exact bytes without replaying effects. See
[the workflow-observation contract](../reborn/contracts/workflow-review-evidence.md).

Eighteen affected native worker/kernel/PostgreSQL cases passed: 11 existing
model-host cases, five review-host cases and two retained-execution cases.
The shared authenticated human-review HTTP caller also passed with the actual
compiled companion worker. Both isolated all-target and composition skills-db
all-target strict lint passed without warnings. Checks ran serially in screen,
with disk checks/cleanup before each Cargo execution. Rust 1.98 logs:
`/private/tmp/brassclaw-workflow-review-{native,lints}.log` and
`/private/tmp/brassclaw-workflow-review-consumer-{native,lints}.log`.

Reports explicitly retain semantic_approval=false and task_completion=false;
structural checks and child results alone do not certify semantic correctness
or a published, settled root task. No activation, protected-root approval,
production catalogue or ordinary factory cutover is claimed. The original
seven runtime failures remain open; their unchanged ordinary-driver cause
was not retested for this review/journal change.

### Dropped global turn waiter and durable cancellation acknowledgement (2026-10-08)

The global-driver candidate now shares one retained acknowledgement future for
an actual service receipt across drive/stop callers. Dropping a waiter cannot
lose an in-progress acknowledgement; a failed write can retry only the same
private admission/outcome. Submitted settlement cannot be transferred before
durable acknowledgement and is transferred once. The bounded addressed stop
covers worker quiescence and database acknowledgement. Audit settlement stays
recordable after the original claim becomes terminal; every new admission,
selection and Tool dispatch still requires the current running claim.

Native PostgreSQL/actual-worker coverage holds the real recording provider call,
drops the driver waiter, observes the five-second stop deadline without false
acknowledgement, then injects a deferred commit failure. It verifies retained
host/receipt/ports, terminal cancellation after worker acknowledgement, concurrent
idempotent persistence recovery, no provider replay or late assistant reply, and
a subsequent task completed by the same global root. The affected 19 native
cases passed in `/private/tmp/brassclaw-cancel-settlement-native.log`; the refined
cancellation regression passed in `...-native-final.log`. All-target isolated
strict lint passed in `/private/tmp/brassclaw-cancel-settlement-lints-final.log`.

This repairs settlement ownership, not ordinary startup. The original seven
composition failures remain unresolved until the approved catalogue, protected
root, retained implementation/resource/continuation owners and ordinary global
factory are connected and their existing acceptance assertions pass.

### Retained first-party dispatch through the actual kernel (2026-10-08)

Host-runtime task preparation can now capture one immutable extension/handler
view and retain a selected first-party capability behind the existing kernel.
Removal or replacement affects new captures; running tasks keep the actual
handler and declaration. Each invocation still checks current stable-Tool
policy and uses the original technical services, governor and trust boundary.
The facade rejects another capability identity and exposes no raw handler.
See [the retained-dispatch contract](../reborn/contracts/retained-first-party-capability.md).

On this diff, serial screen checks passed: 19 native Monty/PostgreSQL cases in
`tests/monty_control/Cargo.toml`, isolated all-target strict lint, the actual
builtin JSON retention/live-policy regression, 14 host-runtime consumer cases,
host-runtime/extensions all-target strict lint, and 29 architecture cases.
Rust 1.98 logs are `/private/tmp/brassclaw-retained-kernel-*.log`; native tests
exercise the compiled global worker and existing V106 review/journal fixtures.
These establish candidate kernel-mediated execution ownership, not
Tool-revision/artifact/ABI approval, catalogue activation, ordinary startup,
CI or resolution of the original seven composition failures.

### Published completion survives waiter fencing (2026-10-08)

Completed admission settlement now compares the actual service reply reference
with trusted finalized-reply evidence, rather than an active task lookup. A
waiter abandoned between real root completion and database acknowledgement
therefore cannot erase the published result. Active host calls stay fenced;
withheld results still require reconciliation. The actual native regression
holds settlement after publication, drops the waiter, records the original
Completed outcome, verifies one provider request and one transcript reply, then
completes another task on the same global root.

All 13 actual model-host/worker/PostgreSQL cases and isolated all-target strict
lint passed on this diff with Rust 1.98, serially in screen. Evidence:
`/private/tmp/brassclaw-completed-settlement-{native,lints}.log`, manifest
`tests/monty_control/Cargo.toml`. An initial test compile error using Display on
an opaque reply reference was corrected to its actual string accessor before
the passing run. This candidate settlement repair does not change ordinary
runtime wiring or resolve the original seven composition failures.

### One task parent for explicitly delegated Recipe children (2026-10-08)

The global Recipe-port candidate now retains one idle parent context for its
admitted task and opens each selected Recipe beneath it. A second named workflow
no longer requests another root. Parent allocation retains the actual transport
ticket before waiting, then its receipt, so a dropped waiter resumes that request.
Monty still sequences steps and passes runtime results as typed inputs; parent
links neither share implicit state nor grant authority.

The actual constrained native regression runs two immutable draft Recipes through
Monty, IBS, child execution, kernel and PostgreSQL invocation records. It checks
hostile-looking data handoff, four actual single-attempt Tool results, one parent
allocation despite an abandoned waiter, release of the parent/two-child graph,
and refusal to close/reset a still-active root task. It supplies no product reply,
approval or activation evidence. The 13 existing model-host and five review-host
cases passed in `/private/tmp/brassclaw-task-recipe-parent-native.log`; all three
retained-execution cases passed after correcting the validator's host transport
and completion sequence in `...-native-final.log`. Isolated all-target strict
lint passed in `...-lints.log`, manifest `tests/monty_control/Cargo.toml`, Rust 1.98.
Checks ran serially in screen. Ordinary catalogue/root/resource/continuation
wiring and the original seven composition failures remain open.

### Real reply and history Tools on the global Match path (2026-10-08)

Added a task-bound first-party adapter for the existing transcript reply
primitive. It checks the exact data address and typed input, then uses the
admitted host's fenced publication path. It executes no workflow and creates no
declaration, approval or permission. The constrained native fixture now runs
two IBS-composed immutable draft workflows on the actual global worker: reply
publication followed by history formatting/writing. Their actual kernel
handlers retain live stable-UUID policy; memory uses the PostgreSQL filesystem
with its required read/write/list/delete mount permissions.

Both native cases passed: successful reply/history completion with hostile-looking
typed text, and a live history block after publication preserving that reply
without model/Tool replay. Assertions cover the actual transcript, persisted
history, service receipt, task settlement, single-attempt invocation records,
fenced retained reply handler and clean worker/instance-owner shutdown. Isolated
and Reborn all-target strict lint passed without warnings. Checks ran serially
in screen on Rust 1.98 with the documented low-debug profiles and disk checks;
manifest `tests/monty_control/Cargo.toml`, PostgreSQL filesystem feature enabled.
Logs: `/private/tmp/brassclaw-recipe-reply-history-native.log`,
`...-lints.log` and `/private/tmp/brassclaw-recipe-reply-reborn-lints.log`.

Earlier fixture failures exposed missing standalone prompt initialization,
incorrect Python newline escaping, unsuitable local filesystem metadata support,
missing delete permission and a completed-receipt ownership assertion; these
were corrected using actual interfaces, without relaxing kernel checks or
fabricating results. This does not approve/activate the drafts, certify the
protected root or wire the ordinary runtime. The original seven failures remain
open while that runtime still constructs the legacy driver.

### Verify selected workflow completion and retain task execution (2026-10-08)

The retained child runner now verifies each requested occurrence against its
supported flat stop-only order before child allocation or Tool intent. It exposes
the actual confirmed prefix separately from a failed/in-flight feed. The global
candidate refuses a root completion claim if any selected workflow is skipped,
uncomposed or incomplete, even when the reply was already published.

Settlement retains the actual service accounting and selected-workflow progress
before database acknowledgement. The bounded report is stored atomically with
the original admission outcome and reused after uncertain commit acknowledgement.
It contains selection checksums and step identities, not runtime payloads, claims
or transport handles. It explicitly supplies no semantic approval or activation.
See [the task-factory contract](../reborn/contracts/global-monty-task-factory.md).

Nineteen distinct actual native worker/kernel/PostgreSQL cases passed across the
focused runs: 13 existing model/factory/recovery cases, three reply/history cases
and three retained-child cases. The new negative draft root publishes the real
reply and runs the history formatter but skips the selected write; the actual
service rejects its completion claim, retains reconciliation evidence and leaves
admission unsettled without inventing a receipt or replaying effects. Existing
cancellation/unknown-commit recovery and successful publication checks remain.
An initial post-settlement test assertion incorrectly expected active reply access
after fencing; it was replaced by that actual invalid-root caller regression.

Isolated all-target strict lint passed without warnings. Rust 1.98, low-debug
profiles, manifest `tests/monty_control/Cargo.toml`; checks ran serially in screen
with required disk checks/cleanups. Logs: `/private/tmp/brassclaw-task-completion-native.log`
(14 passing cases and the initial failed assertion), `...-reply-final.log`,
`...-children.log` and `...-lints.log`. This operational completion report is not
the missing whole-workflow semantic/approval producer, protected-root trust or
ordinary global factory wiring. The original seven failures remain unresolved.

### Preserve admission identity and terminal settlement in PostgreSQL (2026-10-08)

V107 guards admission identity, monotonic lifecycle and immutable terminal
outcomes/timestamps. It rejects deletion/truncation and initial fabricated
completion, preserves exact repeated start checks, and permits actual queued
cancellation without creating a start time. Application private-key/outcome CAS
and worker quiescence checks remain independent. No running task, approval or
legacy component status is promoted by this migration.

Twenty-one native worker/kernel/PostgreSQL cases passed across the focused runs:
16 model/factory/recovery cases and five review consumers. New probes exercise
repeated migration against a retained reservation and rejected legacy rewrites,
terminal reopening, deletion and truncation. Existing real deferred commit
failure/cancellation recovery still passes. The pure-logic review test now checks
that reversed requests fail before child allocation and cannot produce passing
evidence; the runner's earlier order enforcement had invalidated its former
expectation that reversed steps execute. An incorrect test identifier import
was corrected to the actual TurnRunId type before execution. Isolated all-target
strict lint passed without warnings. Checks ran serially in screen with Rust
1.98, the documented low-debug profiles and required disk checks/cleanups.
Manifest: `tests/monty_control/Cargo.toml`; logs:
`/private/tmp/brassclaw-admission-immutability-native.log`,
`...-review-final.log` and `...-lints.log`. The original seven ordinary runtime
failures remain open; their unchanged legacy-driver caller was not rerun.

### Bounded queue cancellation and fatal admission evidence (2026-10-08)

The service now drains admitted transport inputs into a FIFO independently of
free VM worker slots. Owned credits preserve one configured capacity across
channel, FIFO and uncertain Admit. Cancellation removes an unissued input even
behind an uncancelled entry, returning actual task_cancelled with no VM account.
Started work still waits for its real host results and worker containment. Fatal
exit fences queued callers before those waits and retains their original inputs
and ports separately from uncertain admission and actual task evidence. Known
pre-VM backpressure rejections do not accumulate lifetime evidence.

All ten native service cases and sixteen PostgreSQL/model/factory caller cases
passed, including new occupied-worker cancellation/capacity and fatal-queue
evidence cases. Isolated all-target strict lint passed without warnings. Checks
ran serially in screen, with Rust 1.98, low-debug profiles and mandatory disk
checks/cleanups; manifest tests/monty_control/Cargo.toml. Logs:
/private/tmp/brassclaw-queued-admission-{native,model-native,lints}.log. The
ordinary runtime still needs trusted catalogue/root producers and the coordinated
global startup cutover; the original seven failures are not resolved by these
transport checks and were not rerun.

### Shared typed bootstrap usages and exact package source (2026-10-08)

The component-library crate now owns the typed reply/history draft definitions
previously local to native behavioral fixtures. They reuse the existing Tool
identities, explicit one-usage Skills, associated PythonCode and single-component
Recipe steps. Skill prose follows the authoring sections; each variant has ten
ordered input examples. The named history helper is not an incoming intent route.
No Tool, approval, catalogue activation or fallback is created by constructing
these drafts. Composition facade APIs still expose no lower-store handles.

Source verification compares the actual retained immutable documents against
the packaged definitions, including dependencies, association and input layout.
Tool implementation/ABI verification and approval are separate. The new real
PostgreSQL case verifies every reply template's typed captures, stages a changed
code revision, rejects inherited package integrity and reads the unchanged original.
All four focused native cases passed, including actual reply/history, live policy
block and invalid root completion. Affected Skills and isolated consumer all-target
strict lint passed without warnings. Manifest tests/monty_control/Cargo.toml, Rust
1.98, serial screen checks, low-debug profiles and mandatory disk checks/cleanups.
Logs /private/tmp/brassclaw-packaged-usage-{native,skills-lints,consumer-lints}.log.
This supplies canonical source for the remaining bootstrap producer; it does not
wire ordinary global startup or resolve the original seven failures.


### 2026-10-08 — No-Match publication through the retained reply Recipe

The candidate global Python root now returns typed model content from its
No-Match loop and requests the retained named `host-post-reply` Recipe. IBS
composition and Monty child execution publish through the actual kernel-mediated
Tool and durable invocation journal. `GlobalRecipePorts` rejects raw root
`post_reply` transport calls, preventing a root from skipping that dispatch path.
Matching/composition/policy failures never become No-Match or replay the model.

The two former direct-transcript native fixtures now use the actual global
instance owner, task factory, retained draft Recipe and reply kernel. Their
original prompt/history, opaque conversation, provider replay and mounted Tool
assertions remain. A new real PostgreSQL/worker case blocks the live reply Tool
after the model answers: no assistant message is written, exactly one invocation
is recorded as an authorization failure, actual child/accounting evidence is
settled, and ownership is released after actual worker shutdown.

Serial screen validation with Rust 1.98 and the isolated Monty control manifest:
11 global-root cases and 17 existing model-host cases passed in
`/private/tmp/brassclaw-no-match-reply-native.log`. The initial new assertion
expected dispatcher policy-denial rather than the kernel's authorization category;
its original diagnostic remains in that log. After tracing the real mapping,
the corrected case passed in `...-no-match-reply-policy-final.log`; strict
all-target consumer linting passed in `...-no-match-reply-consumer-lints.log`.

These are actual candidate-path observations using unapproved validation drafts.
They do not establish controlled bootstrap approvals, active catalogue publication
or ordinary runtime cutover. The original seven remain unresolved because the
ordinary factory still constructs the legacy per-chat driver.


### 2026-10-08 — Actual root code and generation in execution evidence

The contained interpreter issues `RootExecutionIdentity` from its actually
compiled, checksum-verified source and registered port names. It retains the
VM generation and worker count. Worker protocol 5 carries the identity in every
snapshot; startup verifies requested source/bindings/workers and continuation
ownership, and all later transport replies retain that same identity. A conflicting
reply is a contained protocol failure retaining its actual snapshot and command.
No task/model JSON can change the running root. Companion host/worker upgrades
require the same protocol; no old-wire or interpreter-state compatibility is claimed.

Actual task receipts, pre-admission cancellation receipts and instance exit retain
that identity. Durable task reports bind the original outcome and selected Recipe
progress to it. Cancellation before admission still has no task accounting; its
root identity identifies the instance queue, not fictitious task execution. These
observations establish no protected-root approval or Tool implementation artifact.

All **131** isolated Monty-control acceptance cases passed serially in screen in
`/private/tmp/brassclaw-root-identity-native-complete.log`, with Rust 1.98, native
PostgreSQL, actual companion worker and existing constrained provider fixtures.
Strict affected Host and all-target consumer lint checks passed in
`...-root-identity-host-lints.log` and `...-root-identity-consumer-lints.log`.
The first compilation caught an external test reaching a private continuation-key
field; that test access was removed without exposing the key. The subsequent
full run exposed an older revision-store assertion: new review foreign keys make
plain TRUNCATE return PostgreSQL 0A000 before the trigger. Exact separate expected
codes now cover UPDATE/DELETE, TRUNCATE and TRUNCATE CASCADE; the last exercises
the immutable trigger. Old and new exact revisions remain readable after every
rejected operation. Both initial diagnostics remain in the earlier
`...-root-identity-native.log` and `...-root-identity-native-final.log`.

The ordinary factory still constructs PersistentMontyDriver. Controlled seed
provenance/review, Tool artifact/ABI retention, coherent catalogue activation and
ordinary global startup/live-resource/continuation cutover remain required; no
original-seven success or CI acceptance is claimed.

### 2026-10-08 — Immutable global-root definition and actual worker comparison

Added the exact class-10 `monty-global-orchestrator/1` document contract in the
existing immutable revision store. Source, source checksum, compiler inputs and
host-port names are retained together. Strict parsing rejects changed inputs,
unexpected fields, invalid/duplicate ports and source-integrity failures. This
definition is separate from Skill associations and protected-root approval.

The actual native owner/driver case now starts the worker from source read from
the exact PostgreSQL revision. Two opaque admitted tasks use the same root;
staging a successor does not change it. The receipt's actual source/port identity
matches the original definition and rejects both the successor and changed ports.
The focused case passed in `/private/tmp/brassclaw-root-definition-native.log`;
Skills, Monty host and isolated consumer strict all-target lint checks passed in
the corresponding `root-definition-*-lints.log` files. Rust 1.98, native PostgreSQL,
isolated manifest `tests/monty_control/Cargo.toml`, serial screen execution and
the established debug-free target profile were used.

This is draft-definition/runtime-integrity evidence. It supplies no semantic Q1,
controlled seed approval, Tool artifact attestation or catalogue activation.
Ordinary runtime still constructs the legacy driver; the seven failures remain
open pending the coordinated production cutover.

### 2026-10-08 — Retain the packaged root in ordinary shared component boot

The shared startup path with `postgres`/`skills-db` now retains the exact
packaged class-10 global-root draft before workers. The new stable UUID is
separate from the legacy root. `PgComponentRevisionStore::retain_packaged_draft`
uses the existing allocation lock and exact byte comparison for concurrent
boot and unknown-commit retry. Reusing the original package preserves newer
authored revisions and the allocation counter; changed package bytes append a
draft without approval, activation or changes to operator selections. The
root's port manifest excludes the rejected raw `post_reply` root call.

The actual shared startup regression passed, including repeated boot and
changed-prompt rejection (`...-root-boot-retention-native.log`). The real worker/
PostgreSQL/owned-driver regression passed with eight concurrent retentions,
an authored successor, repeated package upgrade, rejected class collision and
two opaque tasks on the original root (`...-root-boot-retention-worker.log`).
Affected Composition library, Skills all-target and isolated consumer strict
lints passed in the corresponding `root-boot-retention-*-lints.log` files.
Commands ran serially in screen on Rust 1.98 with the established NVMe target
and debug-free profile. Ordinary startup was tested from the root workspace;
worker behavior used `tests/monty_control/Cargo.toml`.

This connects draft retention to an ordinary startup caller. It does not start
the global service there or supply protected-root/controlled-seed approvals,
active catalogue publication, Tool artifact identity or the resource/continuation
cutover. The original seven ordinary message-flow failures remain unresolved.

### 2026-10-08 — Remove fabricated bootstrap graduation and surface recovery errors

Removed the builtin/Zencoder seeders' unconditional Q1 pass and builtin approval
writes, including the private builtin graduation API. Seeding now retains a
pending legacy review request and propagates failures. Repeated boot repairs a
missing request after a component insert but preserves existing review identity,
state, counters, feedback and upgrade payloads. Skill-table records retain their
actual class; protected class-10/50 records are no longer submitted as class 1.
The Action seeder releases its database checkout before retaining the request.
Boot integrity recovery now propagates request-write errors instead of reporting
success after logging them, and counts only requests it actually created.

Real native PostgreSQL startup tests covered queue-write failure after legacy
insertion, successful recovery, absence of fabricated Q1/Q2/graduation records,
builtin and Zencoder requests, correct root class, retained operator feedback,
orphan recovery failure and idempotent recovery. All three startup cases passed
in `/private/tmp/brassclaw-bootstrap-review-native-final.log`; all twenty native
review/immutable-receipt/human-Q2 cases passed in
`...-bootstrap-review-queue-native.log`. Affected Composition `skills-db`
all-target strict linting passed in `...-bootstrap-review-composition-lints.log`.
Root workspace, Rust 1.98, native PostgreSQL, established debug-free NVMe profile
and serial screen execution were used. The first compiler diagnostic is retained
in `...-bootstrap-review-native.log`: the pooled transaction needed explicit
dereferencing for the generic tokio-postgres client API; it was corrected without
changing the transaction boundary.

Historical component labels and receipts remain legacy data; no new trusted seed
or active catalogue is inferred from them. These pending requests do not execute
the eight validator Recipes or implement the immutable controlled system-seed
evidence adapter. Ordinary runtime still constructs the legacy driver, so no
original-seven success or global production acceptance is claimed.

### 2026-10-08 — Retain the exact global-root review subject

Shared component startup now attaches an immutable class-10 review submission to
the already retained packaged root. `submit_retained` verifies the actual stored
candidate, predecessor and complete selected graph without allocating another
revision or moving its head. Exact retries retain their original subject even
after newer drafts exist; conflicting actors and absent candidates fail.
`read_revision` resolves an explicitly named historical revision for authoring,
not active/latest task selection. The package actor is an origin label, not
system-seed approval. Source retention followed by a lost review commit is
repaired by identical startup retry before workers are enabled.

Evidence: root workspace Composition `--features skills-db --lib` ran three
real native shared-boot cases and, under the corrected
`pg_review_submission::tests::native_` filter, three actual PostgreSQL/operator
submission cases. The earlier `pg_review_submission_tests::` filter selected zero
tests and is not passing evidence. Composition all-target strict Clippy passed.
The isolated `tests/monty_control/Cargo.toml` actual worker/database handoff case
passed, including successor/predecessor retention, unchanged live root, review
retry, actor conflict and missing-candidate rollback; Skills and isolated
consumer all-target strict Clippy passed. All used Rust 1.98, the established
shared target/profile and serial background screen execution. Logs:
`/private/tmp/brassclaw-root-review-{subject-native,authored-corrected,subject-composition-lints,subject-worker,subject-skills-lints,subject-consumer-lints}.log`.

These are draft/review-subject and candidate-worker results, not Q1/semantic
approval, controlled system-seed provenance, active catalogue publication or
ordinary global-runtime acceptance. The original seven ordinary composition
failures remain unresolved; their caller is unchanged.

### 2026-10-08 — Running native executable identity and seed approval scope

The installation bootstrap does not depend on external maintainer or human Q2
approval for bundled code/components. The operator clarified this distinction:
development review, automated checks and package/artifact integrity qualify
installation seeds; authored additions/edits on an installed instance use Q2.
AGENTS.md, CLAUDE.md and validator_v3 step 11 now state this explicitly. Missing
qualification/activation machinery remains implementation work.

HostRuntime's new `NativeExecutableImage` owns a private, size-bounded streaming
copy and SHA-256 of the executable actually running in the host. Linux opens
`/proc/self/exe`; its pathname is not reread through `current_exe`. macOS obtains
the running SecCode, validates the retained signed artifact, and requires that
its exact CDHash match the actual running code. The system codesign display call
has bounded output, a deadline and acknowledged termination on failure. An
ad-hoc signature establishes identity, not developer/installation provenance.
The actual [Apple dynamic validity contract](https://developer.apple.com/documentation/security/seccodecheckvalidity(_:_:_:))
and [Linux proc interface](https://www.kernel.org/doc/html/latest/filesystems/proc.html)
were consulted. No custom native parser, unsafe code or external signer approval
is introduced. Artifact paths and OS diagnostics stay private; a retained file
descriptor supports checksum verification after source-path changes.

Evidence on this macOS host: three actual HostRuntime OS regressions passed,
including retained-file corruption, capacity rejection and another signed
executable failing the current-process identity requirement. Root HostRuntime
and isolated Monty consumer all-target strict Clippy passed, with their actual
Cargo manifests and updated lockfiles. Rust 1.98, established shared target/
profile, serial background screen; logs `/private/tmp/brassclaw-native-image-{os,host-lints,consumer-lints}.log`.
Linux execution, other platforms and universal Mach-O acceptance are unverified;
unsupported/unverifiable images fail explicitly.

This artifact primitive is not yet attached to exact Tool metadata/adapter
registrations. It does not cover dynamically loaded code, persist artifacts
across restart, certify semantics/approval or activate the ordinary global
runtime. Required handler/ABI association and drain/reconciliation across
incompatible upgrades remain. Original seven ordinary caller failures remain
unresolved.

### 2026-10-08 — Bind native builtin handlers to exact Tool selections

The concrete builtin constructor now records the verified running executable
artifact together with each actual capability declaration, resolved input schema
and in-process `FirstPartyCapabilityHandler/1` adapter contract. Immutable Tool
metadata can name that exact identity; retained kernel selection compares it
against the actual registration. Deserializing an expected identity cannot issue
a registration. Generic handler replacement removes the new entry's native
qualification, while already selected handlers retain their original artifact
and contract. Current instance Tool policy remains checked at invocation.

Evidence: the isolated `tests/monty_control/Cargo.toml` all-target native matrix
passed 131 actual cases, including real Monty/PostgreSQL/kernel selection,
wrong adapter/artifact rejection, registration replacement, live-policy denial
and failures after completed effects. Root HostRuntime and isolated consumer
all-target strict Clippy also passed. Rust 1.98, established shared target/profile,
serial background screen; logs
`/private/tmp/brassclaw-native-binding-{native,host-lints,consumer-lints}.log`.

This covers the concrete linked base builtin registry, not arbitrary callbacks,
task-specific reply adapters or dynamically loaded code. It is not component or
combination approval, catalogue activation or cross-binary ABI loading. Bundled
installation seeds need no additional external maintainer/human-Q2 approval;
their controlled qualification and ordinary global-runtime wiring remain
implementation work. The original seven ordinary failures remain unresolved.


### 2026-10-08 — Original seven composition regressions resolved

Ordinary `skills-db` runtime startup now starts and retains the supervised global
Monty service after verified component boot and before turn workers/trigger
producers. The admitted task carries its opaque conversation/message/run/attempt
address into the existing service. The lazy per-conversation driver, session
registry and its orchestrator-code adapter have been removed; there is no Rust
agent-loop fallback. Python sequences matching, IBS composition, execution and
reply/history workflows.

The installation-owned catalogue retains exact packaged typed reply/history
components, their complete dependency references, inspected executable programs
and actual native Tool/adapter identities. Its immutable database generation and
component references are verified in a consistent task-start snapshot. Matching
and IBS use the retained selection; child execution does not read latest again.
Runtime data stays separate from Python source. Packaged installation components
need no external maintainer/human-Q2 approval; post-install authored components
still use their distinct approval/activation path. The knowledge prefix is derived
from this limited packaged catalogue, not the full component library.

The real captured kernel handles task-bound replies and retained memory dispatch.
Current instance Tool rules are revisioned and checked before dispatch, including
blocked selected workflows. Durable invocation intent precedes effects; late
outcomes and failed receipts remain retained without effect replay. Owner-aware
turn scope fixes accepted-transcript lookup for ordinary facade callers. Internal
reply readback also preserves its actual conversation scope, and trusted internal
No-Match cannot call a model. The shared native review fixture now reuses the
production admission module instead of loading duplicate copies.

Evidence for the final relevant diff, Rust 1.98 on macOS, serial background screen
execution with `/Users/ollama/brassclaw-target`, incremental/debug info disabled:

- Root `cargo test -p brassclaw_reborn_composition --features skills-db --lib
  --no-run --message-format=json` built the actual test executable. Each of the
  original seven unchanged tests passed individually, with their existing
  assertions and deadlines: default prompt injection, edited default prompt,
  model Tool dispatch, workspace mapping, WebUI facade reuse, disabled model
  budget gating and completed assistant text.
- `native_global_runtime_retains_one_root_across_match_and_no_match` passed:
  one actual root identity across successive unmatched/matched/unmatched turns,
  typed quotes/Unicode/marker input, both composed reply/history workflows,
  eligible prior reply in the next prompt, live Tool block without publication or
  Tier-2 replay, original failed-attempt receipt retention/replacement rejection,
  and internal No-Match without a model call.
- Root composition all-target strict Clippy with `skills-db` passed.
- Isolated `tests/monty_control/Cargo.toml` all-target matrix passed 131 real tests,
  zero failures/ignored; its all-target strict Clippy passed. This includes
  worker containment, opaque task handoff, cancellation acknowledgement,
  admission/attempt fencing, actual kernel policy and retained effect outcomes.
- Root HostRuntime and Reborn all-target strict Clippy passed with
  `brassclaw_host_runtime/postgres,brassclaw_reborn/root-llm-provider`. Formatting
  and `git diff --check` passed. No warning suppression was added.

Logs: `/private/tmp/brassclaw-global-runtime-{worker,original-seven,case-1,case-2,
case-3,case-4,case-5,case-6,case-7,case-8,composition-lints,native,native-lints,
host-lints}.log`. Each case log records one selected test, not a zero-test filter.
Earlier failure entries above remain historical evidence and are superseded for
these seven regressions by this passing batch.

This resolves the original seven failures; it does not complete the v3 plan.
Authored catalogue activation, complete Kohai library-prefix compilation, adaptive
heap settings, live WebUI duration/budget publication, the remaining legacy
authorization cutover and complete fatal-effect reconciliation still need their
existing implementation and production acceptance gates. The current explicit
heap setting is retained during that migration. CI, Linux execution and the full
workspace acceptance matrix are unverified by this local evidence.


### 2026-10-08 — Live instance task settings reach Rust and global Monty

The ordinary `skills-db` runtime now owns one durable Monty task-settings
publisher. WebUI bundles reuse that owner through the existing settings port;
creating a bundle or abandoning an HTTP waiter does not start/stop publication.
Startup passes the exact boot settings snapshot into the owner before workers
start. Duration and token-mode changes publish through the existing service
control lane: the actual worker acknowledges the limits before Rust publishes
the same effective revision. Publication preserves existing task consumption.
The settings row used by ordinary global startup is also the instance-wide
WebUI row; callers cannot select another scope through these endpoints.

Persisted desired edits and actual effective revisions are returned separately.
CAS rejects stale edits; bounded persistence/publication failures remain explicit.
The owner retries durable desired state independently of the browser and joins
on shutdown. Retrieval receives the complete acknowledged settings snapshot;
history/prompt assembly reads the actual shared token-mode watch. Persistence-only
stores never fabricate runtime liveness. Restart is advertised only when supported.
Heap/orchestrator edits are rejected until their separate live publication paths
exist; this batch does not claim uptake of the entire settings row.

The frontend displays desired/effective task revisions and pending/failed uptake,
uses serial status requests, and retains settings when status is unavailable.
All eleven language packs contain the new uptake labels. Preexisting invalid
English/German translation entries were moved into their registration objects.

The full architecture matrix exposed an existing guard misclassification of the
native executable retention primitive. The guard now recognizes only its three
inspected OS-image/private-artifact open sites in their owning functions, rather
than exempting the module. A negative regression retains rejection of config
reads, other read APIs and unrelated open sites. Application state still belongs
in PostgreSQL; executable retention is not a settings persistence mechanism.

Evidence for this relevant diff on macOS/Rust 1.98, serial background `screen`,
`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target`, incremental/debug info disabled:

- Actual `monty_worker` binary build and composition `skills-db` library test
  executable build passed. The unchanged original seven regressions passed again.
- `native_global_runtime_retains_one_root_across_match_and_no_match` passed.
- `native_webui_monty_task_settings_reach_one_running_instance` passed through
  ordinary runtime construction, real PostgreSQL and the real WebUI router:
  live edits during a controlled model wait, both token-mode directions, shared
  instance settings, stale CAS rejection, unattended durable reconciliation,
  actual settled task accounting revision, retained root identity, shutdown
  observation and restart with the persisted revision. Its controlled gateway
  establishes infrastructure acceptance, not external inference-provider proof.
- Strict all-target Clippy passed for composition/product workflow/Reborn/WebUI
  with `brassclaw_reborn_composition/skills-db`.
- Isolated `tests/monty_control/Cargo.toml` all-target tests passed: 131 tests,
  zero failures/ignored. Its strict all-target Clippy passed.
- Product `inbound_turn_contract` and `reborn_services_contract`: 123 passed;
  WebUI `webui_v2_handlers_contract`: 58 passed.
- Architecture all-target checks: 30 passed; strict all-target Clippy passed.
  The initial library-only architecture command ran zero tests and is not
  acceptance evidence. The first actual matrix failure and its corrective
  boundary change remain recorded in the validation results.
- Changed frontend and all eleven packs passed `node --check`; formatting and
  `git diff --check` passed. No warnings were suppressed.

Logs: `/private/tmp/brassclaw-live-settings-{worker,original-seven,case-1..case-9,
composition-lints,native,native-lints,product-contracts,http-contracts,
architecture-all-targets,architecture-lints}.log`.

**Remaining implementation:** adaptive pressure sampling and heap publication;
complete disabled-token propagation (the model accountant still has a startup
construction/omission switch); complete authored immutable catalogue/association
activation and override rules; remaining instance authorization/policy cutover;
complete fatal-effect reconciliation and full-library Kohai prefix compilation.
For the accountant follow-up, retain usage observability and outstanding
reservations across live edits, including isolated model work and cancellation;
do not simply bypass all resource/cost accounting or drop reservations when the
mode changes. Full workspace/CI/Linux and external model acceptance are unverified.

### 2026-10-09 — Portable adaptive-memory implementation constraints

The operator clarified that Linux and Windows are required alongside macOS.
Use one shared budget policy with platform-specific observations of available
capacity, pressure and applicable process/container limits. A macOS-only probe
or a Windows compilation result cannot establish portable runtime acceptance.
The current native executable retention path rejects Windows; ordinary installed
catalogue startup calls that path. Windows runtime acceptance therefore also
requires a verified executable-retention implementation and actual startup tests.

The 70 MiB worker regression deliberately exceeds an artificial 64 MiB initial
physical limit. Neither value is a production memory default or a reason to
introduce page-level reclamation. Limit publication uses scalar accounting and
limit updates, without scanning or moving heap objects. Release completed task
contexts normally; under pressure, reclaim only provably disposable contexts or
whole cache entries in bounded batches. Keep live Recipe state, waits and retained
task selections intact. The operating system manages physical pages.

Choose measurement cadence, reserve and growth/reduction hysteresis from the
required performance baseline. Avoid per-task OS surveys, repeated publications
of unchanged limits and fine-grained page management. Measure sampling and
allocation-accounting overhead separately; no performance improvement or lack
of regression is established by the current functional tests.

Production wiring must stage calculated limits until actual worker acknowledgement,
keep automatic calculation revisions separate from durable settings revisions,
and apply memory backpressure both at submission and before queued VM admission.
The current driver converts submission errors into task admission failures;
persistent waiting under memory pressure needs coordinated durable admission
handling. These remain implementation requirements, not completed acceptance.
This batch completes the task-settings bridge, not the full v3 cutover.
## 2026-10-09 — Completed-task cleanup and hosting-limit audit

The operator requires every configurable Monty resource cap to be removed or
managed through the existing instance WebUI settings. Fixed startup allocator
limits are not a second independent operator policy: the finite allocator
backstop must track the acknowledged logical heap plus explicitly configured
transport/non-VM headroom. Disabling token budgets does not disable that physical
safety boundary or change provider context capabilities.

Normal completion is the cleanup boundary after Monty accepts the returned
values, reply/history persistence is verified and outstanding host calls have
settled. The existing root clears its task-local variables before its next work
wait. `CloseTask` removes its child contexts and task account. The service must
publish that command's post-release snapshot before acknowledging completion,
preserving any other unconsumed root boundary. Retaining a completed receipt
must not retain its child interpreter. Parent Recipes retain only data they still
need; failed or uncertain effects retain their required reconciliation evidence.
Neither releasing allocations nor reducing a logical budget requires scanning,
compacting or reclaiming individual pages. Physical RSS may remain above live
allocation bytes because the platform allocator can retain reusable storage.

The following inventory is implementation work, not an acceptance claim. Defaults
are current source values, not binding target defaults. A settings row or UI
control alone does not establish runtime uptake.

| Resource | Current enforcing path/default | Required disposition |
| --- | --- | --- |
| Logical shared heap | `global_monty_startup` uses acknowledged finite limits; V111 defaults to startup sizing with a 512 MiB ceiling/fallback | Startup/manual are constant without periodic pressure reads; automatic is optional. See the accepted memory-mode entry above; remaining allocator/transport controls are separate. |
| Physical worker allocator | Acknowledged heap + two 64 MiB frames + adjustable `worker_adapter_reserve_bytes` (default 4 MiB, explicit zero supported); real WebUI/native receipt tests pass | Heap/reserve edits resize the backstop without resetting task usage or heap revisions. Status shows the budget above allocator baseline and non-VM reserve. Live frame resizing and its retained-exchange coordination remain open. |
| Source and accumulated compiled source | `VmBounds`: 1 MiB / 2 MiB | Settings propagated to root, child constructor, retained artifact inspection and every subsequent feed; already pinned sources remain retained. |
| Feeds per Recipe context | `VmBounds.max_feeds = 128` | Live limit; preserve consumed feed count, reject new feeds after reduction rather than resetting it. |
| Captured stdout | `VmBounds.max_stdout_bytes = 1 MiB` | Live bounded capture with explicit overflow; no silently discarded output or unrelated task failure. |
| Typed value depth/nodes/bytes | `VmBounds`: depth 48, nodes 1 Mi, bytes 64 MiB | One revision at JSON preflight, actor serialization, root conversion and child conversion. Existing retained values are not truncated on reduction. |
| IPC frame | `ProcessLimits.max_frame_bytes` is the sole finite transport policy; unsigned 32-bit wire length/native allocation size are checked, with no duplicate 64 MiB ceiling | The 65 MiB real-worker request/reply regressions and strict host lint pass locally; see the representation repair below. Production boot still supplies 64 MiB. Finish live WebUI frame/reserve publication and revisioned actor/allocator coordination before claiming the control complete. |
| IPC nesting | `MAX_TRANSPORT_DEPTH = 64`, receive decoder recursion bound | Configured common bound including protocol wrappers; explicitly validate against decoder/stack capabilities or implement a tested iterative codec before supporting greater depth. |
| Active root task slots / pending calls | `GlobalBounds`: 2 workers, 128 pending calls | Adjust admission and actual Monty coroutine capacity together. Do not report a larger Rust semaphore as a larger VM pool. Reductions drain existing work; no per-chat VM or root restart. |
| Child contexts | V115/settings `max_recipe_contexts`, default 64; actual worker snapshot owns count/capacity | Actual service, migration, native live WebUI and affected strict-lint checks passed locally. Reductions retain existing contexts. Full platform/Phase 3a acceptance remains separate; see Phase 0 evidence above. |
| Admitted work backlog | V116/settings `max_queued_tasks`/`max_queued_bytes`, defaults 64 / 64 MiB; atomic logical credits replace fixed channel/1024 ceiling | Actual service saturation, reductions/growth, cancellation, native migration/live WebUI and affected strict-lint checks passed locally. Serialized bytes are not allocator/RSS measurements; full platform acceptance remains separate. |
| Actor ordinary/control lanes | V119/settings `max_actor_requests`, `max_actor_reserved_bytes`, `max_actor_control_requests`, `max_actor_control_reserved_bytes`; defaults 8 / 1 GiB per lane | Shared atomic credits cover queued, executing and completed/uncollected requests. Physical FIFO size no longer limits live growth. Reductions retain debt and separate control capacity; control bytes must fit one full request/response. 39 actual-worker/DB/HTTP cases pass locally, including V119/V120 and live WebUI uptake. Strict host lint passes; the preload blocker is resolved, with strict production composition lint accepted for the recorded preload-repair snapshot below. Full platform/cutover acceptance remains separate. |
| Settings publication lane | V117/settings `max_pending_settings`, default 8; logical credits include executing edits | Actual worker ownership/shutdown/growth beyond eight, migration/live WebUI and affected strict-lint checks passed locally. The same effective publication revision covers this Rust-owned policy; full platform acceptance remains separate. |
| Retained admissions/attempts | V118/settings `max_retained_attempts`, default 256; factory and driver share one private credit per attempt | Local real worker/ownership/migration/WebUI acceptance and strict lint passed. Completion refunds only after durable acknowledgement; unresolved evidence and transfers retain the credit. Complete platform/recovery acceptance remains separate. |
| Recipes retained per task | V113/settings `max_recipes_per_task`, default 8; both selection paths read the acknowledged revision | Live WebUI/CAS and native task acceptance passed. The redundant settlement ceiling is removed; the actual nine-Recipe regression passed after reducing selection capacity, with malformed-outcome rejection and no reply replay. Complete catalogue/preload activation remains separate. |
| Hosting deadlines and VM slice | V120/settings `startup_timeout_millis` / `response_timeout_millis`, defaults 30 s; slice 5 ms; V121 settings source/uptake defaults 2/5 s | Startup/IPC settings and retained exchange deadlines have local actual-worker, V120 migration/global-startup/live-WebUI acceptance and strict host lint. V121 source/read and uptake controls passed native migration, captured-deadline/revision-guard and live HTTP acceptance locally; see the 2026-10-09 evidence below. The preload lint blocker is resolved; strict production composition lint passed for the recorded repair snapshot below. Independent singleton ownership/control cadence and other hosting bounds remain to expose. Task compute time remains distinct from IPC, startup, idle and external waits. |
| Pending WebUI status refresh | V130/`status_poll_interval_millis`, default 3000 ms; saved/acknowledged policy replaces the fixed browser timer | Backend DTO, fourteen native migration cases and two real WebUI/global-runtime settings callers pass. Actual Chrome/native-CLI acceptance also verifies saved and acknowledged timer changes, one outstanding status request, stale-response rejection, unsaved-form retention, navigation cancellation and invalid-input rejection. Evidence: `/private/tmp/brassclaw-browser-status-ui-accepted-20261009`. Restart, platform and full-plan acceptance remain separate. |
| Prepared flow limits | Root `_validate_flow*` uses live typed-value/memory/compute controls and each Recipe's explicit finite iteration bounds; separate depth/path/identifier/count maxima are removed | Six real-interpreter controls and ordinary native global startup passed locally; strict flow and production composition lint pass for the recorded snapshot. This does not activate looped Tool workflows before durable occurrence/continuation support and coherent approval are complete. See the accepted flow-capacity repair below. |

Reuse `MontyVmSettingsStore`, the existing PUT/GET/status routes and settings
owner. Extend their validated snapshot instead of adding a second admin service.
The complete settings generation must reach service admission, actor credits,
worker allocator, root conversion/tracker and existing child contexts before
claiming it applied. Keep desired policy revision separate from automatic heap
calculation/publication revision. Safe reductions never delete active contexts,
reservations or pending effect evidence; usage above the requested capacity is
visible as pending with admission backpressure. Validate checked arithmetic and
cross-field constraints before persistence, then recheck actual usage at the
serialized worker boundary. A manual heap reduction rejected by that boundary
must not be presented as an applied desired setting. Commit uncertainty requires
readback/reconciliation, not a blind second write or rollback that enables work
with inconsistent limits. All these owners must use the same publication path;
an HTTP caller dropping is not publication cancellation.

Production acceptance must exercise normal completion, retained receipts,
parent/child result lifetime, safe/unsafe reductions during running and suspended
tasks, saturation of ordinary versus control lanes, abandoned update waiters,
unknown DB/worker acknowledgement outcomes and recovery. Verify actual heap
refund and unchanged root identity, counts and computation history. Platform
memory measurements and startup acceptance are required on Linux, macOS and
Windows; OS-specific runtime evidence cannot be replaced with compilation alone.
### Validation of the cleanup/acknowledgement repair

On 2026-10-09, the serial `brassclaw-task-memory-tests` screen queue passed:
`cargo test --manifest-path tests/monty_control/Cargo.toml --test service_host
--locked -- --test-threads=1` (11 passed, no failures/ignored tests), and
`cargo test -p brassclaw_resources --lib monty_heap_budget --locked` (7 passed).
Strict all-target Clippy passed separately for the isolated native workspace
and root resource package. Rust 1.98, incremental disabled, dev/test debug info
disabled, target `/Users/ollama/brassclaw-target`; disk guard ran before each
Cargo command. Logs are `/private/tmp/brassclaw-task-memory-{native,resources,
native-lints,resource-lints}.log`.

The actual native service test reads a real file through a child interpreter,
retains a 2 MiB child allocation, writes/verifies the actual reply, finishes
normally, observes that allocation refunded before the completion receipt, and
executes a second task under the identical root identity while retaining the
first ticket/receipt. This establishes worker allocation cleanup and fresh
service observation; it does not measure OS RSS reclamation or complete the
production WebUI/adaptive memory/platform acceptance. The deterministic budget
tests establish that repeated proposals, rejected acknowledgements and safe
manual ceilings do not fabricate a changed effective allocator budget.

### 2026-10-09 — Live execution-limit publication

The existing instance Monty settings path now includes an `execution_limits`
group: source bytes per feed, cumulative compiled bytes, feed count, captured
output bytes, execution slice, and typed-value depth/node/byte limits. V109 adds
this group with the previous startup defaults and advances existing settings
revisions without modifying historical migrations or operator values. Stored
invalid JSON is an error, rather than a replacement default.

A combined serialized worker operation updates task settings and these bounds
at one VM boundary. Existing root and child contexts keep their locals, selected
artifacts, continuations, feed/compiled-byte consumption and compute accounts.
The actor publishes the actual acknowledged bounds before exposing the receipt;
service admission and queued admission use those observations. Private worker
protocol 6 carries the actual bounds with every snapshot; the root identity is
still immutable for that worker lifetime.

Queued inputs are rechecked before VM entry. A newly oversized input is rejected
explicitly without trimming history or ending the instance. A completed host
result exceeding the new transport/value bounds is retained verbatim in trusted
withheld evidence; its task aborts without replaying the effect or killing the
root. Values must fit the current transport, native representation and response
settings; source reductions cannot make the retained root unbootable.

The existing WebUI tab submits the group with its expected revision and displays
its own desired/effective uptake. Numeric edits reject values that JavaScript
cannot represent exactly. This is one execution-limit family, not completed
adaptive-memory, transport/concurrency or full-plan acceptance.

Local Rust 1.98 validation passed with the stable NVMe target, incremental and
debug data disabled: the protocol-6 worker build; all 14 real service tests in
`tests/monty_control`, including retained child state, queued-input rechecks and
oversized completed-result containment; the actual native PostgreSQL/WebUI
settings caller; PostgreSQL settings tests; and the retained global root across
Match and No-Match. Strict all-target control-harness Clippy and affected
composition/WebUI/host-API/product Clippy passed, as did all 26 architecture
checks. Two earlier native runs exposed a wrong shutdown helper and an incorrect
task-error route; both were corrected before the passing service/caller checks.
Logs are recorded as `execution-*` in the local validation receipts. No
performance or Windows runtime acceptance is claimed by this entry.

The paired application/CLI version is `1.7.0-rc.1`; the release workflow builds
locked Linux/macOS application and worker artifacts and labels tagged
prereleases accordingly. Builds on the separate test machine were not used.

### 2026-10-09 — Exact budget-gate persistence

Review of the existing resource persistence path found that V019's
`NUMERIC(18,6)` projection rounded amounts independently of the exact payload.
V110 removes that projection precision and restores only amounts whose previous
value equals the proven six-decimal rounding of the declared payload. Malformed
or conflicting records fail migration instead of being silently repaired.

PostgreSQL gate reads, duplicate-open checks, resolution and pending selectors
verify the indexed ID, status, resource dimension, amount and deadline against
the payload. Timestamp comparison follows the driver's actual microsecond
encoding relative to the PostgreSQL epoch; full payload deadlines still govern
expiry. Resolution reads and transitions under one row lock. A corrupt row in
an expiry batch rolls back every earlier transition in that transaction.
New open requests must be pending. This repairs persistence in an existing
consumer; it adds no v3 operation approval or new scoped authority requirement.

Three local real PostgreSQL regressions passed, covering the upgrade/rejection
path, exact 28-decimal and maximum-u64 round trips, pre-epoch timestamp
precision, corrupt envelopes with no mutation, batch rollback and concurrent
terminal resolution. Final review restricted the migration UPDATE to genuinely
rounded amounts, with regression evidence that already exact rows retain their
timestamps. The final three native tests, all 40 resource tests and strict
all-target resource/composition Clippy passed in the serial local queue.

The prerelease checkpoint's Linux GitHub release build passed; macOS builds
remain in progress. The first separately dispatched test run failed checkout
because a short hash was supplied; a new run used the full checkpoint SHA.
That run found the historical planned-loop fixture still implementing the old
borrowed `drive_turn` interface. Its test-only adapter now accepts the owned
handoff and tracks exact active attempts with a future-scoped guard; cancellation
acknowledges only when that addressed future has ended. Its historical substrate
evidence is explicitly separate from production Monty acceptance. The root-test
repair awaits the next GitHub run. This introduces no production Rust fallback
and does not claim full-plan or Windows acceptance.

### Actor transport capacity batch (2026-10-09, partial local acceptance)

Ordinary and control request counts and reserved wire bytes now have independent
operator settings. V119 fills only missing fields, preserves explicit values and
advances a partially migrated row's revision once. It constrains positive integer
counts to u32 and byte budgets to u64. The native host rejects unrepresentable
values before persistence and requires control capacity for one full bounded
request plus response. These reservations describe wire retention, not physical
heap or RSS.

Transport clones use one ledger for policy and credits. Acceptance rechecks the
current policy under its insertion lock after serialization. Queued, executing,
abandoned and completed-but-unclaimed requests consume the same credits; only
collection refunds them. Increasing limits is not constrained by a startup-sized
FIFO or the previous 1024-request ceiling. Decreases preserve exact retained
commands/receipts and expose count/byte debt without cancelling existing work.
Ordinary saturation cannot borrow the control lane. The service publishes this
Rust-owned policy after the worker settings ACK and before the common effective
revision; rejected or stale publications leave it unchanged.

Actual actor/service acceptance cases are also available directly from their
owning host crate, using the same source files and actual worker as the existing
composition integration workspace. This keeps host-layer verification independent
of the unfinished preload/export consumers, which the operator explicitly asked
to leave alone. No preload failure has been suppressed or reclassified. The
current whole-tree composition build, V119 migration and native live WebUI
acceptance remain unverified until the affected prerequisite work is ready.

Local command `cargo +1.96.0 test -p brassclaw_monty_host --locked --test
actor_host --test service_host -- --test-threads=1` passed all seven actor and
21 service cases with the freshly compiled actual worker (none ignored).
The actor regression observes both successful and worker-rejected transport
exchanges; rejection at an unresolved root boundary is explicit evidence, not
Recipe success or permission to bypass sequencing. Node syntax checks and
`git diff --check` passed. Strict all-target host Clippy remains failed on
`source_structure.rs:148` (`iter_kv_map`) in unfinished preload code, left visible
under the operator's request. No lint suppression was added. Retained evidence
is `/private/tmp/brassclaw-actor-accepted-lint-preload-pending-20261009`.
These checks do not establish V119 native migration, live WebUI, full composition,
Windows/Linux runtime or complete Phase 3a acceptance for this batch.

### Hosting deadlines (2026-10-09, host-layer acceptance)

V120 adds positive u64-millisecond startup and IPC response settings, defaults
30 seconds each, preserving complete explicit rows and filling partial/missing
fields with one revision increment. The response bound must cover startup and
exceed the configured VM slice. Native validation checks the actual host clock
range, without assuming macOS/Linux/Windows have the same maximum. Ordinary
startup builds `RootBoot` and `ProcessLimits` from those settings; a startup
policy edit applies to the next supervised boot, never global VM lifetime.

The parent bounds actual Boot IPC by the smaller startup/response deadline, so
native compilation cannot defeat the in-worker startup tracker. Failure retains
the original Boot and acknowledged exit/containment diagnostics; no second VM,
Recipe replay or Rust-loop fallback is introduced. Startup/IPC bounds are
independent of the shared task compute account and external Tool effects.

Actor clones share one deadline policy. Every accepted request captures its
response deadline with its credit and exact command; queued/in-flight commands
retain that deadline across later edits. The serialized owner uses the captured
value for the actual exchange. Idle containment, future heap commits and shutdown
settlement read the current policy. Coordinated publication applies actor/deadline
policy after worker ACK and before the common Rust effective revision. Stale or
intrinsically invalid requests leave policy unchanged.

A VM slice increase must fit both the current response deadline and all unfinished
accepted exchanges. Validation occurs before persistence and again before worker
mutation; backpressure rejects the unsafe transition without containing a healthy
instance. Increase the response bound first and let shorter accepted exchanges
finish before increasing the slice beyond those older deadlines. Completed,
uncollected results keep credit/evidence but no longer constrain VM execution.

The same nine actor and 22 service cases pass through the actual freshly compiled
worker, including startup compiler containment and deadline ownership/publication.
Strict all-target host Clippy, JavaScript syntax and `git diff --check` pass.
Evidence: `/private/tmp/brassclaw-hosting-deadline-host-accepted-20261009`.
V119/V120 real PostgreSQL upgrade tests and native live WebUI checks are added and
queued; no production, full preload/catalogue or cross-platform acceptance is
claimed from host-only evidence. Settings source/read and uptake bounds remain
explicit work; this entry does not complete all hosting controls or the v3 plan.

### Native actor/hosting acceptance follow-up (2026-10-09)

The root composition library and actual current worker compiled together with
`cargo +1.96.0 test -p brassclaw_reborn_composition -p brassclaw_monty_host
--features brassclaw_reborn_composition/test-support,brassclaw_reborn_composition/libsql,brassclaw_reborn_composition/skills-db
--locked --lib --test actor_host --test service_host --no-run
--message-format=json`. The same build produced the actual companion worker;
no previous worker was restored after target cleanup.

Eight exact native cases passed: V120 hosting deadlines, V119 actor capacities,
V118 attempt retention, V117 settings publication, V116 backlog, V115 child
contexts, one-root Match/No-Match runtime, and live WebUI settings uptake. The
HTTP case applies increased actor count/byte and startup/response settings and
checks the acknowledged complete limits; it finished in 116.47 seconds. With
31 actual host cases and passing strict host lint this is local worker, database
and HTTP evidence, not complete library, provider/profile or platform acceptance.

Strict composition Clippy still fails on `retained_source.rs:355` in unfinished
preload code. Native compilation also reports unused preload fixture helpers.
These diagnostics remain visible, without suppression or preload/export changes,
as explicitly requested by the operator. Historical failed checks above are
retained evidence; the latest host lint passes, while composition lint does not.
Evidence is retained in
`/private/tmp/brassclaw-hosting-native-accepted-lint-preload-pending-20261009`.

The independent ephemeral `/memory` warning is traced to
`factory.rs::build_local_dev_root_filesystem`, which mounts in-memory backends
for memory and tenant records. Repair must reuse supported persistent backends
and trace their callers/mount aliases, avoiding warning removal without actual
persistence. The existing `PgMemoryDocStore` and Postgres root filesystem are
starting points, not proof that this path currently uses them. Settings
source/read and uptake deadlines, remaining frame/allocator/root/flow controls,
full catalogue/overrides/recovery and the other plan gates remain open.

### Durable hybrid filesystem mounts (2026-10-09)

The production Postgres build previously entered the local project-filesystem
builder with its database pool but mounted both `/memory` and `/tenants` on
fresh `InMemoryBackend` instances. The memory Tool's actual
`FilesystemMemoryDocumentRepository` uses that mount, and trigger conversation
services use scoped `/conversations` aliases under `/tenants`; neither was
made durable merely by the presence of a pool elsewhere in the runtime.

The builder now receives the existing pool and initializes the existing
`PostgresRootFilesystem` schema through its migration owner. Both structured
mounts use that backend. Project/workspace/extension file routing remains local.
Domain repositories still own record schemas, scope paths, identity checks,
CAS and encryption; no workflow or authority moves into the storage backend.
Backend failure aborts attachment without an ephemeral fallback. Pure local-dev
fixtures still use isolated in-memory mounts and retain their explicit warning.

The focused native regression writes through the registered memory Tool,
destroys all composed services and cached repositories, rebuilds, and reads and
searches the persisted document/chunks. A CAS patch survives a further rebuild.
The same case persists a trusted actor pairing and conversation binding through
the trigger conversation service, then verifies the exact binding after rebuild.
Finally a closed production pool must fail mount attachment. This acceptance
does not certify all durable task/effect recovery, audit logs or other stores.

The current root library and companion worker compiled together in one serial
Cargo test build. Three exact cases passed, none ignored: native memory and
conversation persistence (32.51 seconds), the isolated local-dev memory Tool
case (9.81 seconds), and one global-root Match/No-Match runtime (27.72 seconds).
The production cases emit no ephemeral-memory warning; the isolated fixture
retains its accurate warning. `git diff --check` passed. Strict composition
Clippy remains failed at unchanged unfinished preload `retained_source.rs:355`;
two unused preload fixture helpers remain visible in compilation output. The
lint attempt preceded the adjacent tenant mount extension, so no successful
final composition lint is claimed. No preload/export fixes or suppressions were
added. Evidence is retained at
`/private/tmp/brassclaw-durable-mounts-accepted-lint-preload-pending-20261009`.

Source inspection also found an independent disconnected legacy WebUI adapter:
`RebornServices::pg_memory_doc_store` is never populated, so the conditional
`StoreBackedReductionRuleStore` attachment in `webui.rs` does not occur.
Do not enable that adapter blindly: `PgMemoryDocStore` uses deprecated
`MemoryDoc` storage, derives a new project UUID from an already persisted UUID,
and parses PostgreSQL timestamp text as RFC3339. The operator-settings cutover
must use the supported `ReductionRuleStore` port with modern durable storage,
preserve any existing rules, reject corrupt records explicitly, wire the actual
production facade, and prove replacement/listing after restart. This remains
required implementation work; it is not repaired by changing filesystem mounts.

Remaining settings-control work must capture the effective source/read and
uptake deadlines when an operation is accepted. Later edits must not shorten
an already accepted persistence/readback/publication operation or claim the new
deadline effective before its acknowledged revision. Cover memory probes,
edit-lock acquisition, uncertain-commit readback, effective retrieval/Recipe
snapshot waits and singleton-ownership checks, not just the HTTP waiter.
The full five-plan objective remains open.


### Settings coordination deadlines — native acceptance (2026-10-09)

V121 adds `settings_source_timeout_millis` and `settings_uptake_timeout_millis`
to the existing execution-limit DTO, database default/check, WebUI form and
actual runtime observation. Defaults preserve the previous 2 s source and 5 s
acknowledgement timings. Missing/partial upgrades fill only absent keys with one
revision increment; explicit values remain. SQL checks positive integral u64
values without unsafe casts; runtime validation checks the real native clock.
The two deadlines are independent of one another, task compute, worker startup
and IPC containment.

Every accepted edit, reconciliation and effective-snapshot waiter captures the
acknowledged policy before its first await. This captured policy covers edit-lock
acquisition, durable reads/writes and uncertain-commit readback, startup-policy
edit probes, optional automatic probes and heap/admission/settings publication
waits. A timed-out automatic probe remains owned; no parallel replacement probe
is spawned. A timed-out publication waiter does not cancel the service-owned
request or turn an unknown effect into rejection/replay. The successor deadlines
become visible to future operations only through the full snapshot published
after worker acknowledgement. The HTTP waiter checks all relevant pending budget
observations rather than treating task-time acknowledgement alone as full uptake.

Reconciliation now validates the complete desired revision and actual host
feasibility before heap or worker effects. An unchanged revision with any changed
snapshot field is rejected; a regressed owner or worker revision is rejected.
An acknowledged worker revision with a lagging owner snapshot can still settle
its exact desired successor after a waiter timeout. This preserves recovery of
accepted publication without applying same-revision corruption.

The added native case holds a real PostgreSQL row lock for 400 ms while requesting
a 200 ms successor source deadline. The accepted edit must retain its old 2 s
policy, finish and acknowledge the exact new limits. It restores normal settings
through the operator port, attempts a same-revision source/heap mutation, and
verifies the existing database revision guard rejects it with unchanged durable
snapshot and actual heap/limits. No database guard is disabled and no VM is
replaced. The additional full-snapshot runtime check is defensive validation;
this test does not claim to bypass the database and exercise corruption that
the supported writer cannot create. It uses the actual global
worker and asserts no model calls. Native migration cases cover defaults,
partial/explicit preservation, invalid/missing fields, platform clock bounds and
revision-exhaustion rollback. The real HTTP regression edits both deadlines
alongside duration/token/actor/hosting limits and checks the complete acknowledged
limits for one retained root.

The serial `brassclaw-coordination-native` batch passed nine native cases:
V121 migration, actual captured-deadline/revision-guard edit, V115–V120 upgrade
regressions and one-root Match/No-Match. Its tenth case failed before settings at
native image verification, and the test executable subsequently no longer existed
in the shared target. Those results are preserved in
`/private/tmp/brassclaw-coordination-nine-accepted-webui-identity-failed-20261009`.
A rebuild initially encountered a concurrent MCP qualification module that was not
yet present; that compile failure is retained separately. No source gate was
relaxed for either failure. After the qualification module existed, a fresh actual
root test/worker build retained verified copies outside the shared Cargo target.
The live WebUI settings case then passed in 136.87 s, with complete actual limits,
one root, live duration/token edits and the optional real 60 s memory cadence
(`brassclaw-rulesets-native-case-4.log`). Singleton ownership requests/checks/releases still
use their independent 2 s policy and heartbeat cadence, and the reconcile/status
poll cadences remain separate inventory work. Remaining frame/allocator/root/flow,
immutable catalogue/recovery, reduction-rule storage and full plan gates remain
open. Preload/export diagnostics are intentionally untouched under operator
steering; no warning suppression was added.


### Reduction-rule persistence repair — native storage/facade acceptance (2026-10-09)

This is operator-settings infrastructure, not a new task workflow or Rust Tool.
Replace the disconnected `MemoryDoc` bridge with a PostgreSQL implementation of
the existing `ReductionRuleStore` port and attach it from the production pool in
`build_webui_services`. V124 supplies a dedicated JSONB ruleset table keyed by
existing tenant/user/project data relationships; these keys grant no authority.

For a ruleset not yet present, serialize its first access with a transaction-local
advisory lock, recheck the destination and import the exact legacy title/tag/project
selection from V016. Derive the legacy project UUID once from the original slug;
never derive another UUID from a persisted UUID. Decode and validate every selected
record and reject corrupt JSON, invalid rules and duplicate IDs before publication.
Preserve legacy rows; a modern ruleset, including an explicitly empty one, becomes
authoritative so later reads cannot resurrect deleted rules from legacy storage.
An atomic explicit full replacement validates the complete incoming ruleset before
I/O and writes JSONB through typed parameters. It also provides the operator repair
path for corrupt rulesets; read failures remain errors until that replacement.
Do not use deprecated MemoryDoc or textual timestamp conversions. Database failures remain errors, with no in-memory fallback.

Acceptance must use native PostgreSQL: legacy import and preservation, sorted
replacement after reopening, explicit empty replacement, malformed/invalid/duplicate
legacy rules, corruption in modern storage and isolation of unrelated data keys.
A separate ordinary runtime/WebUI-facade test must prove attachment and persistence
after shutdown and rebuilding against the same database. The removed engine loader
is not reinstated, and this work does not claim hot-path rule application.


Native storage/facade evidence (2026-10-09): the three V124 regressions passed
through the newly compiled ordinary composition library with `test-support`,
`libsql` and `skills-db`, Rust 1.96.0 and debug/incremental disabled. The cold-read
case concurrently imports one exact legacy selection, reopens the durable adapter,
replaces/sorts, preserves legacy content, isolates unrelated keys and proves an
empty replacement does not resurrect old rules. The corruption case rejects
malformed/invalid/duplicate legacy data without publishing a destination row,
rejects invalid modern data and wrong JSONB shape, proves explicit replacement
repairs those failures, and reports a closed pool as unavailable. The facade case
builds the real global runtime, uses its ordinary `build_webui_services` attachment,
shuts down, rebuilds against the same native PostgreSQL database and reads the
replacement back; no task or model call is fabricated. This is settings CRUD
persistence/attachment evidence, not renewed engine rule execution.

Exact logs: `/private/tmp/brassclaw-rulesets-native-case-{1,2,3}.log`, durations
2.79 s / 1.83 s / 52.21 s. The actual newly built root test image and protocol-7
companion were retained together outside the shared Cargo target with SHA-256
verification (`/private/tmp/brassclaw-rulesets-native-artifacts.json`) before
running native cases. Production native executable verification remains enabled.
This prevents shared target cleanup from removing a running test's signed image.
The WebUI live-budget regression also passed (case 4, 136.87 s). Strict composition
lint stopped at the existing `brassclaw_engine/src/executor/retained_source.rs:355`
preload/export `collapsible_if` diagnostic. No preload source or warning suppression
was added. Accepted native logs, actual executable checksums, validation commands
and the lint failure are retained under
`/private/tmp/brassclaw-rulesets-native-accepted-preload-lint-blocked-20261009`.
This closes the disconnected operator-store finding locally, while full plan
acceptance, remaining hosting controls and platform gates remain open.


### Singleton ownership controls — local native acceptance (2026-10-09)

This repairs global-hosting infrastructure; it does not add a Recipe workflow or
Tool. V125 extends the existing execution-limit snapshot with
`ownership_check_timeout_millis` / `ownership_heartbeat_interval_millis` (both
default 2000) and `max_pending_ownership_checks` (default 8). Validate positive
integral/native-clock values; fill missing keys once, preserve explicit values
and roll back an exhausted revision. Acquisition, actual PostgreSQL ownership
checks and quiescent release use the acknowledged ownership timeout.

Each admitted request captures an absolute deadline and owns one logical credit
through queueing and actual completion, including an abandoned/timed-out caller.
Replace the fixed channel capacity with credits covering queued and executing
requests. Live increases allow more than eight requests; reductions retain debt
and reject new admissions until real settlement. An edit never retimes an already
accepted request. Report actual capacity/count/debt in the existing status view.
Heartbeat rescheduling uses the last successful real ownership roundtrip; changes
to unrelated settings must not postpone it indefinitely. Ownership loss still
fences work and stops the same global VM; no fallback or unreconciled replacement.

Publish the full validated ownership policy only after actual worker settings ACK
and before publishing the owner's complete effective snapshot. Handle a previously
ACKed worker with lagging owner policy through exact revision reconciliation;
never claim Applied from the durable desired row alone. Boot validation and live
HTTP acceptance must compare the actual policy, capacity and common revision.
Remove the arbitrary 32-owner diagnostic registry maximum: retained/quarantined
ownership remains enforced per actual database, with fallible registry allocation
before worker spawn and removal only after verified quiescence. Never drop unknown
ownership to reclaim diagnostic space.

Acceptance: pure request-credit/deadline accounting covers abandoned callers,
reductions and growth beyond eight without pretending database behavior. Native
PostgreSQL exercises exact ownership/exclusion/loss/quarantine, V125 defaults and
partial/explicit preservation plus invalid values and revision rollback. The
ordinary running-global-VM HTTP regression edits all three controls with duration/
token/transport settings, checks actual uptake and retains VM identity.

All fifteen focused cases passed through the compiled ordinary composition
library with `test-support`, `libsql`, `skills-db`, Rust 1.96.0 and disabled debug/
incremental artifacts. This includes real PostgreSQL ownership-session termination
(37.16 s), exclusive/quarantined ownership, V115–V121/V125 upgrade regressions,
deadline/credit accounting, one global root across Match/No-Match and live HTTP
settings uptake (121.58 s). Strict shared-contract lint passed. Strict composition
lint still stops at the untouched unfinished preload `retained_source.rs:355`
diagnostic; no suppression or preload edit was made. Native logs, commands,
compiled-source hashes and SHA-256-verified host/worker images are retained under
`/private/tmp/brassclaw-ownership-native-accepted-preload-lint-blocked-20261009`.
The validation runner now retains each finalized artifact on Cargo's compiler
event, before Cargo exits: the earlier post-exit staging attempt lost the shared
root test image and ran no cases. Native image verification remains enabled.

This establishes local owner-control behavior, not cross-process fatal recovery:
a database connection loss can release its advisory lock before the old worker
fully settles. Durable generation fencing and reconciled replacement remain
separate acceptance requirements. Remaining cancellation/cadence/frame/allocator/
root/flow controls and platform/full-plan acceptance are also open.

### Complete execution-limit API replacements — local native acceptance (2026-10-09)

Legacy deserialization defaults are needed for old database snapshots, but an
incomplete HTTP replacement must not reset newly introduced operator limits.
`UpdateMontyVmSettingsRequest.execution_limits` now requires every field of the
current canonical DTO when an object is supplied. Derive required names from that
DTO's serialized shape; never use its values as request defaults or maintain a
second field catalogue. Preserve missing/null top-level patch semantics. Reject
missing, unknown, duplicate, wrongly typed and null nested fields before invoking
the settings facade. Preserve direct stored-DTO compatibility decoding. The
existing WebUI already sends the complete GET snapshot and CAS revision.

Two focused parser cases cover every omitted field, exact full replacement,
stored legacy defaults, top-level patch semantics and raw duplicate-key rejection.
A native ordinary WebUI regression starts with a custom ownership capacity,
sends actual invalid HTTP bodies, checks 400 responses with unchanged durable
settings/runtime revision/ownership policy/global VM identity, then verifies a
valid duration-only patch retains those limits and reaches the same VM. These
new cases passed after fresh compilation: both focused DTO parser cases cover
Value and raw JSON decoding, and the actual native HTTP regression passed in
27.38 s with the ordinary global VM and real PostgreSQL. The earlier HTTP failure
was a fixture error: naming the Rust value `null` inside `json!` generated a valid
whole-object null patch instead of the intended invalid nested null. Rename that
fixture value; preserve the actual HTTP 400, unchanged-durable-state, live-policy
and VM identity assertions. Both failed runs and their exact artifacts remain
retained as failure evidence. Do not treat their 200 response as a production
decoder error or change the whole-object patch contract.

The expanded product-workflow `--lib --tests` strict lint stops in its unchanged
preload dependency at `retained_source.rs:355`. No baseline suppression or preload
edit was made. Production-only DTO strict lint passed separately (22.64 s);
`git diff --check` and the WebUI field-list consistency check also passed.
The accepted commands, logs, exact unchanged source snapshot and verified native
host/worker images are retained under
`/private/tmp/brassclaw-replacement-native-accepted-preload-test-lint-blocked-20261009`.
Platform/full-plan acceptance and the unfinished preload cutover remain
independent; this does not claim a passing expanded dependency/test lint.

### Configurable cancellation acknowledgement — local native and consumer acceptance (2026-10-09)

V126 adds `cancellation_ack_timeout_millis` (default 5000) to the existing complete
execution-limit policy, preserving explicit values and migrating only missing
fields with one revision increment. Positive integral/native-clock validation
runs before persistence/publication. The WebUI edits/displays this real limit;
whole-snapshot acknowledgement and status include it.

`GlobalMontyDriver.stop_attempt` fences the exact host, marks cancellation and
signals an already submitted control before policy lookup or any await. The
factory reads a private observation-only source containing the last complete
acknowledged revision/deadline. Its receiver does not retain the service owner;
its last acknowledged value remains usable for settling a failed service. This
is infrastructure/control policy, not Recipe sequencing or a new Rust Tool.
An accepted wait captures its original deadline; successor edits do not shorten
it. Timeout still retains the exact attempt and pending real receipt/effect
state, rather than reporting success or releasing recovery capacity.

New native coverage checks V126 default/explicit preservation, invalid/null/
overflow values and atomic revision exhaustion. Older migration fixtures drop
only the future V126 constraint in their private historical schema; production
constraints stay enabled. The existing ordinary running-VM HTTP regression edits
the new field with all other settings. A separate native running-global-VM case
holds an actual provider call, accepts cancellation under the five-second policy,
changes the policy to 250 ms through ordinary HTTP, verifies that the old wait
keeps five seconds and retains durable admission/capacity, then releases the real
provider and transfers the exact settled receipts/evidence without a late reply.
The contained-worker fixture explicitly supplies a settings source; it does not
substitute its own claims, persistence or effect settlement.

The root `skills-db,test-support,libsql` test build and all twelve focused native
cases passed locally: V126 and the retained historical migration cases, the real
provider-wait cancellation case (32.48 s), strict HTTP replacement and the whole
live-settings case (147.91 s). Evidence is in
`/private/tmp/brassclaw-cancellation-native-case-{1..12}.log` and the exact
retained executable/checksum manifest
`/private/tmp/brassclaw-cancellation-native-artifacts.json`. The two strict API
replacement parser regressions also passed with the new field. An unused import
introduced in the new test was removed after the first build; the unchanged
preload fixture warnings remain visible. The standalone real-worker/PostgreSQL
case `dropped_turn_waiter_requires_worker_and_durable_cancellation_acknowledgement`
also passed (8.48 s). That workspace initially failed to compile a concurrently
added model-capture assertion because `ParentLoopOutput` was not in scope; its
reference was qualified against the actual exported type without weakening the
assertion. A final root compilation and native cancellation rerun passed
(32.92 s), with no affected source changes after the final snapshot. Existing
fixture-only unused ownership/selected-program methods remain visible; this
does not establish warning-free full-matrix lint.

Strict shared production lint initially failed when the Cargo target directory
disappeared during compilation (`couldn't create a temp dir`). Its sequential
retry passed for `brassclaw_host_api` and `brassclaw_product_workflow` with
`root-llm-provider`, `--lib -- -D warnings`. Checks used Cargo 1.96.0 on this
machine and the required NVMe target. Successful and failed diagnostics, commands,
artifact checksums and affected-source snapshots are preserved in
`/private/tmp/brassclaw-cancellation-native-accepted-20261009`. Preload/export
work was left untouched as requested; strict composition lint remains outside
this acceptance claim. Remaining control cadences, frame/allocator/root/flow,
durable recovery and platform/full-plan acceptance remain open.

### Adjustable idle settings reconciliation — local native acceptance (2026-10-09)

V128 replaces the controller's fixed one-second poll with
`settings_reconcile_interval_millis` (default 1000), in the existing complete
execution-limit snapshot, migration, status and WebUI. Upgrade preserves explicit
values and increments only rows missing the field. Positive/native-clock checks
run before publication; historical private-schema fixtures remove only the future
constraint before replaying their old migration.

The controller captures the last whole acknowledged policy for each timer, waits
without polling, and preserves the separate immediate operator-save Notify.
Startup/manual add no OS memory reads. Automatic additionally schedules its next
real sample from the last acknowledged sample timestamp and its independent
sampling period, so a longer durable-settings interval cannot postpone that
sample. Failed reconciliation retries after positive configured backoff, bounded
by the Automatic sampling period when enabled; it never repeatedly wakes on an
already due old sample. Unrepresentable native timer state remains a visible
technical failure that a valid save can recover. Very distant logical deadlines
use representable internal timer chunks, preserving the full deadline without
intermediate reconciliation, measurements or policy effects; this is no new
operator budget or admission cap.

New coverage includes native V128 defaults/explicit values/invalid JSON/revision
exhaustion; pure scheduling cases for manual/startup, Automatic and failed reads;
and real WebUI/global-VM/PostgreSQL uptake of a durable edit without a Notify and
prompt saves under a thirty-second interval. The existing full live-settings
regression now sets ten-minute settings reconciliation during the real optional
sixty-second Automatic sampling test. This verifies the two cadences remain
independent through the production caller.

The actual `skills-db,test-support,libsql` root/worker build and all seventeen
focused cases passed with Cargo 1.96.0 on this machine. These include V128 and
the retained V115–V126 migration fixtures, the new real HTTP/periodic-DB case
(26.94 s), captured source deadlines/same-revision protection (26.79 s), exact
cancellation retention (31.62 s), strict HTTP replacement, the actual long-poll
Automatic sampling case (117.60 s), and matched/unmatched multi-turn retention
of one global VM (26.71 s). The two full API replacement parser regressions also
passed with the expanded twenty-six-field contract. DTO/WebUI/migration field
sets match, JavaScript syntax and `git diff --check` pass, and the affected source
snapshot remained unchanged after compilation.

Strict production lint passed for `brassclaw_host_api`/`brassclaw_product_workflow`
with `root-llm-provider`, and for the composition owner with
`cargo clippy --no-deps -p brassclaw_reborn_composition --features skills-db,libsql
--locked --lib -- -D warnings`. The latter is explicitly package-scoped: it does
not certify dependency/full-matrix lint or the unfinished preload path. Existing
preload fixture warnings remain visible. Evidence, actual executable/artifact
checksums, commands and stable-source snapshots are archived at
`/private/tmp/brassclaw-reconcile-native-accepted-20261009`. The copied case runner
initially wrote the previous batch's result-label prefix; its executed script is
preserved and separate named per-case logs establish the actual seventeen
results, with corrected labels recorded afterwards. No tests were rerun merely
to repair labels.

Preload/export work remains untouched. Transport/allocator/root/flow controls,
browser status cadence, durable recovery, platform and full-plan cutover remain
open; this focused acceptance does not mark the full plan complete.


### 2026-10-09 — Repair preload/export qualification and native bootstrap startup

Public exports of a Tool-usage Skill now all pass typed invocation validation
before effects, including exports not chosen by the current Recipe. The new
actual-worker/PostgreSQL regression rejects an unused export whose declared
signature agrees with its source but differs from the Skill input contract;
inspection creates no durable effect intent. Q1 records actual source-parser
observations for every exact class-22 dependency in the usage combination.
Shared native review fixtures and the authenticated HTTP review expose the
complete graph and preserve qualified exports/dependencies in successor drafts.

Ordinary startup qualification now sends the admitted task token to its contained
host wait, retains and checks the exact terminal child failure for negative
policy/input cases, and heap-allocates component initialization rather than
increasing the test stack or skipping seeding. Bootstrap readback verifies the
whole-workflow selection against its workflow journal and the per-dispatch
selection against its invocation journal, checking exact bytes/checksums for
both. The canonical dispatch serializer is shared by producer and trusted reader;
no selection, provenance, policy or no-replay check was removed.

Accepted local evidence: seventeen standalone actual-worker/DB cases (six Q1,
four retained execution, seven utility), the native public-export preflight,
ordinary matched/unmatched global runtime including bootstrap readback/no-replay
(107.84 s), and authenticated HTTP review (51.33 s). Strict production composition
Clippy with dependencies, `skills-db,libsql`, `--locked --lib -- -D warnings`
passed. Cargo ran serially in screen with mandatory disk checks/cleanup; the real
native test executable and matching Monty worker were retained before target
cleanup. The relevant source snapshot stayed unchanged through the final batch.
Logs, failed diagnostics, executed scripts, source and artifact checksums are
archived at `/private/tmp/brassclaw-preload-export-accepted-20261009`.

This supersedes the earlier operator deferral for the named preload/export
failures. Full authored activation/catalogue cutover, public MCP publication,
platform/performance acceptance and remaining resource controls are separate
requirements; the full plan is not complete.

### 2026-10-09 — Preload lambda parameter scope repair

Preload inspection now records lambda parameters only in the lambda body;
defaults remain enclosing-scope reads. A nested parameter cannot hide an
undeclared outer dependency. Actual worker inspection and execution preserve
literal typed data, including quotes, Unicode and source-like text. Lambdas
remain nested functions for retained Tool-call qualification; the existing
unsupported Tool-closure adapter is rejected before dispatch, never activated
as a supported workflow or reclassified as No-Match.

Serial local `tests/monty_control/Cargo.toml` execution passed all 20 cases:
Q1 review 6, retained execution 4 and utility hosting 10. The real PostgreSQL/
IBS preflight includes a Tool-calling lambda and records zero dispatch intents
for rejected libraries. Strict host lib Clippy with dependencies passed with
`-D warnings`. Source hashes were unchanged through the batch; commands, logs,
source copies and tested hosting context are retained under
`/private/tmp/brassclaw-preload-lambda-scope-accepted-20261009`.
This accepts the named preload repair, not complete authored activation,
live adapter-reserve/WebUI wiring, MCP publication or the full v3 plan.

### 2026-10-09 — Preload exception binding scope repair

The actual source visitor now records `except E as name` as a local binding in
its owning function. Ruff's default visitor omits this identifier; previously a
valid error-handling export was rejected as having an undeclared dependency.
Nested handlers use their own scope and cannot hide an undeclared enclosing
reference. No effect, approval, signature or binding checks were relaxed.

Accepted local evidence: all eighteen actual-worker/PostgreSQL regression cases
pass (`utility_host`: 8, `retained_execution`: 4, `q1_review_host`: 6). The new
utility case both inspects and executes a caught exception with typed literal
data. The retained Tool workflow exercises the handler before dispatch while
preserving live policy denial, output failure and no-replay checks. Strict
affected-package Monty-host Clippy with dependencies and `-D warnings` passes.
Cargo ran serially in background screen with the required disk/process guards;
the focused source snapshot remained unchanged. Evidence is archived at
`/private/tmp/brassclaw-preload-exception-scope-accepted-20261009`.

The resource inventory above no longer presents the repaired historical preload
lint blocker as current. Earlier ordinary global startup/HTTP acceptance remains
separate evidence; this focused batch does not certify unrelated concurrent
edits, complete catalogue activation, platform acceptance or full-plan completion.


### 2026-10-09 — Remove duplicate prepared-flow capacities

The global root no longer rejects a valid prepared tree solely for nesting/path
length above sixteen, over 512 identifiers, foreach bounds above 256, repeat
bounds above 64 or projected execution count above 4096. Existing live typed-value,
heap and task-compute controls cover data and execution. Each Recipe must still
provide a positive finite integer iteration bound; full field/reference/order
validation and explicit exhaustion remain, with no truncation. Dictionaries
index declaration/selection membership. Repeat uses an integer counter so a
large declared bound does not fail native `range` conversion before its first
iteration. The source is retained through the normal installation seed path;
this change adds no Rust workflow or fallback.

Six actual-interpreter cases passed (0.56 s), including 520 declared steps,
projection above 4096, eighteen nested controls, a seventeen-part typed path,
an immediate return under a `u64::MAX` repeat bound, and all 300 foreach results.
Invalid bounds/edges, branch/item scope, missing values and declared item limits
remain covered. Strict standalone flow lint and composition production lint with
dependencies (`skills-db,libsql`, `--locked --lib -- -D warnings`) pass. The native
ordinary matched/unmatched global runtime also passed after recompiling the
seeded root (126.93 s). Cargo ran serially in screen. Executed scripts, actual
host/worker hashes, exact tested source snapshots and logs are archived at
`/private/tmp/brassclaw-flow-live-bounds-accepted-20261009`.

The relevant source snapshot stayed unchanged through compilation and the
post-build check. Shared bootstrap approval activation edits appeared afterwards;
this evidence applies to the retained executable preceding them. The archive
preserves that exact preimage and records the boundary rather than claiming the
new concurrent activation code was tested. The flow files themselves remain
unchanged.

Prepared-control acceptance does not qualify or activate new looped Tool
workflows: complete durable occurrence, retry and continuation support must
precede their activation. Existing supported flat reply/history startup remains
accepted for this snapshot. Full catalogue/recovery/platform/performance and
plan completion remain open.


### 2026-10-09 — Adjustable pending WebUI status cadence (implementation)

`status_poll_interval_millis` extends the existing revisioned execution settings,
with a 3000 ms default. V130 fills missing values without replacing explicit
values and updates the complete default; missing-value upgrades increment their
revision atomically. The common validator and SQL reject zero/nonintegers and
values above the browser timer's signed 32-bit representation, rather than
wrapping a large delay into immediate polling. Older private migration fixtures
drop this successor constraint before replaying their historical defaults.

The live owner retains this browser policy in its acknowledged settings/status
snapshot. The WebUI uses its accepted settings value, keeps serial requests while
uptake/restart is pending, and shares an outstanding request across interval
changes and saves. Unsaved form edits do not reschedule polling. A status response
captured before a save is treated as pending for the accepted revision, so it
cannot prematurely end acknowledgement monitoring. No idle browser poll or VM
restart is introduced. DTO replacement validation remains complete-field/CAS.

Implementation is present. Backend and browser acceptance for the settings
uptake path are recorded below; this is not a full-plan completion claim.


**Backend validation for the status cadence:** two complete API replacement
regressions, strict host API/product-workflow lint and strict composition lint
with dependencies passed. After correcting a concurrent new-migration numbering
collision (the unshipped status migration moved from V129 to V130), fourteen
actual PostgreSQL migration regressions passed. Native periodic/durable WebUI
uptake passed in 58.00 s and the full running-global-VM settings regression
passed in 143.26 s, including the new field and unchanged VM identity. The
corrected screen queue waited for an observed concurrent Cargo execution before
rebuilding. No warnings were suppressed and no checks were relabelled as passing.
Evidence and actual executable hashes are archived at
`/private/tmp/brassclaw-browser-status-interval-accepted-20261009`.

The final frontend also follows a newer acknowledged instance cadence from a
status response, without overwriting unsaved form data. Actual browser acceptance
remains required: use the served WebUI and supported backend to verify pending
uptake/restart polling, changed cadence, a still-outstanding request during a
cadence change, an older status response arriving after save, a newer acknowledged
policy from another writer, cleanup on navigation, and stopping requests after
acknowledgement. This is an explicit Phase-7 browser gate, not satisfied by Node
syntax or backend tests. Later concurrent runtime/review-consumer edits and this
last frontend refinement are separately identified in the archive; the native
results apply to their retained executable, not those newer source changes.


### 2026-10-09 — Native browser settings uptake acceptance

The actual CLI `serve` binary, its retained sibling Monty worker and a private
embedded PostgreSQL instance (V1–V130) passed browser acceptance with agent-browser
0.27.1 / Chrome 149. This was the ordinary `local_dev` startup path, with
`DevOnly` readiness and genuine EnvBearer authentication. The served tab and
settings API JavaScript matched the checked source bytes. A development transport
proxy held unchanged responses fetched from the actual status endpoint; it did
not fabricate status, revisions, acknowledgements or Tool replies.

Observed pending reads remained serial during a delayed response. An unsaved
9999 ms form value preserved the saved 800 ms cadence (observed waits 807.5,
801.5 and 801.7 ms). Saving through the UI acknowledged a new revision; releasing
the older pending response neither regressed displayed status nor restarted
polling. A second HTTP writer's newer acknowledged 1400 ms cadence was picked
up while another genuine durable revision remained pending (observed waits
1404.9 and 1400.7 ms), preserving the unsaved form. Leaving the tab aborted the
real native fetch; remount fetched fresh status and acknowledgement stopped
subsequent requests. Zero/overflow timer values and 511 MiB memory edits displayed
validation errors without a settings write.

The API read wrappers now forward AbortSignals. The tab owns its outstanding
read and notification timer, cancels them on navigation and cannot reuse an
aborted read after remount. Duration/retention fields now have associated labels.
Initial development assertions incorrectly assumed every error banner had an
alert role and omitted localized numeric separators; they were corrected against
the actual UI before the invalid-input cases passed. An off-screen CLI click
was repeated with the real Save button scrolled into view and its HTTP write
verified. These diagnostics were not hidden or relabelled.

Evidence, observed revisions/timing, exact JavaScript snapshots and retained
CLI/worker checksums are archived at
`/private/tmp/brassclaw-browser-status-ui-accepted-20261009`. Restart is unavailable
on this owner (`restart_supported=false`), so this does not claim restart, full
production-profile, Linux/Windows or complete-plan acceptance. The separate CLI shutdown issue discovered by this instance is repaired and
accepted in the following entry; its evidence remains distinct from the browser
acceptance snapshot.


### 2026-10-09 — Drain CLI database ownership before embedded shutdown

The CLI now owns and cancels/awaits its retention sweep before runtime shutdown;
dropping a Tokio JoinHandle alone detached that task and retained its pool.
The production coordinator's lifecycle observer uses a weak back-reference,
breaking the coordinator/store/event-bus cycle while retaining a strong upgrade
for each actual parent-resume callback. Independently owned observers preserve
their existing owning constructor. Public route lifecycles are released after
drain, and the CLI explicitly closes its own pool after runtime drain, verifies
zero retained connections/waiters and then stops embedded PostgreSQL. Closing
that pool does not stop an externally configured database.

Two exact-byte bootstrap comparisons now use fallible JSON serialization rather
than an owned-value comparison that triggered strict Clippy. They still compare
the canonical serialized document, preserving selection/checksum identity;
JSON Value comparison to a string is not substituted for those checks.

Accepted evidence: the actual production-composed coordinator drop regression
and existing blocked-parent resume case each pass. The actual retained CLI and
Monty worker start a fresh embedded instance through migrations V1–V131 and
packaged qualification. An authenticated duration edit reaches effective revision
2 at 701 seconds. Five database sessions are visible before SIGINT; shutdown exits
0 in 0.145 seconds, closes the private HTTP/PostgreSQL ports and logs no warnings,
errors or connection errors. Strict CLI Clippy with dependencies and `-D warnings`
passes. Cargo runs serially in background screen with required disk/process
checks; relevant source hashes remain unchanged through acceptance and lint.

Failed intermediate runs remain archived: retention/coordinator repair alone
still left three connection errors; shared target files disappeared during two
builds; an initial native check omitted LLM configuration and correctly retained
its startup warning. The final native rerun uses the same immutable binaries
with the freshly discovered documented test provider configured through the
supported config path. No warning filter, fake provider or fallback was added.

Evidence, exact commands, source snapshots and executable checksums:
`/private/tmp/brassclaw-serve-shutdown-accepted-20261009`. This local instance has
`local_dev`/`DevOnly` readiness; the result does not establish production-profile,
Linux/Windows, model inference, complete MCP publication, catalogue cutover or
full-plan acceptance.


### 2026-10-09 — Remove the duplicate IPC frame ceiling

Native parent and worker now accept the configured finite frame policy without
a second 64 MiB maximum or 1024-byte minimum. Positive sizes must fit the
unsigned 32-bit wire length and native signed allocation size. All three writers
use checked wire-length conversion, and framing rejects unrepresentable limits
before buffer allocation. Existing finite allocator, value and decoder-depth
checks remain active; this is not unlimited transport or allocator acceptance.

Accepted evidence: thirteen process and nine utility cases pass in the standalone
actual-Monty workspace. A real global VM admits a 65 MiB typed string and returns
it intact at its actual matching boundary, preserving root identity. A real
utility evaluates and returns the same size intact. Both use explicitly configured
80 MiB frames with finite 1 GiB startup backstops; global boot applies the
512 MiB logical heap and its derived 676 MiB physical limit. This is configured
allocator capacity, not measured RSS. Existing malformed-frame,
private-diagnostic, dropped-exchange, task/context and containment cases remain
passing. The strengthened exact utility case also passes with an oversized
capacity independently rejected by wire/native representation (its physical limit
is larger), and a one-byte positive policy rejected by actual serialization.
Strict host Clippy with dependencies and `-D warnings` passes. Tests are serial
in background screen with required disk cleanup/process checks. Evidence and
initial/final source snapshots: `/private/tmp/brassclaw-frame-representation-accepted-20261009`.

**Remaining transport publication implementation:** production startup still
supplies 64 MiB frames and a 4 MiB adapter reserve. Extend the existing full
`execution_limits` replacement with frame and adapter-reserve fields, migration
read compatibility, exact API replacement validation and WebUI units/ranges.
Compute the physical backstop with checked arithmetic from the acknowledged
logical heap plus two configured frames and the configured adapter reserve;
retain the separate automatic heap revision and consumed task accounting.
Move `ServiceClient.frame`, `TransportClient.frame`, actor reservation/preflight,
worker receive/write bounds and `WorkerHeap` reserve together through the
serialized service/worker acknowledgement. No stale per-clone scalar may enforce
an older frame policy after the settings revision is reported effective.

An accepted exchange retains its original request/response bound and deadline.
Use the prior framing contract for the policy acknowledgement itself, then switch
both endpoints at that explicit boundary. Validate proposed value/source/stdout
limits against the proposed frame, not the old frame; require control-lane
reservation capacity for a complete request/response. Before a reduction, account
for already accepted/queued exchanges, completed uncollected receipts, retained
root/child results and the actual allocator's live physical usage. Reject an
unsafe manual reduction or keep it visibly pending with admission backpressure;
do not let an old valid queued frame or retained effect result kill the worker,
truncate evidence, reset usage or become an apparently successful cancellation.
A partial/uncertain publication remains unacknowledged and fenced/reconciled;
never roll back through a blind second settings write or replay a Recipe.

Native WebUI acceptance must prove growth above 64 MiB, a safe reduction, a
reduction during real retained/in-flight work, unchanged VM identity/task usage,
old-exchange framing/deadline retention, exact settings-revision conflict handling
and worker/Rust/allocator agreement. This entry completes the representation
prerequisite only; live transport publication, allocator reserve controls,
decoder-depth support, platform/performance and full-plan acceptance remain open.


### 2026-10-09 — Live worker adapter reserve and allocator receipt acceptance

`worker_adapter_reserve_bytes` is now a full-replacement execution limit with
4 MiB default and explicit zero support. V132 fills only missing values,
preserves overrides and increments only migrated revisions; database checks
reject missing/null/string/negative/fractional/overflowed values. The API still
requires every current execution field. Native heap/frame/reserve arithmetic
is checked before persistence, including combined edits.

The worker acknowledges the reserve and derived budget above its allocator
baseline. Rust observation and WebUI status use that receipt; the display does
not call it RSS. Publication shares the existing task/bounds update and retains
consumed task time, heap revisions, pending automatic reductions and root VM
identity. Expected pre-publication denials preserve policy and service life;
uncertain publication keeps evidence and contains the worker.

Actual local acceptance passed 13 process and 23 service cases, two API
replacement cases and all 17 real PostgreSQL settings/migration cases. Historical
migration fixtures reconstruct their earlier constraints without the future
V132 check; production constraints remain enabled. Strict composition lib lint
with dependencies and `skills-db,libsql` passed. Serial Cargo ran in background
screen with compiler/process guards and mandatory target disk cleanup.

A fresh ordinary CLI/default-features instance and packaged worker ran through
V132. Actual Chrome WebUI saves grew the reserve to 8 MiB (revision 3), set zero
(revision 4) and restored 4 MiB (revision 5). Rust/Monty receipts matched, heap
revision stayed fixed, physical budget equalled heap plus acknowledged non-VM
reserve. Blank, negative, fractional and unsafe browser integers made no PUT;
an unrepresentable native edit returned 400 without persistence. Shutdown was
clean, exit 0 in 0.128 seconds, with closed HTTP/PostgreSQL ports and no warnings
or connection errors. This resource check performed no model inference.

Evidence, exact binary/source checksums, commands, receipts and prior failed
diagnostics are retained at
`/private/tmp/brassclaw-adapter-reserve-accepted-20261009`. A browser assertion
was corrected to inspect the visible validation message rather than an absent
alert role; no UI error was suppressed. A compatibility compile crossed an
independent shared gateway trait transition; current trait/wrappers agree and
the queued migration retry passed. That MCP change is outside this acceptance.
Only test fixtures changed after native acceptance; the production prefix was
verified identical. No full-tree, platform/performance or full-plan acceptance
is claimed. Production frames remain 64 MiB: finish retained per-exchange frame
policy/deadline/allocator coordination, decoder support and root coroutine
capacity controls before closing the resource matrix.

### 2026-10-09 — Preload comprehension scope and export qualification repair

The actual retained prior worker reproduced two scope defects. Comprehension
merging dropped a nested lambda's closure flag, allowing a repeatedly invoked
Tool helper to appear as one qualified call. Separately, `:=` inside a
comprehension executed correctly but inspection reported its enclosing local as
an undeclared library dependency. Preserve the closure flag and carry assignment
bindings through comprehension scopes to their owning function/lambda. Lambda
body locals remain isolated; defaults evaluate in their creation scope.

Real PostgreSQL/IBS preflight now rejects all five Tool-calling forms (list, set,
dict, generator and nested comprehension) before any dispatch intent. Pure forms
execute without leaking local names or turning Unicode/source-like input into
code. The two-step pure workflow review now invokes declared preload exports;
it observes the typed handoff and retains rejection of reordered execution.
All 22 focused cases passed (six Q1, four retained execution, twelve utility),
with strict host library Clippy and no ignored cases or warning suppression.
Cargo ran serially in background screen with process guards and required disk
cleanup. Relevant source hashes stayed fixed throughout the validation batch.

Exact snapshots, commands, logs and the previous worker's reproductions are in
`/private/tmp/brassclaw-preload-comprehension-binding-accepted-20261009`.
This completes the identified scope/qualification regressions. Full authored
catalogue activation, shared immutable preload caching, MCP acceptance, platform
and full-plan acceptance remain separate open requirements. No new Tool,
Recipe replay or Rust-loop fallback was introduced.

### 2026-10-09 — Exact adapter reserve at standalone worker bootstrap

A retained native worker reproduced an otherwise valid zero-reserve Boot failing
before it was read: the placeholder heap imposed the default 4 MiB adapter
reserve. Before Boot, record only the armed physical backstop and frame reserve;
Boot still installs its exact adapter/heap layout before compilation or execution.
Keep allocator enforcement and counters intact. This does not lower or alter
the product's 512 MiB configured heap floor.

The actual regression reaches Ready with a 2 MiB standalone VM budget, two
128 KiB frame reserves and zero adapter reserve, acknowledges the exact allocator
geometry, preserves VM identity/wait across inspection and acknowledges process
termination without containment/reap errors. All 14 process-host cases and strict
host library Clippy passed, without ignored cases or warning suppression. Serial
background screen, process guards and required disk cleanup were used; relevant
source hashes stayed fixed. The reproducer, prior-worker digest, exact snapshots
and logs are at `/private/tmp/brassclaw-zero-reserve-boot-accepted-20261009`.
This is standalone hosting acceptance, not full platform or plan acceptance.

### 2026-10-09 — Publish Rust settings at the worker acknowledgment boundary

Source tracing found a revision gap: after the worker acknowledged a runtime
edit, the transport could begin another child RPC before the service published
its Rust task settings and hosting policy. Move that bounded synchronous metadata
publication into the sole transport owner, before observations, the successful
receipt and any subsequent exchange are released. The callback is restricted to
mechanical settings commands; no database, model, Tool or workflow execution is
added. The service continues to retain its unconsumed root boundary and own the
settings receipt. Dropping a waiter does not cancel the accepted publication.

Validate the exact worker acknowledgment before publishing Rust settings. An
expected worker denial never calls the publisher. Publication failure or panic
after a worker acknowledgment retains that actual acknowledgment and fences and
reaps the worker; queued commands retain their identity without starting transport.
Do not roll back an acknowledged revision or retry its command. Expected denial
comparison now separates configured limits from changing active-context counts.
Validate a combined deadline/slice edit against its acknowledged new VM bounds,
rather than the preceding unpublished observation.

All 35 real-worker actor/service cases passed (12 actor, 23 service), including
dropped waiters, queued child progress, expected denial, publication failure and
panic, retained task consumption, and a combined slice/deadline reduction. Strict
composition library Clippy passed with `skills-db,libsql`. Cargo ran serially in
background screen using process guards and required disk cleanup. Relevant source
hashes stayed fixed for the focused batch and native acceptance.

A fresh ordinary CLI/default-features instance and retained packaged worker ran
through PostgreSQL V133. Actual authenticated settings API edits acknowledged
revisions 2–4 in Rust and Monty: duration 701 seconds, a 2-second slice with a
5-second response deadline, then a 5-millisecond slice with a 1-second deadline,
96 child contexts and zero adapter reserve. The heap revision and startup child
process identities remained fixed. A stale revision returned 409 without changing
persisted or effective settings. Shutdown exited 0 in 0.147 seconds with closed
HTTP/PostgreSQL ports and no warnings or connection errors. This resource check
used the existing `local_dev` configured-provider bootstrap and no inference;
it does not establish the final DB-only provider cutover.

Exact source snapshots, diff, commands, logs, binary digests and API receipts:
`/private/tmp/brassclaw-runtime-publication-boundary-accepted-20261009`.
This closes the identified worker/Rust publication gap. Production frame size
still remains 64 MiB. Live frame changes still need retained per-exchange
request/response framing, allocator feasibility, queued work ownership and
revisioned publication through the supported settings vertical. Shared preload
caching, full authored activation, MCP, other platforms and full-plan acceptance
remain open; no Rust-loop fallback or warning suppression was introduced.

### 2026-10-09 — Nonmutating candidate allocator-layout validation

Add `validate_worker_limits` beside `set_worker_limits`, sharing the same checked
logical/physical validation. The observation writes no ceiling, baseline or
counter. The installation worker's private protocol 9 carries a mechanical
`ValidateMemoryLayout` command with expected settings and heap revisions plus
candidate VM/frame/adapter geometry. It checks actual allocation ownership,
positive wire/native frame representation and checked reserve arithmetic, without
advancing Python or publishing settings. A probe does not reconcile or erase an
existing pending automatic heap target. No Tool, model or Recipe workflow is added.

All 50 real-worker cases passed (15 process, 12 actor, 23 service). The new
regression observes a larger 128 MiB VM/80 MiB frame proposal without applying it,
retains a real pending automatic target, and verifies unsafe live-heap reduction,
both stale revision fences, zero and overflow denials with unchanged VM identity,
settings, allocator geometry and waiting state. A separately framed actual native
worker also rejects physical headroom below live allocations while its candidate
VM allowance still covers the observed VM bytes. No state is published on denial.
Its first assertion used the wrong failure-enum spelling; the assertion was
corrected to the actual `UnsafeHeapReduction` wire representation, without a
worker change or weakened state checks. Both scripts/diagnostics are retained.

Strict composition library Clippy passed with `skills-db,libsql`. The initial
`--locked` invocation stopped before compilation because a removed composition
dependency remained in the root lockfile. Cargo repaired that stale edge offline;
package identities and versions were verified unchanged. The subsequent native
`--locked` build passed. Standalone test sources and their lockfile stayed fixed.
Background screen, serial Cargo, process guards and mandatory disk cleanup were
used. The updated vendor patch applies to the verified upstream tree and matches
all 228 recorded vendor files exactly; original upstream hashes remain unchanged.

A fresh ordinary CLI/default-features instance and retained protocol-9 worker
ran through PostgreSQL V133. Authenticated settings edits acknowledged revisions
2–4, combined slice/deadline reduction, child capacity and zero adapter reserve;
stale revision rejection preserved settings and startup child identities.
Shutdown exited 0 in 0.182 seconds with closed ports and no warnings or connection
errors. This uses the existing local configured-provider bootstrap and no model
inference. Native evidence covers the retained binaries and matching archived
source snapshots. Later shared MCP gateway/test edits are outside this resource
acceptance; runtime test formatting leaves its production prefix byte-identical.

Evidence, source snapshots, patch round-trip, binary digests, commands, logs and
actual receipts: `/private/tmp/brassclaw-memory-layout-probe-accepted-20261009`.
This is the allocator feasibility prerequisite, **not a reservation or completed
live frame setting**. Production frames still remain 64 MiB. Continue by retaining
the sole transport boundary across the revision-checked probe, durable commit
outcome and worker/Rust publication. Do not persist based on an earlier unfenced
probe, change Python state during validation, or release a child RPC in between.
A rejected commit applies no runtime successor; unknown commit/publication must
retain evidence and fence without rollback/replay. Complete queued per-exchange
frame/deadline retention, allocator/frame acknowledgment, full settings API/UI
wiring and native retained-work acceptance before claiming live frame support.
Platform, performance, catalogue/preload and full-plan acceptance remain open.

### 2026-10-10 — Durable runtime settings at one transport boundary

Ordinary non-heap edits now retain the sole transport owner across inspection of
the actual current allocator layout, its revision-checked nonmutating probe, the
durable commit outcome, worker publication and synchronous Rust publication.
Child RPCs stay queued throughout. No provisional runtime settings are installed
before persistence. Rejected writes preserve the old revision and require no
rollback. A missing acknowledgment, timeout, panic or publication failure after
a confirmed commit retains actual worker evidence and fences execution without
replay. Unexpected inspection/probe evidence is retained rather than discarded.

The durable write and settled readback keep the source deadline captured before
the edit, independently of the worker IPC deadline. Verify the entire requested
successor on successful writes and uncertain-outcome readback, not just its
revision. The instance edit owner survives an HTTP waiter disappearing; effective
full settings observation still follows the existing revision-aware controller.
The larger task-settings command is boxed to avoid inflating every control-queue
variant; the initial strict lint failure was resolved without suppression.

All 37 actual worker actor/service cases passed. After the representation repair,
the five affected settings cases passed again, including a rejected real file
commit, a completed file commit whose waiter was dropped, retained child state
and feed counts, and an actual synchronized write whose future never acknowledges
completion. The latter fences queued child execution, retains its original work
and the pre-commit worker receipt, and reaps the worker without pretending the
write did not happen. Strict composition library Clippy passed with
`skills-db,libsql`. Cargo ran serially in background screen with process guards
and required disk cleanup; relevant sources stayed fixed in the final batch.

A fresh ordinary CLI/default-features instance and retained packaged worker ran
through PostgreSQL V133. An actual temporary database trigger rejected a valid
operator edit: HTTP returned its redacted 500 error, the persisted and effective
settings remained unchanged, and the worker stayed alive. A test-only trigger
deliberately injected `pg_sleep(1.25)`; that request took 1.259 seconds, exceeding
the proposed 200 ms source deadline and retained 1-second IPC deadline. This is
a deadline-ownership check, not normal settings latency. The accepted deadline held;
Rust/Monty acknowledged revisions 2–6, including combined execution/hosting edits,
with unchanged startup child identities. Stale revision rejection preserved the
final settings. Shutdown exited 0 in 0.183 seconds with closed ports.

The first native run's blanket no-error log assertion rejected the deliberate
database failure. Its script and diagnostics are preserved. The corrected run
requires exactly that expected error, rejects every other warning/error and
connection error, and passed against the same retained binaries without another
Cargo build. No warning or error was hidden. This resource acceptance uses the
existing configured-provider bootstrap and performs no model inference; it does
not establish the DB-only provider or MCP/catalogue cutovers. The final native
source archive's 2,197 files stayed fixed through build and acceptance.

Evidence, exact sources/diff, commands, initial failed diagnostics, native binary
digests and actual API receipts are retained at
`/private/tmp/brassclaw-durable-runtime-settings-accepted-20261010`.
This completes the ordinary runtime-settings persistence/publication fence,
**not** the full resource matrix. Manual heap transactions still use their
existing separate path; combined heap/reserve edits require coordinated full
settings feasibility and publication. Production frames remain 64 MiB. Finish
retained per-exchange frame/deadline policy, allocator/framing acknowledgment,
settings API/WebUI wiring and retained-work acceptance before claiming live
frame control. Shared immutable preload caching, authored activation, durable
recovery, other platforms and full-plan acceptance remain open.

### 2026-10-10 — Ordinary settings-write latency clarification

The preceding 1.259-second request includes the intentionally injected
`pg_sleep(1.25)` in a temporary PostgreSQL test trigger. The trigger was removed
by the test; it is absent from production code and normal settings publication.
Operator saves wake reconciliation immediately through the existing `Notify`;
they do not wait for the periodic settings timer.

A separate fresh ordinary CLI/PostgreSQL instance used the same verified retained
debug binaries, with no test delay/rejection triggers and no model inference.
Forty sequential authenticated settings PUTs (20 duration, 20 hosting edits) took
2.412 ms median, 3.067 ms at p95 and 5.807 ms maximum, including response decoding,
durable persistence and the complete effective Rust/Monty acknowledgment. Every
revision advanced exactly once, the heap revision and worker identity stayed
fixed, and an independent SQL query confirmed the final durable revision. No
warning, error or connection failure occurred. These measurements show no
1.25-second delay in ordinary saves; no runtime optimization is justified by the
fault-injection timing. No recompilation or weaker deadline checks were needed.

The evidence records startup (57.847 seconds) and shutdown (2.594 seconds)
separately; neither is part of the save timings. This is an idle local debug-build
measurement, not loaded-system, release, task throughput or platform acceptance.
Startup performance remains a separate audit item under the full plan.
Exact commands, binary digests, all 40 timings and revision receipts, and logs:
`/private/tmp/brassclaw-settings-latency-accepted-20261010`.

### 2026-10-10 — Atomic combined heap/runtime settings publication

The ordinary settings store now uses the same owned durable transaction for
manual heap edits and other execution/hosting edits. A complete nonmutating
worker candidate checks task/VM/context revisions and the proposed heap/reserve
geometry together before persistence. An explicit manual successor can replace
a pending automatic target; the discarded target does not constrain its new
reserve. Validation neither reconciles pending reductions nor changes Python,
allocator or configured settings state. The private protocol advances to 10;
paired application/worker packaging remains mandatory.

After the exact durable successor is confirmed, one allocator operation publishes
the heap and adapter reserve, then the worker publishes runtime settings. The
synchronous Rust acknowledgment callback publishes the actual heap observation
and controls before releasing subsequent child RPCs. Rejected writes publish
nothing and need no rollback. Unknown writes or post-commit publication failures
retain actual evidence and fence execution without replay. The owned operation
continues when its HTTP waiter disappears. Effective full settings still follow
the revision-aware controller and ownership account. The lower-level legacy
heap-transaction primitive remains available but is no longer used by product
settings edits.

All 53 actual worker cases passed (16 process, 12 actor, 25 service), including
pending-target replacement, stale/invalid full candidates, rejected real file
commits, accepted commits with dropped waiters, retained child state/feed counts
and an unacknowledged synchronized write that fences queued child execution.
Strict composition library Clippy passed with `skills-db,libsql` and `-D warnings`.
The initial locked lint stopped before compilation because new shared ingress
development dependencies lacked lockfile edges. Offline resolution added exactly
those four edges; every package identity, version and checksum stayed unchanged.
Serial background screen and mandatory disk/process guards were used.

A fresh ordinary CLI/default-features instance and its retained protocol-10 worker
ran through PostgreSQL V133. An actual database-side rejection of a combined
heap/reserve/duration edit preserved persisted/runtime settings and allocator
geometry. A successful combined growth and subsequent safe combined reduction
acknowledged heap revisions 2 and 3 with settings revisions 2–7; other edits kept
the heap revision fixed. Worker/startup child PIDs stayed unchanged. The injected
1.25-second SQL delay still proves independent source/IPC deadlines, not ordinary
save latency. Stale edits preserved the final state; an independent SQL query
confirmed the exact instance settings key. Shutdown exited 0 with closed ports.
The expected database rejection error is explicitly required; every other warning,
error and connection failure is rejected.

The first native verification queried all settings rows instead of the instance
composite key and failed after the runtime checks. Its script/logs are preserved.
The corrected query passed against the same retained binaries without rebuilding.
The 2,232 archived source files remained fixed throughout build and acceptance.
Evidence, exact source/diff, failed and passing logs, commands, verified binary
images and actual API receipts:
`/private/tmp/brassclaw-combined-runtime-layout-accepted-20261010`.

This closes the combined manual heap/reserve/runtime settings publication gap.
Production frames remain 64 MiB. Continue retained per-exchange frame/deadline
policy, allocator/framing acknowledgment, settings API/WebUI wiring and retained
work acceptance before claiming live frame controls. Existing configured-provider
bootstrap and no model inference limit this evidence; shared MCP, authored
activation/preload caching, durable recovery, release/platform performance and
full-plan acceptance remain open.

### 2026-10-10 — Retain each accepted IPC frame contract

The transport actor now records the accepted frame bound with its original
request/response deadline, evidence bytes and credits. Envelopes and collected
receipts preserve that bound, including abandoned tickets and completed unclaimed
results. Credit reservations use that exact frame rather than an unrelated
instance maximum. Every actual IPC phase of an owned durable settings/heap
transaction uses the same retained frame contract.

Private protocol 11 carries the accepted maximum in the request and echoes it
in the reply. The worker checks positive/native/wire representation, its receive
capacity and the actual received length before execution, then encodes its reply
under the retained bound. The parent requires the exact echoed contract and keeps
its original response bound throughout the await. Invalid preflight requests
retain their original command without advancing or killing the worker. A real
reply that cannot fit is a contained transport failure with an unacknowledged
command, never a truncated success or fabricated snapshot. The instance allocator
capacity remains separate from a tighter per-exchange contract; paired binaries
are required. This change does not publish a new global frame setting.

All 69 actual worker cases passed (19 process, 13 actor, 25 service, 12 utility).
New regressions verify retained/refunded credits across policy changes and dropped
tickets, nonmutating frame denials, invalid worker contracts before boot and
actual containment when the intent-boundary reply exceeds its accepted frame.
Expected fatal diagnostics remain visible. The initial compilation found two test
assertions requiring a nonexistent `RequestTicket: Debug`; the assertions were
corrected without adding production Debug behavior or suppressing warnings.
Strict host Clippy with `--all-targets` and `-D warnings` passed. Serial background
screen, process guards and mandatory disk cleanup were used.

A fresh ordinary CLI/default-features instance and retained protocol-11 worker
passed actual PostgreSQL V133/API acceptance: combined heap/reserve/duration growth
and reduction, rejected durable writes with unchanged settings/allocator geometry,
retained independent source/IPC deadlines, settings revisions 2–7, stale rejection,
exact instance-key SQL confirmation and unchanged startup child PIDs. Shutdown
exited 0 with closed ports. Exactly the deliberate DB rejection error is required;
every other warning/error and connection failure is rejected. All 2,232 archived
source files stayed fixed throughout build and acceptance. Exact sources/diff,
commands, failed/passing logs, retained binary images and actual API receipts:
`/private/tmp/brassclaw-retained-frame-contract-accepted-20261010`.

**Continue the full live-frame implementation:** production still uses 64 MiB.
Replace per-clone `ServiceClient.frame`/`TransportClient.frame` scalars with the
shared acknowledged policy; add the finite frame setting to the full existing
DB/API/WebUI execution-limits contract. Validate the complete candidate against
its proposed frame, actor lanes and root source before durable mutation.
Distinguish the configured bound for new exchanges, an older exchange's retained
bound, and the worker's actual receive/allocator frame capacity.

Track the maximum bound of all accepted credits, including completed unclaimed
receipts, in the existing actor ledger. A counted ordered set can update this
maximum on acceptance/refund without rescanning memory or polling OS pressure.
A settings reduction must retain capacity for older exchanges: its owned candidate
uses at least the maximum of the new policy and retained contracts, and its ACK
uses the old exchange contract. Later accepts use the newly acknowledged policy.
Do not shrink the worker's receive/allocation capacity while an older exchange
still needs it. Expose desired/effective capacity and debt explicitly; a larger
reserved capacity is not proof that new exchanges may use an older policy.

Coordinate capacity release through the sole transport owner after credits are
refunded. Recheck actual native allocator feasibility; unsafe physical reductions
remain pending with the old effective capacity and explicit evidence. Ordinary
work/control must remain usable under its acknowledged finite bounds; capacity
debt alone is not a small-pressure signal or reason to pause every new task.
Maintain checked heap + two effective frame buffers + adapter arithmetic without
resetting consumed task budgets. Validate the actual protocol/control receipt
representation as well as values/source/stdout limits; no guessed fixed minimum,
blind rollback, lost effect result or silent truncation.

Native live-WebUI acceptance still needs growth above 64 MiB, safe reduction and
reduction during retained/in-flight work, correct old/new request and response
bounds, retained task usage/identity/effect evidence, full settings revision
publication and actual final capacity release. Shared MCP, authored activation/
preload caching, durable recovery, DB-only providers, release/platform performance
and full-plan acceptance remain open.

### 2026-10-10 — Shared frame requirements and retained-credit accounting

Service and transport clients now read one shared frame-policy source instead of
copying a scalar into each clone. The existing actor ledger counts every accepted
frame bound in an ordered map across both lanes. Completion preserves the bound;
collecting a receipt refunds its count and credits together. The maximum changes
only when the last receipt retaining that bound is collected. Failed enqueue
refunds use the same checked accounting path and retain evidence on accounting
failure. A read-only watch exposes configured and maximum retained bounds without
polling, rescanning VM memory or retaining service ownership. These requirements
are distinct from acknowledged native worker allocator capacity.

All 39 actual worker actor/service cases passed, including duplicate maximum
bounds, abandoned tickets, completed unclaimed receipts, exact credit refunds,
change notifications and unchanged root identity. Strict composition-library
Clippy with `skills-db,libsql`, `--locked` and `-D warnings` passed. All 12 archived
relevant source/manifest/lock files remained unchanged through validation. Exact
source/diff, commands and logs:
`/private/tmp/brassclaw-frame-requirements-accepted-20261010`.

This closes the shared parent-side source and retained-credit accounting
prerequisites. Production still configures a 64 MiB frame at startup; no live
worker-capacity acknowledgment or API/WebUI frame setting is claimed. Worker
protocol 11 and process/utility code are unchanged, so their previous acceptance
remains the baseline rather than a newly executed CLI/PostgreSQL check. Continue
the complete live-frame candidate, allocator/capacity publication and retained-work
acceptance above; the full plan remains unfinished.

### 2026-10-10 — Acknowledged frame capacity and release after receipt collection

Private protocol 12 now reports configured frame policy, required retained
capacity and actual effective capacity separately. The complete mechanical
runtime candidate can couple a frame change with heap, adapter, values, duration
and context changes. Its nonmutating worker probe validates finite native/wire
representation, proposed value/source/stdout bounds, allocator feasibility and
capacity for its own retained exchange. Publication updates framing and allocator
geometry synchronously; request decoding follows effective capacity and each
reply keeps its accepted bound. The parent validates acknowledged geometry using
effective capacity, rather than its obsolete startup frame scalar. Mixed protocol
images fail closed; host and worker must be packaged together.

The actor checks frame candidates against all credited older bounds, publishes
new configured policy to every clone only after worker/Rust acknowledgment and
retains old receipts without changing their bounds or deadlines. The last credit
refund wakes the sole owner to request capacity release. This mechanical RPC
advances no Python and preserves settings/heap revisions and task consumption.
Unsafe physical reductions remain explicitly pending, retaining effective
capacity; later ordinary safe boundaries can reconcile them. Frame debt alone
does not pause admission. One shared observation publishes frame status and
allocator geometry together, without OS pressure polling or page reclamation.

All 72 actual worker cases passed (20 process, 15 actor, 25 service, 12 utility).
New regressions cover growth past startup capacity with a real larger task,
unchanged task compute consumption and VM identity, old exchange bounds after
reduction, duplicate abandoned receipts, denied premature release and automatic
capacity release after the final refund. Strict Clippy initially required a
match guard; it was corrected without suppression. The affected 15 actor cases
were rerun; the unchanged 57 cases remain applicable. Both final strict host
`--all-targets` and composition `skills-db,libsql` library Clippy checks passed.
The serial validation guard waited for the other active Cargo build.

A paired native CLI/default-features and protocol-12 worker passed fresh ordinary
PostgreSQL V133/authenticated API acceptance for the existing settings path:
combined heap/reserve/duration growth and reduction, rejected durable writes with
unchanged runtime/allocator state, retained source/IPC deadlines, exact SQL
revision confirmation, stale rejection and unchanged child PIDs. The 1.25-second
SQL delay was deliberately injected for deadline acceptance. Shutdown exited 0
in 0.148 seconds with closed ports. Only the explicitly required DB rejection
error occurred. All 2,235 archived native source files stayed fixed; verified
paired images, sources/diffs, commands and failed/passing logs are retained at:
`/private/tmp/brassclaw-frame-capacity-accepted-20261010`.

**Continue the product integration before claiming live frame settings:** add
`execution_limits.max_ipc_frame_bytes` with a finite 64 MiB default to the neutral
settings DTO, compatibility decode, strict complete API object and migration.
Preserve explicit values and advance each affected durable revision once. Replace
the startup constant, extend the service's complete hosting candidate and publish
frame/actor/deadline changes coherently at the existing durable barrier. Construct
required capacity from the configured successor, the edit's retained exchange
and every older credit; validate actual future control/receipt serialization and
root source before persistence, without a guessed fixed minimum. Existing
callback checks against the old frame must be revised for the complete proposed
frame/actor combination rather than forcing separate operator edits.

Expose desired/effective configured policy, retained requirements, effective
capacity and pending physical release in API/WebUI. Native WebUI acceptance must
exercise growth above 64 MiB, reduction with older in-flight/unclaimed work,
correct old/new contracts, effect evidence, task usage and final capacity release
in the same global VM. Current native acceptance covers ordinary startup and
existing settings edits; the live frame field remains unwired and production
still starts at 64 MiB. Shared immutable preload caching, authored activation,
durable recovery, shared MCP/provider acceptance, DB-only providers,
release/platform performance and full-plan acceptance remain open.

### 2026-10-10 — Live product IPC frame policy and native WebUI acceptance

The preceding product wiring gap is now implemented for
`execution_limits.max_ipc_frame_bytes`. Its finite 64 MiB default remains a
compatibility default, not a second transport ceiling. V134 preserves explicit
stored values, increments only migrated rows once and rejects missing, null,
noninteger, zero or out-of-wire-range values. The strict complete API object
requires the new field; compatibility decoding of old stored rows supplies it.
Startup reads the setting instead of a constant. Native size, source, value,
clock, actor reservation and allocator constraints remain checked.

The complete service candidate coordinates frame, actor, adapter, heap and task
settings at the existing owned durable barrier. Acceptance captures frame and
value contracts together under the actor ledger lock; serialization outside the
lock must recheck both before admission. Older exchanges retain their original
bounds/deadlines and require capacity until their last credit is refunded. A
combined frame/actor edit is validated against the proposed pair; operators need
not first publish an incompatible intermediate policy. A same-frame edit keeps
an already pending physical release rather than forcing it early.

Before persistence, the actual worker validates the serialized future control
acknowledgement and subsequent inspect/reconcile commands against the proposed
frame. It projects the actual root identity, waits, task accounting, revisions,
context counts and allocator geometry, with conservative wire-width counters;
this projection is never published as an observation. Pending automatic heap
application is checked as an additional possible receipt. Temporary transport
buffers leave the VM allocation domain and are dropped before publication. A
rejected candidate cannot apply pending heap housekeeping. There is no guessed
fixed minimum frame. The worker repeats validation before actual publication.
The private protocol is 13; installation must ship the paired CLI/worker.

API and WebUI expose configured policy, required capacity, effective capacity,
largest retained exchange bound and pending physical reduction. The form sends
a complete candidate, validates source/stdout/value and actor relationships, and
continues status monitoring while an older bound still requires capacity.

**Accepted locally:** 61 real subprocess/actor/service cases (15 actor, 21 process,
25 service), two complete/strict API DTO cases and all 18 native PostgreSQL
settings/migration cases pass. The service durable-write regression now couples
8 MiB frame growth with actor/heap/reserve settings while a real file/child task
waits; rejected writes preserve the old policy, an abandoned successful waiter
still settles, reduction to 64 KiB releases capacity, and the next real file
task completes in the same root. A 512-byte future control frame is rejected
without changing worker settings, heap, allocator or identity. Strict host
all-target and composition `skills-db,libsql` library lints pass with `-D warnings`.
Changed frontend JavaScript syntax checks pass.

A fresh ordinary local-dev CLI/embedded-PostgreSQL instance used the verified
paired native binaries. An actual DB trigger rejected a complete frame/heap/
reserve/duration edit without runtime publication. Successful growth 64→96 MiB,
combined reduction 96→32 MiB and release of the edit's retained capacity kept the
same child PIDs. Finite representation and actual root-source denials preserved
the durable revision. In the actual browser, an incomplete 48 MiB frame candidate
was rejected before a write; the complete frame/actor candidate saved once and
showed the persisted and Rust/Monty-effective 48 MiB value. An independent SQL
read confirmed the final settings, stale writes returned 409, shutdown returned
zero and both private ports closed. Only the expected deliberate DB-rejection
error occurred; no warning or connection error was silenced.

The first native attempt's ten-minute browser window expired before browser
authentication completed. Its failing evidence is retained. Only acceptance was
rerun with the same verified images and a longer interaction window; compilation
was not repeated. All 2,236 archived native sources stayed unchanged through the
build and accepted rerun. Sources/diffs, commands, passing/failing logs, paired
images, browser receipt and database/runtime receipts are retained at
`/private/tmp/brassclaw-live-frame-product-accepted-20261010`.

**Latency clarification rechecked on these images:** 40 ordinary authenticated
saves without a delay trigger took 2.937 ms median, 4.307 ms p95 and 4.955 ms
maximum, including durable persistence and effective Rust/Monty acknowledgement.
The earlier 1.25-second save deliberately included SQL `pg_sleep(1.25)` to test
independent durable/IPC deadlines; it was never ordinary latency. The browser
instance's frame/heap growth save took 5.541 ms. These are idle local debug-build
measurements, not release, loaded-system or cross-platform performance claims.

**Remaining acceptance:** exercise native product tasks/child execution,
cancellation, in-flight and unclaimed work through live frame edits, including
physical reduction that is temporarily infeasible and final release. Candidate
worker/actor/service cases do not certify that entire product matrix. Shared
immutable preload caching, authored activation, durable effect recovery, shared
MCP/provider acceptance, DB-only providers, release/platform performance and
full-plan acceptance remain open. Do not mark the full v3 plan complete.

### 2026-10-10 — Host progress during owned settings publication

The latency investigation confirmed that the earlier 1.25-second SQL write was
deliberate deadline fault injection, not normal settings latency. It also found
a real scheduling defect under a held durable publication: the service awaited
the settings operation without polling its accepted host futures. PostgreSQL
showed an answered-invocation update idle inside an uncommitted transaction,
without a database blocker, while the settings update waited on the deliberate
lock. Its Rust future could not advance to commit or queue its continuation.

The service now polls already accepted host I/O while runtime or heap settings
publication owns the transport boundary. Completed host results remain private
until the barrier settles; a child may queue its retained continuation, but the
actor still holds VM execution until durable persistence, worker acknowledgement
and coordinated Rust publication complete. No workflow selection, new dispatch,
effect replay or Rust-loop fallback is introduced. Buffered results are included
in normal draining, shutdown and failure evidence. A host panic or failed finish
contract fences admission and requests containment promptly, preserving both
the actual host result and the actor-owned durable write/receipt.

**Accepted on the final sources:** all 26 real service regressions and strict
host all-target Clippy (`-D warnings`) pass. The strengthened file/child case
verifies actual I/O progress, private completed answers, old-bound queued work,
rejected writes and abandoned successful waiters. The fatal-port case verifies
prompt fencing, no reply replay and the retained actual durable write and
unclaimed worker receipt. One missing test import was corrected before the
passing run; the failed compiler diagnostic remains archived.

Four native ordinary chat tasks completed through authenticated product HTTP
ingress, actual embedded PostgreSQL and the installed pinned reply/history
workflows in the same global VM. A real journal wait survived frame/time growth;
an actual answered-effect transaction committed and queued its old-bound
continuation during a held settings reduction. After release, both runtimes
acknowledged the same revision, each reply/history effect occurred once, task
contexts were released and effective frame capacity reached the reduced value.
The next admitted task used that revision and VM. Shutdown exited zero in
0.208 seconds; no native warning, error or connection failure occurred.
All 2,236 native source hashes remained fixed through build and acceptance.
Paired verified binaries, sources, commands and receipts are retained at
`/private/tmp/brassclaw-host-progress-final-accepted-20261010`. Earlier failing
native observations remain in the separate first-through-fourth attempt archives.

This closes the native task/child and queued in-flight continuation subset of
the preceding live-frame acceptance gap. Native cancellation, temporarily
infeasible physical reduction and the complete unclaimed-work product matrix
remain open, along with shared immutable preload caching, authored activation,
durable recovery, MCP/provider acceptance, DB-only providers and release/platform
performance. The full v3 plan is not complete.

### 2026-10-10 — Native cancellation, journal fencing and live frame edits

Native acceptance exposed two distinct gaps. A dispatch-intent transaction held
the turn snapshot lock while its journal insert waited, blocking the WebUI
cancellation write itself. After repairing that ordering, cancellation still
became a generic Recipe failure: the default runtime's coordinator and task host
used different cancellation owners, and the global driver did not observe the
host signal while awaiting its task receipt. Both failed native attempts and
their diagnostics are retained; neither was treated as passing evidence.

Admission reservation/start, workflow selection and dispatch-intent preparation
now use an unlocked claim preflight followed by effect-free journal preparation
and a final locked claim check immediately before commit. The commit remains
required before any Tool dispatch. Cancellation/reclaim that wins during
preparation rolls back provisional writes. No claim/policy check, unique effect
identity or uncertain-commit retention was removed. Late answer retention still
uses the original invocation keys independently of current dispatch authority.

The default runtime shares one cancellation factory between coordinator wakes
and task hosts. The driver observes that host signal, fences the exact attempt
and awaits the owned settlement with its captured acknowledged live timeout.
A genuinely acknowledged cancelled task returns the typed cancelled exit;
the trusted exit applier independently verifies the durable request. Partial
effects remain retained. Confirmed completion reads the existing finalized reply
evidence rather than attempting another host call after a later fence.

**Accepted:** five actual-worker/native-PostgreSQL retained-execution cases pass,
including the new blocked-intent regression: cancellation returns while the
insert remains blocked, final claim verification rejects it, zero intents commit
and the original exact selection remains. Strict composition library Clippy
with `skills-db,libsql`, dependencies and `-D warnings` passes on final sources.

Four native ordinary product chat cases pass on verified paired CLI/worker
images. Pre-effect cancellation across frame/time growth settled in 0.260 s
with zero intents/replies/history effects. Cancellation after a confirmed reply
survived frame reduction and a successor 250 ms acknowledgement setting;
the original wait settled in 1.061 s, retaining exactly one answered invocation
and one reply, with no history dispatch/replay. Both durable turns are
`Cancelled` with one claim and no failure. Task contexts and old frame capacity
were released, and the following chat completed in the same global VM and
unchanged child processes. Shutdown exited zero in 0.189 s with closed ports
and no warnings/errors. All 2,237 archived source hashes stayed fixed.
Evidence is at `/private/tmp/brassclaw-cancel-delivery-accepted-20261010`.

**Next cancellation contract repair:** `brassclaw_reborn::turn_runner` still wraps
`stop_attempt` in a fixed five-second `MONTY_STOP_ACK_TIMEOUT`, which can shorten
an operator-selected longer acknowledgement policy during driver error/shutdown.
Reconcile that caller with the bounded owner contract and captured revision;
do not add another hidden ceiling or drop owned futures/results. Qualify through
real pending work, a configured deadline above five seconds, failed/unacknowledged
settlement and supervised shutdown. The accepted cases above do not cover this
path, provider waits, arbitrary blocked commits, temporarily infeasible physical
release, fatal recovery or the full v3 plan.

### 2026-10-10 — One owned stop deadline and no replay after worker interruption

The preceding fixed five-second caller deadline is removed. The production
`GlobalMontyDriver` already fences the exact attempt before waiting and bounds
actual service/durable settlement using its captured acknowledged cancellation
revision. `TurnRunnerWorker` now awaits that bounded result; it neither shortens
a longer WebUI policy nor abandons the owner's settlement future with another
timer. The neutral port documents that ownership and failure contract. A debug
event records the captured revision/deadline without task content or secrets.
Unavailable/expired acknowledgement still stops worker admission and preserves
the original attempt/effect evidence; it never establishes absent effects.

Real native qualification exposed a second defect: an acknowledged shutdown
after a confirmed reply persisted the interrupted turn as `Queued`. The worker
now requeues only explicit uncommitted input admission. Worker shutdown and
heartbeat interruption preserve sanitized terminal failure and the original
Monty journal/settlement evidence instead of permitting another claim/replay.
This uses the current supported terminal-failure contract; it does not claim
implemented safe continuation or unknown-effect reconciliation.

**Accepted on final sources:** 20 focused worker checks, strict composition
library Clippy with `skills-db,libsql` and `-D warnings`, and the paired native
CLI/worker build pass. Two actual product cases use authenticated WebUI ingress,
embedded PostgreSQL, the global Monty service and pinned packaged reply/history
workflows. A real confirmed-reply answer-journal wait survives more than six
seconds with a twelve-second acknowledged policy; shutdown settles in 6.523 s.
The independent 300 ms deadline case reports missing acknowledgement after
0.303 s in the owner's actual log timestamps, retains the blocked dispatch
record/admission and then drains the original answer without a history effect.
Neither case manufactures an absent effect or replays the published reply.

Both cases restart the same installed instance. The interrupted turns remain
`Failed` with one claim, one answered invocation and original attempt count one;
the long case records `worker_cancelled`, the expired case records
`monty_stop_unacknowledged`. Journals and the single published replies stay
unchanged. An ordinary new task completes after each restart. Expected forced
shutdown recovery warnings and the deliberately induced deadline error remain
visible; no other warning/error or connection failure occurs, and both restart
shutdowns close their private ports cleanly. These are graceful product restarts,
not fatal global-VM recovery qualification or a claim of full v3 completion.

All 2,237 source hashes stayed fixed through final build and acceptance.
Paired images, sources, commands, native/restart logs, actual PostgreSQL receipts
and deadline timestamp evidence are retained at
`/private/tmp/brassclaw-stop-recovery-accepted-20261010`. The earlier accepted
deadline cases that exposed unsafe `Queued` recovery are preserved separately
at `/private/tmp/brassclaw-stop-owner-accepted-20261010`.
Native real-provider waits, arbitrary blocked commits, infeasible physical
capacity release, complete unclaimed-work acceptance, supervised fatal recovery,
shared immutable preload caching and the other full-plan gates remain open.

### 2026-10-10 — Compatible first-feed compiler sharing

`PreparedReplSeed` compiles the first child feed without executing it. Compatible
empty contexts receive independent compiler tables, heap, globals, typed inputs,
host receiver, OS environment and execution controls, while module/function
bytecode stays immutable and shared. Insertion into a used namespace or a
different input/options layout fails before execution and retains the original
session. Later Recipe feeds still compile against that child's retained state;
this neither creates fresh state per step nor replaces Monty's result handoff.
The function-code `Arc` serializes as the previous Code payload, without a new
dump or worker protocol. Trusted snapshot restoration remains required.

The actual worker now retains source/checksum/alias selections weakly. Every
hit verifies actual source bytes and current source limits; cached aliases never
grant permission or replace current kernel Tool checks. Active contexts retain
their first seed. Closing them releases that ownership and prunes dead entries;
the cache retains no task inputs/results or inactive catalogue indefinitely.
Cold first feeds avoid the previous preflight-plus-execution recompilation.
This is compatible first-feed reuse, not complete dependency-chain or catalogue
prefix compilation, activation-time warmup or measured performance acceptance.

Strict vendor linting also exposed the existing heap page helper's unnecessary
boxed return. Its new append helper uses a safe, length-checked conversion from
directly allocated uninitialized slots, preserving fixed-page addresses and
allocation behavior while removing the raw-pointer cast. The vendored lockfile's
four stale control.6 entries now match control.7; third-party versions are
unchanged. All 228 upstream hashes and the regenerated patch round trip verify.

**Accepted:** 49 actual interpreter checks, 42 actual host/transport/service
checks, 37 retained PostgreSQL/IBS/worker/typed-input/compatibility checks,
strict interpreter and host linting with `-D warnings`, and paired native CLI/
worker builds. Interpreter regressions prove shared bytecode pointer identity,
fresh closure/list state, retained predecessor exports, rejected incompatible
seeding and trusted dump restoration. Worker acceptance includes current policy,
exact effect preservation, cancellation and unsafe export rejection.

Four ordinary native product chats pass through authenticated WebUI ingress,
embedded PostgreSQL and pinned packaged reply/history workflows in the same
global VM. A real journal wait survives live frame/time growth; an accepted older
host result survives durable frame reduction. Contexts and old capacity drain,
the following chat completes, and shutdown exits zero in 0.185 s without
warnings/errors. The synchronized growth save took 5.105 ms in this run; it is
not a full latency benchmark. All 2,238 source hashes remained fixed through
acceptance. Images, sources, commands and receipts are preserved at
`/private/tmp/brassclaw-seed-cache-accepted-20261010`. The integration fixture's
worker image was copied after Cargo exited while its serial tests were running;
that retention limitation is recorded separately from images captured on their
compiler-artifact events before Cargo exited.

Full dependency-chain/catalogue caching, complete authored/preload activation,
provider waits, reconciled fatal recovery, platform/performance acceptance and
the remaining simplified-v3 gates remain open. The native cases use legacy
provider bootstrap and actual model discovery, with deterministic Match tasks;
they do not qualify DB-only provider configuration or inference behavior.


### 2026-10-10 — Cross-platform CI caller repairs (local acceptance)

GitHub run `38068121220` on `f14b7d1635a88e191ac511ddf3f44d93cdb67bfa`
finished with four failing jobs: the tagged Monty upgrade harness, contained
Monty on Ubuntu and macOS, and the CLI package. Native memory capacity passed
on Windows, macOS and Ubuntu; those results do not establish full platform
acceptance. The failed logs are preserved without filtering their diagnostics.

The upgrade lifecycle assertion retained an older completion shape. It now
checks the exact `receipt_ref: null` emitted by the actual global root on
a host error. The corresponding extension-host assertion is aligned too;
neither assertion drops fields or changes failure into No-Match.

Both contained jobs failed to compile `model_host`: its real production
task factory had acquired concrete MCP-gateway dependencies, and the separate
harness lacked the direct `tokio-util` dependency. The factory now retains an
already selected provider lease through a private lifecycle-only port. The
actual MCP adapter retains the original binding and CommandPort, checks its
surface identity and delegates cancellation/settlement disconnect to the same
owner. The constructor receives that optional owner explicitly; production
supplies the real adapter, while the non-MCP harness supplies None. There is
no substitute gateway, conditional test bypass or workflow fallback. The
harness manifest/lock graph records the existing pinned tokio-util directly.

The CLI failure expected activation of the retired outbound MCP client.
Section 26 of monty.composition.md explicitly requires blocking that activation
before network/publication while preserving installation assets and durable
operator intent. The CLI regression now verifies the exact rejection, no
success output, unchanged manifest bytes, rejected duplicate installation
in a new process, and successful removal. The real lifecycle regression also
checks that rejected activation leaves a previously enabled installation
unchanged and absent from the active registry after restart. Supported bundled
first-party lifecycle checks remain separate; the outbound client is not
reactivated to satisfy an obsolete assertion.

**Accepted subset:** 24 actual upstream interpreter/isolated-allocator tests
pass on a frozen local snapshot of the base commit plus these repairs. All ten
executables were copied, fsynced and hashed on compiler-artifact events before
Cargo exited. The 9,026 snapshot file hashes stayed fixed through that run.
The strict upgrade lint subsequently passed in a coordinated serial window.
All 156 contained tests passed, followed by all-target strict contained linting.
Ten focused native follow-ups also passed after correcting shared fixture
ownership and initialization: every source-including caller uses the same
variant builder, and standalone model-host admission initializes the exact
packaged compaction prompt instead of depending on another test running first.
The original dead-code/needless-borrow diagnostics, missed Q1 fixture caller
and test-order failure remain recorded; no warning was suppressed.

The paired application/worker build, real lifecycle rejection test and all
five CLI extension subprocess tests passed. The ordinary-chat vLLM/MCP
acceptance returned a final answer that failed its required content assertion.
That attempt is not accepted. Its diagnostic now reports that reply together with the
persisted task outcome and actual exchange/call counts, preserving the same
content and durable child-call requirements. The next actual vLLM/MCP run passed
all those requirements, including the settled matched child, parent No-Match,
durable exchange disconnect and identical global VM identity. No production or
request behavior changed between the failed and passing inference runs; the
failure's exact content was unavailable before this diagnostic change. This
demonstrates a successful provider path, not reliable model command compliance.
Provider quality/repeated-run acceptance remains under prefix_v3_upgrade.md
Phase E and the full-plan acceptance gates. Strict product Clippy also passed.
None of these local checks certifies peer edits excluded from the frozen source
copies or full cross-platform acceptance.

The subsequent strict upgrade lint failed because another agent cleaned
the shared NVMe target during compilation: fingerprint/dep-info/temp paths
disappeared. The diagnostics remain visible. The screen batch stopped at that
first failure; it did not start another Cargo process or infer success.
The user authorized coordination, and the other chat reserved a serial window
and acknowledged the competing clean. Early release messages were rejected by
the messaging tool's approval policy. After completing the serial window, the
release message was successfully delivered; this chat relinquished Cargo while
that chat validates its changes. The active goal remains open. Push and rerun
the failed CI gates. Reuse the passing
upstream, contained and CLI evidence unless relevant sources change.

Snapshot, ten retained images, compiler/test output, original CI diagnostics
and serial validation commands: `/private/tmp/brassclaw-ci-repair-*` and
`target/validation/ci-repair-20261010*`. Concurrent peer work is excluded from
this snapshot and must retain its independent acceptance. Full dependency-chain
compiler sharing, authored activation, complete prefix/provider sources, durable
fatal recovery and the other remaining plan gates are still open.

### 2026-10-10 — Preload assembly and Engine caller repair (local acceptance)

The CI caller repairs were pushed as `fd909c17032f41577388a9f54b461d2d427e1852`.
[GitHub run 38080007851](https://github.com/chtugha/brassclaw/actions/runs/38080007851)
passed its Monty upgrade gate. Its contained Ubuntu job then exposed a separate
Engine integration-test dependency error: `retained_inputs.rs` references the
actual host API's adapter-reserve constant without a direct dev dependency.
The candidate adds that dependency and the root lockfile edge; it changes no
production Tool or workflow. The contained workspace consumes Engine as a
dependency and does not include Engine's dev-dependency edges in its lockfile.

Preload assembly now uses a dependency queue with a stable-UUID ready frontier,
checking the whole closure before returning an order. Shared dependencies load
once; missing nodes and cycles return errors, never a partial accepted order.
This replaces repeated remaining-node and loaded-order scans with
`O((V + E) log V)` graph work. The executor retains the acknowledged prefix
length of its immutable preload order instead of allocating an order copy and
scanning a loaded-ID map on every step. The cursor advances only after an actual
successful effect-free preload. A dropped/failed feed retains existing execution
fencing; children have their own cursor and typed state. No selection, approval,
Tool binding, dispatch or retry rule changes.

Candidate checks cover stable frontier ordering, shared dependencies, missing
nodes, self/multi-node cycles and a 4,096-node nonrecursive chain. The existing
real PostgreSQL/IBS/global-worker/kernel regression admits exactly four feeds:
two definitions and two usage occurrences. Reloading definitions on the second
occurrence would fail its real budget rather than go unnoticed. The actual
Engine `retained_inputs` integration target is included alongside those focused
checks and strict affected Engine/caller linting. All three graph unit cases,
all four real Engine integration cases, all five contained retained-execution
cases and both strict affected lints passed. The screen batch held
`/private/tmp/brassclaw-shared-cargo.lock` throughout and verified all 9,027
source hashes before and after each check. Actual executable images were
protected on compiler-artifact events. The frozen candidate excludes peers'
authored activation and post-turn reconciliation work. Sources/commands:
`target/validation/preload-assembly-20261010-r2` and
`/private/tmp/brassclaw-preload-assembly-r2-*`.

The obsolete first queued wrapper survived closing its screen, later acquired
the common lock and blocked the corrected batch. Exact process inspection found
its wrapper/shell with no Cargo child. Approved termination retired only those
obsolete processes; the corrected batch then acquired and released the lock.
Do not treat closing a screen as proof that its detached descendants stopped.

The same GitHub run independently found CLI MCP port collisions and PostgreSQL
fallback diagnostics leaking into reply-only stdout. Those separate repairs and
their real subprocess/lifecycle acceptance are pending. Native memory capacity
passed on Linux, macOS and Windows, while a skipped full Windows build and the
failed/cancelled product jobs do not establish whole-platform acceptance.

Engine AGENTS/CLAUDE/MONTY routing now identifies the vendored Monty 1.0 control
source, global service and typed retained path. Old v0.0.16 language observations
are explicitly historical; the deleted per-chat driver, plain-text substitution,
error-to-Tier-2 fallback and fixed allocation-count limit are not current targets.
This documentation correction supplies no runtime acceptance.

**Next resource reconciliation, identified from actual source:** the current
child `TaskControl` inherits the interpreter control trait's combined execution/
preparation clock. `RootControl` separately observes preparation but still debits
it to the task compute account. Constructor preparation and Rust value adapters
also debit that account. Reconcile these callers with Phase 3a's executing-VM-time
contract and validator_v3.md section 7.7 before accepting complete compiler reuse:
retain preparation/adaptation observations separately in the shared account,
keep non-overlapping execution cursors and their live revision/latched failures,
and preserve independent source/heap/value/transport/cancellation limits. Trace
all root, child, constructor and conversion callers; validate actual interpreter
observations and settings changes, not only manually incremented counters.
No clock/resource behavior has been changed or accepted by this preload candidate.
Complete dependency-chain compiler sharing, general catalogue activation, prefix/
provider/recovery and full-platform acceptance remain open.

### 2026-10-10 — CLI instance listener and shutdown repair (local acceptance)

GitHub run 38080007851's CLI failures came from separate real instances sharing
hardcoded loopback MCP port 9090. The failure path also let PostgreSQL's last-
resort `pg_ctl` stop inherit reply-only stdout. `run` and `repl` now request port
zero from the OS by default and expose `--mcp-port` for an explicit stable port.
The host still binds only loopback and publishes the actual bound endpoint
through the existing listener/provider observation. There is no free-port probe
or bind fallback. `serve` retains its explicit port contract. Builds without
`skills-db` reject a nonzero unsupported CLI port before startup.

Listener startup/observation failure drains the already running runtime and
then stops its owned database. Conversation-creation failure also reaches normal
cleanup. Cleanup failures remain visible; ordinary PostgreSQL shutdown errors
are returned instead of being hidden at debug level. The original startup
failure remains primary when cleanup also fails. The last-resort blocking stop
preserves both `pg_ctl` diagnostic streams on stderr, including warnings.

The actual CLI smoke executable passed all 72 enabled cases with two concurrent
test threads, including both originally failing Codex-auth startups, reply-only
piped stdin and a real occupied-port regression. The latter holds an actual
listener while a complete PostgreSQL/global-runtime instance starts and rejects
the explicit port; it checks empty reply stdout and confirmed normal cleanup.
Four preexisting config-init cases remain explicitly ignored; this result does
not claim their acceptance. Strict all-target CLI/embedded-PostgreSQL lint passed.
All 9,027 frozen source hashes remained fixed. Executables were protected on
compiler-artifact events and the actual subprocess application was paired with
the verified unchanged worker. No model reply or successful Tool was fabricated.

Sources/commands/receipts: `target/validation/cli-listener-repair-20261010`,
`/private/tmp/brassclaw-cli-listener-*`, and
`/private/tmp/brassclaw-ci-repair-cli-listener*`. This batch excludes peers'
activation/reconciliation and the subsequent clock correction. Cargo/cleanup
held the common lock; a new GitHub product run remains required.

### 2026-10-10 — Executing-time task account correction (local acceptance)

`SharedMontyTaskBudget` now retains distinct execution, preparation and Rust
adaptation observations. Only execution is compared with `max_compute_time`.
Both cumulative execution and preparation cursors retain explicit baselines and
monotonic ownership across feeds/resumes. Any overflow or regression fails the
shared task closed; live limit increases cannot revive it. Settings publication
still preserves one coherent revision and never resets any observations.

Child `TaskControl` now consumes the actual interpreter's separate observations.
A legacy combined-only callback cannot attribute VM time and fails closed.
Scheduling slices still observe execution plus preparation, and cancellation
still runs at their boundaries. Root preparation scopes preserve exact coroutine/
admission ownership but record preparation independently. Worker constructor/
artifact preparation and Rust data conversion record their own nonoverlapping
categories. Source, heap, value, frame and worker-deadline bounds are unchanged;
this correction does not make compilation/native hangs uncontained.

Nine shared-account cases, one real child-interpreter clock comparison and all
156 selected contained control/worker/service/kernel cases passed. The child
comparison retains its actual REPL over two feeds, checks observed execution and
preparation against the actual tracker, applies a live revision and confirms
cancellation adds no new work. Root A/B/A regression verifies all three task
categories against their actual context ownership maps. Invalid typed input
records adaptation without VM execution; original host failure evidence survives
an exceeded task account without replay. Both strict all-target affected lints
passed. All 9,028 frozen source hashes remained fixed and actual worker/test
images were protected on compiler-artifact events. Sources/commands/receipts:
`target/validation/execution-clocks-20261010`,
`/private/tmp/brassclaw-execution-clocks-*` and
`/private/tmp/brassclaw-ci-repair-execution-clocks-*`.

The initial clock edits were parked when the other chat requested stable sources
for its native build. Its matching worker/tests and source fingerprints were
captured before these eight owned paths were reapplied. Its independent native
reconciliation evidence is preserved, not recertified by this batch. Worker
protocol/serialized task-accounting fields and interpreter dump ABI are unchanged;
the additional internal observations do not alter those schemas. Complete
dependency-chain compiler sharing and full product/platform cutover remain open.

The peer's later monolithic composition test build also emitted an unsuppressed
macOS linker warning: compact-unwind DWARF offsets exceed the 16 MiB encoding
limit. Its protected image's actual `__eh_frame` section is `0x1036110` bytes.
This warning remains unresolved; neither helper/native success nor lint claims
its removal. Test-profile optimization must be measured against the affected
retained source and real tests before any profile change is accepted; disabling
unwinding or linker diagnostics is not a remedy.


### 2026-10-11 — Native development startup and unwind repair (local acceptance)

The composition CI command timed out without a failed assertion in run
`38086683707`. Its unchanged command watchdog is 28 minutes inside a 30-minute
job. Git attributes both values to `ab5a907628` (2026-06-06, CI scope split).
No measured estimate justifying 28 minutes was found. The command includes
compilation as well as all package tests; the failed run spent approximately
eight minutes compiling. This is a CI watchdog, not a Monty task budget.

Source inspection identified repeated retained executable copies and full
SHA-256 integrity checks during native startup. A five-second sample of an
already accepted actual composition startup found 379 of 396 samples on its
active blocking thread in unoptimized SHA-256 compression. The sampled test
completed successfully; concurrent peer compilation makes that first wall time
unsuitable as a performance baseline.

The root workspace now optimizes the `sha2` dependency at level 3 in the dev
profile, inherited by tests. The composition test crate alone uses level 1 to
reduce its native code/unwind records below Mach-O's compact-unwind offset range.
Both preserve debug assertions, overflow checks and normal unwinding; compiler
artifact events confirm those settings. Hashing, retained file identity,
corruption rejection and exact artifact/approval checks remain in place. The
release profile already optimizes SHA-256; this is not a release speed claim.
No architecture-specific compiler flags, warning suppression or test exclusion
was added, and the CI deadline has not been changed.

A frozen 9,006-file candidate and protected baseline executable/worker pairs
establish local macOS evidence. The baseline differs by the Cargo profiles and
unrelated strengthened deployment-test assertions; the selected startup caller
is unchanged. Under one serial validation window that caller completed in
61.75 seconds before and 7.94 seconds after the combined profile change.
This comparison does not attribute the improvement to either override alone,
certify Linux/Windows timing or establish throughput under load.

The baseline composition executable's `__eh_frame` section was 17,044,816 bytes;
the candidate's is 14,408,320 bytes, below 16 MiB. The actual build emitted no
oversized-unwind warning. Eight candidate cases passed: four native executable
identity/copy/corruption/capacity cases, actual startup, retained global
matched/unmatched execution, live WebUI/Rust/Monty settings and seed
idempotency/tamper rejection. Affected strict library lints passed with
`skills-db,root-llm-provider,test-support` and toolchain 1.96.0.

Evidence is retained in `/private/tmp/brassclaw-native-profile-accepted.json`,
its artifact and frozen-source receipts, and
`target/validation/native-profile-artifacts-20261011`. Executables were captured
at compiler-artifact events before Cargo exit; the exact paired worker was
retained independently. The complete frozen composition library binary then passed with four test
threads: 755 passed, zero failed, one preexisting live-provider case ignored,
in 288.68 seconds. Its independently retained ordinary-provider evidence is
not rerun or represented as part of this suite. This is local library evidence,
not the GitHub all-target/platform gate. CI still requires a rerun, and this
entry does not complete the v3 plan or its recovery/catalogue gates.


### 2026-10-11 — Containment ordering and worker EOF diagnostics (local acceptance)

The CI log's repeated generic `invalid worker request` diagnostics were also
traced through actual containment. `GlobalProcess::interrupt` closed stdin
before requesting a kill, allowing a waiting interpreter to observe an EOF
first. It now requests containment before closing stdin/stdout. The process is
still fenced immediately; kill failures, reaping failures and interrupted
command evidence remain retained. Only an actual root shutdown acknowledgement
and observed exit certify graceful shutdown. This does not classify pipe EOF
as success or replace fatal-runtime reconciliation.

Worker request-read failures now preserve their original I/O error kind behind
the same constant diagnostic, instead of converting every failure to `Other`.
The real malformed-frame regression includes a declared 128-byte body cut off
after private sentinel bytes. The actual worker exits unsuccessfully with an
`UnexpectedEof` diagnostic and no framed stdout, while the sentinel stays out
of stderr. Invalid lengths and malformed records retain their classified
errors. No request/source/provider text is emitted and no diagnostic is muted.

All 21 actual contained process-host cases passed, including explicit graceful
shutdown/reap, busy startup deadline, dropped busy exchange, allocation fatal
containment, live memory/frame/settings changes and isolated child/task
cancellation. Affected process-host/worker strict lints passed. The frozen
9,006-file candidate was checked before/after compilation and acceptance;
matching executable artifacts were retained before Cargo exit. Tests ran under
the common Cargo lock while their compile-time worker pathname remained owned.

Evidence: `/private/tmp/brassclaw-worker-containment-accepted.json`,
`/private/tmp/brassclaw-ci-repair-worker-containment-artifacts.json` and the
associated logs/source receipt. This targeted lifecycle repair does not certify
full durable crash reconciliation, other platform acceptance or plan completion.


### 2026-10-11 — Shared PostgreSQL policy and terminal observer repair

`brassclaw_pg::connection` now owns parsed URL/keyword endpoint classification,
TLS enforcement and certificate roots for pools and dedicated sessions. All
hosts/hostaddr overrides are examined; remote or ambiguous endpoints reject
`sslmode=disable` and upgrade `prefer` to validated TLS. Explicit local `require`
remains TLS. Hostaddr-only TLS supplies the actual IP as its certificate name.
Missing endpoints and inconsistent host/port lists fail before connection.
Native roots are preferred, with Mozilla fallback only for an empty native store.
Configuration errors do not emit credentials. Shared instance pools default to
16 connections; separately owned utility pools retain their four-connection
reservation. These are per-pool bounds, not an aggregate capacity guarantee.

The standalone Reborn event-store builder, config/migration CLI pools and
document LISTEN session delegate to this policy. Ordinary composition already
injects its shared pool into durable event/audit repositories. The listener
retains its dedicated session because LISTEN is session-local; it is not another
pool. No SQL retry or uncertain-effect replay was introduced. CLI embedded-owner
retention and aggregate connection/lifecycle accounting remain open Phase-B work.

GitHub run `38136361267` passed 753 composition tests and failed only
`native_webui_cancellation_deadline_keeps_accepted_revision_and_evidence`, with
one preexisting ignored live-provider case. The failure was `WorkerStopped`: an
observer checked worker exit before reading the already durable terminal run
outcome. It now samples worker state before the database read and returns the
terminal disposition first. A finish during that read gets another authoritative
poll. The regression additionally stops the worker and requires readback of the
exact recorded result; recovery fencing and cancellation assertions are retained.

A frozen 9,049-file candidate passed five parsed-policy cases, the event-store
caller case, two public event-store profile cases and three real PostgreSQL
integration cases. Three actual PostgreSQL 17.11 fixtures cover plaintext
rejection, trusted TLS, untrusted issuer/wrong hostname rejection, encrypted
server transport and LISTEN/NOTIFY. Actual CLI config and migration callers pass
against certificate-validated TLS; remote plaintext is explicitly rejected. The
native cancellation regression and global matched/unmatched multi-turn case
pass with the retained worker pair. Relevant strict library and integration
lints pass with toolchain 1.96.0 and no warning suppression. This is local macOS
evidence; it does not establish other-platform runtime behavior or full v3
completion. The corrected CI suite still needs its own result.

The source receipt, compiler-artifact hashes, paired worker, accepted result and
logs are retained in `target/validation/pg-policy-artifacts-20261011-r2`. Changed
source/dependency hashes match the accepted snapshot before submission. Concurrent
authored-catalogue/review/export work present in that snapshot is not certified
or submitted by this checkpoint.

### 2026-10-11 — Management database ownership and retention acceptance

Standalone config, maintenance, repair, secrets and model-list commands now
retain their embedded PostgreSQL owner until their awaited database operation
finishes. Success, early returns and errors explicitly close the utility pool
and stop only a server owned by that command. External and reused servers remain
under their original owner. Operation and shutdown errors are both preserved;
shutdown failure does not authorize replay of a completed operation. Failed
bootstrap after a confirmed server start explicitly stops it, and a subsequent
owned start finishes idempotent role/database creation on the initialized cluster.
Database names are quoted SQL identifiers and encoded DSN values. Newly initialized
clusters retain initdb's loopback authentication rules; configuring another listen
address no longer adds password-free network trust. Existing operator-managed
clusters are not rewritten by this checkpoint.

The actual `models list` caller loads authoritative provider definitions and
active selection from PostgreSQL. Corrupt definitions and connection failures
remain errors without a file/compiled-provider fallback. Other legacy provider
administration paths still need their own DB-only cutover.

The actual retention command exposed incorrectly typed interval parameters and
nonexistent event/audit timestamp columns. Its sweep now uses bigint day values
and `occurred_at`, and commits deletions/link updates in one transaction. A late
forensic deletion failure rolls back earlier deletions and memory-link changes.
Retention settings resolution, active checkpoint retention and the complete
instance pool/listener shutdown and aggregate-capacity requirements remain open;
this checkpoint does not certify them.

Frozen candidate `target/validation/pg-lifecycle-20261011-r4` passed all four
real CLI lifecycle/provider/retention cases, seven actual embedded integration
cases, the embedded unit suite and the focused model-list smoke. Relevant
library, caller and embedded integration strict lints passed without suppression
on toolchain 1.96.0. No test in these suites was ignored. Retained executable
artifacts, worker pair, source hashes, accepted receipt and failed predecessor
logs are in `target/validation/pg-lifecycle-artifacts-20261011-r4`. Eighteen owned
source/dependency files matched this candidate before submission. These are local
macOS caller checks, not full-platform or whole-plan acceptance.

The Monty settings form now supplies English/German explanations for all 29
execution controls and five memory values, with accessible description links.
Syntax checks and actual form rendering verified every translated description
and input association. This establishes help-text rendering, not new runtime
limit enforcement or a complete system-wide fixed-value audit.

### 2026-10-11 — Complete selected definition-prefix compiler reuse

The retained Recipe loader now submits its complete, already-qualified,
dependency-first definition set to the contained worker as one preload operation.
The worker verifies each source checksum and live source bounds, rejects aliases,
effectful initializers and duplicate/reserved names, and compiles one immutable
prefix. Compatible empty contexts share its compiler seed; mutable globals,
defaults, inputs, results and host bindings remain owned by each task context.
Invocations remain separate pinned execution steps. No approval or Tool permission
is inferred from a cache hit, and no runtime input becomes Python source.

The worker retains the previous per-definition feed and compiled-source charges.
The loader advances its definition cursor only after the entire prefix completes
with the expected null result. Used or partially failed contexts cannot reload
the prefix. Weak cache entries disappear when their owning contexts are released;
there is no separate hidden cache-byte ceiling. The shared allocator still
accounts actual live code. The private host/worker protocol is now 14, requiring
a paired upgrade; interpreter extension and dump ABI remain unchanged.

Frozen source `target/validation/preload-prefix-20261011-r2` passed three real
cache cases, five retained PostgreSQL/IBS/kernel execution cases, 21 process-host
cases and 11 child-host cases, with no ignored tests in these suites. Affected
host/Engine libraries and host/contained tests passed strict linting on 1.96.0.
Exact executables, paired workers, source hashes, accepted receipts and failed
predecessor logs are retained in
`target/validation/preload-prefix-artifacts-20261011-r2`. The separate contained
workspace lockfile now includes the accepted PostgreSQL dependency edges.

This establishes complete selected-definition compiler reuse through the retained
worker path. It does not establish activation-time catalogue warmup, qualified
imports, whole-library population, ordinary product performance, cross-platform
runtime acceptance or full v3 completion. Global worker/pending-call constants,
authoring utility limits and the wider fixed-value audit remain open.

### 2026-10-11 — Secret ceremony integrity and portable entropy

Boot and `secrets rewrap` now validate exact AES-256 key material and use the
operating system's secure random source through the existing cross-platform
dependency, replacing direct `/dev/urandom` access. Raw sources must be regular
files and fit the key-file format; canonical and legacy keys must agree.
Management identity comes from PostgreSQL, and failed configuration or encrypted
row queries remain errors. Only filesystem rows explicitly marked encrypted
prevent initialization; ordinary plaintext VFS rows do not manufacture that gate.

Rewrap holds a database transaction/advisory lock while resolving and updating
the existing key ceremony and version. A wrapped ceremony requires the current
passphrase even when stale raw files remain. New raw material is written to an
empty, restricted temporary file, synced and published without overwriting an
existing key. Windows applies account permissions before secret bytes are written.
Wrapping validates its inputs before file mutation. A confirmed wrapped-key
commit precedes cleanup; cleanup reacquires the lock and verifies the exact
current ceremony before removing matching old material. Failures preserve the
committed ceremony and report remaining reconciliation work. Physical erasure
on SSDs, snapshots or copy-on-write filesystems is not promised.

Frozen `target/validation/secret-ceremony-20261011-r1` passed the real subprocess/
PostgreSQL ceremony regression, the existing owned-command lifecycle regression
and seven key-source/crypto/boot tests. Cases include query failure, missing keys
with encrypted data, plaintext VFS state, contradictory file identity, short and
conflicting keys, rejected DB writes, wrong passphrases, stale raw material,
version preservation and symlink rejection. Relevant libraries and caller tests
passed strict linting on 1.96.0. Source hashes and retained artifacts/worker are
in `target/validation/secret-ceremony-artifacts-20261011-r1`. This is local macOS
acceptance; Windows permission execution and deployed Linux acceptance remain
part of the platform/release gates. Concurrent rewrap conflict handling was
reasoned through, not independently fault-injected by these tests.

### 2026-10-11 — Isolated upgrade lock graph and CI reconciliation

CI run 38162471140 on `78d1bdf3e205c6a5f5e3e5817a6e7116dc798508`
passed all 754 composition library cases (one existing ignored case), including
the corrected native cancellation record observer. The full run still failed:
both contained Monty jobs and the isolated upstream-upgrade job rejected stale
lock graphs, and one budget-gate integration fixture failed during retained
source inspection at boot. No full-CI success or resolved boot cause is claimed.

The contained lock graph was repaired in `0c45a615b`. The upstream-upgrade lock
now also reflects the accepted shared PostgreSQL TLS policy: trusted roots and
the required ring edge, without unrelated dependency-version changes. Frozen
`target/validation/upstream-lock-20261011-r2` built and executed all upstream
upgrade test targets with `--locked`, then passed strict target linting on 1.96.0.
The real allocator probe was retained with the test executables before Cargo
exit. Receipts and original CI/failed lock-reconciliation evidence remain in
`target/validation/upstream-lock-artifacts-20261011-r2` and the recorded logs.
This establishes the local isolated graph/compatibility checks; the corrected
cross-platform CI run and retained-source boot failure still require acceptance.

### 2026-10-11 — Retained inspection diagnostics and terminal checkpoint repair

The failed CI run's source-inspection error hid the disposable worker's safe
classification. `RetainedSourceError::Inspection` now includes `UtilityError`'s
classification, without source, stdout or private parser diagnostics. All six
real native-PostgreSQL budget integration callers pass on the frozen committed
candidate with this diagnostic change, and affected Engine library lint passes.
Evidence: `target/validation/source-inspection-artifacts-20261011-r1`, receipt
`/private/tmp/brassclaw-source-inspection-accepted.json`. Local success does not
identify or resolve the intermittent Linux startup failure; CI must observe the
classification before any timeout/allocator explanation is claimed.

A checkpoint omitted the terminal observer's already validated `worker_stopped`
branch while retaining its sample. Restore that exact branch: read durable state
first, return a terminal disposition if present, otherwise reject stopped
admission. Its real cancellation/late-settlement regression passed in the
retained `pg-policy-20261011-r2` candidate (14.32 seconds), alongside strict
affected-package lint; this unchanged evidence is reused. The omitted checkpoint
had an unused-variable warning and incomplete observer behavior; it is not
reported as a working cutover. Other unaccepted catalogue/review edits remain
outside this checkpoint.

### 2026-10-11 — Preparation policy (focused native acceptance)

Disposable parsing, retained source inspection and pure formatter/validator work
must accept explicit preparation limits from the acknowledged instance execution
policy. Share the existing source/value/IPC/deadline settings rather than keep
256 KiB source, 16 MiB frames, five-second wall waits, or a hidden AST depth of
128. Add `utility_worker_memory_bytes` (132 MiB default, strictly larger than the
configured frame) and `max_native_artifact_bytes` (1 GiB default for each capture)
with English/German WebUI labels and explanations. The utility allocator is
separate from the global shared heap; neither ceiling preallocates RAM. Executable
capture streams the actual bytes, rejects oversized images and grants no approval.
Remove the primitive's additional one-GiB rejection so a configured larger
capture is supported. Fixed I/O buffer sizes remain implementation chunk sizes,
not configurable resource ceilings.

Initialize/read instance settings before installation catalogue preparation.
Retain the accepted policy through repeated bootstrap inspection and immutable
approval subject preparation. Later authored generation preparation captures
one acknowledged settings snapshot for the complete operation. Bind its source
after the global settings owner starts, using a weak effective-settings adapter
to avoid a service/catalogue/settings ownership cycle. Existing operations and
retained selections keep their prior limits; new operations use the acknowledged
successor. Include preparation values in desired/effective status only after
normal Rust/worker settings publication succeeds. No restart is needed for new
preparation work. Source inspection remains preflight, not semantic Q1 or approval.

V137 must preserve explicit fields, increase revision once for missing defaults,
size the missing utility default for an already larger IPC frame, reject invalid
numeric values and fail atomically at revision exhaustion. Legacy migration
fixtures must remove this newer constraint when reconstructing an older schema.
Real acceptance must cover migrations, operator HTTP rejection/acknowledgement,
actual parsing of a source above the former 256-KiB ceiling, typed pure evaluation,
immutable old selections, retained execution, artifact integrity and affected
lints. Frozen `preparation-policy-20261011-r2` passed these focused gates:
19 real migration cases, actual WebUI rejection/acknowledgement, Q1 preparation,
global runtime, all six budget integrations, native image checks, utility checks,
five retained-execution cases and affected strict library/caller lint. Receipt:
`/private/tmp/brassclaw-preparation-policy-r2-accepted.json`. This establishes
these preparation limits, not the whole validator/catalogue/settings plan.

Standalone Engine compatibility APIs use the documented default execution policy;
product preparation callers must supply their acknowledged instance policy.
The currently unconnected legacy public Q1 coordinator now requires an explicit
policy argument; this does not establish full validator Recipe activation or
ordinary Q1 delivery. Its complete v3 integration remains a required plan gate.
The wider fixed-value audit (global worker/pending-call capacity, catalogue and
retention capacities, other caches and bootstrap behavioral fixture limits) and
whole-plan/release/deployment acceptance remain open.

### 2026-10-11 — Bootstrap fixture and migration diagnostics (native acceptance)

CI `38171002964` at `863d02ae4d2574937202ee82a6178b759201cff3` passed
755 composition library cases (one existing ignored), all six budget integration
callers and 11 budget-cost cases. It then failed `builtin_bootstrap_seed` while
applying V0. The fixture selected plain `postgres:16-alpine`, supplied no pgvector
installation, and silently passed if Docker initialization failed. V000 requires
`CREATE EXTENSION vector`; installation uses the verified bundled PostgreSQL plus
its packaged pgvector support. This is a separate fixture/prerequisite failure,
not evidence that the earlier intermittent retained-source failure is fixed.

Replace that fixture with the existing native PostgreSQL owner. All real seed
assertions remain, initialization fails visibly, and the production migration
entry supplies the actual `BootedDb` token. Preserve idempotent migration readback
and server/pool teardown order. Expose SQLSTATE or safe connection classification
from refinery's native cause instead of reducing every failure to `db error`.
Do not append private SQL diagnostics, request text or connection details.
Required acceptance includes the complete native seed/reseed/idempotency caller
and an actual read-only-server migration rejection that carries SQLSTATE 25006,
plus affected PostgreSQL and integration-target lint. No fixture skip or manual
successful approval substitutes for these checks. Frozen
`bootstrap-migration-20261011-r5` passed both native tests (5.90 seconds),
PostgreSQL/composition strict library lint and the integration-target lint.
Receipt: `/private/tmp/brassclaw-bootstrap-migration-accepted.json`.
The fixture asserts the actual complete seed inventory by Tool identity and
component class, rather than outdated counts. No Docker/pgvector skip remains.


### 2026-10-11 — PostgreSQL management readers and native verifier policy

Frozen `provider-readers-20261011-r2` passed the actual database reader,
product-command adapter, database CLI list/status and owned-instance lifecycle
regressions. `provider-readers-20261011-r3` changes only the equivalent native
client coercion and passes affected strict library, CLI and adapter lint; its
receipt records exact evidence reuse. Active selection and provider definitions
share one read-only repeatable-read snapshot. Malformed definition identities,
conflicting aliases, unavailable stores and row decoding errors remain errors.
These readers ignore contradictory legacy file/environment selections. This
is reader acceptance, not completed §10: embedded registry/recurring seeder,
remaining boot/auth/embedding/model consumers and provider revision/publication
remain open. Receipt: `/private/tmp/brassclaw-provider-readers-accepted.json`.

Frozen `native-verifier-policy-20261011-r1` passed native WebUI policy publication,
global runtime, actual native image/verifier checks and strict affected lint.
The accepted stdout and response-time settings govern macOS signature verifier
output and execution; oversized output fails instead of truncating. The selected
preparation policy governs artifact captures in installation/review/reply
qualification. English/German setting explanations describe this scope.
Receipt: `/private/tmp/brassclaw-native-verifier-policy-accepted.json`.
Linux descriptor identity is retained; Windows loaded-file identity remains
unsupported and is a required implementation/acceptance gap. File-copy latency
has not been established as bounded by the verifier deadline.

### 2026-10-11 — Atomic provider role publication (native acceptance)

Role provider/model keys must commit or roll back together, including login
completion. Clearing a role removes its stale model override. Kohai and Sempai
may share a provider without sharing prompts, histories or model overrides.
Sempai construction and credential reads precede publication; failed preparation
or database writes preserve both keys and the actual preceding backend. The
existing swappable provider is updated only after commit. Serialize settings and
login publication through the same facade owner; this is not cross-process
provider revision coordination. Expose reload/credential/row decoding failures
instead of returning success or treating unavailable state as empty.

Clearing Kohai must publish the production unconfigured backend through the
existing reload handle, including the cheap slot, while retaining independently
selected Sempai and preceding provider Arcs owned by existing work. Read named
selection and active catalogue from one PostgreSQL snapshot. A missing named
provider is invalid configuration, never a reason to clear the preceding working
backend. Native acceptance must use production migrations, the encrypted secret
store, actual reload adapter and real swappable providers, including deliberate
second-key rejection and credential-store failure. Do not claim model inference,
whole gateway/profile uptake, embedding runtime uptake or full provider revisions
from these checks. Frozen `provider-role-publication-20261011-r2` passes
all ten configuration-library tests, including the native PostgreSQL role test
(3.77 seconds), and strict composition/LLM library lint (28.41 seconds). A CLI
feature variant initially reused stale host-runtime metadata from an earlier
`pg-policy-20261011-r2` snapshot, missing the native-verifier API present in source.
Under the common Cargo lock, rebuilding just that package's artifacts makes the
unchanged frozen source pass strict CLI binary/lifecycle/smoke lint (26.39 seconds).
The r3 receipt retains the exact r2 executable/hash and reused test evidence:
`/private/tmp/brassclaw-provider-role-publication-accepted.json`.


### 2026-10-11 — Uncapped descendant cancellation baseline (focused native acceptance)

Remove the silent 1,000-node cancellation traversal cutoff. Admission already
owns the permitted spawn tree; cancellation must visit all its admitted nodes,
with a visited set to prevent duplicate work or cycles. A terminal intermediate
run does not imply terminal descendants: enumerate its children before skipping
that run's own cancellation. Preserve durable cancellation idempotency, scope
checks and error propagation; this change does not grant Tool permission or
replay work. The set and traversal stack are local and freed at completion.

Extend the existing native PostgreSQL state regression without a model override:
stop the fixture turn worker, admit 1,001 root children plus one grandchild through
the real store, cancel the intermediate child directly, then cancel the root.
Require every root child and the queued grandchild to be Cancelled; do not accept
Failed as successful cancellation. Production shutdown still joins the actual
global Monty service. This exercises state traversal, not a latency or complete
crash-recovery claim. Frozen `complete-cancellation-20261011-r4` passes the native large-tree
regression (1,011.80 seconds including admission setup), cancellation acknowledgement
regression (12.45 seconds) and affected strict library lint (25.68 seconds).
Receipt: `/private/tmp/brassclaw-complete-cancellation-accepted.json`. The
whole-regression time exposes a performance problem; the following atomic
implementation replaces this accepted per-child baseline.

### 2026-10-11 — Atomic tree cancellation and admission fencing (accepted scope)

The uncapped native regression exposes repeated whole-aggregate decoding and
CAS writes during per-child cancellation. Replace the runtime traversal with
`TurnCoordinator::cancel_tree`: the in-memory state machine selects the complete
scope-checked descendant graph and stages root and descendant cancellations on
one private candidate under its admission lock. PostgreSQL and filesystem stores
commit that candidate through their existing CAS boundary once, retrying the
whole state-only transition on conflict. An invalid actor, scope, lineage or
idempotency record leaves the original aggregate untouched. This is host-owned
state authority, not a Rust Recipe runner or replay of Tool effects.

Lifecycle delivery follows the commit, attempts every retained event even if an
observer fails, and keeps original cursors for retry. Cancelled descendants must
remain in retry selection; skipping all terminal runs would lose a failed
cancellation event. Successful cursors are not republished. Normal cooperative
cancellation still uses CancelRequested until runner acknowledgement. New child
admission checks every ancestor under the state lock both before and after
awaited profile resolution. A concurrent PostgreSQL submission either commits
first and is included by the cancellation CAS retry, or loses its CAS and is
rejected against the cancelled ancestry.

Protect terminal intermediate records while descendants still reference their
ancestry. Protected records stay in the pruning queue and become eligible when
children/reservations are removed. Prune once after the atomic cancellation rather
than once per descendant. Extend the native regression to assert exactly one
snapshot version increment for the complete tree and report actual cancellation
time. Add focused complete-tree, durable replay, observer-failure, actor rollback,
retention and awaited-admission race checks. The frozen r2 source passed 136 turn contracts, filesystem contracts, native 1001-child cancellation, acknowledgement and strict affected-package checks (`/private/tmp/brassclaw-atomic-tree-accepted.json`). One durable commit is established; event delivery still took 192.97 seconds because each record lookup decoded the full aggregate. The projection repair below remains required. The earlier
uncapped per-child binary continues as a separate frozen baseline.

### 2026-10-11 — Stored provider credentials and Sempai boot uptake (candidate)

The existing completed-turn review Recipe remains the workflow; this change
repairs role construction in the infrastructure owning its provider boundary.
Resolve encrypted stored credentials before required-key validation, preserving
the provider definition's authentication requirement. Keep legacy environment/file
resolution distinct until section 10's coordinated removal. Explicit DB selections
use their own model and endpoint values; process-wide model/URL overrides cannot
change a selected Sempai or Kohai model. Probes still attach stored credentials
only to the persisted endpoint/adapter, with masked placeholders treated as
unchanged credentials rather than outbound secrets.

Restore Sempai from one consistent DB catalogue/selection snapshot before global
Monty startup, worker admission, review delivery and ingress. Construct the actual
shared provider chain before swapping; no settings write, UI service invocation,
provider seeding or external inference is needed for restoration. An invalid
named selection or credential-store error remains explicit. A cleared role
restores the unconfigured backend and routing mode. Native acceptance must prove
ordinary startup/restart without a model override or WebUI constructor, stored
required-key construction for both roles, rejection without replacing a retained
backend, and independence of role model choices. This does not complete provider
revisions/soft-deactivation, gateway-profile/cost publication, embedding uptake,
DB-only CLI boot, inference or prefix warmup. Acceptance is pending.


### 2026-10-12 — Cancellation projections and private secret lease repair (candidate)

Native atomic-tree acceptance exposed full-aggregate PostgreSQL decoding for every
completion-observer record lookup. Replace only that lookup with a committed,
child-link-aware projection of the selected run. Validate its typed record and
exact scope; detect duplicate UUIDs rather than choosing an arbitrary row. Keep
mutation CAS, lifecycle event delivery, cancellation fencing and durable state
unchanged. The unchanged >1000-child production fixture must establish one commit,
complete cancellation, scoped projection isolation and duplicate rejection, and
report delivery time separately from admission setup. No performance claim before
this native measurement.

The credential-aware provider fixture also exposed failure to deserialize the
reserved system scope from encrypted PostgreSQL lease storage. The private,
authenticated lease decoder may reconstruct only the exact reserved system pair
with no agent/project/thread; public identity deserialization remains strict.
Consume/revoke must lock and authenticate the retained lease and verify the complete
invocation scope before deletion. A wrong-scope attempt must not burn the valid
lease. Concurrent consumers must yield exactly one material handoff. Scope listing
must return actual authenticated active records, not an empty placeholder. Retain
stored provider material across reloads. Remove the obsolete delayed credential
overlay rather than suppressing its unused-function warning.

Required acceptance includes real PostgreSQL leases, repeated required-key provider
reload, Sempai startup before ingress, retained global Monty identity, affected
contract and strict lint checks. Existing provider file/environment boot fallback,
coordinated deactivation/revisions and prefix warmup remain required under section
10; this repair does not establish their completion. Review the older secret row
scope discriminator (missing user identity) and system-scope physical-tenant AAD
before declaring complete storage isolation; implement compatible retained-row
migration rather than silently discarding existing encrypted values.


### 2026-10-12 — Atomic provider deactivation (candidate)

Provider origin no longer blocks removal. The existing PostgreSQL repository
locks the active definition and atomically soft-deletes it with all selections
that reference its canonical ID or aliases (Kohai, Sempai, embedding and other
retained LLM slots), including model/URL/env overrides. Retain the original row
and encrypted credentials. The live service updates Sempai and primary roles
after commit; retained calls retain their preceding provider Arc. A rejected
selection deletion must roll back the entire operation without a live swap.
The remaining historical builtin seeder cannot reactivate a soft-deleted row;
its removal and replacement by the one-time database initialization are still
required under section 10. The UI permits removal of initial and selected
providers, with a short retained-data explanation.

This slice still requires native rejection/success and restart acceptance. It
is not the final immutable/revision-checked provider editing or cross-writer
publication protocol: complete that protocol, remove recurring JSON seeding,
provide explicit DB-backed reactivation and acknowledgement state, and test
concurrent selection/deactivation before closing section 10.


### 2026-10-12 — Committed run projection acceptance (limited native scope)

The retained r2 composition executable passed required-key role publication
(10 tests, 2.69 seconds), actual Sempai boot/restore (15.80 seconds) and the
unchanged native >1000-child cancellation fixture (211.89 seconds total).
Cancellation/event delivery itself measured 1.885918 seconds versus 192.973516291
seconds for atomic-tree r2 before projection, with one durable snapshot commit
in both. Exact child/parent scopes, foreign tenant/agent isolation and duplicate
record rejection passed against PostgreSQL. Admission setup dominates the full
fixture and remains a separate optimization question. This is not a claim about
WebUI settings write latency or provider inference throughput.

The new standalone lease test initially rejected its 26-byte fixture master key
before exercising behavior. It has a valid >=32-byte test key in r3; do not count
its r2 result as lease acceptance. R3 additionally changes provider deactivation,
so its own native and lint acceptance remains required. Frozen source, retained
executables and partial r2 results are recorded under
`target/validation/provider-boot-credentials-artifacts-20261012-r2` and
`/private/tmp/brassclaw-provider-boot-r2-continuation-accepted.json` once the final
retained global-root test completes.


### 2026-10-12 — Deactivation and scoped lease acceptance

Frozen provider-deactivation r3 passes the real encrypted PostgreSQL lease
scope/list/revoke/concurrent-consume fixture (3.16 seconds), ten role service
checks including rejected and successful deactivation (2.87 seconds), actual
Sempai startup/deactivation/restart (19.93 seconds), retained global-root
acceptance (16.65 seconds), strict affected libraries (30.53 seconds) and CLI
consumers (33.55 seconds). Native checks confirm builtin provenance does not
block removal, aliases and embedding selections clear atomically, keys remain,
startup synchronization cannot revive the removed row and retained calls keep
their original backend. Receipt: `/private/tmp/brassclaw-provider-deactivation-accepted.json`.
The archive copy had a filename mismatch after all checks passed; receipt,
source and executable hashes were verified before repairing archive packaging.
This acceptance does not cover later primary-boot changes.

### 2026-10-12 — Primary database boot selection (accepted startup slice)

Resolve the default slot, exact provider definition, encrypted key and model
properties after service/database bootstrap and before model gateway construction.
Selection and context/cache properties must come from the same repeatable-read
catalogue snapshot. Remove CLI file/environment construction from the production
runtime-input builder, embedded context/cache fallback and swallowed catalogue
read errors. No selected primary yields the existing unconfigured gateway;
a named missing/inactive provider or database failure stays an explicit error.
Validate positive declared context windows and supported cache-retention values;
invalid stored values cannot quietly fall back to a different policy.

Extend the actual no-WebUI startup fixture to assert primary and Sempai selections
before ingress, repeated startup, a deliberately unavailable provider table,
retained desired selections after that failure and both cleared after deactivation.
Run DB resolver properties and affected runtime/CLI lint checks. Complete database
initialization, identity migration, legacy file import/removal, model-specific
revisioned specifications, live gateway metadata and operator recovery remain
required under section 10; do not mistake this startup slice for the full cutover.

Frozen r4 passes catalogue resolution and property checks, real encrypted lease
and role publication checks, primary/Sempai startup and explicit database failure
(21.10 seconds), retained global Monty identity (16.66 seconds), and strict affected
library/CLI lint checks. Receipt: `/private/tmp/brassclaw-primary-db-boot-accepted.json`;
retained source and executables: `target/validation/primary-db-boot-artifacts-20261012-r4`.
This establishes the ordinary PostgreSQL startup path, not embedding setup,
catalogue initialization or complete live provider revision publication.

### 2026-10-12 — Embedding catalogue/credential transport repair (candidate)

Embedding maintenance must distinguish an absent role from a broken selected
provider, unavailable database, missing retained master key or authentication.
Read selection and catalogue in one repeatable-read snapshot; resolve aliases
and encrypted keys using the canonical provider ID. Retain the runtime secret
store's physical owner separately from the chat-memory data owner. Maintenance
must read the existing serve master key, never generate a replacement. The CLI
PostgreSQL path uses the same `home/db` root as serve's hybrid composition.

Preserve the selected database endpoint for OpenAI-compatible, NEAR and Ollama
adapters. Normalize an existing `/v1` and trailing slash without duplicating the
API prefix. Optional-key endpoints send no invented Authorization header; a
required missing/blank key fails explicitly. Unsupported embedding adapters
are errors rather than implicit OpenAI dispatch. Endpoint credentials, queries,
fragments and prohibited literal hosts are rejected before HTTP.

Validate batch input lengths, output cardinality, dimensions and finite values;
OpenAI-format replies use their explicit indexes to preserve input/vector
correlation and reject duplicates/out-of-range indexes. See the
[OpenAI response contract](https://developers.openai.com/api/reference/resources/embeddings).
Ollama receives `truncate: false`, following its
[explicit truncation control](https://docs.ollama.com/api/embed).
A failed body read remains an error. Backfill rejects nonpositive batch sizes
before secret/provider/indexing work. The existing cache consumes its actual
config field instead of suppressing an unused setting warning.

Acceptance requires actual PostgreSQL encrypted keys and HTTP transport checks,
including required/optional authentication, aliases, distinct data/secret owners,
custom endpoints, ordered batches, disabled versus broken selection, deletion,
unavailable database and invalid configuration. The controlled protocol endpoint
establishes transport correctness, not model inference quality. Full model-owned
dimension/input/cache settings, live embedding uptake and the instance-wide
provider ownership/initialization cutover remain required under section 10.

Frozen embedding r1 passed adapter contracts (2.49 seconds), real PostgreSQL
plus HTTP transport (3.60 seconds), primary/Sempai startup (21.09 seconds),
retained global root (16.50 seconds), facade/secrets filesystem boundaries and
strict affected library/CLI lint checks. Receipt:
`/private/tmp/brassclaw-embedding-accepted.json`; retained source/executables:
`target/validation/embedding-catalogue-artifacts-20261012-r1`.
This does not cover the subsequent CLI custom-owner correction. `serve` threads
configured installation identity into its physical secret-store owner;
maintenance now reuses the same identity resolver instead of assuming its
fallback owner. A real binary/PostgreSQL credential fixture must pass for that
follow-up. A dev-only secrets dependency prepares genuine encrypted fixture
rows; the normal CLI dependency boundary remains unchanged and checked.

Embedding CLI custom-owner r2 passes the real binary/PostgreSQL fixture (4.77
seconds), the normal-versus-dev dependency boundary (0.27 seconds) and strict
CLI consumers (4.07 seconds). Receipt:
`/private/tmp/brassclaw-embedding-cli-accepted.json`; archive:
`target/validation/embedding-cli-owner-artifacts-20261012-r2`.
The build ran in the stable workspace to reuse Cargo artifacts; all tracked and
nonignored source hashes were checked against an immutable archive before and
after each check. The provider transport/library evidence is retained from r1;
this follow-up changes CLI owner wiring, its genuine encrypted fixture and the
dev-only dependency contract. No deployment inference or complete section-10
acceptance is claimed.

### 2026-10-12 — PostgreSQL secret owner keys (candidate)

The historical secret discriminator omitted logical tenant/user; metadata and
delete checked existence without authenticating ownership. New secret keys
include the complete owner/handle tuple using the existing length-prefixed AAD
and a digest. New secret and lease envelopes additionally authenticate the
physical store tenant and exact row identity. Metadata, replacement, deletion
and consumption authenticate material before proceeding. Legacy secret access
upgrades the authenticated record transactionally, preserving creation time and
master-key version; conflicts, corrupt records and unknown versions remain
visible errors. An old owner-incomplete record belonging to another owner must
be reconciled by its actual owner; it is never overwritten by the new caller.
Existing legacy leases remain readable until consumed or revoked. Historical
envelopes cannot retroactively gain physical-owner binding before upgrade;
credential-account/session envelopes still require their separate repair.

Native acceptance must cover distinct logical users/tenants with the same
handle, cross-invocation material reuse, physical-store ciphertext copying,
wrong-key replacement rejection, authenticated legacy migration, injected
transaction rollback and retained one-shot legacy leases. This is a candidate
until those checks and affected caller/lint checks pass.

Secret-owner candidate acceptance passed against the retained source
`target/validation/secret-owner-20261012-r1` and paired artifacts; receipt:
`/private/tmp/brassclaw-secret-owner-accepted.json`. Native isolation/legacy
rollback took 4.109 seconds; one-shot leases 1.911 seconds; embedding credentials
2.634 seconds; primary/Sempai startup 20.788 seconds. Secret contract suites and
strict affected library/CLI lint passed. The physical-owner limitation for
historical envelopes and separate credential broker work remain explicit.

### 2026-10-12 — WebUI provider attachment (candidate)

WebUI construction now attaches the existing live reload adapter without
rebuilding the primary provider a second time. Runtime startup remains the
sole boot selection owner. Native acceptance changes the desired database model
between startup and facade construction, checks that the admitted model and
Sempai Arc remain unchanged during attachment, and then verifies an explicit
settings selection publishes the new primary without replacing Sempai.

WebUI attachment native acceptance passed against
`target/validation/provider-attachment-20261012-r1`; receipt:
`/private/tmp/brassclaw-provider-attachment-accepted.json`. Startup/facade/live
publication took 23.720 seconds, role publication 2.782 seconds, retained global
Monty 16.412 seconds. Strict composition/CLI lint passed. This removes a
redundant publication, not the outstanding revision/model-metadata cutover.

### 2026-10-12 — Credential broker atomic use and envelopes (candidate)

PostgreSQL session consumption now locks the row, authenticates its exact
identity, checks expiry and remaining uses, increments without integer wrap
and commits before returning. Validation includes spent-use limits. Reissuing
an identical session retains its original counter; conflicting issuance fails.
New credential account/session envelopes bind physical store and row identity,
with an explicit envelope marker separate from master-key version. Existing
logical-owner envelopes remain readable and upgrade on authenticated writes or
consumption. Wrong keys cannot overwrite a retained account. Private trusted
storage readers support the reserved system owner without loosening public
ResourceScope deserialization. Historical envelopes retain their original
physical-owner limitation until upgraded. Native acceptance must observe two
consumers blocked on the actual database lock, exactly one final use, preserved
limits after reissue, copied-envelope rejection, legacy compatibility, injected
update rollback, expiry and system-owner round trips.

Credential session/envelope acceptance passed against
`target/validation/credential-sessions-20261012-r1`; receipt:
`/private/tmp/brassclaw-credential-sessions-accepted.json`. The native broker
concurrency/envelope/legacy/rollback fixture took 3.347 seconds. Retained secret
upgrade and one-shot checks, embedding credentials, startup, storage contracts
and strict affected library/CLI lint passed. Historical envelopes gain their
physical-store binding on authenticated upgrade, not retroactively.

### 2026-10-12 — Selected cache policy and cache correctness (candidate)

Explicit catalogue resolution now uses the stored prompt-cache retention and
rejects invalid values, independently of Anthropic cache environment overrides.
OAuth material remains a separate credential input. Legacy environment-selected
configuration keeps its explicit environment behavior. In-memory response caches
retain nothing when capacity or TTL is zero; TTL begins when the answer becomes
available. Native acceptance covers the actual PostgreSQL selection reader and
real OpenAI HTTP requests to a controlled protocol endpoint, including a response
slower than its configured cache TTL. These repairs do not complete database-owned
chain policy, live cache controls, gateway model metadata or deployment inference.

Selected cache-policy acceptance passed against
`target/validation/selected-cache-policy-20261012-r1`; receipt:
`/private/tmp/brassclaw-selected-cache-accepted.json`. The actual HTTP cache
regression took 2.215 seconds and ordinary startup/WebUI publication 20.286
seconds. PostgreSQL cache-policy selection, library cache/resolution contracts
and strict composition/LLM/CLI lint passed. Live cache controls remain open.

### 2026-10-12 — Live model dispatch selection (candidate)

The ordinary primary gateway carried a fixed boot-time model override even
after its provider wrapper changed. The repaired execution policy captures
backend, canonical catalogue identity and selected model under one wrapper lock
before dispatch, retaining that backend throughout the exchange. New calls use
the acknowledged live selection; explicit profile overrides remain explicit.
Sempai and completed-review gateways carry the selected catalogue identity too.
Model-call sequence allocation is shared across requests and fails rather than
wrapping. Native acceptance must send ordinary chats through global Monty to
real controlled provider endpoints before/after a WebUI selection and verify
request models and retained VM identity. Per-task durable model/design revisions,
gateway technical metadata and pricing publication remain separate open work;
the historical boot pricing table is not claimed live by this candidate.

Live dispatch selection acceptance passed against
`target/validation/live-model-selection-20261012-r1`; receipt:
`/private/tmp/brassclaw-live-model-accepted.json`. The ordinary Monty/HTTP/live
WebUI regression took 9.182 seconds, startup/WebUI 19.897 seconds, role publication
2.766 seconds and retained global-root regression 15.671 seconds. All affected
gateway contracts, named provider snapshots and strict library/CLI lint passed.
The same VM served both exchanges and the retained first backend was not replayed.
This is per-exchange selection evidence; durable per-task model/design selection,
revision-aware pricing, context metadata and complete cache/settings publication
still need their own implementation and acceptance.

### 2026-10-12 — Coherent provider settings view (candidate)

The WebUI settings service reads provider definitions/origin metadata and role
selections from one read-only repeatable-read transaction. It releases that
connection before credential metadata checks. Selected aliases resolve to the
canonical active provider ID for each role; a named missing provider is a visible
error, not an unconfigured selection. Required keys remain editable even when
the definition has no environment reference, and blank environment values do
not report a configured key. Native acceptance races atomic definition/model
publications against settings reads and verifies all three roles belong to the
same observed generation, plus alias handling and database failure. This read
repair does not substitute for immutable model/provider revisions or coordinated
desired/effective publication.

Provider-view acceptance passed against
`target/validation/provider-settings-view-20261012-r1`; receipt:
`/private/tmp/brassclaw-provider-view-accepted.json`. Concurrent definition/model
view checks took 4.145 seconds, ordinary live-model dispatch 6.977 seconds and
role publication 2.816 seconds. Strict composition/CLI lint passed. The next
focused fixture additionally constrains the same real PostgreSQL pool to one
connection, to verify the documented connection-release behavior before
credential metadata reads; that extension is not covered by the r1 receipt.

The r2 provider-view fixture passed with the actual pool constrained to one
connection before credential checks, and retained the concurrent-generation
checks after restoring pool capacity. Receipt:
`/private/tmp/brassclaw-provider-view-accepted.json` now identifies
`target/validation/provider-settings-view-20261012-r2` and paired artifacts.
Native acceptance took 4.141 seconds; strict composition/CLI lint passed.
The retained r1 artifact directory preserves the earlier receipt.

### 2026-10-12 — Provider revision storage (candidate prerequisite)

V138 preserves existing provider configurations as revision 1 and records
subsequent definition/origin/activity changes in immutable history. Identity and
revision assignment are database-owned; revision exhaustion fails closed.
Updates, deletion and truncation of retained history are rejected. The exact
compare-and-replace repository seam is currently a native development-test seam,
while the schema records production legacy writes too. It is not evidence of
revision-aware HTTP/CLI publication, reactivation UI, immutable model specification
records, coordinated credentials or desired/effective runtime acknowledgements.
Those consumers must be wired before the coordinated provider cutover. Native
acceptance covers migration preservation, history immutability, stale concurrent
replacement, explicit reactivation and failure rollback. The provider dialog's
missing Select import is also repaired; it otherwise prevented custom-provider
creation from rendering.

The first V138 native batch passed revision/migration preservation, stale
replacement, history immutability and rollback, plus ordinary Monty and provider
view regressions. Its JavaScript run exposed two ignored optional NEAR wallet
callback contracts; the batch is not accepted. Those components now render a
wallet action only when the caller supplies the real handler and invoke that
handler. The shared shipped login hook still has no wallet signer, so wallet
end-to-end browser wiring remains open; this conditional UI contract is not
wallet authentication acceptance. Negative component cases require no wallet
action when no handler is bound. Retain the failed r1 diagnostics and validate
the coherent successor source before accepting the batch.

Provider revision r2 acceptance passed against `target/validation/provider-revisions-20261012-r2`; receipt `/private/tmp/brassclaw-provider-revisions-accepted.json`. Native revision checks took 4.067 seconds; ordinary Monty live dispatch, coherent settings, role publication, 24 JavaScript cases and strict library/CLI lint passed. Wallet authentication remains outside that UI callback acceptance.

### 2026-10-12 — Coordinated provider setup writes (candidate)

CLI run/repl now carries its configured instance owner into the PostgreSQL build input. A neutral encrypted setup-write primitive can participate in a composition-owned transaction; it authenticates existing material and serializes absent-row writers before insertion. Provider edits use exact definition revisions and commit definitions, encrypted keys and requested primary role keys together. Failed database writes do not perform blind compensating writes over newer publications. Settings carry the revision from the same catalogue/config snapshot and the browser sends it with edits. This candidate still requires native acceptance; definition revisions alone do not claim immutable credential revisions, complete role desired/effective publication, inactive-provider selection UI or full DB-only cutover.

The frozen atomic-setup r1 caller checks passed: ordinary Monty/HTTP provider edits and deliberate database failures (7.316 seconds), revision storage, coherent settings, role publication, HTTP contracts and strict library/CLI lint. The separate absent-row competing-writer fixture failed before dispatch because its test master key was too short. This is not complete acceptance of the neutral setup-write primitive. The successor corrects fixture material and must run that native case plus existing owner/lease checks. Retain r1 source/artifacts and the failed fixture output.

### 2026-10-12 — Revision-checked provider deactivation/reactivation (candidate)

Operator deactivation now requires the displayed revision through HTTP/facade/store. The row is locked and checked before any role changes. Settings return inactive definitions separately from the active routing catalogue; the WebUI offers those definitions for explicit review and re-addition using the same ID/revision. Reactivation validates endpoint/adapter/authentication and does not restore previous role selections. Origin is metadata; either origin can edit its adapter and other supported fields. Native acceptance must cover stale removal, failed setup writes, retained keys, same-row reactivation and another ordinary chat through the original global VM. Full DB-only startup migration, immutable credential/model properties and coordinated desired/effective role publication remain separate open requirements.

Provider reset no longer reports a removal as a successful reset-to-defaults. That duplicate legacy-overlay action is retired; the actual removal path explains retained configuration/credentials and role clearing. Unsupported UI callback behavior is not reported as reset acceptance. Re-addition is explicit through the retained-definition dialog.

Provider lifecycle acceptance passed against `target/validation/provider-lifecycle-20261012-r1`; receipt `/private/tmp/brassclaw-provider-lifecycle-accepted.json`. The corrected absent-row writer case took 3.984 seconds; owner migration/one-shot leases, actual revision history, coherent settings and role publication passed. The ordinary global Monty/HTTP sequence with stale edit/removal, transaction rejection, same-ID reactivation and third chat passed in 7.625 seconds. The same root served all three exchanges. Twenty-five component cases, actual HTTP extraction/dispatch contracts and strict library/CLI lint passed. The CLI owner handoff is compiled/source-traced; this receipt does not claim a fresh-install CLI owner migration or full test-machine acceptance.

### 2026-10-12 — Response-cache context isolation (candidate)

Cache keys previously omitted request metadata and used ambiguous concatenation for stop sequences. Metadata can carry provider chaining/task scope and prefix/component selection. The successor hashes one structured versioned tuple, sorts metadata by key and preserves temperature bits; serialization errors remain visible rather than omitting messages. Native HTTP acceptance must distinguish owner and prefix-generation metadata, retain hits across map insertion-order changes and distinguish embedded separators in different stop lists, while preserving zero-capacity/TTL and completion-based TTL behavior. Cache policy controls and full model/property publication remain open.

### 2026-10-12 — Live response-cache controls (candidate)

Provider definitions carry an explicit `response_cache` policy (enabled, entry capacity, lifetime seconds). Compatible historical definitions have the existing disabled/1000-entry/3600-second defaults; operator saves persist the exact policy in the revisioned DB definition. The explicit DB builder ignores cache environment overrides, including malformed values, while the separate legacy resolver remains to be removed in §10. Primary and selected Sempai chains rebuild through their existing role ports after a successful provider edit; retained old backends remain owned by earlier calls. Both zero capacity and zero lifetime disable retention. Settings → Inference → provider configuration exposes the controls with English/German explanations and defaults supplied by the API. Null policy updates are rejected rather than silently resetting/preserving them. Existing builtin merge temporarily preserves the cache policy; the recurring seeder/other structural-field overwrite path still needs the coordinated DB-only replacement.

A local response-cache hit now reports zero upstream token/cache usage instead of billing the original call twice; cached content/reasoning remain available and hit counters remain observable. Required native acceptance covers real DB/WebUI provider writes and HTTP primary/Sempai chains, LRU capacity, disable/zero settings, expiry, retained old chains, rejected unbounded values, startup restoration and malformed cache environment variables. Separate cache-key acceptance covers complete metadata/list framing, deterministic map ordering and completion-based lifetime. This does not certify pricing/model properties, whole-task model selection or remote model-quality/prefix acceptance.

Live cache-policy r2 acceptance passed against `target/validation/live-cache-policy-20261012-r2`; receipt `/private/tmp/brassclaw-live-cache-policy-accepted.json`. Real WebUI/DB primary/Sempai policy changes and startup restoration took 13.900 seconds; actual HTTP metadata/list framing and billing/lifetime checks took 2.223 seconds. Ordinary global Monty edits/removal/reactivation, library cache/resolution contracts, HTTP extraction/dispatch, 25 component cases and strict library/CLI lint passed. The r1 fixture's trait-interface compile error was repaired using the existing `retained_provider()` contract; no production fallback or warning suppression was added. Actual browser rendering and remote model-quality acceptance remain open.

### 2026-10-12 — Database-owned provider initialization (candidate)

V140 contains the versioned first-party initial data and an initialization marker. Its schema-bound initializer seeds a genuinely empty, uninitialized provider namespace once; if any provider rows already exist, it retains the inventory unchanged, including custom records and tombstones. Later boots only read the marker, never synchronize against a second catalogue. It touches no role selections or credentials and records immutable initialization provenance. A table lock protects the initial empty-inventory decision against concurrent operator insertion. Composition and the management read port call this one SQL-owned primitive before their actual namespace's first consumer. The WebUI recurring Rust/JSON seeder is removed; historical V047/V048 remain untouched. The legacy builtin writer is test-only. Legacy file/ENV readers/imports and CLI/runtime namespace reconciliation remain explicit §10 work; this candidate does not claim the full DB-only cutover or secured legacy-file migration. Native acceptance must cover fresh defaults, existing/tombstoned inventories, concurrent initialization, rollback and no resurrection or structural-field reset on later boots.

The first initializer batch stopped before its assertions because another new migration already used version 139 (`prefix_profile_registry`). The unapplied provider initializer is now V140. Retain the failed r1 schema diagnostic and source. Successor validation compares the exact source inventory as well as hashes, so a newly added source/migration cannot enter a build without appearing in its frozen input manifest. Historical accepted receipts cover their recorded relevant files/artifacts; their older verifier did not independently detect additional source files. The successor must cover the current complete migration inventory before a release claim.

Provider DB initialization r2 passed against `target/validation/provider-db-init-20261012-r2`; receipt `/private/tmp/brassclaw-provider-db-init-accepted.json`. The complete source inventory includes V139 prefix-profile metadata and the uniquely numbered V140 provider initialization. Native initialization/schema binding/rollback/provenance took 4.058 seconds, the actual CLI database readers 4.525 seconds, live cache/restart 14.030 seconds and ordinary Monty edits/removal/reactivation 7.247 seconds. Revision storage, role publication, coherent settings, HTTP/UI contracts and strict library/CLI lint passed. No source files were added or changed during that fenced batch. Remaining file/ENV readers, secured upgrade imports, namespace reconciliation and general desired/effective provider publication still prevent a full §10 cutover claim.

### 2026-10-12 — Secured legacy provider inventory import (candidate prerequisite)

The upgrade importer previously wrote the nonexistent `data` column, committed each provider independently, overwrote conflicts and replaced an existing backup. The successor uses the actual typed definition contract and one domain transaction; matching existing records retain origin/revision/tombstones, conflicts reject the whole inventory, and new aliases are checked against active DB identities. Dry-run validates the same inventory without writes. A private exact-byte backup is staged, synchronized and published without clobbering an existing backup before DB commit; incomplete writes cannot publish a partial final backup. Existing non-regular backup paths are rejected. The original source remains an upgrade artifact, including after a successful import. A changed source is retained and reported rather than deleted after commit. The original importer still needs its final controlled-upgrade extraction/removal after inventory preservation acceptance; it is not a runtime registry or evidence of completed §10 file removal. Required native checks include multi-record rollback, definition/alias/backup conflicts, tombstone retention, malformed/inline-secret input, dry-run, private byte-identical backup and replay after a commit whose acknowledgement is lost.


Secured provider import acceptance passed against `target/validation/provider-import-20261012-r1`; receipt `/private/tmp/brassclaw-provider-import-accepted.json`. The real PostgreSQL multi-record/dry-run/conflict/tombstone/backup replay checks took 4.177 seconds and strict migration-feature lint passed. The explicit `brassclaw migrate` command is the current caller; ordinary startup does not invoke this importer. Original source and content-addressed backup remain upgrade artifacts. A crash or lost commit acknowledgement can safely repeat the exact inventory without revising matching rows; final controlled extraction/removal and broader legacy upgrade preservation remain open.

### 2026-10-12 — CLI installation namespace and atomic config import (candidate)

CLI model readers, config CRUD/import/export/init, repair and explicit migration now resolve the same validated bootstrap installation namespace as serve/run instead of independently guessing `default`. Malformed local bootstrap configuration is an error; explicit legacy `--tenant` access remains visible and does not move or overwrite other namespaces. Embedding maintenance defaults its data owner to the installation namespace while retaining an explicit data override and the installation's separate physical secret owner. This repairs current command divergence; it does not claim the final single-instance DB identity migration or automatic cross-namespace credential migration.

Config import uses the existing atomic config publication primitive, validates the whole change set, rejects conflicting flat/nested references and propagates array-serialization failures. Export/show-all use TOML serialization rather than Rust debug escapes; control characters and quoted keys round-trip. Native acceptance must cover the actual binary, custom installation identity, retained legacy namespace, late database rejection, valid export/re-import, malformed bootstrap config and existing database ownership/embedding commands.


CLI namespace/config acceptance passed against `target/validation/cli-namespace-20261012-r1`; receipt `/private/tmp/brassclaw-cli-namespace-accepted.json`. All seven actual-binary/native-PostgreSQL lifecycle tests passed in 25.446 seconds, including custom namespace, legacy explicit data access, transactional rejection, control-character TOML round-trip, malformed bootstrap config, secret ceremony and embedding maintenance; strict CLI lint passed. Full DB-only identity migration and general desired/effective provider publication remain open.

### 2026-10-12 — Adjustable catalogue preparation and retained policy (candidate)

V141 adds five revisioned execution controls: catalogue Recipe count, component references per Recipe, variants per Recipe, catalogue bytes and activation actor bytes. Settings → Monty VM exposes each with EN/DE explanations. New activation/deployment preparation captures one acknowledged policy before taking its DB transaction; this avoids nested settings/ownership pool acquisition while retaining a pooled client. Checks reject excessive selections before effects and use checked aggregate byte accounting. A committed activation retry returns its original receipt regardless of later admission caps and releases its DB client before restoration.

New `authored-monty-catalogue/2` manifests retain the exact preparation limits. Restoration and deployment inspection parse/qualify with that retained policy instead of a newer, smaller admission policy. Existing component/association/Tool identity checks remain independent. Historical format 1 keeps its existing compatibility behavior and does not acquire unrecorded policy provenance; the controlled legacy/installation upgrade and durable crash recovery gates remain open. This candidate needs real migration/default/preservation/rejection checks, actual settings acknowledgement plus retained authored restoration/replay, component deployment and ordinary global execution before acceptance. The catalogue document's structural contract remains distinct from token accounting and retains the validator's supported recursion constraints; this is not a claim that every remaining system cap is adjustable.


The catalogue-policy r1 build stopped on two new deployment error adapters using a string-only helper for typed errors. The successor uses fixed boundary diagnostics, preserving unavailable settings as a service error and serialization failure as invalid input. No r1 behavioral checks ran; its frozen source/compiler diagnostics remain retained. The complete successor batch must pass before a catalogue-control acceptance claim.


The catalogue-policy r2 build stopped before native tests on concurrent prefix-generation facade fields (`interceptor_config_service` versus the actual `interceptor_config`). This is not catalogue-policy acceptance. The other chat is repairing its owned prefix slice and uses the independent `brassclaw-target2`; build-slot coordination is not required. Successor catalogue checks compile a frozen source copy so shared checkout edits cannot enter the compiler after capture. Original r2 diagnostics are retained; r1 diagnostics were captured from actual tool output after its shared-name log was overwritten. Successor build logs use revision-specific names.


Frozen catalogue-policy r3 stopped before behavioral checks on a prefix-source fixture expression inside `json!`. The other chat independently corrected the source list into a Rust binding before the proposed local patch applied; that failed patch changed no file. Its actual compiler diagnostic is retained with r3 artifacts. No r3 acceptance is claimed. Subsequent frozen validation uses a stable, isolated compilation workspace plus immutable archived inputs, preserving incremental package identities without compiling the concurrently edited checkout.


Catalogue policy r4 acceptance passed against archived `target/validation/catalogue-policy-20261012-r4`, compiled from the isolated `target/validation/catalogue-policy-workspace`; receipt `/private/tmp/brassclaw-catalogue-policy-accepted.json`. New migration/default/preservation/rejection checks took 4.124 seconds; acknowledged cap reductions with exact retained restoration/replay took 7.017 seconds; component deployment (including the then-current quarantined compiler-package storage caller) took 7.311 seconds; ordinary global matched/unmatched runtime took 15.909 seconds. Complete settings-object contract, UI/locale syntax and strict host/product/composition lint passed. The archived source and compilation copy stayed identical throughout. This covers the five catalogue controls and format-2 policy retention, not full legacy/binary-upgrade recovery, all resource controls, prefix job admission or browser/release acceptance.

### 2026-10-12 — Remove composition file boot API and fence explicit credentials (candidate)

Source tracing found no productive caller of the old composition-owned TOML/file/ENV provider boot resolvers. Those public APIs and their file-load error variant are removed; the obsolete overlay loader test is removed with the deleted capability, while actual invalid-DB/startup/reload acceptance remains required. Synthetic legacy environment selection stays test-local and is not exported. Provider construction errors and operator instructions now describe the selected DB configuration and encrypted credential setup. LLM registry file/ENV APIs, repository JSON, controlled upgrade extraction and remaining chain/model metadata cutover are still open.

The explicit credential resolver previously fell back to `api_key_env` when the encrypted store supplied no key, even with environment resolution disabled. It now reads only the supplied credential on that path; both required and optional missing keys cannot inherit a process credential. Settings report stored key presence only. Existing external OAuth/SDK authentication paths remain separate. Required evidence includes a nonempty legacy environment key with missing required/optional stored credentials, real coherent PG settings under that environment, ordinary global HTTP model dispatch/lifecycle, role initialization and strict affected-consumer lint.


Provider-boundary r2 passed the original frozen `target/validation/provider-boundary-20261012-r1` source with the corrected `registry-provider-factory` LLM test configuration; receipt `/private/tmp/brassclaw-provider-boundary-accepted.json` and successor artifact directory `provider-boundary-artifacts-20261012-r2`. The preceding r1 build omitted that feature and ran zero matching tests, which the acceptance guard rejected; it is not regression evidence. The nonempty-environment required/optional credential fence passed (0.484 seconds), 19 catalogue checks passed, real PG settings with the environment key present passed (2.995 seconds), ordinary global HTTP dispatch/lifecycle passed (7.949 seconds), startup Sempai restoration passed (20.076 seconds), and strict affected library lint passed. These results cover explicit credential custody and removal of the composition file-boot API; other legacy LLM registry/model-discovery/chain paths remain open.

### Next provider slice — explicit model discovery

`models::fetch_models_for` currently routes by provider name, ignores edited endpoints for named OpenAI/Anthropic providers, reads environment credentials and returns hardcoded model lists after network/auth failures. A stored key admitted for one persisted endpoint can therefore be sent to a different hardcoded endpoint. Replace that productive facade with typed adapter/explicit endpoint/credential input, visible classified failures and no static model-list success. Keep protocol authentication acquisition separate from catalogue/model selection. Reuse the LLM infrastructure owner; expose actual discovery deadlines/body bounds through revisioned provider settings with short help rather than adding hidden fixed values. Required acceptance includes custom IDs/protocol changes, both formerly named providers on local endpoints, no outbound redirect credential forwarding, no environment-key uptake, HTTP/auth/body/schema failures, actual WebUI/DB changes and restart/retained-request semantics. Model-name discovery is not a complete model-specification/design/activation certificate.


### 2026-10-12 — Explicit model discovery and snapshot-bound probe credentials (candidate)

Model discovery now uses the supplied adapter and exact captured endpoint, never a provider-name shortcut, environment credential or static model fallback. The previous public name-based models module and its fallback-specific tests are removed with that capability. The new transport returns classified HTTP/connection/deadline/body/schema failures and disables redirects. Anthropic pagination follows the documented last_id/after_id protocol under one total deadline/byte ceiling; incompatible pagination cannot silently return a partial list. Protocol source: [Anthropic model-list API](https://platform.claude.com/docs/en/api/models/list). Discovery names remain observations, not complete model-specification/quality/design evidence. Unsupported dedicated discovery adapters still require their actual auth/protocol support before the full provider-function cutover.

Provider definitions/API snapshots expose a model_discovery policy (timeout seconds and combined response bytes), with revision-checked edits, omitted-policy retention across adapter edits and EN/DE form explanations. Fetch models uses the unsaved form policy; later requests use persisted settings. Response-cache policy omission now also retains the preceding policy when an adapter changes. Structural validation rejects invalid policies before setup writes.

Connection probes and discovery capture their definition, endpoint and stored credential in one repeatable transaction. Existing authenticated one-shot lease creation/consumption now supports a caller-owned transaction; no direct decrypted-material read bypass is introduced. Consumption commits and the pooled connection is released before external effects. Concurrent key/endpoint updates can cause a visible serialization failure before I/O; they cannot send a successor key to the preceding endpoint. These neutral ports grant no permission and are internal host setup infrastructure, not plugin execution APIs.

The candidate needs actual native PG/HTTP endpoint/header/redirect/error/deadline/body/pagination checks, one-connection capture, revisioned policy uptake, concurrent key/endpoint fencing, existing secret owner/lease compatibility, normal global model execution, HTTP DTO/UI coverage and strict affected-consumer lint. No acceptance/release claim is made yet. The same coherent credential/model revision capture still needs coordination in the remaining boot/role/embedding/task consumers; this probe slice does not certify full immutable credential/model task selection.


### 2026-10-12 — Coherent selected-provider credentials and honest connection probes (candidate)

The existing provider administration owner now captures primary, Sempai or embedding selection, canonical definition and authenticated one-shot credential in one repeatable-read transaction. It reuses its catalogue reader/alias checks and the existing encrypted broker transaction ports; no second registry, raw decryption reader or invocation grant is introduced. Provider/network construction begins only after consumption commits and the pool connection is released. The ordinary primary startup caller, live primary reload, Sempai restoration/construction and embedding maintenance use that capture. An unconfigured embedding preflight remains read-only and does not require a master key; configured maintenance captures again before constructing its adapter. This is coherent current selection, not immutable credential revision retention, whole-task model-specification pinning or complete desired/effective role coordination.

Connection testing now uses the actual bounded model exchange instead of treating a provider constructor/default empty list as network success. Invalid, HTTP/auth, redirect, capacity and timeout results remain visible. Discovery supplies suggestions without replacing an operator's model with the first returned entry. Draft WebUI policy values reject invalid/partial/unsafe numbers before transmission. Explicit catalogue credentials also exclude environment-supplied extra authorization headers and Anthropic OAuth material; legacy environment resolution remains separate pending final removal.

Preserved validation failures: r1's native settings save had no reload owner; the successor uses the real adapter and verifies its retained identity. r2 passed native discovery/owner/global/cache cases but strict lint rejected the obsolete NEAR environment-discovery constructor; that constructor was removed. r3 compilation caught an incorrect assumption that EmbeddingSection equals LlmSlotSelection; its actual provider/model fields now map explicitly, without credential/endpoint overrides. r4 passed native discovery, one-connection captures, all three role races and secret compatibility; ordinary startup found its remaining type-erased key-store caller. That caller now uses the existing PostgreSQL-aware runtime handle. None of these failed batches is full acceptance. The r5 frozen batch must establish ordinary startup/global exchange, Sempai restoration, embedding credentials, selected cache policy, HTTP/JS contracts, explicit header/OAuth fences and strict affected lint before this candidate is accepted.


### 2026-10-12 — Native Gemini/Ollama/NEAR discovery adapters (candidate)

The next transport revision adds Gemini native resource-name and nextPageToken/pageToken handling, retaining one total byte/deadline policy across all pages. API-key material travels in x-goog-api-key, not the URL. Sources: [Gemini model list](https://ai.google.dev/api/models) and [Gemini API-key HTTP authentication](https://ai.google.dev/gemini-api/docs/api-key). NEAR listing now composes the same versioned models path as its existing completion adapter; an explicitly supplied endpoint remains the only destination. Ollama consumes its actual tags schema. None of these paths supplies guessed model names or selects a discovered model for the operator. Local native acceptance must inspect actual URLs/headers, full pagination and repeated-cursor failure. Session/OAuth and signed AWS discovery still need their own existing authentication owners; API-key tests cannot certify those flows.


Model-discovery r6 acceptance passed against frozen `target/validation/model-discovery-20261012-r6`; receipt `/private/tmp/brassclaw-model-discovery-accepted.json`. The expanded native PG/HTTP discovery/role-race case passed in 6.897 seconds, including Gemini/Ollama/NEAR URLs, actual authentication headers, complete pagination, repeated-cursor rejection, one-connection capture, policy edits and endpoint/key fencing. Strict affected library lint passed in 25.973 seconds. The receipt reuses r5's unchanged successful owner/global/cache/Sempai/embedding/HTTP/JS/explicit-credential cases, with an exact source-difference guard. R5's final lint failure was a misplaced prefix-source documentation comment, corrected without suppression; its original failure and all prior diagnostic/source/artifact records remain retained. Native r5 ordinary global execution took 8.996 seconds; Sempai startup took 20.582 seconds; embedding owner acceptance took 2.554 seconds. These are local constrained/ordinary caller observations, not browser/platform/remote release acceptance or full immutable credential/model/prefix/role cutover.

### 2026-10-12 — Protocol-based provider construction (candidate)

Provider-chain creation and metadata previously selected dedicated adapters using names such as nearai/bedrock/openai_codex. Editing those IDs to a registry protocol could dispatch through a different adapter, while a custom ID for a dedicated protocol could fail or report the wrong model/identity. The resolved LlmConfig now retains the exact protocol separately from its canonical provider ID. Main/cheap/dedicated construction and model metadata use that protocol, and published identity keeps the selected ID. Legacy manually constructed inputs have no resolved tag and retain their compatibility decoding; they are not database selection evidence. The ordinary global HTTP/in-flight/settings/lifecycle fixture now deliberately gives OpenAI-compatible definitions the IDs nearai and openai_codex. A focused resolver case checks renamed dedicated protocols' identity/model metadata. The next frozen batch must cover those production/constructor consumers and strict lint before accepting this candidate. Dedicated authentication and remaining ENV-based chain/session configuration remain separate open requirements.


Protocol-dispatch r1 passed against `target/validation/protocol-dispatch-20261012-r1`; receipt `/private/tmp/brassclaw-protocol-dispatch-accepted.json`. Ordinary global matched/unmatched/provider-settings handoff with colliding dedicated names and retained in-flight identity took 10.393 seconds; live primary/Sempai cache policies, startup Sempai restoration, renamed dedicated identity/model metadata, cheap-provider compatibility, actual model-route consumers and strict affected library lint passed. This proves protocol/identity separation through those callers, not dedicated live service authentication or the remaining explicit chain/session-policy cutover.

### 2026-10-12 — Registry SDK endpoint/deadline/credential transport (candidate)

The next trace found that most registry SDK factories did not inject the supplied request timeout, and Gemini discarded any proxy URL ending in a legacy shim suffix, sending its credential to the SDK default authority. Registry SDK clients now receive one common positive representable deadline and disabled redirect policy. Invalid configured OpenAI/OpenRouter headers return a sanitized error instead of being silently skipped. Gemini v1beta/shim normalization preserves the selected host and proxy prefix; a stable-v1 URL cannot silently become this SDK's hardcoded v1beta API and is rejected for explicit configuration repair. The existing optional-auth HTTP adapter translates the SDK-generated Gemini query key to a sensitive x-goog-api-key header before sending, preserving other query parameters such as alt=sse and rejecting missing/duplicate/invalid credential transport. Required acceptance uses the actual SDK completion path through local HTTP, header/URL inspection, redirect rejection, one-second timeout, absent-auth compatibility and malformed-header rejection; helper source/tests alone are insufficient. Full DB-owned request/retry/circuit/session controls and common egress/client ownership remain open.


Registry transport r1 compilation exposed SDK generic constraints: replacing the builder's HTTP type removes inference for its original transport, and Rig 0.30's Gemini Capabilities<H> names its default model type. The successor explicitly names reqwest for all original builders and uses Gemini's existing public generic CompletionModel::new with the actual adapter. No dependency patch, provider substitution or warning suppression is used. R2 passed the real SDK proxy/header/redirect/deadline case (3.664 seconds), absent-auth compatibility, endpoint normalization, ordinary global dispatch, live role-cache settings and strict library lint. Its source/artifacts/receipt remain preserved; that acceptance precedes the separately traced NEAR failover fence.

The r3 source now prevents a NEAR fallback configuration from changing a failed request on another selected protocol. Its real SDK regression constructs the existing complete static chain with a deliberately populated, different NEAR endpoint/key/model and verifies that a rejected Gemini request remains on Gemini with no extra provider effect. This is a provider-chain protocol fence, not a Rust agent-loop fallback or a Recipe retry implementation. The r3 batch must also pass the ordinary global and selected-role callers and lint before the successor is accepted. Remaining ENV-based chain/session configuration and adjustable request/retry/circuit controls are still required by the provider/settings cutover.


Registry transport r4 accepted frozen `target/validation/registry-transport-20261012-r4`; receipt `/private/tmp/brassclaw-registry-transport-accepted.json`. It reuses r3's unchanged, actually regenerated SDK/ordinary-global/role caller results (3.738, 9.599 and 13.602 seconds for the main cases) and reruns strict library/consumer lint with explicitly invalidated source timestamps (27.653 seconds, with actual Checking output). The frozen-copy runner now stamps changed compilation inputs when applying a snapshot: a preserved source mtime could predate an older build that finished while the shared checkout was edited. R3's prior full receipt is marked partial with that limitation; the original receipt, logs, sources and artifacts remain retained. Content hashes/inventory verification alone cannot establish that Cargo regenerated a stale fingerprint. Future batches must retain the timestamp invalidation and explicit evidence-reuse guards; changed/removed compiler inputs need relevant revalidation. This repairs current registry transport lint freshness, not a blanket recertification of every historical cached command or the unfinished plan.


### 2026-10-12 — Explicit model-request policy and four Inference controls (candidate)

Provider definitions now carry request_policy with per-HTTP-request timeout, additional retries, optional positive circuit threshold and recovery seconds. Omitted edits preserve the existing policy even across adapter changes; explicit null, invalid bounds and unrepresentable durations are rejected. Historical definitions lacking the object retain compatible common defaults, and actual edits persist it in the immutable provider-definition revision. API snapshots supply those defaults; Inference dialogs expose four controls with EN/DE help. Zero retries sends once; the UI's zero circuit threshold encodes an explicit disabled/null threshold. Discovery and VM compute time remain separate controls.

The productive DB-selected chain uses explicit request/cache policy without parsing generic request/retry/circuit/cheap/fallback environment overrides. Its unselected NEAR placeholder contains no environment key, model or fallback. An explicitly selected NEAR definition now requires its configured endpoint rather than inferring a different service from key presence or environment. Remaining dedicated auth/session fields and their controlled upgrade are still open; this is not the full §10 cutover. Legacy generic/file/ENV entry points remain distinct until coordinated removal.

SDK completion errors now retain typed HTTP classification: credential failures do not retry; rate limits and 5xx are transient; other HTTP rejections are nonretryable and carry no upstream body/URL. Context-overflow classification remains intact. Host model adapters map permanent rejections to invalid request/configuration instead of transient unavailable. Required acceptance: real PG/live primary and Sempai clients, one-second vs retained deadlines, exact additional attempts for a 503, no retry on 401, pre-network circuit rejection, independent role state, configured recovery, invalid-write rollback, policy omission/null contracts, explicit malformed environment isolation, HTTP/JS extraction and strict affected lint. Source/JS helper checks alone do not certify those runtime claims.


Request-policy r1 passed against `target/validation/request-policy-20261012-r1`; receipt `/private/tmp/brassclaw-request-policy-accepted.json`. The real PG/live primary/Sempai deadline/retry/circuit/401/recovery/revision case took 12.485 seconds. Explicit invalid-environment isolation, real SDK transport, ordinary global protocol identity (8.055 seconds), live cache policies, Sempai startup, HTTP DTOs, 28 JS cases/syntax and fresh strict affected lint (28.150 seconds) passed. The four controls are accepted through these local callers; actual browser/rendering, platform/release/remote acceptance and complete auth/model/desired-effective cutover remain open.

The extended retry-count range exposed an older unsigned-to-signed backoff overflow and an unchecked host-clock sleep deadline. The successor uses bounded u128 intermediate arithmetic without signed wrap and checks host-clock representability before sleeping. It never silently changes the selected attempt count or supplies a second model/provider. Pure boundary acceptance must include the full u32 attempt range, and the existing actual 503/401/circuit caller must still pass. Retry timing/jitter/Retry-After controls remain an explicitly recorded next policy requirement; this arithmetic repair does not certify their configurability.


Request-policy r2 passed against `target/validation/request-policy-20261012-r2`; the updated receipt preserves r1's unchanged caller evidence and records full-u32 backoff boundary coverage, the real selected-client policy case (12.304 seconds), explicit environment isolation and fresh strict lint (28.939 seconds). Unsigned jitter cannot wrap into a negative/signed sleep, and an unrepresentable deadline returns an error before a timer is created. Pending retry timing controls must carry their ceilings into every actual wait; parsing a Retry-After header and silently clipping it to an older constant would not establish live configurable uptake.


### 2026-10-12 — Model-discovery capability metadata (candidate)

Settings previously reported model listing through stale setup-label flags or absent labels, including false for now-implemented Gemini/custom adapters. The transport owner now exposes its actual implemented protocol capability, and the settings view uses that fact. This describes technical support, not credential validity or permission; individual endpoint/auth failures remain visible. The existing real PG/HTTP discovery regression additionally verifies that an active custom definition without a setup label advertises its implemented listing capability. Session/OAuth and signed discovery remain false until their actual authentication/transport path is implemented and qualified.


Discovery-view r1 passed against `target/validation/discovery-view-20261012-r1`; receipt `/private/tmp/brassclaw-discovery-view-accepted.json`. The real PG/HTTP discovery/capture/policy/role-race regression (6.743 seconds) verified the technical capability view, and fresh affected lint passed (28.953 seconds). Endpoint/credential validity remains separate; dedicated session/OAuth/signed discovery is still unimplemented.

### 2026-10-12 — Configurable retry timing and retained HTTP hints (candidate)

request_policy now includes a compatible defaulted retry_timing object: initial delay milliseconds, maximum backoff milliseconds, jitter percent and maximum provider-requested wait seconds. Its four Inference controls have EN/DE help. Validation requires a representable clock delay, positive base/ceilings, maximum covering the base and 0–100 percent jitter. Every actual wait uses the selected policy. Unbounded exponents use unsigned arithmetic before the configured ceiling; no signed casts or fixed 100-millisecond floor remain. Existing retry-count/config constructor interfaces retain a safe common default; DB-selected chains carry their persisted policy through primary/cheap/fallback wrappers.

Protocol Retry-After parsing preserves the original valid hint without an independent one-hour clamp or missing-header 60-second sleep. Selected retry consumers apply the configured wait ceiling, and missing/invalid hints use their configured backoff. The existing SDK HTTP adapter retains typed status/hint information; its rejection Display/Debug carries no upstream body, URI or key. Only context rejection needs its private error body. SDK success-body and context-error-body capacity still need the coordinated response-resource control; timing acceptance cannot certify those byte limits. Registry adapters share this existing transport rather than losing headers through the SDK's status-only error representation. Dedicated Gemini text-hint arithmetic now rejects overflow instead of wrapping.

Required acceptance adds full-attempt boundary/hint preservation, actual 503 Retry-After:2 through a configured one-second ceiling, existing selected-role timeout/retry/circuit/auth/capture behavior, SDK endpoint/auth compatibility, HTTP/UI extraction, omitted nested-policy compatibility, and strict affected lint. The current candidate is not accepted yet and does not finish dedicated auth, model specs, full role desired/effective publication or all remaining resource caps.


Retry-timing r2 accepted `target/validation/retry-timing-20261012-r2`; receipt `/private/tmp/brassclaw-retry-timing-accepted.json`. R1's exact source passed the actual selected primary/Sempai policy case including Retry-After:2 clipped at the operator's one-second ceiling (12.222 seconds), complete SDK transport, full-u32 timing boundaries, existing retry/Gemini hint contracts, normal global execution (7.596 seconds), live cache policies, Sempai startup, HTTP extraction, 29 JS cases and locale/form syntax. Its final lint rejected an import left unused by the parser change. R2 removes that import and passes fresh strict affected lint (27.087 seconds), with exact source/inventory guards reusing the unchanged passing caller results. Eight request/timing controls now have local native acceptance and EN/DE help; complete response-byte bounds, dedicated auth/model/role publication, browser/platform/release and remote acceptance remain open.


### Candidate: explicit model-response capacity (2026-10-12)

Provider request policy gains optional `max_response_bytes` (null = unrestricted;
positive finite values must fit host allocation indices). Existing definitions
retain unrestricted behavior, rather than receiving an undocumented new cap.
Inference settings expose bytes and explain decoded bodies, streams, errors,
retained clients and unsupported dedicated adapters in English and German.
Selected chains carry the immutable value into all six registry SDK HTTP clients,
including multipart/streaming transports, and direct NEAR AI, Copilot and
Anthropic OAuth model readers. Declared and received decoded lengths are checked;
no partial buffered answer or overflowing stream chunk is returned. A typed
capacity failure is nonretryable, does not trip a transient circuit and maps to
the model gateway's resource error. Exact-boundary responses remain valid.

Dedicated Bedrock, Gemini OAuth and Codex transports remain implementation work.
Finite limits for those adapters are rejected before publication/construction;
null is supported. The legacy Codex ChatGPT compatibility path similarly rejects
a finite value rather than claiming uptake. This is a candidate until frozen
native HTTP/PostgreSQL/role/ordinary-global and affected consumer validation pass;
it does not certify full-provider auth, metadata, browser or release acceptance.


### Accepted registry response-capacity slice; dedicated successor (2026-10-12)

Frozen `response-capacity-20261012-r1` passed actual bounded HTTP readers,
exact-boundary/declared/chunked/context-error/streaming cases, policy validation,
ordinary global execution, primary/Sempai live PostgreSQL updates and retained
clients, cache/Sempai startup/HTTP/UI consumers, and strict affected-package lint.
Evidence is `/private/tmp/brassclaw-response-capacity-accepted.json`, retained in
`target/validation/response-capacity-artifacts-20261012-r1`. This certifies that
snapshot and the recorded OpenAI-compatible native path, not concurrent source
extensions or full dedicated authentication/remote acceptance.

The successor carries the optional ceiling into dedicated Codex and Gemini
OAuth completion bodies. Gemini OAuth also uses the selected request timeout
instead of a hidden 300-second model timeout. NEAR AI/Copilot/Codex model clients
use the common no-redirect transport. Codex and Copilot classify status-only
rejections without copying private bodies; Copilot 401 invalidates its credential
cache for the next explicit request and remains a permanent authentication error.
Anthropic OAuth keeps its credential-renewal attempt but drops the first 401 body
and reports the actual renewed-request failure; normal/refreshed malformed JSON
reports safe coordinates rather than private response text. Finite Bedrock and
legacy Codex ChatGPT limits still reject before construction; their transport
support remains open. The actual Codex SSE request receives a local HTTP capacity
regression. Dedicated OAuth service setup and complete role/auth/model selection
remain separate acceptance requirements.


The first dedicated successor (`response-capacity-20261012-r2`) stopped at the
new Codex regression's unqualified `json!` macro. Its complete compiler output
and frozen inputs are retained. Qualifying the four test macro calls with
`serde_json::json!` repairs that source error; no runtime behavior or warning
suppression is used. A successor must rerun the unfinished behavioral checks.


### Accepted dedicated response slice; bounded allocation successor

`response-capacity-20261012-r3` passed the dedicated Codex HTTP/SSE capacity and
safe 503 classification regression, the existing direct NEAR AI regressions,
selected primary/Sempai native/ordinary-global/cache/startup paths, HTTP/UI
consumers and strict lint. Its acceptance retains r2's failed compiler evidence.
Gemini OAuth's selected timeout/body reader is wired, but a real authenticated
Gemini service exchange remains unverified. Bedrock and legacy ChatGPT finite
body limits remain explicitly unsupported; no cosmetic acknowledgement is made.

The next candidate uses bounded geometric buffer growth instead of exact
reallocation on every small chunk, and adds actual multipart response acceptance.
NEAR AI diagnostics retain status, received length and JSON coordinates while
removing prompt/answer/upstream-body exposure; remaining permanent HTTP failures
are typed rejections rather than retryable raw-body messages. This candidate
requires relevant bounded-reader/direct-provider and selected-role checks.

The import inspection also found remaining unsafe legacy paths in `migration.rs`:
`rename_migrated` can replace an existing backup; config/Sempai writes can partially
commit or replace newer operator values; SQLite copying needs a consistent WAL
snapshot; raw-master migration can overwrite the canonical key/change its DB
ceremony and its wipe follows a replaceable pathname. The explicit importer must
use validated snapshots, conflict-preserving atomic writes and synchronized
no-clobber backups, retaining source artifacts unless deletion can be fenced.
Repair the secret ceremony through its existing owner and never mutate an
unverified replacement source. These remain release-blocking implementation work,
not an approval gate or evidence that automatic migration is safe.


The allocation/diagnostic successor r4 stopped before tests because the same
legacy permanent-error block also appeared in `list_models_inner`; its replacement
needed a local status code there. Both compiler diagnostics and that frozen source
are retained. The successor binds the already captured response status explicitly,
then repeats the unfinished checks. No failed batch is accepted or silenced.


### Response capacity accepted; settings importer candidate (2026-10-12)

Frozen `response-capacity-20261012-r5` passed the complete current bounded-reader
slice: declared/chunked/context-error/streaming/multipart HTTP, exact boundary,
actual dedicated Codex SSE, direct NEAR AI including private permanent rejection
through the retry wrapper, live native primary/Sempai and ordinary global execution,
HTTP/frontend consumers and strict affected-package lint. The no-replay native
case took 13–14 seconds for its deliberate timeout/retry/circuit waits; this is
not a production settings-write latency claim. Diagnostics from failed r2/r4
remain preserved. Native tests certify local adapters and the selected paths;
Gemini OAuth service credentials, Bedrock/legacy ChatGPT, browser/platform and
release acceptance remain open. The optional ceiling defaults to null; no hidden
fixed response cap was added. Bounded geometric reservation prevents per-chunk
whole-body copying. See the scoped accepted receipt/artifacts, not earlier
candidate summaries, for the certified source.

Config/Sempai import now captures and parses one regular-file snapshot, validates
all settings, checks retained database values in one transaction and preserves
matching rows/timestamps. Differing settings fail explicitly. Real writes lock
absent-row writers, create a synchronized content-addressed no-clobber backup,
then commit the complete pending set; dry-run is read-only and creates no backup.
Sources remain present and a changed source after commit is reported explicitly.
The provider importer reuses that backup primitive. Atomic/failure/idempotency
native acceptance is still required. SQLite snapshot/backup and raw-master
ceremony/wipe paths are unchanged and remain the next importer repairs.

### Settings import accepted; raw-master preservation candidate

Frozen `settings-import-20261012-r1` passed both native settings/provider imports,
including deliberate rollback, read-only dry-run, synchronized no-clobber backups,
retained source, unchanged matching timestamps and refusal to overwrite newer
values. The affected legacy integration target builds; strict composition/CLI
lint passed with `migrate-from-libsql` explicitly enabled. Receipt/artifacts retain
this exact feature/source scope. Full SQLite import and whole-upgrade acceptance
are not established.

Raw-master migration now uses the existing validated master-file reader and a
shared no-clobber writer in `secrets_master.rs`, also called by explicit CLI rewrap.
It validates before writing, restricts private permissions before publishing,
synchronizes the key and preserves existing canonical material. One database
transaction serializes absent ceremony rows; existing versions are preserved and
wrapped ceremonies take precedence over stale raw sources. A rejected DB write
leaves recoverable matching files. The source is retained because no supported
portable conditional unlink exists in this owner. The old whole-file allocation,
zero-fill and replaceable-path deletion were removed from this migration path.
Native rollback/conflict/version/wrapping acceptance is still required; CLI's
separate post-wrap cleanup and SQLite snapshot/backup remain to be repaired. The
existing Windows ACL command path is preserved, not claimed as locally exercised.


The raw-master r1 batch compiled the new cases but its copied runner still used
the settings-import filter under a misleading master label. It then failed strict
lint on one collapsible conditional. It is not master acceptance. The successor
uses the intended full master and concurrent-publisher filters, records actual
filters in its receipt and verifies they appear in test output; the conditional
uses the required let-chain style without a lint allowance. Failed r1 inputs,
outputs and artifacts remain preserved.

### Raw-master acceptance; consistent SQLite import candidate

Frozen `master-import-20261012-r2` passed the intended named master rollback,
concurrent no-clobber publisher, retained version/wrapped priority, and affected
settings/provider import cases; receipt entries record their actual filters.
Strict composition/CLI lint passed. Source files remain private and retained.
CLI post-wrap cleanup and Windows ACL runtime acceptance remain open.

SQLite import now opens the original read-only, takes one private consistent
logical snapshot with a bound `VACUUM INTO` destination, and imports every selected
table from that snapshot into one PostgreSQL transaction. Read-only preflight
converts eligible rows without executing insert statements; actual database
constraints/triggers still require the real write path. Successful writes retain
a synchronized content-addressed backup without replacing the original database,
its WAL or older backups. Only absent known optional tables are skipped; existing
malformed tables/columns and invalid optional values produce explicit errors.
The previous column `.ok()` and invalid-value-to-default conversions were removed,
and destination-read failure never becomes a default owner. Unknown trigger
states never become scheduled work; recognized terminal states remain terminal.

SQLite documents `VACUUM INTO` as a consistent transactional logical backup, and
warns that unfinished output is not a completed backup. This owner synchronizes
completed output before publishing it and committing PostgreSQL:
[SQLite VACUUM INTO contract](https://www.sqlite.org/lang_vacuum.html).
Native real SQLite WAL/standalone backup/PostgreSQL rejection/type-corruption
acceptance remains required. Full table mapping, cross-source atomic upgrade,
conflicting historical row classification and trigger effect reconciliation are
not established by this candidate.


The first SQLite native write exposed older broken mappings: the importer and
its historical fixture assumed key/value safety, section/key/value token settings
and path-based memory documents, while the real stores use categorized rules,
provider-owned JSON token settings and typed titled documents. r1's failure and
snapshot are retained. The successor reads the authoritative legacy stores/DDL
and maps those actual fields to V014/V015/V016, including strict booleans,
provider token JSON, tags, metadata and nullable source-thread values. The fixture
now creates that actual source shape; it does not add fake target columns to
make the importer pass. Unknown legacy settings keys remain explicit errors
until their supported disposition is implemented. Filesystem, capability and
other historical mappings still require the same authoritative reconciliation.

Authoritative mapping repair extends to V018 filesystem entries/index declarations/
events, V015 legacy capability metadata and epoch-millisecond hook timestamps.
Data fields preserve opaque bytes, versions, owners, tags and nullable values;
text-to-date/numeric casts use PostgreSQL's actual parameter types. Event sequence
restart uses transactional DDL under the event-table lock, since `setval` is not
rolled back. Recognized terminal triggers stay terminal. Legacy runnable triggers
are preserved paused with a structured count printed by the explicit CLI; their
missing schedule/effect correspondence must be reconciled before activation,
preventing an upgrade from inventing a new execution. This is not a new Component
approval gate. Full-original snapshots retain the old configuration for that
reconciliation. Native coverage now exercises these actual tables and a future
event append in addition to WAL, rollback and malformed nullable data.

The historical migration integration test is now a real native PostgreSQL/SQLite
case rather than an environment-driven successful skip. Its source fixture uses
the authoritative categorized safety, provider-token JSON, typed memory-document
and filesystem/index/event layouts; assertions check content, tags, owners,
versions, epoch time, numeric hook values and paused trigger state. The retained
source/backup and dry-run checks remain. The next frozen batch must execute this
integration target, not merely build it. Retired global settings keys and unknown
legacy layouts fail explicitly; their disposition and historical conflict
reconciliation remain open. No target-only dummy columns are created.

### Native SQLite mapping acceptance; retained-row conflict candidate

Frozen `sqlite-import-20261012-r3` passed WAL/standalone backup and all-table
rollback/type-corruption cases, the actual all-source native integration and
read-only preflight, affected master/settings/provider cases, and strict
composition/CLI lint. Source files, original WAL and previous backups survive.
The integration no longer returns success when PostgreSQL is unavailable.

The next candidate makes SQLite row conflicts explicit. Each static mapped
insert retains an existing row only when PostgreSQL confirms every imported
field is identical, with null-safe comparisons and the same actual parameter
casts. A differing retained payload aborts the whole transaction instead of
silently disappearing behind `ON CONFLICT DO NOTHING`. Matching imports do not
update timestamps or old state. SQL identifiers/expressions are owned static
strings; runtime values remain parameters. Native acceptance will cover a
changed legacy token payload and preservation of both retained and earlier
pending rows. Cross-source atomicity, retired global settings and CLI cleanup
remain separate open requirements.


### Retained-row conflicts accepted; post-wrap key cleanup candidate

Frozen `sqlite-import-20261012-r4` passed explicit differing-row rejection with
whole-table rollback/preservation, matching retained rows, full native public
migration integration, read-only preflight and strict composition/CLI lint.
Historical source bytes remain available. No upsert silently replaces newer rows.

CLI post-wrap cleanup now moves the current public pathname into a private
capture before rechecking the exact superseded key. A differing replacement is
restored without clobbering any newly created pathname; if restoration is not
possible it remains in a persistent private recovery directory. All failure
paths preserve captured material. Only verified material in that private capture
is zeroed and removed, and a concurrently recreated canonical pathname is never
unlinked. Database ceremony locking/rechecking still precedes cleanup. Physical
erasure on SSDs/snapshots is not promised. The replacement-key regression and
actual CLI rewrap consumers require validation; private-directory ownership is
not protection against a malicious installation owner with unrestricted OS access.

Frozen `key-cleanup-20261012-r1` passed the named CLI replacement-key capture/
restore test and matching-key removal, with actual CLI test-target compilation
and strict composition/CLI lint. The receipt does not certify complete rewrap,
restore failure/new-path collision, or Windows filesystem/ACL runtime behavior.
Those are required follow-up checks before release. The implementation remains
within explicit local operator maintenance; it does not change Recipe sequencing,
component approval or task/Tool authority.

### 2026-10-12 — CLI ceremony and network boundary acceptance

Frozen `key-ceremony-20261012-r1` executed the actual CLI ceremony against
PostgreSQL, including failed encrypted-data queries, deliberate late DB
rejection, passphrase wrapping, wrong passphrase, conflicting raw files,
version preservation and symlink rejection. Two private-capture regressions
cover mismatched material, failed no-clobber restoration and recreation of the
public pathname. All post-capture errors identify the recovery directory.
The actual binary acceptance took 10.598 seconds; affected strict lint passed.
This is macOS evidence, not Windows ACL/filesystem acceptance.

Frozen `network-boundary-20261012-r1` executed real connection and incomplete
response failures with credential-bearing URLs. Safe error categories preserve
failure classification without copying backend URLs, query credentials or
response data. Real subprocess acceptance with all ambient HTTP/HTTPS/ALL proxy
variables set proved dispatch to the vetted resolved address; the transport
explicitly disables ambient proxies. Network unit/contract tests and strict
all-target lint passed. Adjustable network capacities, cache eviction,
cross-platform runtime and full product acceptance remain separate work.

Neither milestone completes the v3 plan or authorizes early release claims.

### 2026-10-12 — Dedicated-provider environment isolation

Frozen `dedicated-env-20261012-r2` passed actual resolver regressions with
`registry-provider-factory`, all Gemini request-construction regressions and
a subprocess with poisoned generation environment. Database-selected
configuration no longer reads Bedrock region/profile/cross-region, Gemini
credential-path or Codex endpoint/client/session/refresh overrides. A selected
Codex endpoint reaches its dedicated config. Database-selected Gemini requests
do not read generation, cache, safety, custom-header or alternate-key/API-version
environment options. The retained legacy resolver explicitly enables its own
compatibility behavior; this is not completion of the removal required by §10.
Strict LLM/composition consumer lint passed (40.280 seconds).

The r1 command omitted the feature containing resolver tests. Its zero-test
guard rejected acceptance; the failed logs remain beside retained artifacts.
r2 enabled the owning feature and ran the intended tests. This evidence covers
resolution and request construction, not authenticated OAuth HTTP execution,
complete database-owned dedicated setup metadata, credential-session migration,
whole-task selection retention, browser acceptance or the final product cutover.

`dedicated-env-20261012-r3` extends the accepted environment isolation with
removal of the hidden Gemini 8,192-token thinking budget and forced HIGH level.
Actual request-construction regressions verify model-owned defaults for Gemini
2.5/3 and preserve an explicit 20,000-token output limit. Gemini/resolver
regressions and affected strict lint passed. Google documents model-managed
thinking defaults at https://ai.google.dev/gemini-api/docs/generate-content/thinking.
This is request-construction evidence, not authenticated OAuth execution.

Frozen `database-architecture-20261012-r1` passed the updated provider-selection
architecture guard and strict architecture lint. The guard now requires coherent
PostgreSQL role selection, explicit credentials and selected technical policies
in composition, and rejects file/environment resolver calls there. It no longer
requires removed public file-catalogue composition APIs. This source-boundary
check does not certify complete provider-library/file removal or the full
architecture matrix.

### 2026-10-12 — Network limits retain supplied values

Frozen `network-limits-20261012-r2` passed actual sockets with a response larger
than the former 10 MiB transport ceiling and a delayed response requiring a
timeout above the constructor default. Explicit positive representable values
are retained exactly; zero/nonrepresentable values fail before URL parsing or
dispatch. Buffer growth uses fallible reservation and returns a sanitized
allocation error. Configured overflow still returns its explicitly classified
bounded partial response, without a successful truncated result. Existing
privacy/proxy, policy, address and response-limit regressions passed, together
with network/host-runtime all-target strict lint.

The r1 behavioral checks passed but strict lint rejected an unchecked read count
in the new delayed test server. r2 checks the amount read and passes without
warning suppression. Failure logs remain retained. Constructor defaults, HTTP
Tool schema ceilings and the 128-entry client cache still need actual global
settings controls/acknowledgement; this milestone does not claim that work is
complete, nor cross-platform or full product acceptance.

### 2026-10-12 — Provider library consumes captured database data only

Frozen `provider-library-cutover-20261012-r2` removed public file/environment
provider resolvers, compiled-in/user catalogue loaders, their fail-open
load fallback and the unused model-env fallback helper. Composition's
compatibility branch now uses the same explicit captured definition, broker
credential and technical-policy builder as the database caller. Gemini no
longer retains an environment-override branch for generation, safety, headers,
API-version or alternate credentials. Its request options remain explicit.

The repository provider JSON, Docker copies, CI path references and E2E
rebuild input were removed. LLM unit catalogue fixtures derive the exact V140
installation payload; they do not establish another runtime catalogue. Stored
provider-edit tests use explicit metadata fixtures instead of a file loader.
Old CLI file/env smoke expectations were replaced with an actual PostgreSQL
CLI inventory/status regression: malformed/stale provider files and contradictory
TOML/LLM environment are retained, while list/alias/status return database-only
definitions and the database model (4.430 seconds). Required-key rejection
remains tested by explicit credential resolution.

34 registry, 7 resolver and 31 Gemini regressions passed. Composition catalogue
and immutable stored-edit tests, real PostgreSQL initialization preserving
operator inventory, and ordinary matched/unmatched global-host regression
passed; the latter retained one global root (18.631 seconds). Affected strict
LLM/composition/CLI lint passed (67.151 seconds). r1 failed compilation after
a mechanical edit altered a boolean test matrix; r2 restores that matrix.
The failed source/logs remain retained, and no failed run is accepted.
Mandatory disk cleanup removed only this chat's Cargo target between preserved
CLI and composition artifacts; target2 remains independent.

This does not finish §10: dedicated setup/session and complete model properties,
remaining authentication consumers, migration-product separation, browser and
cross-platform acceptance still require work. The explicit legacy importer
remains an upgrade-only path with retained source/backup and conflict checks.
The full v3 plan and release are not complete.

Operator documentation now describes database Inference controls. Mintlify
4.2.1012 requires LTS Node: initial Node25 check rejected startup, and bundled
Node24.19.0 runs the real validator. It found 84 existing syntax/import warnings
and link checking stops on malformed architecture Markdown. Nineteen literal
host examples now use inline-code formatting; remaining syntax/source-publication
errors and full documentation validation remain open. Installation warned on
Sucrase3.34→glob7.1.6→inflight1.0.6 and optional keytar7.9→prebuild-install7.1.3.
Those warnings are retained. Sucrase3.35.1 removes the obsolete glob dependency,
but a compatible tool dependency update and keychain-addon replacement still
need validation; do not claim latest Mintlify's dependencies are repaired.

### 2026-10-12 — Live host HTTP client retention

Frozen `http-cache-policy-20261012-r2` passed the complete new resource-owner
path. `max_http_client_cache_entries` is a persisted execution control (128
default; zero disables retention) with EN/DE help. V144 backfills missing values
with a new revision and retains explicit zero/larger values. New capacity is
acknowledged by the existing Monty/Rust settings publication before the full
effective snapshot is exposed. Status derives the actual cache capacity and
revision; persisted values alone cannot report uptake.

The hardened transport and metadata-only cache control are attached together
through HostRuntimeServices. Replacing either HTTP egress port removes that
attachment. LRU eviction uses randomized keys containing host, port, vetted
addresses and timeout. Capacity is not an up-front allocation. Actual socket
acceptance proves evicted clients remain usable by active handles. A real SDK
builder queued behind an occupied blocking worker proves an older accepted
build cannot repopulate a new positive-capacity generation. Real host-mediated
HTTP also verifies retention/zero-disable/re-enable through the owning facade.

Native PostgreSQL acceptance preserves missing/default, disabled and larger
values and rejects negative/fractional/null/overflow rows. Deliberate DB
rejection leaves persisted settings and the running cache unchanged. Live
edits to 2,0,5 clients have matching effective revisions and retain the same
global VM. The ordinary matched/unmatched global regression passed (17.210s),
and strict affected lint passed (42.527s); EN/DE and settings JS syntax passed.

The r1 frozen batch captured two V144 migrations during concurrent source work
and failed real initialization with SQLSTATE23505. The current prefix migration
is V145; r2 checks all142 migration numbers are unique before compilation and
passes. Failed logs/source remain retained. No warning is suppressed and no
failed batch is accepted.

This is macOS native/facade/settings-owner evidence, not rendered-browser,
Windows/Linux runtime or release acceptance. Network defaults and HTTP Tool
schema bounds, global worker/pending-call counts, remaining runtime caps,
durable recovery and full prefix/model/validator work remain open. Documentation
syntax repairs additionally quote literal brace examples and escape pipe
alternatives in Markdown tables; full Mintlify validation is still pending.


### Host HTTP omitted-bound defaults — native acceptance (2026-10-13)

`default_http_request_timeout_millis` and `default_http_response_body_bytes`
are persisted instance execution settings with English/German WebUI controls
and short explanations. V146 backfills only missing fields and advances one
revision; it preserves explicit values, including partially populated rows.
Invalid zero/noninteger/null/overflow values fail the SQL checks; native clock
and allocation representability are checked before a live edit and before
startup catalogue preparation. These defaults are independent of model-provider
limits and do not grant network or Tool permission.

The actual hardened transport publishes capacity and both defaults atomically
under its revision mutex, after shared runtime acknowledgement. Its effective
view comes from actual transport state. Requests capture both bounds before
asynchronous work; accepted requests keep those bounds after edits. Explicit
per-request bounds remain exact and independent. Native sockets verify response
limit failure, real timeout failure and an older request completing with its
retained bounds even after waiting longer than the successor timeout.

Frozen `http-request-defaults-20261013-r4` passed network contracts, actual host
egress, PostgreSQL preservation/rejection and the ordinary matched/unmatched
global regression (27.261 s), including invalid live edits and unchanged VM
identity. Strict affected library lint passed (24.269 s), affected test-target lint
passed (67.293 s), and EN/DE/settings JS syntax passed. Receipt: `/private/tmp/brassclaw-http-request-defaults-accepted.json`.
This is local native acceptance; rendered browser and cross-platform deployment
remain open. HTTP Tool handlers still provide explicit defaults and impose their
own schemas/resources, so this slice does not certify all HTTP Tool limits.

The intermediate r2 test assumed a universally invalid enormous clock duration;
clock ranges actually differ. It failed without accepting the batch. The corrected
test derives invalidity from the host clock. The r3 runner rejected an obsolete
compilation-copy baseline before compilation; r4 verified the actual r2 baseline
and completed. Failed logs and frozen sources are retained. No warnings were
suppressed and no execution fallback was introduced.

Documentation validation ran with supported Node 24 and actual preview-cache
permissions. It still reports parser failures in historical documents and video
source imports treated as Mintlify snippets. Literal comparison/placeholder
examples were quoted narrowly. Full documentation validation and the correct
video publication boundary remain unfinished; no warning exclusions were added.


### HTTP Tool timeout consumes the live transport default (2026-10-13)

`builtin.http` and `builtin.http.save` no longer substitute a fixed 10-second
timeout or silently clamp explicit values to 30 seconds. Omission remains absent
through the host port and is resolved by the actual transport's acknowledged
default. Explicit positive milliseconds must fit the current u32 request type
and are preserved exactly. Zero, fractions, negative/overflow numbers, null and
null strings fail before dispatch. Optional numeric null normalization no longer
turns those invalid values into missing inputs. The native schema removes its
fixed default/max and the HTTP resource manifest removes its separate static
wall-time ceiling/estimate; configured transport timeout, task compute, live
network policy and independent resources remain enforced.

Packaged Tool/ToolSkill metadata describes the same timeout contract. Existing
stored rows remain untouched by insert-only seeding, so this is not a certificate
of immutable successor activation on an upgraded instance. Body/inline/output/
header/request bounds remain separate outstanding work. No new workflow Tool,
Rust-loop fallback, permission grant or metadata execution path was added.

Frozen `http-tool-timeout-20261013-r3` passed the actual registered Tool through
native sockets (2.015 s): omitted timeout fails under a 300 ms effective default,
an explicit 120,000 ms value permits a delayed answer, and a 2-second successor
is used by the next omitted request. Typed boundary tests additionally verify
the exact forwarded override and absence plus rejection before egress. All44
HTTP contracts passed; ordinary matched/unmatched global execution retained
one VM (19.409 s). Strict affected test-target lint passed (60.686 s) and existing
settings/i18n JS syntax passed. Receipt:
`/private/tmp/brassclaw-http-tool-timeout-accepted.json`.

The initial validation runner exited on an indentation error before Cargo and
was corrected after syntax verification. The first compiled snapshot exposed
a concurrent component-deployment test using a nonexistent WebUI bundle builder;
the current shared source already contained the correct product facade builder,
which the successor snapshot captured without adding a compatibility shim.
The second run exposed a native fixture's accepted nonblocking socket being read
as blocking; r3 explicitly sets blocking mode with a bounded read timeout. All
failed scripts, source snapshots and logs remain retained. This is local native
acceptance, not full component/prefix/model, platform/release or remote acceptance.


HTTP Tool facade follow-up: frozen `http-tool-boundary-20261013-r1` passed the
existing no-lower-substrate-handle and single-runtime-network-boundary guards,
plus strict architecture lint. The inline hardened-transport constructor reuses
the existing staged policy/secret pipeline and actual resource controller. It
adds no dispatch, policy or credential access to that controller. Receipt:
`/private/tmp/brassclaw-http-tool-boundary-accepted.json`.


### Complete HTTP Tool data and live response defaults (2026-10-13)

The HTTP primitive forwards an omitted response bound to the actual acknowledged
transport default. Explicit positive allocation-representable bounds are exact.
Both inline and saved responses use the existing strict, sanitized host port;
network overflow is a classified failure, not converted into a partial success.
The formatter preserves every sanitized header and complete text or base64 data;
it does not impose another model-visible byte budget. Redaction can expand data
and JSON/base64 encoding can use more bytes than the fetched body. The raw
network byte bound and model/execution representation limits remain independent.

Removed the HTTP Tool's fixed inline48KiB/save10MiB defaults and 256KiB/10MiB
response ceilings, 15MiB output ceiling, 256KiB upload cap, fixed request/response
header count/name/value caps, formatting overhead budgets and shared1MiB input
fallback. The generic first-party input/output limits for OTHER Tools remain
unchanged. JSON accounting now uses a bounded counting writer, including the
runtime adapter, instead of allocating a second full serialized copy. It retains
exact accounting and the existing error classifications/bounds for other Tools.
Protocol validation, current kernel/network policy, credential blocking/redaction,
scoped filesystem authority and task/frame limits remain enforced.

Save results must identify the exact requested scoped path; metadata cannot
silently substitute another destination. Bundled metadata agrees with full data
and strict overflow. Bundled authentication guidance no longer suggests raw
credentials in headers or obtaining them from memory/session data; a qualified
kernel-staged injection binding is required before that usage can activate.
PATCH guidance no longer asserts inherent idempotence or permission to retry an
unknown effect. These metadata corrections do not establish full library/usage
qualification or immutable seed-successor activation; those gates remain open.

Frozen `http-complete-response-20261013-r4` passed actual registered HTTP/native
sockets and scoped REAL FILE writes (9.707 s): a4-byte live default rejects5 bytes,
a12MiB successor admits full9MiB escaped text with over32/long response headers,
tiny binary data survives base64 encoding, a2MiB upload with over64 headers and
long public names/values arrives intact, and an11MiB binary response is saved
through the actual narrowed write grant. All134 built-in Tool contracts passed
(14.933 s), including credential/network/input checks, exact large-output usage,
redaction expansion and other Tools' unchanged limits. The actual filesystem
worker panic is classified without the former unreachable-code suppression.
Ordinary matched/unmatched global execution retained one VM (20.568 s). Strict
affected test-target lint passed (53.751 s); settings/i18n JS syntax passed.
Receipt: `/private/tmp/brassclaw-http-complete-response-accepted.json`.

Failures remain retained. The first two native runs rejected the SAVE operation
because the test requested /workspace but its real mounted filesystem exposed
/host; the production authority check was correct. The initial diagnosis calling
it an upload rejection was wrong; /host fixed the fixture without broadening
permissions. The next full matrix found two obsolete assertions: all tools had
static output estimates and a bound above256KiB was invalid. New assertions
require unknown HTTP estimates/no static ceiling, preserve other Tool estimates,
and reject nonrepresentable numbers instead of newly valid explicit bounds.
No test was deleted to hide a defect and no warnings were suppressed.

Frozen `http-complete-boundary-20261013-r1` additionally passed the no-lower-handle
and single-runtime-network-boundary guards plus strict architecture lint. The
legacy partial port remains a compatibility interface; the HTTP Tools no longer
select it. This is local native primitive/resource acceptance, not the full
catalogue/model/durable-recovery/platform/release/test-machine completion gate.


### Immutable authoring: lineage is independent of allocation (2026-10-13)

The prior review-submission contract incorrectly required candidate version to
be exactly base+1 and used that base as the allocation CAS. An intervening
unactivated package/author draft could therefore block editing a retained base
or force a misleading rebase. New format2 subjects retain an explicit observed
allocation counter, while the exact UUID/class/version/checksum base remains
review lineage. The next allocated version is counter+1; the base may precede
that counter. New identities still require zero/no base. Existing format1 bytes,
checksums and adjacent-version semantics remain readable. V148 changes only
the relevant constraints, preserving immutable receipts and foreign keys.

The operator facade adds a read-only allocation metadata route, never a latest
selection, approval or Tool grant. The WebUI reads/displays this counter with
English/German help, invalidates it when candidate identity/text changes, rejects
browser-unsafe integers and preserves the exact counter with an uncertain POST.
A matching retry returns its original receipt before checking newer counters.
Changed request fields under that ID conflict. Staging, graph validation, head
advance and subject retention remain one transaction; failed writes roll back
all of them. Activation still independently verifies the active review base.

Frozen `review-lineage-20261013-r1` passed real PostgreSQL/operator HTTP regressions
(13.690 s): base1 + package-draft2 produces unreviewed candidate3 based on1;
a later package4 does not alter exact retry/readback; old1/2 bytes survive;
stale/changed counters and malformed identities fail; operator auth is enforced;
legacy format1 migration preserves exact text; deliberate DB rejection leaves
no candidate3 or head advance; invalid format2 SQL fails constraints. No active
catalogue pointer is changed. Ordinary global execution retains one VM (17.445 s).
Strict affected test-target lint passed (67.536 s), four UI state tests and JS
syntax passed. This fixes an authoring prerequisite; it does not activate an
override, migrate mutable editors or implement complete seed-upgrade rules.
Receipt: `/private/tmp/brassclaw-review-lineage-accepted.json`.

The new route's locked descriptor table and crate dependency direction passed
in `review-lineage-boundary-20261013-r2`, with strict WebUI/architecture lint
(19.089 s). The first boundary batch caught a missing public route-constant
export, which was corrected without a shim or test deletion. Failed evidence
remains retained. Rendered browser and full component/seed/activation acceptance
remain open; the full plan is not complete.


### Accepted persistent Recipe routing choices (2026-10-13)

The catalogue API/UI now carries a separate generation-bound ordinary-routing
choice. Format-3 manifests retain stable disabled Recipe UUIDs; omitted choices
preserve prior disables, including withdrawn/reintroduced Recipes. Matching
eligibility and MCP discovery use the same generation filter; internal
reply/history, component approvals and live Tool policy remain independent.
The existing activation CAS, immutable receipt and exact uncertain-commit retry
are reused. Whole-Recipe disables are operator management decisions, not edits
of approved executable definitions. Historical manifests restore their exact
format/identity. The UI displays current generation, complete selections and
persistent disables, with EN/DE explanation and locked uncertain retries.

Local native acceptance passed against frozen `routing-overrides-20261013-r3`:
real publication rejection rolls back pointer/receipt; duplicate/unknown/stale
rules fail without publication; captured tasks retain their original match;
disabled custom and packaged routes use actual No-Match while internal
reply/history continue; one global VM remains; withdrawal/reintroduction
preserves disables; full ordinary startup reseeding does not re-enable; explicit
re-enable restores matching. Native routing took 14.712 seconds, existing authored
activation 7.067 seconds, reviewed deployment 8.956 seconds and ordinary global
execution 17.552 seconds. Strict affected test-target lint and nine UI state tests
pass. The native fixture uses a controlled model gateway; this is not external
provider or rendered-browser acceptance.

The r4 successor adds actual historical format-1/2 byte-identical restoration;
its native case and strict affected lint pass. Hash comparison proves its only
source change from accepted r3 is this expanded test, so existing authored,
deployment and global results are reused. Receipts:
`/private/tmp/brassclaw-routing-overrides-r3-accepted.json` and
`/private/tmp/brassclaw-routing-overrides-r4-accepted.json`.

Preserved failures: r1 incorrectly expected a receipt to remain historical after
identical selections reactivated the same immutable generation. The assertion
now checks during withdrawal; no production behavior was weakened. R2 then
found a prefix/deployment fixture master key shorter than the existing 32-byte
minimum. The fixture now supplies a valid retained test key; security checks are
unchanged. Both failed snapshots/logs remain retained.

Component-body override selection, changed seed-base qualification/upgrade,
individual intent editing/pagination and full prefix publication coordination
remain open. Before enabling reserved-role body overrides, reconcile complete
Recipe revision review with the current per-usage approval checks and preserve
internal role input/completion contracts. Do not remove reserved-ID guards alone
or treat reviewed usages as approval of an arbitrary changed workflow. This
milestone does not mark the wider plan complete.


### Accepted complete catalogue eligibility and batched routing verification

(2026-10-13)

The neutral retained-eligibility builder no longer adds fixed instruction,
reference, visit, row, character or aggregate-byte limits below the catalogue
owner’s acknowledged preparation policy. It inspects shared immutable snapshots
once, compares complete exact revisions across distinct graphs, borrows strings
until sorting/deduplication, and allocates output with checked sizes and fallible
reservation. Inconsistency and allocation failure remain errors; no partial set
or No-Match is manufactured. Empty links/templates remain invalid.

Routing metadata verification now reads all requested canonical anchors in one
repeatable-read aggregate. Missing rows, reviewed-state exclusions and a corrupt
nonpreferred input-class row remain errors. Ordered input-class arrays and the
complete checksum document match the prior reader, including all packaged
aliases. This eliminates the per-template database round trips by construction;
there is no comparative latency benchmark or broad speed claim.

The five real-PG/retained compiler tests pass, including 22,000 repeated selected
instructions and a complete validated in-memory graph above 4096 references.
Those graph fixtures are not component-approval or production-activation evidence.
Native operator/global acceptance activates 9,000 templates and confirms exact
replies and actual Recipe outcomes at indices 0, 4096, 8192 and 8999 with one VM.
The native catalogue case took 52.255 seconds including startup, staging and
seeding; it is not a per-turn measurement. Existing routing/restart (11.942s),
authored activation (6.985s), reviewed deployment (8.742s), ordinary global
execution (16.964s) and strict affected test-target lint (58.207s) pass.

Receipt `/private/tmp/brassclaw-catalogue-eligibility-accepted.json` covers frozen
`catalogue-eligibility-20261013-r2`. It reuses r1’s unchanged engine-test evidence:
r2 changes only the native composition checksum fixture, which had incorrectly
expected just the primary template. The reference now covers every alias; no
production check was weakened. Both frozen captures/logs are retained. The model
gateway in these tests is controlled; external-model, browser and platform
acceptance remain separate.

Store/parser limits, physical database/index constraints, full Recipe review,
component-body overrides, compatible changed-seed-base upgrades and all other
full-plan gates remain open. In particular, V106’s selected-variant structural
and step-execution observations explicitly exclude semantic/task-completion
approval. Do not manufacture a full Recipe approval from them or merely remove
reserved role guards. Validator Step 6’s private, globally orchestrated review
entry point and Step 7’s eight qualified workflows must supply the missing
producer/admission path before the complete review/override cutover.


### Named internal Recipe admission verified (2026-10-13)

Host admission now supplies `TaskEntry::Chat` or a named `InternalRecipe`. The
global Python root resolves private work through `resolve_internal_recipe`,
composes its retained Recipe and sequences the same steps. Completed-turn review
uses `completed-turn-review`; it no longer calls public matching or manufactures
an `internal_review` match status. Ordinary chat text cannot select the entry.
Opaque internal completion is verified by the owning task ports against its actual
durable receipt; ordinary chat ports reject it. Exact worker envelope validation
covers both the pre-token and VM boundaries, while historical chat payloads remain
valid on the matching wire version. Protocol 15 rejects older workers before
readiness. This is an ABI revision, not a new component architecture or Tool grant.

Native protocol-15 global tests pass: completed-turn review (13.403s), lost-write
reconciliation (10.086s), partial batch (10.436s), routing/restart (11.894s), authored
activation (6.956s), reviewed deployment (9.423s), and ordinary global execution
(19.654s). These use controlled model fixtures and actual PostgreSQL/worker/kernel
paths. All26 actual service/worker tests and strict host test-target lint pass in
`named-internal-entry-20261013-r8`; exact queued/fatal input expectations include
the new entry. Isolated frame rejection (0.552s), opaque model handoff (3.689s), IBS
effects/no replay (16.037s) and focused lint (42.141s) pass in r10. Worker artifacts
are hash-verified and preserved before mandatory own-target cleanup. The isolated
workspace lock refresh changes only the network path package’s dependency list;
existing registry versions remain unchanged. Protocol mismatch checks use actual
old/new worker executables. Receipts are
`/private/tmp/brassclaw-named-internal-entry-host-accepted.json` and
`/private/tmp/brassclaw-named-internal-entry-outside-accepted.json`, with r6 native
logs retained separately. R6 full composition lint remains a recorded failure,
not a full-matrix acceptance claim.

Failures were retained and repaired in sequence: the pre-token adapter still
checked the old six-field shape; current exact shapes now include the optional
host entry. Out-of-crate fixtures were missing that field and fatal-retention
expectations omitted it; constructors and complete expectations now agree.
Adjacent prefix compile/registration issues found before task ownership was split
were corrected without disabling checks. The user now assigns prefix work to the
other chat; no further prefix implementation edits are part of this goal.

The updated goal owns simplified-v3, validator-v3 and Monty composition plans;
prefix_v3_upgrade.md is implemented independently by its owning chat and consumed
at integration. This entry transport is a prerequisite for validator Step6’s
private named selection. It does not qualify or schedule the eight validator
Recipes, approve changed component bodies, migrate mutable editors, complete
changed-seed-base upgrades or satisfy protected-root recovery GateA1. Continue
those implementations; do not mark the plan complete from this evidence.


### Complete validator subjects and pure rule aggregation (2026-10-13)

The immutable submission reader now prepares `component-validator-subject/1`
inside the caller's repeatable-read/serializable transaction. It retains the
complete typed candidate and proposed dependency envelopes, exact bytes and
references, original submission bytes/checksum and exact base-root comparison.
It never selects latest, executes candidate source or writes a review verdict.
The comparison root is explicitly not a retained old dependency-version graph
or proof of approved ancestry. Preparation bounds reject whole inputs and an
initial byte lower-bound check precedes transport duplication.

A reusable pure class-22 `aggregate_validator_checks` preload export requires
complete unique rule coverage and consistent typed outcomes/diagnostics. The
required list belongs to the retained validator; it is not candidate authority.
No Rule aggregation result by itself is trusted successful Q1 evidence.

Frozen `validator-subject-20261013-r4` passed actual retained IBS/source/preload
qualification with required IDs bound as Recipe constants, positive and negative
execution in the contained Monty worker, and strict skills/composition test-target
lint (`skills-db,test-support,migrate-from-libsql`, Rust 1.96.0). The exact final
reader passed real PostgreSQL in frozen r3, including advanced heads, distinct
historical base dependencies, invalid/missing refs, bounds, isolation rejection
and no activation writes. Its unchanged result is reused in r4. Seven existing
submission/inspection/lineage regressions passed in r2; their applicable evidence
is retained. R1's SQL parameter-width failure and r3's incorrect test expectation
are preserved as failed captures, not successful acceptance.

This closes input-preparation and aggregation prerequisites only. It does not
complete validator-v3 Step2's writer migration, eight class-specific validation,
private approved validator catalogue selection, durable review admission,
semantic/behavioral coverage, trusted Q1/Q2/activation writers or full cutover.
The legacy three-string utility validator and its seeded labels remain legacy.
The existing mechanical source inspector must be exposed through a registered
bounded generic primitive for validator Recipe use; there is no accepted
`validate_class_N` workflow Tool. Prefix work remains owned by the other chat.


### Registered mechanical source facts (2026-10-13)

`builtin.source_inspection` now has a real native registration, exact input
schema and neutral backend port. Ordinary skills-db startup binds the actual
contained parser backend to a privately retained packaged worker and the
acknowledged settings view before ingress. Parsing/compilation stays in the
existing disposable worker; candidate code is never executed. This is a generic
fact operation, not a Rust class validator or review workflow.

The matching Skill, associated preload export, ToolSkill and canonical execution
Recipe are packaged drafts in `validator_components::source_inspection_drafts`.
The actual native Tool identity is supplied separately, not invented by those
drafts. Retained IBS/source qualification verifies their binding/export pair.
No new validator route, approval, maturity label or catalogue activation is
manufactured. Exact class routing and trusted verdict writers remain open.

The backend captures the effective preparation revision and all finite worker,
source, value and frame limits per operation. It has no additional static 1 MiB
builtin fallback or wall-time ceiling. Source/null/extra-field errors, current
policy denial and capacity failures remain distinct; parser source/node resource
exhaustion is classified as Resource. Complete source/fact data is never trimmed.
Observed process/time usage and actual serialized output bytes are accounted.
Unsafe source reductions below the retained global root fail before persistence;
the error now explains that retained-root constraint rather than implying only
that a limit is too large. Packaged worker copies use an `.exe` filename on
Windows; this fixes their executable naming, not the still-open Windows loaded
native image/whole-product qualification.

Native frozen r6 proves startup registration/artifact identity, nonexecution of
source containing `raise`, full checksum preservation above 1 MiB after live
budget growth, safe reduction, unchanged settings on unsafe reduction, current
policy denial/re-enablement, actual parser node exhaustion and one retained
global VM. Native r4's unchanged ordinary matched/unmatched runtime result is
reused. Host r5 proves the unavailable backend, registered/visible package
contracts, retained packaged-copy integrity and affected-package strict lint.
The final r6 lint result is recorded by the retained validation receipt; do not
infer it from test success alone. Evidence is limited to these primitives,
draft assembly and ordinary startup; the eight complete validator workflows,
semantic/behavioral coverage, durable global review delivery and full Q1/Q2
activation/cutover still require implementation.

Contract and settings explanation: [source-inspection.md](../reborn/source-inspection.md).
Prefix implementation remains owned by the other chat. Its missing package-test
expectation was reported and repaired by that owner; no prefix implementation
was edited here.


### Durable component review intent and complete IBS flow emission (2026-10-13)

V149 atomically links every immutable submission to one immutable review request
with the same ID and exact subject checksum. A deferred database invariant
prevents committing a subject without its work reference. Upgrade backfills old
subjects without rewriting them. Application acknowledgement/read/retry verifies
that link. The narrow sealed request reader uses a coherent snapshot without
loading the full component graph again. These are work intents, not attempts,
selected validators, Q1 passes, Tool grants or activation.

Frozen review-request-r1 passes all eight real PostgreSQL submission/inspection/
lineage regressions, actual upgrade backfill and forced commit rollback, the
ordinary global Sempai candidate-submission path and strict affected lint.
Old review evidence must be reconciled by the eventual delivery owner before
new effects; a backfilled request cannot authorize replay. Global validator
selection/admission, observed coverage, trusted verdict/Q2 writers remain open.

The retained IBS emitter's stale 255-step and 16-field-path guards are removed.
Current global Python uses live data bounds rather than those obsolete quotas.
Frozen retained-flow-r1 preserves all 300 executable steps and the final result
reference plus a recursive typed 17-field path; existing native retained-input
selection and invalid-binding regressions and strict engine lint pass. This
covers preparation, not arbitrary repeated Tool effects or complete control-tree
support. Store/parser/transport guards listed in the limits audit remain separate.

The validator-coverage/1 revision-document reader and catalogue preparation
consumer pass frozen validator-coverage-r2: private selected class routes,
exact unique rules/selected steps, executed collection of every whole rule result
and pinned aggregation IDs. See [validator-coverage.md](../reborn/contracts/validator-coverage.md).
Actual preparation negative cases, the sealed full-subject PostgreSQL reader,
contained aggregation behavior, ordinary authored-catalogue activation and strict
engine/composition test-target lint pass. No declaration proves actual rule
execution, semantic sufficiency, approval or successful Q1 by itself.

Frozen validator-coverage-r5 additionally rejects ambiguous class routes,
validator metadata missing its private tag and empty activated variant lists.
The real PostgreSQL catalogue preparation regression checks precise rejection
and unchanged active selection before ordinary activation/restoration succeeds.
The reusable collector now requires each complete result's label to equal its
pinned input name. Retained IBS rejects renamed rule bindings; actual contained
Monty execution rejects swapped labels and preserves rejection diagnostics across
collection/aggregation and subsequent successful invocations. Skills, engine and
composition strict test-target lint pass. These are component preparation and
contained behavior results, not an implemented full class-review delivery or
approval writer. The eight class validators and production Q1/Q2 gates remain open.

### Immutable typed structural seed successor (2026-10-13)

`validator_components::structural_presence_draft` packages one shared pure
PythonCode export taking explicit non-null name/description/content strings.
It checks only stripped presence and emits a typed `structural_presence` result.
Installation retains it through the immutable revision store without activation,
confidence changes, legacy source rewriting or replacement of the eight old
validator rows. A later authored allocation head remains intact after reseeding.
See [structural-presence-check.md](../reborn/contracts/structural-presence-check.md).

Frozen structural-seed-r1 passes actual contained execution with quotes, Unicode,
newlines, marker-looking data and Python-looking content, unchanged packaged
source/checksum and rejection/success isolation. Ordinary matched/unmatched global
runtime passes. Its integration test initially failed compilation because a new
local binding shadowed an existing row variable; structural-seed-r2 changes only
that binding. The actual native installation/reseed test then passes with retained
authored head, original immutable revision and all eight legacy bodies/checksums.
Strict affected skills/engine/composition test-target lint passes. Unchanged runtime
evidence is retained from r1 rather than rerun. This finishes the limited typed
successor staging prerequisite, not complete class validators, trusted Q1/behavior
provenance, production review admission or approval/activation.

### Executor-owned validator checking observations (2026-10-13)

The retained executor can enable observations beneath its actual same-task
parent. `RetainedValidatorCoverage::observe` constructs an opaque audit result
only from a complete matching selection, actual ordered settled fingerprints,
whole correlated rule results and consistent aggregation. Concrete values alone,
an incomplete executor, missing/changed results or another selection cannot
produce it. Owner report limits reject overflow without granting replay.

Frozen validator-observations-r2 passes a real constrained root/task/parent/child
fixture executing the rule, collection and aggregation feeds, including the
observation negative cases. Existing native HTTP human association review also
passes. Strict affected test-target lint passes after sharing the already-loaded
test runtime module instead of suppressing its duplicate-module warning.
The observation report explicitly disclaims semantic approval and Q1 qualification;
this is not production class-review admission or a complete class validator.

The Skill guide assigns semantic agreement to author review, supported Q1 audits,
behavioral evidence and human Q2 together. A structural Q1 record's explicit
`semantic_approval=false` is not by itself an invalid audit. No blanket rejection
of that supported usage-review combination was added. Whole-class requirements,
the approved private review owner, sealed subject/attempt linkage, actual task
settlement and trusted qualifying evidence persistence remain required.

### Durable validator preparation selection (2026-10-13)

V150 retains one immutable `component-validator-selection/1` preparation record
per submitted subject, linked to its exact request/checksum. Strict readers verify
the candidate, exact validator root/closure references, private class route,
embedded variant, layout, invocation map and selection byte digest in one coherent
snapshot. Identical retries retain the original bytes; different bytes conflict.
Newer validator drafts do not replace the original selection. The operator
submission view exposes it read-only as optional `review_selection_bytes`.

Frozen validator-selection-r3 passes actual PostgreSQL retention/retry/conflict/
immutability, a successor-draft preservation case and the operator read path with
unchanged unreviewed status. All eight native submission regressions and strict
affected product-workflow/skills/engine/composition test-target lint pass. Earlier
r1/r2 test macro and local-shadowing compilation failures were corrected without
changing runtime semantics or suppressing warnings. Migration versions were checked
for uniqueness, preserving the other chat's prefix migrations.

The explicit `scope="preparation-only"` is not an approved active selection,
execution claim, attempt, Q1 verdict or activation. The production review owner
must still establish approved private catalogue selection, full class requirements,
retained implementations and attempt/effect fencing before global Monty admission.
It must capture actual checking results and settlement through the trusted writer
before qualifying Q1. Existing store/parser/inspection bounds remain part of the
outstanding resource-control cutover; this milestone does not certify those limits
or completion of the eight validators and full review delivery.

### Typed restoration of retained validator preparation (2026-10-13)

The sealed preparation reader now retains the exact verified revision graph for
typed restoration. The engine recompiles the original selected variant and
bindings, rechecks coverage and compares complete canonical selection bytes,
including derived order and retained approval references. Operator inspection
uses this restoration too. Checksummed but reordered preparation metadata remains
an explicit error; no newest selection or executable feed is substituted.

Frozen validator-restore-r1 passes actual PostgreSQL restoration, successor
retention and the checksummed-order-corruption case through the operator reader.
All eight native submission regressions and strict affected test-target lint pass.
These remain preparation checks: source qualification, actual Tool artifact
restoration, approved private catalogue selection, durable attempt/admission and
the complete trusted review/evidence/activation owner are still required before
claiming production class-review delivery or full-plan completion.

### Remove the artificial embedding input quota (2026-10-13)

Concrete OpenAI-compatible, NEAR AI, Ollama and Bedrock adapters no longer impose
the approximate fixed 32,000-byte input limit. `EmbeddingProvider::max_input_length`
now distinguishes an explicitly known byte limit from no artificial limit; cache
and adapter forwarding preserve that contract. Model token limits are not converted
to guessed byte limits. Actual model/service rejection still propagates, and
Ollama requests continue to disable truncation.

Frozen embedding-input-r1 passes 21 embedding/cache contracts, provider contract
checks and a real PostgreSQL/HTTP transport regression with complete larger UTF-8
input, cached-hit/miss ordering and one actual provider rejection without replay.
Strict affected test-target lint including the changed Bedrock feature passes.
This proves transport/caching behavior, not deployed model quality or live AWS
inference. The model-name dimension fallback, vector-cache controls, dedicated
model metadata and full class-review owner remain outstanding requirements.

### Embedding cache control and provider error privacy (2026-10-14)

The neutral cache now treats zero as disabled and exposes its actual entry
ceiling plus synchronous resizing. It allocates entries on demand rather than
reserving the configured maximum, evicts least-recent entries on shrink and
releases its map when disabled or cleared. Both operations fence cache inserts
from requests already in flight, while still returning those request results
to their original callers. A same-value edit preserves the generation; hashing
and provider awaits do not hold the cache lock.

Frozen embedding-safety-20261014-r3 passes 23 neutral contracts, including
actual single/batch await interleavings and a maximum-size logical ceiling
without upfront reservation. The real PostgreSQL/provider/HTTP caller passes
complete UTF-8 input and ordered cache behavior, actual 401/429/500/decode
failures and a refused connection. Errors preserve their classification/status
without exposing upstream bodies, credentials, input text or transport URLs;
failed requests are not replayed. Bedrock now uses the shared finite-vector
validation and sanitized SDK classifications. Strict affected-feature lint,
including Bedrock, passed; no live AWS service was exercised.

This accepts the cache API and error boundary, not the full provider/resource
cutover. Persisted model-owned dimensions/cache limits, ordinary memory-tool
embedding attachment, safe live provider replacement/vector-space isolation,
embedding transport timeout/body uptake and WebUI controls/help remain open.
Frozen embedding-safety-20261014-r4 separately passes the final NEAR AI
sensitive bearer-header change through the native PostgreSQL/provider/HTTP
caller (5.09 seconds) and strict composition lint including Bedrock (85.30
seconds). Its cache source, neutral crate inputs, workspace manifests and
lockfile are unchanged from r3; both scripts pin the same Rust toolchain, so
the 23 neutral contracts are reused. No live AWS, browser/platform or full-plan
acceptance is claimed. No prefix implementation is changed here.

### Provider-revision embedding model contracts (2026-10-14; live attachment open)

`embedding_models` now stores exact model names with positive u32 dimensions
and a nonnegative u32 cache ceiling in the provider definition. It is part of
the existing immutable definition/revision and consistent selection snapshot,
not another provider catalogue. Strict decoding rejects duplicate names, null,
missing/unknown fields and invalid numeric values. The management port retains
omitted maps, accepts explicit empty maps, enforces expected revisions and
rejects metadata incompatible with the protocol before publication.

The embedding factory consumes the selected model's stored dimension/cache
contract. Unknown names are supported when explicitly configured; there is no
1536 fallback or implicit provider/model/endpoint in the raw constructor. Zero
cache entries bypasses retention. Captured old adapters retain their selection;
this does not implement live replacement of ordinary memory consumers.

V151 changes only missing metadata, preserves explicit empty/zero/custom maps,
tombstones, roles and credentials, and retains prior revision JSON. An already
selected unknown model keeps its historic expected shape as explicit migration
data, without claiming remote-model evidence. The historical V140 initializer
is retained by the successor wrapper; only a genuinely fresh catalogue gets
installation-owned default metadata. Repeated startup does not rewrite edits.

Native r2 management/upgrade (3.31 seconds) and initialization (1.84 seconds)
passed. R3 passed real custom-model/dimension/cache-off and privacy HTTP (3.52
seconds), discovery (6.14 seconds), WebUI handler contract and all 28 dependency
boundary checks. HTTP fixture hosting moved to cfg(test)-guarded common test
support; production ingress rules were not weakened. Strict affected-package
lint including Bedrock passed on r4 (88.94 seconds). The unchanged typed model
contract tests and 17 frontend helper checks also passed. Controls include
EN/DE explanations of dimension validation, zero/off and memory payload cost;
real browser rendering, live AWS and remote installation remain unverified.

The ordinary global startup regression initially failed on a missing prefix
profile row before ingress; that failure remains recorded, rather than hidden
as a passing metadata result. The prefix owner added initialization ordering;
the separate native global-Monty regression passed on the exact r4 snapshot
(37.35 seconds), retaining VM identity across matched and unmatched turns.
Full acceptance still requires ordinary memory attachment, vector-space
identity, live role/cache/transport policy consumers and the remaining full-plan
gates. These metadata checks do not establish full v3 completion.

### Explicit embedding spaces before live memory attachment (2026-10-14)

Memory embeddings now carry a host-selected space identity independent of
dimensionality. Composition hashes the configured provider namespace/identity,
protocol/endpoint, exact model, dimension and semantic vector revision. Cache
edits preserve compatibility; an explicit vector revision separates a changed
embedding function under the same name. This is configured compatibility, not
attestation of an externally served model's weights. V153 accepts the optional
positive vector revision without rewriting historic provider or vector rows;
initial version one keeps the previous serialized contract compatible.

Indexer outputs stamp their selected provider space. VFS projection uses an
exact space-specific field and index specification; ranking queries select that
field before top-k. PostgreSQL filters candidate projections by the requested
key before loading/ranking vectors. FTS retains untagged historical chunks;
vector queries never infer their compatibility. Supplied queries require a
space and finite, nonempty, representable vectors before index effects, and
configured backends reject a mismatched provider space. Changing a query vector
clears its old tag. All provider wrappers preserve the selected identity.

Frozen embedding-spaces-r3 passes all 52 memory tests and strict seven-package
lint with Bedrock (58.22 seconds). Applicable r2 evidence passes real PostgreSQL
competing same-dimension/untagged vector isolation before ranking (4.12 seconds),
model metadata upgrade and initialization, actual configured HTTP cache edits/
vector-revision changes (4.05 seconds), ordinary global Monty startup (35.17
seconds), WebUI handler and all 28 architecture contracts. Production model/
space/migration inputs are hash-verified unchanged between these snapshots;
only an equivalent condition cleanup followed the integration run. Frontend
helper/syntax checks pass and EN/DE version guidance is present. Live AWS and
rendered browser behavior remain unverified.

This establishes the storage prerequisite. Ordinary memory-tool provider
attachment, safe live source/snapshot replacement and cache/transport policy
uptake remain required, alongside the rest of the full implementation plan.
The initial PG test fixture failure is retained: it omitted the filesystem
backend's migrations; the corrected fixture invokes real backend initialization.
No production validation or ingress boundary was weakened.


### Live memory embedding attachment and captured transport controls (2026-10-14; candidate)

The ordinary first-party registry now receives one installation-owned embedding
source slot. Runtime startup attaches the existing PostgreSQL role/definition/key
reader using the runtime configuration namespace and actual secret/session owners
before global Monty starts. Search/write capture a coherent current provider per
operation; reads/tree do not require model resolution. Stable selections reuse the
same provider/client/cache. Invalid selected configuration fails before write
effects; only an absent embedding selection permits lexical-only operation.
Previously captured requests retain their selected adapter and vector space.
No Recipe sequencing or Rust-loop fallback is introduced by this infrastructure.

Captured provider request timeout and optional response-byte capacity now reach
OpenAI-compatible, Ollama and NEAR AI embeddings. The deadline covers auth, batch
execution and response reads; construction is separately bounded before effects.
The existing bounded decoded-body reader is shared with LLM transports. Redirects
are rejected, overflow is an error, inputs/results are never truncated and failures
never trigger implicit embedding retries. Unsupported Bedrock body limits remain
explicit configuration errors. Existing provider controls have EN/DE help describing
embedding uptake and retained request limits.

Validation is pending. Frozen live-memory-source-r1 failed on test-owned runtime
shutdown through an Arc; r2 compiled but correctly returned no provider because the
fixture wrote to `default` while its runtime selected `reborn-cli`. The successor
fixture explicitly supplies its configuration namespace. Neither failure is a
production acceptance claim. Required successor acceptance covers real PG/HTTP,
registered native memory operations, live Tool blocking, source changes, invalid
configuration before effects, lexical disable behavior, transport deadline/body/
redirect failures, old captured limits, ordinary global startup and affected lint.

Immediate revision-aware WebUI effective-state acknowledgement/cache resizing,
complete approved memory Recipe chat acceptance, browser/platform/release evidence
and the other plan gates remain open; lazy request-time DB capture does not prove
those publication requirements.


Live-memory-source-r5 acceptance passed for the attachment/transport scope above:
real registered memory operations and current DB selection (19.876 s), four
actual embedding protocol transports/limits/privacy (15.964 s), adapter contracts
(0.832 s), matched/unmatched global startup (34.847 s), all 52 memory tests
(1.858 s), affected host memory contracts (0.479 s), architecture (1.586 s) and
strict affected test-target lint with Bedrock (61.750 s). Receipt:
`/private/tmp/brassclaw-live-memory-source-accepted.json`; retained source/artifacts:
`target/validation/live-memory-source-artifacts-20261014-r5`.
R3 and r4 failures were test JSON-output and whole-policy-revision assumptions;
the actual kernel/storage validation remained unchanged. Their logs are retained.

The next candidate adds actual WebUI publication through that source, synchronous
cache-only resizing without replacing the client, and retirement/disable fencing
of obsolete cache entries while accepted requests keep their results. The existing
settings owner calls preparation after commit and propagates uptake failure;
failed persistence never invokes the runtime publication. This is not yet complete
desired/effective provider/model revision coordination or a browser acceptance claim.

A related current-source finding is also repaired in this successor: memory backend
writes discarded indexing failures after document persistence. Each operation now
retains an advisory indexing-failure observation. A committed write stays successful,
but reports `indexing_status: incomplete` and an explicit warning rather than hiding
the failure or inviting a repeat of the effect. File/text search persistence remains
independent of vector-index success. Observations contain no provider payload/key
and stay isolated between operations. Actual native outage, retained read/text search,
warning isolation, API cache edits/rejection/disable and affected regressions/lint
must pass before this successor is accepted.


Live-memory-source-r6 acceptance passed: native registered memory/WebUI facade
cache/outage case (20.238 s), protocol transports/limits/privacy (16.056 s),
adapter contracts (0.827 s), ordinary global startup (35.506 s), all 52 memory
contracts (1.850 s), affected host memory tests (0.475 s), architecture
(1.563 s) and strict affected test-target Bedrock lint (59.160 s). The accepted
receipt and retained archive now identify `live-memory-source-artifacts-20261014-r6`.
This closes the exact native cache-publication and indexing-warning slice above;
complete desired/effective coordination, approved ordinary memory Recipe execution,
browser/platform/deployment and all remaining plan gates are still required.

### Retained dependency graph depth (2026-10-14; candidate)

The retained snapshot's fixed 64-level dependency traversal is replaced by an
iterative DFS. It reserves at most one iterator frame per selected revision,
checks cycles/missing dependencies/shared subgraphs and retains exact roots,
versions/checksums and association class checks. Allocation failure is explicit;
there is no native recursive stack growth or new depth quota. Engine preload and
source-dependency consumers already use iterative traversals. This changes
structural eligibility, never review evidence, activation or a Tool grant.

Pending checks include a 2,048-node exact graph with deep cycle/missing/unrelated/
duplicate cases, shared roots and dependencies, plus real PostgreSQL deep exact
selection preserving older references after successors are staged and rejecting
a deep cyclic selection. These are unreviewed retention fixtures, not activated
Recipes. The independent parser/store byte/node/count caps in the limits audit
remain separate required work. Affected caller/lint and global startup acceptance
must pass before this graph slice is accepted.


Retained-graph-depth-r1 acceptance passed: both neutral graph cases (0.344 s),
real PostgreSQL exact retention/successor/cycle checks (4.477 s), ordinary global
startup (37.148 s) and strict skills/engine/composition test-target Bedrock lint
(61.262 s). Receipt `/private/tmp/brassclaw-retained-graph-depth-accepted.json`;
source/artifacts `target/validation/retained-graph-depth-artifacts-20261014-r1`.
No review/activation claim is made for the graph fixtures, and independent
store/parser capacities remain open.


### Correlated lifecycle delivery receipts (2026-10-14; candidate)

Lifecycle delivery formerly cleared every acknowledged cursor at 4,096 marks and
used cursor alone as event identity. PostgreSQL snapshots have separate per-thread
cursor sequences. The successor keys delivery and deferred failures by exact
scope/run/cursor, never clears acknowledged marks because a count was crossed,
and serializes publication only for the same event using weakly retained gates.
Different events, including reentrant observer transitions, remain independent.
Deferred errors are non-consuming observations until that exact event is delivered;
concurrent callers cannot steal another caller's failure. All coordinator mutation
paths use the correlated error port; poisoned error state fails visibly.

Pending acceptance covers a real store with 4,100 heartbeats followed by an old
idempotent submission, controlled concurrent required publication without timing
sleeps, reentrant cancellation, all existing coordinator/cancellation contracts,
and real PostgreSQL same-cursor independent chats with observer failure isolation.
Required-observer restart/durable acknowledgement, safe receipt reclamation and
other fixed retention/lease policies remain required work; these small identities
are not retained task inputs/results, and this slice does not establish their
complete durable recovery contract. No Task/Recipe sequencing moves to Rust.


Lifecycle-delivery-r4 acceptance passed: all 139 coordinator contracts (0.905 s),
real PostgreSQL equal-cursor/error-isolation case (4.214 s), ordinary global
startup (35.761 s) and strict turns/reborn/composition test-target Bedrock lint
(93.536 s). Receipt `/private/tmp/brassclaw-lifecycle-delivery-accepted.json`;
retained source/artifacts `target/validation/lifecycle-delivery-artifacts-20261014-r4`.
Failed batches are preserved: r1 lacked the development-only Tokio multithread
feature; r2's test assumed heartbeat fanout; r3's fixture used an invalid opaque
gate ID. The successor uses 1,367 real claim/block/resume cycles and the supported
ID grammar. Validation/production heartbeat behavior was not weakened.
This accepts in-process identity/coalescing/retention and exact failure attribution,
not the remaining durable observer restart/reclamation or other capacity owners.


### Prepared identity fencing and child cleanup (2026-10-14; candidate)

The compatibility coordinator's prepared-ID guard consumed its scope entry before
checking a mismatched submission; a second wrong-scope request then bypassed the
check. Admission failures also discarded the binding, while the explicit child
submission path never released its prepared entry. Scope checks now retain the
entry on rejection, successful submissions/explicit abort release it, and child
submission accepts only its supported prepared parent or child scope. Poisoned
identity state fails explicitly. Committed submissions are woken before any
post-commit cleanup failure is returned; IDs remain data/attempt relationships,
not Tool permission. Parent-to-child handoff stays supported.

Pending tests cover repeated wrong-scope rejection without admission, preservation
after a real unavailable-profile admission error, correct-scope retry, wrong child
origin rejection, successful parent/child handoff and capacity reclaimed after child
submission/abort. This fixes an identified compatibility-contract defect, not the
complete prepared-ID live-capacity/retention owner. Source search found definitions,
tests and forwarding methods but no current ordinary product prepare_turn caller;
confirm actual ownership before adding a cosmetic WebUI setting or removing the
port. Dynamic typed-value and retained-store/parser limits remain required work.


Prepared-identity-r1 acceptance passed: all 141 coordinator contracts (0.929 s),
ordinary native cancellation propagation (227.909 s), global Monty startup
(36.650 s) and strict turns/reborn/composition Bedrock test-target lint (65.268 s).
Receipt `/private/tmp/brassclaw-prepared-identity-accepted.json`; frozen source and
artifacts `target/validation/prepared-identity-artifacts-20261014-r1`. This accepts
compatibility scope fencing and successful child cleanup, not durable reservation
ownership, configurable prepared-ID retention or the full plan.


### Captured typed-value execution bounds (2026-10-14; candidate)

Ordinary global Recipe input preparation and each requested child step now read
acknowledged resource bounds. Borrowed contract APIs preserve selected schemas,
missing/null/default rules and exact associations without cloning reviewed code
or large defaults merely to overlay limits. Step input/result validation, Tool
argument binding and behavioral fingerprints share the captured profile. Actual
Tool dispatch passes that profile into its durable intent; answer retention uses
the same profile even after a later settings edit. V155 retains depth/node/byte
bounds as immutable intent data and replaces the journal's fixed 64 MiB checks
with the admitted byte bound. Historical intents retain their previous profile;
no effect is replayed and no scope or component approval changes.

Pending acceptance covers 300,000-node retained data above the former 262,144-node
consumer limit, explicit reductions and schema-invalid rejection, unchanged
fingerprints/contracts, ordinary global replies/history with two effective live
profiles and immutable persisted bounds, plus affected native/lint consumers.
This slice does not remove the transport/schema parser's 64-level depth support
restriction or immutable metadata caps. Other trusted review/qualification helpers
still need explicit profile propagation and complete durable recovery/dispatch
revision coordination. These remaining owners must not be represented as shipped
or adjustable merely because the WebUI already exposes value controls.


Typed-resource-r1 retained-input acceptance passed (six cases, 9.400 s); the
composition test build exposed a test-only u64-to-i64 conversion and a directly
included native fixture missing the new invocation profile argument. Those are
corrected, and all standalone `tests/monty_control` journal callers/capacity
constructors now declare their explicit test profile or carry the actual
executor-captured profile. The failed batch is retained, not full acceptance.
Source review also replaced unbounded journal JSON serialization with bounded
streaming and removed the extra answer graph copy. The PortAnswer envelope does
not consume the returned value's node/depth budget; complete escaped/envelope
bytes still obey the journal byte bound. R2 adds exact encoded-byte/escaped-data
regressions and actual kernel/PG late-answer retention with a one-level/five-node
profile, as well as global live-profile and standalone affected-caller checks.


Typed-resource-r2 acceptance passed: reused six retained-input cases from r1
(9.400 s), bounded encoding (1.350 s), ordinary global Monty live profiles with
immutable journal bounds (37.086 s), strict affected Bedrock test lint (73.382 s),
all six standalone retained-execution cases (124.015 s), real kernel/PG late-answer
retention after cancellation (16.204 s), actual global IBS/effect retention
(15.745 s) and strict standalone test-target lint (17.779 s). Receipt
`/private/tmp/brassclaw-typed-resource-accepted.json`; frozen source and retained
artifacts `target/validation/typed-resource-artifacts-20261014-r2`. The original
r1 composition build failure is retained and not described as acceptance.
Separate EN/DE help edits explain step/answer retention, encoded journal bytes
and the still-supported transport depth maximum; both files passed Node syntax
checks and diff review. Browser acceptance, other trusted review-helper profiles,
transport/parser depth support and the remaining plan gates stay open.


### Behavioral review retains each step's data profile (2026-10-14; candidate)

Executor-owned observations now retain the exact acknowledged depth/node/byte
profile captured when their step began. Workflow and usage behavioral-review
producers compare expected values with that observation's profile, instead of the
old fixed checksum limits or a later step's current settings. Evidence records
include only the finite profile and existing fingerprints, never raw values or
a caller-provided success/profile assertion. Exact selected revisions, order,
failure/negative expectations, persistence integrity and independent semantic
Q1/human Q2/activation contracts remain unchanged.

Pending native coverage uses the actual global Recipe helper, selected preload
exports and same-task parent/child contexts to carry a 300,000-item result into
the next step after changing bounds. It verifies complete data, per-step retained
profiles, immutable persisted workflow evidence, wrong-value comparison and
oversize expectation rejection without manufacturing activation/task completion.
A separate real kernel/PG JSON usage preserves a 300,000-node Tool answer and
exact negative output expectations through behavioral review. Its numeric data
fits the primitive's existing input/output ceilings; this test does not certify
removal of those separate first-party ceilings.

Source inspection additionally confirms fixed shared first-party input/output
1 MiB, coding input 6/21 MiB and manifest wall-time 5 s policies in
`first_party_tools/mod.rs`. They require an actual live kernel/handler/profile
cutover and upgrade of retained primitive metadata, with adjustable defaults and
user explanations; they must not be bypassed merely to make this review test pass.
Remaining transport/parser/store capacities and these Tool-policy owners stay in
the required limits audit and must close before full plan/release acceptance.


Review-data-profile-r1 compiled, but its first native case correctly rejected
the new draft's input contracts for missing explicit `checks` arrays before
execution. The fixture now declares them for each input; parsing and production
validation were not weakened. The failed source/artifacts/logs are retained.
R2 must establish the complete handoff and actual usage/review/persistence cases
plus affected ordinary caller and strict root/standalone lint before acceptance.


Review-data-profile-r2 acceptance passed: real Monty complete 300,000-item
handoff across an acknowledged inter-step reduction with retained per-step
profiles (5.520 s); real kernel/PG large answer and exact negative expectations
(4.733 s); existing behavioral persistence/failure evidence (6.110 s); ordinary
global Monty startup (35.618 s); strict affected Bedrock test lint (55.649 s);
all nine standalone review cases (58.676 s) and strict standalone lint (3.395 s).
Receipt `/private/tmp/brassclaw-review-data-profile-accepted.json`; frozen source
and artifacts `target/validation/review-data-profile-artifacts-20261014-r2`.
The first batch's missing fixture `checks` rejection is preserved. No validation
rule, review success gate or warning was suppressed.

This accepts resource-profile coordination in executor observations and the
usage/workflow behavior producers. The fixtures remain unapproved drafts; no
activation, semantic review, human Q2 or product completion is inferred. Other
bootstrap/prefix review helpers, transport/schema/store limits, global Tool
technical-policy/WebUI publication and the remaining plan gates still require
implementation and their corresponding acceptance.


### Global Tool-policy management port (2026-10-14; candidate)

The legacy Tools permission UI reads registry defaults/scoped permission modes;
its production facade has no attached permission store, so mutation is unavailable.
It is not the global UUID-keyed policy source used by retained Monty Recipes.
A distinct instance-wide management port now lists existing global policy rows
and changes enabled/block state by exact Tool UUID and expected revision. It
creates no rows, maps no aliases to authority, and grants no component approval.
Retained declared aliases are display metadata; a new draft does not replace a
running task's identity. V156 constrains policy rows to class-0 heads without
resetting existing overrides/revisions.

The actual runtime facade attaches this port. GET `/api/settings/tool-policies`
and PUT `/api/settings/tool-policies/{tool_id}` use product handlers and the
existing instance-token operator route classification/mounting. Revision fields
use exact quoted-decimal wire values, preventing JavaScript rounding. Concurrent
edits, stale/invalid/unknown inputs and database rejection remain distinct and
leave coherent state. Native acceptance must cover database CAS/rollback/retained
aliases/class integrity, actual router/facade/kernel blocking under an independent
operator data scope, bearer ingress exclusion of ordinary session credentials,
existing startup/block restoration and affected contracts plus strict lint.

This port is a cutover prerequisite, not a declaration that all Tools are unified.
The ordinary model-call factory still uses legacy GrantAuthorizer and separate
capability-grant construction; registered primitive identity enrollment, full live
technical parameters, retained alias/version coordination and the coordinated
no-operation-approval dispatch cutover remain required. Do not replace the existing
UI with a misleading global toggle until those callers are coupled to this source.
No Rust workflow loop/fallback or approval bypass is introduced here.


Instance-tool-admin-r1 passed native CAS/class/rollback (4.214 s) and actual
router/facade/global-kernel blocking, restoration and startup (36.125 s). A later
default-feature library test build exposed an existing bootstrap dependency gap:
PostgreSQL seeding used the Skill revision store and a runner-gated catalogue ID
helper without declaring that dependency. PostgreSQL composition now includes
the required Skill/store dependency. The unchanged installation ID algorithm
lives in the component-library bootstrap owner and is shared by seeding and the
optional runner; no seed is skipped, no check bypassed or activation manufactured.
This does not silently enable a different runtime/runner feature. R2 must cover
the supported default library consumer plus full candidate/API/ingress/lint checks.
The preceding failed batch is preserved and is not full acceptance.


Instance-tool-admin-r2 passed native CAS/class integrity (4.411 s), actual router/
facade/global-kernel blocking and restart (36.261 s), default library revision wire
(0.277 s), complete WebUI descriptor contracts (0.166 s) and existing handlers
(6.400 s), real bearer/operator classification (0.350 s) and operator-route mount
exclusion (0.013 s). Strict lint then found duplicate loading of a shared native
PostgreSQL fixture. The new test now imports the crate's existing canonical native
fixture; no warning is suppressed. R3 retains unchanged behavioral evidence and
must run the changed fixture plus strict affected lint before acceptance.


Instance-tool-admin-r3 acceptance passed: canonical native fixture CAS/rollback/
class integrity (4.273 s), strict affected Bedrock test-target lint (53.929 s),
with unchanged r2 actual router/global-kernel/restart (36.261 s), exact wire
(0.277 s), complete descriptor (0.166 s) and handler contracts (6.400 s), and
real bearer/mount exclusion (0.350/0.013 s) retained. Receipt
`/private/tmp/brassclaw-instance-tool-admin-accepted.json`; source/artifacts
`target/validation/instance-tool-admin-artifacts-20261014-r3`. The r1 default-
feature dependency failure and r2 duplicate-fixture lint failure are preserved.
Both are repaired; no check/seed/warning was suppressed. The feature repair keeps
all existing installation UUIDs and separates mandatory PostgreSQL metadata
seeding from optional runner selection.

The global policy management prerequisite is accepted through the actual API and
existing retained dispatcher. It does not establish ordinary model-requested Tool
identity enrollment/global authority, full technical parameters, the final UI
replacement, the full authorization cutover or overall plan completion.


### Host-selected instance authority in the model-call adapter (2026-10-14; r3)

`DispatchAuthorityModel` is a nonserialized host contract, not a request field,
operator setting or permission. `InstanceToolAuthorizer` reports `InstancePolicy`;
`DefaultHostRuntime` forwards its immutable authorizer's model. The adapter captures
that model from the trusted host with its private surface snapshot. Instance-policy
invocations discard scoped grants while preserving task/data scope, host-selected
mounts, provider trust evaluation and fresh invocation identity. Legacy hosts keep
their existing scoped behavior. Disabling operation approval alone does not select
the new model. Actual dispatch still passes through current kernel policy and
technical enforcement; no new grant, approval or workflow fallback is introduced.

The native caller regression uses the actual model-call port factory, registered
JSON primitive, real kernel, versioned stable-UUID policy source and PostgreSQL
input/result storage. A foreign stale scoped grant cannot authorize or prevent
the instance-policy call. The same advertised surface survives live block and
effect-ceiling changes; denied calls retain no successful result, and a subsequent
allowed revision returns the real parsed JSON result. This source is a controlled
policy fixture, not installation identity enrollment or component approval.

R1 compiled but its fixture omitted the visibility policy (the host's default
hides every capability); the corrected fixture uses the production adapter's
visibility policy. R2 then exposed a real existing denial-conversion defect:
the fixed `authorization` category was incorrectly validated as arbitrary text
and rejected as a sensitive marker. R3 represents it as a known typed denial
category with the intended exact wire value. Unknown reasons retain all sensitive
marker checks; credential-shaped suffixes remain rejected. Neither failed batch
is accepted evidence. Their logs/artifacts are preserved.

R3 native adapter/kernel/PG acceptance passed (4.210 s), ordinary global Monty
matched/unmatched/restart regression passed (36.564 s), all five invocation-context
cases passed (0.321 s), 83 existing capability-port cases passed (0.055 s), all 67
turn-host contract cases passed (0.235 s), and strict affected Bedrock test-target
lint passed (66.650 s). Receipt `/private/tmp/brassclaw-instance-policy-adapter-accepted.json`;
source and executables are retained under `target/validation/instance-policy-adapter-20261014-r3`
and `target/validation/instance-policy-adapter-artifacts-20261014-r3`.

This completes the host/adapter prerequisite, not Phase 8 or the overall plan.
Ordinary factories still construct `GrantAuthorizer`; local-dev wiring constructs
scoped grants and the pure-PostgreSQL factory omits provider trust. Complete actual
registered Tool/retained-alias enrollment and the durable current-policy source,
attach instance authority before admission/ingress, and coordinate those factories'
context/surface construction before claiming production uptake. Preserve kernel
sandbox/network/secret/resource checks and existing task selections. Full technical
settings and the final WebUI replacement remain required. Do not switch these
factories to an empty/incomplete policy source or claim an API toggle controls
unregistered Tools.


Affected architecture boundary acceptance also passed against the same frozen
source (1.541 s). Its executable, hashes and log are retained under
`target/validation/instance-policy-adapter-architecture-20261014-r3`; the acceptance
receipt includes this case. No dependency-boundary exemptions were introduced.
All eleven changed dispatch/contract/compiler inputs in the working checkout
were verified against the tested r3 snapshot. Later plan prose is not compiler
input and does not invalidate this evidence.


### Native startup registration preparation limits (2026-10-14; r4 accepted)

Source inspection of the production identity path found an extra fixed 1 GiB
artifact ceiling in `factory.rs`, before the global settings owner was started.
Its native constructor also used default verifier output and response deadlines.
The ordinary PostgreSQL factory now reads one complete persisted execution-limit
snapshot before native capture, converts it with the existing preparation adapter,
and explicitly passes artifact bytes, verifier output bytes and response timeout.
The native registration constructors require these explicit verifier limits;
no production caller can silently select their former verifier defaults. A failed
or invalid settings read stops startup before native registration. First-install
absence uses the settings store's canonical defaults; local-only fixtures supply
the same explicit defaults. No Tool grant or component approval is created.

The existing artifact-size control already has EN/DE help. Its explanation now
states that the saved limit applies at startup before Tools load, while retained
artifacts remain unchanged. The response-time and captured-output help already
explains native verification and required termination acknowledgement.

The actual factory/native test persists a one-byte artifact ceiling through the
supported settings store, observes real capture rejection, restores capacity and
verifies the actual native registration and retained artifact. Lowering the next
capture limit rejects a new build without resizing/replacing the old registration.
On macOS it additionally persists a one-byte verifier-output limit and observes
actual OS-verifier rejection before restoration. An unsupported persisted settings
key exercises failed source validation without bypassing database constraints.
It restores the exact valid payload with proper revision increments. Worker
readiness remains off during this factory-only acceptance.

Failed batches are retained: r1's deliberate corruption omitted the required
revision increment; r2 attempted an invalid zero byte ceiling which the database
correctly rejected. The successor respects both guards and uses an unsupported
persisted key to test read validation. R3 compilation exposed two standalone
support callers missed by a crate-only search; both now pass explicit fixture
preparation limits and have their own actual standalone acceptance. No constraint,
check or warning was suppressed or disabled.

R4 passed all eight local-factory cases including the native capacity/verifier
case (8.472 s), the ordinary global Monty regression (36.135 s), strict affected
Bedrock test-target lint (57.912 s), default-feature strict library lint (32.364 s),
all six standalone retained-execution cases (124.128 s), all nine standalone Q1
review cases (56.928 s) and their strict lint (10.359 s). EN/DE JavaScript syntax
and diff checks passed. Receipt `/private/tmp/brassclaw-native-startup-capacity-accepted.json`;
source/artifacts are retained at `target/validation/native-startup-capacity-20261014-r4`
and `target/validation/native-startup-capacity-artifacts-20261014-r4`. All seven
changed native registration/test/help inputs in the worktree match that snapshot.

This establishes configurable native startup preparation and preservation of
already captured artifacts on this local platform. It does not establish Windows
or Linux execution, all installation Tool identity enrollment, general live
technical Tool parameters, factory instance-authority attachment, the remaining
scheduler/transport/store limits or overall plan completion. In particular,
`native_image.rs::loaded_file` still explicitly lacks Windows support; this needs
an actual loaded-image contract and native acceptance, not a mutable-path fallback.


### Explicit pending metadata and the legacy prompt repair boundary (2026-10-14; r2 accepted)

Tracing installation Tool identities exposed three legacy inserts that silently
changed a caller's `validation_status=pending` into `validated` when `source=system`.
Tool, ToolSkill and Skill stores now honor their existing explicit status input.
Conflicting inserts retain the original identity/state rather than rewriting it.
A source label does not establish native registration, immutable v3 approval,
a complete Skill interface or an active routing selection. This change does not
rewrite historical rows or manufacture approvals.

R1 native metadata acceptance passed, but the ordinary restart regression then
found a second incorrect coupling: the prefix-generation Tool deliberately has
pending raw metadata and separate controlled bootstrap qualification, while a
legacy boot helper rejected every source-labeled pending row in the seed scope.
That helper is now explicitly `repair_packaged_legacy_prompts`. Its responsibility
is limited to compiled-in legacy prompt projections and exact live-prose hashes;
changed protected prose or an incompatible prompt state remains an error.
Unselected draft metadata remains untouched. It is not promoted and is not a
boot prerequisite by label. Required active workflow qualification remains in
the subsequent global catalogue owner before workers/ingress start. This is not
a replacement for the full Phase 0a/3a/7/8 activation and recovery gates.

The actual PostgreSQL regression checks all three explicit pending statuses,
conflicting inserts, absence of pending intent projection and immutable revision
creation, and rejection/preservation of altered protected prose. The ordinary
global matched/unmatched/blocked-Tool restart regression and existing supported
active-catalogue guards passed. R2 timings: native metadata/repair 4.158 s,
global runtime/restart 36.102 s, catalogue activation/forged-approval guards
16.437 s, strict affected Bedrock test-target lint 52.990 s. The r1 failed restart
batch remains preserved; no row status was rewritten to make it pass and no
integrity/active-catalogue check or warning was suppressed.

Receipt `/private/tmp/brassclaw-pending-seed-metadata-accepted.json`; source and
executables are retained under `target/validation/pending-seed-metadata-20261014-r2`
and `target/validation/pending-seed-metadata-artifacts-20261014-r2`. All eight
changed current writer/repair/test/caller inputs match the tested snapshot.

Remaining: Recipe/PythonCode/ExtensionCatalogue legacy inserts still derive
status from source and lack the same explicit insert-state input. They need the
coordinated authoring/bootstrap/projection cutover, not a label-based approval
exception. Complete installation qualification across all component classes,
immutable workflow approval, native Tool/retained-alias identity enrollment and
ordinary factory global-authority attachment remain implementation work. No
external maintainer Q2 gate is requested for shipped first-party components.


### Separate authored insertion from installation projection (2026-10-14; r1 accepted)

The remaining Recipe, PythonCode and ExtensionCatalogue stores previously derived
`validated` status from a `source=system` field. Ordinary insert and transactional
`create_and_submit` now always create pending metadata, irrespective of source.
A deterministic-looking Recipe marker cannot assign maturity or Wilson evidence
on the authored path. Installation owners explicitly call the private Rust
`insert_installation_projection` methods; only those named call sites choose the
legacy validated/tier projection. Their source consistency check is not provenance.
No serialized request field selects this path, and it does not create immutable
v3 approval, a registered primitive or activation. Full trusted bootstrap still
requires its actual integrity/Q1/behavioral contracts; authored changes still
require their own trusted Q1/behavior/human Q2.

The builtin, builtin-host and Zencoder installation owners now use the explicit
projection method. Model/operator/DocPlan authoring stays on pending insertion.
Historical metadata/projections are not rewritten. This preserves the shipped
legacy projection while eliminating field-derived state promotion; it is not a
replacement for the complete component cutover.

Tracing queue submission exposed another real defect: ExtensionCatalogue inserted
its row before separately enqueuing review. It now uses the same connection and
transaction for the row and queue intent, like Recipe and PythonCode. A database
queue rejection rolls back the newly inserted catalogue; it cannot leave an
orphan while returning failure.

Actual PostgreSQL checks cover plain/queued system-labeled authored inserts for
all three classes, pending state/tier/queue identity, misuse rejection of the
installation method and deliberate class-23 queue rejection with zero orphan
rows. The prior explicit-pending and protected-prompt tests also pass. Timings:
two native cases 5.912 s; ordinary global match/unmatch/blocked-Tool restart
36.163 s; supported active catalogue/forged-approval guards 16.394 s; strict
affected Bedrock test-target lint 53.056 s. No warning, queue error or approval
check is suppressed. Receipt `/private/tmp/brassclaw-explicit-installation-projection-accepted.json`;
source/artifacts `target/validation/explicit-installation-projection-20261014-r1`
and `target/validation/explicit-installation-projection-artifacts-20261014-r1`.
All seven changed current writer/owner/test files match the tested snapshot.

All six legacy stores inspected in this work no longer contain a source-label
`CASE` that silently promotes an ordinary insert. Legacy validated projections
are still not proof of v3 approval. Complete all-class qualification, immutable
workflow/version enforcement, Tool identity/retained-alias enrollment and ordinary
factory current-policy attachment before claiming Phase 8 or overall completion.


### Windows loaded-image candidate (2026-10-14; local checks passed)

The native image owner now has a Windows candidate using a pinned main-module
mapping and its OS-reported device filename. It opens that file while denying
write/delete sharing and rechecks the mapping before staging. It does not use a
mutable `current_exe` path or a request-supplied artifact as production identity.
UTF-16 names grow on truncation, with allocation/representation errors explicit.

Existing native-image regressions on this macOS host and strict affected lint
passed. A local Windows GNU test-target cross-check also passed. Receipt:
`/private/tmp/brassclaw-windows-native-image-candidate-checked.json`; frozen inputs
and executable are retained under `target/validation/windows-native-image-20261014-r1`
and `target/validation/windows-native-image-artifacts-20261014-r1`.

This is not Windows runtime acceptance. The existing Ubuntu/macOS/Windows native
CI gate now also runs native-image tests, including the Windows-only real copied
executable rename/replacement and write-fencing cases. Their actual Windows
results are required before claiming this identity contract works there. Linux
execution and complete installation/retained Tool qualification remain open.


### Instance Tool-policy browser editor (2026-10-15; r3 accepted)

Settings → Tool Permissions now reads the existing UUID-keyed instance policy
API and edits only `enabled`, with the exact quoted `expected_revision` retained.
The old scoped capability-permission client/editor and unused manager are removed.
There is no `ask` choice. Retained declared aliases and stable UUIDs are displayed;
this editor creates no Tool, policy row, approval or activation. EN/DE text explains
instance-wide allow/block, subsequent dispatch, completed actions and independent
sandbox/network/secret/resource checks. Ordinary model-call registration and its
remaining authority cutover are still separate required work.

Mutation handling fences double events immediately. Failures, stale revisions
and inconsistent acknowledgements lock editing until an explicit successful
refresh. Refresh reads current state; it never rebases or repeats the old edit.
An old read is cancelled before publishing an exact successful acknowledgement
into the browser cache. Malformed/duplicate identities and imprecise numeric
revisions fail visibly instead of becoming editable defaults.

The existing native global runtime test exposes its real router/facade/PostgreSQL
to the optional `tests/browser/instance-tool-policy.mjs` acceptance. Chromium runs
original React/Query/static modules. It checks real allow/block persistence, a
second operator's concurrent write, 409 fencing and refresh without replay. A
transport fault closes the browser response after the real database commit;
one application `fetch` is issued and one revision is saved. Chromium can retry
that idempotent PUT internally, always with the same stale revision; CAS rejects
any repeated mutation. The test separately records application calls, complete
HTTP attempt bodies and persisted revisions. No response/result is fabricated.

R1 failed because the browser inherited German while the test selected an English
label. R2 found the legitimate transport retry after the deliberate connection
loss; its fixed HTTP-attempt assertion did not describe the application invariant.
The corrected fixture pins its language and verifies exact application requests
and one durable edit across transport retries. Both failed batches remain retained;
no product integrity, dispatch, policy or approval checks were weakened. R2/r3
reuse the unchanged r1 Rust executable instead of compiling unchanged inputs.

R3 native browser plus surrounding matched/unmatched/blocked-Tool/global restart
regression passed (43.263 s). Strict affected Bedrock test-target lint passed
(91.554 s). Four client-only precision/schema/error cases, changed JS syntax and
diff checks passed. Receipt `/private/tmp/brassclaw-instance-policy-ui-accepted.json`;
source/artifacts `target/validation/instance-policy-ui-20261015-r3` and
`target/validation/instance-policy-ui-artifacts-20261015-r3`. All nine changed
current acceptance inputs match this tested snapshot; the legacy manager remains
removed.

Scope: actual browser component and native actor-injected router/facade/database,
not a new bearer-authentication claim or final packaged CLI/release qualification.
Existing operator/bearer boundary evidence remains independently applicable.
Persistent native Tool/alias enrollment, ordinary factory global authority,
complete technical parameters, fixed scheduler/store/transport limits,
Linux/Windows acceptance and full-plan/release/test-machine completion remain open.


### Global pending host-call capacity (2026-10-15; r2 accepted)

The former startup constant of 128 now has an explicit
`execution_limits.max_pending_host_calls` setting. V158 preserves explicit
operator values, adds 128 only where missing, advances those rows' revisions,
and extends the preceding database default without invoking that expression
during migration. The new database/type contract rejects invalid/nonpositive
or unrepresentable counts. Worker count remains an independent scheduler issue.

Startup consumes this exact setting. Live publication includes it in the existing
complete durable settings candidate. The sole worker validates it against actual
pending futures, late answers still retained in the VM and the root's worker
count before any durable write. Unsafe reductions reject the edit while retaining
all continuations and the previous revision; accepted edits update the existing
root without executing Python or replacing its identity. The actual worker
receipt and Rust watch expose its effective limit/occupancy/minimum. Product
status derives this field from that observation, not the desired database value.
EN/DE WebUI help explains the control, and status displays live pending calls.

Private worker protocol 16 carries the new settings candidate and capacity
receipt; old workers must be rejected rather than falsely acknowledging uptake.
Existing callers that supply no pending-call edit retain their actual current
limit. The native interpreter regression uses the supported worker-coroutine gather
pattern with 129 real work-wait futures. It checks growth beyond the former
product constant, rejects smaller limits without losing keys and accepts a safe
reduction. This is a mechanical static-worker probe, not implementation of live
worker-slot resizing. PostgreSQL migration and ordinary WebUI/global runtime
tests cover persistence, rejected precommit reductions, acknowledged growth/
reduction and existing worker identity. Non-default 257 is configured before
ordinary startup and explicitly observed again after same-database restart.

R1 compiled and passed native migration (5.009 s), authenticated live edits
(106.976 s), ordinary global/restart (37.146 s) and strict root lint (66.733 s).
Its first mechanical probe used an unsupported direct-future gather expression
and failed; no interpreter/authority boundary was relaxed. R2 uses the supported
worker-coroutine gather and adds explicit non-default startup/restart acceptance.
The unchanged worker, migration and live-edit evidence are retained and reused.
An initial wrapper syntax error occurred before any r2 build and was corrected.
Both failed records remain; no command, warning or product check was suppressed.

R2 passed non-default global/restart (38.884 s), strict affected Bedrock test-target
lint (91.446 s), all22 process contracts including129work waits (6.864 s), all26
service contracts (1.506 s), all6 retained execution cases (124.285 s), all9 Q1
cases (58.025 s) and their strict lint (15.913 s). The remaining affected actor
consumer passed all15 actual transport cases and strict lint (0.793 s/0.441 s).
All executables are retained/verified. Receipt
`/private/tmp/brassclaw-pending-host-calls-accepted.json`, source/artifacts
`target/validation/pending-host-calls-20261015-r2` and
`target/validation/pending-host-calls-artifacts-20261015-r2`; actor artifacts are
in `pending-host-calls-actor-artifacts-20261015-r2`. Twenty current changed
runtime/interface/test inputs match the tested source. EN/DE help received a
prose-only clarification of idle work waits after freeze; syntax checks passed.

This closes this local pending-call control and its actual worker/Rust uptake.
Rendered resource-form interaction, packaged CLI/platform/release qualification,
worker-slot sizing, remaining limits, Tool identity/authority cutover and overall
plan/test-machine completion remain required. No runtime speed improvement or
full-plan acceptance is claimed.

## 2026-10-15 — Reusable global-worker scheduling prerequisite

The isolated Monty extension now exposes the private
`asyncio._replace_sibling(slot, coroutine)` operation. An executing gather child
can append a coroutine at exactly the next group slot or replace an already
completed slot. It cannot replace an active sibling, select another group,
reuse a started coroutine or create a detached task. The original group waiter,
host futures and normal failure propagation remain. Replacement releases the
previous coroutine and result instead of retaining a growing result history.
Python still owns scheduling and Recipe sequencing; this adds no Rust workflow
loop, Tool grant or alternate VM.

Reference-count diagnostics exposed an existing ownership defect when a
suspended one-shot or REPL snapshot was destroyed. The owning snapshot now
releases stacks, frame namespaces, scheduler references, pending effects and
globals before destroying its heap, without executing Python or claiming an
external effect was cancelled. Resume transfers the original owned state;
explicit REPL recovery retains the original namespace. Dump fields retain their
existing non-null representation. No dump ABI or application-protocol bump was
needed for these ownership changes.

Accepted evidence: `tests/monty_scheduler` enables the actual interpreter's
`memory-model-checks` and `test-hooks`. All eight new cases pass, including
2,000 replacements, active-slot rejection with no rejected code execution,
extension/failure propagation, an actual pending host checkpoint with only two
remaining scheduler tasks, dump/load and REPL namespace recovery. The three
existing native coroutine-accounting cases also pass. Strict test-target Clippy
passes. Source/artifacts are retained under
`target/validation/sibling-scheduler-20261015-r5` and
`target/validation/sibling-scheduler-artifacts-20261015-r5`; receipt:
`/private/tmp/brassclaw-sibling-scheduler-accepted.json`. Nine current own code
inputs and both executables match their recorded hashes. The refreshed
`CONTROL.patch` reconstructs all 228 vendored source/manifest files from the
hash-verified upstream checkout. Initial compiler/test failures remain recorded;
no diagnostic was silenced or reference-count checking disabled.

**Worker sizing remains unfinished.** Do not expose a cosmetic admission setting
or use this operation in the production root before these owning paths are
implemented together:

1. Retire host accounting entries for actual completed coroutine contexts while
   preserving cumulative execution/preparation/adaptation totals. The current
   `RootControl` retains a bounded lifetime map, which rejects repeated context
   replacement. Retirement must follow the interpreter's real lifecycle and must
   never discard an active task budget, pending host answer or unresolved effect.
   Inspect the current task-release handshake: it happens at the next work wait,
   so returning a worker immediately after `finish_task` cannot simply drop its
   still-owned account.
2. Implement a permanent Python capacity coordinator in the same owning gather.
   Growth starts actual workers; reduction drains existing work before removing
   workers. Reuse completed slots instead of accumulating nested gather frames.
   Preserve active Recipe selections, waits, attempt fences and intermediate
   values. Shutdown must also stop the coordinator and all retained workers.
3. Coordinate the configured admission capacity, actual coroutine capacity,
   context bound and pending-call minimum through the complete revision-checked
   settings transaction. Preflight before persistence, publish actual worker
   uptake, report desired/effective/draining status, and preserve one root VM.
   Reject impossible simultaneous reductions; do not acknowledge uptake from
   Rust semaphore capacity alone.
4. Exercise ordinary matched/unmatched tasks, provider/MCP waits, concurrent
   settings edits, cancellation, growth, draining reductions and shutdown through
   the paired native product worker. This isolated interpreter gate does not
   establish product resizing, full resource controls, fatal recovery, Windows
   execution, complete v3 acceptance or release/test-machine completion.

## 2026-10-15 — Actual coroutine-account retirement accepted

`ExecutionControl::context_exited` now reports real spawned-coroutine success or
unhandled failure after closing the final execution interval. Snapshot creation,
suspension and destruction do not report an exit. Retirement rejection latches
an uncatchable control failure and does not clear prior failure or cumulative
clocks. Existing controls have a default no-op hook; the global host implements
its ownership checks explicitly.

The global host moves exited-context execution, preparation and adaptation into
separate cumulative totals and removes the live map entries. It refuses to
retire any context still owning an admitted task. Answer import preparation can
remain owned by a coroutine that has just exited; the host retains one pending
retirement through that serialized scope's final sample, then retires it when
the exact scope closes. This prevents dropping preparation time or falsely
rejecting ordinary idle-worker shutdown. It acknowledges neither task completion
nor cancellation of an external effect and releases no durable attempt state.

Accepted source/artifacts:
`target/validation/context-retirement-20261015-r3` and
`target/validation/context-retirement-artifacts-20261015-r3`; receipt
`/private/tmp/brassclaw-context-retirement-accepted.json`. All13 actual global-host
cases pass (0.558 s), including 2,000 replacements, real work-wait readiness,
live-map plus retired-total equality and shutdown with retained VM identity.
An unexpected worker exit retains its unfinished task budget and fails closed.
All10 scheduler/ownership cases and3 existing coroutine-accounting cases pass
(0.406 s/0.400 s), including real-exit callbacks, no callback on snapshot
teardown and uncatchable callback rejection. Both strict test-target lints pass
(8.619 s/4.975 s). Five current changed inputs and three retained executable
hashes match the accepted source; the vendor patch reconstructs all228 files.

The initial host checks caught retirement before the answer-import scope closed;
that ordering was repaired and its failed evidence retained. The next check
caught a test coordinator returning a list as the final root result, which the
existing host correctly rejects; the fixture now uses the normal async main
with a `None` result. No production boundary was weakened.

This closes the context-retirement prerequisite in the preceding milestone.
Live worker sizing, actual coordinator/draining behavior, settings publication,
paired native worker/product acceptance and the full plan remain unfinished.
Retirement must never be used to discard an unfinished task. The existing
`release_task` work-wait handshake still deserves separate completion/receipt
fencing review before expanding the root pool; its current implementation removes
an account without inspecting its phase. The ordinary protected Python root
calls `finish_task` before returning to that wait, but a root host invariant must
not rely only on that source ordering.

## 2026-10-15 — Exact completion acknowledgement fences worker handoff

The global host now distinguishes a pending `finish_task` call from other task
ports. Its exact retained continuation/context must receive the trusted service's
null completion acknowledgement before the task enters the finished phase.
The service already emits that acknowledgement only after the completion owner
confirms its exact outcome. Failed, aborted, withheld or non-null answers do not
qualify. A subsequent `await_next_task` releases an account only from the finished
phase; otherwise the VM fails closed and retains its task ownership and actual
rejected answer. No completion, effect or Tool permission is inferred merely
from source ordering or a worker reaching the next wait.

An old admission fixture intentionally swallowed a task-port error and resumed
its work wait without finishing. Its expectation now reflects the required
invariant: oversized/duplicate admissions remain rejected before binding, and
that unfinished handoff fails with its original task budget retained. Positive
reuse is verified through the ordinary native product instead of inventing a
successful durable finish for that fixture. Negative root checks also cover a
refused finish answer and an invalid completion payload.

Accepted receipt: `/private/tmp/brassclaw-task-handoff-accepted.json`; final
source/artifacts `target/validation/task-handoff-20261015-r3` and
`target/validation/task-handoff-artifacts-20261015-r3`. All14 global-host cases,
27 real-worker service cases and22 process cases pass. The additional service
case reads a real input file, writes and verifies the real reply, then has its
completion owner refuse confirmation. The reply remains written exactly once;
queued work is fenced without dispatch or replay, and the service retains its
unfinished task state while settling held I/O before containment completes.

The paired worker was rebuilt from the changed source. Native PostgreSQL
ordinary matched/unmatched/MCP chat and same-DB restart acceptance passes
(38.015 s), preserving root identity across task reuse. Strict affected
host/service/process test-target lint passes. The final additional test changed
only `service_host.rs`; all other9148 frozen inputs and reused executable hashes
were verified unchanged before reusing the native result. The updated service
suite/lint pass (1.918 s/0.653 s). The validation wrapper initially rejected an
unchanged fresh Cargo artifact it had already retained; the corrected wrapper
accepts that repetition only with Cargo's fresh flag and exact hash agreement.
No production warning or boundary was suppressed.

This closes the unfinished completion-fence finding in the preceding milestone.
Live worker-pool sizing/coordinator/draining, full durable recovery, remaining
resource and authority cutovers, platform execution, release and fresh-machine
acceptance remain required. This is no full-plan completion or throughput claim.

## 2026-10-15 — Actual root worker-pool hosting prerequisite accepted

`GlobalVm` now has an explicitly selected private `await_pool_update` hosting
boundary. A root declaring that alias starts one unowned coordinator alongside
its workers. The host rejects a second coordinator or a coordinator context
already owning a worker/task. Startup counts the control wait as an additional
pending call, and readiness requires the actual coordinator/work waits. Roots
without that alias retain their existing static-pool path; the shipped Python
root has not yet been changed or activated with this new interface.

The host can deliver a typed `{workers, active_ids}` event to that existing
Python coordinator. Python owns creation/reuse through the verified gather
operation. Rust records actual worker-ID/coroutine-context membership at each
work wait and removes it through actual context retirement. Desired capacity is
independent of the observed effective count; draining workers remain effective
until they exit. Admission respects the smaller desired count, including debt
from still-busy workers and refusal to admit work on draining IDs. Idle draining
workers receive the trusted hosting stop event; busy futures are never answered,
replaced or cancelled as a side effect of resizing. Shutdown separately closes
the coordinator after stopping the workers.

`WorkerPoolCapacity::settled` requires a parked coordinator and the exact
contiguous desired worker-ID set, not count equality alone. Pending-call minimum
is `max(desired, effective) + 1` for this interface; the extra control future is
never hidden from occupancy. Context bounds grow before Python spawns workers
and shrink after actual drains. Invalid bounds/control envelopes are rejected
before mutation. Ordinary task-port resolution cannot answer the private pool
wait, and task account/completion fencing remains enforced.

Accepted source/artifacts:
`target/validation/worker-pool-core-20261015-r1` and
`target/validation/worker-pool-core-artifacts-20261015-r1`; receipt
`/private/tmp/brassclaw-worker-pool-core-accepted.json`. All17 global-host cases
pass (0.592 s) and strict test-target Clippy passes (3.114 s). The new actual-VM
cases exercise 50 grow/drain/reuse cycles with the same root and original idle
worker continuation, rejection of an insufficient pending-call budget, a busy
worker outside the reduced target retaining its exact future/budget, admission
debt, regrowth without duplicating that worker, and complete worker/coordinator
shutdown. Boot rejects an unbudgeted control wait and a duplicate coordinator.
Both current changed inputs and all retained executable hashes match the tested
source. There were no compiler or test failures in this batch.

**This is not live product sizing yet.** Before activating the production root:

- Extend the paired private IPC protocol/snapshot with exact desired/effective
  worker membership and coordinator state. Current `process.rs` validates
  `pending.minimum == root.workers()` and `outstanding.len == boot.workers`; these
  fixed-pool assumptions must be replaced coherently. Keep boot identity immutable
  and validate live membership separately. Every outstanding key still belongs
  to the same root generation.
- Add complete candidate preflight and actual VM progression to the revision-held
  settings commit/publication path. A count setter or early coordinator call is
  not an acknowledgement. Publish effective/draining status only from validated
  observations. Preserve in-flight tasks, host results and attempt references.
- Wire service control-wait handling, drain selection, actual admission limits,
  dynamic shutdown inventory and cancellation. Counts alone cannot prove settled
  membership; do not admit draining IDs or discard active capacity debt.
- Add the persisted/WebUI worker setting and short EN/DE help. Existing profiles
  with pending-call budget2 are valid for the current two-worker static root but
  cannot support two workers plus the new control wait. Migrate that dependency
  with a new settings revision and visible effective state; preserve larger
  operator settings. Do not hide the extra future or publish an impossible count.
- Upgrade/qualify the shipped protected Python root and test the paired native
  worker, authenticated settings, actual PostgreSQL persistence/rejection,
  ordinary matched/unmatched/MCP/provider waits, growth, draining, cancellation
  and shutdown. This isolated root gate is no replacement for those acceptance
  requirements or the rest of the full plan.

## 2026-10-15 — Paired pool protocol and retained admission capacity accepted

Private worker protocol17 now carries `WorkerPoolSnapshot`: actual desired/
effective membership, sorted worker IDs, parked-coordinator state and its exact
continuation. This is separate from immutable boot `RootExecutionIdentity`.
Receivers require the selected root's pool mode, unique/sorted worker waits,
unique outstanding keys, the same VM generation, a distinct owned coordinator
key and the observed pool's pending-call minimum. Boot counts the extra control
wait and requires settled actual membership. Static-root observations remain
explicitly pool-less. Older protocol workers are rejected before readiness.

Mechanical `RequestWorkerPool`, `DrainWorker` and `ClosePoolCoordinator` commands
reach the existing global VM. Accepted target changes also update the retained
Recipe/task registry's admission capacity, preserving existing task records and
reduction debt. Without that registry update, a grown VM would still reject
admission above its initial task count. These commands are private hosting
infrastructure, not a Recipe workflow or an exposed operator settings API.

Accepted source/artifacts:
`target/validation/worker-pool-wire-20261015-r1` and
`target/validation/worker-pool-wire-artifacts-20261015-r1`; receipt
`/private/tmp/brassclaw-worker-pool-wire-accepted.json`. All17 root-host,24 actual
process,27 service and15 actor cases pass (0.653 s/6.566 s/1.558 s/0.452 s), with
strict affected test-target lint (3.969 s). Six current source/fixture inputs and
six retained executable hashes match the tested source. The actual subprocess
performs20 grow/drain/reuse cycles and complete worker/coordinator shutdown with
unchanged boot identity, process ID and original idle continuation. A second
case admits three real unresolved task scopes beyond the boot capacity of two,
reduces capacity without discarding their futures or registry records, rejects
new admission debt, then regrows without duplicating those workers. No successful
intent/model/Tool/task-completion result is fabricated. This batch had no
compiler or test failures.

The remaining cutover must still include complete revision-held candidate
preflight/publication, service control handling and dynamic shutdown, persisted
worker settings and EN/DE help, dependency migration for existing pending-call
budget2 profiles, qualification of the shipped Python coordinator and ordinary
native PostgreSQL/WebUI/provider/MCP acceptance. Settings preflight must also
check that the future membership/continuation observations fit the configured
frame/value limits before committing a requested growth; an early small control
reply does not prove the eventual expanded snapshot will fit. No production root
or WebUI worker setting has been activated by this prerequisite. General durable
recovery, all other resource/authority/validator gates and full-plan/release/fresh
machine acceptance remain required.

## 2026-10-15 — Sparse draining membership counted before regrowth

Settings-preflight tracing found a gap in the preceding pool-count rule. After
shrinking, a busy worker can retain a high ID outside a subsequent target range.
The required worker set is therefore the union of target IDs0..desired-1 and
actual retained active IDs. Its pending minimum is
`desired + count(active_id >= desired) + 1` for the coordinator. The earlier
`max(desired, effective) + 1` assumption is superseded by this correction.

The host validates that complete prospective union before changing desired count,
context bounds or VM state. Its nonmutating `validate_worker_pool_target` also
checks coordinator quiescence, proposed typed-value bounds and current pending/
withheld occupancy. It reserves no memory and does not prove future frame size.
Actual pending observations and context-bound resizing use the same union,
including the interval before newly created workers reach their work waits.
Private protocol18 gives the paired receiver that corrected minimum contract;
protocol17 workers are rejected before readiness. Dump ABI remains unchanged.

Accepted source/artifacts:
`target/validation/worker-pool-union-20261015-r1` and
`target/validation/worker-pool-union-artifacts-20261015-r1`; receipt
`/private/tmp/brassclaw-worker-pool-union-accepted.json`. All17 root-host,25 actual
process,27 service and15 actor cases pass (0.617 s/6.555 s/1.545 s/0.456 s), with
strict affected test-target lint (3.995 s). Three current changed inputs and all
six retained executable hashes match. No compiler/test failures occurred.
The new real-process case holds worker7 busy, drains to actual IDs[0,7], then
requests target IDs0–6. An eight-call budget rejects the request with original
membership/future intact. After increasing the pending limit to nine, regrowth
keeps all eight actual workers, original root identity and unresolved task scope,
with nine observed calls including the coordinator. No host-effect or task-finish
result is fabricated.

This repair precedes complete worker-settings publication. Remaining work still
includes the future expanded-snapshot frame/value projection, the revision-held
candidate/DB/VM publication path, service draining/shutdown, persisted/WebUI
worker settings and help, shipped coordinator qualification and ordinary native
product acceptance. A projection is not an actual pool observation. No live
worker setting or full-plan completion is claimed by this gate.

## 2026-10-15 — Complete worker-settings candidate and bounded projection accepted

Private protocol19 extends `RuntimeSettingsCandidate` and
`RecipeCommand::UpdateRuntimeSettings` with optional `max_workers`; `None` retains
current capacity. `RuntimeSettingsObservation` reports the actual desired pool
limit separately from effective membership. The durable transport owner carries
the field into its existing preflight/commit/publication barrier. Revision
validation precedes expensive worker sizing. All construction/pattern sites and
legacy callers were updated; no persisted or WebUI worker field is exposed yet.

Preflight validates the target/retained-ID union, current occupancy, typed control
bounds and prospective wire size before mutation. The new private
`process_pool_projection.rs` retains the real DTO's remaining fields and streams
future worker IDs, wait tuples, outstanding/boundary keys and coordinator through
a bounded counting writer. It uses maximum-width key ordinals and allocates no
arrays or output buffer proportional to requested capacity. The repeated sizing
keys are projections only and never become observations, selections or grants.

Publication applies the complete revision/limits and lets Python advance the
existing coordinator. Rust pumps only the private hosting waits and control
yields, never a task port or workflow. It acknowledges only when the coordinator
is parked and every target ID is present. Retained workers outside the target
remain observed/draining. A coordinator that omits a target fails containment
instead of reporting successful uptake. The retained task registry's admission
target changes without resetting its existing records/budgets. The existing parent
transport deadline contains a broken coordinator; it is no global VM lifetime.

Accepted receipt: `/private/tmp/brassclaw-worker-pool-candidate-accepted.json`;
source/artifacts `target/validation/worker-pool-candidate-20261015-r2` and
`target/validation/worker-pool-candidate-artifacts-20261015-r2`. All17 root-host,
27 actual process,27 service,15 actor and9 Q1 checks pass (0.596 s/6.710 s/1.519 s/
0.467 s/55.923 s), with strict affected test-target lint (4.483 s). Ten current
source/fixture inputs and seven retained executable hashes match. A real
million-worker probe is rejected by streamed frame sizing with its original
root, settings, membership, outstanding keys and process identity intact. A valid
four-worker candidate publishes revision2 and actual settled membership; a stale
candidate remains rejected. An intentionally incomplete coordinator cannot emit
a settings ACK. No compiler/test failure occurred in these batches; the latter
fault test intentionally contains its worker.

Remaining work includes the public service settings field/observation, orderly
service draining after completed task/child cleanup, dynamic shutdown, persisted
worker setting and EN/DE help, pending-call dependency migration, shipped root
qualification and authenticated native product acceptance. Reductions report
actual effective/draining membership; they must not drop active tasks or infer
cleanup from a count. The ordinary shipped root remains static until this
coordinated cutover. No full-plan or release acceptance is claimed.


## 2026-10-15 — Service worker-pool publication and cleanup accepted

`ServiceHostingLimits.max_workers` now reaches the complete revision-held worker
candidate. Legacy/static product callers explicitly preserve their existing
policy with `None`; this milestone does not expose a persisted/WebUI setting or
activate the shipped coordinator. A nonzero pool target requires a pool-enabled
root. Startup requests must agree with the actual boot count.

The service retains a credited settings request while settling real root host
calls/control yields before consuming the parked pool coordinator. It publishes
Rust settings only after the worker reports the requested target, all required
actual worker IDs and a parked coordinator. Target and effective membership stay
distinct during reductions; service and transport expose actual pool observations.
It never restores a root continuation consumed by a pool change.

Admission respects the new desired count and busy-worker debt. Finished task
accounts/child heaps are released and their actual completion receipts delivered
before surplus idle workers drain. Busy host futures remain owned. Shutdown
closes the actual work-wait inventory and coordinator, including workers added
since startup, rather than iterating the boot-time count.

Accepted source/artifacts: `target/validation/worker-pool-service-20261015-r3`
and `worker-pool-service-artifacts-20261015-r3`; final receipt
`/private/tmp/brassclaw-worker-pool-service-accepted.json`. Its new actual-file
service regression rejects an infeasible eight-worker target before polling the
durable writer, acknowledges a synced real write before worker/Rust publication,
and preserves the pool/revision after a real create-new write rejection. It grows
two workers to four, retains four busy tasks (one holding a real child heap)
through reduction to one, blocks queued admission while over target, verifies
actual file replies and task receipts, drains only after cleanup, regrows and
shuts down the unchanged root/process gracefully. All 28 service tests and strict
host-target lint pass on r3. The preceding r2 full batch passed 17 root, 27
process, 28 service, 15 actor and 9 Q1 tests plus strict lint; r3 changes only the
service test's durable-write assertions. Initial r1 test compilation found two
moves through Arc; borrowed observations fixed them. No warnings were suppressed.

Remaining: persisted worker setting and EN/DE help, pending-call dependency
migration, shipped coordinator qualification and ordinary native product
acceptance. This is service acceptance, not full-plan or production-pool completion.


## 2026-10-15 — Persisted live global-worker control accepted locally

The former `workers: 2` startup literal is now the positive instance setting
`execution_limits.max_workers` (default two). Startup reads it, declares the
private coordinator port and starts the shipped Python pool coordinator before
ingress. The coordinator adds/replaces actual gather siblings; Rust retains
hosting/admission/resource enforcement. No per-chat VM, Rust workflow loop or
historical static-root fallback was added. Test fixtures now declare the real
shipped root's coordinator and close it explicitly after its actual workers.
Constrained draft entries that only reuse definitions keep their own entry ports.

Both live publication paths carry the requested target through the worker/Rust
revision barrier. Observation derives desired capacity from the actual root,
with effective membership and draining reported separately. Admission debt and
post-task release precede retirement. An unchanged target in a complete settings
save is normalized only at the serialized current revision, preserving duration
edits while a coroutine executes CPU work. A regression proves that a supported
one-second limit reaches that executing task, retains pre-terminal consumption
and reports explicit terminal ComputeExceeded accounting instead of invented zero.

V159 adds missing worker defaults with one settings revision, preserves explicit
larger values and raises pending-call capacity only to cover workers plus the
coordinator. Positive integer/range/dependency checks apply in Rust and PostgreSQL.
A pure SQL default wrapper preserves the previous default expression and evaluates
it exactly once per insert, never during migration. Actual PostgreSQL acceptance
covers small legacy limits, overrides, a counted volatile default, invalid writes
and revision-overflow rollback. V158's predecessor regression explicitly removes
the successor constraint when constructing its historical schema.

WebUI exposes the field, validates its pending-call dependency and shows actual
versus desired slots/draining. EN/DE help explains the default, one-VM semantics,
absence of per-slot memory reservation, safe retirement and pending-call minimum.
JavaScript syntax checks passed for the tab and both language files; this does not
claim rendered-browser, external-model or remote-machine acceptance.

Final receipt: `/private/tmp/brassclaw-worker-pool-product-accepted.json`;
source/artifacts `target/validation/worker-pool-product-20261015-r8` and
`worker-pool-product-artifacts-20261015-r8`. R8 passed 17 root, 15 actor, 27
process and 28 service tests plus strict root (including Bedrock compilation)
and standalone host test-target lint. Six retained-Recipe and nine Q1 tests plus
four actual native PostgreSQL/settings/global-runtime cases passed on r7 and
are retained with that exact source/paired-worker archive. R8 differs only by
replacing a negated optional-map predicate with its equivalent `is_none_or`
form; the runner verifies that exact sole diff, documents all three truth cases,
and reruns the worker identity/lifecycle callers. No warning was suppressed.
The native settings case uses the supported HTTP settings route; global acceptance
starts four slots, shrinks/drains to one, regrows four, retains the same root/PID,
then exercises ordinary Match/No-Match, MCP ingress and restart. Its model gateways
are test fixtures; no live external-provider inference claim is made.

Earlier failures remain in their archives: old pending minimum/reduction values,
missing coordinator declarations in fixtures, an unsupported millisecond test
setting and a mistaken numeric assumption for terminal budget accounting were
corrected. The root-workspace lint also found the optional-map simplification,
which was fixed before final acceptance. Current source and retained binaries'
hashes were verified. Reusing unaffected evidence avoids another full native
compile solely for the equivalent predicate.

This closes the local worker-setting wiring gate, not the full plan. Correlated
MCP child admission when parent slots are saturated still needs its separate
acceptance/implementation; a configurable worker count alone does not solve that
case. Durable fatal-runtime reconciliation, the complete validator/authority/store
cutovers, remaining limits, platform execution and release/fresh-install/model/
prefix/function acceptance remain required. No commit/tag/release or remote build
was performed for this milestone.


## 2026-10-15 — Fenced dependent-slot VM primitive accepted; MCP cutover still open

Saturation is a real remaining Phase 3a issue: `McpChatBridge::call` durably stores
`parent_run_id` and allocates the child `call_id` before ordinary correlated chat
submission. The global driver later calls `submit_when_available`; the service
retains a waiting parent in its worker slot. All parent slots can therefore await
children that cannot enter the same pool. Increasing the setting alone cannot
establish progress. MCP must continue to send an ordinary message through matching
and the pinned Recipe runner; it must not call Monty/IBS/Tools directly.

`global_dependents.rs` now supplies an in-process hosting primitive, not a new
workflow Tool. A reservation binds one actual parked parent continuation/token
to one typed child-run UUID. The parent must still own a running, uncancelled,
nonterminal task scope. Reservations are exclusive per parent and child. One
related coroutine can be added/reused without changing the ordinary target count
or replacing the VM; its actual work wait is observed separately from reserving
it. Normal/retained/reserved slots plus the coordinator remain bounded by the
live pending-call policy and typed control-input limits. Counts use unions without
allocating an array proportional to the desired target.

Both slot selection and final task-account insertion fence the exact child run.
A different idle slot cannot consume the reserved run. Existing-run, nil-run,
duplicate-parent/child and stale/cancelled-parent requests fail before entering a
child. Normal admission still respects ordinary capacity/debt; a related binding
requires the actual reservation and live parent, not an increased global limit.
Rejected conversion rolls back only an untouched, unbound reservation in a Ready
VM. An entered/unknown attempt retains its identity and evidence. The idle slot
cannot drain until explicit owner release; an entered child additionally needs
its exact token and actual finish acknowledgement/work-wait return. A ready
parent answer is retained without consuming its continuation until dependent
release; fatal Abort stays available for instance containment. The real-file
regression resumes that exact held parent answer after release and observes its
next requested effect without executing/replaying it. Separate child-context
cleanup remains the process/service owner's obligation.

The private coordinator update now includes `borrowed_ids`. Ordinary production
updates supply an empty list, which the current shipped coordinator ignores.
Only the constrained dependency fixture consumes it so far. No process command,
borrowed-capacity snapshot schema, registry/service admission or MCP production
entry has been activated, and protocol remains19. Reservation is not proof of
coroutine creation: a future transport handler must drive real hosting boundaries
and verify the requested dependent work wait before acknowledging capacity.

Accepted receipt `/private/tmp/brassclaw-dependent-slot-core-accepted.json`;
source/artifacts `target/validation/dependent-slot-core-20261015-r5` and
`dependent-slot-core-artifacts-20261015-r5`. All21 actual global-VM,27 subprocess
and28 service tests pass, plus strict host-target and root host-package lint.
New cases run a real parent/child/grandchild with unresolved host futures in one
VM, reject the next reservation at actual pending capacity, preserve exact keys
on wrong/oversized inputs and cancellation, prevent ordinary-slot run theft,
and read/write/sync a real child file result before acknowledging finish and
releasing/draining its exact slot. No successful matcher, model, Recipe or Tool
result is invented. This proves hosting mechanics, not full Recipe/kernel/MCP or
durable recovery acceptance. Initial test compilation used a nonexistent cancel
method and was corrected to the existing API. The first child-entry regression
then exposed the second ordinary-capacity guard; it was fixed rather than bypassed
or weakening ordinary admission. R5 additionally fences early parent resume
and passes the same21/27/28 tests and strict lints. All diagnostics remain retained.

### Required next integration, before claiming saturation resolved

1. Extend the paired worker protocol and actual pool observation to distinguish
   requested dependent IDs, active dependent slots and draining surplus workers.
   Preserve complete frame/pending/context preflight and retained task accounting.
   Add explicit reserve/release commands; verify actual coroutine creation, fail
   closed on a missing target, and release only after registry child cleanup.
2. Thread trusted parent-wait and child-run identities through ordinary admission.
   MCP already records lineage before submitting, but a run UUID alone cannot
   identify an attempt. Bind the originating provider surface/exchange to the
   live parent TaskPorts/attempt and exact continuation; reject stale generations,
   conflicting lineage and cycles. Reuse existing turn lineage contracts where
   applicable. Keep correspondence metadata separate from Tool permission.
3. The service must reserve a related slot while retaining the parent's real host
   future, prioritize that child without letting unrelated queued work steal its
   slot, preserve normal target/debt and reuse/drain only after real completion.
   Multiple children of one waiter need bounded, cancellation-aware sequencing;
   cancellation/rejected preparation must not orphan reservations. No polling,
   Rust Recipe executor, per-chat VM or automatic Tier-2 replay is permitted.
4. Upgrade/qualify the shipped Python coordinator to create these same-root
   coroutines. Expose dependent versus draining observations and explain their
   relationship to the ordinary worker setting in WebUI. Running tasks retain
   their original exports, budgets and attempt identities.
5. Accept through actual saturated ordinary MCP/child chat ingress, including one
   parent slot, every parent slot occupied, nested dependencies, sibling calls,
   low pending/retention limits, live policy/settings edits, cancellation/disconnect,
   duplicate requests, failure containment and effect preservation. Re-run the
   ordinary match/no-match/restart path with paired artifacts. Until then this is
   a prerequisite and the parent/child saturation gate remains open.

Additional source finding for the same transport owner: `Exchange.calls` retains
completed cancellation tokens and rejects a new request at the fixed4096 map
size. Its in-flight lifecycle and capacity must be repaired under the full limit
inventory; durable request/outcome deduplication must remain authoritative. Do not
replace the durable cache with a fresh execution or hide this cap behind a label.

## 2026-10-15: dependent-slot worker transport accepted, production producer open

The first next-integration prerequisite above is implemented in paired protocol20.
The shipped coordinator now consumes `borrowed_ids`; snapshots distinguish
reserved IDs, actual dependent coroutines and ordinary surplus draining workers.
Validate is nonmutating, bounded prospective-frame projection precedes reserve,
and reserve acknowledges actual coroutine creation. A deliberately noncompliant
coordinator fails closed. Release rejects a child before its real finish ACK or
while its Recipe registry context remains retained. An early parent answer keeps
its exact unconsumed continuation, without another host effect.

Receipt `/private/tmp/brassclaw-dependent-slot-wire-accepted.json` identifies
`target/validation/dependent-slot-wire-20261015-r3` and its preserved executable
artifacts. R2 passed21 global,15 actor,29 process,28 service,6 retained-execution
and9 Q1 checks; native PostgreSQL worker/pending migrations, live WebUI settings
and ordinary match/no-match/MCP/restart acceptance; and strict affected main and
standalone lints. R3 changes only the subprocess regression: all29 process tests
and process lint pass again; its receipt explicitly reuses unaffected r2 results.
It grows ordinary capacity1→3 and shrinks3→1 around a child retaining1 MiB,
rejects an unsafe pending-capacity reduction without changing the revision,
drains only the unrelated surplus and verifies the child's actual retained
value. Final child cleanup frees its heap before releasing the parent result.
The real production worker and test artifact hashes are retained with the receipt.

Remaining next-integration items2,3 and5 are still required. In particular,
ordinary service admission has no producer for the reserve command, and the
provider exchange is not yet tied to its exact live parent attempt/surface.
The normal-path native checks use their existing gateway fixtures, not the live
inference endpoint. This milestone does not close the saturated MCP, complete
fatal-recovery, Linux/Windows or full-plan gates. Protocol19 notes above describe
the preceding accepted stage and are not a compatible worker alternative.


## 2026-10-15: MCP in-flight ownership and live capacity accepted

`mcp_chat_bridge.rs` now retains cancellation state only while HTTP waiters own
it. Exact duplicates share one registration; the last waiter cancels its token
and removes the registration on completion, error or future drop. Explicit
waiter counts are updated under the index mutex. Empty batches release the
index backing allocation. Durable PostgreSQL correlation remains authoritative
for request identity and original outcomes; late cancellation of a completed
request does not poison recovery or submit another chat.

The former fixed 4096-entry bound is now `max_mcp_inflight_calls`, a positive
u32 execution setting with default 4096 and EN/DE WebUI help. The bridge captures
the existing acknowledged effective settings rather than a desired database
write. Live reductions preserve accepted registrations and duplicate waiters,
while rejecting new identities until occupancy permits them. V160 adds missing
values with one revision, preserves explicit values and evaluates an existing
custom default expression once. Invalid or exhausted revisions fail atomically.

Accepted receipt `/private/tmp/brassclaw-mcp-call-lifecycle-accepted.json` points
to `target/validation/mcp-call-lifecycle-20261015-r3` and retained paired worker/
composition artifacts. Native ordinary global chat/MCP/restart acceptance passes
with 4097 simultaneous cancellation registrations, reduction to one without
canceling them, 128 synchronized concurrent-drop rounds, retained state
references, aborted waiters, more than 4096 completed IDs, late cancellation and
original durable cached replies. This exercises registrations, not thousands of
simultaneous Recipe effects. Strict composition test-target lint also passes.
The receipt explicitly reuses unchanged r2 product complete-update tests, native
V160 migration, live WebUI/runtime uptake and strict host API/Monty/product/
composition lint. Current selected source and executable hashes were verified.
JavaScript syntax checks pass; rendered resource-form acceptance remains open.

Earlier r1/r2 source used Arc reference counts for cleanup. Review exposed a race
between concurrent Drop bodies and their later field destruction. Those outputs
remain historical evidence, not certification of concurrent cleanup. R3 uses
serialized waiter counts and adds the corresponding regression coverage.

This closes the cancellation-index leak and its fixed active capacity. It does
not close ordinary service dependent admission, exact provider-surface/attempt
correlation, pending-admission cancellation, full fatal recovery or platform/
release gates. The provider response ceiling, server request default and
request-ID size remain in the limits audit. No direct MCP-to-Monty execution
path, Rust task loop, fake approval or effect replay was introduced.


## 2026-10-15: pre-enqueue cancellation and abandoned admission waits accepted

Task preparation no longer waits indefinitely for queue capacity or memory
admission after its host is cancelled. The instance service separates waiting
from enqueue and can return sealed proof that this particular input was never
queued. The proof identifies the original root, run and actual port owner; it
is not a VM receipt, checkpoint, permission or synthetic execution outcome.
Accepted tasks still return their real ticket and follow VM/host-future settlement.

The driver retains its shared submission future before polling it. An abandoned
drive waiter therefore leaves the actual submission with its attempt owner.
After fencing, the supervisor polls that same future, retains any accepted
control, or obtains the real pre-enqueue cancellation proof. It never recreates
input, ports, catalogue selection or an admission. The future uses a weak attempt
reference and records ownership before exposing its result. Cancellation and
ordinary settlement retain distinct acknowledgement paths.

The factory verifies and retains proof before database I/O. Only an unstarted
reservation with no Recipe selection, Tool record or observed dispatch can
close through `cancel_unsubmitted`. The existing monotonic schema records its
failed/cancelled admission without inventing an execution report or No-Match
review event. A failed or uncertain write keeps the exact proof and private
address for acknowledgement; only a durable success releases retained state.
Already started admissions are rejected. The existing acknowledged, configurable
cancellation timeout bounds the wait; there is no new product timer or cap.

Receipt `/private/tmp/brassclaw-cancel-preparation-accepted.json` identifies
`target/validation/cancel-preparation-20261015-r3` and preserved paired protocol20
artifacts. All 28 service tests and six focused real PostgreSQL/model-host cases
pass, including full queue/memory waits, a rejected deferred COMMIT followed by
an exact acknowledgement retry, a dropped drive waiter while admission remains
paused, refusal to reclassify started work, active cancellation, late answers,
completed replies and retained interrupted catalogue preparation. The ordinary
runtime test uses real correlated chat ingress and cancellation while holding the
actual settings publication mutex and admission pause. It verifies zero dispatch,
no fabricated service receipt, durable cancellation, refunded queue/retention
credits and a later matched reply on the same global instance. Accepted live
cancellation-policy and global match/no-match/MCP/restart tests also pass.
Strict affected main and standalone test-target lints pass; selected current
source, retained executables and the paired worker hashes were verified.

R1 found a missing test namespace and r2 found that the shared service fixture
used a dependency available only in the standalone workspace. The native test
now uses its qualified token type and the shared fixture uses existing Tokio
notifications. No dependency or warning suppression was added. R3 also covers
ownership after a dropped admission waiter; earlier happy-path evidence alone
does not establish that case.

This closes cancellation during completed port preparation and admission waits.
An abandoned catalogue/database preparation before that boundary still retains
its original uncertain evidence. Non-cancellation admission errors must not be
relabeled as cancellation. Ordinary dependent-slot admission, immutable provider
surface/parent-attempt correlation, full fatal recovery and platform/release
acceptance remain required. No Rust workflow loop, per-chat VM, direct MCP
execution connection or effect replay was introduced.

## 2026-10-15 — MCP provider-surface provenance accepted; dependent admission validation follows

Production provider exchanges now persist the exact selected live capability
surface and use that port's pinned advertisement. The bridge rejects a wrong or
finished surface, including a port fenced while exchange persistence waits.
Migration V161 preserves historical NULL provenance and makes all exchange
identity columns immutable; it cannot assign a historical call to a successor.
The ingress native test graph explicitly declares its shared fixture's
`tokio-postgres` dependency and enables composition's test-only transport seam.
Production connections require live provider provenance.

Receipt `/private/tmp/brassclaw-mcp-parent-origin-accepted.json` identifies frozen
`target/validation/mcp-parent-origin-20261015-r4` and preserved paired artifacts.
Exact live-session/HTTP/PG provenance passed (20.74 seconds), ordinary global
match/no-match/MCP/restart passed (36.46 seconds), the real ingress listener passed
(18.32 seconds), the ingress architecture boundary passed (0.43 seconds), and
strict composition/ingress test lint passed (94.36 seconds). The first two
results reuse unchanged r2 source/artifacts; r4 adds only the ingress dev
dependency and Cargo-generated lockfile repair. The frozen receipt does not
cover subsequent dependent-service edits or prove parent/child saturation.
That production admission wiring and its new native/contained-worker regressions
are the next validation batch; the remaining full-plan gates stay open.

## 2026-10-15 — Ordinary dependent admission reaches the single global VM

The ordinary task factory now correlates a persisted MCP child call with its
immutable exchange surface and the exact retained parent host/provider/port
owner. It checks the actual parent claim and started private admission without
starting an operation. Historical NULL provenance never guesses a successor.
These identities supply scheduling correspondence, not Tool authority or a
Recipe executor.

The service resolves that parent port against its actual live task and parked
continuation. Related work has queue priority and uses existing protocol20
Validate/Reserve/Release commands when ordinary slots are occupied. Every child
still enters ordinary chat, matching, IBS and Recipe execution. Parent answers
remain retained until their children settle. Descendant cancellation fences
actual controls; overload and stale/finished parent relationships are errors,
never fabricated cancellation or No-Match. Queue preparation cannot wait on a
sibling's last queue credit; a new admission pause rejects unstarted dependants
while accepted work keeps progressing.

Completion closes the real child registry/heap before releasing the dependent
slot and acknowledging its receipt. The service first defers the completed
work-wait call so the Python pool coordinator is quiescent. Ordinary reductions
still count busy surplus debt. A parent host panic triggers containment
immediately even while its normal answer would be held for a child; late child
I/O and task evidence remain retained.

Receipt `/private/tmp/brassclaw-dependent-service-accepted.json` identifies frozen
`target/validation/dependent-service-20261015-r10` and paired preserved artifacts.
All34 actual worker service cases pass (3.07 seconds), including nested loans,
held parent answers, descendant cancellation, low capacity, admission pauses,
a full sibling queue and fatal parent panic with a retained real child read.
The native PostgreSQL regression passes real HTTP MCP sibling calls while their
parent holds the only ordinary slot (20.84 seconds); both children settle through
normal Recipes with the same VM/PID and cleaned loans. It also checks cached
request delivery and live Tool-policy denial. The ordinary global match/no-match/
MCP/restart regression passes (35.90 seconds). Strict host/composition/ingress
lint with tests passes (55.21 seconds). r10 reuses unchanged r9 service/global
results and rebuilds the HTTP test; selected current source and executable
hashes, including the worker pair, were verified. Gateway fixtures supply model
responses; this is not live inference or remote deployment acceptance.

Remaining integration requirements stay open. In particular, definitive
pre-admission capacity/value/shutdown rejections need sealed no-VM proof and a
durable non-cancellation settlement contract, including exact retry after unknown
COMMIT and release of retained attempt capacity only after acknowledgement.
Current error retention is safe against replay but must not accumulate forever
under repeated overload. Extend V099/V107's currently cancellation-only unstarted
settlement rules through a coordinated migration and proof/driver/factory change;
never relabel rejection as task_cancelled or fabricate an execution report.
Uncertain Admit/worker outcomes must remain reconciliation evidence. Further
native multi-parent/nested/settings/cancellation/recovery cases and remaining
transport limit settings are required before closing the full saturation and
plan acceptance gates. No release or full-plan completion is claimed here.

## 2026-10-15 — Durable pre-VM rejection acknowledgement accepted

The service now issues sealed `UnsubmittedTask` proof with a distinct cancelled
or rejected cause. Preparation errors return explicit rejected submissions;
queued rejections publish proof before their error waiter wakes. Uncertain Admit
or worker failures issue no such proof. The proof binds the original root, run
and actual port owner and grants no authority. A rejected value retains its
proof even if cleanup of an unused dependent slot subsequently fails.

The driver retains the actual proof before durable I/O, shares acknowledgement
across abandoned drive/stop waiters, and applies the acknowledged lifecycle wait
deadline. It returns the original rejection cause, never cancellation or No-Match.
The factory verifies the original host/ports and zero execution evidence and
retains its private admission through unknown writes. V162 permits the exact
classified unstarted rejection outcomes while keeping identity and terminal
settlement immutable; no fake VM receipt or execution report is added. Capacity
is refunded only after durable acknowledgement of the same proof. The existing
WebUI acknowledgement control now explains cancellation and rejection in EN/DE;
its API key remains compatible.

Receipt `/private/tmp/brassclaw-unsubmitted-rejection-accepted.json` initially
identifies `target/validation/unsubmitted-rejection-20261015-r1`. All34 service
cases passed (3.15 seconds). New actual PostgreSQL/worker cases passed: a COMMIT
response withheld by a real TCP proxy after server commit (3.54 seconds), repeated
driver rejections with retained capacity one (2.04 seconds), and queued history
rejection after a live value limit reduction (2.13 seconds). The committed but
unacknowledged outcome retained its slot, then retrying the original proof
acknowledged it without re-execution. Existing cancellation, opaque driver,
failed preparation, uncertain invocation and confirmed-reply regressions passed.
Native ordinary HTTP MCP/global/restart and cancellation passed (20.11/35.60/
15.84 seconds); strict host/composition/ingress and standalone service/model test
lint passed (67.14/18.69 seconds). Both translation modules passed syntax checks.
No suppressed diagnostics or external inference is part of this evidence.

A remaining integration issue was identified from actual MCP consumers: terminal
and cached receipts require a matched Recipe, but these rejected chats correctly
have no Recipe or assistant reply. Extend terminal verification to accept only
an exact settled pre-VM failure with no selection/invocation and the original
correlated user reference, preserving all existing matched execution proof.
Verify the error over real HTTP with live queue limits, cached duplicate delivery,
chat closure and restored admission, and retain unresolved/uncertain originals
without replay at startup. These subsequent MCP edits are not covered by r1 and
are the next frozen validation batch; the wider plan gates remain open.

## 2026-10-15 — MCP rejected-chat receipts and exact request ownership accepted

Terminal and cached MCP receipts now recognize a definitive pre-VM failure only
from the exact settled admission, original accepted user reference, supported
two-field failure outcome and absence of Recipe selections/Tool invocations.
They require a terminal unsuccessful run with no assistant text/reply reference.
Other results retain the complete existing matched-Recipe qualification checks.
The transport returns its existing error result; no Recipe, reply, execution
report, cancellation or No-Match outcome is fabricated.

The real HTTP regression lowers the live queue-byte ceiling, receives a durable
rejection, reads the same cached error on duplicate delivery, restores admission
and completes another ordinary child in the same global VM. A deferred PostgreSQL
trigger then rejects final MCP chat closure after the error response was durably
saved. An actual runtime restart with the same persisted operator identity
reconciles that response and closes the original chat without submitting a new
command or calling the model again. Startup now verifies cached receipts and
propagates turn-storage/decode and close failures instead of marking itself
prepared after those errors. Missing/unfinished original runs remain unresolved.
The regression corrupts the actual stored turn snapshot, requires a storage
error, restores its exact original payload and retries the same startup owner.

In-memory request ownership now uses exact normalized decimal identities matching
the durable JSONB reservation. Numeric spellings such as `1` and `1.0` share one
waiter/cancellation entry. Normalization never converts integer IDs through f64;
large distinct integers, string IDs and distinct finite numbers stay distinct.
Actual PostgreSQL comparisons cover signed zero, exponents, integer/float bounds
and very small/large finite numbers. Capacity-one duplicate leases share their
owner, cancellation through the equivalent ID reaches it, and final release
reclaims the entry.

Receipt `/private/tmp/brassclaw-mcp-request-identity-accepted.json` identifies
frozen `target/validation/mcp-request-identity-20261015-r2`. Native HTTP/provider-
surface/restart acceptance passed (32.72 seconds), ordinary global Match/No-Match/
MCP lifecycle acceptance passed (36.27 seconds), and strict host/composition/
ingress test lint with Bedrock passed (60.26 seconds). Unchanged service/model/
proxy and standalone-lint evidence is retained from the earlier rejection
batches, with its original source/artifacts and limits. Ten selected current
source hashes, all six retained executable hashes and the paired worker were
verified. Concurrent prefix changes were compiled intact; these MCP results do
not certify their separate authoring/qualification acceptance.

The preserved r5 failure exposed one additional cached-recovery mapping to
Unresolved; the accepted successor repairs it. The earlier source-inventory
failure occurred before Cargo and involved only two verified documentation
changes; it is not a behavioral pass. No warnings were suppressed.

Remaining startup classification work includes the transcript-reference helper:
it currently wraps missing, ambiguous and storage failures together. Separate
those outcomes at their owning interface and prove real transcript-store failure
propagation before claiming every recovery storage error is classified. The
fixed request/response byte ceilings and request-ID storage/index capacity,
native multi-parent/recovery cases, platform/release/fresh-install and all other
open plan gates remain required. No full-plan or release completion is claimed.

## 2026-10-15 — Typed transcript recovery errors accepted; MCP byte controls candidate

The exact transcript-reference helper now retains typed Missing, Inconsistent
and Storage failures. Missing thread/message evidence remains unresolved;
backend/decode failures abort recovery, while conflicting commands or ambiguous
references produce a distinct correlation error. Terminal recording and cached
receipt verification use that classification, so a failed transcript read cannot
be counted as missing and seal startup as prepared. No command is resubmitted.

Frozen `target/validation/mcp-transcript-recovery-20261015-r1`, receipt
`/private/tmp/brassclaw-mcp-transcript-recovery-accepted.json`, passed the real
PostgreSQL/HTTP parent/restart regression (35.21 seconds), ordinary global/MCP
lifecycle (37.37 seconds) and strict affected test lint (55.97 seconds). The new
regression mutates the original stored transcript to remove its message, corrupt
its collection and duplicate its accepted user reference, restores exact original
data, verifies distinct outcomes and retries the same startup owner successfully.
It also rejects a conflicting command. The existing no-replay/call-count checks
remain. This closes the specific transcript classification follow-up above;
subsequent byte-control edits are outside that accepted snapshot.

The next candidate adds `max_mcp_request_bytes` and `max_mcp_response_bytes` to
acknowledged execution limits and the WebUI with EN/DE help. V163 preserves full
and partial overrides, adds missing defaults once per row/revision and evaluates
the prior INSERT default once. Both values are positive u32 bytes, additionally
allocation-representable on the executing host. Production ingress authenticates
before decoding and captures the acknowledged request limit for that body read;
Kohai captures the response limit before each HTTP operation. Overflow is an
error without truncation or automatic replay. Existing reads keep captured bounds.

Required candidate acceptance includes the real facade/PG migration and invalid
updates, a body beyond the former 1 MiB ceiling, a held body read across a live
reduction, the actual gateway consuming a complete answer beyond 1 MiB, and a
response overflow after a completed effect followed by the original cached
receipt without another invocation. Run affected protocol/ingress checks and
strict lint after the coherent edit. Frontend syntax currently passes; neither
runtime uptake nor rendered form acceptance is claimed yet. The separate
256-byte request-ID/index issue, total transport buffering/admission, platform/
release and all other open plan gates remain required.

The byte-control r1/r2 acceptance failures are preserved. R1 exposed a separate
64 KiB command guard in the catalogue binder, advertised schema and provider
adapter; the candidate now removes that extra invocation quota while keeping
exact typed usage validation and the captured HTTP bound. Declaration/metadata
and catalogue-count limits are separate remaining preparation work.

R2 then reached an ordinary matched Recipe but returned a failed chat. A read-only
inspection of that retained test executable's own PostgreSQL cluster, discovered
through its actual pg_ctl child and live postmaster port, established the exact
sequence: the reply Tool answered successfully, then another Tool answered
`retained_tool_resource`, and the admission settled with `recipe_execution_failed`
and `intent_outcome=match`. Receipt log:
`/private/tmp/brassclaw-mcp-large-response-inspect.log`. The shared 1 MiB fallback
at `BuiltinFirstPartyTools::bounded_input_size` rejected the subsequent history
memory write. The response reader is not the cause of that failed Recipe.

The candidate removes that additional fixed fallback for memory_write. Monty
callers retain their captured typed-value/IPC bounds; direct calls retain native
representation limits, and memory backend, kernel, mounts, current Tool policy
and freshness remain effective. Memory output and other primitive caps are not
changed or claimed resolved. Re-run the unchanged complete-large-answer assertion
and the existing memory authority/input regressions; never replay the completed
reply or route this failure through Tier 2. The candidate remains unaccepted until
its complete caller-level tests and lint pass.

## 2026-10-15 — Live MCP byte controls and complete history handoff accepted locally

V163 and the acknowledged settings projection now supply independent positive
`max_mcp_request_bytes` and `max_mcp_response_bytes` controls, defaulting to 1 MiB.
The actual WebUI exposes both with EN/DE explanations. Production ingress checks
authentication before JSON decoding and captures the request bound once per body
read. The real Kohai HTTP consumer captures its response bound before sending,
uses fallible buffer growth and rejects overflow without truncation or replay.
A live reduction preserves a read already accepted under its previous bound.

The extra 64 KiB invocation checks and advertised command maxLength were removed
from the command binder/provider adapter. Exact typed usage contracts still
apply. The fixed shared input fallback for memory_write was also removed: it
previously rejected the history step after the reply had already been posted.
The task's typed-value/IPC and backend/kernel/mount/freshness checks remain.
Other memory output and primitive caps are separate open work.

Receipt `/private/tmp/brassclaw-mcp-transport-bytes-accepted.json` identifies frozen
`target/validation/mcp-transport-bytes-20261015-r3` and retained r8 artifacts.
The parent/single-VM HTTP regression passed (38.03 seconds), including a complete
answer beyond 1 MiB with its history, held-body reduction, authentication before
decode and a failed response read after a completed effect followed by the same
cached run with unchanged invocation count. That result is reused from r3's
passing case; subsequent browser-harness failures did not change Rust or assets.

The ordinary global/MCP regression with the exact repository browser script
passed (52.29 seconds). Real Chromium/React uses the native facade/PostgreSQL;
it saves both controls, verifies persisted/effective revisions, reads EN/DE help
and restores the original values. `tests/browser/monty-vm-resource-limits.mjs`
has no machine-specific report path and supports an explicitly installed native
browser executable. The tested copy and repository source have the same SHA-256.
This is native facade/browser evidence; packaged ingress authentication, browser
CI automation, Windows/Linux execution and remote deployment are not inferred.

Additional passing cases: native authenticated WebUI (95.34 seconds), migration
preserving full/partial overrides and one evaluation of a volatile default
(1.84 seconds), five MCP protocol tests, the complete-settings update contract,
actual ingress listener (17.37 seconds), eleven existing memory/input/mount tests,
three MCP declaration/catalogue contract tests and strict host_api/product_workflow/
host_runtime/monty_host/composition/ingress test lint with Bedrock (108.80 seconds).
All 44 execution controls have one nonempty label/help entry in both EN and DE;
changed modules pass syntax checks. Fourteen selected current runtime/asset inputs,
nine retained executable hashes, the paired worker and the exact browser source
were verified. Cargo remained serial in the local target. Mandatory disk cleanup
at 91% preserved executables first and did not touch the other chat's target2.

Browser r3-r6 failures were test interaction failures: off-screen controls were
not opened/edited. The final harness explicitly scrolls and uses fresh observed
references, verifies actual field values and requires a real PUT. There was no
API-response mock, replacement Recipe, weakened large-answer assertion, Rust-loop
fallback, extra approval or replay of the reply which the earlier failed test
had already published. Earlier failed batches remain preserved.

Remaining required work includes request-ID256/raw JSONB index admission,
aggregate transport buffering, structural declaration/catalogue capacities,
other primitive input/output and memory-search result limits, complete immutable
component/validator/policy/recovery cutovers and all remaining platform/release/
fresh-install/model gates. These local results do not finish the full v3 plan.


## 2026-10-15 — Explicit Memory-search counts accepted locally

Frozen `target/validation/memory-search-counts-20261015-r1`, receipt
`/private/tmp/brassclaw-memory-search-counts-accepted.json`, retained executables
in `target/validation/memory-search-counts-artifacts-20261015-r1` establish:

- `builtin.memory_search` preserves explicit positive integer counts. Omission
  still defaults to five; zero, negatives, fractional/string/null values and
  values outside the backend count representation fail instead of defaulting
  or silently clamping. The generic optional-null normalizer preserves this
  field for validation. The native schema and bundled Tool/ToolSkill/Skill
  descriptions agree with the count contract.
- `MemorySearchRequest` no longer clamps final/candidate counts to 1,000/5,000.
  Both repository and backend reject invalid counts before index/provider work.
  Candidate counts remain at least the explicit final count in either builder
  order. The u32 representation of backend counts/ranks remains a technical
  boundary; requests do not preallocate their declared maximum.
- FTS uses the existing filesystem paging owner until the requested candidate
  count or actual exhaustion. Per-page limits are batching, not a total cap.
  Actual returned rows are reserved fallibly, repeated chunk paths are not
  scored twice, and offset overflow errors. Enumeration keeps its existing
  live-read semantics; this is not a point-in-time database snapshot.

Evidence: all 113 memory tests pass, including 5,205 actual indexed documents
and 1,050/5,205 requested results across pages (filesystem suite145.078s);
12 registered memory/input/mount cases0.509s; native retained Tool/PostgreSQL
count/default/live-policy/source checks21.503s; ordinary global Monty
Match/No-Match/MCP/restart38.346s; strict memory/host-runtime/composition test
lint with Bedrock73.810s. Toolchain1.96.0, locked dependencies, serial screen,
NVMe target, debug0/incremental0. The longer filesystem test includes document
creation and indexing; it is not a query-performance measurement.

The successor adds 1,050 writes/search results through the actual registered
PostgreSQL Tool and passes in the MCP request-digest batch recorded below. The remaining
Memory output/input guards, other primitive limits, authored component cutover,
platform/browser CI, packaged, remote and full-plan release gates remain open.
This changes infrastructure and existing primitive contracts, not Recipe
sequencing or the global VM lifecycle; no Rust workflow fallback is introduced.


## 2026-10-15 — MCP long request identity and native paged Memory accepted

Frozen `target/validation/mcp-request-digest-20261015-r1`, retained artifacts
`target/validation/mcp-request-digest-artifacts-20261015-r2` and receipt
`/private/tmp/brassclaw-mcp-request-digest-accepted.json` establish:

- The 256-byte MCP request-ID guard is removed. Existing acknowledged HTTP
  request/response bounds still govern the transport. IDs remain strings or
  numbers; this does not change routing, component qualification or authority.
- V164 retains the original JSONB ID and adds a generated SHA-256 uniqueness
  key. Numeric normalization preserves PostgreSQL scalar equality, including
  decimal/exponent spellings and signed zero, without converting via f64.
  `trim_scale` preserves numeric value while removing insignificant fractional
  zeroes ([PostgreSQL mathematics](https://www.postgresql.org/docs/15/functions-math.html)).
  Strings stay distinct from numbers. Lookups require the digest **and exact
  original JSONB equality**; a hypothetical collision fails before admission
  rather than returning another request's receipt. The predecessor unique
  constraint is located by its ordered columns, avoiding guessed truncated names.
- Actual native HTTP/global-Monty acceptance sends an incompressible ID over
  14 KiB, preserves its response ID, reads the original run on repetition and
  rejects reuse with a different command. Equivalent numeric IDs retain that
  same durable ownership, cancellation index and effect count.
- The same real database reconstructs the predecessor index over populated
  completed correlation rows and executes the shipped migration transactionally.
  All original IDs, timestamps, phases, commands, references and responses are
  identical after backfill. Normal correlation immutability guards remain.
- The registered retained Memory Tool performs 1,050 writes and returns the
  requested 1,050 lexical results through PostgreSQL, crossing the actual
  filesystem page size. Missing limits still default to five, an explicit count of 25 is
  honored, invalid optional values fail, current Tool policy/source changes
  remain enforced and disabled embeddings make no provider calls.

Evidence: native held-parent/ordinary HTTP39.253s; registered PostgreSQL Memory
104.222s (including all writes/startup/other source-policy checks); ordinary
Match/No-Match/MCP/restart38.259s; strict PG/composition test lint with Bedrock
66.138s. Locked Rust1.96.0, serial screen, same NVMe target and debug0 profile.
Current migration/runtime/test hashes and retained executable hashes verified.
No release/performance claim follows from these integration timings.

The initial new harness stopped before Cargo because two independently changed
prefix/skills test files needed review for worker reuse; they were inspected,
left untouched, and included in the frozen compilation. The r2 harness reuses
only the unchanged worker and successfully checks the complete frozen inputs.
Earlier `mcp-request-identity` evidence concerns decimal waiter ownership and is
separate; it has not been replaced by this digest receipt.

MCP declaration/catalogue quotas, aggregate transport buffering, remaining
primitive resource profiles, full authored/bootstrap/component acceptance,
platform/browser-CI, packaged, remote/model and release gates remain open.


## 2026-10-16 — Complete Memory data accepted locally; remaining controls traced

Accepted source removes the generic 1 MiB JSON input/output fallback for the
four Memory primitives. Output accounting still counts the full encoded value
and kernel obligations still enforce explicitly supplied output ceilings. Memory
manifests stop advertising the shared five-second/one-MiB ceiling and invented
input-independent estimates. Source tracing shows `hard_ceiling` itself was
metadata on this path; do not claim that removing it speeds up a formerly
five-second-limited operation. Actual read/search outputs and live kernel policy
pass in `memory-complete-data-20261015-r1`.

Two additional fixed controls remain in `first_party_tools/memory.rs` and must
be repaired before the full-plan resource cutover is complete:

1. Replace `memory_tree`'s silent depth1–10 clamp with strict optional-value
   validation and explicit-depth semantics under an acknowledged, explained
   resource policy. Build/serialize deeply nested output without risking the
   native stack; retain the admitted task's recursive value bounds and preserve
   directory ordering/result format. A larger requested depth must not silently
   become10. Missing, null and invalid values must remain distinct.
2. Remove `MAX_MEMORY_PATCH_RETRIES=8` and the hidden primitive retry loop, as
   specified in the 2026-10-17 retry-policy revision below. Retry selection,
   persistent counts, eligibility and fresh kernel checks belong to the approved
   Recipe/runner contract. Without that contract a conditional-write conflict
   stops explicitly. A completed write must never be repeated because indexing,
   output or a subsequent step fails. Do not add a second configurable retry
   runner inside the Rust Tool.

The existing Tool-policy WebUI/port/store currently exposes only enabled/CAS
revision, and `PgSelectedToolPolicy` currently supplies no resource ceiling.
Do not represent a native fixture's configured output rule as an implemented
WebUI technical-control editor. Wire real settings/kernel consumers and their
acknowledgement before claiming that broader technical-policy gate complete.


Accepted receipt `/private/tmp/brassclaw-memory-complete-data-accepted.json` and
retained artifacts `target/validation/memory-complete-data-artifacts-20261015-r1`
cover:13 registered Memory cases0.858s; surface metadata0.011s; actual retained
Tool/PostgreSQL caller104.697s including complete escaped/Unicode data above
2 MiB, exact encoded-output accounting and configured kernel output8-byte denial
followed by immediate4-MiB acceptance; ordinary MCP/history36.036s; global
Match/No-Match/restart37.682s; strict host-runtime/composition test lint with
Bedrock62.618s. Source/test and retained executable hashes were verified against
the frozen snapshot. Rust1.96.0, locked dependencies, serial screen, same NVMe
target/debug0 profile. Prefix-owned edits were frozen but left untouched.

The actual worker was rebuilt because `brassclaw_resources` has an unconditional
PostgreSQL dependency; an unchanged protocol alone is insufficient evidence for
an unchanged complete source graph after a migration. The rebuilt worker has
exactly the prior SHA-256 `d1938fae7f3fe752f0b4e26cd66a1c16fb38e4b04219df6c48044f0f84bb2bf3`.
This confirms the prior compatible-worker bytes, without inventing a different
VM or transport. Earlier source-graph reuse reasoning must be read with this
additional build evidence.

That source review identified a concrete performance issue in `FilesystemMemoryDocumentRepository`
(repaired by the accepted successor below):
write/create/append conflict checks call `list_documents(scope)`, which loads
all document/chunk/version bodies in the scope to inspect path relationships.
Replace that unrelated enumeration with scoped ancestor/target-subtree reads
using existing filesystem primitives, preserving the exact document-kind,
reserved-sidecar/path filtering, conflict/error and CAS behavior. Review actual
backend atomicity and concurrent parent/child writes before changing this path;
add focused real backend coverage rather than assuming a preflight check is a
transaction. Keep complete listing/search behavior unchanged. The existing
5,205-document and native1,050-write fixtures provide real caller coverage, not
an independently controlled query-performance benchmark. The accepted successor below repairs this path; independently controlled
production performance acceptance remains open.


## 2026-10-16 — Narrow Memory preflight and atomic filesystem namespace accepted locally

Memory writes now inspect exact ancestors and the requested target subtree,
using the same document-kind, scope, sidecar filtering and original conflict
validator as complete listing. Unrelated documents/chunks/version bodies are
not enumerated. Existing CAS and complete listing/search behavior are retained.

Source inspection also exposed a preexisting correctness gap: InMemory `put`
checked descendants but not file ancestors; PostgreSQL checked only before its
mutation, permitting conflicting parent/child creation. The implementation fixes
these at the actual storage boundary. PostgreSQL put/opaque write/append and
directory creation use shared ancestor/exclusive leaf advisory transaction locks,
validation and the actual mutation/commit in explicit ReadCommitted transactions.
Sibling leaves share ancestor locks and can proceed independently. There is no
application-global filesystem mutex or new resource/deadline cap. InMemory
checks the same namespace under its existing state mutex. Opaque append resets
stale metadata in both implementations. Byte-only LocalFilesystem continues to
reject versioned CAS/records and relies on its native filesystem namespace.

The locking/read-snapshot rationale follows PostgreSQL's documented
[advisory transaction locks](https://www.postgresql.org/docs/15/explicit-locking.html#ADVISORY-LOCKS)
and [Read Committed](https://www.postgresql.org/docs/15/transaction-iso.html#XACT-READ-COMMITTED).
Existing invalid stored hierarchies are not silently rewritten or deleted;
conflicting mutations fail. Mixed old writers or direct SQL do not acquire this
new protocol, so this is not evidence for unrestricted mixed-version rolling
writes. The instance cutover/recovery gate must retain one fenced writer generation.

Native acceptance now passes sequential ancestor rejection, concurrent
parent/child put/write/append, CAS winner counts, directory-vs-file creation,
observed blocking with independent sibling progress, abort/rollback lock release
and DB rejection without effects (4.290s). All Memory contracts pass, including
complete scope/sidecar/conflict behavior and the 5,205-document case. Registered
Memory cases pass (3.718s), as do the native 1,050-write/full-result caller
(87.960s), MCP/history (36.050s), global Monty (final successor50.989s), architecture
and strict affected-package test lint with PostgreSQL/Bedrock (70.489s).

Receipt `/private/tmp/brassclaw-memory-namespace-accepted.json` retains exact
frozen source, executable hashes and test provenance under
`target/validation/memory-namespace-artifacts-20261016-r8`. Unchanged filesystem
and Memory results are explicitly retained from earlier snapshots. The final
successor rebuilds composition and rechecks global execution after an equivalent
settlement-condition lint correction. The bootstrap observer also converts its
classified service/port errors explicitly, rather than requiring an unsupported
Display implementation. There is no error suppression or alternate workflow.
The removed full-scope read is established by source and caller coverage; these
fixture durations are not a controlled production performance benchmark.

Crate guides now list actual supported backends rather than a removed LibSQL
RootFilesystem implementation; migration/import support remains separate.

## 2026-10-16 — Existing native settings browser checks wired into CI

The `native-settings-browser` job in `.github/workflows/reborn-tests.yml` runs
both existing scripts through the real PostgreSQL/global-Monty fixture, using
Node24, a locked `agent-browser@0.27.1` development dependency and Chromium.
Changes under `tests/browser/` now select Reborn validation. The classifier's
regressions, JavaScript syntax and full workflow actionlint checks pass. Actual
local browser runs pass Tool allow/block, stale-edit fencing and lost committed
acknowledgement (45.320s), plus MCP byte controls, EN/DE help, persistence/effective
revision and restoration (49.693s). Receipt:
`/private/tmp/brassclaw-settings-browser-ci-accepted.json`. The freshly installed
locked CLI and the tested CLI have the identical native binary SHA-256; npm
reports zero dependency vulnerabilities. See [browser instructions](../../tests/browser/README.md).

A later shared edit to `bootstrap_recipe_flow.rs` makes worker shutdown/join
run after admission or settlement errors. It landed after the frozen r8 build
and is explicitly excluded from those earlier native/browser executable claims.
The Memory tree successor below compiles it and accepts ordinary global/MCP paths;
its dedicated failure-cleanup qualification remains in the prefix owner's gate.

This is local browser acceptance and verified workflow wiring. The job still
needs execution on GitHub; Linux/Windows/macOS installed-product and remote/model
release gates remain open. Memory depth, patch attempts and complete technical
Tool-policy editing remain the separate work identified above. No full-plan
completion or release readiness is claimed.


## 2026-10-16 — Memory tree policy and iterative construction accepted locally

Replace the hidden depth10 clamp with live `default_memory_tree_depth` (1) and
`max_memory_tree_depth` (10). Both are positive u32 values, default<=maximum,
with EN/DE WebUI help. Explicit requests are preserved or rejected, never
clamped. V165 preserves complete/partial overrides, advances incomplete rows
once, retains an operator default's single evaluation and rejects malformed
values or revision overflow transactionally.

A registered host primitive captures the complete acknowledged resource view
before backend I/O. Production binds that view before ingress and fails closed
while unavailable; it does not read newer desired rows. Tree construction uses
a flat directory arena and bottom-up statistics for actual JSON depth, nodes
and encoded bytes before creating nested Values. Ordering and directory/file
format remain unchanged. Existing task/transport/kernel limits remain effective;
the separate transport/parser depth64 restriction has not been removed. A high
requested/configured tree depth cannot bypass the result's acknowledged typed
value profile. Old task profiles continue to apply at the retained outer port.

Native acceptance now passes invalid/null values surviving normalization,
exact depth13 after publication, omitted live defaults, rejected database edits
without runtime changes, restoration, one global VM and actual retained Tool/PG
callers (91.888s). Algorithm tests cover ordering, exact byte/node/depth boundaries,
JSON escaping and a rejected 4,096-directory path before nested Value creation
(0.416s). Registered Memory cases and declaration contracts also pass (0.800s).
V165's actual PostgreSQL upgrade/default/override/constraint/overflow test passes
(3.368s), along with rendered EN/DE tree and byte controls, save/effective revision
and restoration through global Monty (60.059s), MCP/history (44.187s), architecture
(1.613s) and strict affected test lint with Bedrock/PostgreSQL (96.686s).

Receipts `/private/tmp/brassclaw-memory-tree-accepted.json` and
`/private/tmp/brassclaw-memory-tree-api-accepted.json` retain frozen
`target/validation/memory-tree-20261016-r2`, actual binary hashes, paired worker
and per-check provenance. The canonical API parser regressions pass (0.211s):
new controls participate automatically in required-field checks; stale clients
cannot silently omit/reset them. The prefix owner corrected its own typed UUID
SQL parameter before the r2 native build. No prefix source was changed here.
Later additions of two prefix-test module declarations in composition `lib.rs`
are excluded from this frozen executable's claims. All other owned runtime,
seed, migration, UI and test inputs still match the snapshot. GitHub execution,
platform/installed-product and complete matched-library acceptance remain open.
Seed Tool/ToolSkill/Skill metadata is aligned with the new parameter semantics;
this does not certify the separate full-library preload/immutable-upgrade gates.
Patch conflict attempts and complete live technical Tool policy remain open.


### Required follow-up: project Memory document paths without full bodies

At the time of this finding, `FilesystemMemoryDocumentRepository::list_documents` obtained complete
scope records, including document/chunk/version bodies, through `query_all` and
only then filters names. The new tree result preflight does not bound those
unrelated input bodies. Before full resource/performance acceptance, add a real
metadata/path projection at the owning RootFilesystem boundary and use it for
complete document listing. Inspect every backend and scoped/mount wrapper;
preserve exact kind/scope/sidecar/path filtering, permissions, ordering and
complete pagination. Do not substitute a row/byte cap or truncate the listing.
Prove the actual PostgreSQL and in-memory paths across several pages, with large
stored bodies, sibling sidecars, all scope axes and the registered Memory tree.
The local acceptance below now resolves this document-listing finding. Its
separate version-archive scan and full-plan gates remain open.


## 2026-10-17 — Body-free Memory listing/preflight accepted locally

Add explicit structural entry metadata reads to the one RootFilesystem trait,
with native PostgreSQL/in-memory implementations, capability advertisement and
existing composite/scoped boundaries. No full-body fallback, new Tool/workflow,
query predicate over payloads, or blanket permission is introduced. In-memory
paths use an ordered map for direct prefix ranges and body-free pages; canonical
VirtualPath ordering agrees with PostgreSQL's explicit C path collation.

Memory listing filters each metadata page immediately, retains only eligible
document paths, deduplicates across pages and preserves complete enumeration,
scope/sidecar rules and ordering. Narrow conflict checks use metadata for exact
ancestors and target descendants. Chunk cleanup collects all matching metadata
paths before deleting so offset pages cannot skip shifted rows; unrelated kinds
are preserved. At this milestone, the version-archive maximum scan in
`save_document_version` still requested full records and remained separate work
(accepted in the next milestone below): audit its selected fields and integrity
contract before changing it, without deriving a logical
version from an unchecked path or hiding a damaged index record. Its unchecked
increment and signed index conversion also need explicit representation bounds;
damaged version indexes must not silently disappear from the maximum scan.
Keep the distinction between a rejected document CAS (no write occurred) and an
archive failure after a committed document write. The latter must never become
a retryable conflict or cause the completed write to be replayed.

Native evidence covers several real metadata pages with large bodies
and indexed values, scoped read/read+list denial, longest-mount routing and
capability overclaim rejection. The PostgreSQL test uses a real login role with
SELECT only on path/kind/is_dir: metadata and repository listing must work while
full body/query reads fail, proving no body fallback. All four scope axes,
reserved sidecars, corrupted-header errors, caller contracts, complete chunk
cleanup and ordinary registered Memory/Monty consumers pass.

The retained receipt `/private/tmp/brassclaw-memory-projection-accepted.json`
identifies frozen `target/validation/memory-projection-20261017-r6`, its actual
composition binary and paired worker. Unchanged filesystem evidence is reused
from r3 (96 tests); memory (115), secrets (49), outbound (7) and registered Memory
Tool (13) evidence is reused from r4. Native restricted-column PostgreSQL
acceptance passes (5.272s), namespace/CAS races (1.927s), the registered Memory
workflow with large results/live tree settings (50.963s), global Match/No-Match
(37.248s), MCP/history (36.436s), dependency boundaries (1.588s) and strict
affected-package test lint with PostgreSQL/Bedrock (68.368s). These elapsed test
times are evidence of execution, not comparative performance measurements.

The fixture now explicitly initializes the real filesystem schema. Shared
unit/integration contract imports and private fixture path calls were corrected;
the prefix owner repaired its own unrelated bootstrap error conversions. No
tests, diagnostics or runtime checks were suppressed. Source/hash verification
confirms the tested runtime inputs and retained artifacts match the current
checkout; later prose changes do not expand the runtime claim. This completes
the local projection repair, not the version-archive, complete matched-library,
platform, installed-product, release or full v3 acceptance gates.


## 2026-10-17 — Selected archive-index projection accepted locally

Keep the existing Memory write usage and Recipe path. Add a generic native
RootFilesystem projection of one explicitly selected indexed field, with a
distinct capability, PostgreSQL/in-memory support, scoped read+list and existing
mount routing. It returns no body or unrelated indexed fields; invalid selected
data fails rather than becoming missing. Propagate it through capability-forwarding
wrappers. No new workflow Tool or parallel storage plane is needed.

The version allocator scans complete pages and retains only its running maximum.
Validate each relevant archive's positive i64 index against its canonical path;
reject missing/wrong/negative/zero/inconsistent counters. Use checked signed
addition and exact conversion to the existing unsigned address API. Preserve
unrelated record kinds. No total row limit or unchecked path-derived version is
introduced. Remove the now-unused full-record scan wrapper and keep existing
tests on the supported pagination helper.

Passing acceptance covers real in-memory/PostgreSQL selected-field pages and types,
scoped/mount/capability gates, actual indexed-column permission denial, malformed
selected JSON, multi-page archives, last representable counter/exhaustion and
malformed/mismatched paths. It verifies that an archive failure after a committed
document write is an error, never a no-write conflict; a stale retry must not
replay or alter the committed document. These checks do not implement durable
effect reconciliation or the separate live patch retry policy.

Receipt `/private/tmp/brassclaw-memory-archive-accepted.json` retains frozen
`target/validation/memory-archive-20261017-r1`, actual executables and the paired
worker. All 96 filesystem, 116 Memory, 49 secrets, seven outbound and thirteen
registered Memory Tool tests pass. Native PostgreSQL metadata/column-permission
acceptance passes (5.438s), the archive contract (4.022s), namespace/CAS races
(1.940s), registered Memory/live tree/large results (51.482s), global routing
(38.588s), MCP/history (36.160s), dependency boundaries (1.599s) and strict
affected-package test lint with PostgreSQL/Bedrock (68.743s). The archive repair
needed no compile-error iteration or diagnostic suppression. Test elapsed times
do not establish comparative production speed.

Source verification retains all executable hashes and identifies the subsequent
prose-only clarification of archive errors and planned encryption. Runtime
behavior is unchanged by those comments. The prefix owner subsequently changed
its bootstrap/catalogue wiring and tests; those later changes are listed in
`/private/tmp/brassclaw-memory-archive-current-source-verification.json` and are
outside the frozen acceptance. This result does not certify the whole current
shared checkout. Durable archive recovery/reconciliation,
live patch retry policy, complete matched-library, platform/installed-product,
release and the full v3 plan remain open.


## 2026-10-17 — Single-write Memory patch accepted locally

The existing Recipe still binds `memory_write` and invokes its associated usage
once. Its Rust primitive must perform one conditional patch write, not repeat
reads and writes up to the hidden `MAX_MEMORY_PATCH_RETRIES = 8`. Remove that
loop and constant. A confirmed CAS conflict returns a redacted explicit error;
it is not a success payload, No-Match, automatic retry or fallback. Failed writes
must not emit successful chat-memory/indexing effects. Preserve the current
single successful write, metadata and derived-index behavior.

This removes a hidden retry policy rather than introducing a configurable second
runner inside a Tool. An approved Recipe/Skill retry must use the exact durable
runner contract: original invocation/arguments, persistent attempt counts,
eligible trusted no-effect/deduplication evidence, and fresh kernel checks for
policy, cancellation and execution attempt before every new dispatch. Until that
contract is selected and supported, conflicts stop. Do not manufacture approval
or turn an archive/index failure after a committed write into a retryable conflict.

Native acceptance must force a real concurrent update while the actual patch
CAS is blocked on the PostgreSQL namespace lock. After the competing writer
commits, the Tool must return an error without rereading/patching the new body.
Verify the competing body and record version, absence of patch archive/index
effects, and live denial on the next kernel dispatch. No mock filesystem or
arbitrary sleeps may stand in for observing the blocked backend operation.
Full Recipe retry/recovery acceptance remains a separate required gate.

Accepted receipt `/private/tmp/brassclaw-memory-patch-accepted.json` retains
`target/validation/memory-patch-20261017-r2` and the actual native executable/
paired worker. Thirteen unchanged registered Memory cases pass (0.872s, reused
from r1); the real Memory/PostgreSQL/namespace-lock race and next-dispatch live
block pass (56.339s), alongside global Match/No-Match (42.274s), MCP/history
(40.125s), architecture (1.584s) and strict affected test lint with PostgreSQL/
Bedrock (56.822s). The competing body, exactly its version increment, all document
sidecars/index payloads, chat-memory count and embedding request count survive
the conflict unchanged. The existing global VM identity is retained.

The first native behavioral run also passed. Strict lint initially rejected
unused imports in the prefix owner's new compiler-approval module. Its owner
removed them and boxed the large approval variant. The reuse check rejected
treating that source change as imports alone; native callers were rebuilt and
rerun against r2. No diagnostic was suppressed. Later independent prefix changes,
if any, are excluded by the source-verification receipt. These elapsed test
times are execution evidence, not comparative performance measurements. This
accepts removal of the hidden primitive retry policy; it does not qualify a new
automatic Recipe retry, durable deduplication or full recovery/cutover support.

### Required next vertical: real operator Tool output ceilings

`PgInstanceToolSettings`, the product DTO/Tools tab and `PgSelectedToolPolicy`
currently expose/read only enabled/revision; the kernel receives
`resource_ceiling: None`. Add a durable nullable per-Tool output-byte ceiling
through that existing UUID-keyed CAS boundary, with EN/DE help, exact integer
transport, clearing semantics and actual kernel uptake across retained aliases.
Old/partial clients must not silently erase an existing ceiling while toggling
allow/block. Keep component activation/approval independent and preserve existing
operator overrides during seed upgrades. Test revision races, rejected writes,
corrupt values and lost acknowledgements without replay.

The existing obligation handler actually enforces output bytes, but explicitly
rejects wall-clock and nonempty sandbox ceilings in
`validate_supported_resource_ceiling`/`validate_supported_sandbox_quota`.
Those unsupported controls require their real runtime/containment/recovery work
before they become editable; exposing their DTO fields alone is not completion.
Token controls must honor the instance's disabled-token-budget mode rather than
introducing another artificial cap. The output-ceiling vertical does not complete
these other technical-policy gates. Native acceptance must use the real operator
facade/router/PostgreSQL and a retained Tool, proving live rejection/increase/
clear behavior, complete results without clipping and the same global VM.


## 2026-10-17 — Operator Tool output-ceiling local acceptance

V166 adds nullable `max_output_bytes` to the existing UUID-keyed Tool policy row,
using exact positive unsigned byte representation without numeric rounding.
Existing enabled values and revisions are unchanged. The operator DTO carries
required complete `limits`, with nullable quoted-decimal `max_output_bytes`;
missing/partial/unknown limits and ambiguous numbers fail before mutation.
The existing CAS transaction persists enabled/limits/revision together, decodes
before commit and preserves retained aliases and seed overrides. Kernel policy
reads the limit in the same current row, with existing attempt and post-prepare
admission checks. No second settings publisher or authority registry is added.

Ordinary Recipe dispatches have unknown output estimates. Instance policy now
permits that unknown advisory value while retaining actual output obligations;
known excessive estimates still fail before effects. All other estimate checks
and the legacy authorizer remain unchanged. This is required for a usable result
ceiling, not permission to omit measured enforcement. Oversized results fail
after redaction, without truncation or replay, including effects already completed.

The Tools tab edits/clears this limit with EN/DE explanations and exact decimal
values beyond JavaScript integer precision. Allow/block preserves the full limit.
Draft edits, stale revisions and lost acknowledgements retain the existing
refresh/no-replay fence. Required native coverage includes V166 predecessor
upgrade, full unsigned range, invalid/fractional/corrupt state, concurrent CAS,
rejected persistence and actual browser/operator/PG/global-Monty application.
Exercise an output denial after a real reply, retain that reply/invocation through
correlated resubmission/recovery, then raise/clear the limit on later calls in
the same global VM. This does not complete unsupported wall/sandbox controls,
token-budget-mode reconciliation or the broader technical-policy/cutover gates.

Local acceptance: `tool-output-policy-20261017-r5` and receipt
`/private/tmp/brassclaw-tool-output-policy-accepted.json`. Two wire tests,
five frontend API checks, actual PostgreSQL policy/upgrade rejection (2.879 s),
registered Memory/kernel policy checks (54.886 s), ordinary global Monty plus
Chromium EN/DE operator editing (55.048 s), MCP/history (39.423 s), architecture
(1.590 s) and strict affected-package PostgreSQL/Bedrock test-target lint
(55.824 s) pass. Reuse guards retain unchanged artifacts and caller evidence;
three final test-only cloned comparisons become equivalent borrowed slices.
The initial regression accidentally reused an invocation ID; separate calls now
have separate contexts. Browser clearing uses real keyboard deletion because an
empty CLI fill did not produce a React edit request. No assertions or warnings
are suppressed. Native actor-injected operator routing is not a new bearer-auth
certificate; full installed Linux/Windows, technical controls and release gates
remain open. This milestone does not mark the whole plan complete.


## 2026-10-17 — Remove secondary MCP discovery caps

Discovery no longer applies its separate 64 KiB declaration/sentence ceiling,
1024-program ceiling or 64-variable/positive/negative-example maxima. The
retained revision parser, catalogue admission, command type/layout checks,
exact qualification and captured HTTP resource limits still apply. Names retain
the existing public-name contract. No incomplete catalogue is published and no
unqualified component becomes advertised.

Frozen `mcp-declaration-20261017-r1` passes four contract/actual-worker cases
(2.134 s), ordinary global Monty (43.724 s), actual MCP/history (39.754 s) and
strict affected PostgreSQL/Bedrock test-target lint (58.103 s). The large parser
case retains an 81 KiB sentence and all 65 typed positions/examples. A 1025-input
duplicate fixture retains exact Conflict/no-publication behavior; it does not
claim successful qualification of 1025 distinct public commands. Receipt:
`/private/tmp/brassclaw-mcp-declaration-accepted.json`. Complete library discovery,
enclosing revision limits, transport/parser depth and aggregate buffering remain
required work; this is not whole-plan or installed-platform acceptance.


## 2026-10-17 — Iterative Monty input arena prerequisite

The existing JSON-to-Monty adapter recursively builds an owned MontyObject for
every child, then merges/rebases its arena into each parent. Replace that adapter
with one bounded post-order worklist and one validated MontyGraph. Preserve
JSON value-node/depth and string/key-byte accounting, unsigned integer fidelity,
container order and classified failures. Check graph ID representation and use
fallible reservations before allocating traversal/child storage; partial failure
must drop only flat arena/worklist state, without recursive owned subtrees.

This is shared value-transport infrastructure, not a new Tool or workflow.
Prove deep construction/rejection on a small Rust thread stack and preserve
actual worker/global/MCP behavior with relevant strict lint. Do not remove the
separate process transport depth guard yet: receiving JSON, recursive output,
clone/drop, journal and recovery consumers need their own safe implementation
before larger acknowledged depths can traverse that boundary. A deep adapter
unit test is not ordinary global/deep-wire acceptance.


Local prerequisite acceptance: `input-arena-20261017-r1`, receipt
`/private/tmp/brassclaw-input-arena-accepted.json`. Seven host unit cases
(0.417 s) include exact scalar/budget checks and 12,000-level construction,
rejection and flat arena cleanup on a 64 KiB thread stack. Actual worker live
bounds/retained state (0.539 s), task-local oversized-result handling (0.020 s),
ordinary global Monty (45.064 s), MCP/history (39.927 s) and strict host/composition
PostgreSQL/Bedrock test-target lint (66.901 s) pass. Source and paired artifacts
are frozen; no benchmark or cross-platform/deep-wire claim follows from these
local tests. No dependency, wire format, Tool, workflow or resource policy changes.

Next required depth work, before removing `MAX_TRANSPORT_DEPTH`:

1. Replace recursive output conversion with bounded occurrence traversal,
   preserving shared host-argument accounting and rejecting unsupported/cyclic
   values, invalid keys and duplicates before publication. Error cleanup must
   also be iterative when earlier results contain a deep owned subtree.
2. Make parent/worker encoding, decoding and recovery safe for configured depths.
   Reuse the captured value/frame policy; disabling serde recursion protection
   alone is insufficient. Keep duplicate-key/type checks and exact complete data.
3. Trace owned JSON cloning/dropping and journal/PostgreSQL serialization as well
   as the VM converter. Remove the fixed transport clamp only after real live
   WebUI acknowledgement, deep task/child/wait/recovery, rejected reductions and
   no-replay acceptance prove the entire path. Keep immutable revision/schema
   retention and live technical policy distinct; a parser helper is not cutover.


## 2026-10-17 — Iterative output and host-argument custody

Replace recursive Monty output traversal with a bounded post-order occurrence
worklist. Count repeated graph references each time they expand into JSON;
reject unsupported/cyclic nodes, nonfinite numbers and invalid/duplicate keys.
Keep one ValueBudget and one owned JSON forest across every positional and
keyword argument. A late argument/key/byte failure must clean up earlier deep
results iteratively before any host request can be published.

Reserve the forest's worklist for visited value nodes before constructing their
owned data. Its disjoint trees contain no more nodes than that reserved count,
so error cleanup can reuse the same buffer to dismantle arrays/objects without
recursive Drop or another allocation. Validate frame/child counts including
queued occurrences and allocate child vectors before draining owned values.
On success, preserve exact order, types and keyword names. Keep the process
transport depth restriction until caller encoding/decoding, ownership and durable
journal consumers also support configured depths. Require small-stack deep
success and late-failure tests, DAG-expansion/aggregate budget coverage and actual
worker/global/MCP acceptance; no helper-only deep-wire claim.


Local output prerequisite acceptance: `output-forest-20261017-r1`, receipt
`/private/tmp/brassclaw-output-forest-accepted.json`. Eleven host unit cases
(0.446 s) pass, including 12,000-level output and late result/argument/key/byte
failure cleanup on a 64 KiB stack, exact scalar/aggregate budgets and bounded
DAG expansion. Actual worker live bounds (0.557 s), task-local value failure
(0.020 s), ordinary global Monty (45.147 s), MCP/history (39.812 s) and strict
host/composition PostgreSQL/Bedrock test-target lint (60.696 s) pass. No dependency,
wire format, workflow or resource policy changes. Initial module docstrings in
`monty_host/src/{lib,global}.rs` now describe the production global service;
body bytes and line positions remain unchanged, so no extra Cargo run is needed
for those subsequent comments. This does not complete the authoring-doc audit.

Remaining depth consumers include `process.rs` frame/recovery codecs;
`retained_recipe.rs` canonical checksum walking and input/answer cloning;
`host_runtime/obligations.rs` dispatch cloning, recursive redaction and result
serialization; and `pg_monty_invocation.rs` journal serialization. Memory tree
already checks the acknowledged value-depth bound before building nested JSON.
Preserve that effective-profile check while making the downstream consumers safe;
remove neither the transport guard nor kernel enforcement prematurely. Actual
profile changes/deep journal/child/wait/recovery and installed platform gates
remain open. Small-stack adapters alone do not establish deep-wire support.


## 2026-10-17 — Shared streaming serialization and iterative data checks

Use a shared serialization utility in the existing neutral `brassclaw_common`
crate for kernel/primitive byte counting, Monty frame encoding and invocation
journal encoding. Preserve exact Serde JSON bytes, error classifications, writer
ceilings and complete output; count through a writer instead of allocating a
second full serialized result. `serde_stacker` protects nested serialization
callbacks, including wrapper/envelope types. Its upstream docs explicitly exclude
Clone/Drop/Display from that protection; do not claim ownership or decoding safe
by introducing this adapter alone (https://docs.rs/serde_stacker/0.1.14/serde_stacker/).

Make borrowed data-bound traversal and typed-value checksum traversal iterative.
Preserve object-key node/byte charges and the exact typed-value-sha256/1 format.
Separate positive execution-data depth bounds from the still restricted
schema/parser profile; do not relax strict_json or live transport admission.
Test historical hash vectors, deep small-stack validation/hash/serialization,
late byte/writer errors and actual worker/kernel/PostgreSQL/global consumers.
Cloning, redaction, owned-value cleanup, strict parsing/decoding and full deep
WebUI/child/recovery acceptance remain prerequisites for removing the64guard.


Local streaming prerequisite acceptance: `json-streaming-20261017-r1`, receipt
`/private/tmp/brassclaw-json-streaming-accepted.json`. Two serializer/byte/error
cases, 12 contract cases, 11 host cases, 15 real-worker actor cases, two retained
service cases, seven retained-input cases, all 137 registered built-in Tool
cases, two invocation encoding cases, actual Memory/kernel/PostgreSQL (52.167 s),
ordinary global Monty (41.474 s), MCP/history (39.008 s), architecture (1.579 s)
and strict affected PostgreSQL/Bedrock test-target lint (97.992 s) pass. The
separately locked control workspace also passes live-frame retention/reduction
and malformed-frame rejection against the real worker. The two lockfiles add
only serde_stacker0.1.14 and the corresponding dependency edges.

The journal filter initially named the file instead of its Rust module alias;
the runner rejected its empty run. Corrected exact tests reuse the unchanged
frozen binaries and all prior successful results. No empty run, suppressed
warning or assertion contributes acceptance. Disk91% triggered the mandated
cleanup of this chat's target after preserving its executables; target2 was
untouched. A later two-line API doc clarification preserves body bytes and line
positions, requiring no additional Cargo execution for prose.

These tests prove deep encoding/bounds/checksum helpers and current ordinary
callers, not deep ordinary recovery or full plan completion. Kernel dispatch
cloning/redaction, caller-owned JSON disposal, strict parsing and reader/journal
recovery remain explicit blockers to lifting the transport profile. The v1
checksum format and current schema/parser admission are unchanged.


## 2026-10-17 — Kernel dispatch output ownership

Replace the completion handler's recursive dispatch clone and redaction with a
borrowed traversal that owns intermediate output in one cleanup guard. Reserve
cleanup capacity before constructing nodes; never transfer a deep subtree to an
unguarded local across a fallible check. Validate transformed object keys before
assembly so collisions fail without dropping a replaced subtree recursively.
Retain the guard through byte/resource limits, handoff cleanup and the awaited
audit write; transfer the final value only after completion succeeds. Keep
payload-free leak rejection events, preview clearing and exact failure kinds.

Compile the immutable LeakDetector when wiring the handler, share it across
handler clones and reuse it for all strings/keys. Regex compilation is setup
work, not per-node or per-dispatch work. A 64 KiB regression initially identified
regex compilation's independent stack requirement; retain that failed evidence.
The corrected test constructs the real handler before moving it onto the small
stack, matching service wiring, and tests six 12,000-level completion scenarios:
copy, redaction, late secret rejection, late key collision, byte-limit rejection
and missing-audit-sink rejection. It checks retained metadata and leak events.
The real source/result DTOs still need explicit dismantling in this regression;
this does not establish safe public DTO ownership or decoding.

Do not remove transport/schema depth restrictions based on this handler repair.
Retained execution still clones inputs, host arguments, completion boundaries
and answers. Public Value ownership, strict parsing, partial decoder failure,
PG journal recovery and actual deep task/child/wait/live-profile acceptance
remain required. No new Recipe, Tool or workflow executor is introduced.

Local acceptance: `dispatch-output-20261017-r2`, receipt
`/private/tmp/brassclaw-dispatch-output-accepted.json`. All 33 obligation contract
cases, seven obligation unit cases and 137 registered Tool cases pass. Actual
ordinary global Monty (45.331 s, including post-effect output-limit no-replay),
Memory/kernel/PostgreSQL (54.121 s) and MCP/history (43.608 s) pass against the
paired real worker. Strict HostRuntime/Composition PostgreSQL+Bedrock test-target
lint passes (77.485 s) without warning suppression. Frozen source and retained
executables are preserved. R1's cold-regex small-stack abort remains separate
failed evidence; it is not counted as acceptance. This is local macOS evidence,
not installed Linux/Windows acceptance or completion of the full v3 plan.

## 2026-10-17 — Guard partial strict-JSON parser ownership

Move strict JSON parsing into `skills/value_contract/json.rs`. Deserializer
seeds return no owned Value; visitors retain indices into one guarded forest.
Reserve cleanup capacity before adding each pending node. A closing container
consumes only its child roots, preserving earlier sibling indices; map visitors
retain key/index metadata, reject duplicates before values, then assemble only
after fallible parsing/budget checks. Keep the forest until decoder.end succeeds
so trailing garbage also cleans up the complete root iteratively. Stack-protect
Serde callbacks using the already pinned serde_stacker release. Preserve key
node/UTF-8 byte charges, source-byte admission, error reasons and all value types.

Keep ContractLimits' supported parser/schema depth at 64 and Serde's reader
recursion limit enabled. This internal repair does not permit larger live
profiles, protect public Value DTO disposal or fix derived protocol deserialization.
The latter can still hold an owned deep field before later fields fail and needs
its own safe representation/decoder path. Do not disable reader recursion simply
because these strict-parser visitors now hold only indices.

Exercise the public strict_json caller on a 64 KiB stack: exact 66-node parsing,
late node/depth rejection, completed earlier subtrees followed by bad syntax,
Unicode/number errors and escaped duplicate keys, plus trailing-data rejection.
Check shallow scalar/object equality against Serde and unchanged profile denial.
Keep component/association/revision, retained-input, native global/kernel/MCP,
independently locked retained execution, architecture and strict affected lint
acceptance. Both lockfiles add only Skills' edge to the already pinned package.

Initial R1 parser evidence was partial, recorded in
`/private/tmp/brassclaw-guarded-parser-partial.json`: 13 value-contract, five
association, three revision, 35 Skills unit and seven retained-input cases pass.
Actual ordinary global Monty (46.347 s), Memory/kernel/PostgreSQL (54.551 s),
MCP/history (44.515 s), architecture and strict Skills/Engine/Composition
PostgreSQL+Bedrock lint (107.055 s) pass on `guarded-parser-20261017-r1`.
The independent retained-execution build exposed a missing direct Common
dependency in its source-included journal and the prefix owner's new internal
result-reference module missing from three harness roots. The former is repaired
in the standalone manifest/lock; the latter remains with its production owner.
Do not count the failed standalone build as acceptance. Its repaired module and
feature wiring still need independent compilation/execution, including supported
feature shapes, before this prerequisite is recorded as fully accepted.

The first retained-input run also lacked a sibling packaged worker. Staging the
actual paired worker made all seven cases pass without source changes; the four
unchanged Skills suites reuse their retained successful executables. At 91% disk
use, the runner preserved binaries before cleaning only its own Cargo target.
No parser/transport admission relaxation, warning suppression, fabricated Tool
result or full-plan completion follows from these prerequisite results.


Parser prerequisite accepted locally: receipt
`/private/tmp/brassclaw-guarded-parser-accepted.json` records the R1 root snapshot
and R3 independent snapshot separately. All non-Composition crate inputs and the
root lock remain identical, so the earlier parser/Engine/native/architecture/lint
checks retain their stated scope. This does not certify later prefix-only
production changes or new prefix functionality.

The standalone manifest now directly declares Common, used by its included
production journal. The prefix owner separated the actual result formatter from
the database evidence adapter, and split admission into shared claim/journal core
plus service lifecycle methods. The appropriate harnesses use those actual
production parts. An initially proposed empty fixture feature was removed;
unsupported feature paths and warning exemptions are not used. A second run's
unused-method warnings remain failed evidence, resolved through the source split.

R3 independently builds all three affected harnesses and passes all seven actual
retained-execution/journal cases (128.190 s), ten Q1/journal cases (58.999 s) and
strict three-harness lint with --all-features (3.311 s). The parser's source,
budgets and public API are unchanged from its passing R1 regression. Public
DTO/retained execution cloning and disposal, generic wire decoding, configured
deep execution/recovery and installed Linux/Windows acceptance remain open.
The full v3 goal remains unfinished; no release, activation or speed claim follows.

## 2026-10-17 — Retained host-answer ownership

Use an immutable host-API JSON handle for PortAnswer::Return and HostAnswer::Return.
Clones share read-only data; they do not recursively clone the tree. Reserve
cleanup work storage before retaining a complete tree, and transfer a parser's
existing guarded storage into the retained handle. Final release dismantles the
tree iteratively. Debug output stays private; serialization and equality must
preserve standard JSON values and exact compact wire bytes. The VM still builds
its own mutable invocation values; sharing immutable host data grants no Tool
permission or cross-task mutable state.

Do not rely on derived internally/adjacently tagged enum deserialization for
these data-bearing answers: Serde may buffer deep Content before the guarded
field is constructed. Parse answer fields directly and keep data guarded through
later tag, duplicate, unknown-field, metadata and syntax failures. Keep the
existing supported process depth/admission gate until the rest of Request,
WorkerCommand, RecipeCommand/Boundary/Event, schema binding, journal recovery and
public output/input ownership are qualified. Test-only codec qualification with
reader recursion disabled does not enable deeper live values.

Qualify deep host-data construction, shared clone, ordinary drop, equality,
serialization, partial syntax and later enclosing-field failure on a small stack.
Verify Return/error wire shapes and complete typed exception roundtrips. Bound
exception metadata by its pinned nonrecursive wire schema, independently of the
operator's arbitrary JSON data depth setting. Actual worker, retained result
failure/cancellation, PostgreSQL answer encoding, normal global task, MCP/history,
architecture and strict affected root/standalone checks remain required.

Additional remaining limit found in the pinned control fork:
monty-types/src/exceptions.rs still uses fixed 64 KiB MAX_OBJECT_LEN and
MAX_DOC_LEN for structured Unicode/JSON error payloads. Map those copied-payload
bounds to acknowledged execution/preparation settings or remove them with proper
allocator/host accounting. Preserve exact error position/document behavior and
an explicit oversized/unavailable result, never a partial document. Exercise
boundary/oversized error handling in real worker and recovery; source comments
about omitted data are not configurable-limit acceptance.

Local host-answer prerequisite accepted: receipt
`/private/tmp/brassclaw-shared-answer-accepted.json` separates the R1 runtime
snapshot from the R3 fixture-only successor. Three small-stack JSON-handle
tests, thirteen host units, fifteen actor cases, two live/completed-answer
service cases, seven Engine retained-input cases and two journal cases pass.
Ordinary global Monty (43.670 s), actual Memory/kernel/PostgreSQL (53.520 s),
MCP/history (41.264 s), 28 architecture checks and strict affected root
PostgreSQL+Bedrock test lint pass. Independent retained execution (seven cases),
Q1/journal (ten), child execution (eleven) and live-frame uptake pass too.

Strict standalone lint first caught a redundant reference in the new handle
comparison. The fixture assertion is repaired without suppression; its actual
dependent-task and invalid-completion tests and strict six-harness lint pass
in R3. All runtime inputs remain byte-identical to R1, so their earlier passing
evidence is reused. The first successor launcher failed source verification
because its snapshot copy followed symlinks; it ran no Cargo/test command.
R3 preserves and verifies those links. No failure is counted as acceptance.
Current-source proof separately lists later concurrent prefix changes; those
changes are not certified by this receipt. The outer protocol, deep recovery,
remaining fixed limits, installed platforms and full v3 plan remain unfinished.

## 2026-10-17 — Prepared-run identity count repair

Remove the compatibility coordinator's fixed 4096-entry preparation ceiling.
These reservations retain routing scope until submission or explicit abort;
they are not VM capacity or Tool permission. Allocate the next map slot fallibly
before insertion, reject identity collisions instead of replacing a fence, and
retain every outstanding reservation through unrelated aborts/admission errors.
An entirely drained ledger releases its backing allocation without pressure
polling or compaction of an occupied table. Existing submit/child/abort semantics
and durable store admission remain unchanged.

Current source inventory finds only the compatibility implementation and contract
callers of prepare/abort, so do not invent a WebUI control with no production
consumer. Run the actual coordinator regression across 4097 simultaneous
reservations, old/new foreign-scope rejection, failed profile admission,
unrelated and repeated aborts, exact requested-ID admission and subsequent fresh
work. All142 coordinator contracts pass (0.882 s), including this new regression;
strict affected all-target lint passes (8.723 s). Receipt
`/private/tmp/brassclaw-prepared-identity-count-accepted.json` pins the two-file
coordinator successor and complete frozen inventory. Other turn-store/subagent
retention limits and the full v3 plan remain separate unfinished work.

## 2026-10-17 — Explicit subagent goal retention

The in-memory goal backend previously evicted the oldest entry at 4096,
irrespective of whether its task still needed it. Remove this eviction/count
quota and its insertion-order queue. Retain each accepted scope/run goal until
the existing owner calls delete; duplicates remain errors. Reserve insertion
capacity fallibly and reject poisoned storage rather than recovering silently.
Delete only the addressed key, releasing the backing table when fully drained.

Remove the separate 64 KiB serialized-goal ceiling from both in-memory and
PostgreSQL backends, including the redundant complete JSON allocation used to
measure it. This does not change upstream admission/execution limits or database
representation constraints. Store complete task/handoff strings as data; no
truncation, interpolation or replay. The obsolete payload-limit error mapping
is removed from both existing adapters. Preserve existing backend keys and
duplicate/deletion semantics rather than inventing new scope/approval rules.

Local acceptance: all106 surrounding subagent tests pass (0.612 s), including
in-memory >4096 complete retrieval/drain, large escaped/Unicode task and handoff,
duplicates, scoped deletion and poisoned-state errors. Real PostgreSQL complete
committed/reopened large-value roundtrip, duplicate/foreign rejection and exact
deletion pass (4.310 s); ordinary global Monty passes (44.606 s). Strict affected
Reborn/Composition PostgreSQL+Bedrock test lint passes (96.758 s) without warnings.
Receipt `/private/tmp/brassclaw-subagent-goal-retention-accepted.json` pins the
four-file source successor and artifacts; current-source proof separates later
concurrent prefix/Cargo changes, which this batch does not certify. The gate/tombstone stores and other retention
controls remain separate work; this is no new workflow Tool or Rust task loop.

Follow the complete goal path before claiming product-wide large-goal support:
loop_support/subagent_spawn_port.rs still advertises/enforces the separate
DEFAULT_SUBAGENT_GOAL_MAX_BYTES=64 KiB per-field limit. Its prompt composer has
64/128 KiB sanitized/raw limits, collapses whitespace, then incorrectly carries
task data through LoopSafeSummary's 512-byte/status-delimiter contract.
Do not weaken log/status summary protection to carry actual task text. Repair
the actual typed prompt/ingress consumer and its acknowledged data bounds, or
use the complete retained ordinary-message path where that is the qualified
design. Prove complete literal task/handoff transport through the consuming
path; a backend roundtrip alone does not close these consumer limits.

Also trace SpawnCompensationState::rollback and rollback_batch_compensation:
they currently discard cancellation, gate/goal/result/thread cleanup errors and
can drop unresolved cleanup state. The completion observer does propagate goal
deletion errors; preserve that behavior. The spawn owner needs explicit retained
cleanup outcomes and no replay of admitted children/effects; a warning, another
Rust workflow loop or a manufactured successful rollback is insufficient. This
must be resolved alongside R09's real child-admission/continuation cutover;
the existing authorized:true primitive is not child execution evidence.

## 2026-10-17 — One committed Tool-dispatch identity

R09 tracing found that SelectedTools constructed a kernel ExecutionContext while
PgMontyAdmission::begin_tool_invocation generated a second independent UUID.
The first-party ResourceScope therefore could not identify its committed effect
intent. Reuse the host-created ExecutionContext invocation identity in the
intent insert; both ExecutionContext and its validated ResourceScope already
carry that identity. Reject nil identities before writing an intent. Keep the
independent private invocation key, exact retained selection, claim fencing,
attempt count and record-before-effect ordering. An invocation ID is correlation
and deduplication data, never permission or proof that an effect completed.

Update every actual retained-kernel fixture to commit the identity it dispatches;
source-only journal probes supply their own explicit fresh identities. Extend the
existing actual PostgreSQL/kernel journal regression to verify the committed ID
before execution and inspect real resource-governor reservation/reconciliation
events for that same ID, including nil rejection without an inserted intent.
Cancellation, uncertain/lost answer commits, replay rejection, retained execution,
Q1 and ordinary global/Memory callers remain required acceptance. This is a
prerequisite for real correlated child admission, not an implementation of it.
Compilation, runtime checks and strict affected root/standalone lint are pending.

Accepted locally in dispatch-identity-20261017-r2: the actual PostgreSQL/kernel
journal regression passes (17.933 s), including nil rejection, identical committed
and reserved/reconciled IDs, cancellation, uncertain answer commit and no replay.
All seven retained-execution cases (125.869 s), ten Q1/journal cases (60.398 s),
strict three-harness all-feature lint (22.265 s), ordinary global Monty (46.606 s),
Memory/kernel/PostgreSQL (53.949 s) and strict affected root PostgreSQL+Bedrock
test lint (105.838 s) pass without warnings. R1's missing UUID qualifier in the
new test remains failed compilation evidence; R2 repairs only that name before
running this complete batch. It is not counted as a passing check.

Receipt `/private/tmp/brassclaw-dispatch-identity-accepted.json` and current-source
proof `/private/tmp/brassclaw-dispatch-identity-current-source-verification.json`
pin all eight changed files and retain the unchanged paired worker explicitly.
Later concurrent prefix/Cargo changes remain outside this frozen evidence.
Real R09 child admission, inherited task selection, durable cleanup and the full
v3/platform/release gates remain unfinished.

R09 implementation constraints from the actual caller trace:

- FirstPartyCapabilityRequest already receives ResourceScope.invocation_id. Use
  the committed ID to correlate a task-bound admission operation; do not add a
  caller-controlled parent/run/attempt field or treat that ID alone as authority.
  Verify the original intent, arguments, exact parent attempt and live admission.
- OwnedGlobalTaskFactory::dependency_parent currently proves only an MCP exchange
  and its live provider surface. A delegated Recipe child needs its own durable
  admission provenance, matched to the actual retained parent TaskPorts before
  borrowing dependent capacity. Do not forge MCP receipts or require a provider
  exchange for non-MCP children.
- InstalledMontyCatalogue::TaskCatalogue contains immutable view/source selection
  plus already task-bound SelectedTools. Copying SelectedMontyRecipe.tools into
  a child would keep the parent's host, admission, answers and kernel bindings.
  Retain the inherited definition/generation selection, then bind fresh child
  execution/Tool ports to the child's real host and admission. Do not capture
  latest instead, or share mutable parent execution state.
- The existing task-bound kernel reply/review registrations demonstrate the
  supported contextual binding seam. Admission must perform one declared system
  primitive; Recipe/Monty owns choosing work, waiting, consuming results and
  completion. Do not route this through the legacy Rust spawn decorator or hide
  a complete child workflow in a new Rust Tool.
- Preserve full typed goal/handoff through admission and prompt consumption.
  Ordinary message/data transport and internal Recipe entries already exist;
  inspect their exact contracts before choosing the supported child entry. A
  safe status summary is not a raw task-data carrier. Retain uncertain admission
  and cleanup outcomes without replay or destructive best-effort rollback.

## 2026-10-17 — Complete prepared model input behind existing prompt authority

The actual R09/model-usage trace above reaches a second data-carrier mismatch:
LoopPromptBundleRequest.inline_messages uses LoopSafeSummary, a short status/log
contract with 512-byte and delimiter restrictions. It cannot carry a complete
Recipe task, handoff, document or assembled model prompt. Do not weaken that
status contract, resurrect the unused permissive PgKohaiPort parser, or expose
raw resolved_messages to bypass the actual ThreadBackedLoopModelPort authority.

Implement a host-only PreparedLoopPromptRequest with typed system/user/assistant
messages and the correlated InvocationId, distinct from root-port JSON. Carry
complete strings as data, never source. Derive opaque body/role/order references
from SHA-256 under run, scope and invocation identity; debug/milestones contain
metadata only. Reuse InstructionBundleBuilder, InstructionMaterializationStore,
LoopPromptBundleAuthority, current surface validation, safety context and hooks.
Move materialized message bodies into the store instead of cloning the whole
body collection. Reject mixed context cursors, unverified Recipe hints, empty
message sets, nil identities and stale surfaces. An explicit enabled message
limit rejects the complete request rather than clipping it; disabled artificial
budgets ignore that hint. The prepared path has no default 32/maximum 128 message
window or status/snippet text-size clamp. Captured usage input/resource admission
and provider technical limits still belong to the actual consuming model usage.

Forward through RebornLoopDriverHost and its resume wrapper, HookedLoopPromptPort
and SubagentLoopPromptPort. Hooks retain their materialization and refreshed
single-use bundle authority. Child capability-view narrowing remains host-owned;
the complete Recipe prompt does not pass through legacy goal reformatting.
MontyTaskHost uses its existing begin/finish acknowledgement barrier, preserves
late prompt answers for reconciliation and rejects preparation after fencing.
There is deliberately no new raw-body dispatch_port operation or Tool grant.

Scoped local acceptance in prepared-prompt-20261017-r2/r3: all three complete
prepared-prompt contracts, eleven hook prompt cases, nine existing child prompt
cases and four ordinary prompt cases pass, followed by strict affected Turns,
Hooks, LoopSupport and Reborn test lint. R1 retains the failed diagnostics for
private profile-constructor use in the two new test fixtures; R2 fixes those
fixtures using the actual public RunProfileResolver, without exposing the private
constructor. The R3 concrete MontyTaskHost resolver/fencing case and all four
surrounding task-host cases pass (0.579 s); strict Reborn test lint passes (3.335 s),
actual PostgreSQL/global-Monty matched/unmatched regression passes (45.870 s),
and strict affected Composition PostgreSQL+Bedrock test lint passes (66.778 s).
There are no suppressed warnings. The concrete resolver test uses the existing
recording model gateway: it proves exact transport and run/turn identity, not a
real provider call or activated canonical model Skill. R4 adds one direct child
wrapper/store test for complete data and retained capability-view narrowing;
all ten child prompt cases pass (0.296 s) and strict LoopSupport test lint passes
(13.689 s), recorded separately in the receipt.

Receipt `/private/tmp/brassclaw-prepared-prompt-accepted.json`, immutable r2/r3/r4
artifact receipts and `/private/tmp/brassclaw-prepared-prompt-current-source-verification.json`
separate eleven exact changed inputs from later concurrent prefix/Cargo changes.
R4 changes only a cfg(test) child regression; its production prefix is identical
to accepted R3. Current combined workspace reconciliation remains required.

This closes a prompt-data infrastructure prerequisite, not the complete model
or child workflow. Still implement and qualify the existing canonical
pc-host-kohai-complete usage with a finite strict typed mapping, exact task-pinned
full prefix/model/provider identity, structured response/tool-request retention,
actual model accounting, current kernel policy and durable effect identity.
Retain the actual production interceptor path; do not wire the dead permissive
bridge or expose root-only controls to arbitrary children. Three basic message
roles do not by themselves implement provider Tool-result/replay metadata.
Then complete real R09 admission, inherited immutable definitions with fresh
child-bound execution ports, explicit Monty wait/result handling and durable
cleanup. Installation seed qualification applies; no external maintainer Q2 is
needed for first-party shipped implementation. Full v3/platform/release gates
remain open. The prefix owner has been notified of the current captured-prefix
lookup's fallback to the active generation; a selected task must never use it.

## 2026-10-19 — Selected single-model primitive and native answer custody (candidate)

The next canonical model usage retains `host.kohai_complete` and the existing
pc-host-kohai-complete identity for qualification; it does not revive the legacy
permissive PgKohaiPort or protected-root model component. The implemented native
adapter's finite input profile is `{prompt:{user_query:string,
chat_history:[{role:system|user|assistant,content:string}]}}`, with every field
required and no extras/defaults. There is no caller prefix selector, provider
selector, raw source or root-control field. The task-owned kernel registration
captures the immutable selected catalogue prefix/generation/hash; it shares its
read-only prefix text across that task's model bindings. Actual calls materialize
owned message data through the existing scoped prompt authority, hooks, model
policy/accounting/interceptor ports and cancellation barrier. Binding a catalogue
must not require a configured model for unrelated Tier-0 work; the actual model
dispatch requires a valid retained route. Internal deterministic-root model
restrictions remain; extending qualified internal model work is separate work.

The primitive returns the complete existing LoopModelResponse as JSON text,
including structured proposals and unknown usage. Recipes perform explicit
parsing, semantic checks, proposal handling and completion. The consuming Recipe
must use this actual revised profile; old ok/answer assumptions, placeholder
fields and state[previous_result] are not compatible. Three text roles do not
implement provider Tool-result replay history. Retain the required future
structured profile and exact matching adapter rather than silently skipping it.

V168 adds append-only native model-response evidence tied to the existing
committed invocation. Preparation checks the original admission/claim, invocation,
exact arguments, selected capability and privately captured prefix reference.
Received responses enter an owned pending state before encoding/database work.
The native answer is committed before later kernel/VM output validation. Failure
or ambiguous acknowledgement retains the actual response and blocks owner
release. Explicit supervisor persistence reconciliation accepts only an already
received invocation and cannot call the model or resume the Recipe. Successful
persistence releases the full native response in memory; unresolved data remains
owned. Bounds are the invocation's acknowledged data_max_bytes, with no new
fixed response quota or extra invocation approval. The shared neutral admission
transport assembly remains the actual claim/journal/lifecycle implementation used
by existing standalone callers; it does not install a model handler there.

The selected model host path retains a received native response before its final
delivery fence. Cancellation may therefore preserve the original late answer in
the journal and a correlated private withheld receipt, while forbidding delivery
to Monty, a reply, a new effect or another model call. Root model ports retain
their existing restrictions; no raw-body/control JSON route is added.

Validation history: R1 failed to compile a new test fixture that used
ResourceCeiling::default despite that type having no Default. R2 compiled but its
new fixture failed strict recursive association-schema parsing (missing explicit
checks), before any provider call. Those checks were added rather than weakening
the validator. A duplicate R2 launch stopped before Cargo because its output
directory already existed; its prelaunch refresh makes that snapshot pathname
unsuitable as executable-source evidence. This limitation is preserved with the
failed artifacts. R3 used a fresh unique source snapshot and refreshed owning
input timestamps, with an issued-once launch marker. It compiled and stopped
before provider I/O because the fixture lacked the mandatory model-route
resolver. R4 supplies the exact operator-selected route through a managed
single-route resolver, preserving production validation. It reached the fourth
actual response's native persistence barrier after the normal, output-ceiling,
database-rejection, reconciliation and policy checks. Its cancellation assertion
incorrectly expected the barrier deadline to return an error: the existing API
returns a receipt containing pending-call evidence. R5 checks that the bounded
receipt is unsettled with pending calls and no abandoned calls, then verifies
late persistence and withheld delivery. R5 has a separate immutable snapshot and
issued-once marker. Its actual live-model case passed (16.048 seconds), together
with the ordinary global Monty regression (46.295 seconds) and five task-host
contracts. Strict Composition lint then found the same admission fixture loaded
as two modules. R6 gives the root tests one shared fixture declaration and
updates the standalone Q1 harness to declare that same fixture once. These are
exactly four test-only declaration/import edits; production inputs, shared
admission implementation and behavioral test bodies are unchanged. R5's passing
behavior is retained with that explicit reuse scope. R6's strict lint found an
obsolete parent import formerly needed by the nested fixture. R7 removes that
import and retains the same behavioral reuse scope. Its affected strict lints
and actual standalone journal/Q1 checks remain pending; none of the preceding
failed batches is recorded as a full acceptance pass.

R7's strict Composition lint passed (38.317 seconds). Its standalone journal
case then exposed V168's foreign-key effect on the old bare-TRUNCATE assertion:
PostgreSQL rejects that statement with 0A000 before the immutability trigger.
R8 checks that exact dependency rejection separately, then requires 23514 for
TRUNCATE CASCADE and verifies all three original intents remain. It changes only
that standalone test; the R5 behavior and R7 root lint retain their original
source provenance. R8 completed the standalone journal (21.425 seconds), all ten
Q1 tests (73.266 seconds) and strict standalone lint (30.315 seconds). No warnings
were suppressed and no database protection was weakened. The scoped accepted
receipt is `/private/tmp/brassclaw-native-model-usage-accepted.json`; immutable
sources and retained executables are under the matching R5/R7/R8 directories in
`target/validation/`. R5 owns the actual live/global/host behavioral evidence;
R7 owns root strict lint and R8 owns the repaired standalone caller checks.
Their declaration-only/test-only source differences and reuse are recorded
explicitly. All twenty owning current-source inputs match R8. Concurrent prefix
and other dependency work retains its own acceptance scope.

The live native case uses a real PostgreSQL accepted run/claim and persisted
route, actual Reborn host/ref/interceptor and retained kernel, and the explicitly
selected operator model. It is invoked with --ignored, not counted through a
skip. It checks normal full response capture, post-output-cap rejection, deliberate
native-result INSERT rejection and persistence-only reconciliation, strict input,
foreign/replay/current-Tool-block rejection, immutable response evidence, and a
fourth real response blocked at native persistence during durable cancellation
and bounded fencing. Its registry-factory configuration is a technical fixture,
not provider-catalogue/operator-installation acceptance. Full model accounting,
real Monty invocation of the canonical export, exact packaged artifact and
association/bootstrap qualification, coherent activation, complete structured
history, real R09 child admission, recovery, platform/release and installed QA
remain required. Do not mark the complete model/Recipe or full-v3 plan finished.

### 2026-10-19 — Canonical model drafts and the model-tier invariant

Candidate implementation in `brassclaw_skills::model_completion_components`
defines the existing Kohai usage's typed successor, a finite N06 JSON-prompt
usage, and canonical parse/model commands. Resolve the existing Kohai Tool,
ToolSkill, Skill and PythonCode UUIDs before staging successor revisions; the
constructors do not allocate replacement identities or establish provenance.
The parser reuses `builtin.json` with the fixed parse operation and a concrete
three-text-role result contract. Its specialized result profile is a distinct
usage of that existing primitive, not another JSON executor. Each usage declares
its preload exports, recursive inputs/results, exact association, dependencies
and one-attempt stop contract. Loading definitions performs no dispatch.

The model command captures one complete `prompt_json` string, parses it in a
separate binding/executable pair, passes the resulting object as typed data to
the pinned model export, and publishes its complete response JSON through the
existing reply usage. It does not infer answer text, run proposals or hide
normal chat history ownership. The parser's own canonical command separately
renders the validated object and publishes it. Both workflows use one UUID per
component step, actual `knowledge`/`stepnumber` syntax and explicit result edges.
Neither constructor activates a route or bypasses Q1/combination/bootstrap checks.

Binding preparation now rejects `host.kohai_complete`, `builtin.shell` and
`builtin.spawn_subagent` in a deterministic workflow, before any effect or
component approval can be treated as execution authority. The catalogue keeps
the same check. Explicit model workflows require RequiresModel/Tier 1.

The R1 validation batch passed. Its new real PostgreSQL regression stages
one immutable graph, selects exact rooted views, checks parse/model/reply
bindings and hostile text as data, rejects deterministic model classification,
and inspects the actual preload sources/interfaces with the packaged Monty
worker while verifying that inspection creates no invocation intent. It is
structural preparation evidence, not observed model-Recipe execution, trusted
combination approval or production activation.

Evidence: two retained-Tool preparation tests passed (4.817 seconds), seven
retained-input regressions passed (8.959 seconds), strict Engine and Skills lint
passed (19.189/5.686 seconds), and the ordinary global runtime regression and
strict Composition test lint passed (53.885/129.034 seconds). The worker was
rebuilt from the same frozen cohort. Receipt:
`/private/tmp/brassclaw-canonical-model-definition-accepted.json`, with retained
sources/executables under `target/validation/canonical-model-definition-*20261019-r1`.
Four owning current files match that cohort exactly. The Skills module index
retains the same model export but gained the prefix owner's separate
prefix_measurement_components declaration during validation. That module and
its dependency cohort are excluded from R1's acceptance; its code was preserved.
The exact source-applicability check is recorded separately. No component
activation or external approval shortcut follows from these checks.

Close these implementation dependencies before canonical model activation:

1. Replace/migrate the JSON Tool's shared fixed 1 MiB input/output and static
   resource ceilings with the actual acknowledged resource/admission contract.
   Preserve bounded parsing/allocation and kernel enforcement; simply replacing
   the limit with an unlimited allocation is insufficient. The canonical parser
   must accept complete eligible data within the selected adjustable profile,
   including a real regression above the former cap. Its finite three-role
   profile does not implement structured provider Tool-result replay history.
2. Execute the canonical commands through Monty's retained workflow, real
   matching, native JSON/model/reply ports and normal history/completion. Cover
   invalid role/shape before any provider submission, live Tool blocking,
   missing model/prefix, exact retained artifacts/associations, cancellation,
   post-effect persistence failure and recovery without replay. Qualify the
   model's packaged implementation identity in the actual catalogue verifier;
   the constructor's checksum string is not executable-integrity evidence.
   Implement the controlled installation-owned bootstrap without an external
   maintainer or installed-instance human-Q2 gate. Preserve authored Q1/behavior/
   human-Q2 and immutable overrides separately.
3. Qualify the complete configured provider chain and accounting/recovery, not
   only the raw registry-provider fixture used for native custody. In particular,
   `RetryProvider` currently treats unknown request/response failures as retryable
   and the connection default has additional retries. A one-attempt model usage
   must not acquire unrecorded provider resubmissions/failover after an unknown
   outcome. Trace SDK/decorator retries through actual submission and durable
   invocation/count ownership; enforce the exact approved retry/deduplication
   contract and current policy before another submission. Include the protected
   root's model ports/aliases in the live-policy and durable response audit:
   keeping those control ports private does not establish kernel Tool-policy
   uptake or durable outcome retention. The host factory's default model policy
   guard is currently NoOpPolicyGuard; prove actual production wiring rather
   than inferring it from the new canonical Tool's kernel checks. Disabled token budgets
   do not disable this effect fencing. R03 now specifies the actual response
   profile instead of the retired ok/answer wrapper; its concrete extractor,
   structured proposal/history consumers and real R09 child admission remain
   implementation work.

### Acknowledged JSON primitive limits — implementation and acceptance scope (2026-10-19)

The native JSON Tool now captures the existing acknowledged `max_value_depth`,
`max_value_nodes` and `max_value_bytes` profile once per operation. Its separate
shared 1 MiB input/output fallback and five-second static resource ceiling are
removed. Kernel Tool policy and any explicit current Tool ceiling still apply.
The EN/DE settings help names JSON and complete encoded request/result bytes;
there is no second JSON-only settings publisher or cosmetic quota.

A common bounded reader constructs values under depth/node/byte admission,
protects Serde callbacks and keeps partial/temporary trees under iterative
ownership through syntax, resource, duplicate replacement and trailing-data
failures. Component contracts reuse it with their existing strict duplicate-key
rejection and public schema depth profile; the general JSON Tool preserves its
native last-value-wins behavior. Query clones only its selected result and
stringify borrows structured data with a bounded pretty writer. JSON parsing
runs outside the async executor. A resource failure in `validate` remains an
error, never a fabricated `valid=false` syntax result. Literal `"null"` JSON
paths remain literal data instead of being normalized to omitted arguments.
This does not raise the existing whole-transport depth support gate or remove
an authored usage's explicit finite data contract.

Installation qualification itself uses JSON before the live settings owner
exists. The installation owner prepares one private bootstrap snapshot from
exactly the selected startup settings revision, then transitions once to the
acknowledged read-only live view before ingress. Unbound access, a second
bootstrap/binding, revision rollback or conflicting same-revision limits fail.
This is resource selection, not a Tool permission, authored approval or VM.

Scoped local acceptance is recorded in
`/private/tmp/brassclaw-json-acknowledged-accepted.json`, with immutable sources,
executables, logs and the combined receipt under
`target/validation/json-acknowledged-*20261019-r1/r2`. R1 passed the four shared
JSON cases, thirteen typed-contract cases, two JSON operation checks, five
registered JSON cases and retained live-policy case; its native fixture stopped
before startup because it incorrectly used a single-thread Tokio runtime. It
is not a passing full batch. R2 corrects that fixture, the resource-surface
expectation and bootstrap-state allocation. Its actual registered JSON/PG test
passed in 38.597 seconds: complete escaped/Unicode result of 1,440,076 bytes
(request 2,340,121 bytes), live 4 MiB/1 MiB limits, rejected PostgreSQL write with
unchanged persisted/effective limits, later increase, retained live Tool block,
literal null path and unchanged global VM identity. Ordinary global matched/
unmatched workflow passed in 51.869 seconds; the dependency boundary passed in
0.215 seconds. Strict Common/Skills/Host/Composition test-target lint passed in
3.808/6.967/19.585/121.739 seconds with the recorded features and toolchain.
These are check durations, not product performance claims. Unchanged R1 cases
are reused with their exact source/artifact provenance; no redundant rerun is
claimed. EN/DE JavaScript syntax and the diff also pass inspection.

Thirteen current JSON implementation/test/dependency/help inputs match the
accepted R2 snapshot exactly. Concurrent prefix-owner changes to
`global_monty_startup.rs`, `runtime.rs` and `source_inspection.rs` are preserved
and separately recorded in
`/private/tmp/brassclaw-json-acknowledged-current-source-verification.json`.
They bind source inspection to the same bootstrap/live primitive source before
catalogue qualification; R2 does not certify that later integration or the whole
current checkout. Qualify the final shared startup cohort before release.
Canonical model workflow qualification/activation, configured provider
retry/durable fencing, root model policy and release/platform acceptance remain
separate work.

### Bound one-submission model operations — implementation candidate (2026-10-19)

The existing canonical model usage declares stop/max_attempts=1. Its native
adapter now supplies a typed `SingleSubmission` restriction through
LoopModelRequest, authorized prompt materialization, HostManagedModelRequest
and both provider gateways. The loop and provider layers own separate enums
with one explicit conversion. This is infrastructure for the existing usage,
not a new Rust workflow or Tool. Public Monty task-port payloads cannot inject
or weaken the host-only policy. Existing unbound callers retain their configured
mode until their separately required cutover.

Retry, failover and smart routing consume the restriction. A bound operation
uses its primary provider/model once; an unavailable primary remains an error,
never an excuse to substitute another route. Existing health accounting remains
active and its failure counter now saturates instead of overflowing. OAuth
adapters cannot hide a model resend after authentication rejection; normal
preflight authentication remains distinct. Bedrock applies a per-operation SDK
retry override. Shared provider HTTP clients disable protocol-level hidden
retries, and the legacy Codex client uses that same deadline/redirect transport.
A bound Codex request uses its selected model instead of lazy fallback. Codex
HTTP rejections use the private shared classifier rather than leaking upstream
bodies/URLs. Response-cache keys distinguish submission policies, Tool-request
conversion preserves the restriction, and forensic request evidence records it.
EN/DE retry help explains the precedence without claiming that connection
settings authorize a Recipe replay.

Validation is pending in `model-single-submission-20261019-r1`. The new physical
HTTP error regression exercises the real SDK plus retry/routing/failover stack.
The native PostgreSQL candidate now uses the actual configured provider-chain
factory with additional retries, a separate cheap model and a circuit breaker.
Its relay forwards to the operator-selected real model, loses an actual completed
answer, and must observe exactly one submission plus the original durable
attempt/error receipt and refusal to replay that occurrence. Subsequent existing
actual-response, output-validation, persistence-only reconciliation and late
cancellation cases remain. Relay-only observations must never be manufactured
into a received native answer. These checks must actually pass before acceptance.

This does not qualify/activate the canonical commands or prove general retries,
full accounting/recovery, all cloud authentication profiles, protected-root
model policy/durability or the final v3 cutover. Those requirements remain open;
no guard-only Boolean check or Rust-loop fallback substitutes for them.


### Unknown model-spend holds — implementation candidate (2026-10-19)

The resource accountant previously released model reservations on any provider
failure and on cancellation, including after the provider could have completed
work or after a settlement acknowledgement was lost. The existing model usage
still owns one operation; this repair adds no workflow Tool or Rust Recipe loop.
A synchronous host-private dispatch boundary now follows reservation and precedes
entry into the assistant, Sempai-review and system-inference provider work.
The completed-turn review adapter uses the same boundary. Cancellation claims an
unsubmitted reservation before governor I/O, fencing concurrent dispatch. After
possible dispatch, failure/cancellation retains the original hold and returns a
classified accounting error with its opaque reservation reference. Generic
provider errors cannot establish zero spend. Observed successful responses remain
eligible for exact retained reconciliation without resubmission or double charge.

Validation is pending. Focused regressions cover unsubmitted cancellation,
post-dispatch cancellation, duplicate dispatch refusal, unknown accounting commits
and a real file-backed governor reopened while its unresolved hold remains.
This is not restoration of accountant ownership after restart, provider billing
reconciliation or complete durable recovery. Those gates remain open. Existing
real-model/native results from `model-single-submission-20261019-r1` passed, but
that batch stopped at strict Composition lint because a new source-inspection
test module preceded production items. Moving that unchanged module to the end
repairs the lint without suppressing it; the successor snapshot must qualify
that move and the new accountant integration before acceptance.


### Observed model custody before accounting — implementation candidate (2026-10-19)

Native model retention previously began only after the host model port returned
success. A successful provider operation followed by an accounting error could
therefore miss the original native response journal. A new host-private
LoopModelResponseCustodian boundary takes the actual sanitized host response
before post-provider accounting. Accounting still receives the actual success
when the custody write fails; neither error permits another provider operation.
The real Reborn host and hook model wrapper forward this explicit contract.
Unsupported hosts reject custody before provider dispatch, without a legacy
stream_model fallback. The selected native callback is consumed once and only
its successfully retained bytes may return to Monty; task/attempt delivery
fencing remains after custody and accounting.

The R03 pure component candidate adds the finite actual assistant-response
schema and an extract_model_answer export. It preserves optional usage and
reasoning observations, rejects proposals and incomplete/wrong-shaped data, and
returns assistant text as data. A consuming Recipe must still select a separately
bound JSON usage with that profile; no parser binding, approval or activation is
manufactured here. Pending tests cover the actual wire serializer and contained
Monty projection, including a real native model answer. The real-model fixture
now installs a fresh per-build hook dispatcher so response custody crosses the
actual hook wrapper too. Controlled caller tests exercise accounting rejection
after response custody and a real file-write rejection with successful-outcome
accounting. These do not substitute for complete PostgreSQL accounting/recovery,
provider billing reconciliation or restoration of accounting ownership on restart.


Unknown-spend R2 validation passed for its frozen source cohort:
`/private/tmp/brassclaw-model-accounting-accepted.json`, artifacts under
`target/validation/model-accounting-artifacts-20261019-r2`. All nineteen accountant
and nine system-inference tests, all67 neutral host contracts, the strict task-port
input test, gateway/task-host/hooks/milestone contracts, actual configured-model
loss/custody, native global Monty/JSON/bootstrap callers and the independent
PostgreSQL invocation-journal test passed. Strict affected-package, Composition
and standalone test-target lints passed. Unchanged SDK/retry/failover/cache cases
reuse their exact R1 source/dependency/artifact evidence. This does not certify
the later pre-accounting custody/R03 edits or general recovery/activation.

The successor custody candidate now takes ownership synchronously before the
first persistence await, including in PgRecipeModelResponses. Pending observations
survive cancellation before or during that await. Confirmed persistence frees the
held response data while retaining a small receive-once fence; duplicate receive
cannot replace an original observation. Reconciliation persists only the already
owned response and never submits provider work. Validation remains pending for
this successor cohort. Full durable accountant ownership/billing reconciliation,
canonical command qualification/activation and protected-root policy/custody remain
open as specified above.


Synchronous custody/R03 validation passed in the corrected R4 source cohort:
`/private/tmp/brassclaw-model-custody-accepted.json`, artifacts under
`target/validation/model-custody-artifacts-20261019-r4b`. R3 had already passed
all68 neutral host contracts and the gateway/task-host/hooks/milestone targets,
but its native build exposed a missing direct Mutex dependency. R4 uses the
existing standard-library mutex with explicit poison errors, without a new
dependency or warning suppression. Unchanged R3 host evidence is reused after
complete affected source/dependency and retained executable hash verification.
The actual R03 worker projection passed in2.315s, configured real-model/native
custody in25.232s and ordinary global Monty in74.145s. Strict affected-package,
Composition and separate workspace test-target lints passed in33.617/122.210/
40.916s. These are test durations, not product performance claims. R4 does not
claim canonical activation, full restart/accounting recovery or root cutover.

A separate admitted-run regression now targets the remaining combined physical
accounting case. It uses the real configured model, native kernel/response
journal and PostgreSQL governor/accountant, with an explicit free/local pricing
policy. A transparent test observer forwards the real accounting hooks and
captures their actual request/response; it fabricates no admission, response or
settlement. A PostgreSQL trigger rejects settlement after the actual answer has
been durably received. Pending validation must prove the outward failure, exact
stored answer, retained original hold, fenced same-occurrence replay and exact
settlement from those observations after rejection is removed. Reopened governor
state must retain that settlement. Host exchange counts measure the host boundary;
physical SDK resubmission evidence remains the separately recorded R1/R2 scope.
No synthetic reply/completion, new attempt or provider replay is permitted.
This is an in-process original-owner reconciliation case, not restoration of
accountant ownership after restart or verification of an external paid bill.
The remaining fixed model-quote fallback and PostgreSQL deadline are recorded
with their required settings/uptake semantics in runtime-limits-audit.md.


### 2026-10-19 — actual model/accounting integration accepted; resource-store policy in progress

R5 passed the separate admitted-run test through the actual configured model,
immutable native response store and PostgreSQL resource accountant. A rejected
settlement retained both actual answer and original hold; original-owner
reconciliation settled observed usage without another model exchange (8.125 s).
Global startup/ordinary chat integration passed (58.430 s), with the concurrently
implemented prefix bootstrap cohort included. Strict affected full-feature
Skills/Composition lint passed (123.717 s). This does not certify complete
restart recovery, canonical model Recipe activation or the full prefix upgrade.

The next implementation replaces the fixed PostgreSQL governor/gate timeout
with `resource_store_timeout_millis` in the existing WebUI execution settings,
including EN/DE help, upgrade preservation and acknowledged live capture. Both
stores share the actual selected source. One read/CAS-write retains its starting
policy; post-fatal settlement can use the last acknowledged revision without
retaining the global service. A timeout remains an unknown write outcome, never
zero spend or replay permission. Behavioral/migration validation is pending.


### Model usage observation audit — remaining accounting migration

The neutral `ModelWorkOutcome::from_model_call` adapter previously counted
transport chunks as output tokens. Its successor takes the actual optional
provider output-token observation: unknown stays `None`, reported zero stays
`Some(0)`, and repartitioning identical text into chunks does not change usage.
A focused regression covers all three cases; validation is pending. This edit
is separate from the frozen resource-store-policy validation cohort.

Further confirmed work is required in `budget_accountant.rs`: both
`usage_for_response` (absent provider usage) and `usage_for_model_work` currently
settle reservation estimates as spent input/USD. The richer assistant path also
uses chunks as its missing-usage output fallback. Remove this conflation through
an explicit observed/estimated/unknown durable accounting contract; retain actual
received response and original unresolved reservation without provider replay.
The generic system-inference response currently loses provider usage before
its outer guard receives it, so the migration must carry the genuine observation
through that supported boundary as well. Do not make an absent usage object mean
zero spend, manufacture tokens, or declare an estimate to be an actual bill.
Known free-model pricing and disabled token caps do not establish unknown token
usage. Keep original pricing/route/config evidence with each retained quote and
reconciliation; live settings must not reprice an earlier operation. Complete
the actual PostgreSQL/native-owner acceptance, including recovery and reported
versus unknown usage, before claiming this accounting migration complete.


### Packaged task model implementation verification — in progress

The catalogue implementation verifier now recognizes the existing task-bound
`host.kohai_complete` adapter's exact `packaged-task-model/1` /
`TaskModelCapability/1` declaration, alongside the reply adapter. Both require
the exact callable and retained executable checksum, reject extra/mismatched
ABI fields, and rehash the actual retained executable before selection. The
existing stable Tool identity query and trusted exact association/activation
checks still apply. An actual-executable regression is added; validation is
pending after the resource policy batch. This prerequisite does not seed,
approve, activate or qualify a complete canonical model Recipe, nor replace
the protected-root model path. Full matching/IBS/JSON/model/reply/history and
controlled system_seed behavioral qualification remain required.


### Resource-store deadline acceptance — 2026-10-19

The frozen `resource-store-policy-20261019-r1` cohort passed: actual ordinary
runtime/governor/gate policy test (27.025 s), real PostgreSQL migration test
(1.926 s), global matched/unmatched runtime regression (49.998 s), strict
affected HostAPI/Resources/ProductWorkflow/Composition tests lint (87.815 s),
and standalone model/retained-execution/Q1 host compilation and strict lint
(33.896 s). Strict full-field settings replacement also passed (0.307 s).
The delayed write committed exactly once after the client timeout; readback
confirmed that outcome without repeating the mutation. The live setting changed
both actual stores without replacing global Monty. The EN/DE settings modules
and UI field passed JavaScript syntax checks. These results do not include the
later token-observation and task-adapter verifier edits, full restart recovery,
canonical model qualification/activation or complete plan acceptance.


### Configurable model reservation fallback — implementation in progress

`model_quote_output_tokens` replaces the separate fixed 8192-token fallback in
the accountant. The existing WebUI execution settings expose it with EN/DE help
that distinguishes reservation estimates from response limits. The precedence
is explicit work estimate, selected cost row maximum, then this acknowledged
fallback. A configured value never truncates a response or reinstates disabled
token caps. V170 preserves explicit values, fills missing values, advances only
incomplete revisions and checks positive u32 representation.

The production accountant is constructed with an initially unbound host source
that must bind the actual acknowledged watch before ingress. Estimates fail
before reservation while unbound; nested accountants share the same source.
Each accepted hold keeps its original estimate and revision despite later edits.
Standalone/test composers retain the canonical default unless explicitly bound.
The public budget-composition helper remains source-compatible; the ordinary
runtime uses its shared internal variant with the actual live source.

Focused actual global-settings/PostgreSQL quote and migration regressions are
added, pending validation after the model-contract batch. They submit no model
requests and claim no provider usage. Exact selected-model technical maximum
propagation, original pricing/route persistence, explicit unknown/estimated
versus observed settlement and restart accounting ownership remain open.


### Model adapter contract acceptance — 2026-10-19

Frozen `model-contract-20261019-r1` passed the token-observation invariance
regression (0.179 s), actual retained-executable/model-adapter verification
(3.327 s), real authored catalogue activation regression (33.082 s) and strict
Turns/Composition full-feature test lint (148.941 s). Worker reuse was verified
against its complete normal/build/vendor/source graph and emitted binary hash.
These checks preserve the existing reply path and verify the model adapter
implementation prerequisite. They do not activate a canonical model workflow
or cover the later reservation-estimate setting, unknown-usage settlement or
restart recovery.


### Resource status persistence — implementation in progress

`PersistentResourceGovernor::account_snapshot` previously ran every status read
through an unconditional whole-snapshot write/CAS, including unchanged state.
A typed `ResourceSnapshotUpdate` store operation now distinguishes unchanged
inspection from a period rollover requiring persistence. PostgreSQL omits the
write and version increment for an unchanged snapshot; actual mutations retain
the same captured exchange policy, CAS/conflict and unknown-outcome behavior.
Other stores keep their existing transactional behavior through the default
trait method. No new polling, pressure counter, quota or receipt eviction is
introduced.

Period advancement remains durable: only a changed anchor selects the write
path, and a rejected rollover write returns an error rather than a reset ledger.
An actual PostgreSQL regression rejects writes while unchanged reads succeed,
checks unchanged CAS versions, preserves failed release state, verifies a real
expired period requires a successful write and retains active holds, and checks
that an absent status read creates no row. Source review and formatting are
complete; executable acceptance is pending after the quote-policy batch. This
removes redundant writes, with no measured latency or full normalized-ledger
scaling claim. Full-snapshot mutation cost/history growth remains separate.


### Reservation estimate acceptance — 2026-10-19

Frozen `model-quote-20261019-r1` passed all accountant units (0.680 s), strict
full-field settings replacement (0.232 s), actual global-settings/PostgreSQL
quote behavior (27.866 s), V170 migration (1.875 s), ordinary global runtime
(53.289 s), actual configured model/native-answer/PostgreSQL accounting-error
reconciliation (21.416 s), strict affected full-feature tests lint (110.082 s)
and standalone host compilation/strict lint (33.604 s). The actual worker was
rebuilt for the changed neutral settings contract. The frozen source includes
concurrent prefix-candidate changes; global startup integration is covered,
not complete prefix qualification. The later status-write optimization is not
covered by this cohort. Pricing/unknown-usage/restart/canonical activation gates
remain open as documented above.


### PostgreSQL status-read acceptance — 2026-10-19

Frozen `resource-status-read-20261019-r1` passed Resources units/contracts
(0.255/0.430 s), actual PostgreSQL unchanged-read/durable-rollover regression
(4.368 s), live database deadline (25.505 s), preserved live quotes (24.211 s),
ordinary global runtime (51.828 s), real model/native-answer/accounting-error
reconciliation (7.401 s), affected strict test lint (108.467 s) and standalone
host compilation/strict lint (33.652 s). Unchanged reads do not write or advance
the CAS version; failed rollover writes return errors and retain stored state.
This establishes removed writes, not a measured latency improvement or complete
normalized-ledger scaling. Later timed-limit setter changes remain separate.

### Timed resource cap edits — implementation in progress

The shared limit setter previously replaced the period anchor on every edit.
For an active rolling/calendar period, a cap-only change must preserve its
original window and spent/held usage. If the old window already expired, advance
it before installing the new cap, preserving active holds. An explicit period
change retains the existing usage-reset behavior. Legacy `PerInvocation`
metadata/enforcement is deliberately preserved, avoiding an implicit weakening
or reinterpretation of existing limits during this repair.

The setter now follows that sequence. An actual PostgreSQL regression covers
active cap changes, expired ledgers, explicit calendar changes, pending holds
and retained original receipts. Source review and formatting are complete;
focused Resources/default-store and PostgreSQL acceptance is pending. This
repairs kernel accounting semantics, without claiming the legacy money defaults
or gate configuration have acquired a complete live WebUI owner.


### Timed resource cap acceptance — 2026-10-19

Frozen `resource-cap-period-20261019-r1` passed Resources units/contracts
(0.256/0.443 s), actual PostgreSQL active/expired/cross-period cap changes
(4.397 s), unchanged-read/committed-rollover regression (1.849 s), and affected
strict full-feature test lint (108.578 s). Active timed windows keep their
anchor; expired usage advances before a cap edit; explicit calendar changes
retain reset semantics, pending holds and original receipts. Legacy per-call
metadata/enforcement remains unchanged. No Monty worker/provider execution was
needed for these owner-level cases; this cohort makes no successor global or
provider execution claim. The preceding broader accepted integration evidence
retains its stated source scope. Financial defaults, unknown/estimated usage,
original pricing/route ownership, canonical model/root cutover and complete-plan
acceptance remain open.

### Finite model-response parsing and explicit answer consumer (2026-10-19; candidate)

The missing N06 assistant-response usage now reuses the existing JSON Tool with
its exact recursive actual wire profile, separately from prompt parsing and pure
R03 extraction. Its canonical command is `parse assistant model response json=%`
with a `response_json:string` capture and complete JSON rendering/reply. Loading
only defines `parse_assistant_model_response(inputs)`; each invocation parses
once, with no model call, source interpolation, usage defaults or proposal effects.

A separate `answer model prompt json=%` candidate orders prompt parsing, one
selected model operation, complete response parsing, R03 and the existing reply
usage. It retains every binding/input/result edge and shares only the JSON Tool
identity between the two distinct parsing usages. The raw completion command
continues to publish the complete native response JSON. Empty answer text is
valid for parsing/extraction; the existing reply primitive rejects blank
publication, which must never replay the completed model operation or enter
Tier 2. Structured capability proposals require their own approved profile.

New source-level/retention and actual contained global-Recipe regressions are
being validated. The contained consumer uses the actual JSON kernel and durable
PostgreSQL invocation journal, with pure observation rendering rather than a
model/reply substitute. It cannot issue approval records or activate routing.
Canonical model qualification/activation, original pricing/accounting ownership,
restart recovery and the protected root cutover remain open.

### Host-reported system-work metering handoff (2026-10-19; validation pending)

`SystemInferenceResponse` now preserves the host gateway's optional complete
`LoopModelUsage`; old serialized responses without the field still decode as
unknown. `ModelWorkUsage.token_usage` carries all input/output/cache counters,
including reported zero, across assistant and system-work handoffs. The generic
accountant uses those observations with the work's selected model-profile rate
row, plus the actual returned text bytes and elapsed time, instead of always
charging its input/output/USD reservation estimate. The shared pricing helper
also serves assistant-response accounting. A failed governor write retains the
chosen numerical settlement in the original in-flight entry.

This repairs a loss of observations, not the complete accounting migration.
In particular, the older LLM `CompletionResponse`/`ToolCompletionResponse` still
carry four unconditional numeric token fields, and the Reborn gateway constructs
a present usage object from them. Provider parsing can therefore already have
lost the distinction between absent usage and reported zero before this handoff.
That upstream presence/provenance repair needs complete provider/wrapper/cache
coverage and actual missing-meter responses; never infer absence from zero or
change real zero to an estimate. Successful unknown-meter budget debits still
use the legacy estimate without sufficient durable provenance. Pricing/route
snapshots, original reservation-to-native-invocation correlation, crash recovery
and the instance-budget/legacy-gate cutover remain required. No new fallback,
provider retry, generated token count or canonical model activation is added.

Validation adds complete/zero/absent DTO and pricing regressions, an additional
real system-inference call in the existing selected-model fixture, comparison
with its actual upstream usage, and reopened PostgreSQL ledger checks. The
finite-response parser regression is also rerun with the exact expected native
policy rejection: its original Python worker error/ResumeHost command and root
identity must be retained, while schema rejections retain a distinct result-
contract failure. The previous R1 run passed Engine/Skills preparation/lint but
failed the test's incorrect blanket assertion of no worker error on a denied
Tool; it is not accepted as passing native-parser evidence.

The same source review found run-keyed cleanup that could forget a successor
hold after an overlapping old settlement/release. Cleanup now compares the exact
reservation ID atomically; mutation/recovery reacquires and checks that same
owner, and rejects mismatched work before marking it dispatched. This is a
local in-flight fence, not durable model-work/invocation identity or crash
recovery. The focused meter regression also retains a new hold across an old
cleanup and verifies that mismatched work cannot mutate its dispatch state.

System-inference now requests `SingleSubmission` as well. Its former configured
provider policy could retry an uncertain external operation or use a cheap-model
cascade under one recorded work reservation. The actual selected-model fixture
keeps retries and an intentionally unrelated cheap model configured; the system
call must still submit exactly once to the retained model. This does not add a
Recipe retry policy or treat a provider error as no effect.

### Exact work binding after model reservation (2026-10-19; successor pending)

Source review also found an admission handoff race: `reserve_estimate` returned
only unit, and `pre_model_work` then assigned its request to whichever run-keyed
entry existed. An unsubmitted cancellation/release and successor reservation
could intervene. The private reservation helper now returns the exact hold ID;
a single-write binder requires that ID, Reserved state and no previous work
binding before attaching the request. Missing, released, dispatched or successor
holds fail closed without modifying another request. No new model call, retry,
resource grant or durable-recovery identity is introduced.

A focused actual in-memory governor regression covers release before binding,
a same-run successor, immutable request binding and final hold cleanup. This
successor is outside the currently running frozen system-meter-handoff R2
cohort; it needs its own affected-package and actual PostgreSQL admission
acceptance before a current-source claim. The earlier cleanup/meter observations
retain their exact R2 source/artifact scope.

### Accepted scope of the response-parser/system-meter cohort (2026-10-19)

Frozen `system-meter-handoff-20261019-r2` passed the corrected finite parser's
actual global Recipe/JSON kernel/PostgreSQL path (26.324 s), pure R03 (0.018 s),
retained canonical parse/model/answer graph preparation (4.925 s), configured
actual-model/native-custody plus system-meter/reopened-ledger acceptance
(35.010 s), native accounting failure/original-hold reconciliation (12.128 s),
and ordinary matched/unmatched global Monty (50.834 s). Strict affected-package
and standalone Monty-control test lint passed (111.207/27.926 s).

The corrected meter/state regression, system-inference units, compaction and
thread consumers passed (0.414/0.042/0.196/0.283 s). Two unchanged meter contract
regressions and nineteen unchanged accountant cases retain their passing R1
source scope; R1's invalid snake_case test fixture is excluded, corrected and
passed in R2. The earlier parser R1's incorrect native-denial expectation remains
recorded as a failure, superseded by the actual R2 evidence; no warning/error was
suppressed. Source inventories/hashes/modes and regenerated changed inputs were
verified before/after; actual executable artifacts were retained before mandatory
own-target cleanup. Receipt: `/private/tmp/brassclaw-system-meter-handoff-accepted.json`.

This accepts the finite candidate and host-meter transfer/local cleanup fence,
not canonical model approval/activation, the protected root cutover, upstream
absent-meter provenance, complete billing/pricing/route recovery, or whole-plan
completion. The subsequent exact reservation-to-work binder is a distinct
current-source successor and remains outside this R2 evidence until validated.

### Accepted exact reservation/work handoff (2026-10-19)

Frozen `reservation-work-binding-20261019-r1` passes all21 accountant regressions
(0.686 s), actual acknowledged-policy/PostgreSQL reservation consumers (27.204 s),
ordinary matched/unmatched global Monty (51.673 s) and strict affected test lint
(108.696 s). The test build covers LoopSupport and Composition together with the
same full feature profile. The worker is reused from the prior exact unchanged
source inventory; current executables are retained with verified source hashes,
modes and regenerated input timestamps. Receipt:
`/private/tmp/brassclaw-reservation-work-binding-accepted.json`.

This completes the local admission publication fence described above. It does
not add durable model-work identity, original native-invocation correlation,
pricing provenance, upstream absent-meter support, canonical activation or
whole-plan acceptance. The preceding real-model/system-meter and standalone
checks retain their R2 source scope; no successor provider run is claimed here.

### Model operation identity and reservation acknowledgement (2026-10-19, validation pending)

A host-issued, non-nil `ModelWorkId` now follows assistant preparation and
system-inference accounting. The native model usage uses its already-admitted
invocation UUID; the private legacy root codec creates its identity at ingress
and rejects caller-supplied identities. Existing serialized identities survive
round trips. Missing legacy metadata stays unidentified and cannot enter new
accounted dispatch; restoration never invents a replacement identity.

Model budget reservation IDs derive from the logical operation and run with a
versioned namespace, separate from the kernel's native Tool reservation. Closed
governor records prevent a second admission of the same logical operation. A new
operation requires a new identity even when prompt sizes and route are equal.
Admission retains the original request before durable reserve I/O; the explicit
`Reserving` state forbids dispatch until acknowledgement. A successful reserve
cannot resurrect an entry cancelled while the governor was committing. Cleanup,
dispatch and outcome handling check the exact admitted request; delayed cleanup
must not release an equal-sized successor in the same run. Production guards
carry that original request through cancellation.

This is accounting infrastructure, not a new workflow Tool or permission grant.
It does not complete durable restoration of the quote/rate/request owner, native
failure custody, canonical model activation, structured model replay or global
root cutover. Targeted checks must cover the actual governor commit/cancellation
boundary, equal-sized successor isolation, PostgreSQL holds and the native
invocation identity through the real provider path before claiming acceptance.

### Provider meter presence (2026-10-19; source implementation, unvalidated)

`CompletionResponse` and `ToolCompletionResponse` carry an optional complete
`TokenUsage` instead of four unconditional numeric counters. Reasoning,
decorators, the Reborn gateway, trace recording/replay and test instrumentation
preserve absence. The local response cache explicitly reports known zero for
its no-provider-dispatch exchange while retaining its stored original meter.
A missing, partial, inconsistent or unrepresentable provider meter does not
become a reported zero, a wrapped integer or a saturated count. It also does
not discard a received answer or justify another provider submission.

Dedicated NEAR AI, Anthropic OAuth, Copilot, Codex and Gemini adapters check
native observation presence. Gemini stream assembly merges only supplied usage
fields. Bedrock's absent/negative observations remain unknown. Rig construction
selects the wire-usage contract explicitly; its normalized numeric defaults
cannot establish presence. OpenAI's Rig 0.30 raw DTO retains prompt/total but
not completion counts; checked subtraction is permitted only from those two
reported counters under that protocol's documented relation. Direct adapters
retain explicit completion-counter presence. Unknown custom Rig contracts
report unknown usage until their structured contract is declared.

Limits of this batch: it does not normalize provider-specific cache billing
semantics or establish retained price/route provenance. The existing governor
still has a conservative estimated-debit path when usage is unknown; that is
not reported consumption. Its former transport-chunk output count is replaced
by the captured output estimate. Durable known/estimated/unresolved accounting,
partial/raw failure custody and safe recovery remain mandatory subsequent
cutover work. No complete pricing or recovery claim is made here.

The cache semantic distinction is confirmed by primary contracts:
[OpenAI Chat Completions](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/retrieve)
defines prompt tokens and a cached subset; [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
uses separate fresh/read/write input counts. Those meanings must be normalized
explicitly with retained provenance before the pricing cutover; compatible JSON
field names alone do not establish the same billing semantics. Bedrock's
[TokenUsage contract](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_TokenUsage.html)
also defines separate optional cache observations. Live provider acceptance is
limited to the explicitly selected local model; no AWS/OAuth account execution
is manufactured.

Model-operation identity acceptance: `model-work-identity-20261019-r2` passed
its corrected actual-governor cancellation boundary, PostgreSQL same-size
successor/closed-operation checks (28.49 s), ordinary global Monty (53.82 s),
compaction/thread/turn/product consumers, and the real provider accounting
reconciliation with work UUID equal to the admitted native invocation (9.94 s).
Strict affected test lint (108.08 s) and the separate Monty-control workspace
lint (39.59 s) passed. Unchanged work-ID wire tests and 21 accountant cases
retain exact R1 evidence; R1's new fixture queried a nonexistent unrestricted
account snapshot and failed, then R2 explicitly configured that real account.
No warning or runtime condition was suppressed. Receipts and exact sources are
in `/private/tmp/brassclaw-model-work-identity-accepted.json` and
`target/validation/model-work-identity-20261019-r2`. Subsequent provider DTO edits
are a separate, still unvalidated cohort. The shared prefix chat's newer edits
are outside this frozen acceptance. Full durable owner/price restoration,
canonical model activation and whole-plan acceptance remain open.

Provider-meter presence acceptance: `provider-meter-presence-20261019-r3`
passed strict affected test lint, including the root package (87.04 s).
R1's unchanged provider and Reborn checks passed; its Composition build exposed
ten old scalar accesses in provider boot/cache tests, repaired in R2 without
changing their expected known observations. R2 then passed all 22 accountant
cases (0.67 s), trace contribution (5.99 s), actual PostgreSQL cache TTL (3.88 s)
and live role-cache settings (71.05 s), finite native JSON parsing (25.22 s),
ordinary global execution (60.71 s), actual model custody/system usage including
metadata-only transport omission (44.42 s), and original-reservation accounting
reconciliation (10.42 s). R2's strict lint identified one obsolete doc comment
left above the deleted saturation helper; R3 removes only that comment and
reuses the unchanged runtime results. No lint allowance or warning suppression
was added. The receipt `/private/tmp/brassclaw-provider-meter-presence-accepted.json`
links all scopes, sources and retained binaries. This freeze intentionally
excludes the other chat's newer prefix changes; combined final integration
remains required. Cache billing semantics, durable known/estimated/unresolved
cost provenance, raw conversion-failure custody and canonical activation remain
open; these results establish none of those unfinished gates.

Canonical model-answer candidate acceptance (2026-10-20):
`canonical-model-20261020-r2` ran the complete nine-step prompt-JSON -> explicit
model usage -> finite response-JSON -> pure R03 -> existing reply workflow on
that runtime's already-running global Monty service. The actual selected
`cyankiwi/Ornith-1.5-9B-AWQ-INT4` endpoint answered; PostgreSQL retained the
original response, prefix and single invocation attempt, exactly one assistant
message was stored, and the instance root identity stayed unchanged. Existing
installation model Tool/ToolSkill/Skill/PythonCode UUIDs were reused for retained
successors. Quotes, Unicode and source-looking history remained typed input.
Actual caller acceptance passed in 31.24 s and strict Composition test lint in
149.59 s. R1 found three test-construction errors (settings-store accessor,
heartbeat's cursor return, shutdown Arc); R2 repairs those test-only issues.
The receipt `/private/tmp/brassclaw-canonical-model-accepted.json` retains exact
source/artifacts and limitations. This development execution does not create
combination approval, activate a catalogue route, exercise ordinary matching or
certify the managed turn's final settlement. Newer shared prefix work remains
outside this frozen scope. The provider-observation successor currently under
validation changes the custody boundary and has no acceptance claim yet.

Completion-status correction discovered during the provider-observation
successor (2026-10-20; not yet accepted): the real one-token truncation test
returned a successful internal receipt in `provider-observation-20261020-r3`.
Source inspection found that Rig inferred Stop from text instead of its raw
provider `finish_reason`. A direct call to the selected real endpoint reported
`length` with exactly one completion token. Earlier model-answer observations
remain evidence that the selected steps and custody/reply ports ran; they do
not establish faithful natural-completion classification or canonical activation.
The successor reads actual provider-specific terminal fields, preserves unknown
and incomplete states, prevents textual Tool recovery from promoting an
incomplete response, and excludes incomplete replies from completed-response
caching. Related OAuth/Responses adapters now require coherent terminal status;
Tool proposals cannot establish completion by their presence alone. Invalid JSON
arguments retain their original text and prevent executable classification;
Codex no longer removes explicit empty argument values. Cloud-code Gemini no
longer invents STOP before a terminal field arrives. These changes need their
new parser/caller regressions, real endpoint acceptance and affected strict lint
before any completion claim. The native provider-observation table remains
separate from a successful LoopModelResponse, with the original invocation's
captured capacity and immutable provenance. Auxiliary Sempai/system observations,
unparsed HTTP/parser-error custody and durable observed-failure spend/price
reconciliation remain distinct required work.


Provider-observation follow-up under validation (2026-10-20): R5 compiled and
passed adapter/parser/cache checks, but its real one-token candidate still
completed. Source inspection then found Rig 0.30's OpenAI Chat Completions
serializer drops the core request's `max_tokens`. R6 maps that exact transport
to `max_completion_tokens`; real registry/SDK HTTP text, Tool and omitted-limit
requests pass. Its real one-token call now exposes an earlier SDK conversion
failure: an empty-content truncated response produces no adapter DTO. R6 is
not an accepted candidate run; absence of that DTO must not be treated as a
successful answer or replaced with invented content.

The successor adds a separate immutable native wire-observation stage before
OpenAI-compatible Chat Completions SDK parsing, retaining original status and
lossless bytes (base64), correlated with the same admitted invocation/prefix.
The existing captured `max_value_bytes` bounds its encoded envelope; failed
persistence retains the original observation for supervised persistence only.
Cancellation and parallel calls keep separate owners. Full response bodies are
freed after durable retention. No unparsed bytes become executable code or a
public reply. The actual-model candidate now checks completed, one-token,
adapter-evidence write failure and wire-evidence write failure cases, without
replaying any model effect. Other provider protocols/streaming and partial
transport failure custody still need their own implementation and acceptance.

In the same pending validation batch, each model reservation captures its
configured ModelCost once. Assistant, auxiliary and retained reconciliation
use that captured rate and reject an incompatible effective profile, avoiding
repricing an admitted operation through a later table read. This is in-process
quote ownership, not durable rate/version provenance or live provider-price
configuration. The legacy conservative estimated debit when a meter is absent,
observed-failure settlement and crash-restored pricing remain required work.
No source in this follow-up is marked accepted before actual execution/lint.


Provider-observation prerequisite accepted (2026-10-20): the final
`provider-observation-20261020-r10` receipt retains the exact frozen sources,
binaries and predecessor scope references. The real four-case candidate passed
on the existing instance VM (47.64 s), all 23 accountant regressions passed,
and ordinary matched/unmatched global execution passed. Actual native
lost-response/write/cancellation custody passed in 48.86 s, retained original
accounting in 7.68 s, all 60 gateway contracts, the five selected Monty host
contracts and the neutral custody contract passed. Affected/root PostgreSQL +
Bedrock test-target Clippy passed with `-D warnings` (143.27 s). R7/R8 protocol
and candidate results are reused only across verified test-fixture-only deltas;
the receipt identifies their original source scopes.

The previous successful-response fixture's 96-token ceiling had never reached
the SDK wire. An effective 1024-token ceiling also truncated a real response.
Its final natural-completion custody fixture therefore sets no artificial
application output ceiling and still requires an actual upstream `stop`.
The independent one-token truncation candidate remains enforced and passed.
No response, approval, retry or successful output was fabricated to satisfy
these tests. Complete buffered OpenAI-compatible HTTP bodies now survive SDK
conversion errors under the original invocation, independently of an adapter
DTO or a valid LoopModelResponse. Other transports, auxiliary owners, partial
network failures, durable failure-spend/price reconciliation, ordinary canonical
activation and the final combined prefix/platform/release gates remain open.

A separate SDK request-parameter successor is now under validation. Explicit
trusted formats map DeepSeek/OpenRouter output limits, Ollama `num_predict`,
Gemini generation configuration and all six SDK transports' stop sequences.
It preserves omission/unsupported-parameter behavior and introduces no output
cap default. Its new loopback HTTP rejection matrix verifies actual SDK request
bodies; this is not real cloud inference or whole-plan acceptance. Anthropic rejects an absent finite output limit before HTTP; supplying
the selected model's technical maximum remains pending model-contract work. These root-source changes are outside the accepted R10 frozen scope.


SDK request-parameter prerequisite accepted (2026-10-20):
`sdk-request-parameters-20261020-r3` passed all three actual single-submission
HTTP tests, including six registry/SDK transports, and the Rig request/response
contracts. The matrix made 24 calls: 22 original HTTP requests and two explicit
Anthropic required-output-limit preflight errors. No technical maximum was
invented. The existing-VM four-case model candidate passed in 48.48 s; affected
PostgreSQL + Bedrock test-target Clippy passed with `-D warnings` in 136.86 s.
The receipt `/private/tmp/brassclaw-sdk-request-parameters-accepted.json` records
frozen sources, binary hashes and the verified reuse of the unchanged
Composition caller across an LLM-unit-test-only change. These are SDK transport
and development candidate results, not cloud-provider inference, canonical
catalogue activation, durable pricing, combined prefix or whole-plan acceptance.


Subagent consuming-path successor under validation (2026-10-20):
`subagent-prompt-20261020-r1` removes the separate spawn schema/decoder
64 KiB limits and the prompt composer's 64/128 KiB limits. Full direction,
task and handoff use host-only prepared message materialization; the short
LoopSafeSummary contract remains unchanged. The new context-prefix prompt
method retains normal context loading, cursor/history policy, identity,
instructions, memory, safety, surface validation, hooks and the existing
single-use prompt authority. Complete-message Recipe callers retain their
existing explicit-message behavior. The child's capability view is intersected
on all three prompt entry points. The initial persisted child input preserves
its literal task/handoff rather than rewriting data through the status sanitizer.

Focused regressions include exact large Unicode/whitespace/code data, real
PostgreSQL goals plus 142 eligible transcript messages with token budgets
disabled, hook materialization and authority reuse rejection. These checks and
ordinary global Monty/affected strict lint are pending, not accepted results.
This is a host infrastructure prerequisite; it does not activate R09 or repair
its missing contextual child admission, inherited immutable selection and
durable cleanup/recovery. Existing technical transport/runtime constraints and
the separate gate/tombstone issues still require their own consuming-path work.


Subagent retained-bookkeeping successor prepared (2026-10-21, not yet validated):
The separate root changes in `subagent/gate_resolution.rs` remove the fixed
4096-state admission ceiling, reserve changed maps/vectors before logical
insertion, reject a conflicting replay of the same gate/child identity and
release backing tables after explicit full drain. The cached count is exact
bookkeeping, not a resource/admission setting. Existing result delivery and
explicit gate deletion remain in the completion observer.

`tombstone_store.rs` removes oldest-record eviction, retains immutable child
outcomes, rejects conflicting writes and exposes exact expected-record deletion
with idempotent absence. Draining releases its map allocation; poisoned storage
still fails. This store currently has no ordinary runtime consumer: tests and
readiness metadata are not durable cancellation/discard integration. The new
method explicitly does not establish durable acknowledgement by itself. R09
recovery and retained rollback outcomes remain required work. The two-file
changes are outside the currently running `subagent-prompt-20261020-r2` scope.


Future-direction reconciliation (2026-10-21): `path_to_v3.5.md` has been read
in full following the updated goal. Its ongoing-plan handoffs preserve the v3
owners and acceptance gates; it does not authorize a second loader, admission
service, universal ledger, model-workflow Tool or Rust business loop. Current
subagent changes repair host-only data materialization and retained bookkeeping
in their existing owners. The legacy subagent composer keeps its existing
framing for compatibility; R09's selected PythonCode/Recipe must own message
construction and sequencing at its actual cutover. The explicit prepared-message
path remains available without automatic legacy transcript/goal assembly.

General pure-component approval and durable dynamic effect occurrences remain
distinct shared gates (`V35-03/04`, `V35-13/14`); existing closed bootstrap or
finite one-call evidence cannot stand in for them. Native request/response
custody and actual settlement remain native responsibilities; business proposal
interpretation, repeated candidate sequencing and completion belong to pinned
components. No prefix ownership or pending v3.5 work is claimed by this change.
The ongoing v3 plans and their installed/platform/release acceptance stay open.


Subagent boundary/retention successor R4 under validation (2026-10-21):
Actual R3 PostgreSQL boot/history and global-VM regressions passed, but its
architecture library target executed zero tests and was explicitly rejected.
The actual architecture dependency target then found two pre-existing violations:
a lower runtime ID in the prepared host-message contract and a test listener
inside product source. The R4 successor uses a distinct `LoopPromptMessageSetId`
with explicit native UUID correlation and moves only the real loopback fixture
lifecycle into `tests/common`. Architecture rules remain unchanged.

R4 combines these repairs with the prepared gate/tombstone retention changes
and the exact first-child-message regression, then rebuilds affected host and
prepared-message contracts, actual architecture targets, native PG/global/model
callers and strict affected lint. No R3/R4 whole-scope acceptance is claimed yet;
failed predecessors remain attributed. The retained-bookkeeping-only launcher
is superseded by this combined scope and must not be run redundantly.


Subagent host/boundary/retention prerequisite accepted (2026-10-21):
`subagent-prompt-20261021-r4` passed both unchanged architecture targets
(28 dependency + 4 composition checks), all 48 Subagent host adapter tests,
106 Subagent store/observer tests, 12 prompt-hook tests, four host-prompt tests,
the instruction bundle regression, all three complete-message contracts and
the actual host preparation/fencing contract. Real PostgreSQL seeded/integrity
boot and complete-goal/142-history-message cases passed (7.25 s). Ordinary
global Monty passed (53.31 s), the real four-case canonical model candidate
passed (40.82 s), actual original native response custody passed (46.71 s),
and original-hold accounting recovery passed (13.67 s). Affected/root
PostgreSQL + Bedrock test-target Clippy passed with `-D warnings` (121.20 s).
The receipt `/private/tmp/brassclaw-subagent-prompt-accepted.json` pins the exact
frozen inputs, binaries and unchanged worker. Other-chat prefix/vendor changes
remain outside this source scope. No new source change followed the freeze.

The former task/handoff/composer caps and short-summary misuse are repaired;
complete input data is retained literally. In-process gates/tombstones no longer
use 4096-entry ceilings/eviction, and exact owner/conflict/deletion/drain rules
are observed. The distinct host message-set ID preserves native UUID correlation
without carrying effect authority. The actual test HTTP listener lives in the
existing test-support directory. Architecture rules were not weakened. R1's
wrong test-only import, R2's missing production Direction boot, the R3 zero-test
architecture invocation and its two actual boundary failures remain failed
historical evidence. R4 rebuilt and executed all affected checks after repair.

This accepts host infrastructure and retained in-process bookkeeping, not R09
child admission/activation, inherited immutable child selections, durable
rollback/cancellation/discard recovery, a general pure-approval schema or
dynamic effect occurrences. The tombstone store has no ordinary runtime
consumer; exact deletion alone is not durable acknowledgement. The actual model
candidate remains a development InternalRecipe. Full v3, combined prefix,
platform, release and installed-function acceptance remain open.


### Claim-fenced child admission prerequisite (2026-10-21; r2 accepted)

The existing Turn coordinator and aggregate stores now expose a separate trusted
`submit_claimed_child_run` / `submit_claimed_child_turn` entry. It retains the
original parent runner/lease proof and checks exact scope, run, turn, Running
status and unexpired non-null lease inside the existing state transition,
before cached replay and after asynchronous profile preparation. PostgreSQL CAS
retries retain that same supplied proof. Unsupported stores return Unavailable;
they never call the unfenced compatibility entry. Lifecycle publication, prepared
identity checks, descendant reservation, queued wake and deferred publication
errors retain their existing owners/order. Claims grant no Tool permission.

Frozen `claimed-child-20261021-r2` passes both actual unchanged architecture
targets (28 dependency / 4 composition), all 86 Turns library tests, all 145
coordinator contracts and six real filesystem contracts. New cases cover stale
identity before and after cached replay, cancellation during profile preparation,
expired parent claims and reopen/exact replay. The actual PostgreSQL test drives
real thread input, coordinator, lifecycle wrapper, aggregate admission, reopened
store, lineage and cancellation (4.39 s including launcher). Ordinary global
Monty passes (52.60 s); affected/root PostgreSQL + Bedrock test Clippy passes with
`-D warnings` (120.38 s). Receipt:
`/private/tmp/brassclaw-claimed-child-accepted.json`. Exact frozen inputs and
preserved binaries are retained; the unchanged worker hash is checked.

The unused status-string helper, its dead-code allowance and its sole `heck`
dependency were removed. Filesystem test fixtures now own TempDir cleanup through
the whole test instead of leaking it. R1 did not compile: its frozen lock selected
the peer's local onig patch while its frozen manifests still selected registry
onig. R2 projects the preceding accepted graph with only the unused Turns edge
removed, and preserves the separate current-root prefix module. No peer source
or dependency selection was reverted. Final combined-root acceptance remains.

This closes the trusted coordinator/store prerequisite, not R09's actual native
Tool dispatch, durable child-admission provenance, fresh child bindings against
inherited selections, explicit Recipe wait/results or unresolved cleanup/recovery.
The legacy child submission API remains compatibility-only and is not made safe
by this new entry. No bundled approval, child execution or whole-plan acceptance
is inferred. The separate pending host-fence ceiling repair is outside this
accepted source snapshot.


### Remove the second host-call ceiling (2026-10-21; r3 accepted)

The per-attempt Monty cancellation barrier no longer imposes a separate fixed
64-call limit. The existing acknowledged global `max_pending_host_calls` remains
the service admission owner, with its existing WebUI control and EN/DE help.
Call-address representation exhaustion remains an explicit technical error.
The barrier retains every started call and late answer; it does not grant Tool
permission or reserve a second capacity pool. No pressure measurement was added.

Frozen `fence-capacity-20261021-r3` passes the actual unchanged architecture checks
(28 dependency / 4 composition), all three barrier units and all six actual Reborn
Monty-host contracts. The new caller regression admits 65 independently prepared
calls, observes 65 pending at fencing, then checks every original withheld answer,
unique identity, settled acknowledgement and refusal of further dispatch. It uses
the existing recording gateway; it does not claim 65 actual provider calls. Prompt
bundles retain their single-use authority: each is consumed before issuing the
next bundle, while the already admitted waits overlap.

Ordinary global Monty passes (57.28 s), actual real-model native answer custody
passes (37.47 s), and affected/root PostgreSQL + Bedrock test Clippy passes with
`-D warnings` (120.39 s). Receipt:
`/private/tmp/brassclaw-fence-capacity-accepted.json`; exact frozen inputs and
preserved artifacts remain in the corresponding validation directories. The
architecture binaries were reused after exact source/dependency applicability
checks and executed against the new checkout; the production consumers were
rebuilt. Worker bytes are unchanged and checked.

R1's wrong test crate name and R2's incorrectly pre-issued prompt grants remain
failed historical fixtures. Neither prompt authority nor architecture rules were
weakened. Only the corrected R3 scope is accepted. The new child actor/CAS safety
repair is a later unaccepted scope; R09 native admission/provenance, inherited
selection, explicit waits/results, durable recovery, full-plan/platform/combined
prefix/release acceptance remain open.


### Claimed child actor and backend CAS safety (2026-10-21; r1 accepted)

The new claimed child entry now verifies the actual stored parent's actor before
admission and cached replay, including after async preparation. This preserves
the original tree's cancellation identity; it adds no Tool grant or feature-role
check. The existing malformed mixed-actor compatibility-tree test established
why this matters. The legacy unfenced submission contract is unchanged.

Filesystem claimed admission now requires the requested backend version CAS.
It never substitutes an unconditional write after Unsupported. Its positive
reopen/replay fixture uses actual CAS-capable InMemoryBackend; a separate real
LocalFilesystem negative case proves refusal leaves the complete persisted
snapshot unchanged. The older byte-only compatibility operations remain explicitly
process-local and are not production authority evidence. This supersedes the
previous positive LocalFilesystem claim fixture for the stricter contract.

Frozen `claimed-child-safety-20261021-r1` passes actual architecture checks
(28 dependency / 4 composition), all 86 Turns library tests, 145 coordinator
contracts, seven filesystem contracts, actual PostgreSQL actor/claim/reopen/tree
cancellation (4.47 s), ordinary global Monty (54.55 s), and affected/root
PostgreSQL + Bedrock test Clippy with `-D warnings` (119.57 s). Receipt:
`/private/tmp/brassclaw-claimed-child-safety-accepted.json`. Exact sources and
preserved binaries scope the result; peer changes remain excluded and untouched.
No model path changed; real-model custody retains its separate earlier scope.
R09 native dispatch/provenance, inherited child selection/waits and durable
uncertain cleanup, plus full-plan/combined-root/platform/release acceptance,
remain required. No whole-plan completion is claimed.


### Fresh bindings against a captured catalogue (2026-10-21; r1 accepted)

The existing effect-free task-binding block now lives in
`task_catalogue_binding.rs`. Normal capture still owns matching, exact approval
checks and the active-generation transaction. The constructor receives that
captured view and creates new host/admission-bound policies, native runtimes,
answer maps and model-response owners, sharing immutable definitions only.
It reads no latest catalogue and copies no other task's ports. A normalized
source comparison verified the moved body was identical except passing the owned
query explicitly. No child admission, provenance, activation or new Tool API was
added by this refactor.

Frozen `task-binding-20261021-r1` passes actual architecture checks (28 dependency /
4 composition), the existing actual operator/PostgreSQL activation and withdrawal-
during-capture regression (35.75 s), ordinary global Monty (53.82 s), and strict
Composition/root PostgreSQL + Bedrock test Clippy (118.13 s). Receipt:
`/private/tmp/brassclaw-task-binding-accepted.json`. The pending ordinary task
still executes its original captured graph after withdrawal with its original
normal-match generation. Peer prefix changes were preserved in root and explicitly
excluded from the frozen source projection. Final combined acceptance remains.

The later retained-integrity/fresh-active verification split and its extended
withdrawal test are a separate scope under validation. R09's native child effect
and durable selection/cancellation/result handoff remain open; this constructor
cannot establish that missing provenance or promote general pure approval.


### Retained catalogue integrity versus fresh eligibility (2026-10-21; r1 accepted)

`verify_retained_selection` now retains every former exact stored-byte/checksum,
revision and approval check while leaving active-generation eligibility to
`verify_source_selection`. Fresh capture/source projection still calls both.
The new method grants no approval, permission, restoration or child provenance.
It accepts an already selected owner-held view, not arbitrary source/flags, and
keeps the existing coherent read-only transaction contract.

Frozen `retained-catalogue-20261021-r1` passes actual architecture checks (28 + 4),
the extended real operator/PostgreSQL withdrawal-during-capture test (35.92 s),
ordinary global Monty (53.51 s), and strict Composition/root PostgreSQL + Bedrock
test Clippy (118.92 s). In one actual repeatable-read snapshot after withdrawal,
the old exact graph/approvals pass retained verification and fail fresh eligibility;
the pending ordinary task then executes its original captured workflow through
the same VM. Receipt: `/private/tmp/brassclaw-retained-catalogue-accepted.json`.
Peer measurement qualification remains intact in root and excluded with its other
peer sources in the explicit frozen projection. No architecture rule or approval
requirement was weakened. Native child admission/provenance/entry, durable task
catalogue pins and full-plan/combined-prefix/platform/release acceptance remain open.

### 2026-10-21 — Ordinary catalogue source pins and retrieval failure integrity (r1 locally accepted)

Ordinary task bindings now carry the exact captured catalogue UUID and SHA-256
of its immutable manifest. The existing admission start transaction pins that
pair before the first host operation, including No-Match; it adds no second
workflow ledger or database round trip. V174 preserves legacy receipts with an
immutable unknown-source format and permits a new reference only during the
original reserved-to-started transition. Internal protected workflows retain
their separate program subjects; absence never implies latest, approval or child
provenance. The checksum is computed once when constructing task bindings.

Frozen catalogue-pin r2 passed the real previous-schema upgrade (5.28 s), actual
operator withdrawal during capture (32.82 s), ordinary matched/unmatched global
Monty (52.15 s), architecture 28 + 4 and strict affected root/Composition lint
(122.55 s). Its independent Monty workspace stopped before compilation because
its lock still retained the removed Turns-to-heck edge. That edge is now removed
without changing package versions or the peer's local onig graph. R1's earlier
upgrade-test i32/u32 compile error remains recorded; the actual refinery API is
u32 and the corrected test has passed. These partial results are not combined
acceptance or full-plan completion.

The subsequent source review found the older PostgresSource matching consumer
still converted matching failures or disappeared Recipes into broad retrieval,
IBS failures into empty successful SplitResults, and dependency failures into
missing context. Those branches now propagate errors. Missing/ambiguous variant
metadata, missing or filtered declared components, malformed/missing dependency
records and unavailable cache state also fail explicitly. Only real No-Match
enters the broad scan. This repairs compatibility-path error semantics; it does
not qualify its older source-substitution/cache model as the final immutable
v3 execution path. That path remains subject to the coordinated legacy removal.

The matcher's fixed three-candidate SQL limit is removed. Ranking, class order,
workflow deduplication and score-spread eligibility remain; disambiguation returns
every eligible distinct workflow. The existing acknowledged catalogue admission
policy remains the ordinary-path capacity owner. New real PostgreSQL tests cover
six distinct workflows with aliases and ranking, plus the actual retrieval facade's
matching/metadata/IBS/component/dependency failures and complete ambiguity. Existing
six Phase-J dependency cases now use the shared native PostgreSQL fixture instead
of silently returning when Docker is absent. Their fixtures use actual column
parameter types and mandatory registry triggers rather than redundant manual
registry inserts. Traversal records its root as already present, so a cycle back
to that owner does not return a duplicate owner in the dependency results.

Frozen `retrieval-integrity-20261021-r1` is locally accepted: architecture 28 + 4,
21 Engine intent units, all six actual PostgreSQL retained-instruction cases
(10.98 s), actual prior-schema upgrade (4.06 s), all four actual retrieval facade
cases (7.77 s), six native Phase-J dependency cases (12.79 s), operator withdrawal
(33.26 s), ordinary global Monty (52.44 s), strict Engine/Composition/root
PostgreSQL + Bedrock test lint (99.38 s), independently locked Monty test builds,
actual PostgreSQL uncertain-intent/late-answer journal (17.61 s), and strict
independent test lint (38.33 s). Full input hashes, inventory, modes, artifacts,
commands, logs and scope are retained in
`/private/tmp/brassclaw-retrieval-integrity-accepted.json` and its named snapshot.
Peer prefix/measurement/local-onig changes remain intact in root and excluded
from this explicit projection; final combined acceptance remains required.
No native child operation,
new approval, Tool grant, recovery replay, general pure-component qualification,
combined-prefix/platform/release/installation acceptance follows from these changes.

### 2026-10-21 — Registered file-read range overflow (r1 locally accepted)

The existing `read_file` primitive now computes its end line with saturating
addition before restricting it to EOF. A valid large explicit limit with a
nonzero starting line previously could overflow `usize` and panic; no cap,
encoding or output format was changed. A new registered capability/kernel/
LocalFilesystem regression verifies the exact Unicode tail for offset 2 plus
`usize::MAX`, and an empty EOF result when both offset and limit are maximal.

Frozen `read-range-20261021-r1` passes all 17 registered coding contracts,
architecture 28 + 4, ordinary global Monty with the new native application image
(56.23 s), and strict first-party/host/Composition/root PostgreSQL + Bedrock test
lint (123.08 s). The unchanged actual worker dependency closure and architecture
inputs were checked before reusing their artifacts. Receipt:
`/private/tmp/brassclaw-read-range-accepted.json`. This is no claim of live coding
limit controls, bounded file-growth protection or faithful raw-document snapshots;
those remain separate work. Full-plan, combined-prefix, platform and release
acceptance remain open.


### 2026-10-21 — Exact retained preparation policy (r2 locally accepted)

Restoring historical authored format 2/3 now preserves the original policy JSON
instead of serializing newly defaulted fields into its immutable bytes. The
private restoration helper verifies that the retained document decodes to the
actual preparation policy; activation still emits a complete current policy.
Original checksum, exact-byte, UUID, approval and active-generation checks remain.
The existing real operator/routing/restart test covers omitted later settings in
formats 3 and 2, full format 2 and format 1, followed by actual matched tasks that
pin each original historical source ID/hash. This does not supply a binary-upgrade
mapping for incompatible installation bases.

Frozen `retained-policy-20261021-r2` passes architecture 28 + 4, extended historical
restoration (58.39 s), authored withdrawal (32.44 s), ordinary global Monty
(52.33 s), strict Composition/root PostgreSQL + Bedrock test lint (122.15 s).
Receipt: `/private/tmp/brassclaw-retained-policy-accepted.json`. R1 failed in
validation-script setup before Cargo; that failure is retained. Full-plan,
combined-prefix, platform and release acceptance remain open.

### 2026-10-21 — Coding resource controls and patch safety (r3 locally accepted)

The patch primitive must bound the actual encoded result before allocating the
replacement or writing, including BOM and restored line endings. Its checked
segment accounting must preserve exact/fuzzy match order and avoid retaining a
per-match span vector. An actual registered/kernel/LocalFilesystem regression
must reject a 4 KiB to 32 MiB replacement without mutation, then permit a normal
replacement. Exact UTF-8/UTF-16 and newline boundary cases qualify the accounting.
This safety-only source is frozen separately as `patch-expansion-20261021-r1`;
it is locally accepted: architecture 28 + 4, three text cases (0.21 s),
all eighteen registered coding cases (1.47 s), ordinary global Monty with the
new native image (55.67 s), and strict first-party/host/Composition/root
PostgreSQL + Bedrock test lint (123.46 s). Receipt:
`/private/tmp/brassclaw-patch-expansion-accepted.json`. This safety-only receipt
does not cover the following root-only resource-control work.

Further root-only work adds live coding read/write/patch bytes, read-window
lines, directory entries/default depth, glob results, grep page/output/scan
bytes, visited entries and diff input/preview/context values through the existing
acknowledged execution policy. Keep each original default and explain each
setting in EN/DE; zero explicitly disables default read/grep pagination. Remove
coding's shadow shared input/output and manifest ceilings while preserving
current kernel Tool-policy and task/transport constraints. Capture one confirmed
profile before each operation; a concurrent edit must not change its admitted
limits. Actual bounded filesystem reads must prevent stat-to-read growth from
escaping the captured read/scan/patch bounds. Migration must fill missing keys
without replacing explicit overrides, bump only changed revisions, and preserve
single evaluation of the prior column default. Historical retained policy bytes
must remain unchanged. These settings, migration, bounded reads and UI wiring are excluded from the
safety-only snapshot. They are locally accepted in the separate frozen
`coding-policy-20261021-r3` projection: architecture 28 + 4, eleven coding internals,
nineteen registered coding contracts, twenty-one existing coding-family contracts,
four manifest cases, retained current-kernel-policy checks, native PostgreSQL
upgrade (4.47 s), actual WebUI-facade → durable settings → acknowledged resource
source → retained native Tool calls (28.80 s), historical exact-policy restoration
(56.40 s), ordinary global Monty/new worker (54.48 s), and strict affected
first-party/host/Composition/root PostgreSQL + Bedrock test lint (124.75 s).
Receipt: `/private/tmp/brassclaw-coding-policy-accepted.json`. R1's accidental
memory-only test-list expansion and R2's incorrect test virtual mount are recorded
as failures; R3 corrects the tests without changing Tool permissions. The actual
new worker and unchanged primitive inputs were verified before artifact reuse.
Peer prefix/local-onig and translation changes remain preserved in root and
excluded from this explicit projection. Actual browser, merged-source, platform,
all coding Skill/Recipe activation and release acceptance remain open. Additional
shared-API/independent-workspace contract checks are queued serially. Directory
enumeration still needs bounded backend forwarding, and complete line reads can
avoid their current per-line temporary vectors; neither is certified by this
receipt. Faithful raw snapshots and remaining primitive/parser/store caps remain
separate open work. The full plan is not complete.


The shared-policy follow-up also passes on that same R3 projection:
`/private/tmp/brassclaw-coding-policy-contract-accepted.json` records both dynamic
full-replacement API cases (0.33 s), independently locked Monty control compilation
and actual worker membership/preflight (0.70 s), and strict independent test lint
(33.45 s). This covers the shared contract expansion without claiming the complete
independent suite or merged peer changes.

### 2026-10-21 — File-result and directory materialization (r3 locally accepted)

Complete line reads now format selected lines directly into one fallibly grown
string while still counting all eligible lines, instead of keeping a full line
reference vector and separate per-line strings. Formatting, ranges, explicit zero,
zero-default pagination, total counts and Unicode/line-ending decoding remain.
A registered full-read regression includes 1,000,001 empty lines and the transition
from six to seven line-number digits, plus an exact tail and explicit zero window.
This is a source-level allocation change, not a measured speed or RSS claim.

Directory walkers use the existing bounded-list operation with the remaining
visited-entry allowance plus one overflow witness; display pagination is not a
hidden walk limit. Existing Resource versus explicitly truncated results remain.
Composite forwards this operation through the same mount. PostgreSQL limits
unique direct children after grouping descendant paths and returns no payloads;
in-memory listing retains only the selected child metadata. Generic third-party
backends retain their compatibility default and are not certified physically
bounded. Core backend contracts exercise Unicode/literal wildcard parents,
several descendants of one direct child, partial/complete/zero windows and mount
forwarding; native PostgreSQL checks retain missing-path/non-directory errors.
No new resource setting, pressure sampling, workflow or authority is introduced.
Receipt `/private/tmp/brassclaw-coding-materialization-accepted.json` accepts
the exact `coding-materialization-20261021-r3` projection. All ten recorded
checks pass: architecture, 64 filesystem unit tests, 26 filesystem contracts,
20 registered coding tests, 21 existing coding-family cases, actual PostgreSQL
bounded enumeration, live settings through retained primitives, ordinary global
Monty and strict affected test-target lint. R2 supplied the actual native/host
execution and rebuilt worker; its strict lint exposed unreachable test-helper
visibility. R3 changes only that visibility and a test name, rebuilds the
filesystem tests and passes strict lint without suppression. The receipt
explicitly reuses R2 execution evidence after exact production-input comparison;
it does not claim a new R3 native execution. Failed R2 diagnostics remain retained.
Peer prefix/local-onig changes remain excluded and preserved. Browser, platform,
raw snapshot, whole-plan and release acceptance remain open.

### 2026-10-21 — Prepared model selection before budget waits (r2 locally accepted)

The existing Recipe/model usage remains one model operation. Its host adapter
now retains the mutable primary role's concrete backend, provider identity and
resolved model during prompt preparation, before policy/accounting awaits.
Dispatch uses that retained selection; subsequent prepared work may select the
new role after a live change. Interceptor, MCP and local result hydration wrap
the retained inner gateway rather than being bypassed. Interceptor iteration
identity remains shared across prepared calls. Sempai captures its own role
before its separate accounting awaits. Retention has no provider I/O or MCP
connection and is not a Tool grant; live input/output guards and kernel policy
remain effective. Immutable/legacy gateway compatibility is unchanged.

The regression uses PostgreSQL-admitted context, actual prompt resolution and
two transparent HTTP paths to the operator-selected real model: prepare A,
switch the live role to B, dispatch A, reject a prepared B call after a live
input-limit reduction without provider I/O, then dispatch newly prepared B.
The exact `model-preparation-20261021-r2` projection now passes compilation,
the real-model/native/global checks and affected strict lint below.
This is a selection-boundary prerequisite, not a completed pricing cutover:
the ordinary runtime's separate boot cost table, qualified pricing provenance,
system-inference preparation and durable backend/price recovery remain open.
No component activation, full-task model freeze, merged prefix, platform or
release acceptance follows.

The first executable batch passed architecture, loop-support contracts, Reborn
unit tests and LLM gateway tests, then exposed the existing review-budget-denial
case: 104/105 host tests passed, but primary accounting classified the rejected
Sempai preflight as possibly submitted Kohai work. The failed receipt/logs remain
in `model-preparation-artifacts-20261021-r2`. The repair adds a Rust-only prepared
submission outcome. Only the interceptor's own pre-primary preparation failure
can establish non-submission; failure categories and serialized model values
cannot. The exact retained work then releases through the real governor, fencing
dispatch first. Unsupported accountants retain conservative behavior; a failed
release retains the original hold and reports accounting failure. Separate Sempai
spend remains separately accounted and is never released by this primary proof.
The existing native PostgreSQL regression now also rejects a release write and
checks that the token hold survives while neither provider is called. Success,
provider-entry failures, cancellation and unknown settlement retain their existing
no-release/no-replay rules. This shared-contract successor passes actual
compilation, native checks and affected consumer/independent-workspace lint
in `/private/tmp/brassclaw-model-preparation-accepted.json`.
All sixteen recorded checks succeed: architecture, neutral/prompt/thread
contracts, Reborn units, all 105 host and 60 LLM gateway tests, hooks, actual
prepared backend/live-limit behavior (25.64 s), real response custody without
replay (34.60 s), ordinary global Monty (53.18 s), strict affected root PostgreSQL
+ Bedrock test lint (123.68 s), and the separately locked invocation journal
(17.56 s) and lint (35.13 s). Exact sources and artifact hashes were verified.
The pre-Cargo Python-launch failure and original host-test failure remain
preserved. The focused ordinary WebUI/in-flight provider-change regression also
passes (27.45 s) using the retained native image, as does explicit neutral Turns
test lint (13.08 s): `/private/tmp/brassclaw-model-preparation-runtime-accepted.json`.
That existing controlled HTTP fixture establishes protocol/selection retention;
the separate actual-model test above supplies upstream inference evidence.
None of these results completes pricing,
system-inference preparation, durable recovery, merged peer or full-plan gates.

### 2026-10-21 — Provider preflight non-submission (r1 locally accepted)

The successor extends the same native proof to the selected provider adapter's
own request preparation: live input/output guards, concrete route/model and replay
identity, message conversion and sequence exhaustion all precede completion
entry. The routed gateway's configuration/pool preparation also precedes its
model entry; the pool contract explicitly forbids submitting a completion.
Provider Tool-definition lookup is the final independently identified pre-entry
failure. Provider futures, response custody, output validation and failures after
entry retain unknown outcome semantics. No error category alone establishes
non-submission. The real-model prepared-call regression now requires the typed
NotSubmitted outcome for its live input-limit rejection while retaining the
same zero-HTTP-call assertion. The successor
`provider-preflight-20261021-r1` passes all eight checks in
`/private/tmp/brassclaw-provider-preflight-accepted.json`: architecture,
all 60 gateway cases, actual-model prepared selection/non-submission (13.96 s),
post-entry response custody/no replay (72.00 s), ordinary live provider changes
(25.10 s), global Monty (50.67 s) and strict affected test lint (103.97 s).
The native configuration covers the actual OpenAI-compatible path; unchanged
root PostgreSQL + Bedrock/shared-contract/independent evidence keeps its prior
separate scope. Artifact hashes and complete frozen inputs were verified.
No pricing, durable backend recovery, system-inference, catalogue activation,
merged peer, platform or full-plan completion is established.

### 2026-10-21 — Accounting representation and configuration (r2 locally accepted)

Source inspection found that finite floating-point defaults outside Decimal's
representation could become unlimited zero caps or a replacement factor of one.
The pure configuration validator now rejects overflow and positive underflow
for all six USD defaults and the estimate factor. The shared accountant builder
validates and returns an explicit error; its runtime and native quote consumers
propagate that result. Zero amounts retain their existing unlimited convention.
Regression source covers each field, valid zero/large values and the public
builder's refusal to create an accountant from unrepresentable defaults.

The accepted database path rejects present malformed numeric
keys and validates the complete merged budget document. Writes serialize absent
and existing keys under one namespace lock in read-committed transactions;
foreign stale-snapshot write transactions reject. The legacy importer takes that
lock before its existing table lock, preserving lock order and a stable complete
view, and validates dry runs without writing. It now retains all ten existing
budget fields instead of silently omitting seven. Native regressions prepare two
actual queued writers, rollback/malformed/deletion cases and full-field import.

Quote and settlement calculations use checked arithmetic. Negative rates,
overflow and positive cache-price underflow cannot become free spend or panic.
The accountant retains original response/meter data before pricing, preserves
the original hold on failure and rejects conflicting replacement observations.
Regression source covers quotation rejection, the full Decimal ceiling and
primary/auxiliary failures across input/output/cache/sum/underflow cases.
Review this coherent candidate and its affected callers before compilation.
No model-price provenance, live WebUI defaults cutover, full-plan or merged peer
acceptance is claimed by these source edits.

The first accounting run passed architecture, configuration and 245/246 loop-support
cases. The new maximum-value test wrongly expected a snapshot for an unconfigured
account after its final hold was released. The governor correctly removes that
empty account. Its successor explicitly configures an unlimited account, checks
the full retained Decimal amount, then requires both a zero balance and removal
of the in-flight reservation. No production change or warning suppression was
needed for that test correction. The failed run remains retained.


The exact `accounting-representation-20261021-r2` projection passes all fourteen
checks in `/private/tmp/brassclaw-accounting-representation-accepted.json`.
Architecture/configuration results are reused only from R1's unchanged relevant
inputs; all 246 loop-support cases pass on the corrected successor. Native
PostgreSQL publication (2.94 s), full-field import (2.00 s), existing import
(2.05 s), live quotes (24.43 s), actual-model custody (69.36 s), global Monty
(50.27 s), affected root PostgreSQL + Bedrock strict lint (124.02 s), and the
separately locked actual journal (18.08 s) and lint (35.07 s) pass. All retained
artifact hashes were verified. These results establish numeric/default/store
behavior, not provider billing provenance, live legacy-budget WebUI cutover,
component activation, merged peer, platform, release or full-plan acceptance.

### 2026-10-21 — Aggregate resource representation (r3 locally accepted)

Following the numeric-default repair through the governor revealed saturated
USD/integer accumulation and saturating subtraction. Two independent holds could
therefore lose a contribution when one was released. The accepted successor preflights
all cascade balances with checked arithmetic, including exact USD contribution
retention, before publishing reservation/settlement/release changes. Combined
spend/holds remain representable even without policy caps; disabled token budgets
continue to disable enforcement while retaining counters. A failed settlement
keeps its original active reservation and cannot partially update an ancestor.

Persisted snapshots also validate duplicate identities, retained reservation
identity/estimate/cascade and original active balances. Old saturated or damaged
holds cannot be silently erased by a later release. Zero rows, supported v1
migration and old closed records without actual receipts remain compatible.
Unrecoverable historical spend is not guessed or manufactured. The candidate
adds one shared contract across actual in-memory, JSON, scoped-filesystem and
native PostgreSQL governors, all eight reservation dimensions and seven measured
settlement dimensions, plus malformed real-file retention and full integer-cap
regressions. The serial successor checks now pass for the exact scope below.
No new Tool, workflow loop, policy grant, fixed operational cap, component
activation or full-plan acceptance follows from this change.

The first aggregate-ledger run passes all 45 resource unit tests and 61/63
contracts. One old test explicitly required the saturation now being replaced;
its successor requires rejection, no rejected reservation record, and exact
release of the original hold. The new scoped-filesystem fixture also used an
invalid virtual root; it now uses the same canonical tenant/user mount shape as
the existing filesystem contract. Both failures remain preserved. No production
code changes or warning suppression follow from these test corrections.

The R2 successor passes all 63 resource contracts and the affected accountant
consumer cases, and rebuilds the real worker. Its new native test then fails to
compile because three Monty settings types were imported from host_api rather
than their owning resources crate. Source exports are now verified and the test
import corrected; production inputs remain unchanged. The failed build/logs and
already passing artifacts remain retained for the successor's scoped reuse.

The exact `resource-ledger-20261021-r3` projection passes all fourteen checks in
`/private/tmp/brassclaw-resource-ledger-accepted.json`; eleven retained artifact
references, including the rebuilt worker, have verified hashes. R1's 45 resource unit cases and unchanged architecture checks, and R2's
63 contracts/accountant cases are reused
only under the recorded exact relevant-input equivalence. Actual R3 PostgreSQL
aggregate checks (4.67 s), status (1.98 s), cap edits (2.02 s), resource-store
policy (25.43 s), real-model custody (50.73 s), global Monty (50.66 s), strict
affected root PostgreSQL + Bedrock test lint (124.13 s), and the separately
locked actual journal (18.35 s) and lint (35.03 s) pass. Failed R1/R2 runs remain
preserved. Historical lost spend is not reconstructed. No pricing provenance,
catalogue activation, merged peer, platform, release or full-plan completion
is established by this aggregate-ledger acceptance.

### 2026-10-21 — Prepared internal model selection (r2 locally accepted)

The ordinary guarded system-inference adapter previously selected a mutable role
only inside its dispatch, after policy and accounting waits. Its new synchronous
non-I/O retention contract captures the concrete gateway first. The selected
port binds the complete original typed request and rejects a different task,
prompt, input or limits. The guard retains its policy/accountant around that
selection; execution still uses SingleSubmission and current live model limits.
Detached settlement after caller cancellation retains its existing behavior.

The existing real-model/native prepared regression now also pauses the actual
guarded system caller through a transparent policy barrier, switches the role,
resumes real PostgreSQL governor accounting and requires the original HTTP route
and actual returned usage. It also rejects repurposing and a changed live input
limit without provider I/O. No model response, meter, governor success or policy
result is manufactured by the barrier. The exact successor now passes compilation and execution as recorded below. This is selection support;
qualified pricing, actual system-response custody after validation, native
non-submission reconciliation and the unguarded failure-explanation path remain
open. No catalogue activation, merged peer or full-plan claim follows.

The first internal-preparation run passes architecture, all neutral/loop-support
units and compaction contracts. The new native test fails to compile because
its provider u32 counts were compared directly with u64 ledger counts. Its two
comparisons now use the same lossless u64::from conversion as the accountant.
The complete diagnostics are retained; production inputs and the already passing
shared tests remain unchanged for scoped successor reuse.

The exact `system-preparation-20261021-r2` projection passes all eleven checks in
`/private/tmp/brassclaw-system-preparation-accepted.json`; ten retained artifact
references have verified hashes. The unchanged R1 architecture, neutral,
loop-support and compaction results are reused only after the two native-test
widening conversions are isolated. R2 passes the actual primary/internal model
selection regression (16.02 s), retained-response/no-replay regression (44.84 s),
global Monty (52.51 s), affected root PostgreSQL + Bedrock strict test lint
(125.79 s), and the separately locked actual journal (18.24 s) and lint
(35.32 s). The exact previous worker is reused only after checking its complete
normal/build/target dependency input closure and unchanged locks/manifests.
Neither neutral adapter belongs to that worker closure. This acceptance retains
the explicit pricing, system-custody/reconciliation, explanation-path, authored
catalogue, merged peer, platform, release and full-plan limitations above.


### 2026-10-21 — PostgreSQL structured-entry boundary (locally accepted)

Before adding the missing R10 raw acquisition profile, source inspection found
that PostgreSQL stat always returned sensitive=false for structured records,
unlike the in-memory backend. Existing generic coding primitives rely on this
flag to avoid reading or overwriting application records. The candidate's exact
metadata query projects record-kind presence and returns the existing FileStat
without discarding that classification. Full and bounded directory checks keep
their existing semantics through that same projection. No payload is added to
the stat result and no new domain workflow, Tool, role grant or policy cap is
introduced. The crate guide's obsolete libsql check is replaced with its actual
feature configurations and the authoritative development-policy link.

A prepared native regression uses actual first-party native registrations,
retained artifact verification, instance-wide Tool policy and the real kernel on
both in-memory and PostgreSQL filesystems. An opaque file reads normally; after
a trusted record conversion, read/write/patch and direct grep must fail
authorization. Directory, glob and tree grep omit the protected entry; all
operations leave both body and version unchanged. A trusted CAS conversion back to opaque bytes
restores ordinary reads. Actual native acceptance is recorded below. This
repairs classification, not the R10 complete raw snapshot, atomic acquisition,
local hostile-directory hardening or catalogue activation requirements.

The first native run exposed a second concrete issue before record protection:
ordinary writes unconditionally called legacy create_dir_all even for an
already existing parent. The entry-plane in-memory backend intentionally does
not implement that operation. The coding adapter now accepts an existing,
non-sensitive directory after stat, rejects a sensitive or file-shaped parent,
and retains ancestor checks and directory creation for a missing parent. The
same actual kernel regression exercises ordinary write and patch before record
conversion on both backends. R1/R2 validation failures remain recorded. This is
not atomic filesystem acquisition or local race hardening.

Frozen entry-sensitivity-20261021-r2 with R3 validation passed all 14 recorded
checks. The actual retained six-Tool/kernel regression passed on both real
backends (7.429 s), bounded PostgreSQL directories (2.261 s), live coding settings
(28.320 s), ordinary global Monty (53.046 s), strict affected PostgreSQL/Bedrock
test lint (125.426 s), and the separately locked actual journal (17.982 s) and
lint (37.031 s). Coding units/host contracts pass. The unchanged filesystem
passes are explicitly reused from R2 validation; architecture checks run on the
successor. Receipt /private/tmp/brassclaw-entry-sensitivity-accepted.json and all
ten retained artifact/worker references were read and SHA-256 verified. The
worker reuses verified bytes: actual feature resolution excludes the changed
PostgreSQL module, and its other changed dependency input is unembedded prose;
all other local dependency/compiler inputs remain identical. No full-plan, raw
snapshot, atomic acquisition, platform or merged-peer acceptance is claimed.

### 2026-10-21 — Internal model non-submission reconciliation (locally accepted)

Internal system inference previously discarded the native provider non-entry
proof while mapping errors. A guarded call therefore retained its reservation
even when a local input preflight or the selected native gateway proved that
no provider request had begun. The candidate adds a Rust-only submission result
to the existing SystemInferencePort, using the existing ModelSubmission enum;
its default is Unknown. There is no new wire/model-controlled flag.

The native adapter proves NotSubmitted only for its pre-entry checks or the
existing private native non-submission marker. Timeouts, provider failures,
invalid returned capability calls and worker loss remain Unknown. The guarded
adapter retains its selected backend before waits, forwards the exact proof
and invokes the existing accountant reconciliation for the exact original work
identity. Unsupported accountants retain conservative failure settlement. A
failed release acknowledgement stays an accounting error and retains the hold;
caller cancellation does not abort the owned settlement task. Neither proof
nor reconciliation dispatches another provider call or refunds separately
accounted Sempai work.

The actual selected-provider/PostgreSQL regression is extended with two fresh
internal task identities: a live native input-limit rejection and a local input
preflight rejection after accounting admission. Both must preserve prior actual
spend, release their real holds and leave upstream HTTP counts unchanged.
Native acceptance is recorded below. Actual system-response custody,
qualified/durable pricing, the explanation-path cutover, full task pinning and
the remaining catalogue/platform/full-plan requirements are still open.

Frozen system-nonsubmission-20261021-r1 passed all 11 recorded checks: all
shared Turns/LoopSupport units and compaction contracts, actual selected-model
and PostgreSQL non-submission cases (22.395 s), actual model custody/reconciliation
(62.079 s), ordinary global Monty (50.909 s), strict affected PostgreSQL/Bedrock
test lint (126.908 s), and the separately locked actual journal (18.169 s) and
lint (35.706 s). Receipt /private/tmp/brassclaw-system-nonsubmission-accepted.json
and all ten retained artifact/worker references were read and SHA-256 verified.
The unchanged worker was reused after checking its complete local dependency
closure and all manifests/locks. The native regression retains actual provider
responses and explicitly uses ZeroCostTable; this supplies no billing provenance
or complete system-response custody claim. Remaining plan requirements stay open.

### 2026-10-21 — Owned provider model identities (locally accepted)

The swappable provider interned model names using Box::leak and a process-global
map. Its fixed 1,024-name/256-byte limits replaced legitimate identities with
<model-name-overflow>, so repeated configuration changes could falsify metadata
while retaining old allocations for the process lifetime. This is an ownership
problem, not a useful operator quota.

The candidate changes the private-workspace LlmProvider metadata interface to
Cow<str>: immutable providers borrow, decorators forward that ownership, and
the mutable swappable wrapper returns an owned snapshot. ProviderSnapshot keeps
an Arc<str>; replacing a snapshot releases the old allocation when no owner
needs it. The global interner, both limits and sentinel substitution are removed.
All implementations and caller uses were inspected, including dedicated
providers, reliability/cache/recording wrappers, test support and Reborn callers.
Display tracing keeps the same text; no model request, retry, Tool permission,
route/model identifier grammar or serialized DTO is changed. The distinct
Engine/embedding traits retain their existing contracts.

One metadata lifecycle regression replaces the obsolete interner-cap tests. It
performs 2,048 actual swappable-snapshot replacements, alternates ordinary and
long names, verifies exact names/retained selections and checks that weak
references to replaced name allocations expire. It invokes no provider and
simulates no completion. Actual provider/kernel/runtime callers and strict
affected compilation pass as recorded below. Existing
pricing, catalogue, platform, merged-peer and full-plan limitations remain.

Frozen model-name-owned-20261021-r1 passed all 11 recorded checks, including
all 812 provider units, all 60 gateway contracts, the actual primary/system
selected-model case (21.133 s), actual PostgreSQL response custody/reconciliation
(41.662 s), ordinary live-role runtime selection (25.336 s), ordinary global
Monty (50.916 s), strict affected PostgreSQL/Bedrock test lint (126.443 s), and
the separately locked actual journal (18.308 s) and lint (35.134 s). Receipt
/private/tmp/brassclaw-model-name-owned-accepted.json and all ten retained
artifact/worker references were read and SHA-256 verified. The worker is reused
after full local dependency/manifests/locks equivalence; these provider sources
are outside its closure. No warning was suppressed. Remaining pricing, system
custody, catalogue, merged-peer, platform and full-plan requirements stay open.

### 2026-10-21 — Opaque bounded-read race and full ceiling range (locally accepted)

After stat protection, a concurrent trusted conversion could still turn an
opaque file into a structured record before the bounded reader fetched its
body. The shared default discarded entry kind; PostgreSQL projected bytes
without checking kind at all. The candidate keeps the stat preflight and checks
the actual returned entry, while the PostgreSQL statement masks structured
contents and projects/checks kind in that same query. Structured reads use the
unified entry API instead. The old opaque-reader API, recursive result schemas,
Tool bindings and live policy remain unchanged.

PostgreSQL also cast usize ceilings directly to signed i64, turning usize::MAX
negative on 64-bit hosts. Its parameter now uses the equivalent comparison
ceiling for all representable PostgreSQL lengths. This changes no resource
policy and never shortens the returned bytes.

The existing native six-Tool regression is extended on actual in-memory and
PostgreSQL backends. A delegating adapter exercises the shared default reader
through the real kernel: both stat calls capture actual opaque metadata; the
writer then commits a real record conversion before get. That read must fail
authorization without changing the converted body/version. Direct native
backend reads additionally reject records, preserve opaque bytes with
usize::MAX ceilings, and permit an empty file with a zero-byte ceiling. The
barrier returns no invented metadata or effect result. Compile and caller
acceptance passed in R2 as recorded below. Atomic write/patch protection, R10 raw snapshot/fidelity
and local hostile-directory hardening remain separate required work.

The first batch built the worker and passed filesystem units/contracts and
extension units/manifest consumers. Its broader host units exposed one stale
preexisting assertion: the missing component-DB backend already returns the
canonical safe Backend failure, but the test still expected “not wired”. The
successor asserts the exact Backend kind and “component database backend
unavailable” summary, retaining failure instead of weakening it or fabricating
a hash. Production error mapping is unchanged; the failed batch remains recorded.

Frozen opaque-read-20261021-r2 passed all 16 recorded checks, including the
actual kernel/backend race (6.928 s), PostgreSQL bounded directories (2.042 s),
live coding settings (26.707 s), ordinary global Monty (50.953 s), strict
affected PostgreSQL/Bedrock lint (118.942 s), and independently locked journal
(17.368 s) and lint (33.340 s). Receipt
/private/tmp/brassclaw-opaque-read-accepted.json and all retained artifact hashes
were verified. Three existing Docker-dependent host tests remain ignored; no
Docker, merged-peer, platform, atomic-write, raw-fidelity or full-plan acceptance
is claimed.

### 2026-10-21 — WebUI/native execution-limit ranges (locally accepted)

The WebUI scalar parser now uses Rust's actual u32 fields and the u32 IPC frame
limit; the browser polling timer retains its stricter signed limit. HTTP default
timeout/body ceilings are u64 and no longer have an unrelated u32 form ceiling.
All numbers still require exact JavaScript safe-integer representation, and the
server retains platform/clock/body-allocation feasibility checks. Previously
missing catalogue/value-depth u32 guards now reject invalid form values early.
The existing ordinary global-runtime regression adds HTTP defaults above u32
on 64-bit hosts and checks the same persisted/effective revision in the HTTP
cache and Monty without replacing the VM. The exact scalar parser boundary
checks and JavaScript syntax pass. The frozen http-ui-ranges-20261021-r1
ordinary PostgreSQL/global runtime case passes (53.070 s), as does strict
affected test lint (98.126 s); all three results and artifact hashes are
verified in /private/tmp/brassclaw-http-ui-ranges-accepted.json. No rendered
browser, 32-bit execution, merged-peer or full-plan acceptance is implied. No
new cap, setting, pressure sampler or completion claim is introduced.

### 2026-10-21 — Atomic opaque coding writes (locally accepted)

The existing write/patch usages need a native storage guard: a trusted conversion
between preflight and mutation could otherwise be overwritten through Any. One
unified `put_opaque` operation now checks the target kind atomically with native
CAS/namespace mutation. Ordinary trusted `put` conversion remains available.
PostgreSQL predicates and in-memory mutex protection share their existing put
implementation; no second filesystem trait or workflow Tool is introduced.

A declared capability, scoped permission forwarding and composite mount routing
cover the operation. Local files preserve supported Any/Absent behavior and
reject unsupported version CAS before effects. Byte-only memory adapters preserve
prompt safety/schema/event handling and reject unsupported CAS without entering
that path. Coding Tools transfer the bytes to this guard; new write buffering
uses fallible reservation. Native writes create/infer missing parents after the
ordinary-directory preflight instead of a redundant legacy mkdir.

The actual kernel/backend regression covers both write and patch conversions
at the native mutation boundary, all record CAS modes and missing-parent creation
on PostgreSQL and memory, alongside the prior read race. Scope/mount routing,
real memory-repository adapters and existing Local coding contracts are included.
All changed sources were reviewed before compilation. Frozen
opaque-write-20261021-r1 passes all 18 recorded checks: native race (9.746 s),
namespace/CAS (2.953 s), live coding (28.251 s), ordinary global Monty (53.052 s),
strict affected root PostgreSQL/Bedrock lint (181.663 s), independent journal
(18.546 s) and lint (35.769 s), plus shared contracts. Receipt
/private/tmp/brassclaw-opaque-write-accepted.json and all retained artifact hashes
were verified. A validation-harness disk guard interrupted after 15 successes;
the initial log/checkpoint is retained, mandatory own-target cleanup was done,
and only the remaining checks resumed. Three baseline Docker host cases remain
ignored; no product failure or warning was suppressed.
No new numeric cap, setting, pressure sampler or installed-instance approval is
introduced. Full patch content/version CAS, raw snapshot fidelity, legacy byte
migration, hostile local containment, catalogue/platform/full-plan acceptance
remain separate required work.

The native write audit also removes saturating record-version/event-sequence
advancement. `checked_next` returns None at representational exhaustion; memory
put/opaque-put/byte-append/event-append reject before publishing changes. Native
PostgreSQL's signed version overflow aborts and rolls back. Actual maximum-state
regressions check retained body/version/events. This is the storage integer
representation, not a configurable resource quota; no counter can be reused.

### 2026-10-22 — Retained worker claim handoff (locally accepted)

The child-task bridge needs the actual worker claim. Ordinary worker handoff now
retains it as an Arc through the existing global Monty task host, with consistent
scope/actor/message/run/profile/internal-source checks. Tokens stay Rust-only;
they are not Python/model inputs or Tool grants. Existing explicit-attempt
callers carry no full claim rather than reconstructing one from IDs. Durable
child admission must still recheck the original claim and preserve the captured
catalogue, waits, cancellation and complete results; those R09 steps remain open.
The metadata regression checks Arc identity, nine mismatches and absent-claim
behavior without invoking or simulating work. Existing worker fixtures now model
the actual Queued-to-Running claim transition and complete host metadata. Actual
PostgreSQL/global-path and affected caller validation passed in frozen
claimed-handoff-20261022-r1: Turns units (0.238 s), worker contracts (3.004 s),
ordinary global Monty (56.533 s), strict affected root PostgreSQL/Bedrock test
lint (171.959 s), independent journal (18.289 s) and lint (34.888 s). Receipt
/private/tmp/brassclaw-claimed-handoff-accepted.json and all retained artifact
hashes were verified. The worker contract fixtures establish transport/identity;
the native case exercises the actual product worker with PostgreSQL. This is
not child execution, catalogue inheritance, merged-peer or full-plan acceptance.
No numeric cap,
setting, pressure sampler, Rust workflow fallback or activation is added.


### 2026-10-22 — Task-bound native child service (locally accepted prerequisite)

R09 infrastructure now has a Rust-only typed child workflow/input/outcome port
and a narrow retained-native binding constructor for builtin.spawn_subagent.
Binding preserves the original handler, exact image/schema identity, planner,
substrate and supplied live authorizer. Only the matching native capability gets
the task-bound service after the original resolver succeeds. Default services
have no child port. Claims, catalogue provenance and workflow selection cannot
be supplied as model-authored authority fields. Complete result text is distinct
from admission, failed/cancelled/recovery-required outcomes and unknown effects.

This is a service-binding prerequisite, not an activated child profile. The
existing authorization-only builtin remains unchanged; no Recipe may treat its
authorized:true receipt as child success. Actual claimed admission, private
parent/child linkage, original catalogue inheritance, dependent scheduling,
explicit wait/full-result handling, cancellation and bootstrap qualification are
still required before the five subagent Recipes can activate. The new boundary
regressions inspect binding without invoking child work and exercise real native
identity retention and kernel denial, never fabricated successful children.
Frozen child-binding-20261022-r2 passes all eight recorded checks: service
binding (0.456 s), original service resolution (0.024 s), actual native identity/
kernel denial (0.714 s), retained live policy (0.041 s), ordinary PostgreSQL/
global runtime (54.422 s), strict affected root/PG/Bedrock lint (129.185 s),
independent native invocation journal (20.392 s) and its strict lint (38.394 s).
/private/tmp/brassclaw-child-binding-accepted.json and retained executable hashes
were verified. R1 failed only on the new module's wrong LoopMessageRef import;
its diagnostics remain archived. R2 corrects that root export. The marker port
never produces an admitted/completed child. No R09, merged-peer, platform or
full-plan acceptance follows.


### 2026-10-22 — Explicit Monty child Recipe entry (R1 transport accepted; successor pending)

R09 now has a host-only child_recipe transport entry. A trusted catalogue owner
can supply one exact selected Recipe and complete typed inputs together with
fresh child ports and the original dependency parent. The global factory requires
an actual retained catalogue reference for this entry. Monty executes the
selection and the existing reply/history completion path without intent rematching
or a fabricated Match. Missing selection fails before effects and never enters
Tier 2. Ordinary chat and receipt-only internal jobs retain their distinct paths.

No production catalogue provider returns this child capture yet. This protocol
prerequisite neither establishes durable parent/child provenance nor implements
admission, cancellation, inherited catalogue ownership or bootstrap activation.
Those remain mandatory R09 work. The closed native regression uses actual draft
IBS, kernel, PostgreSQL and global Monty; it temporarily makes the actual matcher
table unavailable to expose accidental rematching. It invokes no child scheduler
or model and manufactures no approval or successful child run. Frozen R1 passes
all seven checks: envelope, explicit child transport (4.184 s), existing Recipe
failure (16.980 s), native journal (17.330 s), ordinary PG/global runtime
(53.505 s), strict affected root/PG/Bedrock lint (118.841 s) and independent lint
(33.504 s). /private/tmp/brassclaw-child-entry-r1-accepted.json and all seven
retained executable hashes were verified. The owned-selection/input-custody
successor below is separate pending work, not certified by the R1 result.


### 2026-10-22 — Typed input and native answer custody (frozen projection accepted)

Child selection now moves once into execution. Selected inputs and child request
data use the existing immutable JsonValue owner: host clones share read-only
custody and guarded cleanup, while each VM invocation gets its own mutable values.
Internal/prefix/model consumers receive only mechanical input-owner conversions;
this changes no prefix policy, provider owner or component definition.

General native Tool answers now retain their original private PgMontyInvocation
handle before execution and retain the actual PortAnswer with cheap JsonValue
payload custody. Pending/acknowledged status is distinct. Settlement may only
re-acknowledge those exact received answers (and already received model evidence)
through the original idempotent journal API. It never dispatches a Tool/provider,
resumes Python, selects latest or converts the original failed Recipe to success.
Failed tasks already retained their ports before this change; the repair supplies
the missing generic acknowledgement state and original-key reconciliation path,
and removes the additional serialized full-answer copy.

The actual ordinary approved reply regression rejects native answer COMMIT after
the real reply effect, requires the original owner/intent to remain, removes the
rejection and acknowledges the original receipt after fencing. It must prove one
reply, one invocation/attempt, the exact original answer and a failed task even
after acknowledgement. Frozen R2 passed all functional checks, including actual
native COMMIT rejection (28.539 s), ordinary global runtime (53.905 s), the real
operator-selected model (101.851 s) and affected strict lint (132.183 s). Its
independent lint rejected two unused API references; no warning was suppressed.
R3 uses the actual invocation accessor/type in the shared journal writer and
passes the independent journal (18.896 s), native COMMIT regression (27.862 s),
affected strict lint (126.827 s) and independent strict lint (34.068 s).
Unchanged functional/model evidence is retained explicitly from R2. The receipt
/private/tmp/brassclaw-task-native-custody-r3-accepted.json and all ten retained
artifact hashes were verified. The peer measurement qualifier was absent from
this frozen baseline, so merged-peer acceptance remains required.

### 2026-10-22 — Actual child admission owner (scoped frozen R4 acceptance)

Subsequent source work binds the original native spawn_subagent registration to
the actual turn coordinator and claimed-child admission port before workers.
The explicit workflow/typed-input profile preserves literal data and cannot
return the legacy authorized:true receipt. The existing legacy caller remains
separate pending its coordinated retirement. Delegation checks the original
private admission and immutable Tool journal, derives child IDs from the native
invocation, retains the original address before writes and submits with the
actual parent claim. Explicit capture binds fresh ports against the parent's
retained catalogue, with no new matching or latest lookup. It observes the exact
terminal child run/reply and uses the existing configurable status poll policy.
Native child delegation does not declare an OS-process spawn effect.

Frozen child-owner-20261022-r4 passes seven new checks plus five unchanged
HostRuntime checks. The actual native candidate (28.082 s including fixture
startup) admits a child using the original worker claim, transports hostile
literal inputs, proves the exact child/run/reply relationship and keeps the
original catalogue after a real replacement publication. Invalid child inputs
create no second thread; a stale parent cannot dispatch another child. The
actual waiter is armed while the child is running: lifecycle notification must
complete it inside the test's 20-second bound despite a configured 30-second
polling fallback. Notifications are hints; completion always rereads durable
state. Existing ordinary global runtime (52.793 s), original COMMIT-result
custody (24.637 s), independent entry/journal checks and both strict lint graphs
also pass. /private/tmp/brassclaw-child-owner-r4-accepted.json and all seven
retained artifact hashes were verified. Initial compilation diagnostics (wrong
store trait, then two missing test imports) were corrected without suppression.

This is contained native infrastructure evidence. It does not approve/activate
the draft parent usage or demonstrate the parent Monty export's full qualification.
Cancellation, unknown child-admission acknowledgement, noncircular bootstrap and
all five seeded subagent Recipe migrations still require their own acceptance.
It is not R09, merged-peer, platform or full-plan completion.

### 2026-10-22 — Exact child reservation release (scoped frozen R3 acceptance)

The next repair gives claimed children a persisted optional release marker.
Only actual claimed admission creates the known-pending marker; legacy/root
records preserve absent/unknown state. The exact child UUID is the idempotency
address: marker and one-slot decrement share the same aggregate mutation/CAS.
An unkeyed release may not consume known claimed-child reservations. Pending
claimed children remain protected from terminal-record pruning. Memory, actual
PostgreSQL, filesystem with real CAS, lifecycle and boxed-store consumers are
wired; no byte-only CAS fallback may acknowledge this release. Native child
observation retains the actual result before attempting acknowledgement/release.

Source includes a real PostgreSQL regression that cuts the server's actual
COMMIT response after commit, then reopens the store and repeats the same child
release without consuming its sibling. The filesystem/memory contract checks
reopen and absent legacy evidence, and native child acceptance checks its
committed release marker. Frozen R3 passes ten checks: actual COMMIT-loss
(4.483 s), filesystem/coordinator, claimed-admission, ordinary global/native-child,
independent entry/journal and both strict lint graphs (131.239/36.034 s). The
receipt /private/tmp/brassclaw-child-reservation-accepted.json and all eight
retained artifact hashes were verified. The initial R2 test reached its final zero-count
assertion but incorrectly required an empty serialized reservation array, which
the actual schema omits. R3 decodes the real defaulted snapshot type instead.
Before R09 activation, finish these checks and the cancellation/admission tests.
The unkeyed legacy path for untracked children still needs its coordinated
retirement/reconciliation; the new guard does not certify that legacy recovery.

### 2026-10-22 — Child cancellation, received-result acknowledgement and payload release (scoped R3 acceptance)

The explicit child selector now uses the public reply/incoming workflow set,
matching ordinary routing. The internal history finalizer is not a child goal:
ordinary child execution requires its own correlated reply, and must reject that
helper before admitting a child or appending history. The contained candidate
test supplies the helper's valid recursive inputs to distinguish this guard from
an input-validation failure.

The native owner also exposes evidence-only acknowledgement of terminal child
results it has actually received. Ordinary factory settlement invokes it after
native-answer acknowledgement. It retries the exact child's persisted release
marker, never child admission/execution or Tool delivery. Successful release is
remembered to avoid repeated database writes. The original failed native Tool
remains failed; an unknown child admission is neither observed nor reconciled by
this hook. Failed/recovery-required child effects remain retained independently.

Frozen child-observation-release-20261022-r3 passes nine checks and all five
retained artifact hashes were verified in
/private/tmp/brassclaw-child-observation-release-r3-accepted.json. Its real
native regression (28.065 s) additionally cancels a claimed child
before execution and checks durable acknowledgement, one claim, no Tool effect,
reservation release, live parent and unchanged VM. Another actual child completes
its reply while a PostgreSQL trigger rejects only its reservation release; after
removing that rejection, repeated evidence acknowledgement must release the slot,
retain the original Tool error and prove one child reply/claim/dispatch attempt.
The actual native reply COMMIT regression (24.026 s) additionally uses read-only
weak witnesses to prove that unacknowledged answer bytes remain in host custody
and that confirmed journal acknowledgement releases their redundant Rust payload
while the failed task/immutable invocation stay retained. The dispatcher owns
the exact return value independently; concurrent identical acknowledgements are
idempotent and a second receive cannot replace an acknowledged result. Native
model-wire custody (24.176 s), ordinary global execution (50.765 s), independent
entry/journal/factory-cancellation and both strict lint graphs
(121.401/34.656 s) also pass. Unchanged store/COMMIT-loss evidence is retained
from child-reservation R3. Initial successors caught a missing test import and
an extra-field contract without its required schema; both were corrected, with
their failing diagnostics preserved and no suppression.
No candidate approval or activation is fabricated. Parent Monty-export/bootstrap
qualification, unknown admission acknowledgement, ancestor cancellation across
handoff phases, saturation and all five seeded Recipe migrations remain R09 work.
This is scoped native/ownership evidence, not merged-peer, platform, R09 or
full-plan/release acceptance.

### 2026-10-22 — Cancellation below terminal parents and unknown admission (scoped acceptance)

Explicit tree cancellation now traverses descendants even when the requested
parent is already terminal. Completed/failed parent and intermediate states stay
unchanged; scope/actor checks, atomic rollback and idempotent event identities
remain. A parent failure does not prove that child admission did not commit.

Frozen unknown-child-admission-20261022-r1 passes ten checks with all eight
retained artifact hashes verified in
/private/tmp/brassclaw-unknown-child-admission-r1-accepted.json. The actual
PostgreSQL child-admission COMMIT response is cut (4.358 s); the same child,
accepted message, lineage and reservation remain after the caller error. Parent
failure fences a repeated old-claim submit, authorized cancellation reaches its
queued child, and exact repeated release neither creates a replacement identity
nor changes the parent's failure. Shared Turns/filesystem/coordinator contracts,
the native child/ordinary global caller, independent journal and strict affected/
independent lint (125.475/35.065 s) also pass.

This is coordinator/database transport acceptance. It is not a full Native
ChildPort admission-fault/recovery caller, automatic message-linkage repair,
parent component activation or supervised restart. Those R09 gates remain.

### 2026-10-22 — Preloaded child usage through Monty behavioral review (scoped acceptance)

The existing native child regression now executes its first staged Python export
through the actual source inspector, retained behavioral runner and SelectedTools
adapter, with the original parent claim/admission. It checks the actual journal
answer/one attempt without manually acknowledging that occurrence. The child
still executes in the instance's original global service with the old catalogue
and complete literal inputs/results. The contained review worker is reaped;
this neither starts another production orchestrator nor approves/activates the
draft parent. The remaining four candidate occurrences are not declared complete.
Review task/frame/actor/heap/deadline limits come from acknowledged instance
settings and actual worker layout, rather than new fixture policy caps.

Frozen child-preloaded-export-20261022-r1 passes this real regression (29.153 s)
and affected test lint (116.212 s). Its receipt
/private/tmp/brassclaw-child-preloaded-export-r1-accepted.json and both artifact
hashes were verified. Only test/review wiring changed; predecessor production
evidence is reused explicitly. Full ordinary global parent Recipe qualification,
noncircular installation bootstrap and the five seed migrations remain open.

### 2026-10-22 — Exact transcript submission acknowledgement (scoped acceptance)

The next repair makes coordinator-supplied message linkage immutable after its
first accepted/deferred User binding. The same turn/run pair can be acknowledged
again; conflicting pairs, Assistant/redacted messages, empty IDs and incoherent
partial bindings fail. PostgreSQL busy deferral receives the same state guards
as memory/filesystem. An exact PostgreSQL acknowledgement skips serialization,
snapshot rewrite and generation advancement. Snapshot increments reject signed
representation exhaustion instead of wrapping; negative versions fail, while
the version-zero bootstrap/legacy format remains supported. PostgreSQL thread
CAS additionally preserves tenant identity and rechecks a raced ensure winner's
scope. This is reference storage, not turn lifecycle or invocation authority.

Frozen `message-submission-20261022-r1` passes the actual memory/filesystem
contracts, real PostgreSQL repeats/conflicts/kinds/redaction/exhaustion,
cross-tenant zero-version and blocked INSERT ensure-race checks. PostgreSQL
redaction now explicitly stores Redacted status and clears both content and
private provider replay data, matching the other backends. Actual native child
and ordinary global Monty regressions and affected strict test lint also pass
(eight checks, five retained executable hashes verified).
Atomic admission/message linkage and observation/recovery of an unknown Native
ChildPort admission remain separate required work; no safe whole-Recipe retry
or automatic success can be inferred from these prerequisites.


### 2026-10-22 — Atomic transcript and admission commit (scoped acceptance)

The PostgreSQL turn store now supports an installation-wired metadata participant
for newly admitted records. The transcript owner binds the exact existing user
message within the coordinator's transaction, after the turn aggregate/aliases
and before COMMIT. It checks canonical tenant/scope/row identity, message kind,
status and exact IDs; rollback covers the run, reservation, alias and message.
Normal skills-db runtime factory wiring installs the participant. No raw input
enters turn state, no Tool executes, and heartbeats/old tasks/idempotent admission
replays do not rebind. The remaining producer-side mark is an exact read-only ACK.

Frozen `atomic-admission-20261022-r1` passes eight actual PostgreSQL admission/
storage tests (18.165 s), including root/child binding before producer ACK,
redacted/foreign input rollback, deferred COMMIT rejection after both writes and
loss of the actual server COMMIT acknowledgement. Native child (26.464 s),
ordinary global Monty (52.557 s) and affected strict test lint (131.353 s) also
pass; two retained executable hashes are verified. Existing uncertain Native ChildPort
operations, parent qualification/activation, restart, merged-peer and full-plan
acceptance remain open; atomic persistence alone does not establish those gates.


### 2026-10-22 — Original-child observation after an unclear admission (scoped acceptance)

The native ChildPort now checks the original run after an admission error, rather
than treating a lost COMMIT response as proof of failed admission. The read-only
check requires the exact parent/child lineage, actor, received time, source/reply
references, retained execution profile and already-submitted input. Only that
committed original child proceeds to ordinary observation; missing, unavailable,
redacted or mismatched evidence remains uncertain with original custody. There
is no re-submission, new run, retry count reset, input repair or effect replay.

The existing five-case native/preloaded child regression is being extended to
route its real PostgreSQL clients through a byte-preserving loopback transport.
It cuts the actual first child admission COMMIT acknowledgement after the server
commits; it invents no database result or coordinator response. Frozen
`child-admission-reconcile-20261022-r1` passes that real native/preloaded child
case (30.814 s), ordinary global regression (52.108 s) and affected strict
test lint (129.368 s). The original child, old catalogue, one invocation and
full reply are verified in the same global service; two executable hashes
are retained and verified.
Unobservable commits, lost thread/message acceptance ACKs, operator reconciliation,
ordinary parent activation/bootstrap and full R09/restart gates remain open.


### 2026-10-22 — Scoped inbound receipt lookup (scoped acceptance)

PostgreSQL replay previously ignored the requested scope and actor and silently
skipped malformed snapshots; filesystem replay scanned with a system scope and
ignored those request fields. The new PostgreSQL reader selects only matching
JSONB receipts in the exact canonical scope, validates snapshot/entry/message
identities and rejects ambiguous or malformed candidates. Filesystem replay
uses the existing full scope-keyed hash path directly, validates its contents
and checks the actor. Memory's existing exact lookup is retained. These are
correlation/isolation checks, not feature or invocation permission grants.

Common actual Memory/ScopedFilesystem/PostgreSQL tests cover actor, agent,
project and owner separation, including identical source/event IDs in two
scopes. PostgreSQL additionally tests corrupted receipt identity and two real
conflicting legacy rows. Frozen `inbound-replay-20261022-r1` passes both complete backend contracts,
nine actual PostgreSQL storage/admission tests (19.456 s), actual lost-COMMIT
native child (27.249 s), ordinary global Monty (52.691 s) and strict affected
test lint (127.992 s). Five retained artifact hashes are verified. Cross-thread acceptance races remain separate required work: current
PG receipts live per thread, and filesystem acceptance overwrites the key after
writing the message. Neither reader fix proves transactional event uniqueness.
Those writers must be repaired before claiming full input-ACK recovery.


### 2026-10-22 — Atomic inbound event identity (scoped acceptance)

V176 adds one canonical PostgreSQL receipt per exact scope/source/event tuple,
with a checked SQL SHA-256 key, retained raw identity fields and thread FK. It
validates and backfills the existing receipt/message graph without choosing a
winner for conflicting legacy rows. Temporary indexed migration data is dropped
at commit; malformed/ambiguous graphs abort migration. Only receipt metadata
is indexed for legacy reads. PostgreSQL accepts a message and its receipt in
one transaction, validates exact duplicate actor/text/reply and preserves the
original IDs. Read-only duplicate ACKs do not rewrite the snapshot. Deletion
removes canonical receipts with the scoped thread deletion in one transaction.

Filesystem acceptance claims the original message ID with real atomic Absent
before writing the canonical message. A pending/missing message stays unresolved;
no Any overwrite or non-atomic fallback can replace the claim. Memory/filesystem
now reject changed duplicate text/reply, aligning with the PostgreSQL contract.
Actual message counters use checked increments; representation exhaustion is an
explicit error and not a new resource quota. Safe database error classifications
retain SQLSTATE without exposing private server diagnostics.

Common real-backend caller tests cover concurrent cross-thread event identity,
exact duplicate ACK, conflicting actor/text/reply, redaction and explicit deletion.
Additional source tests cover pending filesystem claims, counter exhaustion,
actual PostgreSQL lost COMMIT ACK, deferred commit rejection, nullable scope
keys, and real refinery upgrade/rollback for corrupt and ambiguous old graphs.
Frozen `inbound-atomic-20261022-r2` passes 40 Thread unit tests, both complete
backend contracts, 15 actual PostgreSQL storage/admission/upgrade checks, native
lost-COMMIT child (30.648 s), ordinary global Monty (53.345 s) and strict affected
lint (126.992 s). The first run rejected an invalid virtual path in two new test
fixtures; the correction uses the established tenant/user mount grammar and
retains that failed evidence. Production source is unchanged from the first
warning-free compilation and real PostgreSQL pass. Identical test/worker hashes
permit reusing that PostgreSQL pass. The actual worker was rebuilt with V176;
all 43 retained executable hashes are verified. Compiled extra test targets are
not claimed as executed behavior. General partial-write/operator recovery,
ordinary parent Recipe qualification/activation, restart, merged-peer/platform
and full plan acceptance remain open.


### 2026-10-22 — Original thread/input observation after lost acknowledgements (accepted scoped native checks)

The native ChildPort now observes the original empty scoped thread after an
unclear ensure write, and the original accepted input through its exact scoped
event receipt after an unclear input commit. It checks the actual owner, IDs,
status, unbound references and opaque input data. Only the already committed
original records allow the unfinished handoff to continue; no second ensure,
message acceptance, child admission or completed effect is replayed. Missing,
redacted, partial or mismatched evidence remains uncertain with original custody.

The existing real five-case native child regression now targets lost replies at
three phases: thread ensure, input COMMIT and child admission COMMIT. The
byte-preserving transport uses actual idle ReadyForQuery for autocommit and the
real COMMIT CommandComplete for explicit transactions; it invents no successful
record or response. It must retain the original child, one invocation, old
catalogue and ordinary global VM, including cancellation and reservation-write
failure cases. The actual native child check passes (29.941 s), ordinary global
Monty passes (51.604 s), and strict affected/root PostgreSQL + Bedrock test lint
passes (127.622 s). Both retained executable hashes are verified; the unchanged
worker closure reuses the actual V176-capable worker. This accepts readable
acknowledgement loss during the original call, without resubmission.
Unreadable/delayed outcomes, operator recovery, ordinary parent
qualification/activation and complete R09/restart/full-plan gates remain open.


### 2026-10-22 — Complete global parent review and classified child failure (scoped combined acceptance on 2026-10-23)

Internal Recipe sources can retain their actual captured ordinary catalogue
reference for explicit native child handoffs. The immutable reference participates
in the original admission/invocation checks; it is not an approval, routing
fallback or request-carried permission. Existing standalone internal work keeps
its absent catalogue reference.

The closed native candidate includes the actual optional nullable sanitized
failure object. A new case changes real Tool policy after child capture and
checks failed state, one attempt, no reply and no model replay through the
preloaded export. Failed child evidence remains retained. A complete parent
Recipe is separately submitted to the already-running instance global service
with a real durable claim, its captured catalogue and typed inputs. Python
executes binding/export/result handoff and completion; its child uses the same
VM and old selection after a coherent catalogue publication. The private review
constructor is test-only and supplies no component approval/activation. Source
review and formatting pass. The separate six-case native regression passed
(31.063 s); complete-parent preflight rejected its missing pure preload declaration.
That test Recipe now declares a function/export. Common-root execution also
corrected the fixture's actual runtime tenant/agent/project/thread-owner scope
and its trusted internal admission/canonical result-reference settlement. The
existing production validation and live kernel checks remain effective. The
actual complete-parent check and six-case child failure/acknowledgement regression
pass in the shared global service; no draft parent is activated.

The common-root `run-10` acceptance builds the application/worker and affected
test targets, passes 503 selected tests with none failed/ignored, affected/root
strict test Clippy, formatting and whitespace checks, and verifies every retained
executable hash. It also covers real V176 upgrade/atomicity, rejecting foreign
thread namespaces before writing, native answer no-replay, ordinary global
Match/No-Match, selected prefix/source/model-budget callers and shared backend
contracts. Compiler-generated function-pointer equality and duplicate HTTP test
module loading were repaired without suppressions. Exact selectors, counts,
artifacts and limitations are recorded in
`docs/validation/2026-10-22-consolidation.md`.

One common Git root/index and one serial NVMe Cargo target replace the two mutable
build mirrors/target2. Exact old source/binary/receipt archives remain evidence.
Active plans now assign each shared responsibility one implementation owner and
reference it from consumers, including the v3.5 dependency handoffs. Installation-
owned production qualification, ordinary parent activation, the five historical
subagent Recipe migrations and full R09/restart/plan acceptance remain open.


### 2026-10-23 — Complete raw UTF-8 file acquisition (scoped native acceptance)

The existing registered file reader has a separate `raw_utf8_preserved` profile.
Its complete four-field result preserves actual UTF-8/BOM/newline bytes and
correlates the immutable capture with the native invocation and path/content
BLAKE3 identity. Ranges/forged fields and invalid/incomplete reads are errors.
Existing live read ceilings, kernel policy and filesystem protections remain;
there is no new Rust document workflow or fixed quota.

The actual extension/native caller suites pass 73 cases. Real WebUI/PostgreSQL
limit publication and current-policy blocking pass on a retained handler in the
same global VM; memory/PostgreSQL structured-entry transitions also pass. With
strict affected Clippy, formatting and whitespace this accepts 75 selected tests,
not every compiled target. Exact hashes/commands, reused scope and two corrected
failed attempts are recorded in
[the raw-file validation report](../validation/2026-10-23-raw-file-snapshot.md).
The raw canonical usage, separately approved R10 pure export, persistence/lineage
and doc-conversion migrations remain open. General pure approval belongs to
validator Steps 3/8–10, consumed by V35-03/04; the closed compiler exception is
not expanded. Final embedded documentation validation retains its own snapshot.

Final `r10-raw-source-20261023/run-4` builds the common application/worker with
updated embedded guides/changelog, passes version smoke, actual native live
settings/policy startup, strict root/composition Clippy and format/whitespace.
All 61 retained executable hashes are verified, and the accepted app/worker are
restored to the one canonical target after disposable cleanup. Reused 73 native
cases and the backend race retain their exact earlier scopes. Fifteen additional
actual Monty utility evaluations check the proposed R10 Markdown helper only;
no component approval/activation is inferred. Only unembedded reports/plans are
finalized afterward; the final equivalence receipt identifies those differences.


### 2026-10-23 — Pure review prerequisites and backend test ownership (scoped acceptance)

The common source now exposes exact pure source/dependency observations, every
root export check, strict untrusted pure declarations and original sealed review
subject/selection identities. These are validator Steps 3/8–10 prerequisites, not
durable approval, complete class-Q1, human Q2, seed qualification or activation.
[The contract](../reborn/pure-component-review.md) records the exact remaining
writer/readers and the per-unbound-invocation approval gate; composition R10 and
V35-03/04 reuse that owner instead of creating parallel machinery.

The first batch found existing PostgreSQL test listeners in product composition.
Their complete real protocol transport now belongs to optional PG test support;
domain selectors/assertions stay in the callers. A subsequent literal-IP parsing
defect was repaired before acceptance. No architecture check or warning was
suppressed. Run 3 passed **67 focused tests**, including ten real atomic/ACK-loss
regressions, native child old-selection/same-global-VM execution and both complete
architecture suites; strict affected/root Clippy, fmt and whitespace passed.
The ordinary matched/unmatched global-root pass is separately reused from run 1.
[The scoped report](../validation/2026-10-23-pure-review-foundation.md) records
failed batches, exact commands/artifacts, source applicability and open boundaries.
One common source/index and serial Cargo target remain in use. A checkpoint push
does not close the remaining v3/v3.5 or installed-release acceptance gates.


### 2026-10-23 — Independent pure execution coverage (scoped native acceptance)

Authored preparation now checks each variant's actual unbound executable steps
separately from Tool-usage dependency UUID coverage. A helper cannot inherit
independent execution approval from the same usage or another variant. The only
current independent coverage is the existing exact packaged generation workflow,
verified against its full pinned selection. Retained-selection/MCP verification
repeats this boundary, including child admission's retained-view caller. General
pure evidence/approval/Q2/bootstrap/pins remain open under validator Steps 3/8–10.

The [scoped validation record](../validation/2026-10-23-independent-pure-execution.md)
records **39 passing selected tests** across the exact-source reuse chain, actual
operator/PG/global publication/rejection, waiting-task retention, restart/old
formats, component deployment and child callers. Strict root/composition lint,
format and whitespace checks pass. Initial fixture/required-data/free-space
failures and a reused-log-path receipt correction are preserved explicitly.
The independent HF reference now loads the frozen Qwen3.5 configuration without
a generic-config fallback, using a consistent isolated environment; full native
comparisons pass. The actual oversized Sempai measurement remains capacity-negative
and unactivated, with its diagnostic visible. Prefix files were not changed.

No old invalid dependency-only selection is silently grandfathered or routed
to Tier 2; stored data/pointers remain available for explicit diagnosis. Installed
repair/recovery, general pure qualification and the rest of the full v3/v3.5
acceptance are not implied by this checkpoint.


### 2026-10-23 — Complete retained validator diagnostics under live limits (scoped acceptance)

The retained validator observer now fingerprints each actual result using that
step's captured data limits, replacing its incorrect fixed-default checksum
validation. The unchanged checksum format preserves old valid observations.
The actual Monty regression emits/collects/aggregates 300,000 complete diagnostic
strings, changes limits between steps, then reduces the live profile before
observation. Original per-step bounds remain authoritative; changed data and
separate report overflow reject, and the negative result remains negative. No
successful Q1 or approval is manufactured.

[The scoped report](../validation/2026-10-23-validator-observation-limits.md) records
seven passing actual-worker/PG consumer tests, fresh root/worker/test builds,
strict root/Engine/composition lint, format and whitespace checks, plus exact
source/artifact hashes. It closes this consumer bug, not the eight validator
routes, trusted durable pure evidence/Q2/bootstrap, activation or full-plan
acceptance. Continue those remaining gates under validator Steps 3/8–10.

### 2026-10-23 — original validator subject and actual input custody

The existing private validator observer now has a distinct sealed subject-bound
result. It derives complete subject JSON data from immutable storage, compares
the original stored validator selection and verifies every executed input against
the pinned task/constant/result edge using each step's captured limits. An actual
changed subject can produce exactly the same positive output; that execution is
rejected for the original submission. No unbound fixture or same-class subject
can supply its observation. Borrowed complete input fingerprints preserve the
existing format and shared depth/node/byte accounting without result-tree copies.

[Scoped acceptance](../validation/2026-10-23-validator-subject-inputs.md) passed
29 fresh contract, actual-worker/PG and ordinary-global cases, plus strict
root/Engine/Skills/composition test lint, formatting and whitespace. This does
not establish approved mandatory checks, retained interpreter/run/attempt/settlement
provenance, successful class Q1, general pure approval/Q2/seed qualification or
ordinary activation. Validator Steps 3/8–10 continue to own those remaining gates;
composition and v3.5 consume their shared implementation.

### 2026-10-23 — one retained interpreter image for ordinary global hosting

Ordinary startup now reuses Source-Inspection's existing retained worker image
through catalogue inspection and GlobalMontyOwner's actual service spawn. The
constructor no longer accepts/reopens a mutable installed pathname. The catalogue,
source backend and live owner share the same artifact; the existing singleton
registry retains it through unknown quiescence/dropped quarantine receipts. This
adds no production image capture, manager, VM or resource setting. The existing
acknowledged artifact policy still controls preparation. Sixteen standalone
fixture constructors now consume the same retained-image boundary.

[Scoped acceptance](../validation/2026-10-23-retained-global-worker.md) passes
16 actual-worker/PG/ordinary and standalone lifecycle cases, root and standalone
strict test lint and formatting. Initial strict lint found duplicate fixture
loading; importing the already loaded fixture fixed it without an exemption.
The final source rebuilt and reran all 16 cases; the failed receipt remains.
This closes live hosting identity/custody, not durable interpreter/run/attempt/
settlement records, complete class Q1/pure approval, automatic crash recovery,
platform/installed release acceptance or the remaining implementation plans.
