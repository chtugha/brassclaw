# Retained first-party kernel dispatch

## Binding preloadable Skill interface (v3)

A Skill is one Tool-usage pattern with prose and explicitly associated PythonCode
exposing a preloadable function interface. Declare public names/signatures,
private helpers/constants and dependencies. Resolve one approved catalogue
snapshot; pin exact interface/code/association/artifact revisions and export
resolution. Load definitions in deterministic dependency-first order, rejecting
cycles/conflicts and effectful initializers. Invoke the pinned export on demand
with typed data; loading is not an invocation or a new Recipe effect step.

Preloaded code does not automatically enable a Tool. Its matching ToolSkill
binding and current kernel checks still apply before every actual dispatch.
Reusable code and immutable constants may be shared; mutable arrays, defaults,
closures, inputs and results remain isolated per task/attempt/invocation. Running
and resumed tasks keep their selected exports when new revisions activate.
Each real Skill has a canonical execution Recipe with a matching command.
MCP tools/list derives from available approved mcp-call-skill-recipes, not raw
Skill rows. Keep the server always running; Kohai connects/advertises to the
provider after final prefix addition just before sending a prompt, then
disconnects that request on the complete answer. Refresh discovery at
startup/restart and qualified Skill/Recipe catalogue changes. Existing calls
keep their advertised contract and normal chat task snapshot.
MCP tools/list gives its exact sentence, variable positions/types, escaping and
valid examples; the model sends the completed command for intent matching.
MCP accepts the completed listed command, opens a new ordinary chat, sends it
as a user message, forwards the correlated chat result and closes the chat.
It accepts no Python and has no direct Monty/IBS/Rust Tool execution connection;
the existing chat ingress, matcher and Recipe runner remain unchanged. No component or
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../../skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


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

`MontyTaskHost::reply_capability` adapts the existing `host.post_reply` primitive
for registration behind that kernel. It retains the exact admitted host and
checks the requested data address and typed answer before transcript dispatch.
Run/attempt freshness, cancellation and publication stay with the host. The
adapter creates no capability declaration, policy rule or component approval;
the catalogue/registration owner must provide those independently. A retained
handler cannot publish again after its host is fenced.

The actual global Match-path fixture composes separate reply and history
Recipes from immutable drafts. Monty passes the published reply reference into
the history workflow as typed data. Both Tools use retained kernel handles;
history writes use the native PostgreSQL root filesystem. A live history-Tool
block after publication records failure without replaying the reply or entering
Tier 2. This establishes constrained whole-task behavior, not approved bootstrap,
Tool artifact/ABI attestation or ordinary runtime cutover.

The component library now supplies the same typed reply/history draft definitions
used by these real native callers. Each Skill has explicit usage prose and an
exact association; reply and history variants have ten ordered input examples.
The named history helper must remain outside incoming intent routes. Draft
construction reuses a selected Tool UUID and never creates a new primitive.
Exact package-source comparison covers the actual retained Recipe, ToolSkill,
Skill, PythonCode, input layout and formatter bytes. It returns their retained
selection separately from the Tool's metadata/artifact obligations. This is source
integrity only: it neither issues combination approval nor publishes a catalogue.
A changed immutable code revision fails that comparison while an already retained
original remains readable. The bootstrap owner must still supply actual required
reviews, implementation/ABI verification and coherent activation.
