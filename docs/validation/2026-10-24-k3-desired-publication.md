# K3 — Consistent desired and actual provider revisions, 2026-10-24

**Scoped acceptance passed:** 104 freshly executed Rust cases, 45 UI contract
cases, eight JavaScript syntax checks, strict affected/root/default lints,
formatting and source/artifact integrity. Evidence is retained under
`target/accepted-evidence/k3-desired-publication-20261024/run-1/`.
This does not close K3, establish prefix readiness or task-wide binding, or
certify installed/browser/platform/release acceptance.

## Implemented boundary

The existing selection owner now supplies the same role grammar and opaque
`selection_revision` calculation for client capture and administration reads.
Settings returns additive `desired_publications` beside the actual
`runtime_publications`. Desired role, provider definition/revision and
authenticated encrypted credential reference come from the same existing
repeatable-read, read-only transaction. A stored selection or successful settings
GET never becomes a client-publication acknowledgement.

The PostgreSQL secret owner adds authenticated reference inspection without
`FOR UPDATE`, legacy migration, lease creation/consumption or plaintext output.
It shares record authentication and the existing randomized-ciphertext reference
calculation with consuming reads; it does not introduce a second secret store or
plaintext fingerprint. Legacy bytes keep their own reference until an actual
consuming owner migrates them. Wrong logical/physical ownership, integrity and
master keys fail closed. Actual lease/consume and migration behavior is retained.
The former later independent key-metadata reads are removed from this settings
snapshot, so they cannot pair a different credential generation with its desired
provider revision.

An externally written unknown/inactive role remains visible for operator repair
with unresolved desired metadata; it does not make the entire valid catalogue
vanish, become unconfigured, or allow fallback client publication. Real database
or encrypted-store failures remain unavailable rather than fabricating metadata.

The existing canonical WebUI query supplies a detailed role panel for primary,
Sempai and embedding. It compares complete desired/actual identities, including
key-only changes, and distinguishes effective, disabled, pending, unconfirmed
and invalid selections. English/German help separates client uptake from prefix
serving, task binding and model requests. Original mutation receipt/reconcile
and optimistic role edit handling remain with their existing owners.

## Actual acceptance

The expanded actual provider/PG caller verifies a held credential writer row
cannot block GET; host key rotation changes desired identity without inventing
a definition revision; the original actual client remains distinct until reload;
reconciliation restores exact equality; later model edits and invalid external
selections stay distinct; and invalid selection cannot publish a fallback.
Existing original-request ACK-loss, concurrent role CAS/deactivation and failure
custody tests pass together (five cases, 15.46 seconds).

The actual secret-owner case verifies repeatable-read retention across a real
same-value re-encryption, new-snapshot identity change, read-only legacy inspection
without migration, distinct exact reference after consuming migration, missing
records, wrong owners/master keys and rejected physical ciphertext copies
(1.97 seconds). All 49 existing secret-store tests also pass. This new port is
PostgreSQL-specific; generic filesystem storage semantics are unchanged.

Existing service/role, model-limit, HTTP descriptor/routing and 32 architecture
cases pass. The actual registered Memory/PG/WebUI caller passes in 68.18 seconds.
Separate actual primary and Sempai Ornith inference passes in 42.31 seconds;
these are provider-client checks, not a full Sempai Recipe/serving activation.

The full existing HTTP/Prefix caller passes in 341.15 seconds with all 319
source declarations, original model artifacts and offline reference encoder.
The oversized Sempai candidate remains `fit=false`; the genuine
`371599 > 262144` tokenizer warning is preserved. No truncation, design approval
or serving activation is inferred. GUI tests establish contract/rendered-tree
behavior under their stated harness, not a rendered-browser session.

## Evidence identities and remaining work

- accepted.json: `5d404098f49d7307e7945dc1f9fafc9a968196eaa0d12857855a8a71414debdc`.
- source.json: `7a4f6b0089d0e9df2e7bde3cc716c70fc1c8e6a5528615b518a0a6093573c314`.
- validated-desired-source.json: `28e5c0f4fc10af5b1ce53fae723d137b8865bf93057043eb7320255fafe23f1e`.
- Actual composition executable: `1ff0d10671d617189fe036ab88cd55149614fb5e1e054049358bc6565dfb2007`.
- Actual secrets executable: `af881206a7ebe80cd16b399eabbc1a45a25a541f3a1954d4ff4eb5a1f93724bc`.
- Actual WebUI handler executable: `55d04d4454c60fbf2a6aad3aad7bc78cfd0f1036c54ec30e09b78b43e1d97658`.

Sixteen changed Rust/JavaScript inputs are preserved immutably. The 530 worker
inputs are unchanged; its actual retained executable is hash checked. Cargo runs
serially in screen with the inherited canonical target, preserving actual test
executables before required cleanup. No test-machine build or deployment occurs.
The existing unchanged architecture executables are freshly run against this
source; no earlier Rust result is counted as a new execution.

Revision-aware failed-publication attempt metadata, task-wide target/prefix
bindings across effects/children/waits, dedicated auth/login and settings/import
writers, other protocol/session boundaries and K4–K7 remain open. Pending here
means desired and actual differ; it is not yet a durable per-role failed-attempt
record. The existing original receipt remains the durable mutation outcome.
