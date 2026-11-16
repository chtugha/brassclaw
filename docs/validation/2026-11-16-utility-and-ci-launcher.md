# Utility diagnostics and CI launcher — 2026-11-16

`UtilityError` retains the actual `io::Error` when the worker cannot be spawned.
Display/Debug expose only its kind and OS code; the error source remains
available without printing the private request/source. Other failure kinds and
containment/reaping behavior are unchanged. The real missing-executable regression
checks NotFound, original-request retention and absence of private input/path in
the public diagnostic.

Original native job `456db112-d304-44bb-a8c9-0b9d984520ee`, image
`im-vQVW6Uxx7d4t1xtksm5dhN`, environment `brassclaw`, Rust 1.100.0/Linux x86_64:

```text
cargo +1.100.0 test --locked --manifest-path tests/monty_control/Cargo.toml --test utility_host --target x86_64-unknown-linux-gnu --no-run --message-format=json
```

All fourteen registered native cases pass, zero failed/ignored. This includes
real utility deadlines/fatal allocator containment, parser non-execution, hostile
typed values, scope isolation and output rejection. Exact source manifest:
`d6dd068b6c04ed03f0b03a3f92d34e41233851e49df89a05fda9ab6971fcaa76`.
Test executable SHA-256:
`1327da04ce928520a9c6f1d014cfd129e57d45c26f2683e8c316b1eea14ee3a8`.
Actual companion SHA-256:
`9e2152bfce4cddbf00d672632f461bc3842b7a6e35139979339f4e7648bf3fb2`.
Request, source, real logs and immutable artifacts/results remain in
`target/validation/parallel-repair-batch-20261116-r1/ci-utility/`.

Composition's module declaration order is corrected by rustfmt. The root CI
partition launcher keeps exactly its discovered test targets/Cargo commands,
but removes the outer 12-minute timer that counted compilation against a test
deadline. Test/runtime-specific deadlines and the workflow provider limit remain.
Changed Rust formatting, Bash syntax and ShellCheck pass. Full-feature
CLI/Composition all-targets strict Clippy passes in the recorded repair snapshots;
see the [CLI receipt](2026-11-16-unnecessary-cli-transition.md).

This is scoped native Linux utility and launcher/source acceptance. It does not
fix or qualify the separate original-journal/aggregate-deadline findings, certify
macOS/Windows runtime behavior or activate a general remote-test mode. The
connected [I/O subplan](../plans/subplan_step_ci_prerequisite_io_of_bob1.md) remains
private/in progress at this checkpoint; no full GitHub CI result is claimed.
