# Monty resumable control gate

This independent workspace executes the versioned extension at
`vendor/monty-control` without changing production dependencies. Its lockfile
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
600→30-second settings publication at compiler exit. Extension `control.3` uses
dump ABI `0xBC03`; reconcile older continuations before any upgrade. Static
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

Extension `control.3` uses dump ABI `0xBC03` and owned preparation guards for
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
