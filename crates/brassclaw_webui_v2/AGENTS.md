# Agent Map — brassclaw_webui_v2

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

## Start Here

- Read `CLAUDE.md` first; it lists the route table and boundary rules.
- Read `Cargo.toml` for the feature gate and dependency shape.
- Use these contracts as the source of truth before changing behavior:
  - `src/descriptors.rs`
  - `tests/webui_v2_descriptors_contract.rs`
  - `tests/webui_v2_handlers_contract.rs`

## What This Crate Owns

- The native WebChat v2 routes and their host-owned ingress
  descriptors.
- Axum handler functions that dispatch to `RebornServicesApi`.
- The HTTP wire shape (`WebUiV2HttpError`) for redacted error
  responses.

## Do Not Move In Here

- Bearer-token validation, CSRF/origin enforcement, body/rate-limit
  middleware. Those live in host composition, gated by the
  `IngressPolicy` the descriptor declares.
- Direct access to dispatcher, `HostRuntime`, run-state, DB stores,
  capability hosts, or any runtime lane.
- Product adapter transport/rendering logic, storage backend details,
  or unredacted user content in responses, logs, or docs.

## Agent Notes

- Adding a new route requires both a handler **and** a matching entry
  in `webui_v2_routes()`. The descriptor test will fail otherwise.
- Handlers receive `WebUiAuthenticatedCaller` via `axum::Extension`.
  If the extension is missing, axum surfaces 500 — exercise this with
  the regression test in `webui_v2_handlers_contract.rs` rather than
  hand-rolling a fallback.
- All HTTP errors must travel through `WebUiV2HttpError`, never
  hand-built `StatusCode` returns. That keeps the redacted-error
  vocabulary intact.

## Validation

- Fast: `cargo test -p brassclaw_webui_v2 `
- Lint: `cargo clippy -p brassclaw_webui_v2 --all-features --tests -- -D warnings`
- Boundary: `cargo test -p brassclaw_architecture reborn_crate_dependency_boundaries_hold`
