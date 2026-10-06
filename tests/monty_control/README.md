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
