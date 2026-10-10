# Isolated Monty control extension

Upstream: https://github.com/pydantic/monty, tag `v1.0.0`, commit
`85c5d1f6bef038405cfc40a4eed94806e303567e` (the isolated upgrade gate's
Cargo.lock). MIT license and upstream notices are retained. Only the four
library crates and their source/manifests are vendored. Original file hashes
are recorded in UPSTREAM-SHA256.json; Cargo.toml is narrowed to these crates.

CONTROL.patch uses zero-context hunks so blank context markers do not create
trailing-whitespace warnings when the patch is tracked. Apply it only to the
verified upstream files, using `git apply --unidiff-zero`.

Extension version: `1.0.0-brassclaw.control.7`. The application dependency graph uses
these sources, including the Engine API migration, contained authoring utilities
and global installation worker. The original seven composition regressions have
passed through the ordinary runtime. Complete production lifecycle, adaptive
settings and catalogue acceptance remain open; interpreter evidence alone does
not establish completion of those gates.

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
persistent workers are reused. Admission association, preparation ownership,
task-local interruption and safe finishing still require integration. Control
errors remain interpreter-terminal; do not use them to enforce one task's limit
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
publication; full live frame uptake and queued-exchange retention remain open.
Dump ABI stays `0xBC05`; the process protocol and interpreter dump are separate.

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
neither the dump ABI nor the worker protocol changes. Interpreter, worker and
ordinary product acceptance must establish isolation and retained selections
before claiming this optimization is accepted.
