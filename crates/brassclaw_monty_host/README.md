# Monty execution host — isolated upgrade candidate

`service::ServiceOwner` adds instance-owned boundary routing on top of the
transport actor. Boot waits for the actual root's idle work waits. Admissions
retain opaque product IDs separately from private worker handles. Every started
host future stays owned until its actual result is observed, even when a turn
waiter is dropped. Cancellation fences that attempt and signals its root scope;
it cannot settle an external operation by dropping the future. Other workers
continue during the wait. Completion requires the task port's actual transcript
validation, the root's return to its work wait and a child-context release with
no pending host correlation.

Fatal exchanges retain original commands, issued admission handles, pending
work, actual returned values, private withheld answers and containment/reap
evidence. Port panics contain the instance rather than losing the owning future.
Shutdown drains task ports, advances real VM control boundaries, closes the
parked workers and observes normal process exit. Caller join waits remain
cancellable without discarding the owner. None of these receipts authorizes
effect replay or acknowledges remote quiescence.

Actual file-I/O cases in `tests/monty_control/tests/service_host.rs` cover idle
boot, exact opaque handoff, two sequential tasks on the same root, A waiting
while B completes, dropped-waiter cancellation with its late actual read,
strict shutdown, fatal admission and host panic. These are isolated hosting
checks, not production Recipe, kernel or provider acceptance. PostgreSQL
ownership/health fencing, durable reconciliation, live resources and the
application driver/No-Match model-port adapter remain cutover work.

This crate supplies mechanical root-VM and child-VM boundaries for Phase 3a.
The application now uses its contained authoring utility adapter. Global
production orchestration is still not wired and does not resolve the seven composition
message-flow failures or establish production/global lifecycle acceptance.

Use [recipe.md](../../recipe.md) for definitions: a Recipe is ordered task
instructions plus an explicit component inventory. IBS/composition assembles
selected components and bindings. Monty owns sequencing and result handoffs.
`RecipeVm` is a child interpreter context, not a Recipe, compiler or Rust
workflow runner. Its methods advance exactly one caller-selected feed or
continuation; they never select the next step, dispatch a Tool, query a provider,
look up a component, retry or fall back to another agent loop.

The trusted supervisor must pair a context with the exact admitted task/attempt
and retain its approved catalogue snapshot. `PythonArtifact` checks source
integrity and syntax and holds aliases through suspension. It is not an approval
record or complete component revision manifest. Bindings expose requests only;
current kernel policy and fencing must be checked at each actual dispatch.

Each feed receives typed `inputs` with names matching `[a-z][a-z0-9_]*`, resets
`result`, and exports its assignment as typed data. Other locals survive between
steps. The global Monty caller must publish successful results under stable step
IDs, apply the selected recursive schemas/default rules, and resolve subsequent
step inputs. Child results are returned explicitly. No runtime value is inserted
into Python source. The value adapter accepts strings, i64/u64 integers, finite
numbers, booleans, null, lists and string-keyed objects. Other Python objects and
out-of-range integers fail explicitly. Traversal has aggregate node, depth and
UTF-8 payload bounds across all arguments of a call; no Python repr conversion
or full intermediate container cloning occurs. The graph adapter deliberately
uses the pinned Monty version's unstable representation APIs and needs review
on every interpreter upgrade.

Each context has an exact in-memory continuation identity. Wrong, stale and late
host answers remain private diagnostic evidence and cannot consume a different
continuation. Output overflow is an error; produced stdout remains available.
Errors terminate the context rather than replaying an already begun step.
Cancellation releases parked VM state and returns any pending host identity.
That VM-local acknowledgement does not prove an external operation stopped;
the supervisor still owes durable fencing and effect reconciliation.

