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
A→B→A, graph-based arguments, dependent tool results, state retained within a
Recipe and isolation between unrelated Recipe contexts, host failure/abort,
bounded stdout and suspended execution timing. The Recipe tests use the actual
orchestrator source, a real Monty host object and deterministic host replies.
They verify sequencing and interpreter argument routing; production host
dispatch and kernel enforcement still require caller-level acceptance.
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

`global_lifecycle` executes the prospective `global_mode.py` source unchanged.
Its bounded coroutine slots are all waiting before input arrives. While A's
host call is pending, B's slot processes an empty input, reports its host error,
and accepts C with a separate question. A's and C's errors return their slots
to work waits; only explicit shutdown ends the VM. Host routing keeps the real
Monty receiver and typed arguments. This is interpreter evidence, not simulated
Recipe/provider success or a production-service test.

The new source is deliberately **not seeded or activated** by the legacy driver.
Its host protocol requires task-scoped routing, pinned `program_ref`/step IDs,
typed inputs/results, and verified completion refs. Recipes publish their own
replies through the transcript port; the root must not post their last result
again. History receives the finalized reply ref, not interpolated source text.
The existing seed Recipes and host dispatcher do not yet implement that full
protocol. Non-reply completion, successful matched/No-Match execution, actual
cancel acknowledgements and fatal resource failure recovery still need caller
acceptance. Keep production activation behind the complete upgrade gate.

Monty 1.0 graph values, `CallArgs`, explicit `CompileOptions`, separate
`monty-types` imports and capped `PrintWriter::CollectString` all require adapter
changes. Duration setters reset feed/turn accumulators; cumulative `elapsed()`
is preserved. Reassigning the full task duration at each host transition would
incorrectly give the task a new budget.

Changes to `basic_mode.py` also change the protected builtin content checksum.
Existing databases must use the verified repair/upgrade path before booting the
new binary; never bypass the integrity check or silently replace an operator
override. The new failure contract deliberately exposes invalid/missing
instructions instead of hiding them behind a direct LLM fallback. Fresh No-Match/history
PythonCode now uses explicit typed inputs and prior results. Fresh history IBS
metadata references real component/binding IDs, with the memory primitive seeded
before the Recipe. Existing stored bodies and legacy history metadata still need
a revision-preserving upgrade. Child host binding/dispatch and the global
reply-ref history protocol remain unimplemented; fresh seed correctness is not
production cutover evidence.

The legacy timing workspace uses a separate dependency graph: combining old and
new Monty fails Cargo's unique `links=python` constraint through incompatible
optional PyO3 versions. Both use the same benchmark helper, 10 warmups and 200
samples of compile/start plus 1,500 integer additions. Run both sequentially on
the same host/profile. Local Rust 1.98.1 debug measurements were old p50/p95
510,625/857,750 ns and new 525,000/758,542 ns (+2.8%/-11.6%). These are diagnostic
microbenchmarks, not the end-to-end §5.1 acceptance or a product speedup claim.

The separately versioned [resumable control gate](../monty_control/README.md)
now provides actual interpreter evidence for control checkpoints during busy
Python, state-preserving yields and final/error-window reporting. This upstream
workspace stays pinned to unmodified v1.0.0. The extension remains isolated and
does not establish production global hosting, complete native preemption or
settings delivery. Its snapshot ABI intentionally rejects upstream snapshots.
