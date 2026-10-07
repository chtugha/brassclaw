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

The ordinary factory still needs the real approved-catalogue and protected-root
owners, retained implementation identity, shared live resource wiring and durable
continuation/recovery supervision. The original seven composition failures
remain open while it constructs `PersistentMontyDriver`. This task factory is a
prerequisite, not a Rust-loop fallback or proof that the v3 cutover has shipped.