Parent and child interpreters can share one live task compute account, with one
cumulative-clock cursor per interpreter. Parked parent execution is not charged
again during the child. Actual bytecode control yields retain the exact frames;
cancellation and technical-budget failures remain uncatchable by Python.
Host-side typed input, host-return, argument and result conversion now debit
that same account in separate synchronous intervals. Rejected conversions also
debit their actual work. Cancellation and the live revision are checked before
and after conversion; no dispatch/result is published after a budget failure.
Completed external return values and stdout remain private failure evidence.
These intervals never wrap a VM run, child or external wait, so they do not
double count interpreter time or charge parked tasks. Monty's internal
REPL source scanning and compilation are now charged by the versioned interpreter
extension's preparation clock, separate from both the VM execution windows and
these adapter intervals. Compiler-exit cancellation preserves REPL metadata and
prevents any opcode execution. Interpreter graph import, call argument export and
returned-value export now use owned preparation guards. Nested conversion does
not debit twice; VM reentry and native waits pause the preparation clock as
appropriate. Resource failure releases imported VM references and prevents
publishing an exported dispatch/result. One-shot artifact/root construction,
snapshot and remaining cleanup still need accounting. Native
operations without interpreter polling, full logical heap attribution/adaptive
memory, production allocator/process containment, durable continuation manifests, production
boot/inbox/ports and WebUI effective-revision acknowledgement remain Phase 3a
requirements. Do not use this candidate to bypass that gate.

Validation: actual interpreter cases in
[`child_host.rs`](../../tests/monty_control/tests/child_host.rs). They test child
computation and VM mechanics, not catalogue approval or production Tool effects.

The `global::GlobalVm` candidate owns one actual instance-root interpreter and
its correlated async futures. It accepts verified class-10 source from its
caller; there is no embedded production script fallback. Starting is distinct
from Ready: all configured workers must be suspended at distinct work waits
before the root reports its VM-level handshake. Early completion, task-bound
ports at boot, unsupported dispatch and missing/duplicate waits fail closed.
A busy boot can control-yield without falsely becoming Ready. This handshake is
not production facade readiness, which also requires the documented migrations,
instance lock, integrity, resource and ownership gates.

`GlobalVm::start_ready` drives that handshake before returning a root and checks
an explicit startup deadline, including time spent constructing it. Zero or
unrepresentable deadlines are rejected. Compiler/native work is synchronous:
process containment is still required to interrupt it within a bounded deadline.
`work_waits` exposes only actual worker admission continuations, sorted by worker
ID; generic port calls are excluded.

Root execution telemetry follows actual scheduler switches. `execution_accounting`
reports cumulative execution partitioned by VM-local coroutine, with preparation
separate. The map is bounded to the verified root's main/worker/discarded contexts.
`pending_context` correlates a real continuation with its issuing coroutine.
A coroutine can serve successive tasks; its identity is not an admitted handle
or authority grant. `GlobalVm::admit` requires a task budget and associates the actual waiting coroutine
with its issued task token and existing shared account before any Python runs.
Root execution deltas debit that same account as child VMs. Invalid data and
duplicate live tokens reject before binding; fatal admitted execution retains
the routing handle and account in the private worker snapshot. Returning to the
trusted work wait clears only that task association, never the cumulative clock.
Root accounting failures are retained explicitly by `task_compute_failure` and
the shared account; they do not enter the instance-terminal control latch.
Task-local root interruption now reaches the protected Python handler; it does
not set the instance-terminal control latch. Correlated preparation ownership
and outer value adaptation debit the same task account, with separate bounded
telemetry maps. The isolated extension is control.5 / dump ABI 0xBC05; older
suspended artifacts require reconciliation before upgrade. Durable finishing
and effect reconciliation remain the production service's responsibility.

A framed VM error with root lifecycle `Failed` now fences further worker IPC.
The actor classifies it as `InstanceFailed`, retains the real failure/snapshot
and undispatched command receipts, then kills and reaps the worker. A child VM
error whose root is still Ready remains task-local. Reaping an instance does not
settle its external effects or authorize replay; the owner still holds the
documented durable ownership and reconciliation responsibilities.

Root host arguments use the same aggregate typed adapter as child requests.
Each call has a generation-specific continuation key mapped to its exact Monty
future ID. Only pending futures in the actual waiting snapshot can receive
replies. Foreign, stale or wrongly timed answers remain private evidence without
consuming a different continuation. Pending future correlation stays available
for supervisor reconciliation even after fatal VM abandonment.

