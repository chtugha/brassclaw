# Original refresh readback without publication locks — 2026-10-25

**Scoped acceptance: 273 fresh Rust cases, current strict Root/Auth/Composition/
FirstParty tests/default lints, format, whitespace and source integrity pass.**

The existing coordinator reads an original action before considering publication.
Absent, still-Submitted and already finalized originals require no publication
transaction or mutex. Only a stored outcome with pending account publication
enters the existing reconciliation. Original admission remains the sole authority
to make the first issuer call, including races after a missing lookup. No issuer
retry, replacement identity, latest-provider selection or new storage owner was
introduced.

The PostgreSQL and CAS-filesystem owners apply the same distinction to direct
reconciliation and repeated admission. New admissions still serialize and recheck
inside the existing transaction/CAS protocol. Known pending results still select
and finalize from the protected current account/action, retaining the original
result and atomic final account receipt. Read-only results may reflect their
read instant; an unconfirmed response never grants another dispatch.

## Actual acceptance

Archive: `target/accepted-evidence/k3-refresh-readback-20261025/run-1/`.
Receipt SHA-256:
`ad6109939a288de26958fe05b1184f7645d8bac850362a52756f505c7946812c`.
Its `source.json`, seven-file `change-manifest.json`, retained executables,
commands and logs identify the current common root/target, Rust 1.96.0 and the
serial Screen/Cargo-lock window. All cases below were freshly executed against
that captured source. No intermediate failed compile or check occurred.

| Actual caller cases | Count |
| --- | ---: |
| Auth product contracts | 83 |
| GSuite credential/resource consumers | 34 |
| Native PostgreSQL Auth refresh/boundaries | 15 |
| Filesystem Auth adapter/custody | 45 |
| Host OAuth/encrypted-store custody | 12 |
| Actual provider multiplexer | 6 |
| Public product-auth routers | 77 |
| Ordinary global Monty/live-model caller | 1 |

Two new native PostgreSQL cases hold real account and action row locks while
checking missing, Submitted and confirmed originals through direct owners and
the actual coordinator. Repeated admission returns Original, changed explicit
provider conflicts and action revisions remain unchanged. The coordinator uses
an unavailable provider client, proving original readback needs no current
issuer client. A genuine rejected account UPDATE first leaves a durable returned
result; subsequent independent finalizers are observed through PostgreSQL's
actual blocking graph and publish one account revision after the lock is
released. Both finalizers return the same confirmed action.

The new filesystem case holds the actual adapter's local account publication
mutex and checks missing/Submitted/final originals and the coordinator. It uses
the real InMemory CAS backend and does not itself establish persistence. The
retained persistent PostgresRootFilesystem/FilesystemSecretStore case also runs
fresh, covering independent claims, rejection/restart and finalizer custody.
LocalFilesystem remains byte-only. The StorageTxn interface does not establish
multi-key transaction support on these backends.

Build timings: Auth 1.51s, FirstParty 1.70s, Composition 196.40s. Native PG cases
30.94s, filesystem 1.97s, Host 8.38s and global 35.97s pass. Strict affected/root
tests 83.07s, default Composition 20.21s and format 4.64s pass. These are observed
batch times, not a new runtime timeout or an estimate for individual operations.
The native test watchdogs do not alter production settings.

The previously accepted worker retains529 exact inputs plus the separately
qualified Cargo.lock Network dev-edge delta. Its executable SHA-256 remains
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`;
the receipt retains that comparison. The new callback projection was outside the
common tree and was not a build input or an acceptance claim.

## Remaining scope

This closes only the assigned cheap original-refresh readback correction within
K3's existing authentication custody owner. Callback result/account/flow and
manual-token success custody, issuer expiry, DB-owned dedicated setup/original
grants, Codex controller/rotating sessions and remaining protocol consumers stay
open. K3–K7 and the full plans remain active. No release, deployment, real issuer,
cross-platform acceptance, commit, push, tag or SSH operation is claimed.
