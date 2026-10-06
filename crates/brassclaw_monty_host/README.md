# Monty execution host — isolated upgrade candidate

This crate supplies mechanical root-VM and child-VM boundaries for Phase 3a.
It is a dependency of the isolated `tests/monty_control` workspace only. It is
not enabled in the application, and does not resolve the seven composition
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
Task-owned child clocks remain separate. Root CPU attribution, process startup
supervision, allocator/heap containment, durable admission and port registry,
complete pinned component manifests, product boot/shutdown wiring and production
acceptance are still required. The application still uses the legacy driver.

The candidate class-10 source now resolves a Recipe's finalized reply reference
through the admitted task host before handing its content to the history Recipe.
The production service must bind `resolve_reply` to the same exact attempt's
`MontyTaskHost::published_reply_content`; accepting a `msg:` prefix alone is
insufficient. No reply is posted again by the root.


`process::GlobalProcess` and the `monty_worker` binary now supply an isolated
root-worker transport candidate. The binary installs `LimitedAllocator` and
arms a finite physical ceiling before reading or compiling source. It never
resets the ceiling per task/command. The supervisor launches an absolute binary
path directly with a cleared environment and private stdin/stdout pipes; allocator
diagnostics remain on stderr. No model, component selection or Rust workflow
loop is installed in the worker. Source is supplied by the verified boot caller.

Protocol 1 uses bounded length-prefixed JSON, strict unknown-field rejection,
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
child hosting in the contained worker, root task CPU attribution, live resource
revision coordination and shared adaptive logical heap enforcement remain
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
