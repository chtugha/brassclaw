# Original DCR custody, lifecycle and native filesystem acceptance — 26 October 2026

Status: **accepted for the bounded scopes below; K3 and all full plans remain open**.
The former private Auth/DCR package is integrated in the common repository. Its
private source directory is a historical pre-integration snapshot, never an active
checkout or build source. Prefix implementation remains owned by the other chat.

## Evidence and applicability

Machine receipt: `target/validation/auth-dcr-common-20261026/run-10/accepted.json`.
The receipt verifies **342 distinct passing Rust cases**, with zero ignored cases
in the selected executions:

| Scope | Cases | Actual evidence |
| --- | ---: | --- |
| Auth units/product/boundary | 92 | R7 successful executions; identical current Cargo-emitted R9 executable hashes |
| Secret units/DCR/store contracts | 83 | Same exact executable reuse from R7 |
| Filesystem units and contracts | 106 | Same exact executable reuse from R7 |
| Original Auth setup | 2 | Fresh R9 execution |
| DCR registration, cleanup and legacy continuity | 32 | Fresh R9 execution |
| Native Secret owners and index creation | 14 | Fresh R9 execution with actual PostgreSQL and encrypted persistent filesystem |
| Credential lifecycle | 8 | Fresh R9 execution; actual native PG and CAS filesystem contracts |
| Protected HTTP and combined native Chromium | 5 | Fresh R10 execution against the exact R9 Rust image |

R9 strict Auth/Secrets/Filesystem/Composition test Clippy passed with `-D warnings`.
Its exact common compiler inputs, toolchain, emitted image/worker identities and
logs are retained under `run-9/`. R10 changes only the external Node browser
script; it verifies that no Rust or embedded input changed and runs the archived
image without another Cargo invocation. Its script bytes/hash, original runner,
73 owned source files and applicability manifest are retained under `run-10/`.
This is frozen-input acceptance, not certification of later concurrent changes.
All failed R1–R9 receipts remain intact; successful scopes are individually
qualified, never relabeled as successful whole runs.

## Implemented boundary

- First DCR preparation, registration dispatch claim, full encrypted registration
  response, selected client/PKCE material and cleanup disposition belong to the
  same original Auth flow. Unknown acknowledgements do not select a replacement
  flow or repeat a registration POST. Modern and genuine pre-custody selections
  retain actual inputs, first expiry, scope and original account binding.
- Remote management credentials are read only from the exact retained issuer
  response and checked against its original HTTPS origin. Malformed client
  results can still retain genuine management evidence. Hostile destinations
  receive no management bearer. Only an empty 204 confirms deletion; a rejected
  or unknown DELETE retains its original outcome and is never automatically
  repeated. Completed accounts retain their registrations.
- Local retirement stores complete original slot selections, authenticated
  closures and unresolved/confirmed dispositions. Absent identities use
  nonmaterial tombstones, not invented value references. Native lost-plan ACK,
  partial closure, restarted owners and actual SQL-barrier alias refinement are
  tested. Exact prior plans remain reusable after valid refinement.
- Legacy callback/client/refresh material is sealed from actual prior values,
  never fabricated issuer replies. Cleanup fences only captured generations;
  a newer unselected value survives. Original callback/account/refresh continuity
  is exercised through native PG and persistent filesystem owners.
- Credential lifecycle uses the existing stable owner selection across later
  invocations/surfaces, retaining declared owner/project/thread/session boundaries.
  Current grants/ownership are rechecked at the actual account mutation. PG closes
  selected secrets and records receipts in its account transaction. CAS filesystem
  publication records intent before closure, retains handles through unresolved
  outcomes and clears them only with confirmed receipts. Later owners reconcile
  the same selections. Deactivation cannot undo Revoked. These CAS lifecycle tests
  use InMemoryBackend; they do not claim a whole-process persistent-FS restore.
- Account/flow owner enumeration no longer silently loses large sets or rejects
  the former 1,024-record/session-root ceiling. The tested case covers 2,050
  accounts and 2,050 flows, including 1,025 session roots. Duplicate account IDs
  fail before secret effects instead of being silently deduplicated.

## Native filesystem repair

Concurrent first use exposed a real PostgreSQL `ensure_index` race. A successful
declaration insert did not serialize the later `CREATE INDEX IF NOT EXISTS`.
Declaration, canonical-spec row lock, physical DDL and commit now share one
ReadCommitted transaction across independent adapters/processes. Failed DDL
publishes no declaration. Tests cover first/repeated Exact/Prefix/FTS creation,
incompatible declarations and a deliberately rejected native DDL.

SQL index names hash the complete unsanitized prefix/name identity before any
lossy shortening. Short punctuation, path-separator and long-prefix aliases have
actual native coverage. Historical physical indexes are preserved; they are not
dropped under potentially shared old names. No applied migration was rewritten.

The added index transaction changed the location of an old test's generic
"next COMMIT" fault. The corrected Secret fault selector requires the actual
encrypted entry and exact handle in a Bind before cutting its COMMIT. Reads and
index initialization cannot satisfy it. The intended lost-secret-write outcome
and original readback assertions remain unchanged.

Seven lifecycle test lookups also omitted their ExtensionOwned requester. They
now supply the exact owning extension; production authorization was not weakened.

## Actual browser scope

The combined test uses Chrome for Testing 149.0.7827.54 through pinned
agent-browser 0.27.1, the actual OAuthOriginalSetup component and common JS, the
Auth HTTP route/facade, native Auth manager and encrypted PgSecretStore. Native
persistence and HostOAuthProviderClient use the same separate port construction
as the production factory. Issuer egress is controlled; the component fixture
explicitly simulates a blocked popup and never visits the real issuer.

The first successful start acknowledgement is cut only after the native operation
returns. Actual browser statuses are `[503, 200, 202, 200, 200]`: lost start reply,
same-original retry, uncertain DELETE, same-original reconciliation, then readback
after reload. A 200 readback does not turn the displayed unconfirmed deletion into
success. The script checks first expiry/IDs, retention before transport, redacted
sessionStorage, the safe explicit authorization link and visible unknown cleanup.
Independent Rust assertions require exactly one POST and one DELETE per original
and both actual canceled flows in PostgreSQL. The browser marker and all five
HTTP results pass in 9.02 seconds.

R9's script incorrectly expected 202 for reconciliation and used an inexact reload
label; its failed result and live audit observation remain preserved. R10 corrects
those assertions and bounds each CLI process with the explicit configurable
`BRASSCLAW_BROWSER_COMMAND_TIMEOUT_MS` (30,000 for this check). This is a test-harness
deadline, not a product setting or a whole-goal deadline. It invokes the pinned
native CLI directly so termination does not leave a Node-wrapper child waiting.

## Remaining work in the existing owners

Continue K3's existing sequence in the implementation journal. Account-lifetime
reclamation of retained client/source dependencies, broader terminal artifact
disposition, issuer access expiry through final injection, requirement-specific
grant/selection, Codex session setup, remaining protocol/settings/caps and complete
K3 acceptance are not closed. K6 still owns pending CAS-plan transfer/restore and
safe upgrade fencing. Full Extensions-page/actual successful issuer callback,
global startup, installed release and Linux/Windows acceptance are not established
by this component/native test. Prefix accepted-design/serving and its independently
failed bootstrap remain the Prefix owner's work; this package neither bypasses
that gate nor adds a Rust workflow fallback.
