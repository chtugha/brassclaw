# Isolated Monty control extension

Upstream: https://github.com/pydantic/monty, tag `v1.0.0`, commit
`85c5d1f6bef038405cfc40a4eed94806e303567e` (the isolated upgrade gate's
Cargo.lock). MIT license and upstream notices are retained. Only the four
library crates and their source/manifests are vendored. Original file hashes
are recorded in UPSTREAM-SHA256.json; Cargo.toml is narrowed to these crates.

CONTROL.patch uses zero-context hunks so blank context markers do not create
trailing-whitespace warnings when the patch is tracked. Apply it only to the
verified upstream files, using `git apply --unidiff-zero`.

## Current compatibility and scope

Source reconciliation: 2026-10-15. This document describes interpreter and worker
primitives supporting v3. Application contracts and the full production acceptance
matrix remain governed by the linked guides and plans below.

| Surface | Current identity | Authority |
| --- | --- | --- |
| Vendored extension | `1.0.0-brassclaw.control.7` | [Cargo.toml](Cargo.toml) |
| Interpreter dump ABI | Current and minimum supported version `0xBC05` | [dump_format.rs](crates/monty/src/dump_format.rs) |
| Installation worker transport | Private protocol `19` | [process.rs](../../crates/brassclaw_monty_host/src/process.rs) |

The dump ABI and process protocol are separate contracts. Pair compatible host
and worker artifacts; an older protocol mentioned in a milestone below is not
the current transport contract. Reconcile retained continuations before an
incompatible upgrade; compatibility metadata never authorizes effect replay.

The application dependency graph uses these sources, including the Engine API
migration, contained authoring utilities and global installation worker.
Recorded ordinary-runtime and first-feed acceptance is summarized below. It
does not establish completion of every production lifecycle, catalogue or
adaptive-settings gate.

## Application-owned v3 boundaries

[recipe.md](../../recipe.md), [skills.md](../../skills.md) and
[validator_v3.md](../../validator_v3.md) define the workflow, preload and review
contracts. Coordinate their implementation with
[monty.composition.md](../../docs/plans/monty.composition.md) and
[prefix_v3_upgrade.md](../../docs/plans/prefix_v3_upgrade.md). A compiled source
checksum or a reusable seed establishes neither approval nor catalogue coherence.
The following are binding application responsibilities, not a claim that these
interpreter primitives complete their runtime/store enforcement.

The application must select one activated, approved catalogue snapshot and retain
exact Recipe/variant/step/input, Skill/interface/code/dependency, association
approval and Tool implementation references for each task. It must resolve exports,
reject conflicts and cycles, and load qualified definitions and immutable
constants in deterministic dependency-first order without effects. Invoking the
pinned export with typed inputs is a separate operation. Existing tasks, children,
waits, retries and resumes retain their selected implementations and isolated
mutable state when successors activate.

Matching ToolSkill binding and current kernel policy remain required before each
actual Tool dispatch. Neither bytecode sharing, a cached alias nor successful
compilation enables a Tool. Trusted `system_seed` qualification requires exact
integrity, Q1 and behavioral evidence through the installation owner; installed
authored changes use their separate Q1/behavior/human-Q2 path. The interpreter
does not manufacture that evidence or perform catalogue activation.

`PreparedReplSeed` supports first-feed compiler reuse. It does not implement the
complete Skill export loader, combination approval or library-prefix compiler.
Production child contexts belong to the one supervised global instance service;
this optimization must not introduce a per-task global VM or a second execution
path. MCP commands retain ordinary chat/Recipe execution and its authority checks.

## Task time and interpreter clocks

