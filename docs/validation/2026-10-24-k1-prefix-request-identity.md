# K1 — Exact prefix request identity, 2026-10-24

K1 in the new opening conformity block of `simplified_v3.md` is implemented and
accepted for the existing native PostgreSQL/global-Monty consumers. K2–K7 and the
regular remaining plans are still open. No release or testmachine deployment.

## Change

Both existing-definition and concurrent insert-winner paths compare the complete
explicit input, request metadata and original custody scopes. An explicit changed
or empty snapshot conflicts before admission/effects. Automatic source capture
reconnects use an original lookup, without selecting a new compiler or source.
Exact retries retain the original program, input, definition, thread and run.

Typed Conflict/Unavailable errors survive the runtime and operator API: conflicts
are HTTP409, uncertain storage remains unavailable instead of becoming HTTP400.
Measurement requests already retained all seven explicit fields; their negative
HTTP cases now verify each field, with no additional dispatch or replacement run.

The existing actual component-deployment caller exercises the changed path. It
cuts the real reservation COMMIT acknowledgement using the existing PostgreSQL
wire transport, checks the persisted original before admission, and resumes that
same job. A separate real SQL table lock forces two competing INSERTs through
the winner comparison. Its observer uses a distinct autocommit connection and
`pg_blocking_pids`, avoiding a cached transaction-local activity snapshot.
All application readers in this caller share the actual transport pool, preserving
the embedded server's aggregate connection capacity without increasing a cap.

## Observed validation

Evidence: `target/accepted-evidence/k1-prefix-request-identity-20261024/run-3/`.

| Check | Result |
| --- | --- |
| Affected composition test build, actual common features | Passed, 177.365 s |
| Architecture selectors against the current tree | 4 composition + 28 dependency tests passed |
| Existing native component-deployment caller | 1 actual test passed, 354.088 s |
| Root/composition strict common-feature Clippy, tests | Passed, 71.278 s |
| PostgreSQL-default composition library strict Clippy | Passed, 13.582 s |
| Format / whitespace / final source and artifact identities | Passed |

**33 actual tests**, not a count of helper assertions. The existing unchanged
architecture executables and worker were hash/source qualified before reuse.
Cargo ran serially in `screen`, inherited the configured canonical target and
retained needed executables before mandatory targeted cleanup. No regular
application executable was built or claimed current by this package.

The native caller includes actual compiler/source-capture, immutable model
artifacts, independently pinned Hugging Face 5.20.2 references, native encoding,
global durable measurement, operator deployment and retained selection cases.
The core source job carried 1,693,651 snapshot bytes. Release and Sempai captures
contained 319 source records; each used five native calls without replay or
activation. Actual durable measurements used two native calls, preserving the
same result on reconnect and conflicting changed requests.

The full Sempai diagnostic input exceeds the model's declared 262,144-token
context (largest probe: 371,462). The reference tokenizer's warning remains in
the native log, and the observed capacity is false. This does not qualify a
serving prefix, imply successful inference, truncate sources or raise a model
limit. Accepted design/whole-unit selection and the final serving guard remain
with the Prefix plan; this package does not close them.

## Exact identities and retained failures

- `accepted.json`: `81d2153030ca18217d67960c90553229201b6885b101a4ba2f5bf4edfcc07ad5`.
- `source.json`: `1ab1bbaadfe0815ba8e89b47e84739f85365fb71e1b28449e85c8f0a74dd1267`.
- Actual composition test executable: `07c47c09c3d15db06df033888e432d8cb5fcf8f74035b0eeffc94cc9dede6294`.
- Adjacent retained worker: `d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

Every runtime source and all 270 declared release documents were reverified
before the next package. The receipt, source inventory, actual runner/formatter,
logs, model originals/reference lock and executables remain retained. Progress
notes and unrelated scanner content are explicitly outside the embedded package
claim. Subsequent documentation updates do not rewrite this receipt's source.

Run-1 remains a failed test-observer attempt. Run-2 retains its successful identity
and source-job helper observations but failed the later model-catalogue GET after
a second application pool was introduced. Run-3 fixes that pool ownership and
passes the complete actual caller. Neither failed run is presented as acceptance.
The prior private pure-source review has its separate scoped report; no Q1,
behavioral approval, public activation, general recovery or full-v3 completion
is established by these development checks.
