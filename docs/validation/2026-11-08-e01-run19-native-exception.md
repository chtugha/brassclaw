# E-01 pinned native exception contract correction

Root Run 19 compiled the combined harness without warnings, then its first
prerequisite failed during startup with `checker native Python exception differs
from expected stop`. Read the terminal log at
`/Volumes/SSDE/brassclaw/target/validation/bob-native-repair-20261105/run-19-composition/prerequisite-1/common-startup.log`.
It contains no suite, expected line or actual final exception. This package did
not execute Cargo, DB queries, tests, Python or Modal.

## Established source cause and remaining observation gap

The existing E-01 `stop_document` expectation is wrong for the pinned worker.
All eight class checkers JSON-decode `{}` and then access
`_subj["candidate"]["revision"]["document"]`. In pinned
`vendor/monty-control/crates/monty/src/exception_private.rs`, `key_error` uses the
raw string key as the exception argument. `into_python_exception` transfers that
argument directly into public `MontyException`. Its Display implementation in
`crates/monty-types/src/exceptions.rs` writes the exception type and message
directly after the traceback. Thus the final public line is exactly
`KeyError: candidate`, without quotes. E-01 incorrectly expected
`KeyError: 'candidate'`. Internal `SimpleException::py_str` has a separate KeyError
repr branch; the worker does not use that branch to format its public exception.

The process worker in `brassclaw_monty_host/src/process.rs` copies
`error.exception.map(|error| error.to_string())` into the framed diagnostic;
the transport copies that diagnostic unchanged into `ProcessError`. There is no
additional quoting/normalization along this path. The correction changes the
shared `stop_document` fixture once, covering all eight checking roots.

This is a proven source-contract mismatch, not proof of which suite actually
failed in Run 19. The old error discarded that distinction. E-01 validates the
captured observation before persisting the observed record, so the rejected
exception was not retained by that path. No actual Run 19 terminal exception is
claimed from the generic log. Root's next prerequisite must confirm the runtime
outcome; if another mismatch exists, the new projection identifies it precisely.

## Same-family source review

- `stop_json` uses `!`: the pinned JSON loader maps ExpectedSomeValue to
  `Expecting value`; its coordinate formatter uses line 1, column 1, character 0.
  `ExcType::JsonDecodeError` displays as `json.JSONDecodeError`. The exact current
  expectation is retained.
- Every helper negative suite raises its explicit fixed `ValueError` message:
  mismatch, contradictory outcome, empty/bad matrix, duplicate slot/step,
  missing selected result (nullable default fails rule-object validation),
  foreign non-null result, numeric `ok`, and empty diagnostic. These messages
  are not KeyError arguments and receive no repr quoting from public Display.
  Their expectations remain unchanged; no arbitrary Python failure is accepted.

## Minimal controlled diagnostic projection

On exact final-line mismatch, the existing invalid-configuration error now
includes a JSON object with root UUID, fixture variant, expected final exception
and actual final exception (or null). Only the final line is projected, not the
traceback, command, source or full input. These are closed shipped fixtures with
fixed literals and no host/secret access. JSON serialization escapes control
characters. No logging side channel or temporary print is added.

All strict negative custody requirements remain: actual
`ProcessFailure::Vm(VmFailure::Python)`, the original Start command and pinned
export/source/checksum, typed input fingerprint, no successful result, exact
final exception line, terminal original admission and joined worker closure.
NameError, worker death, transport failure and cancellation still cannot satisfy
an expected stop. No fallback, replay or changed approval boundary.

## Source handoff

Only `bootstrap_checker_execution.rs` and `bootstrap_checker_fixtures.rs` change.
The sibling SHA-256 manifest supersedes their initial-package entries; previous
manifests remain historical. Existing claim identity hashes both owners, so new
bytes have new claim identities without rewriting old records.

Actual checks: connected source review above, rustfmt edition 2024 with
`skip_children=true`, `git diff --check`, SHA-256 generation/readback. No native
success claim. Root owns integration and prerequisite-first acceptance on the
combined source. Exact existing E-01 selectors and package boundaries remain
unchanged. Transfer this correction only over Root's accepted preceding E-01
commits, not the older branch base.
