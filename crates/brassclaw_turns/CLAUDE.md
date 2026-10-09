# brassclaw_turns guardrails

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
per-call Q1/Q2 is created; only eligible usages are exposed. See [the complete interface contract](../../skills.md#preloadable-function-interface-binding-v3-target).
This is a binding target, not proof of implemented loader/store/runner support.
Current-source observations and historical step-body examples below must be
migrated to this interface before being accepted as updated v3 implementations.


## Recipe architecture routing

Follow [the binding Recipe authoring contract](../../recipe.md) and root AGENTS.md for
v3 component work. Recipes instruct the orchestrator using reusable Tools,
ToolSkills, Skills and small PythonCode components. One referenced component
per Recipe step; internal PythonCode composition is allowed. IBS pins approved
immutable versions in BuildInstruction at task start, including nested includes;
Recipes reference stable UUIDs without versions. Preserve typed inputs/results
and task state across steps and waits. Replacement versions do not alter active
tasks; current global Tool policy still applies at every dispatch. Older scoped,
source-substitution or fresh-step descriptions below are implementation/legacy
notes, not permission to extend those paths as the v3 target. These requirements
remain subject to the documented runtime implementation gaps.

- Own host-layer turn coordination contracts only: canonical turn scope, turn/run IDs, adapter-safe coordinator APIs, runner transition ports, store traits, and redacted lifecycle events.
- Stay above the Reborn kernel facade. Do not depend on or re-export raw `CapabilityHost`, dispatcher, process host, runtime-lane adapters, raw filesystem, network, secrets, MCP, script, or WASM handles.
- Product adapters use `TurnCoordinator` methods only. Trusted workers may import `brassclaw_turns::runner` explicitly; do not add runner transition APIs to the public prelude.
- Mutating adapter-facing APIs must take scoped idempotency keys. `submit_turn` accepts requested run-profile hints and `received_at`; responses/state expose resolved profile id+version, not lower runtime handles.
- Consume canonical binding/session refs from upstream services. Do not parse Slack/Telegram/Web/CLI identity, channel conversation IDs, or raw transcript content in this crate.
- Active-run exclusivity is keyed by canonical scoped thread `(tenant_id, agent_id, project_id?, thread_id)` and must not include channel IDs or user IDs.
- Blocked/resumable runs keep the same-thread active lock until resume, cancel, fail, or complete. Running cancellation is two-phase: public cancel requests move to `CancelRequested`, and a trusted runner cancellation completion moves to terminal `Cancelled` and releases the lock exactly once.
- Store lifecycle metadata and references only. Do not persist raw prompts, assistant content, tool input, secrets, host paths, or backend error details in turn state or events.
- Keep concrete PostgreSQL/libSQL adapters and product projection/egress wiring out of the core contract unless a scoped follow-up explicitly adds them with parity tests.
- Loop-framework contracts live here only when they are neutral runner/host
  protocol: `LoopFailureKind`, `AgentLoopDriver`, `AgentLoopDriverHost`,
  `LoopXxxPort` traits, run-profile descriptors, refs, prompt bundle contracts,
  checkpoint load/stage contracts, progress events, and cancellation signals.
- Implementations of those contracts live elsewhere: host adapters in
  `brassclaw_loop_support`, driver-side integration in `brassclaw_reborn`, and
  reusable loop mechanics in `brassclaw_agent_loop`.
- Add a new `.rs` file before widening an existing contract file with an
  unrelated responsibility. Do not create broad `common`, `misc`, or `helpers`
  modules.
