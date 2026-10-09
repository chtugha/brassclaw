# monty-alloc

The global allocator [Monty](https://github.com/pydantic/monty) workers run
under: it exposes live-byte usage to the interpreter and ends the process if
the worker's hard memory ceiling or system allocator refuses an allocation.

Monty executes untrusted Python, so a host has to be able to cap what a session
may allocate. Enforcing that in the allocator catches every byte the worker asks
for, wherever in the process it is asked for. Counting rather than asking the
kernel for `RLIMIT_AS` is what makes the limit portable and its units
meaningful: it bounds bytes the process requested, not virtual address space, so
mapped text, thread stacks and file mappings do not consume the budget. The
tradeoff is that it binds only what reaches this allocator — everything
sandboxed code can allocate, but not a direct `mmap`.

```rust
#[global_allocator]
static ALLOC: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

// The caller chooses headroom above the interpreter's soft limit.
let hard_budget = monty_types::memory_limit_with_headroom(Some(8 * 1024 * 1024), false);
monty_alloc::set_hard_limit(hard_budget).unwrap();
```

The interpreter compares live usage with the session's soft limit at execution
checkpoints and raises `MemoryError` after crossing it. The caller gives this
crate a higher hard budget, leaving room for exception machinery and allocations
between checkpoints; this crate adds the worker baseline and ends the worker if
that ceiling is crossed. `None` lifts the hard ceiling. See
`limitations/resource_limits.md` for how each outcome surfaces to a host.

## Ending the process

Exceeding the hard limit cannot raise a Python exception — it happens below the
interpreter — so the worker dies and its host replaces it. Neither a panic
(whose machinery allocates) nor a plain abort will do: `SIGABRT` is also what a
stack overflow produces, and a host that cannot tell those apart cannot report
`MemoryError`.

The `exit-code` feature picks how the process ends:

- **on** — `process::exit(monty_types::OOM_EXIT_CODE)`, the dedicated status a
  parent reads to classify the death. Used by the `monty subprocess` worker,
  whose parent is [`monty-pool`](https://crates.io/crates/monty-pool).
- **off** (default) — `process::abort()`, which on wasm is a trap. A wasm
  module has no exit status to offer, and its host already treats a turn that
  ends without a terminating event as a dead instance.

Only a binary or a wasm module may declare a `#[global_allocator]`: in a native
cdylib it would hijack the allocator of the embedding host process.

## Only crates in this workspace

Published so the `monty` binary can be, not for direct use. Finite baseline-plus-budget overflow is rejected on every target; an invalid
replacement preserves the previous hard limit.


## BrassClaw VM ownership extension

The versioned `control.6` extension can separately measure allocations owned by
the VM execution domain. Call `enable_vm_accounting()` before constructing any
VM, then enter `VmAllocationScope` around synchronous interpreter work. The guard
is thread-bound and restores nested scopes. Every allocation has a private
ownership header: resize/free retain the original tag even outside that scope
or on another thread. Accounted bytes include headers and alignment padding.
Never hold this guard across a migrating async future.

`vm_live_bytes()` reports the shared root/child domain. `set_vm_limit(bytes)`
publishes a finite soft ceiling without resetting retained allocation charges;
the serialized worker owner must exclude simultaneous VM execution. A reduction
below actual live bytes is rejected. The independent hard allocator backstop
still contains between-checkpoint and compiler allocations, including transport.
Neither counter measures RSS, stacks or direct mappings. The hosting adapter
must adopt received data into VM allocations and keep unrelated frame handling
outside the scope. See `BRASSCLAW.md` for the verified measurement boundary and
remaining production/adaptive acceptance requirements.


`control.7` supplies `set_worker_limits(vm_bytes, reserve_bytes)` for that same
quiescent owner. It replaces both the logical limit and finite physical backstop
after validating actual VM/process allocations and checked arithmetic. The
reserve covers bounded transport/exception overhead separately; growing the VM
does not reset either live counter or move VM allocations into baseline. Typed
errors distinguish invalid settings/accounting from unsafe reductions. A failed
publication leaves both limits unchanged. Concurrent allocation/execution must
be excluded by the worker owner. This API is not an OS capacity measurement.

`validate_worker_limits(vm_bytes, reserve_bytes)` shares those checks without
publishing either limit or changing counters or baseline. It is an observation,
not a capacity reservation. The host must keep the worker quiescent across
feasibility, durable commit and publication; a later unfenced write cannot rely
on an earlier successful probe.
