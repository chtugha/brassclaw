# CI native executable start and package runner — 2026-11-16

Baseline: Main `3a670a5ac37d32895b2c80bfb6445fa0dc0bfe9c`. The preceding
original-verification subplan is accepted and published. Implement this bounded
follow-up before returning to new B1 work. No new workflow, approval owner,
global VM, task replay, platform roadmap cutover or provider configuration.

## Evidence and limits of the diagnosis

GitHub Tests run `43833823308`, Composition job `133185748813`, reports
`ExecutableFileBusy` / OS error 26 from actual retained-source utility spawn.
Its enclosing Cargo command is also killed at 28 minutes, after 17m38s compiling.
The 28-minute command and 30-minute job settings originate in `ab5a907628`;
the existing historical implementation journal records no measured justification.
Do not count cancellations or this command timeout as passing tests.

`PackagedExecutableImage::stage_packaged` already creates a unique private copy,
checks bytes, syncs it, sets permissions and closes its writer before publication.
Do not weaken that custody or execute a mutable original instead. Linux exec can
still be blocked by a writable descriptor inherited during another thread's
process start. An owned, warning-free subprocess/descriptor probe on the already
authorized `brassclaw2` reproduces errno 26 while a child holds that descriptor,
then successful execution of exactly the same bytes after close. No Cargo,
application, provider or database was touched. The Modal container did not
reproduce the exclusion; preserve that failed diagnostic rather than claiming
identical host behavior or the exact GitHub interleaving was proven.

Evidence: `target/validation/ci-linux-executable-custody-proof-native-20261116-r1/`
and the failed Modal diagnostic in `ci-linux-executable-custody-proof-20261116-r1/`.
The first diagnostic's Python multithreaded `os.fork` warning is retained; its
successor uses an explicit subprocess with exact inherited descriptors.

Primary technical contracts:

