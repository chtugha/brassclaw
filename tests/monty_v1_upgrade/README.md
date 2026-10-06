# Monty 1.0 upgrade gate

This independent test workspace pins `monty`, `monty-types` and `monty-alloc`
to the same `v1.0.0` tag and lockfile. It exercises the new API before changing
the application's two interpreter dependencies. Run Cargo sequentially:

```sh
cargo test --manifest-path tests/monty_v1_upgrade/Cargo.toml --locked --all-targets -- --test-threads=1 --nocapture
cargo clippy --manifest-path tests/monty_v1_upgrade/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path tests/monty_legacy_baseline/Cargo.toml --locked --all-targets -- --test-threads=1 --nocapture
cargo clippy --manifest-path tests/monty_legacy_baseline/Cargo.toml --locked --all-targets -- -D warnings
```

The API checks cover source compilation, repeated work waits in one VM, async
A→B→A, the graph-based arguments and dependent tool results, fresh snippet state,
host failure/abort, bounded stdout and suspended execution timing. The Recipe
tests use the actual orchestrator source and deterministic host replies; they
verify sequencing, not the production namespace or kernel dispatch adapter.
The task-accounting check passes real root/nested cumulative execution clocks to
`brassclaw_resources::MontyTaskClock` and the shared task account. Duplicate
checkpoints, parked waits and feed-limit resets add no extra consumption. This
uses explicit final host checkpoints; upstream `Complete`/error clock recovery
and attribution between concurrent tasks still require an extension/host contract.

The allocator probe is a separate executable. Only it installs Monty's global
allocator. Tests check both a returned interpreter memory error and allocator
termination with the documented OOM exit code, then run another probe to confirm
that the parent survives. Each child has a containment deadline and is reaped.
This proves the backstop in an isolated process. It does not establish logical
Monty heap attribution, adaptive memory, or a production hosting transport.

Passing these tests does **not** complete Phase 3a. Outstanding requirements
include live changes during active pure-Python execution, bounded resumable
CPU scheduling, shared task timing over nested executions, every production
adapter, task failure isolation within the global VM, durable settings/runtime
acknowledgement, allocation-setting migration, performance baseline and rollback.
No saved interpreter state should be restored across the version change without
the explicit reconciliation protocol.

Monty 1.0 graph values, `CallArgs`, explicit `CompileOptions`, separate
`monty-types` imports and capped `PrintWriter::CollectString` all require adapter
changes. Duration setters reset feed/turn accumulators; cumulative `elapsed()`
is preserved. Reassigning the full task duration at each host transition would
incorrectly give the task a new budget.

Changes to `basic_mode.py` also change the protected builtin content checksum.
Existing databases must use the verified repair/upgrade path before booting the
new binary; never bypass the integrity check or silently replace an operator
override. The new failure contract deliberately exposes invalid/missing
instructions instead of hiding them behind a direct LLM fallback. The legacy
No-Match/history seed recipes still need their declared step-isolation and host
binding gaps repaired and validated before production cutover.

The legacy timing workspace uses a separate dependency graph: combining old and
new Monty fails Cargo's unique `links=python` constraint through incompatible
optional PyO3 versions. Both use the same benchmark helper, 10 warmups and 200
samples of compile/start plus 1,500 integer additions. Run both sequentially on
the same host/profile. Local Rust 1.98.1 debug measurements were old p50/p95
510,625/857,750 ns and new 525,000/758,542 ns (+2.8%/-11.6%). These are diagnostic
microbenchmarks, not the end-to-end §5.1 acceptance or a product speedup claim.
