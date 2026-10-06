# Isolated Monty control extension

Upstream: https://github.com/pydantic/monty, tag `v1.0.0`, commit
`85c5d1f6bef038405cfc40a4eed94806e303567e` (the isolated upgrade gate's
Cargo.lock). MIT license and upstream notices are retained. Only the four
library crates and their source/manifests are vendored. Original file hashes
are recorded in UPSTREAM-SHA256.json; Cargo.toml is narrowed to these crates.

CONTROL.patch uses zero-context hunks so blank context markers do not create
trailing-whitespace warnings when the patch is tracked. Apply it only to the
verified upstream files, using `git apply --unidiff-zero`.

Extension version: `1.0.0-brassclaw.control.3`. Production still uses the old
compatible interpreter; this library is used only by `tests/monty_control`.

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

Dump ABI `0xBC03` stores the owned preparation account and deliberately rejects
upstream dumps and previous `0xBC01`/`0xBC02` extensions. It does not reset
preparation on reattachment.
It does not authenticate snapshots; the existing trusted-producer requirement
still applies. Deployment must reconcile old continuations before cutover.

This does not prove heap isolation, async coroutine CPU attribution, complete
native-operation preemption, bounded compiler/graph response, snapshot/cleanup
accounting, one-shot construction accounting, production global
hosting or WebUI acknowledgement. Phase 3a acceptance remains required.


The same unreleased `control.3` extension fixes the worker allocator's finite
ceiling arithmetic: baseline-plus-budget overflow is rejected before replacing
an armed limit, and live allocation/refund counters fail closed on overflow or
underflow. `tests/monty_control` installs the actual allocator only in its
isolated `allocator_probe` binary. The real native Monty allocation is stopped
by the previous finite cap even after an invalid replacement was rejected.
This is physical worker-backstop evidence, not shared logical heap/adaptive
accounting or production process supervision. The dump ABI remains `0xBC03`;
these allocator changes do not change serialized interpreter state.
