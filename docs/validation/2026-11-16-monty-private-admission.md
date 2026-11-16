# Standalone Monty private-admission regression — 2026-11-16

Base Main: `989e71d67f8638d2059b85f7cc8298371500ae16`. The changed input is
`tests/monty_control/tests/process_host.rs`; production execution is unchanged.
The previous CI's next admission tried to resolve a waiting coroutine while
another real host call was still the current interpreter boundary. `resolve_inner`
correctly requires `ResolveFutures`; `defer` converts that actual call to a pending
future without executing a host operation or returning success.

The corrected test retains the private call's key, checks duplicate-run refusal,
defers it, and defers each subsequent actual chat intent call before the next
admission. Every original key remains outstanding, all accounting stays on the
same global root/process, and the probes are explicitly terminated and reaped.
No Recipe result, model reply or durable completion is invented.

## Actual execution

- Modal environment `brassclaw`, App `ap-DGdvgqoAppnQt6Xs5bdgED`, original job
  `25831316-9d16-4302-87c3-7ad97cee517f`, immutable image
  `im-vQVW6Uxx7d4t1xtksm5dhN`, derived from the existing Rust 1.100 image.
- Actual `rustc 1.100.0 (0ef1bc386 2026-11-09)`, native Linux x86_64 GNU.
- Built `cargo +1.100.0 test --locked --manifest-path tests/monty_control/Cargo.toml
  --test process_host --target x86_64-unknown-linux-gnu --no-run --message-format=json`.
  Selected the real Cargo artifact by manifest, package, target/source and test
  profile, checked native ELF identity and resolved shared libraries.
- Ran that actual binary in the same container as its Cargo-generated worker
  executables. Its discovered registry contained 32 tests; all 32 passed, none
  failed/ignored/filtered. Test execution: 4.73 seconds; actual Cargo compilation:
  5m28s. No PostgreSQL server was started by this target.
- Exact source-manifest SHA-256:
  `1ba1bdcba3830d5f867225e5fdb680316b1a06ae663cefdea97f20db7c8bd911`.
- Test executable SHA-256:
  `8cc77f1d7675e2a07b2f9534b505040cd5f13d81c47cf3adc553451b6fbb14c1`.
  Actual worker SHA-256:
  `13643192d8ca4d6e091089b78728ee340045b2f46f2288830b8a482d01015bc7`.

The first Screen ended nonzero **after** successful remote tests because local
artifact collection would cross the 90% disk-use gate. Its original completion
remained collection-pending. A separate read-only collection recovered those
exact bytes to SSDE using the existing verified collector, moved the already
downloaded artifact instead of duplicating it, and resolved the original job.
No new compute, rebuild or test replay was submitted. Both Screens ended; their
final notifications were accepted and actually received.

## Manifest diagnostics and limits of the claim

The build emitted three Cargo manifest warnings about `homepage` duplicating
`repository`, in the vendored `monty`, `monty-types` and `monty-alloc` packages.
Removed only those redundant metadata fields. Actual local Cargo 1.100
`metadata --offline --no-deps --format-version 1 --manifest-path
tests/monty_control/Cargo.toml` then completed with empty stderr. This metadata
repair changes no executable sources; it does not require replaying the passing
native tests. No compiler diagnostics were emitted and no warnings were silenced.

Original request/source manifests, real logs, immutable completion, exact native
artifacts, reviewed fixed-target runner and separate collection/metadata acceptance
are retained in `target/validation/prepared-ci-admission-20261116-r1/`.
This is a scoped explicit native qualification, not activation of a general Linux
test mode or proof of the macOS/Windows process backend. The ordinary activated
Apple builder remains unchanged. Other CI failures retain their separate scope.
