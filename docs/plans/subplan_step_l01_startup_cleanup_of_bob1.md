# L-01 follow-up: owned PostgreSQL after a runtime-construction error

Concrete adjacent finding: after `start_postgres_for_run` succeeds, `execute`
still uses `build_reborn_runtime(...).await?`. A rejected Full host-home mount
therefore drops the owned server without ordered shutdown. `confirmed_host_home_root`
captures the explicit path; `factory::canonicalize_local_dev_host_home_root`
validates it later. An existing file used as HOME demonstrates this exact path.

1. Keep the resolved input, profiles and existing factory validation unchanged.
   Do not create/repair an operator's invalid home or select a weaker policy.
2. Evaluate the existing run-input handoff and runtime construction in one
   fallible scope. After it returns an error and releases its inputs, explicitly
   call the existing owned `ManagedPostgres::shutdown` before returning.
3. Preserve the original construction error and any shutdown error. Reused or
   externally supplied servers retain their existing no-stop ownership contract.
   Successful startup keeps the existing runtime/MCP/PG shutdown order.
   If ordered stop itself fails, preserve/report the unreconciled owned instance
   for explicit recovery rather than invoking the immediate `Drop` fallback.
   Apply that same rule at the run caller's existing input-error, MCP-error and
   final-stop branches; do not reform the global ManagedPostgres lifecycle.
4. Add one real isolated REPL rejection case: HOME is an existing file, Reborn
   home/PG port are fresh and owned, and the default Full policy remains selected.
   Assert the actual host-home rejection, initialized PG data, absent pidfile,
   closed port and no immediate-Drop warning. Do not fabricate a runtime failure.
5. Update the old CLI-PG subplan's R.2 example to describe the actual error path;
   retain the original completed PG hookup and the factory's migration ownership.
6. Build the changed CLI and execute this exact case through the existing native
   test owner, reusing cached dependencies and unchanged prior positive evidence.
   Review the relevant lint applicability; no whole workspace replay for prose.

This repairs the concrete product caller, not every factory's internal lifecycle
or the separate Simplified recovery cutover. Implementation and actual Linux
acceptance are complete: job `094c40b5-5ae7-468e-8bf5-58d64b22d1ac` built the changed
CLI and passed all six selected cases without skips. They include both successful
REPL policies, the actual post-start invalid-home rejection, the transition error
and the existing occupied-MCP-port regression. Each owned cluster is initialized
for real and verified stopped. No failed-stop injection or general PostgreSQL
Drop/lifecycle repair is claimed. Strict CLI/Composition all-targets lint with
`postgres,skills-db` and `-D warnings` passes in job
`493927ec-5e65-493e-abf2-c229a256b08e`. Separate later prerequisite-I/O repairs
retain their own validation; they do not invalidate this unchanged caller proof.

## GitHub source-boundary follow-up

GitHub job `133156451673` on `985f94d93` passes both 99-case unit targets, all
72 smoke cases and the other CLI integration targets. Its subsequent architecture
check rejects the real test's temporary-port listener in the inline test module.
The existing scanner already supports truly test-exclusive source files with
`#![cfg(test)]`. Move the complete existing `runtime::tests` module to sibling
`runtime/tests.rs`, retain its outer `#[cfg(test)]` declaration and add that inner
gate. Keep every test, qualified name, import, fixture string and assertion;
change no production statement, scanner rule or allowlist. Compare rewrapped
Rustfmt-normalized modules to prove exact preservation, then run the actual
architecture regression and compile/lint the changed test layout.

The normalized original and extracted test modules are identical (SHA-256
`0367d271870c5ebdddf88672aeb39c22968246da7b65cb7d5d3646fe846f8570`).
The original failure is retained in `ci-cli-architecture-20261116-r1`; corrected
local Screen r2 passes the exact `reborn_product_api_crates_do_not_bind_http_ingress`
case. Strict compilation/lint of the new layout passes in job
`87c8b6c5-87bf-40d0-8aed-71688e425e89`, source manifest
`d4bc57113fa0655765f827acfa121db9f53d3e61dbf8bb7c356a246adbd8dc87`,
using the existing all-targets CLI/Composition graph with `postgres,skills-db`
and `-D warnings`. Both Screens ended with confirmed final delivery. Existing actual
runtime results remain applicable to its unchanged bodies; no new HTTP host or
suppression is introduced.
