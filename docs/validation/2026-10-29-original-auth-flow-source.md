# Original Auth-flow encrypted generation — 29 October 2026

The existing protected Auth-flow custody now optionally retains a
`credential-requirement-source/1` record: the exact selection digest and an
opaque authenticated encrypted-generation reference. The selection binds the
original flow/scope, complete usage, account/access handle and issuer deadline.
The actual owner captures the reference at the first publication/choice. It is
neither secret material nor current eligibility, task-family lineage or a grant.

Native PostgreSQL captures through the existing SecretStore inspection method
inside the caller's Auth transaction, without acquiring another connection under
row locks. Manual publication now writes its actual retained output before
capturing its proof, inside that same transaction. Selection, account, material
and flow still commit or roll back together.

RecordFS captures at its existing first decision CAS for manual, OAuth and
explicit selection. Staged retries and lost acknowledgements reuse that original
proof, including its absence. They never reread today's material to replace an
old decision. Existing publication/head/account CAS and partial-effect recovery
remain enforced. No new queue, registry, lease, workflow or primitive is added.

The existing account-record-source port reads only the requested original
completed flow. It verifies receipt and exact selection/source binding without
reading today's head or material. Missing historical proof stays absent even
when material later appears. Pending/canceled flows do not supply completion
proof; corrupted bindings fail. The metadata-only Auth fake rejects source proof
as unavailable. Default absent-field serialization preserves historical shapes.

## Validation

[The machine record](2026-10-29-original-auth-flow-source.json) identifies all
24 changed Rust files, exact source/image/log hashes and retained failure history.
The final shared-checkout test build, affected formatting, connected Composition
library/tests Clippy with warnings denied, default Composition library Clippy and
whitespace checks passed. Auth library/tests strict checks also passed.

193 distinct cases passed with no ignored cases:

- 73 freshly executed Composition cases: 3 custody cases, 11 RecordFS requirement
  publication cases, 6 native PostgreSQL requirement cases, 29 native manual/DCR/
  OAuth cases and 24 RecordFS manual publication cases.
- 120 Auth cases from run-2 remain applicable: only Composition sources/tests
  changed afterward; their observed Auth/dependency inputs and logs were checked
  unchanged. They were not rerun or represented as fresh run-4 executions.

The real memory-backed and PostgreSQL-backed RecordFS and native PostgreSQL
paths verify changed head/generation, conditional refusal of a replaced
ordinary generation, absent-source non-backfill, full-usage/owner mismatch,
manual/OAuth publication, lost writes/commits, rollback, cancellation, legacy
archives and explicit corruption rejection. Issuer responses in these tests
are fixtures; this is not real external-provider acceptance.

Two compile defects were corrected: an omitted new field in the DCR `Self`
constructor and overly private RecordFS helper visibility. The first backend
run exposed an unsupported Delete in the new absence fixture and an archive
fixture retaining the new proof while removing its selection. A distinct
material-free account now proves absence; the inconsistent archive is explicitly
rejected before testing the actual historical shape. No production assertion,
warning or failed result was suppressed.

The final Screen `brassclaw-original-flow-source-r4` completed with exit0;
runner16725 is gone and the common Cargo lock was independently checked free.
Final source inventory SHA256:
`af54b6a698b3593328722cf0e470dd6a1c13d207de446e174607982ef323ffc6`.
Composition image SHA256:
`d0ec4643b31932a24aa242b572309ce88d65c0aa09c455575c5f5471c7213389`.
Machine record SHA256:
`c40fc12a12932590b67651e8034d5fce3989003e3a228bb5fc5465ad0e43ddec`.

## Remaining boundaries

The proof identifies material observed at the original publication/choice. It
does not independently prove issuer-return lineage or current usability. Actual
family progression must verify its own original setup/previous selection and
flow address, exact source proof, live account eligibility and conditional
lease/consume; a supplied digest or flow ID grants nothing.

Original task-family own-flow progression, refresh-input capture/continuation,
native Auth-wait production, aggregate readiness for multiple credentials,
actual native extension/ToolSkill registration, raw-handle migration and complete
global/UI/final-use qualification remain K3 step4 work. Native built-in
credential lists remain empty; no workflow is activated by this infrastructure.
Durable recovery, full K3/Exact/Prefix/platform/release/deployment remain open.
Preserve the [existing source-use guard](2026-10-29-tool-source-capture-and-use.md)
and [Exact's typed prefix contract](2026-10-29-exact-prefix-input-binding.md).
