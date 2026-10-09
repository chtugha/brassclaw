# Agent Map — brassclaw_host_runtime

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
- Use these Reborn contracts as the source of truth before changing behavior:
- `docs/reborn/contracts/host-runtime.md`
- `docs/reborn/contracts/runtime-workflows.md`
- `docs/reborn/contracts/kernel-boundary.md`

## What This Crate Owns

- Host-side composition shared across Reborn runtime lanes and the kernel-facing services/adapters, currently:
- The production host runtime `DefaultHostRuntime` (`production`) and runtime-service composition/readiness: `HostRuntimeServices`, `ProductionWiring*` (component/config/issue/report), `RegisteredRuntimeHealth` (`services`).
- Capability planning and surface: `plan_capability`/`ExecutionPlan`/`PlannerError` (`planner`); the capability-surface policy `CapabilitySurfacePolicy`/`VisibleCapability`/`VisibleCapabilityAccess` (`surface`); the hot capability catalog `HotCapabilityCatalog`/`HotCapabilityRecord`/`publish_hot_capability_catalog` (`capability_catalog`).
- First-party capabilities: the `FirstPartyCapabilityRegistry`/handler/request/result (`first_party`) and the builtin tool set `BuiltinFirstPartyTools` with capability IDs (echo/time/json/http/shell/read_file/write_file/list_dir/glob/grep/apply_patch) and `builtin_first_party_handlers`/`_package` (`first_party_tools`).
- Extension contract discovery: `default_host_api_contract_registry`, `default_host_port_catalog`, `discover_extensions_with_default_host_api_contracts*` (`extension_contracts`).
- Obligation handling: `BuiltinObligationHandler`/`BuiltinObligationServices`, `ProcessObligationLifecycleStore`, and the secret-injection/network-policy stores (`obligations`).
- The runtime process port `RuntimeProcessPort`/`LocalHostProcessPort` + command execution types (`process_port`), the `TurnRunScheduler`/`TurnRunExecutor` scheduler-backed run concurrency (`turn_scheduler`), and memory-context builders (`memory_context`).
- Low-level mediation by composing `brassclaw_network`/`brassclaw_secrets`/`brassclaw_resources` (egress, redaction, secret leases, accounting) — never duplicating that logic in runtime crates.
- Crate-local public API, tests, and fixtures needed to prove that ownership.

## Do Not Move In Here

- product loop strategy, prompt assembly, channel UX, migrations, or duplicated low-level network/secrets/resource logic.
- Secrets, raw host paths, backend error details, and unredacted user content in errors, events, snapshots, logs, or docs.

## Validation

- Fast local check: `cargo test -p brassclaw_host_runtime`
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`
- If production persistence behavior changes, add/maintain PostgreSQL and libSQL parity tests.

## Agent Notes

- Keep edits inside this crate unless a contract explicitly requires a neighboring crate change.
- Prefer caller-level tests when a helper gates dispatch, persistence, network, secrets, approvals, resources, events, or process side effects.
- If the contract and code disagree, stop and treat the task as a contract-change request instead of silently changing ownership.
