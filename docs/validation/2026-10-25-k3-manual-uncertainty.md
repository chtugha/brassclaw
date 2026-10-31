# Manual submission uncertainty — 2026-10-25

**Scoped acceptance: 353 distinct passing Rust cases: 85 fresh final-run
executions and 268 explicitly qualified results, with current strict/default/fmt,
whitespace and source integrity. Ten applied source/test/embedded files match.**

The generic manual completion coordinator no longer cancels a flow or revokes its
acknowledged account after an unconfirmed completion. PostgreSQL delegates that
same coordinator. Both public combined/split submit routes reuse the existing
backend error/timeout mapper without automatic abandonment. Invalid split input
also preserves the original pending interaction. Explicit operator abandonment
remains separate. Failure still returns an error; no unconfirmed result is called
success or automatically replayed.

The private coordinator drops its no-longer-needed account-service field and both
actual facade constructors are updated. No public workflow port, Recipe, Tool,
crypto, VM, store, queue, authority or Rust task execution loop is introduced.

## Evidence and limits

Receipt: `target/accepted-evidence/k3-manual-no-compensation-20261025/run-2/accepted.json`.
SHA-256: `adde369f072cc5b3fe0a9af0879f3c20284b9d549909cead66f5965830bb4b03`. Source inventory, exact commands/features/environment,
retained executables, failure logs and qualification are preserved there and in
run1. The configured Cargo target and Screen/lock were used serially.

| Cases | Count | Applicability |
| --- | ---: | --- |
| Actual public product-auth routers | 83 | fresh final run |
| Ordinary global Monty/current packaged content | 1 | fresh final run |
| Architecture dependency boundary | 1 | fresh final run |
| Memory/native PG/persistent FS setup and completion | 3 | exact run1 result |
| Native Auth/custody, FS, Host, DCR, multiplexer and HTTP helpers | 110 | exact run1 results |
| Callback/lifecycle/manual facade | 22 | exact run1 results |
| Independent Auth/GSuite/Workflow | 133 | prior accepted exact-source results |

Each of the three actual store tests retains its six original setup readback
outcomes and adds completion rejected before writing and acknowledgement lost
after the real completed flow. These are 24 scenarios within three Rust cases,
not 24 separately counted tests. The real account remains configured; rejected
completion retains the original pending flow, and a completed flow retains its
actual account ID. A unique label explicitly scopes these new completion cases
to newly created accounts. They do not qualify bound/automatic account selection,
original raw-input comparison or full multi-record publication.

The new public split case submits empty/padded/control input against a real
pending memory interaction, then successfully submits valid input to that same
original. The other new public case loses a reply after the actual memory owner
has written the credential: one backend-error path and one actual 30-second
product timeout. Both retain the real configured account and show zero abandon
or continuation effects. These HTTP tests use the actual memory owner; the
separate native matrix establishes the persisted completion boundary. Neither is
a claim of physical network-COMMIT fault injection for this package.

Run1 compiled without a compiler error and passed the native/custody/facade cases,
then the new timeout test exposed its wrong expected HTTP status. The existing
contract returns 504 for timeout and 503 for backend failure. Only that expectation
changed. The full corrected public target reran; the split/global/boundary cases
also ran freshly. The other compiled targets have no dependency on that separate
integration-test file and retain exact inputs. No failed case, warning, skipped
backend or zero-case result is suppressed or counted as success.

Final strict affected/root tests: 76.50s; default: 14.61s;
format: 4.63s. Times are observed validation durations, not new deadlines.
Previous accepted secret-generation/callback/setup/refresh/Network and Prefix
successors are preserved. The worker retains 529 exact inputs plus the separately
qualified Network lock dev-edge delta; no parallel Cargo, retired target2, remote
compilation, Git/release or SSH operation occurred.

Full K3 step2 still owns encrypted immutable original manual input, exact initial
account selection/result, atomic or CAS account/interaction/flow publication,
lower-store cleanup, unsafe latest-account recovery and actual browser retry
identity. DCR partial-artifact disposition follows in the same step. Issuer
expiry, setup/original grants, Codex and K4–K7 remain open. The server/browser
30-second constants remain explicitly in the existing K3 step6/audit; testing the
current timeout does not make it adjustable or establish no effect after abort.
This is not full-plan, platform-wide, release or deployed acceptance.
