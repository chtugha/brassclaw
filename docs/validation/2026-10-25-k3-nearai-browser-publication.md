# Original NEAR browser login publication — 2026-10-25

**Scoped acceptance passed: 78 fresh Rust cases and19 UI cases. Full K3 remains open.**

The existing NEAR browser start now captures provider-definition revision, the
primary role's exact content reference and authenticated encrypted key-row reference
in one read-only repeatable-read snapshot. Its state and returned mutation ID bind
this original authenticated setup action. The callback prepares the retained
definition and commits credential, canonical role selection and original provider
mutation receipt together. Current provider/role/credential changes reject stale
completion. No session file or shared legacy session token is published by this
browser route. Existing instance authentication and the state-guarded callback
boundary remain; no Tool grant, catalogue, event queue or workflow runner was added.

The new conditional write belongs to the existing PgSecretStore. It shares trusted
writers' physical-owner/secret-identity lock, authenticates and locks existing
records, compares before legacy migration, and uses conflict-preserving insertion
for an originally absent row. A mismatched reference writes nothing. The original
provider lock/config/secret lock order and transaction rollback remain effective.

Exact committed callback retries authenticate their encrypted proof and read the
original receipt before fresh preconditions. Reconciliation uses current desired
configuration and cannot restore older credentials over a later edit. A newly
constructed settings-service owner with empty RAM state can read this committed
original. Uncommitted expired/missing states cannot manufacture another mutation.
That is bounded restart-readback coverage, not a cold process/installation test.

Callback and WebUI now retain one runtime-owned settings-service handle. Merely
attaching a surface does not republish a client. The UI waits for this login's
receipt and successful runtime reconciliation; an already active provider name
cannot acknowledge another login. Generation checks also fence late query refresh
and success navigation after unmount or a newer login. Codex retains its separately
tracked legacy controller/polling until its own migration.

## Actual checks and limits

Archive: `target/accepted-evidence/k3-nearai-publication-20261025/run-3/`.
Receipt SHA-256: `a9e47f92ff0c5afcbbd5703774619c74d1b4c9a88e166724e0fbc5fca9d97d38`.
All78 Rust and19 UI cases pass with no ignored cases. Three new actual
PostgreSQL/public HTTP/live SDK cases cover lost COMMIT acknowledgement, rebuilt
owner/original readback, later credential changes, original proof mismatch,
role/credential/definition conflicts, rejected receipt insertion rollback, expired
state, competing absent/present key writes and wrong-master rejection. The
controlled model peer observes the original/current expected Bearer token; legacy
session-file bytes remain unchanged and request metadata contains no token value.
This proves SDK transport/custody, not live issuer or model-quality acceptance.

Existing actual cases additionally cover original provider receipts and later
writers, the global Monty service with an in-flight model edit and shared callback/
WebUI owner, secret ownership/legacy migration, absent-row wrong-key races and
one-shot leases. Nine existing settings service cases, the callback descriptor,
two WebUI descriptor and58 handler contracts remain covered. Six new receipt-helper
and three actual-hook control-flow tests accompany ten existing mutation UI tests.
The actual-hook harness holds query refresh unresolved: the preceding hook passes
one/fails two cases; the final hook passes all three. This harness controls hooks/
transport and is not a rendered-browser/platform claim.

The test build took185.25s; WebUI contracts build24.38s. Strict affected/product
 tests lint with `-D warnings` passed80.93s and default Composition lint19.64s.
Workspace format, whitespace and exact source integrity pass. Changed source
copies, runners, actual executables and raw logs are retained. The existing actual
Monty worker is reused only after all530 original inputs are hash-identical.
Before strict lint, the system volume reached91%; needed executables were preserved
and disposable Composition cache cleaned under the shared Cargo lock, restoring90%.
Source, evidence and the canonical target configuration were preserved.

Run1 failed on a test-local HTTP/DB client shadow and redundant import; both were
corrected without suppression. Run2 compiled and passed two actual callback cases,
then rejected an incorrectly short wrong-master fixture before the intended record
check. The final fixture is structurally valid and different. The independently
reproduced UI refresh race was repaired after run2 exited. Run3 freshly validates
all final sources. Earlier failed runs and original/proposed UI regression sources
remain diagnostics, never extra passed results.

## Remaining owner work

NEAR wallet still has its separate legacy file/session/raw-selection writer.
It can be shadowed by a stored provider key and must join the coordinated owner;
this browser acceptance does not certify it. Codex controller/rotating encrypted
sessions, full DB-owned dedicated auth setup/identity and other protocol submission
custody remain K3. Generic AuthProduct flow/session names are not proof of device
code support; its actual challenge/exchange interfaces must be extended/reused.
K4–K7, complete plans, Prefix serving, full process recovery, platform deployment
and release acceptance remain open. No full current Prefix HTTP check or remote
machine work is claimed by this bounded successor.
