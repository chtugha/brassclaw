# brassclaw_host_runtime guardrails

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

- Own host-side composition shared across Reborn runtime lanes.
- Keep runtime-specific request shapes in the runtime crates; adapters should translate into host API contracts and delegate here.
- Compose low-level services such as `brassclaw_network` and `brassclaw_secrets`; do not duplicate URL parsing, DNS checks, private-IP filtering, HTTP clients, secret stores, or redaction logic in runtime crates.
- Host HTTP egress lives under `src/egress/`: keep request validation/sanitization, credential-source resolution, staged network-policy lookup, staged secret injection, transport dispatch, response sanitization, and response-body storage as separate pipeline steps instead of rebuilding a monolithic service method.
- Production host HTTP egress must be constructed with staged `NetworkObligationPolicyStore` and `RuntimeSecretInjectionStore` handoffs. Request-carried policy and direct `SecretStoreLease` sources are legacy/test compatibility paths only.
- Preserve the accounting invariant: `network_egress_bytes` is outbound request bytes only, with response bytes tracked separately.
- Keep raw secret material inside the narrow lease/injection path. Reject runtime-supplied manual credentials, scan raw and percent-decoded URL forms, redact leased values from runtime-visible errors and responses, strip sensitive response headers, and block credential-shaped runtime requests/responses before they reach external services or runtime callers.
- Do not own product workflow, authorization/approval policy, persistence migrations, or event emission unless a later Reborn contract explicitly moves that composition here.

## Agent-loop touch points

- `turn_scheduler.rs` owns scheduler-backed run concurrency. It does not own the
  canonical loop tick or product inbound serialization.
- `surface.rs` owns host-runtime capability-surface shaping and versions.
- `production.rs` and `services.rs` compose runtime services and readiness
  evidence used by Reborn loop wiring.
- Production wiring must reject local-only runtime policy shapes, not just require
  that some `EffectiveRuntimePolicy` value is present.
- First-party runtime tools belong under `first_party_tools/`; do not append new
  built-ins to broad runtime files.

## Adding code

- Add a new runtime service module when the service has its own authority,
  readiness, or resource accounting boundary.
- Add a first-party tool file per capability, except for tightly-coupled
  v1-compatible coding-tool families that share one legacy surface contract.
- Keep readiness checks near the runtime service they validate; driver/product
  readiness belongs in `brassclaw_reborn`.

## Common mistakes

- Do not call `AgentLoopDriver` or compose loop families here.
- Do not own product adapter routing or workflow idempotency.
- Do not bypass host API contracts with runtime-specific shortcuts.
