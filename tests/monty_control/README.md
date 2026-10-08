# Monty resumable control gate

Current extension: control.7 / dump ABI 0xBC05, private root-worker protocol 5.

`flow_host.rs` runs the actual class-10 pure prepared-flow helpers in the contained
utility worker. Four cases cover whole-tree preflight failures, branch dominance,
iteration-local results, explicit repeat carry, actual list collection and runtime
shape failures. No component/provider/Tool success is substituted. The transient
`recipe-flow/1` candidate still needs persisted authoring/IBS schema, exact approval,
effect occurrence and production caller integration; these are not tests of the
seven production composition message flows.
The sections below record earlier validation milestones; version numbers and
then-open items within those milestones describe their recorded state. The
current host contract is in the linked hosting README.

The extension exposes actual coroutine identity at control
and host boundaries and separates execution from preparation. The new root case
checks A→B→A attribution and excludes real host waits. `scheduler_accounting.rs`
checks native Python callback reentry, exception handling, exact snapshot identity,
and short completion/error clocks. Coroutine identities are reused by persistent
workers; the host now binds admitted task accounts and protected root scopes
explicitly. Durable finish/cancellation acknowledgement remains production work.
The extension also prevents call/coroutine counter wrap before dispatch/ownership.

This independent workspace executes the same versioned extension at
`vendor/monty-control` used by production dependencies. Its lockfile
pins the resolved gate dependencies. Run Cargo sequentially:

```sh
cargo test --manifest-path tests/monty_control/Cargo.toml --locked --all-targets -- --nocapture
cargo clippy --manifest-path tests/monty_control/Cargo.toml --locked --all-targets -- -D warnings
cargo clippy --manifest-path vendor/monty-control/Cargo.toml --locked -p monty -p monty-types --lib -- -D warnings
```

Seven control checks use the actual interpreter: pure-loop pause/resume without
replay or a synthetic return value; execution of another interpreter while the
first is parked; retained REPL locals/exception state/later feeds; uncatchable
busy-loop cancellation; live duration revision using the real shared Rust task
account; final/error execution-window accounting; fail-closed snapshot restore;
and deferral of control suspension across synchronous native callbacks.

The existing compatibility, global lifecycle, Recipe failure/state and task
accounting cases are compiled against the extension via explicit test targets.
They reuse their original source rather than duplicating assertions or replacing
production architecture. Passing them is interpreter evidence, not proof of
successful provider/Recipe execution through the production global service.

The shared-account check retains 31 seconds of previously recorded task usage
and publishes a 600→30-second revision during actual Python execution. It does
not spend 31 seconds executing Python or establish production settings delivery.
Raising the limit after failure cannot revive the terminated account.

REPL source scanning and compilation now use serialized preparation windows,
charged on success and syntax failure, with live control checked on entry/exit.
Compiler-exit cancellation restores the moved REPL compiler tables and prevents
any opcode from running. Three additional actual-interpreter cases cover that
failure path, preparation persistence and old-ABI rejection, and a real shared
600→30-second settings publication at compiler exit. Extension `control.4` uses
dump ABI `0xBC04`; reconcile older continuations before any upgrade. Static
feed/turn clocks and `elapsed()` remain VM-only; the control clock additionally
includes `preparation_elapsed()`. Restore adapters must preserve both parts and
their shared-account cursor, without debiting earlier preparation twice.

Unpolled native operations, compiler response bounds, one-shot construction,
snapshots and remaining cleanup still need separate bounded
accounting/containment evidence. This hook does not attribute a shared global
coroutine clock to different tasks. Root service work and task-owned child
execution need separate ownership clocks. Heap attribution, adaptive memory,
allocator backstop, persistent settings acknowledgement, immutable component
pinning and the global production dispatcher remain Phase 3a requirements.
The original seven composition failures are still blocked by UUID-only legacy
Thread loading; this test workspace neither bypasses that path nor activates the
new global source.

The isolated [`brassclaw_monty_host`](../../crates/brassclaw_monty_host/README.md)
candidate now has eleven real child-interpreter cases in `tests/child_host.rs`.
They cover typed hostile text/u64/nested values, retained locals and mandatory
result assignment, actual parent/child computation and independent task progress,
foreign/stale/late continuation evidence, parked and busy cancellation, live
shared duration changes without a usage reset, unsupported values/unbound calls,
explicit source/stdout limits, eager async resumption and catchable-versus-terminal
child exceptions. A completed child result remains evidence when a later live
budget check rejects the parent. These cases are VM-boundary evidence; the
candidate remains outside the production graph.

Host-side typed conversion now charges the same task account in non-overlapping
synchronous intervals, including rejected inputs. The added case observes actual
conversion charges, retained usage through a subsequent real feed and cancellation
before another feed. Live-budget failure retains both a completed child return
and prior stdout. Monty's internal preparation/export and native containment
remain separate open requirements; this does not meter the whole hosting call.

Five `tests/global_host.rs` cases exercise the root hosting candidate against
actual `global_mode.py`: complete boot work waits, early/incomplete/busy boot,
opaque and empty-input task interleaving, an actual child exception, pending-call
retention, malformed/oversized/extra-field rejection, foreign/stale replies and
explicit shutdown. No successful Recipe/provider/durable finish is fabricated.
Together with the eleven child cases, these 16 checks and strict library/caller
lints passed sequentially. Production dependency pins and driver wiring remain
unchanged; this is not the seven composition tests' acceptance.

After the preparation change, all 47 actual interpreter checks in this workspace
and strict extension/host/caller lints passed in the sequential screen queue.