Admission accepts exactly the current root envelope's seven fields:
`task_token`, `conversation_id`, `message_id`, `turn_id`, `run_id`, `user_input`,
and `history`. The trusted production admission adapter must validate their
exact durable identities and history cutoff before delivery; shape checking
here cannot grant authority. Opaque IDs and empty user messages remain values.
Unknown fields, including Rust-only claims, are rejected before Python receives
them. Malformed/oversized input leaves the work wait and Ready root intact.
This candidate envelope is not the complete future framed transport protocol.

Generic port resolution cannot answer a work wait. Admission and explicit
instance shutdown have separate APIs; only Stopping can send None to a worker.
An external/task failure uses a catchable root `Raise`, allowing its Python task
coroutine to report failure; root `Abort` means fatal instance supervision.
No Rust code selects Recipes, calls providers, chooses subsequent steps or
invents successful completion. `global_host.rs` tests real root suspension and
an actual child's exception; it leaves the durable finish port unresolved.
Shutdown/fatal cleanup does not claim external-operation quiescence.

The root has its own cooperative execution-slice control and never applies one
600-second task clock to the lifetime/shared coroutine clock of the global VM.
Root and children share task consumption with independent clock cursors. Shared adaptive logical heap enforcement, production process/allocator
supervision, durable admission and port registry,
complete pinned component manifests, product boot/shutdown wiring and production
acceptance are still required. The application still uses the legacy driver.

The candidate class-10 source now resolves a Recipe's finalized reply reference
through the admitted task host before handing its content to the history Recipe.
The production service must bind `resolve_reply` to the same exact attempt's
`MontyTaskHost::published_reply_content`; accepting a `msg:` prefix alone is
insufficient. No reply is posted again by the root.

The candidate also accepts a **prepared**, transient `recipe-flow/1` tree from
IBS. This is not a new persisted Recipe field or an activated component. Its
nodes select exact prepared step IDs, select a branch of a single-key tagged
result, iterate a bounded homogeneous list, carry explicit values through a
bounded repeat, or return a typed value. The complete tree is structurally
validated before the first component call. References select task inputs,
constants, prior results or lexical loop items; they never evaluate expressions
or paste values into source. Repeat and foreach frames isolate iteration-local
results. A return ends the Recipe; exhausted repeats, unknown union tags and
oversized lists fail the begun Recipe without Tier-2 replay or truncation.

Each step occurrence is explicitly addressed by node IDs, branch tags and loop
indices. The production host still must validate/persist that identity together
with the exact task attempt and pinned manifest; an in-memory address is not a
durable effect receipt. It must enforce recursive input/result contracts,
approved associations, complete branch coverage and manifest integrity before
using this tree. `flow_host.rs` tests the real pure Monty validation/collection
helpers. It does not simulate component, provider or Tool execution and does not
prove a stored Recipe or the production model path works.


`process::GlobalProcess` and the `monty_worker` binary now supply an isolated
root-worker transport candidate. The binary installs `LimitedAllocator` and
arms a finite physical ceiling before reading or compiling source. It never
resets the ceiling per task/command. The supervisor launches an absolute binary
path directly with a cleared environment and private stdin/stdout pipes; allocator
diagnostics remain on stderr. No model, component selection or Rust workflow
loop is installed in the worker. Source is supplied by the verified boot caller.

Protocol 3 uses bounded length-prefixed JSON, strict unknown-field rejection,
monotonic request/reply sequencing and exact VM continuation generations. The
bounded encoder fails rather than truncating or allocating an unlimited buffer;
frame headers are checked before allocating receive buffers. Boot succeeds only
at the actual configured-worker work-wait handshake, and a second Boot cannot
replace that root. Admission, generic port replies and explicit shutdown remain
separate mechanical commands. Unknown claim fields never enter ordinary Python
state. This private ephemeral wire format is not a durable continuation manifest.

An independent parent response deadline kills and reaps a blocked worker without
relying on interpreter polling. Framing/transport errors and dropped exchanges
also fence the process. The rejected/interrupted command remains private evidence;
OS kill failures and lack of a reaped exit are distinguishable from acknowledged
termination. Normal shutdown reaps after the actual Stopped handshake without
racing a forced kill against graceful exit. Neither mode proves external host
operations settled and neither permits replaying a completed effect.