Execution and preparation observations describe distinct costs. The combined
ExecutionControl clock described in the historical milestones is not itself the
v3 task-duration policy. The binding default is 600 seconds of executing VM time
per task, excluding idle, queue and external waits; it never limits the global
orchestrator's lifetime. Observe and account preparation separately through its
qualified owner, preserving the existing task account and effective settings
revision through children, waits and retries. Do not silently add preparation to
that executing-time budget. See [validator resource rules](../../validator_v3.md#77--preserve-task-budgets-and-distinguish-terminal-engine-failures)
and [simplified v3](../../simplified_v3.md).

## Historical control milestones

The following entries retain the implementation sequence and limitations observed
at each milestone. Their protocol numbers and pending acceptance statements are
historical; use the current compatibility table and recorded acceptance section
for the later state. An earlier proof does not qualify unrelated v3 features.

The atomic accounting updates use `try_update`, available before the declared
Rust 1.96 minimum. This replaces the deprecated `fetch_update` alias while
preserving the checked arithmetic, memory ordering and returned previous value.
It changes neither the control extension version nor the dump/worker ABI.

`control.4` adds `ExecutionObservation`: actual VM-local coroutine identity,
cumulative executing time and cumulative preparation time. Resource checks and
both sides of scheduler switches publish these clocks. Actual task exit closes
the outgoing interval before discarding its identity; mechanical snapshot
cleanup preserves it. New REPL feeds restore main context 0. None denotes
discarded-context service work. The default hook retains the prior combined-clock
API for task-owned children. Preparation is not assigned to the last loaded
coroutine: importing a host result can resume a different worker.

`GlobalVm` partitions execution into a bounded coroutine map and retains actual
pending-call/context correlation. This is telemetry, not an admitted task budget:
persistent workers are reused. At this milestone, admission association,
preparation ownership, task-local interruption and safe finishing required
integration. Control errors remain interpreter-terminal; do not use them to enforce one task's limit
in the global root. Coroutine and host-call identity counters reject exhaustion
before mutation/dispatch or taking heap references. They never wrap; refused calls
release their arguments/pending effects.

The changed serialized context and control error contract requires dump ABI
`0xBC04`. Older continuations must be reconciled before upgrading; do not replay
completed effects. All 228 upstream hashes and the exact patch round trip were
verified against the recorded upstream checkout.

The trusted ExecutionControl hook observes cumulative active execution time
at periodic resource checks and execution-window exits, including completion
and errors. REPL source scanning and compilation have separate preparation
windows, checked on entry and exit and included in the same cumulative control
clock. Owned guards also account for interpreter graph imports, host-call argument
exports and returned-value exports. Nested conversion shares an existing window;
VM reentry pauses preparation, and native sleep pauses both clocks. No account
lock is held across interpreter work or a control callback. Rejected syntax and
compiler-exit cancellation retain their charges.
Successful compiler metadata is restored before returning a control failure;
no opcode executes after that failure. The upstream VM-only feed/turn clocks
and `elapsed()` telemetry retain their meanings; `preparation_elapsed()` exposes
the additional cumulative preparation clock as a Result, with explicit accounting
failure. An active preparation without its live guard cannot be restored as idle
execution. Errors latch and become uncatchable. The control Arc is never serialized;
a controlled snapshot requires trusted reattachment before resuming. Reattachment
does not erase observed terminal failures or time consumption.

ControlYield snapshots before the next opcode, retaining operand stack, locals,
frames and exception state. Resume injects no external result. Synchronous
native reentry cannot yield because its Rust stack is not serializable; it
retains yield requests until returning to snapshot-safe execution. Native
operations can observe cancellation through existing resource checks, but
unpolled native operations remain a bounded-response gap. Synchronous `run`
does not service yields; callers must use iterative execution.

Dump ABI `0xBC04` stores the owned preparation account and deliberately rejects
upstream dumps and previous `0xBC01`/`0xBC02` extensions. It does not reset
preparation on reattachment.
It does not authenticate snapshots; the existing trusted-producer requirement
still applies. Deployment must reconcile old continuations before cutover.

The earlier clock-extension proofs did not establish heap isolation, complete
native-operation preemption, bounded compiler/graph response, snapshot/cleanup
accounting, one-shot construction accounting, production global
hosting or WebUI acknowledgement. Phase 3a acceptance remains required.


The same unreleased `control.4` extension fixes the worker allocator's finite
ceiling arithmetic: baseline-plus-budget overflow is rejected before replacing
an armed limit, and live allocation/refund counters fail closed on overflow or
underflow. `tests/monty_control` installs the actual allocator only in its
isolated `allocator_probe` binary. The real native Monty allocation is stopped
by the previous finite cap even after an invalid replacement was rejected.
This is physical worker-backstop evidence, not shared logical heap/adaptive
accounting or production process supervision. The dump ABI remains `0xBC04`;
these allocator changes do not change serialized interpreter state.


`control.5` adds `ControlYield::raise`, an ordinary catchable exception at the
retained opcode boundary. A trusted task handler must be active, and its external
futures must be settled before interruption. This neither supplies a call result
nor clears a terminal control error. The isolated root host establishes that
scope through its private `enter_task` handshake and refuses further dispatch
after task compute exhaustion. Actual returned host answers displaced by a task
failure are retained with their exact continuation for trusted reconciliation.
Root worker failure handling remains alive; it does not acknowledge effects.
Dump ABI `0xBC05` conservatively fences the changed interruption contract, including
previous `0xBC04` continuations. Private worker protocol 3 carries the retained
answers; they are never ordinary model-visible task state. Production startup,
scoped cancellation and durable reconciliation remain acceptance requirements.


`control.6` adds allocator ownership headers and an explicitly activated VM
allocation domain. The worker enters a synchronous, thread-bound scope for
interpreter construction/execution and controlled root/child adaptation. Each
allocation retains its tag through scope exit, nested scopes, reallocations and
destruction on another thread. Requested storage includes the private header
and alignment padding. Incoming frame decoding and outgoing JSON serialization
remain outside that domain; exported values allocated during VM adaptation keep
their tags until released. Root/child interpreter heaps, compiled source and
retained execution bookkeeping share one counter. This is neither RSS nor
baseline subtraction, and does not include stacks or direct mappings.

The interpreter's effective soft memory ceiling includes the worker's live
shared limit. The separately armed physical allocator backstop also accounts
for transport and remains finite. The instance service requires an initial
logical limit before its real boot handshake; a standalone physical-backstop
probe may deliberately omit the soft preflight. Private worker protocol 4
carries actual domain bytes and desired/effective heap revisions. Manual edits
below live usage are rejected without mutation. Automatic reductions retain the
effective revision and block new admissions until actual reclamation permits
the desired limit. Settings publication neither resets allocation ownership nor
task compute. Expected control-edit denials preserve the global worker; uncertain
transport failures still require containment and reconciliation.

The dump structure is unchanged (`0xBC05`); ownership tags are allocator metadata,
never serialized pointers or checkpoint authority. No old continuation/replay
compatibility is asserted. Real allocation operations and actual root-plus-child
execution are tested in `tests/monty_control`. Adaptive OS measurement, physical
backstop resizing, production settings storage/publication and the complete
global Recipe/catalogue cutover remain required. That milestone still restricted logical growth to the startup physical cap;
the control.7 resize contract below removes this limitation.


`control.7` adds `set_worker_limits` for a serialized, quiescent worker owner.
It validates a finite logical ceiling plus a separate non-VM reserve, rejects
logical or physical reductions below actual allocations, and rejects arithmetic
overflow before publishing either limit. Actual live counters and ownership tags
are never reset, and baseline cannot absorb growing VM consumption. The worker
can grow beyond its startup physical cap and shrink after real reclamation.
Unsafe automatic reductions remain desired/pending and stop new admissions;
manual denials preserve the last acknowledged worker and resource revision.

Dump ABI remains `0xBC05`; this changes allocator publication, not snapshot
layout. Private worker protocol 5 is unchanged. Real allocator and root/child
resize acceptance is required in `tests/monty_control`; this primitive does not
implement host-pressure measurement, adaptive settings migration or WebUI uptake.

The same unreleased `control.7` adds nonmutating `validate_worker_limits`, using
the exact logical/physical validation shared with `set_worker_limits`. It writes
neither ceiling, baseline nor counters. The installation worker's private
protocol 9 exposes a revision-checked candidate-layout observation without
advancing Python, reconciling a pending automatic heap target or changing its
configured frame. This is not a reservation or a durable settings transaction.
The transport owner must retain the quiescent boundary through persistence and
publication. Full live frame uptake and queued-exchange retention were still open
at this milestone; later recorded acceptance covers specific subsets below.
Dump ABI stays `0xBC05`; the process protocol and interpreter dump are separate.

## Current first-feed reuse

`PreparedReplSeed` compiles a first feed against pristine compiler tables.
Compatible empty child contexts copy those tables while sharing immutable module
and function bytecode. Each context supplies its own heap, globals, inputs,
host receiver, execution control and OS environment. Existing contexts reject
seed insertion; later feeds compile against their retained namespace. No task
values or continuation state belong to a seed. Seed loading accounts table
copying and input preparation to the current task before execution.

The worker caches these source-and-alias selections weakly, rechecking actual
source integrity and live source bounds on every lookup. Active child contexts
retain their first seed; closing the contexts permits its reclamation. This is
first-feed reuse, not complete dependency-chain or catalogue-prefix compilation.
Function bytecode's `Arc` serialization preserves the existing Code payload;
neither the dump ABI nor the worker protocol changes.

## Recorded acceptance and remaining limits

These are summaries of recorded evidence, not checks rerun by this documentation
update. The [2026-10-10 first-feed acceptance record](../../docs/plans/simplified-v3-implementation.md#2026-10-10--compatible-first-feed-compiler-sharing)
reports 49 interpreter, 42 host/transport/service and 37 retained PostgreSQL/IBS/
worker checks, strict linting and paired native builds. Its ordinary product
cases exercise authenticated WebUI ingress, embedded PostgreSQL and pinned
reply/history workflows in the same global VM, including waits and live settings
edits. The record also states an integration worker-image retention limitation;
retain that qualification when reusing the evidence.

The [native host-progress record](../../docs/plans/simplified-v3-implementation.md#2026-10-10--host-progress-during-owned-settings-publication)
closes the task/child and queued in-flight continuation subset of the earlier
live-frame gap. These observations supersede the corresponding historical
pending statements, not the full acceptance matrix.

First-feed reuse remains narrower than complete dependency-chain/catalogue
caching, activation-time warmup, complete authored/preload activation, provider
waits, reconciled fatal recovery and platform/performance acceptance. No complete
v3 cutover or speed improvement is established here. Track remaining gates in
[the current implementation plan](../../docs/plans/simplified-v3-implementation.md),
[validator_v3.md](../../validator_v3.md) and the composition/prefix plans above.

### Complete selected definition-prefix reuse (2026-10-11)

The application worker now compiles the complete selected dependency-first
definition set into one `PreparedReplSeed`. Empty contexts share immutable code
and compiler tables while keeping their mutable heap, globals and host binding
isolated. Loading rechecks every exact source and current bounds, preserves
per-definition feed/source charges, rejects effects and symbol conflicts, and
does not grant Tool authority. Weak cache ownership permits reclamation after
the last context closes. This adds private worker protocol 14; pair host and
worker artifacts. Extension `control.7` and dump ABI `0xBC05` remain unchanged.
See the [retained-worker acceptance record](../../docs/plans/simplified-v3-implementation.md#2026-10-11--complete-selected-definition-prefix-compiler-reuse)
for exact evidence and the remaining warmup, product and whole-plan gates.


### Host-selected internal entry envelope (2026-10-13)

Private application/worker wire protocol 16 added acknowledged
global pending host-call capacity, real occupancy observations and safe quiescent
reduction checks to the complete runtime settings transaction. Older workers
are rejected rather than reporting false uptake. Protocol 15 added an explicit
host-owned chat or named internal Recipe entry while retaining legacy chat data
envelopes on the same wire version. Pre-token and VM admission both validate the
complete envelope. Internal selection bypasses public matching; Python still
sequences the retained Recipe and its task owner verifies the durable receipt.
Ordinary chat cannot select this entry by sending JSON-shaped text. This changes
neither the upstream Monty version nor the v3 component architecture.

Version mismatch fails before a worker can become ready. Application and worker
must ship together; merely sharing a pathname is not protocol compatibility.
This is not approval of a protected root, full validator qualification or recovery
Gate A1 acceptance. Historical protocol observations above retain their scope.

### Reusable owning-group coroutine slots (2026-10-15)

The unreleased extension adds `asyncio._replace_sibling(slot, coroutine)`, a
private BrassClaw scheduler operation, with no CPython equivalent. Only a
currently executing gather child can use it. A slot equal to the group's length
adds one new coroutine; a smaller slot must already have completed. Larger
indices, booleans, negative indices, non-coroutines, reused coroutines and
running slots are rejected. The original group waiter and ordinary error path
remain in place. Replacing a completed slot releases its old coroutine and
result; repeated replacement does not append permanent result slots. No task is
detached and no other group's awaitable can be selected through this interface.

The hosting kernel's context, admission, task and memory checks still apply.
This is mechanical scheduling, not a Tool, a permission grant, Recipe execution
or a live product worker-setting implementation. The Python root must own
capacity sequencing; the product still needs coordinated admission publication,
draining reductions and actual VM acknowledgements. Context-account retirement
is covered by the subsequent prerequisite below; product resizing remains open.

Suspended one-shot and REPL owners now release their VM stacks, scheduler,
pending effects and globals on destruction without advancing Python. Resume and
explicit REPL namespace recovery transfer their original state. Internal optional
fields allow that transfer without dummy interpreters or heaps; serialized values
retain their original shape and loading still requires non-null owned state.
At this prerequisite, dump ABI remained `0xBC05` and the private application
protocol remained `16`. The subsequent wire cutover below uses protocol `17`.

The isolated [scheduler gate](../../tests/monty_scheduler/Cargo.toml) enables
reference-count diagnostics and exercises 2,000 replacements, active-slot
rejection, extension/failure propagation, actual host waits, dump/load,
suspended-state destruction and namespace recovery. Its eight new cases and
three existing coroutine-accounting cases pass with strict linting. These are
interpreter prerequisite observations, not acceptance of live product resizing,
fatal recovery or the complete v3 plan.

### Actual coroutine exit accounting (2026-10-15)

`ExecutionControl::context_exited` now observes a spawned coroutine's actual
success or unhandled failure after its final clock interval closes. Suspending,
dumping or destroying a snapshot emits no exit acknowledgement. The tracker
latches callback rejection as an uncatchable control failure; it preserves all
prior failure and cumulative clock state.

The global host retires the exited context's execution, preparation and
adaptation entries into separate cumulative totals. It refuses retirement while
an admitted task still owns that context. A serialized answer import can still
own preparation after the coroutine exits: its single pending retirement is
held through the last sample and performed when that exact scope closes. This
does not acknowledge a task finish, cancel a host effect or release durable
attempt ownership.

The actual `GlobalVm` gate now performs 2,000 owning-group replacements, reaches
the real idle work waits, verifies live-map plus retired-total equality and shuts
down with the same VM identity. An unexpected worker exit retains its unfinished
task budget and fails closed. All 13 global-host cases, 10 scheduler/ownership
cases and three coroutine-accounting cases pass, together with both strict
test-target lints. These observations do not implement a live worker setting or
qualify the paired product worker, recovery or the full v3 plan.

### Observed pool transport (2026-10-15)

Private protocol `17` introduced optional actual worker-pool membership and the
coordinator continuation separately from the immutable boot identity. A pool
root's extra control wait counts toward pending capacity. Receivers validate
membership ordering, unique work-wait keys, coordinator ownership, generation
and pending minimum; old protocol workers are rejected before readiness.
Mechanical resize/drain/coordinator-close commands retain the same root process,
and the retained task registry follows accepted admission targets while keeping
existing task records and capacity debt.

All 17 root-host, 24 actual-process, 27 service and 15 actor cases pass with strict
linting. Process cases cover 20 grow/drain/reuse cycles and complete shutdown;
three unresolved task scopes can be admitted beyond the boot count of two, then
survive reduction and regrowth with their original futures and task records.
These are paired private-worker observations. The shipped root still uses its
static pool until the complete revision-held settings, service and WebUI cutover
is implemented and accepted. No live product worker setting, full recovery or
whole-plan completion is established here. Dump ABI remains `0xBC05`.

Protocol `18` corrects pending/context sizing during regrowth with sparse draining
IDs. The required worker set is the union of target IDs `0..desired` and retained
active IDs, plus the coordinator wait. For example, target IDs0–6 with busy
draining ID7 require eight workers plus one control wait. The host validates that
union before mutation and reports it even before newly spawned workers park.
The paired receiver uses the same contract; protocol17 workers are rejected.
All84 affected root/process/service/actor cases and strict lint pass, including a
real worker retaining ID7, refusing an insufficient eight-call budget unchanged,
then safely regrowing with nine calls. This repairs a prerequisite; complete
settings frame projection, publication and live product sizing remain open.

Protocol `19` adds optional `max_workers` to the complete revision-checked settings
candidate. `None` preserves current capacity; it does not disable a limit.
Preflight streams the prospective membership/continuation arrays through a
bounded byte counter without allocating arrays proportional to the requested
count. The private worker advances only hosting waits/control yields during
publication, verifies every target ID is actually present and retains extra
draining workers. A missing target cannot produce a successful acknowledgement.
All95 affected root/process/service/actor/Q1 checks and strict lint pass.
The service's public settings API, persisted/WebUI worker field and shipped
coordinator activation remain required; this is not full product sizing.


Local product acceptance on 2026-10-15 now wires the public service API,
PostgreSQL `execution_limits.max_workers`, startup and the shipped coordinator,
and live WebUI control/help to protocol 19. Desired and actual membership remain
separate while tasks drain. Unchanged target repetitions preserve CPU-active
settings publication. The worker identity predicate was simplified without
changing its three-case decision. See the r7/r8 evidence and limitations in
`docs/plans/simplified-v3-implementation.md`; this does not certify general
MCP child-admission saturation, fatal recovery or other-platform execution.

Protocol `20` adds exact dependent-slot observations and explicit validate,
reserve and release commands. Preflight leaves task/pool state unchanged and
checks the prospective frame before reservation. Publication advances only
hosting boundaries and acknowledges a real created coroutine; a coordinator
that ignores its requested dependent ID fails closed. Child admission retains
the parent's exact task/continuation and admits only the reserved child run.
Release requires the real finish acknowledgement and registry context cleanup.
Parent answers remain unconsumed until release. The shipped coordinator now
handles borrowed IDs; ordinary capacity edits preserve their active state.
This is a paired protocol upgrade: protocol19 workers are incompatible.

The r2/r3 local evidence on 2026-10-15 covers21 global,15 actor,29 process,
28 service,6 retained-execution and9 Q1 tests, native PostgreSQL migration,
live WebUI settings and ordinary match/no-match/MCP/restart checks, with strict
affected-package lint. The r3 subprocess regression additionally grows and
shrinks ordinary capacity around a child retaining1 MiB, rejects an unsafe
reduction unchanged and verifies the retained value after both edits. Ordinary
service dependency admission and exact provider-surface/attempt correlation
remain unwired; these checks do not certify saturated MCP ingress.
