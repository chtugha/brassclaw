# Final OAuth refresh account fence — 2026-10-25

**Scoped acceptance:96 freshly executed Rust tests and current strict Root/Auth/
Composition including tests/default lints, format, whitespace and integrity pass.**

The provider-backed account wrapper previously read the current account after
issuer completion, then asked setup to perform an unconditional update. A writer
between that final read and setup could revoke or replace the account, only for
the refresh result to overwrite the newer state. Its early read checks did not
close that final window.

The existing CredentialSetupService now requires atomic expected-snapshot update.
All four existing implementations are updated: PostgreSQL revision CAS, durable
filesystem record CAS, the contract fake and the constructor-only test double.
The actual wrapper uses this port for successful refresh, issuer failure and a
missing refresh token. The store compares the exact observed account before
mutation and uses the revision from that same read in the actual CAS write.
An independent writer after that read causes a storage conflict. Existing
scope/provider/ownership/grant validation remains in the shared domain updater.
No status/credential/ownership edit is silently replaced and no conflict is
reported as successful refresh. These are storage fences, not permissions or
durable action receipts. No new field, table, dependency or backend is introduced.

## Observed callers

Archive: `target/accepted-evidence/k3-oauth-refresh-cas-20261025/run-1/`.
Receipt SHA-256: `421c3dd40f24c34df91aa79c780a0ff300ba1b885b9afa14027a6af9d46ecce4`.
Nine applied files (eight Rust including one new native test module, and embedded
CHANGELOG), exact source/binaries/worker/commands and runner are retained. Canonical
root/target, Rust1.96.0 and one serial screen/Cargo lock were used. No intermediate
source compilation or lint failure occurred.

All81 actual Auth contract cases pass, including a new test that revokes the real
fake account inside the final setup handoff for all three outcomes. The wrapper
must use atomic setup; unconditional setup in that test is an explicit failure.
Each result is BackendConflict and retains revoked status and original credentials.
These are contract/fake tests, not remote issuer evidence.

Two native PostgreSQL cases pass through independent actual PgAuthProductServices
owners. Concurrent successors of one exact snapshot produce exactly one winner
and revision1; a rebuilt owner reads that same winner and original creation date.
A prior revocation and a wrong-scope update remain unchanged. This validates native
account metadata CAS, not token encryption. The durable filesystem caller also
rejects an old original after an independent owner reads the published successor.

Six composed provider-registry cases, three auth bundle constructors and two
existing public OAuth callback adapter cases pass. The ordinary global Monty/live
model caller passes36.80s with current packaged content. The exact worker remains
applicable after530 input comparisons, SHA
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

Auth contract build5.07s and Composition test build193.93s pass. Native PG4.93s,
all remaining contract/adapter groups under one second each, current strict
Root/Auth/Composition tests82.45s, default Composition20.17s, format4.60s and
whitespace/source integrity pass. All96 are fresh, with no ignored/zero-selected
success or lint suppression. Earlier refresh-artifact acceptance retains its own
exact separate scope and does not become durable refresh acceptance here.

## Remaining custody

This closes the final write race within authentication custody step2, not the
whole step. Original refresh action identity, pre-egress ownership and unknown
outcomes across cancellation/restart remain. Generic PG callback/manual-token
publication and error cleanup still need exact validation, atomic account/flow/
interaction custody and reconciliation before deleting a potentially referenced
credential. Further source trace found missing PG exact-scope filtering, absent
insert semantics and manual cancellation/continuation parity; those are the next
existing-owner repair, not a second auth backend. Complete expiry/final injection,
DB auth setup/original grant capture and actual Codex encrypted-session cutover
remain. K3–K7 and full plans/platform/deployment acceptance are open.