The exchange future must belong to the instance actor, never to the user-turn
future. Cancelling a user task must fence only its task/child and leave the root
and unrelated tasks alive. Dropping an instance transport exchange is fatal
supervision requiring the coordinated instance recovery contract. Product
startup, retained PostgreSQL ownership, durable attempt/effect reconciliation,
production child-port coordination, bounded finish/effect reconciliation, live
resource revision coordination and shared adaptive logical heap enforcement remain
required before production wiring. This process candidate does not close those
gates or the original seven composition failures.

Actual subprocess acceptance is in
[`process_host.rs`](../../tests/monty_control/tests/process_host.rs): boot/idle
lifetime, opaque admission, second-Boot refusal, exact retained work waits on
invalid/oversized input, real domain-failure reporting, explicit graceful
shutdown, independent busy-startup timeout, fatal native allocation containment,
dropped-exchange fencing, and malformed frame/unknown-field rejection without
private diagnostics. Successful Recipe/model/finish effects are not fabricated.


Before parent-side JSON encoding, an iterative borrowed traversal checks data
node/depth/byte bounds, including pending traversal entries. It uses the caller's
VM value limits with a transport nesting ceiling below the receiver's JSON
recursion limit. Deep or oversized task/host-return data is rejected before
serialization, retains the whole command and leaves the current worker boundary
untouched. This prevents a nested provider result from exhausting the parent
serializer's call stack before worker containment can apply. These are technical
transport errors, not token accounting or permission to truncate eligible history;
production must provide bounded transfer/reference handling for larger eligible
data. The actual caller case covers retained nested data plus a separate frame
encoding overflow followed by successful opaque admission.


Protocol 3 includes task-owned Recipe contexts inside the same contained worker.
The admission envelope contains exactly six data fields: conversation, message,
turn and run IDs, user input and history. After real root admission the worker
returns a fresh `TaskHandle` and supplies its string as Monty's routing token.
The supervisor associates that handle with its exact admitted attempt host;
the handle grants no authority and contains no claim or credential.

`Open` creates the sole root Recipe context for that handle, or an explicitly
parented child belonging to the same task. `Start` and continuation commands
advance only the selected actual interpreter boundary. Locals survive feeds;
child inputs/results cross as typed data. Failed/cancelled root contexts cannot
be recreated under the same handle. Cancelling a parent releases its descendants
and returns actual pending host identities. Late answers remain retained command
evidence. A running child yields cooperatively; cancelling it leaves the root
and other tasks alive. Terminal host errors abort only the child, while domain
errors remain catchable. The root transport rejects task-local abort answers.

Boot supplies one validated task-settings revision. All contexts for each task
share its live compute account; constructor work, conversion and execution are
charged without resetting usage across steps, children or settings edits.
Compare-and-publish settings conflicts are explicit and snapshots report the
effective revision and measured consumption, including admitted root coroutine
execution, correlated interpreter preparation and outer typed-value adaptation.
Task-scoped root interruption preserves actual pending-answer evidence. These
counters do not establish complete snapshot/cleanup accounting, durable finishing
or WebUI/DB desired-to-effective coordination. Closing a task while the root still owns it
is rejected without deleting its account or child contexts. Shutdown
cannot close root workers while task records remain; the supervisor must fence
and reconcile tasks before explicit release. Context/task release is only a
local hosting receipt, never a durable completion or effect settlement.

Actual subprocess cases cover A/B state separation, hostile typed strings,
parent/child return values, foreign parent/continuation rejection, descendant
and busy-child cancellation, unchanged root PID, stale returns, live settings
without usage reset, uncatchable child abort and premature shutdown rejection.
Pinned manifests, recursive component contracts, durable effects and the product
startup/port/ownership cutover remain required before production acceptance.

`transport_actor::TransportOwner` now owns the IPC future independently of turn
waiters. Acceptance is synchronous and bounded by unclaimed receipt counts and
reserved command/response frame bytes. A dropped ticket or wait future neither
cancels the accepted command nor kills the instance. The completion inbox retains
its exact serialized command, outcome and whether transport started until an
explicit supervisor collection. Original commands can be decoded for private
reconciliation; collection never authorizes replay. Failed exchanges keep their
exception/stdout/snapshot evidence, OS containment/reap errors and exit status.

