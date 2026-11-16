# Executable start custody — scoped acceptance, 2026-11-16

Baseline Main: `3a670a5ac37d32895b2c80bfb6445fa0dc0bfe9c`. The
[owning follow-up](../plans/subplan_step_ci_executable_start_of_bob1.md) repairs
the observed native utility spawn error and the independent package CI watchdog.
This does not certify all CI, platforms, component approval or Bob completion.

## Actual implementation

One private Monty transport helper handles only `ExecutableFileBusy` before
exec succeeds. It waits one documented Tokio timer tick under the accepted
absolute deadline; all other errors return their original cause. Once a child
starts, this start path does not retry it. The known executable,
environment clearing, pipe setup and kill-on-drop remain unchanged. There is
no new task retry, process count quota, budget, permission or alternate VM.

Utility start and its framed operation share the existing response deadline.
Global start and Boot share the smaller existing startup/IPC bound. Public
exchange signatures and ordinary IPC budgets stay unchanged, as does the
separate termination/reap custody.
The pinned Tokio 1.54.1 source polls the inner future before checking its timer;
the final callers therefore check the deadline before sending and after decoding,
rejecting a ready-but-late response. Global spawn errors now retain the real OS
cause through `Error::source` with kind/errno-only public formatting.

## Real kernel cases

Build job `a6148bb9-53be-4b0a-ac47-7a35945250ff` collects the actual native Linux
Monty-host library test executable, SHA-256
`998f52d9a9877a4fabae58b2aa51cb648a4bc25611d2fe2638d014dbf9a393c2`,
manifest `fb8a752894c694af2852434e98acbbaa19cdf5a78180f3bebb1258f168f74a66`.
It compiles but does not execute these cases in Modal. The original helper/test
module is unchanged in the final source; the later caller deadline guards are
separately compiled/linted below. No claim that the older complete library
artifact includes those later guards is made.

The exact collected executable is transferred to the already authorized
`brassclaw2`, checksum and native shared libraries verified before execution.
No compilation, application/provider operation or database runs on that host.
All six exact registered cases execute on its Linux 6.12.116 kernel:

- A real successful child returns its actual listing.
- A missing executable retains NotFound immediately.
- An already expired deadline starts nothing.
- An actual writable file handle prevents exec; release allows one child.
- Permanent write contention reaches its deadline with the actual OS cause.
- Cancellation before exec leaves no detached future/late start.

Result: **6 passed, 0 failed, 0 ignored**, actual SSH/test exit 0. Acknowledged
process completion precedes removal of the owned temporary resources. Results,
exact remote script, native log and identity remain in
`target/validation/ci-executable-start-native-accept-20261116-r1/`.
The earlier diagnostic that did not reproduce ETXTBSY inside Modal remains a
failed diagnostic; it is not substituted for the native-kernel evidence.

## Connected callers and final source

Job `41e92fd8-ac6f-4e01-870b-114e28f5bfea` compiles the actual standalone
`tests/monty_control` utility/process consumers and their real worker on the
existing warm cache. Actual Cargo JSON, package/src/profile checks, ELF/shared
library verification and complete test registration precede execution.

- **utility_host: 14 passed, 0 failed/ignored**.
- **process_host: 33 passed, 0 failed/ignored**, including the new real missing
  global-worker diagnostic case and existing startup deadline, reaping, memory,
  cancellation, private input and retained admission cases.

Manifest: `cd6113ca91213df7cf4902149c0dcd5715c0473c46d3a5c0d31ea048f9cb9efa`.
Actual results/artifacts and the reviewed runner/delta remain in
`target/validation/ci-executable-start-consumers-20261116-r1/`.
Runtime failures would be collected as failures, never synthesized as success.

Final strict Monty-host all-targets Clippy passes with `-D warnings`, job
`5325fae3-c34d-45a3-bbb6-d4787120fc67`, manifest
`c84df4c9f6e0ab4385f5e0b0f7e15f76f1b97029e19aeffe1057568589343bb4`.
The five current changed Rust inputs match both final build/lint manifests.
The earlier strict result is preserved but does not cover the later guards.
Formatting and whitespace checks pass. Each completed Screen exits 0 without
signal/error, final IPC is accepted and actually received, and its socket is
absent in the recorded private socket directory.

There are **53 scoped actual cases**, not a full-platform/CI qualification.
The native-kernel helper cases and container-hosted caller cases have distinct
honest applicability; none is silently skipped or counted as another platform.

## CI launcher source check

The package command no longer has its 28-minute Cargo watchdog or its unmeasured
30-minute job override. The hosted provider boundary remains. Existing package,
feature, all-target and test arguments remain; Cargo honors the checked-in lock.
Feature arguments use a Bash array, including zero arguments for empty helper
output. The old unquoted splitting and SC2086 suppression are removed.
`actionlint .github/workflows/reborn-tests.yml` (including installed ShellCheck)
passes. Other jobs and application/test/communication deadlines are unchanged.
The next actual GitHub result remains required; push and syntax acceptance are
not a passing workflow receipt.
