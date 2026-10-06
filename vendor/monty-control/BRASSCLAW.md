# Isolated Monty control extension

Upstream: https://github.com/pydantic/monty, tag `v1.0.0`, commit
`85c5d1f6bef038405cfc40a4eed94806e303567e` (the isolated upgrade gate's
Cargo.lock). MIT license and upstream notices are retained. Only the four
library crates and their source/manifests are vendored. Original file hashes
are recorded in UPSTREAM-SHA256.json; Cargo.toml is narrowed to these crates.

CONTROL.patch uses zero-context hunks so blank context markers do not create
trailing-whitespace warnings when the patch is tracked. Apply it only to the
verified upstream files, using `git apply --unidiff-zero`.

Extension version: `1.0.0-brassclaw.control.1`. Production still uses the old
compatible interpreter; this library is used only by `tests/monty_control`.

The trusted ExecutionControl hook observes cumulative active execution time
at periodic resource checks and execution-window exits, including completion
and errors. Errors latch and become uncatchable. Its Arc is never serialized;
a controlled snapshot requires trusted reattachment before resuming. Reattachment
does not erase observed terminal failures or time consumption.

ControlYield snapshots before the next opcode, retaining operand stack, locals,
frames and exception state. Resume injects no external result. Synchronous
native reentry cannot yield because its Rust stack is not serializable; it
retains yield requests until returning to snapshot-safe execution. Native
operations can observe cancellation through existing resource checks, but
unpolled native operations remain a bounded-response gap. Synchronous `run`
does not service yields; callers must use iterative execution.

Dump ABI `0xBC01` deliberately rejects upstream dumps and previous binaries.
It does not authenticate snapshots; the existing trusted-producer requirement
still applies. Deployment must reconcile old continuations before cutover.

This does not prove heap isolation, async coroutine CPU attribution, complete
native-operation preemption, export/compile CPU accounting, production global
hosting or WebUI acknowledgement. Phase 3a acceptance remains required.