Control/completion traffic has its own bounded queue and receipt credits. Full
ordinary admission capacity cannot consume this reserve. Controls have priority,
with at most eight consecutive control commands before available ordinary work
is preferred; explicit instance termination has priority in either case.
Rejected full/closed requests return their complete unaccepted command. The actor
advances only requested VM operations, never a workflow or Tool dispatch.

The supervisor keeps the owner and joins it before releasing PostgreSQL ownership.
Cancelling a join wait retains the join handle. Root Stopped is followed by a real
normal exit acknowledgement; transport failure and requested containment retain
separate receipts for queued commands that never started. Dropping the owner
requests fatal containment, while dropping a turn waiter leaves it alive. Neither
mode acknowledges external effects. This RAM inbox is not the durable run/effect
ledger or a crash-recovery manifest. Frame reservation is not a physical heap
measurement; decoded data/metadata overhead remains part of host resource design.

Actual actor acceptance covers a polled-then-dropped turn waiter, retained opaque
admission, reserved controls under ordinary backpressure, frame credits, graceful
exit and closed admission, real native allocator failure with undispatched queue
evidence, and a Python child failure that leaves the actor/root available. Native
failure uses Monty's supported string allocation; `bytearray` is unsupported.
Test joins/waits are bounded. The actor remains in the isolated candidate workspace
and is not yet connected to the production driver.


Current isolated extension: control.5 / dump ABI 0xBC05; private worker protocol 3.
The root establishes a protected task scope before dispatch. Compute exhaustion
can interrupt pure root bytecode without terminating unrelated workers. An
outstanding host result settles its exact future before the task handler runs;
its actual answer remains in the private snapshot for reconciliation. This does
not establish production cutover or bounded task cancellation acknowledgement.

`RecipeCommand::CancelTask` addresses one issued private task handle, requests
root interruption and fences every existing child context. It rejects new
contexts/feeds and retains all accounts and pending-call keys. Its response is
`CancellationRequested`, not task completion or external-effect acknowledgement.
Root bytecode reaches its protected handler at a control boundary; a pending
root call first settles its actual future, retaining its returned answer for
reconciliation. Unrelated workers remain available. The verified root must
await each task-scoped host operation before issuing another; concurrent calls
in the same root scope fail before dispatch and preserve pending evidence.


`utility::execute` supplies a separate contained parser/pure-formatter operation
for the Monty 1.0 utility-caller migration. The same allocator worker has an
explicit one-operation `--utility` mode (its own private protocol 1); it never
boots or replaces the global orchestrator. Source and typed inputs are separate,
all input roots share aggregate preflight limits, and unsupported output fails
rather than becoming null. No host ports are installed; sleeps and external
operations fail. Parsing executes no Python. Success requires the exact reply
and successful process exit. Parent deadlines and allocator exhaustion kill/reap
only the disposable utility; private error receipts retain original source/data,
stdout, diagnostics and OS containment evidence. Application Q1/formatter and
syntax callers still need coordinated API/dependency migration; this isolated
API does not remove their legacy runtime.

Four real utility cases cover parsing, hostile/control data and opaque IDs, fresh
state, denied host/file/sleep operations, invalid/cyclic output, stdout overflow,
aggregate input rejection before spawn, busy deadline and fatal allocator
containment followed by a successful independent operation. Existing root/actor
process cases still pass; strict affected host/caller Clippy checks pass after
using the shared opaque envelope fixture in the formatter round-trip.

## Worker packaging

Build `cargo build -p brassclaw -p brassclaw_monty_host --bins` (or replace the
first package with `brassclaw_reborn_cli` for the standalone product). Install
`monty_worker` beside the product executable. Docker and release assets include
both executables; the installer verifies both checksums before replacement.
There is no PATH search or in-process utility fallback. Library tests require
`cargo build -p brassclaw_monty_host --bin monty_worker` first in the same profile
and target directory. These serial commands belong in the validation queue.
