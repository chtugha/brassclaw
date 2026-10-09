# Reborn WebUI Ingress Agent Contract

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

This crate owns the host-side serve lifecycle for the Reborn WebChat v2
HTTP gateway. It is deliberately small: the product/API boundary is
held in `brassclaw_reborn_composition` (route descriptors + the
`Router`), and this crate's only job is to bind a listener and drive
the axum serve loop with the `Router` it gets handed.

## Boundaries

- Bind `tokio::net::TcpListener` and call `axum::serve`. This crate is
  intentionally outside the `reborn_product_api_crates_do_not_bind_http_ingress`
  forbidden list — that rule exists to keep product/API library crates
  from owning server lifecycle, and this crate is host-owned ingress
  code, not product/API.
- Provide concrete `WebuiAuthenticator` implementations the standalone
  `brassclaw` binary can wire (env-bearer first; DB / OIDC are
  follow-ups). Token comparison must be constant-time (`subtle::ConstantTimeEq`).
- Do not touch `ProductAdapter`, `ExternalActorRef`, `ProtocolAuthEvidence`,
  or other external-protocol shims — WebUI is a Path A native host
  surface (see `docs/reborn/how-to-port-channel-to-reborn.md`).
- Do not depend on v1's `src/`, `brassclaw_engine`, channel code, or
  v1 DB infrastructure.
- Do not store transcripts, threads, or any business state. Everything
  the gateway needs flows through `RebornServicesApi` from the
  `Router` the composition crate hands us.

## Allowed dependencies

- `brassclaw_reborn_composition` (consumes the composed `Router` +
  `WebuiAuthenticator` trait + `WebuiServeConfig`)
- `brassclaw_host_api` (identity types: `TenantId`, `UserId`)
- `axum`, `tokio`, `tracing`, `thiserror`, `async-trait`, `secrecy`,
  `subtle`

Any other workspace crate dependency requires an architecture-test
update + explicit PR rationale.

## Adding a new authenticator

1. Add the impl module under `src/`.
2. Implement `WebuiAuthenticator` from `brassclaw_reborn_composition`.
3. Use constant-time comparison for any secret material.
4. Add a unit test that exercises `authenticate` against a known
   token + a wrong token.
5. Add a caller-level test in `tests/` that spins up `serve_webui_v2`
   with the new authenticator on a random port and verifies bearer
   accept / reject through a real `reqwest::Client`.
