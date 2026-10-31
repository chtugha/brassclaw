# Native OAuth and account-choice requirement publication — 27 October 2026

Status: scoped native acceptance; original family/material consumers remain open.

The existing PostgreSQL Auth owner now publishes exact requirement selections
inside the account/flow transaction for OAuth completion and explicit account
choice. Both paths retain original flow custody. Flow -> account -> head locking
keeps their order consistent with manual publication. Completed choice retries
read original custody without selecting today's account or rewriting a newer head.
The shared protocol checker rejects using completed OAuth/manual flows as account
choices. Expired choices persist their actual expired disposition.

R2 receipt: `target/validation/native-other-requirements-20261027/run-2/accepted.json`.
SHA256: `98637760d7a3c31f547feeef69516564bd75216c478b97c596241a658c215246`.

R2 ran 122 fresh Composition cases:44 native PostgreSQL Auth and78 filesystem
regression cases, zero ignored. Its strict tests/default lint, format and whitespace
checks passed.98 actual Auth cases and strict all-target lint are reused from R1:
R2 verifies that the sole intervening compiler-source change was a Composition
test assertion outside Auth's dependency closure, and authenticates the original
actual executables. These are220 distinct cases, not220 fresh R2 executions.
R1 stopped on a test-only missing PartialEq assertion; compare the two existing
state fields instead of changing the production type. Its failed receipt/log and
source snapshot remain preserved.

New native cases exercise actual committed choice with lost COMMIT reply, original
readback after a later choice/account revocation, rejected final-flow SQL with
transaction rollback/retry, independent competing absent-head choices, and retained
OAuth returned-result reconciliation after rejected publication. OAuth exchange
input is a trusted retained-result fixture through the real owner, not an external
issuer/network acceptance. Secret storage is actual encrypted PostgreSQL storage.
No issuer exchange or completed effect is replayed to repair publication.

The actual Composition image SHA256 is `d0260b0cebbbdf7658e40fe99c5f0d75201322bfb0f3a8c4bf07087439f71f3d`;
the requested worker build retains the exact unchanged worker image recorded in
the receipt. Prefix Source6/Source2 were verified unchanged. No build/test process
remained when the source freeze was lifted. This does not certify FS/fake non-manual
publication, deliberate sharing, task-family/material capture, all K3–K7, platforms,
deployment or complete plans. Those remain implementation work in existing owners.
