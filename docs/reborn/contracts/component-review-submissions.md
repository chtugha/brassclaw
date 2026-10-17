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
- `expected_previous_version`: the explicitly observed allocation counter,
  separate from `base`. It includes unactivated author/package drafts. The next
  version is this counter plus one. Omitting it retains the older adjacent-base
  behavior for existing clients; new WebUI requests provide it explicitly.
- `dependencies`: every transitive proposed dependency's exact reference. Versions
  are positive integers and checksums are lowercase 64-character SHA-256 hex.

Actor identity comes exclusively from authenticated instance-operator ingress.
Unknown request fields, duplicate/missing/unrelated/cyclic dependencies, wrong
classes/checksums and a stale allocation counter are rejected. The stored proposal may reference
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
`component-review-submission/2` subject for explicit counters; old `/1` subjects
remain byte-identical and readable. V148 preserves old receipts and separates
lineage from allocation without rewriting their contents. It returns `review_status="unreviewed"`
and `catalogue_activated=false`. No legacy live component row, validation queue,
successful evidence or activation record is written by this path. Its authoring
document preserves values such as `override_prompt_creation`; future activation
and override-policy enforcement still need their own coordinated implementation.

`GET /api/webchat/v2/component-review-submissions/{submission_id}` returns the
receipt, exact subject bytes and every selected revision's exact bytes. It uses
only the retained selection, never latest heads. GET and implicit HEAD require
instance-operator authentication, just like POST. Ordinary signed sessions do
not inherit that authority from a mounted bearer branch.

When retained, the optional `review_selection_bytes` field exposes the original
`component-validator-selection/1` preparation record. V150 links it immutably
to the same submission ID and subject checksum. It includes the exact candidate
reference and existing retained Recipe selection bytes/checksum; later validator
drafts do not replace it. Inspection reads subject and selection in one coherent
read-only snapshot. No selection is created by GET or inferred from latest rows.

Its explicit `scope="preparation-only"` does not mean that a validator is approved,
admitted, executed or complete. The review owner must independently verify the
active approved private route, full class requirements, retained implementations,
attempt ownership and current policy before effects. It must establish actual
checking observations and settlement before producing qualifying Q1 evidence.
An unchanged preparation retry recovers the same bytes; different bytes under
the original submission ID conflict. Retention grants no automatic replay.

The sealed preparation reader supplies the actual exact revision graph to
`RetainedValidatorCoverage::restore_preparation`. It recompiles the selected
variant and typed bindings, rechecks the private coverage declaration and
compares the complete canonical retained selection bytes. This verifies step
order and other derived IBS fields, rather than trusting their presence in
checksummed JSON. The operator read path applies this check too. Inconsistent
records remain explicit errors requiring repair; they are neither discarded nor
replaced with a latest selection. Restoration still does not establish approval,
qualify Python source, load a retained Tool artifact or grant execution.

`GET /api/webchat/v2/component-revision-heads/{component_id}` returns only the
class and last allocated version (zero/null class for an unknown identity).
It requires the same instance-operator token. This is allocation metadata, not
a latest-revision selector, approval or active-version view. Read the counter
after selecting the candidate identity and keep it unchanged with an uncertain
request. A new package draft must not force an operator to adopt that draft as
the review base.

## Retry and capacity

V149 adds one immutable `reborn_component_review_requests` reference per retained
submission, keyed by the same submission ID and exact subject checksum. Insertion
and a deferred database invariant commit the subject and required review intent
together. A request failure rolls back revision allocation too. Upgrade creates
requests for older subjects without rewriting their bytes. Repeated submissions
retain the same request; requests cannot be changed, deleted or truncated.

`read_review_request_in_transaction` verifies the request/subject checksum link
inside the caller's repeatable-read/serializable view. Full graph loading remains
in the separate exact subject reader. A request is not a validator selection,
execution attempt, Q1 pass or activation. Global review delivery must resolve an
approved private validator, retain its exact manifest and reconcile prior evidence
before effects. This reference creates no second scheduler or automatic replay.

Keep the original ID and candidate/base/dependency selection after a timeout,
disconnect or uncertain commit. An identical retry recovers the existing receipt
before allocating a version, even when candidate/dependency heads have advanced.
Changed actor, candidate bytes, base, allocation counter or selection under that
ID conflicts (409).
Canonical dependency ordering is a set representation, not Recipe step ordering;
step order and source formatting remain in the exact candidate bytes.

Concurrent edits from one observed allocation counter cannot both win. Multiple
pending proposals may share the same exact lineage base when their allocation
counters differ; activation must still check the current approved base. A concurrent transaction or
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

The validator input reader `read_validator_subject_in_transaction` reads the
submission and its complete exact candidate/dependency graph in the caller's
repeatable-read or serializable transaction. Its private result contains typed
full revision envelopes, exact revision/subject bytes and checksums. It never
reduces a candidate to name, description and content, substitutes newer revisions,
executes source text, or writes a verdict or activation. Supplied preparation
limits reject the entire input; they never trim fields or dependencies.

An edit also includes the exact historical base root as `base_comparison`.
The submission records that root's reference, not its former dependency-version
selection or approval evidence. This comparison must not be treated as a complete
approved workflow baseline. Validator selection, behavioral/semantic review,
durable global-Monty admission and trusted evidence persistence remain separate.

`validator_components::aggregate_draft` defines a pure preloadable PythonCode
building block for exact required-rule coverage and correlated errors. Its rule
list must come from the pinned validator, never from the reviewed candidate.
It rejects duplicate/missing/unknown rules, nonboolean outcomes and contradictory
diagnostics. The draft does not qualify a class validator, prove executor
provenance or permit a caller to write successful Q1 evidence.

`validator_v3.md` Step 2's ordinary authoring, Sempai and component-management
migration is not complete. Legacy mutable writers and the old queue still exist;
this endpoint is the new operator staging path, not proof that those callers
have migrated. Missing association/structured records must be migrated explicitly.

Step 9's durable review intent is retained, but its selected-validator admission,
execution and trusted verdict delivery to global Monty remain open.
Validator selection, constrained draft execution, actual trusted evidence and
activation cannot be inferred from a retained subject. Normal global startup,
resource/continuation wiring and the original seven composition failures remain
open; these persistence/HTTP regressions cannot certify that cutover.
