# Post-turn review reconciliation and native release

> **Current status — 2026-10-22:** use [the combined implementation and ownership
> index](../implementation-status.md) for implemented boundaries, applicable validation and remaining
> work. Earlier dated source observations below are historical where superseded;
> binding contracts remain in force. Implement each shared responsibility in its
> named owning plan, then reuse it here. A source-only/closed check is not public
> activation, deployed support or full-v3 completion.


The operator’s **Reconcile retained effects and accounting** action is in Settings’
validation panel. It performs native evidence/accounting recovery, never a second
Sempai review. Recording an observation remains a separate action.

The supported API is
`POST /api/webchat/v2/post-turn-reviews/{attempt_id}/reconcile`, with the instance
operator bearer and body `{"evidence_checksum":"<checksum from inspection>"}`.
Inspect the attempt first with
`GET /api/webchat/v2/post-turn-reviews/{attempt_id}`. A changed observation without
an existing reconciliation proof returns a conflict; inspect again. Caller bodies
cannot supply usage amounts, resolved effects, provider responses or release flags.

The delivery owner also performs this bookkeeping between admissions and after
its worker stops. Neither path admits the original reservation again. Recipe
sequencing, proposal handling and acknowledgement remain unchanged by this work.

Release requires all of the following:

- A real retained `TaskControl` supplies the actual Monty settlement receipt. The
  service has closed the task’s children and settled host futures; reported compute
  usage is present and has no accounting failure or withheld root answers.
- The owner and immutable selection, prefix and model identities still match the
  stopped work. A successful internal completion must match that attempt’s stored
  acknowledgement. Failed/incomplete outcomes remain failed/incomplete.
- Every operation intent has an exact native answer, and its kernel invocation
  returned an accounted result. A generic runtime error cannot establish this.
  Missing output writes are repaired from the native bytes and checked by readback;
  conflicting recorded bytes prevent release.
- Provider work either never entered reservation, or has an observed successful
  response and confirmed model accounting. Failed provider calls and lost responses
  remain unknown. Dropping a future after provider entry cannot release its hold.
- Every possible submission in an attempted batch is read by its immutable derived
  ID. Existing subjects must match the exact candidate, dependencies and base. Absent
  subjects after native settlement are recorded as absent. No missing candidate is
  submitted, no batch is resumed, and nothing is activated.
- Exact settlement and reconciliation evidence has been persisted and read back
  before the owner drops the retained native state and capacity credit.

`reconciliation_recorded` means the immutable native proof exists.
`retention_released` reports whether this call released a retained native owner;
repeating a completed action can report false alongside an existing proof.
`work_replayed` is always false. A row phase or operator note alone never authorizes
release. The original work phase, response and completion receipt stay immutable;
late native model evidence is kept in the separate proof rather than rewriting them.

## Accounting retry contract

`LoopModelBudgetAccountant::reconcile_retained_model_work` and
`ResourceGovernor::reconcile_retained` are opt-in interfaces. Unsupported custom
implementations return no reconciliation support and keep resources retained.

The governor-backed accountant retains the usage computed for the initial
settlement attempt, including pricing. Both bundled governors can settle an active
reservation or read back an already-settled reservation with exactly the same usage.
A changed usage, unknown reservation or released reservation fails. Identical
readback does not charge again or emit another reconciliation event.

## Deliberate unresolved cases

Missing native owners after process loss, unknown provider spend, unsupported
accountants, unknown kernel accounting, incomplete worker settlement and conflicting
records stay unresolved. This does not implement provider billing reconciliation
from external invoices or reconstruct lost native contexts after restart. An
operator’s assertion of zero spend cannot replace those prerequisites.

Evidence can contain private conversation/model content. The operator-only route
uses the existing bearer, origin/CSRF, body-limit and rate-limit boundaries; exact
retained text is displayed as text, never executed. Reconciliation proofs are
append-only, checksum protected and bounded to 32 MiB; evidence is never truncated.

## Local validation status (2026-10-10)

Passing evidence covers both bundled governors’ exact idempotent accounting
readback, recovery of a settlement whose acknowledgement was lost, rejection of
changed model requests/responses, six WebUI state checks, the authenticated
PostgreSQL-backed inspection/reconciliation HTTP boundary, and a successful review
through the real global Monty service. The model is a deterministic provider
fixture; these tests do not establish Ornith response quality or remote deployment.
Affected implementation crates also passed strict Clippy with warnings denied.

The final lost-response-write and partial-submission regressions passed through
real PostgreSQL and the global native service: **2 passed, 0 failed**. They cover
failed response/proof persistence, recovery through the operator facade, immutable
proofs, original failed outcomes, a partially committed candidate batch, one
provider invocation, no replay, and clean native shutdown. The cloned-pool owner
lookup and fixture subject-count issues found in the first run are corrected.
Concurrent callers report release only for the actual native-entry remover.

The final build captured matching worker/tests with unchanged before/after source
fingerprints. It used `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target`, incremental
compilation disabled, no dev/test debug symbols, and the composition feature set
`skills-db,root-llm-provider,test-support`. Native failure acceptance ran
`brassclaw_reborn_composition native_review_reconciliation --test-threads=1`;
both route descriptor contract cases also passed from that build. The macOS
linker reported a large-unwind-table warning; it did not prevent compilation or
execution. General Sempai workflow conversion, process-loss native reconstruction
and full v3 cutover remain separate work.

Source fingerprint manifest SHA-256: `b93eb410f73472514734fcdf2ee837478832113740dc25601b54446a25bcdbad`.


## Named internal admission

Completed-turn review uses the host-selected entry
`{"kind":"internal_recipe","name":"completed-turn-review"}`. Monty asks the
owning task ports to resolve that named Recipe with its retained source reference,
then composes and executes the selected steps. This private entry does not call
public intent matching or synthesize a Match. Ordinary chat delivery supplies
`{"kind":"chat"}`; JSON-looking message text cannot change that entry.

An internal Recipe returns an opaque durable receipt reference. The mechanical
service does not interpret its namespace as approval or completion evidence. The
review owner verifies the exact attempt reference, stored acknowledgement and
finished Recipe execution. Ordinary chat ports reject internal completion.
Cancellation, owner checks, selection retention and kernel dispatch checks remain
in force. This transport also provides the named entry needed by the planned
validator workflows; it does not implement or qualify those eight validators.

Worker protocol 15 carries this envelope. Both validators before and after the
worker adds its routing token reject unknown fields or malformed entries. Legacy
chat payloads without an entry retain their ordinary meaning under the matching
wire protocol. Mixed worker/application protocol versions fail before readiness.
Native protocol-15 PostgreSQL/global checks pass for completed-turn review,
lost-response-write reconciliation and partial-batch handling without replay.
The source message contains JSON-shaped private-entry/receipt text, but ordinary
chat still produces its normal reply. Twenty-six actual service/worker tests pass,
including retention of the exact entry after fatal admission/queued-worker failure,
with strict host test-target lint. Isolated model handoff, IBS effect/no-replay and
invalid-frame consumers also pass with focused lint. Actual protocol-14/15 worker
binaries reject mismatched frames in both directions. The model fixtures remain
controlled; external-provider/platform and full-Q1 acceptance are separate.

Evidence is frozen in `named-internal-entry-20261013-r6` (native global cases),
`named-internal-entry-20261013-r8` (host-owned tests/lint), and
`named-internal-entry-20261013-r10` (isolated consumers/lint). R6 full composition
lint was not accepted because of a prefix-owned baseline warning; it was reported
to that work’s owner. Later focused checks do not retroactively turn that full
composition command green.
