# Agent Map — brassclaw_turns

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
- `docs/reborn/contracts/turns-agent-loop.md`
- `docs/reborn/contracts/turn-persistence.md`
- `docs/reborn/contracts/turn-runner.md`
- `docs/reborn/contracts/loop-exit.md`

## What This Crate Owns

- Host-layer turn coordination contracts (above the Reborn kernel facade), currently:
- Adapter-facing coordinator: `TurnCoordinator`/`DefaultTurnCoordinator`, `TurnAdmissionPolicy`, run-wake notifier ports (`coordinator`); request/response surface `SubmitTurnRequest`/`ResumeTurnRequest`/`CancelRunRequest`/`GetRunStateRequest` (`request`) and `SubmitTurnResponse`/`ResumeTurnResponse`/`CancelRunResponse`/`ThreadBusy` (`response`).
- Trusted runner transition ports (`runner`, kept out of the adapter prelude).
- Canonical typed IDs and references: `TurnId`, `TurnRunId`, `TurnRunnerId`, `RunProfileId`/`RunProfileVersion`, `IdempotencyKey`, `TurnLeaseToken`, gate/message/result/binding refs (`ids`); turn scope/actor (`scope`).
- Admission control: limits, buckets, capacity denials, providers (`admission`).
- Run-profile contracts: `AgentLoopDriver` + descriptors/run/resume requests, run-profile resolution/registry/resolver, prompt/context/model/capability profile ids, resource-budget tiers, scheduling/concurrency classes, redacted provenance (`run_profile`, which has its own `CLAUDE.md`).
- Loop-exit protocol: `LoopExit`/`LoopCompleted`/`LoopFailed`/`LoopBlocked`/`LoopCancelled`, evidence ports, applier, mapping, validation (`loop_exit`).
- Lifecycle events + projection: `TurnLifecycleEvent`, `TurnEventKind`, `TurnEventSink`, projection service/cursor/source (`events`).
- Status/error vocabulary: `TurnStatus`/`TurnRunState`/`TurnError`/`TurnErrorCategory`, admission rejections, sanitized failure/cancel reasons (`status`).
- Turn/checkpoint state stores: `TurnStateStore` + records (turn/run/checkpoint/idempotency/active-lock, persistence snapshot) (`store`); checkpoint + loop-checkpoint state stores (`checkpoint_state`); in-memory (`memory`) and filesystem (`filesystem_store`) backends.
- Crate-local public API, tests, and fixtures needed to prove that ownership.

## Do Not Move In Here

- raw CapabilityHost/dispatcher/runtime handles, raw prompts/content/tool inputs/secrets/host paths, or channel identity parsing.
- Secrets, raw host paths, backend error details, and unredacted user content in errors, events, snapshots, logs, or docs.

## Validation

- Fast local check: `cargo test -p brassclaw_turns`
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`
- If production persistence behavior changes, add/maintain PostgreSQL and libSQL parity tests.

## Agent Notes

- Keep edits inside this crate unless a contract explicitly requires a neighboring crate change.
- Prefer caller-level tests when a helper gates dispatch, persistence, network, secrets, approvals, resources, events, or process side effects.
- If the contract and code disagree, stop and treat the task as a contract-change request instead of silently changing ownership.
