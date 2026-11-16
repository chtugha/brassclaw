# Run/REPL storage transition — 2026-11-16

The implementation follows L-01/L-02/L-03 of
[the Unnecessary plan](../../unnecessary_stuff_v3_at_bob1.md). The original audit
identified a complete services-input replacement, not only a missing policy.

`RebornBuildInput::into_postgres_with_reborn_home` consumes the resolved input
and replaces only its LocalDev storage. It preserves mount overrides, authority,
identity, authentication/OAuth and technical/runtime fields without a second
transfer list or environment resolution. Missing policy, Disabled and existing
Postgres inputs fail explicitly. The CLI retains its outer runtime input and
applies only its existing explicit tenant assignment.

The Run helper no longer calls the general migration runner. The existing
Composition factory still migrates before its first schema consumer. Serve's
earlier migration remains necessary for its pre-runtime ACL seed and is unchanged.
The Connection plan now records the existing parser/TLS/pool owner and Event-Store
delegation, without claiming all callers/capacity/lifecycles are accepted.

The adjacent invalid-host-home path exposed a real cleanup gap after PostgreSQL
startup. Runtime-input handoff/construction now completes in one fallible scope;
on failure the caller explicitly stops only its own server. A failed ordered stop
is reported and preserves the unresolved cluster instead of invoking immediate
Drop termination. External/reused servers retain their ownership semantics.
This does not change the global ManagedPostgres lifecycle or test injected failed
stops. See the [bounded subplan](../plans/subplan_step_l01_startup_cleanup_of_bob1.md).

## Actual CLI acceptance

Original job `094c40b5-5ae7-468e-8bf5-58d64b22d1ac`, App
`ap-N8d87iSCHy5pmlRpHDE3hX`, environment `brassclaw`, immutable image
`im-vQVW6Uxx7d4t1xtksm5dhN`; Rust 1.100.0, native Linux x86_64 GNU.
Source manifest SHA-256:
`9f6e5a607b97b8991709cc08c02bad2a1ed3434da848c1a154d38b10e2d8f1bf`.

The actual Cargo command was:

```text
cargo +1.100.0 test --locked -p brassclaw_reborn_cli --features postgres,skills-db --lib --test smoke --target x86_64-unknown-linux-gnu --no-run --message-format=json
```

The normal companion worker was built through its existing Cargo target first.
The actual registered tests ran as UID 65534 with isolated HOME/TMP, no inherited
database endpoint and real dedicated bundled PostgreSQL. All six passed, none
failed or ignored:

- `runtime::tests::run_postgres_transition_rejects_missing_contract_before_server_start`
- `runtime::tests::run_postgres_transition_failure_closes_owned_server`
- `repl_exit_command_exits_cleanly_without_touching_v1_state`
- `repl_confirmed_host_access_exits_cleanly_and_closes_owned_postgres`
- `repl_invalid_host_home_rejects_and_closes_owned_postgres`
- `run_rejects_occupied_explicit_mcp_port_without_reply_stdout_or_abrupt_pg_shutdown`

Positive REPL cases keep Full/confirmed LocalYolo, start from fresh unmigrated
state, reach actual runtime readiness and exit without a model call. Negative
cases exercise actual failures before/after server creation. The owned pidfile,
listening port and absence of the immediate-Drop fallback are checked together.
No production Rust-loop fallback or fabricated approval/result was added.

Collected product SHA-256:
`d672432856228c487edc7d32bdac2e412b600c4a9bf43e1cf005292a44473e0b`.
Smoke executable SHA-256:
`9bfe81b99509f10eb53919596c4b7a503d86e432da6fc39f482b619317ca4f8b`.
Normal worker SHA-256:
`27c8c78ac5662f51b21e5ad094528e71843eb5d64ea51c9880b4a68d4a7b1f11`.
Original request/source, Cargo JSON/log, immutable completion and artifacts are
retained under `target/validation/unnecessary-cli-native-20261116-r2/`.
The Screen ended with exit 0, no signal/error; final IPC was accepted, actually
received, and the session disappeared from its recorded private socket directory.

The broader Home-fixture change also warranted the remaining smoke cases. Job
`34cbea74-e285-48e3-8514-87eb6dfb56e1` verifies and directly executes the same actual
native Cargo artifacts, under the original CLI volume lock. All 68 remaining
cases pass, zero failed/ignored, with the four already accepted smoke cases
explicitly filtered and accounted for. Thus all 72 registered smoke tests have
passing results, in addition to the two CLI unit cases above. No Cargo command,
source sync, duplicate executable download or repeat of those four cases was
needed. The original source/worker/product hashes remain unchanged. Logs and
immutable completion are retained under
`target/validation/cli-remaining-smoke-20261116-r1/`. That Screen also ended with
exit 0, no signal/error and confirmed final delivery/session closure.

## Completed plan acceptance and separate CI scope

Both updated Composition input-contract cases pass from shared native build
`cea2ea67-fb8c-460e-a5e3-503b58ac1588`, source manifest
`4079bfe4f4a613ff796b5ee72551206ce7f2880c6116a80a8e90bc4af85958a8`:
`712a81eb-0837-49b0-bef9-0233df0f5674` preserves the exact resolved contracts,
including LocalSafe with no host-home; `af4c8596-1fe6-4bc0-9c08-62e0a56247a0`
rejects missing policy, Disabled and existing Postgres. Both execute the actual
registered native tests with no failures/ignored cases. The executable SHA-256 is
`cbd649299090f9e2317d1cf97cc7acf94bfc078476edee39d33a84e02f6844a0`.
Their immutable results are in
`target/validation/ci-composition-accept-20261116-r3/`. These complete L-01's
remaining exact-body acceptance; L-02 shares the real startup proof and L-03
requires only its completed source/documentation review.
Strict all-targets Clippy for CLI and Composition with `postgres,skills-db` passes
with `-D warnings`: job `493927ec-5e65-493e-abf2-c229a256b08e`, manifest
`b3ed9234b09f666b5f209180dc75cc170bf83e34d6a455c4caa1fc982acad247`.
That captured source includes the latest CLI caller and Prefix-test repairs;
the subsequently discovered prerequisite-I/O and Pure fixture corrections have
their own updated build/lint acceptance. Updated strict job
`6b61210e-d964-4719-b6e7-efb990cae7b4`, manifest
`988929e0db0fc0a9c886f2fc9cd5dc849fae39e755a1462199849212dbd912de`,
passes for its recorded thirteen-file repair snapshot, including the initial
I/O correction. The final-boot reader follow-up is a later private change, not
covered by that earlier run. The CLI and Input source in this receipt remains
unchanged and accepted. The changed
Rust files pass formatting; shell syntax/ShellCheck pass for the independently
repaired CI partition launcher. The CLI-PG subplan's R.1 code sample matches the
actual helper exactly.

Other native CI cases exposed separate fixture and aggregate I/O deadline gaps;
their actual failures remain retained and are repaired under
[their own subplan](../plans/subplan_step_ci_prerequisite_io_of_bob1.md).
These six passing CLI cases establish neither full CI nor macOS/Windows runtime
acceptance, general Linux-mode activation, full authored approval or completed
Bob plans. No unchanged successful case needs replay merely for documentation.
