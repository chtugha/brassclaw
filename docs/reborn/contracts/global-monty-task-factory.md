# Global Monty task factory

`OwnedGlobalTaskFactory` owns admission and task-port construction for the
global-driver candidate. It is exercised by the actual contained worker and
native PostgreSQL tests in `tests/monty_control/tests/model_host.rs`. It is not
registered by the ordinary application factory yet and does not certify the
production cutover.

The factory checks the exact conversation, accepted-message, turn and run IDs
against the admitted host before database I/O. Conversation IDs remain opaque.
The global driver loads the exact admitted message and complete eligible prior
history separately; neither is interpolated into Python source.

Before persisting an admission, the factory retains its private address and host
in a bounded registry. A lost commit acknowledgement, catalogue failure or
dropped preparation future therefore cannot discard that address or authorize a
new reservation. Unresolved preparation consumes capacity and requires explicit
reconciliation. The database's run-level admission uniqueness remains effective
after local ownership is transferred. No claim token enters Python/model data.

The supplied catalogue provider must capture one coherent approved generation
with its complete revision, review and implementation references. The factory
does not match, compile a workflow, run Tools, supply an empty catalogue or
convert catalogue errors into No-Match. The native fixture explicitly supplies
a draft-validation catalogue, which is not a production activation owner.

Recipe composition opens one task-owned idle parent context. Each selected
Recipe executes beneath that actual same-task parent; later named workflows
therefore do not try to allocate a second task root. Monty supplies their inputs
and passes returned values explicitly. Parent ownership shares no implicit
variables, claim identity or authority between Recipe contexts. The worker
checks parent ownership/liveness and retains the shared task compute account.
Opening the parent retains its accepted transport ticket before awaiting and
the actual receipt afterwards. Abandoning that wait resumes the original
request, never allocates another root. A rejected/failed request remains evidence.
Child release is not root completion, durable settlement or permission to reset
the active task's account.

Only the already-running global service executes Python and returns an actual
task receipt. Successful settlement requires the host's real published reply,
no withheld late answers, and acknowledged durable admission settlement. Failed
settlement retains the host, private admission, child/Tool port state and actual
receipt together. `take_failed_settlement` transfers those records only after
durable acknowledgement; it does not assert external-effect reconciliation or
permit retry. Concurrent acknowledgements cannot release a different admission
registered under the same attempt address. No registry mutex crosses I/O.

The driver retains one shared durable acknowledgement future for each actual
service receipt. A dropped drive/stop waiter leaves that future available to the
next addressed stop. Concurrent drive/stop calls cannot duplicate a successful
acknowledgement after the factory releases a completed task. A persistence
failure permits only idempotent acknowledgement of the original receipt and
private admission; it cannot rerun Python, a provider or a Tool. Taking submitted
settlement requires successful durable acknowledgement and transfers ownership
once.

The addressed stop deadline covers both real worker settlement and the durable
acknowledgement. A still-running host future or failed/uncertain database write
returns an error and retains evidence. The admission audit write remains allowed
after cancellation, lease expiry or reclaim: its original private key, checksum
and exact-outcome comparison fence the write. Admission/start/selection/Tool
intent checks still require the current running claim. Audit persistence does
not reopen dispatch or publish a late reply.

For Completed audit settlement, the factory compares the service's reply
reference with the host's recorded finalized reference. This supervisor evidence
remains readable after waiter fencing; it authorizes no active call. The active
driver/Recipe completion lookup remains fenced, and withheld results still
require reconciliation. A stop after actual root completion records that original
Completed outcome rather than inventing cancellation or replaying the reply.

Completion also requires every selected Recipe to have completed its supported
flat stop-only workflow. The child executor checks each requested occurrence
against the retained order before allocation or Tool intent; it does not select
the next step. A failed/in-flight feed is excluded from the confirmed successful
prefix. Uncomposed, skipped or failed selected workflows cannot be certified by
a published reply. An invalid root completion claim is a protocol failure with
real service/task evidence retained for reconciliation, never a successful receipt.

After actual service quiescence, the factory retains a bounded execution report
before database acknowledgement. The report includes selected Recipe checksums,
confirmed step prefixes, pending/failed step identities and actual task compute
accounting. It records no raw inputs/results, claim secrets or transport handles.
The admission transaction stores it with the original outcome; an uncertain commit
reuses the retained report and outcome without replaying execution. Legacy fixture
settlements without a root service receipt do not receive such a report.

This operational report is separate from `WorkflowReview` expectations and their
observed value fingerprints. It sets semantic approval and catalogue activation
to false; neither a complete execution nor a failed negative case supplies Q2,
protected-root trust or association-combination approval. Empty selected-Recipe
lists on genuine No-Match are distinct from skipped selected workflows.

V107 also protects admission identity and terminal outcome in PostgreSQL.
Admissions begin reserved, may start once and settle monotonically; queued
cancellation may settle directly without inventing a start time. Exact repeated
start checks remain no-ops. Settled outcomes/timestamps cannot be rewritten or
reopened, and evidence cannot be deleted/truncated. The migration is repeatable
and preserves existing rows. These guards complement private-address CAS;
they neither establish worker quiescence nor grant Tool permission.

The ordinary factory still needs the real approved-catalogue and protected-root
owners, retained implementation identity, shared live resource wiring and durable
continuation/recovery supervision. The original seven composition failures
remain open while it constructs `PersistentMontyDriver`. This task factory is a
prerequisite, not a Rust-loop fallback or proof that the v3 cutover has shipped.
