# Existing PostgreSQL AuthProduct boundary parity — 2026-10-25

**Scoped acceptance:34 freshly executed Rust cases, current strict Root/
Composition tests/default checks, format, whitespace and source integrity pass.**

This repairs actual missing boundaries in the selected PgAuthProductServices
owner. It reuses the existing AuthProduct scope, stable owner types and shared
domain validators. It introduces no second backend, registry, auth executor,
workflow loop, permission grant or VM.

Direct account/flow/interaction reads now check the complete stored scope.
Exact-scope account/flow lists filter those records; stable-owner read models
remain distinct and retain original invocations, surfaces and sessions. Owner
flow queries use one query instead of seven identical synthetic-surface scans.
Setup binding lookups preserve session_id rather than reconstructing a partial
scope. These are existing identity/storage contracts, not new feature-role checks.

Absent writes now insert-or-conflict; an explicit existing identity cannot be
silently overwritten through UPSERT. Interaction deletion authenticates its
scope and predicates on both original revision and exact metadata. Manual
cancellation selects the matching interaction, rereads its current record under
the existing lock/CAS and changes that flow only. The actual abandon caller
propagates cancellation/deletion failures instead of discarding cancellation
errors. A repeated matching terminal manual cancellation remains idempotent.

Callback failure uses the existing original-state/expiry validator. Continuation
acknowledgement requires a completed flow and preserves the first acknowledgement.
Terminal flow cancellation retains classified errors. Credential selection uses
the existing advertised-choice validator and actual current eligible account;
repeated completion keeps its original selected identity. Expired selection
persists its derived terminal state before returning the classified error.

## Observed evidence

Archive: `target/accepted-evidence/k3-pg-auth-boundary-20261025/run-1/`.
Receipt SHA-256: `c397ba577910310beffd42669c531b382c51765f0b7eb06541ea5836eea2b14b`.
Two Rust source/test files plus compiled CHANGELOG were applied after exact
comparison with the accepted snapshot-CAS base. The source inventory, exact
executable, worker, commands and runner are retained. Canonical source/target,
Rust1.96.0 and one serial screen/Cargo lock were used. No intermediate source
compile/lint failure or diagnostic suppression occurred.

Nine native PostgreSQL cases pass19.62s: the preceding two actual metadata CAS
cases plus seven new boundary cases. They cover all seven logical non-tenant
scope dimensions and physical tenant isolation, direct mutations/lists, conflicting
original identities, bound-session setup, stable-owner/gate queries across original
invocations, matching manual cancellation, foreign abandonment, original state
failure, first continuation, advertised and currently eligible selection, expiry
and genuine UPDATE/DELETE-trigger rejection. After each injected failure, actual
records retain retry state; the later original abandon completes without
silencing the earlier error. The continuation-only fixture deliberately creates
no credential or continuation effect; it certifies acknowledgement state only.

Ten actual route-adapter cases and14 runtime-credential-selection cases pass.
The ordinary global Monty/live-model case passes36.59s with current packaged
content. These are scoped native/adapter callers, not full public OAuth/issuer
or manual-token success custody. Worker source remains applicable after all530
exact input comparisons, SHA
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

Actual test build187.32s, strict current Root/Composition tests
76.36s, default Composition14.31s, workspace format4.56s
and whitespace/source integrity pass. All34 cases are fresh and nonignored;
older Auth81 and refresh-artifact evidence remain their separate qualified scopes.
No full-plan/platform/release/test-machine acceptance follows.

## Remaining custody work

This closes only these boundary prerequisites in Simplified K3 authentication
custody step2. Original refresh identity, pre-egress ownership and prepared token
destinations must survive cancellation, unknown issuer effects and restart.
Current Host failure mappings do not establish no effect. Generic callback success
still requires complete provider/result/PKCE/bound-generation checks and atomic
original account/flow publication. Manual-token success and error cleanup likewise
need exact secret/account/interaction/flow custody. The generic callback facade
must never delete a potentially published credential after an unknown completion.

Use the same existing AuthProduct owners and immutable output artifacts; do not
replay issuer exchange or replace the task's original credentials with latest.
Complete optional issuer expiry/final staging, typed DB auth setup/original grant,
Codex encrypted sessions and remaining protocol/K3 acceptance afterward. K4–K7
still precede ordinary plan continuation. No commit, push, tag, release or SSH
operation was performed in this package.
