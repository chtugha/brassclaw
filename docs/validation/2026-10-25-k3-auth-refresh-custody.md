# Original Auth refresh custody — 2026-10-25

**Scoped acceptance:283 distinct Rust cases:83 fresh current Auth cases and200
exactly qualified prior native/consumer results. Current strict Root/Auth/
Composition/FirstParty tests/default, format, whitespace and integrity pass.**

The existing AuthProduct owners now retain the original refresh request and
actual prepared token destinations before a possible issuer call. A typed action
UUID is preserved through the coordinator, public refresh DTO and GSuite. The
public DTO requires `refresh_id`; identical repeats read the original, changed
explicit account/provider/requester conflicts, and a new ID cannot bypass an
unresolved effect on the same refresh credential. A genuinely different authorized
credential remains distinct. Local weak mutexes are accelerators, not authority.

Completion durably records the exact normalized returned metadata before account
publication. The existing PG owner uses normalized refresh rows and a partial
unresolved-credential index; final account/receipt publication is one transaction.
CAS filesystem storage retains original intent/result plus a backward-readable
account envelope. Only the matching final account/receipt in that envelope
certifies publication. Selection and CAS use one account snapshot, preserving an
independent winner; all ordinary account writers retain its journal. No second
provider registry, token executor, task workflow, grant or VM was introduced.

Original reconciliation performs store transitions only. Lost acknowledgements,
failed account writes, cancellation and restart never authorize another issuer
call. Returned-but-unpublished metadata survives a rejected publication and can
be reconciled from its original ID. Unknown effects remain unconfirmed. Stale or
foreign output cannot replace current credentials; omitted refresh rotation keeps
the original refresh handle. Recovery lookup failures propagate without replay.

The redacted public receipt separates original outcome from current account
state and carries an exact published metadata reference, never an auth grant.
GSuite uses its active operation identity while keeping account storage scope;
it checks that exact confirmed receipt and current snapshot before retrying
resource egress. OAuth requests, results, prepared destinations and nested action
Debug output redact backend secret handles. Stored metadata still retains the
handles required by its existing encrypted owner.

## Evidence and limits

Final archive: `target/accepted-evidence/k3-auth-refresh-custody-20261025/run-2/`.
Receipt SHA-256: `224b3b54bb15beae28258bf644d3faf48c8ec0cc49b00d03de81248d79cb5030`.
The initial31-file candidate was applied after exact base/proposal comparison.
All15 provider implementations, six setup implementations and12 typed literal
constructors were source-reviewed; syntax assistance is not behavioral evidence.
Canonical root/target, Rust1.96.0 and serial screen/Cargo lock were used.

| Actual cases | Count | Final applicability |
| --- | ---: | --- |
| Auth product contracts, expanded OAuth Debug redaction | 83 | Fresh run2 |
| GSuite executor/credential consumers | 34 | Qualified run1 |
| PG refresh/boundary cases | 13 | Qualified run1 |
| Filesystem Auth adapter cases | 44 | Qualified run1 |
| Host OAuth provider/secret custody cases | 12 | Qualified run1 |
| Provider multiplexer | 6 | Qualified run1 |
| Callback/lifecycle facade cases | 12 | Qualified run1 |
| Public product-auth router cases | 77 | Qualified run1 |
| Ordinary global Monty/live-model caller | 1 | Qualified run1 |
| Dependency boundary | 1 | Qualified run1 |

Four new PG cases cover independent owners/original claims, changed explicit
identity, real rejected final account UPDATE with durable result/restart recovery,
unknown/stale and foreign outputs. The44 filesystem cases include one new real
PostgresRootFilesystem/FilesystemSecretStore case: old bare JSON, independent
claims, genuine account-write rejection, recreated owners, original-only recovery,
concurrent finalizers and preservation through ordinary writers. Other filesystem
cases use the actual adapter with its InMemory CAS backend; they do not establish
persistence by themselves. LocalFilesystem remains byte-only and is not a CAS
fixture or a new sidecar implementation.

The Host/real encrypted PG case deliberately drops successful claim/result
acknowledgements after real commits and forces a genuine aggregate-recovery decode
failure. It proves zero extra issuer-boundary calls and retained original secret
material. Its issuer transport is controlled; physical HTTP no-replay has its
separate [Network acceptance](2026-10-25-k3-network-no-replay.md). The cancellation
case exercises the actual coordinator with a blocked test provider, not a claimed
real remote issuer cancellation. The seven GSuite mismatch subcases use the actual
executor with controlled account/egress boundaries. Public routers use scoped test
services; no deployed issuer/browser/platform acceptance follows.

Run1 executes all283 cases successfully. Its strict check then fails on one
unused legacy import and two unnecessary comparison references. All three are
repaired without suppression; the review additionally redacts OAuth Debug handles.
Run2 proves every other protected input unchanged and checks exact import/
comparison/Debug-only transformations plus the expanded Debug test. Its
`runtime-evidence-qualification.json` retains the200 applicable native/consumer
results and original artifact/log hashes; they are not relabelled as fresh runs.
Current Auth83 rerun covers the changed formatting behavior.

Run1 Composition build195.39s, native PG26.14s, FS2.03s, Host7.75s and global36.15s
pass. Run2 Auth build1.72s/tests0.33s, current strict Root/Auth/Composition/
FirstParty tests80.26s, default Composition19.96s and format4.56s pass.
Worker inputs remain exact under the separately reviewed existing lock-only
Network dev edges. The original worker SHA is
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.
The initial strict failure is retained. No required case is ignored or silently
converted to success.

## Remaining K3 work

This closes the bounded original-refresh custody prerequisite. Generic callback
success still needs complete original payload/prepared destinations and result
custody, final atomic account/flow and safe cleanup disposition. Denied/failed/
unknown completion must never dispatch a success continuation. Manual-token
secret/account/interaction/flow success and lost-ack compensation remain open.
Complete issuer expiry through final staged use, typed DB-owned setup/original
grant, actual Codex controller/rotating-session migration and remaining protocol
consumers afterward. K4–K7 still precede ordinary remaining-plan work. No commit,
push, tag, release or SSH operation was performed in this package.
