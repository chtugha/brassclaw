# Retained first-party kernel dispatch

`HostRuntimeServices::capture_first_party_capabilities` captures one actual
extension-catalogue view and immutable handler registry for task preparation.
It requires explicit runtime/trust policy configuration, executes nothing and
grants no permission. `retain` validates the selected capability's declaration,
provider/runtime agreement and real handler registration from that view.

`RetainedFirstPartyCapability` exposes the captured descriptor and one bounded
capability identity. Its invocation goes through `DefaultHostRuntime`, current
host trust evaluation, `CapabilityHost` authorization/prepared-dispatch checks,
obligations, `RuntimeDispatcher`, the existing first-party adapter and the actual
retained handler. It shares the original governor, invocation-service resolver,
secret/network/mount enforcement and audit/event ports. A different capability
ID is rejected before kernel invocation. There is no raw-handler or lower-store
accessor on this facade.

Registry removal/replacement and later handler registration changes affect new
captures. Existing selected handles keep their declarations/handlers across
waits. The registry snapshot shares existing immutable catalogue data; it does
not copy the entire registry for each selected Tool. Current policy remains
independent: a live stable-Tool block still prevents the next invocation.

This supplies real implementation ownership through kernel mediation. It does
not establish association/component/workflow approval, active catalogue
publication, Tool-revision-to-artifact/ABI verification, ordinary startup or the
original seven composition acceptance results. The catalogue/registration owner
must supply those remaining contracts; an actual captured handler is not their
substitute. MCP/native-library retention requires its own supported adapter.

Caller coverage uses the real builtin JSON Tool, live stable-UUID policy and
captured declarations after registry removal. Native Monty caller cases use the
same facade for actual IBS-selected child executions, journaled Tool results
and constrained validation evidence. They do not fabricate catalogue activation.
