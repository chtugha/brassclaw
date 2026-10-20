# Embedded Oniguruma allocator bridge

Based on the exact Cargo registry `onig_sys` 69.9.3 source (embedded header reports Oniguruma 6.9.10).
Original upstream licenses and sources are retained. No regex matching behavior
is intentionally changed; the default upstream features remain available.

The `brassclaw-global-allocator` feature (BrassClaw Rust 1.96 toolchain) compiles
the embedded C sources with their malloc/calloc/realloc/free calls bound to the
private Rust functions in `src/allocator.rs`. They use `std::alloc`, not a new
allocator, budget owner or monitor. Isolated `monty_worker` already installs
`monty_alloc::LimitedAllocator`; ordinary product binaries keep their own global
allocator. Allocation/free/reallocation ownership and actual worker OOM handling
therefore use the existing allocator. A checked, aligned private header preserves
the exact Rust layout, zeroed payload and failed-reallocation semantics.

A system or dynamic Oniguruma library cannot satisfy this feature: the build
fails rather than silently linking untracked C allocations. Both the main and
standalone control workspaces must patch the same dependency. The encoding crate
explicitly enables the feature; tokenizers still uses its exact existing Onig
backend. This does not switch to a regex iterator that can suppress errors.

Check actual C allocations through `onig::Region` in the existing isolated
allocator probe, including growth outside its original VM scope, free/refund and
oversized C allocation termination. Run exact full-token/template encoding and
existing contained model/Recipe integration against the resulting retained worker.
The shim's tests separately check overflow, alignment, zeroing and reallocation.

This closes an allocation-accounting bypass only after those checks pass. It is
not an OS RSS limit or proof of all native/runtime residency: allocator metadata,
retained arenas, stack and other native libraries still need the documented
non-VM reserve and platform containment/qualification. No cache, model-design,
answer-quality or serving activation evidence follows from this change.