- [Linux execve](https://man7.org/linux/man-pages/man2/execve.2.html) documents
  ETXTBSY for an executable open for writing.
- [Tokio Sleep](https://docs.rs/tokio/latest/tokio/time/fn.sleep.html) documents
  millisecond timer granularity. A named one-tick contention wait is scheduler
  resolution, not another task/resource budget or an arbitrary retry quota.
- [GitHub Actions limits](https://docs.github.com/en/actions/reference/limits)
  currently limits GitHub-hosted jobs to six hours. Keep that provider boundary;
  do not invent another compilation-charging Cargo watchdog.

## 1. One pre-exec start boundary

- [x] Add one private helper in the existing `brassclaw_monty_host` transport
  owner; both `utility::execute` and `GlobalProcess::start` use it. Inspect the
  actual constructors and all `ProcessError` patterns before edits.
- [x] Accept only the caller's already configured `tokio::process::Command` and
  one finite absolute deadline derived from the existing accepted limits. No
  shell, PATH search, caller-controlled executable, environment widening or new
  permission. Preserve piped I/O, cleared environment and kill-on-drop.
- [x] Only `io::ErrorKind::ExecutableFileBusy` may wait and try starting the same
  executable again. This is before exec succeeds and before any request bytes or
  Python run. All other OS errors return immediately with their actual cause.
  No successful start, partial protocol, worker exit, timeout after execution,
  Recipe or Tool operation is retried.
- [x] Yield via Tokio's named minimum timer tick, capped by the original absolute
  deadline. No busy spin, blocking sleep, attempt quota, extra lifetime or reset
  of the accepted policy revision. A permanently busy file must end boundedly.
  Cancellation while waiting leaves no successfully started worker behind.
- [x] Retain the actual last OS error and distinguish deadline exhaustion from
  ordinary spawn failure. Public formatting exposes kind/errno only; private
  source/input/path data stays redacted. `Error::source` preserves the OS cause.
  Do not discard diagnostics merely to keep the previous generic error text.

## 2. Keep one operation/startup deadline

- [x] Utility captures its absolute response deadline before starting. Its
  existing framed operation uses the remaining same deadline, not a fresh full
  timeout after contention. Existing separate termination/reap containment stays.
  Validation, typed request/result checks and one-operation protocol are intact.
  Inspect the pinned Tokio implementation: 1.54.1 polls the inner future before
  its timer. Check the absolute deadline explicitly before sending any bytes and
  before accepting a fully decoded response, so an already-ready future cannot
  bypass an elapsed shared deadline. Keep existing containment on that failure.
- [x] Global start captures the smaller existing startup/IPC deadline. Start and
  Boot exchange share it. Add only a private deadline-cap handoff inside the
  existing exchange owner; keep public exchange APIs and ordinary exchange
  deadlines unchanged. Do not start another VM or replay Boot after any success.
- [x] Preserve every error-construction path and consumer, including source
  chains, redaction and existing interruption/kill/reap evidence. This is not an
  approval, task attempt, durable recovery or Tool-retry policy change.

## 3. Real acceptance, no synthetic spawn success

- [x] Add focused helper regressions using an actual copied test executable and
  actual OS handles: ordinary start, immediate missing executable, held writable
  descriptor released after the pending start, permanent contention reaching its
  deadline, and cancellation before exec. Drive the first poll while the writer
  is held rather than relying on an arbitrary sleep to create the race.
- [x] Verify no successful child/body exists while the OS rejects exec. After
  release, require the real child exit/output; after cancellation/deadline, prove
  no delayed execution. No fabricated parser answer, VM receipt or error result.
- [x] Real write-exclusion cases require a native Linux kernel exhibiting that
  behavior. Compile once on the existing native-Linux Modal image, then run the
  unchanged collected test executable on the authorized Linux test host if the
  Modal kernel lacks exclusion. Do not claim those mandatory cases passed from
  Modal alone, silently skip them or compile on the test machine.
- [x] Reuse the same compiled worker for existing `tests/monty_control` utility
  and process-host regressions. Preserve all protocol/resource/cancellation/
  non-replay assertions. Inspect dynamic dependencies and exact artifact identity
  before execution; use only dedicated temporary files/processes, no existing DB.
- [x] Complete source review, formatting and strict affected all-target lint in
  independent locked build environments. Reuse unchanged original-verification
  evidence honestly; new startup/transport claims need their own real results.

## 4. Package CI watchdog, after the start-boundary package

- [x] Remove the actual `timeout --signal=INT --kill-after=30s 28m` wrapper from
  the crate-tests Cargo command. Preserve package/feature/all-target arguments,
  failure propagation, existing cache custody and all required tests.
  Use the checked-in Cargo lock. Replace the existing SC2086 suppression and
  unquoted feature splitting with a Bash array populated from the existing
  one-line feature helper; empty output must produce zero extra arguments.
- [x] Remove this package job's unmeasured 30-minute override as well, so it
  cannot immediately replace the removed command timer. The documented hosted
  provider limit remains; do not replace it with another guessed duration.
  Other jobs and actual application/test/communication deadlines are unchanged.
- [ ] YAML/ShellCheck review passes; inspect the subsequent actual GitHub result. A validated source commit or push is not a passing CI run.
  Do not blindly retry failed/uncertain jobs or change unrelated matrix semantics.

## Completion

- [ ] Record actual source/artifact/platform applicability, preserved diagnostic
  failures, real case results, Screen delivery and closure separately. Update
  changelog/status, stage only accepted paths, review the complete staged delta,
  commit/push to explicit origin/Main and verify the remote SHA. Continue with
  remaining CI findings and then B1; this subplan does not complete the Bob plans.

Current scoped acceptance: six native-kernel helper cases, fourteen utility and
thirty-three process-host cases pass. Final strict Monty-host all-target lint
and actionlint/ShellCheck pass. See [the complete scoped receipt](../validation/2026-11-16-executable-start-custody.md).
Publication and the subsequent full GitHub workflow remain separate gates.
