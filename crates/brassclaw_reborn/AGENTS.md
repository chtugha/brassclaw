# Agent Map — brassclaw_reborn

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

- Read `CLAUDE.md` first; it is the crate-local guardrail file.
- Read `Cargo.toml` for actual dependencies and feature shape.
- Use these neighboring contracts before changing behavior:
  - `crates/brassclaw_agent_loop/CLAUDE.md`
  - `crates/brassclaw_turns/AGENTS.md`
  - `crates/brassclaw_loop_support/CLAUDE.md`
  - `crates/brassclaw_reborn_composition/CLAUDE.md`

## What This Crate Owns

- Standalone Reborn composition/adapters bridging neutral contracts to concrete Reborn loop execution.
- `planned_driver.rs`, `planned_driver_factory.rs`, `driver_registry.rs`, and `text_loop_driver.rs` driver behavior/registration/readiness.
- `loop_driver_host.rs` concrete loop host-port composition for claimed runs.
- `loop_exit_applier.rs` validation/application of loop exits and runner transitions.
- `app_loop_family.rs` app loop-family composition and `milestone_events.rs` milestone event surfacing.
- `turn_runner.rs` the concrete turn-runner composition over the neutral `brassclaw_turns` runner contract.
- `runtime.rs`, `model_gateway.rs`, `model_routes.rs`, `production_readiness.rs`, and secrets/model runtime seams.

## Do Not Move In Here

- Loop family/executor behavior owned by `brassclaw_agent_loop`.
- Neutral runner/host contracts owned by `brassclaw_turns`.
- Product-facing binding/idempotency/gate routing owned by product workflow.
- Hidden fallback from planned to text-only paths; fallback must be explicit product/ops policy.

## Validation

- Fast local check: `cargo test -p brassclaw_reborn`
- Run specific integration tests when touched: `driver_registry`, `planned_driver_e2e`, `loop_driver_host`, `model_routes`, `production_readiness`.
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`

## Agent Notes

- Add a new file when adding a new driver, registry concern, host factory concern, or runtime adapter.
- Keep `runtime.rs` limited to planned-runtime composition and explicit profile/runtime setup.
- Do not expose planner strategy slots through Reborn APIs.
- Do not duplicate neutral DTOs from `brassclaw_turns`.
