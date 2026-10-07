# Immutable component review subjects

The operator authoring path retains an **unreviewed** candidate and its complete
declared dependency graph. It reuses `ComponentRevisionDraft` and
`PgComponentRevisionStore` (V101); V104 adds the immutable submission receipt.
This infrastructure is not IBS, a validator executor, an active catalogue or a
Tool grant. Q1/behavior/human Q2 and coherent activation remain separate.

## Operator API

`POST /api/webchat/v2/component-review-submissions` accepts:

- `submission_id`: a canonical non-nil UUID chosen before the first request.
- `candidate_bytes`: the exact JSON text of a `component-revision/1` envelope.
  It retains the document, declared dependencies and explicit association bytes.
  Python-looking text remains data and is never executed by this endpoint.
- `base`: an exact `{uuid, class_code, version, checksum}` reference for an edit;
  null/omitted for a new retained identity. The UUID/class must match the candidate.
- `dependencies`: every transitive proposed dependency's exact reference. Versions
  are positive integers and checksums are lowercase 64-character SHA-256 hex.

Actor identity comes exclusively from authenticated instance-operator ingress.
Unknown request fields, duplicate/missing/unrelated/cyclic dependencies, wrong
classes/checksums and a stale base are rejected. The stored proposal may reference
draft dependencies: retention does not mislabel them as approved. Review must
establish that all execution-relevant includes, bindings, variants, schemas and
effects agree with this declared selection. The revision document's object shape
alone does not establish complete class-specific validation or semantic approval.

Version allocation uses the existing head compare-and-swap. The revision,
complete graph check and immutable submission commit together in one
repeatable-read transaction. Incomplete graphs and failed commits roll back head
allocation too. A trusted caller joining an external transaction must roll back
on any error and acknowledge only the actual successful commit.

The receipt identifies the candidate/base/dependencies and SHA-256 of the exact
`component-review-submission/1` subject. It returns `review_status="unreviewed"`
and `catalogue_activated=false`. No legacy live component row, validation queue,
successful evidence or activation record is written by this path. Its authoring
document preserves values such as `override_prompt_creation`; future activation
and override-policy enforcement still need their own coordinated implementation.

`GET /api/webchat/v2/component-review-submissions/{submission_id}` returns the
receipt, exact subject bytes and every selected revision's exact bytes. It uses
only the retained selection, never latest heads. GET and implicit HEAD require
instance-operator authentication, just like POST. Ordinary signed sessions do
not inherit that authority from a mounted bearer branch.

## Retry and capacity

Keep the original ID and candidate/base/dependency selection after a timeout,
disconnect or uncertain commit. An identical retry recovers the existing receipt
before allocating a version, even when candidate/dependency heads have advanced.
Changed actor, candidate bytes, base or selection under that ID conflicts (409).
Canonical dependency ordering is a set representation, not Recipe step ordering;
step order and source formatting remain in the exact candidate bytes.

Concurrent edits from one base cannot both win. A concurrent transaction or
unknown commit can return a retriable storage failure; retry the original request.
Reading by ID proves retention, not which pending client request committed it.
An exact POST acknowledgement is required to settle an in-memory pending mutation.

The WebUI advanced editor holds its pending request while mounted. It permits
correction after a first explicit rejection, but retains original bytes after an
uncertain response. It offers loading by stable ID after a reload. It does not
persist unsent/pending source in browser storage; keep the ID and original request
for recovery. This is not a durable browser outbox.

Technical limits reject whole subjects; none truncates their content. Candidate
revision text is bounded at 8 MiB; ingress at 14 MiB; the generated stored subject
at 16 MiB. Graphs have at most 4096 revisions, depth below 64 and 64 MiB of exact
revision text. The endpoint raises Axum's default 2 MiB JSON extractor limit only
to its declared 14 MiB limit; host per-route/global limits remain effective.
These are transport/storage constraints, not token budgets.

## Remaining cutover work

`validator_v3.md` Step 2's ordinary authoring, Sempai and component-management
migration is not complete. Legacy mutable writers and the old queue still exist;
this endpoint is the new operator staging path, not proof that those callers
have migrated. Missing association/structured records must be migrated explicitly.

Step 9 must admit durable submitted review work to the global Monty orchestrator.
Validator selection, constrained draft execution, actual trusted evidence and
activation cannot be inferred from a retained subject. Normal global startup,
resource/continuation wiring and the original seven composition failures remain
open; these persistence/HTTP regressions cannot certify that cutover.
