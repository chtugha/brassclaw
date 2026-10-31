# Original OAuth callback custody and verified PKCE cleanup — 2026-10-25

**Scoped acceptance: 334 distinct passing Rust cases and final strict affected/root,
default, formatting, whitespace and source-integrity checks.** All 201 affected
Composition/architecture cases ran freshly at run5. The final equivalent
`and_then(|_| Err(error))` → `and(Err(error))` correction is qualified by an exact
single-expression source comparison;133 independent Auth/GSuite/Workflow results
were already exactly qualified. None is described as a fresh final-source run.

## Implemented boundary

The existing AuthFlowManager retains the complete original callback, exact
prepared destinations and one admitted Dispatch. Raw code and PKCE input must
match their declared hashes. A changed explicit original conflicts; a retained
original never calls the issuer again or needs current provider/consumed PKCE.
A private backward-readable flow envelope captures the bound account at creation,
keeping public AuthFlowRecord hash-only. Every selected provider and ordinary
flow/account writer participates; old missing snapshot proof stays missing.

The actual returned metadata is retained before account publication. Provider,
code/verifier hashes, label, destinations, optional account ID and scope coverage
are checked. Unknown/unusable results remain accountable without success. The PG
owner finalizes account, flow and receipt in one existing transaction. The actual
CAS-filesystem owner selects an irreversible flow publication decision, then
stores the matching receipt alongside the account and both custody journals.
Independent original finalizers recover lost acknowledgements without overwriting
later edits or replaying effects. Pre-decision cancellation/expiry fences
publication; after the decision, cancel reconciles or reports pending/error and
cannot falsely acknowledge Canceled while the chosen account CAS may still commit.

Only verified Completed state dispatches a success continuation or renders/broadcasts
completion HTML. Stable sanitized conflict codes additionally carry nonretryable
`conflict:true` and HTTP409. Provider denial/failure cannot become success.

Public generic and Google error cleanup additionally verifies exact durable flow
ID, full scope, constant-time state hash and OAuth challenge kind before removing
a verifier for a terminal status. Invalid/missing/partial/disallowed scopes,
missing code, wrong state, forged denial and pre-dispatch errors preserve a valid
pending verifier. A failed final-state read remains an error. Verified success
still releases its cache entry and original success remains cache-independent.
This repairs a real pre-effect cache-eviction defect, rather than accepting404
as the expected result after malformed input.

## Actual evidence

Archive: `target/accepted-evidence/k3-auth-callback-custody-20261025/run-6/`.
Receipt SHA-256: `bd01bd3a9f03894b1f0de907ded55af57360ef49e9c86e27d565ea92ff682e90`.
The exact 37-file manifest, captured common-root sources, retained executables,
commands, qualification record and logs are preserved. One configured Cargo target,
Rust1.96.0, screen and the canonical serial lock were used. Prefix peer's accepted
source-export successor remains byte-for-byte unchanged.

| Cases | Count | Execution/source qualification |
| --- | ---: | --- |
| Auth product contracts | 83 | unchanged independent run2 target, qualified through run3/run5 |
| GSuite consumers | 34 | unchanged independent run2 target, qualified through run3/run5 |
| ProductWorkflow auth interaction | 16 | unchanged independent run3 target, qualified through run5 |
| Native PG refresh/callback/boundaries | 18 | fresh run5; final equivalent-expression qualification |
| Filesystem Auth/accounts/custody | 48 | fresh run5; same qualification |
| Actual Host/encrypted-store custody | 13 | fresh run5; same qualification |
| Provider multiplexer | 6 | fresh run5; same qualification |
| Auth/OAuth HTTP helpers | 11 | fresh run5; same qualification |
| Direct callback consumer | 7 | fresh run5; same qualification |
| Lifecycle/manual consumers | 15 | fresh run5; same qualification; not manual custody acceptance |
| Public WebUI auth routers | 81 | fresh run5; same qualification |
| Ordinary global Monty/live-model caller | 1 | fresh run5; same qualification |
| Actual architecture dependency boundary | 1 | fresh run5; same qualification |

No ignored/zero-case suite is counted. Native PG cases cover independent claims,
real rejected account publication, durable returned outcomes, restart, later
account changes, cancellation/expiry and unusable returns. Actual persistent
PostgresRootFilesystem/FilesystemSecretStore cases cover lost admission/result/
decision/account/final acknowledgements, rejected account publication and two
independent finalizers. StorageTxn is an interface; these backends remain CAS-only.
LocalFilesystem's byte-only support is not upgraded by this report.

The real Host facade retains encrypted returned material after failed PG account
publication and resumes the original with no provider/verifier. Public tests prove
zero provider/continuation effects for rejected callbacks, subsequent legitimate
success exactly once, no false denied HTML success, original failure/no issuer
replay and changed-payload conflict without state mutation.

Run1 failed on Auth self-crate paths; run2 exposed an accidentally removed PG
continuation marker, restored exactly. Run3's new native cases passed, but four
old hash/scope/failure/conflict fixtures required correction. Review then found
the actual PKCE bug. Run4 failed on two test-only OAuthState Display uses, corrected
with existing as_str(); run5 passed all cases but failed one strict unnecessary
closure lint. Every failed source/archive/log remains preserved, with no warning
suppression or retrospective green receipt. Run6 final strict checks pass in
74.36s, default in35.18s and format in4.65s. These are observed
batch times, not runtime settings or new task deadlines.

The retained worker has 529 exact inputs plus the separately qualified original
Cargo.lock Network dev-edge delta. Its executable SHA-256 remains
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.
No new VM, Rust-loop fallback, queue, issuer retry or approval gate was introduced.

## Remaining owner work

This closes the bounded original OAuth callback custody/PKCE correction in K3
step2. Manual-token secret/account/interaction/flow success custody, owned partial
artifact disposition, issuer expiry/final staging, dedicated DB-owned setup and
original grants, Codex controller/rotating sessions and other protocol consumers
remain open before K4–K7 and ordinary plan continuation. Manual HTTP handlers also
abandon on every failure/timeout and must migrate with their storage owners.
No real issuer, platform-wide/deployment/release/full-K3/full-plan acceptance,
Git commit/push/tag or SSH operation is claimed.
