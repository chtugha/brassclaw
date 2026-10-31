# OAuth refresh artifact custody — 2026-10-25

**Scoped acceptance:26 actual Rust results, current strict Root/Composition
including tests/default lints, format, whitespace and source integrity pass.**

Refresh previously reused fixed account-specific access/refresh handles. A later
completion could overwrite ciphertext referenced by a different selected account
revision before its later account comparison rejected publication. Each refresh
completion now has its own private storage destinations, validated before issuer
submission. These UUIDs identify storage writes; they are not durable refresh
action/attempt identities or authority. Callback destinations remain flow-specific.

When an issuer rotates a refresh token, the actual DCR material owner copies the
exact old encrypted client registration to the replacement's association. Static
clients implement the same interface without a per-token association. Omitted
rotation returns no replacement and preserves the original refresh/registration.
Initial or rotated binding failure cleans only known unpublished outputs and
attempts both token/material cleanup. Callback flow-material cleanup is also
attempted even after another cleanup fails. Stable errors remain visible and
contain no raw provider material. Published-account cleanup is not broadened.

## Evidence and limits

Final archive: `target/accepted-evidence/k3-oauth-refresh-artifacts-20261025/run-2/`.
Receipt SHA-256: `ebf730658fd06c49f5f16bd3754f80ff83e39e2ab917535dadc01f5d83407e12`.
Run1 retains actual executable/source/worker, all failure and behavioral logs.
Run2 retains final source, commands and exact syntax-repair/evidence applicability.
Rust1.96.0, canonical common source/target and serial screen/Cargo lock were used.
The reviewed changes are three Rust source/test files plus compiled CHANGELOG.
All530 worker inputs match their retained originals; the actual worker is
`d7860b5f96f9a9001e219cf4d23a2a0dbeffcfc2ac22cc44e9fe0a0e33f2877c`.

Run1's actual OAuth-client11 cases pass7.09s, DCR14 cases pass1.97s and ordinary
global Monty/live model1 case passes36.24s. Four new native encrypted PostgreSQL
cases cover separate completion/original token artifacts, omitted rotation,
initial/rotated binding failure, exact original DCR client/endpoint/secret binding,
foreign scope rejection and a real DELETE-trigger failure. The latter proves
failure remains visible while other owned cleanup still runs. Controlled egress
records one call per offered exchange; this is not physical issuer replay safety.

The actual test build passes189.04s. Initial strict lint fails on exactly two
collapsible nested `if` forms. Both are corrected to ordered short-circuit let
chains with identical bodies. Run2 recomputes those exact transformations from
archived source and compares formatted bytes with current source; every other
protected input is unchanged. The26 actual results are qualified reuse with
their original binary/source identity, not rerun or mislabeled fresh. Final current
Root/Composition tests lint passes72.48s, default Composition14.19s, workspace
format4.55s and whitespace/integrity pass. No lint suppression or ignored/zero
case acceptance is used. The failed run remains retained.

## Remaining implementation

This closes only custody step1 in the existing Simplified K3 authentication order.
The existing account wrapper still needs its final atomic snapshot fence (the
next prepared package), durable original refresh ownership before external effects,
unknown-result/restart readback and atomic account/flow publication. Generic
callback and manual-token error cleanup must reconcile publication before deleting
material. Issuer expiry must reach final injection, typed DB auth setup/original
grant capture and actual Codex encrypted-session/controller consumers remain open.
No new registry, executor, task workflow, VM, Rust-loop fallback, remote operation
or full K3/plan/deployment acceptance is introduced.