Extension `control.4` uses dump ABI `0xBC04` and owned preparation guards for
interpreter graph imports and exported arguments/results. Nested conversion
uses its existing clock, VM reentry pauses preparation, and real native sleep
pauses both clocks. Telemetry returns explicit errors on accounting failure;
active preparation without its live guard fails closed after dump restoration.
Imports release their VM references when the closing resource check fails, and
exports cannot publish a dispatch/result after failure. Additional cases use
real input/child-return cancellation and a real child's list round-trip, native
sleep, guard unwinding/foreign ownership, and active-preparation dump rejection.
Compilation, graph and host conversion intervals remain distinct. Response
bounds, snapshot/cleanup and one-shot construction accounting remain open.

The graph change passes 51 distinct interpreter checks: the 50-case affected
suite plus the final 14-case control run with the added child graph regression
and explicit rejection of both older ABIs. Strict extension, host and caller
lints pass. Cargo ran sequentially in background screen; unchanged checks from
the first graph queue are reused for the final test-only addition.


The updated root startup caller passes six real-interpreter boundary tests,
including `start_ready`'s configured-worker handshake, rejected zero deadline
and an actual busy-boot deadline. The task root resolves its finalized reply
reference before the history handoff. Strict host/caller lints pass. Evidence:
`/private/tmp/brassclaw-monty-startup-handshake.log`,
`/private/tmp/brassclaw-monty-startup-host-lints.log` and
`/private/tmp/brassclaw-monty-startup-caller-lints.log`. Unchanged control, child
and compatibility evidence above is reused; this still is not global production
or the original seven composition failures' acceptance. Synchronous compiler
and native work still require bounded process supervision.


`allocator.rs` runs the real `allocator_probe` child process with the actual
Monty allocator installed. It proves that an overflowing finite limit is rejected
and the retained finite cap still ends a real native allocation with Monty's OOM
exit code/diagnostic. Strict allocator/caller lints pass. Existing registry pins
are preserved. This verifies the worker's physical backstop, not the production
supervisor or shared adaptive logical heap account.


`process_host.rs` adds six actual subprocess cases against the same worker entry
point as the hosting candidate. They prove real boot/idle lifetime, preserved
opaque admission, duplicate-Boot rejection, failed/oversized admission and
command evidence, real domain-failure reporting, explicit graceful shutdown,
independent busy-startup timeout/reaping, fatal native allocator containment,
dropped-exchange fencing and malformed protocol rejection without private
payload diagnostics. The worker itself supplies no Recipe/model/effect success.
The six existing root cases and six worker cases pass with `--locked` and retained
registry pins. After clearing the worker environment, all six affected worker
cases and strict host/caller lints pass again. Evidence:
`/private/tmp/brassclaw-process-host-final.log`,
`/private/tmp/brassclaw-process-host-contained.log`,
`/private/tmp/brassclaw-process-host-contained-lints.log` and
`/private/tmp/brassclaw-process-host-contained-caller-lints.log`.

This establishes contained root VM mechanics, not production actor ownership,
per-task cancellation, child containment, live WebUI revision uptake, shared
adaptive heap accounting or the original seven composition tests' acceptance.


Final parent-side data preflight acceptance passes the same six actual worker
cases with added retained-depth and encoder-frame overflow checks. It prevents
recursive parent encoding of excessive data, without consuming the worker's
pending boundary. Strict host/caller lints pass:
`/private/tmp/brassclaw-process-host-bounded-data.log`,
`/private/tmp/brassclaw-process-host-bounded-data-lints.log` and
`/private/tmp/brassclaw-process-host-bounded-data-caller-lints.log`.


Current isolated extension: control.6 / dump ABI 0xBC05; private worker protocol 4.
The root establishes a protected task scope before dispatch. Compute exhaustion
can interrupt pure root bytecode without terminating unrelated workers. An
outstanding host result settles its exact future before the task handler runs;
its actual answer remains in the private snapshot for reconciliation. This does
not establish production cutover or bounded task cancellation acknowledgement.


`utility_host.rs` verifies the real one-operation contained parser/formatter
mode. It supplies no host-effect success. Parsing does not execute; hostile
strings/control characters and opaque envelope values round-trip as data.
Unsupported/cyclic output and stdout overflow fail explicitly. Aggregate input
rejection preserves the original request before spawn/serialization. Busy
execution deadlines and actual native allocator failure retain reaped exits;
the caller and a later independent utility remain available. The utility's
private protocol 1 is separate from global worker protocol 3 and does not create
a per-chat orchestrator. Production utility-call migration remains open.


The control.6 allocator ownership cases exercise real zeroed/overaligned
allocations, nested scopes, growth/shrink outside scope and cross-thread freeing.
Actual root/child interpreter heaps share the counter and preserve retained
parent objects after child release. The process caller checks manual rejection,
pending automatic reduction, admission backpressure, revision fencing and real
soft-limit preflight without killing the root. The service caller verifies heap
edits during real paused file I/O, expected denial without service failure and
publication after a waiter is dropped. Physical backstop probes deliberately
omit a soft limit only in the standalone mechanical worker; ServiceOwner refuses
such a boot. No adaptive OS sampling or production WebUI uptake is claimed.


The control.7 resize cases exercise the actual allocator, physical reduction
rejection while non-VM data remains live, retained ownership across expansion,
unsafe/overflow rejection without mutation and finite physical enforcement after
shrink. The process case allocates and retains a 70 MiB Monty string beyond the
original 64 MiB worker cap, preserves its locals/root/PID through rejected edits,
and reduces the budget only after releasing that context. These are worker
resize cases; adaptive OS measurements and WebUI policy uptake remain separate.
