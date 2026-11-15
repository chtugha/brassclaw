# History formatter and CI dependency repair — scoped acceptance

Source: Main `829bd0f2a2cfdcae3b6f3c5a07309f50e82536dc` plus the captured
Root changes. This checkpoint does not complete B1/B2/B3, general authored Pure
approval, full History workflow qualification, deployment or platform acceptance.

## Exact function and runtime data

The product History consumer and installation qualifier share the canonical
`history_formatter_draft`. Its Python body, export and three-string contract
remain unchanged. The existing Spec/Suite/Context owner independently qualifies
this actual retained root after a separately checked private text-receipt
instrument. The root receives its own S approval, null Q2 and a one-root closure;
the instrument receives no public own-function approval. Boot checks the exact
consumer revision. Existing completed originals are reused without execution.

Literal oracles cover ordinary/empty strings, quotes, newlines, marker text,
Unicode, NUL, CRLF and supplementary Unicode. A failed native startup exposed a
real fixture-storage mistake: PostgreSQL cannot represent NUL inside a JSONB
string. An isolated PostgreSQL diagnostic confirmed the rejection. The correction
retains fixture runtime data as JSON text and supplies the decoded values through
the existing typed task-input bindings. The Recipe contains contracts/references,
not the runtime NUL or substituted Python source. Fingerprints verify those same
original inputs. Legacy suites omit the new optional carrier and preserve their
serialized form. No oracle, database check or authored guard is removed.

## CI and dependency changes

Actual Main-push CI exposed stale standalone Monty lockfiles, advisory/license
failures, a Screen test timing race, a v1 compatibility lint, worker discovery in
Cargo 1.100's changed test layout and the Ubuntu 26 PostgreSQL loader ABI.

The resolved graph updates TLS verification/roots, Tokio, constant-time helpers
and the executor. Local-only libSQL migration drops unused network features;
YAML test compatibility uses serde_norway; testcontainers and Bollard use current
compatible APIs; OIDC test keys use RSA 0.10 with a separate development RNG.
Production RNG behavior is unchanged. All advisory waivers are removed. Required
Blue Oak notices accompany release artifacts. The five native PostgreSQL CI jobs
use the existing release Ubuntu 24.04 ABI and install libxml2 explicitly.

Worker discovery recognizes only the exact Cargo layout and matching executable
hash; it adds no PATH search or caller-selected worker. The Screen regression
uses explicit synchronization rather than a short elapsed-time assumption.
Three daemon-independent sandbox configuration/preflight tests are no longer
ignored; their lazy client performs no Docker operation.

## Actual checks and preserved failures

Evidence is under `target/validation/bob-native-repair-20261105/` unless stated.

- Run118: Rust 1.100 strict all-targets check of ten affected packages with the
  actual database, migration, v1, provider and test-support features. Passed in
  125.679 s, zero warnings/diagnostics, unchanged captured source.
- Run119: native programs and normal worker compiled in 655.996 s without
  warnings. Both current R1/History source-contract tests pass.
- Run119 `reused-artifacts.json`: exact SHA/features/profile/target-kind identity
  retains 61 prior HostRuntime/Skills/MontyHost cases and two architecture cases.
  This evidence does not substitute for changed Composition execution.
- Run119 `acceptance-pg`: all six actual cases pass, none ignored, unchanged
  source. History qualification/reuse takes 319.062 s; existing generation root
  qualification/reuse 311.914 s; real completion and approval COMMIT-ACK cuts
  628.220 s across two isolated clusters; unknown first intent plus restart
  324.534 s; both real SQLite-to-PostgreSQL migration cases 4.356 s.
- Run119 `acceptance-oidc`: all five signed-token/JWKS, rotation, single-flight,
  backoff and unknown-key cases pass inside the sandbox in 0.824 s. Only their
  own loopback server is contacted; no existing provider/account is used.
- `target/validation/modal-mount-controller-r2-20261115/`: 72 offline build
  controller/worker checks pass. The current Screen suite's 55 checks pass in
  `target/validation/ci-repairs-20261115/screen-tests.json`.
- Dependency R2 resolves all three actual workspaces with Cargo 1.96 and locked
  metadata. `target/validation/ci-deny-final-r2-20261115/` confirms the final
  all-feature graph against today's already fetched advisory database: zero
  advisory, license and source errors/warnings; 47 duplicate-version warnings
  remain visible. No allowance is added to silence them.

Earlier failures remain: Run115's JSONB/NUL error, Run116's missing Map-to-Value
conversion, the original source-test namespace failure and the offline fixture
that accidentally consulted live build state. Their corrected current cases
pass. The final deny attempt first encountered a real sandbox advisory-lock
restriction; only that reviewed check was repeated outside the sandbox.

The PG Screen command completes with exit 0 and no signal/error; its socket
directory is empty and no own PG/Monty process remains. Its final IPC is **not
delivered**: an extended tool/permission wait kept delivery busy until the
600-second notification deadline expired. Local status/log inspection at the
next safe opportunity confirms test success separately. No final chat receipt
is claimed, and no process is interrupted or replayed for notification delivery.

## Remaining boundaries

Authored approval, complete joint subjects/check coverage, B2 publication and
B3 durable recovery remain open. The Docker checks above establish configuration
and pre-dispatch refusals, not real daemon execution. Existing sandbox cleanup
currently discards remove errors and lacks complete cancellation/operation
custody; its connected repair belongs to Simplified Phase 3a/B3-08. The same
adapter still has fixed resource defaults outside the live Monty settings;
they remain in that cap migration. This checkpoint does not claim every limit
is WebUI-adjustable. V102/V200 physical evidence limits retain their separate
B1-06 successor requirement.

Reporting/plan prose is updated after these checks. It changes no tested Rust
or component bytes and is not an exact final release-image claim. Subsequent
GitHub CI results must be assessed separately from these local receipts.
