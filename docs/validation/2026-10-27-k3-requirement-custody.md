# Exact credential requirements and original setup custody — 27 October 2026

Status: scoped local acceptance. K3 step4 and the complete plans remain open.

The original Tool usage declaration now travels through authorization,
HostRuntime obligations/resolution/auth errors, MCP errors and GSuite errors.
Its identity includes the requester, handle, provider/setup/scopes, audience,
target and prefix. The new versioned stored envelope rejects unsupported nested
fields instead of dropping them. Its own serialization is the supported stored
shape; the legacy bare-descriptor decoder is unchanged.

Exact auth-gate identities hash that complete canonical declaration in a separate
namespace. The regression constructs a legacy scope containing the former suffix
and proves that the actual returned gates differ. Historical requirements without
the new declaration retain their old addresses and supply no new grant evidence.

New manual/generic setup can retain the declaration and previous-selection
precondition in its first existing Auth-owner write. Native PostgreSQL publishes
the original interaction/flow metadata pair transactionally; filesystem setup
retains its interaction decision and reconciles its projection with exact CAS.
Independent reopening/recovery retains the first expiry and requirement. A changed
audience, removed requirement or wrong provider cannot reinterpret that original.
The native case deliberately loses the real commit acknowledgement and recovers
the same saved setup. Absent custody fields preserve the old serialized shape.

A preexisting test-only listener was found under production `src` by the unchanged
architecture guard. Its source now resides in
`crates/brassclaw_reborn_composition/tests/support/oauth_original_browser.rs`;
the same test module executes real native HTTP and Chromium assertions. No guard,
assertion, warning or test was suppressed.

## Observed validation

The serial Screen runner compiled the common checkout with inherited
`CARGO_TARGET_DIR`, Rust1.96.0 and the existing native-test feature configuration.
R4 receipt: `target/validation/requirement-custody-20261027/run-4/accepted.json`,
SHA256 `de78519676c69c00c8d44b69305e6f0183d1cc781402b8ae83ed05e77758f5b7`.
Its `source.json`, 70-path `owned-manifest.json`, explicit removed source path,
actual retained executables and command logs identify the tested inputs.

- Strict ten-package test Clippy:95.84s; emitted worker:6.25s; selected actual
  library/integration executables:232.98s; default-library Clippy:31.66s.
- 682 distinct fresh Root Rust cases passed. There were685 successful executions;
  three new cases also occurred in their wider owner suites. Exact deduplication
  is retained in `distinct-test-accounting.json` rather than double-counted.
- Five new custody/serialization cases pass across the fake, actual PostgreSQL,
  memory-backed record filesystem and PostgreSQL-backed record filesystem.
  Native Auth37, filesystem Auth75, actual HTTP egress, MCP dispatch, GSuite,
  authorization and both architecture suites pass. Chromium reports its actual
  original-identity/expiry, lost-acknowledgement and cancellation assertions.
- Three existing Docker sandbox cases were explicitly unexecuted. They are
  excluded from the passing count and remain outside this scoped acceptance.
- Workspace format and whitespace pass. Root independently rehashed all22
  retained executables and checked complete current compiler-input membership
  and hashes, including added/removed files. The actual processes were terminal
  before reopening the shared implementation window.

The Prefix owner's separate image-only check uses those same compiled inputs:
`target/accepted-evidence/prefix-model-card-joint-20261027/run-1/accepted.json`,
SHA256 `aebf5c2f3f37cc6f734a95d9fb514d22442e9f19b3c002352438e068c7e54fa8`.
It records six fresh Rust cases, three UI cases and one offline case. Its
source/model-card scope does not establish accepted designs or production serving.

R1's new deserializer lint, R2's architecture failure after534 passing executions,
and R3's capacity-preflight failure remain archived. All22 R2 images were verified
before selectively cleaning disposable build caches; no source/evidence was
removed. These older attempts are not passing receipts for their failed gates.

## Remaining implementing owner

Continue [Simplified K3 step4](../plans/simplified-v3-implementation.md), preserving
this transport/setup precursor. The selection table/read seams and metadata types
do not yet publish requirement/account bindings, deliberately share accounts or
establish original task-family credential selections. Before that publication,
retain and verify actual immutable material provenance through the existing
Secret owner; account metadata or a copied hash is not that proof. Implement
native atomic flow/account/selection publication and filesystem original-decision/
CAS recovery, then actual readiness/setup/resolution/injection and family/child/
wait consumers. Conditional selected-generation lease issuance, Google/DCR
original setup, Codex sessions and the remaining technical settings stay open.
No release, deployment, activation, full K3 or full-plan completion follows here.
