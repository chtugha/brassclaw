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

Unpolled native operations, compilation and graph export need separate bounded
accounting/containment evidence. This hook does not attribute a shared global
coroutine clock to different tasks. Root service work and task-owned child
execution need separate ownership clocks. Heap attribution, adaptive memory,
allocator backstop, persistent settings acknowledgement, immutable component
pinning and the global production dispatcher remain Phase 3a requirements.
The original seven composition failures are still blocked by UUID-only legacy
Thread loading; this test workspace neither bypasses that path nor activates the
new global source.

The isolated [`brassclaw_monty_host`](../../crates/brassclaw_monty_host/README.md)
candidate now has ten real child-interpreter cases in `tests/child_host.rs`.
They cover typed hostile text/u64/nested values, retained locals and mandatory
result assignment, actual parent/child computation and independent task progress,
foreign/stale/late continuation evidence, parked and busy cancellation, live
shared duration changes without a usage reset, unsupported values/unbound calls,
explicit source/stdout limits, eager async resumption and catchable-versus-terminal
child exceptions. A completed child result remains evidence when a later live
budget check rejects the parent. These cases are VM-boundary evidence; the
candidate remains outside the production graph.

Five `tests/global_host.rs` cases exercise the root hosting candidate against
actual `global_mode.py`: complete boot work waits, early/incomplete/busy boot,
opaque and empty-input task interleaving, an actual child exception, pending-call
retention, malformed/oversized/extra-field rejection, foreign/stale replies and
explicit shutdown. No successful Recipe/provider/durable finish is fabricated.
Together with the ten child cases, these 15 checks and strict library/caller
lints passed sequentially. Production dependency pins and driver wiring remain
unchanged; this is not the seven composition tests' acceptance.
