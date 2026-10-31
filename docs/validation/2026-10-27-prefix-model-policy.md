# Original provider policy in model snapshots — 27 October 2026

**Scoped acceptance passed.** Model-snapshot configuration facts are bound to
their original provider revision. This is not canonical generation admission,
budget planning, accepted model design, final prompt measurement or serving.

## Connected implementation

The existing `PgPrefixModelDesigns` owner now verifies recorded configuration
facts when capturing, reading, listing or retrying a model snapshot. It reuses
`PgProviderRepo::read_retained_revision` for the exact recorded provider/model
identity. A per-read cache avoids resolving the same provider revision again for
each model snapshot. No new endpoint, table, mutable limit, provider selection or
role-specific policy is introduced. The shared Inference/Prefix limit editor
already uses the canonical provider settings and remains unchanged.

The original namespace, protocol, configured context and recorded token limits
must agree with that historical revision. A self-consistent content hash is not
enough to establish this provenance. Mismatches and incomplete limit-owner/value
pairs fail closed, including catalogue reads; invalid records are not silently
hidden or replaced by current values.

Earlier immutable snapshots containing neither limit field remain readable,
byte-identical and explicitly unknown. A later provider edit does not backfill
them. Unknown context for a nondefault model likewise stays unknown. These read
compatibility rules do not permit future Generate to infer missing hard limits.
The model's configured context is neither a prefix ceiling nor proof of deployed
capacity, tokenizer identity or effective cache geometry.

## Observed checks

Seven tests execute on the actual freshly emitted Composition image, zero ignored:

- Three native PostgreSQL cases verify original reads after provider CAS edits,
  exact lost-response retry, distinct revision captures, foreign-namespace
  rejection, hash-consistent substitutions and incomplete limit records,
  historical missing limits and nondefault-model context. Corruption fixtures
  append real schema-valid records; existing originals are not edited/deleted.
- Two existing versioned design-declaration tests remain green.
- Two existing actual operator Pair tests retain authenticated HTTP/PostgreSQL
  submit/status, complete native journals, mismatched suffix classification,
  missing-prefix rejection and durable reconnect without replay. The required
  operator-admission marker is reached.

Strict Composition test-feature Clippy passes in 79.12 seconds with warnings
denied; default-library Clippy passes in 16.19 seconds. The actual test-image
build passes in 214.10 seconds. The worker build is requested and yields the
same verified bytes. Shared format/whitespace checks pass. The separate 119
Auth tests belong to the other owner's receipt and are not added to these seven.

## Exact evidence

Prefix receipt:
`target/accepted-evidence/prefix-model-policy-joint-20261027/run-1/accepted.json`

SHA-256:
`6cf0777a555c846794954aa39a4571c25491d40e17a82d3c165d3e5481c36df4`.

Shared compilation/check record:
`target/validation/fs-requirement-publication-20261027/run-1/accepted.json`,
SHA-256 `b4cdeedd8d22b9a862ff0284c57018b9315567bfec8bffa8868d7b0c94cf04f5`.

- Composition executable:
  `ef2e30cafdd2843d9add0fb3b4b38ac345edc9a1c5590675a501bede5ce616db`.
- Actual sibling Monty worker:
  `a4387d629652101c90de222af962218f4ca3e153408198ca3be9c21f20311143`.
- Two-path change manifest:
  `8b1fb7258862c71f9cee6150b509af151d2edd41a6f3115a523c995a1202048d`.
- Complete compiler-input inventory:
  `987191e5164373361378719115e76e362688ffc48bcacf19722bf6ff3b29de20`.

Original runners, exact source copies, complete source inventory and raw logs
are retained. The image-only Prefix checker executes each selected case once
after the shared build, with exact positive counts and source/binary verification.
The receipt, executable, worker, inventory and both current source hashes were
independently checked again after completion. These are development checks on
the canonical workspace/target with Rust 1.96.0, not installed-instance validation.

## Remaining boundary

Draft/Generate must still resolve the applicable canonical policy at admission,
pin its revision, use the existing pure budget planner with explicit reservations
and bind the result to accepted pair/profile designs and eligible source/artifact
inputs. The completed captured-source candidate remains a separate prerequisite.
Final provider/model formatting, both-target request checks, live technical guards,
model-design acceptance and serving/cache/quality qualification remain open.
No testmachine deployment, provider configuration or model limit was changed.
