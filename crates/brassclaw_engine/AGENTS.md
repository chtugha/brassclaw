# Agent Map — brassclaw_engine

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

- Read `CLAUDE.md` first; it is the crate-local guardrail file (five primitives, thread state machine, execution loop, capability leases, data-retention rule).
- Read `MONTY.md` before touching Tier-1 CodeAct/scripting; it documents the embedded Python interpreter's pin and supported-feature limits.
- Read `Cargo.toml` for the actual dependencies and feature gates and note the ReDoS guardrail at the top of `src/lib.rs`: do not add `fancy-regex` without redesigning `executor/orchestrator.rs::__regex_match__`.
- Full roadmap: `docs/plans/2026-03-20-engine-v2-architecture.md`; per-project sandbox: `docs/plans/2026-04-10-engine-v2-sandbox.md`.

## What This Crate Owns

- The unified thread-capability-CodeAct execution engine (engine v2), which replaces ~10 legacy abstractions with five primitives — Thread, Step, Capability, MemoryDoc, Project. Currently:
- Core data types (`types`, no async/no I/O): `Thread`/`Step`/`Capability`/`MemoryDoc`/`Project` and their IDs, `ThreadEvent`/`EventKind`, `ThreadMessage`/`MessageRole`, `Provenance`, conversation surfaces, and the `EngineError`/`ThreadError`/`StepError`/`CapabilityError` family.
- External-dependency traits the host implements via bridge adapters (`traits`): `LlmBackend` (over `LlmProvider`), `Store` (over the `Database` backends), `EffectExecutor` (over `ToolRegistry` + `SafetyLayer`), and `WorkspaceReader`.
- Capability management (`capability`): `CapabilityRegistry`, `LeaseManager`, `LeasePlanner`/`CapabilityGrantPlan`, and the deterministic `PolicyEngine`/`PolicyDecision` (`Deny > RequireApproval > Allow`, with provenance taint).
- Gate pipeline (`gate`): `GatePipeline`, `LeaseGate`, `ExecutionGate`/`GateController`/`GateDecision`/`GateResolution`/`ResumeKind`, and tool-tier classification (`ToolTier`, `classify_tool_tier`).
- Step execution (`executor`): the retired `ExecutionLoop`/`context`/`compaction` modules were removed in v3 Phase C.7 — turn sequencing belongs to the supervised global Monty service; `GlobalMontyDriver` only hands admitted work to it. Live modules: Tier-0 structured tool calls (`structured`), Tier-1 CodeAct via Monty (`scripting`, `orchestrator`), the Tier-0 deterministic facade (`tier_zero_orchestrator`), context/prompt building (`thread_context`, `prompt`), composition/dynamic-tool/kohai ports (`composition_port`, `dynamic_tool_port`, `kohai_port`), DB skill loader (`db_skill_loader`), LLM code-audit gate (`code_audit`), and execution trace recording (`trace`).
- Internal runtime (`runtime`): signal/`ThreadOutcome` messaging (`messaging`) and internal store writes (`internal_write`). The `ThreadManager` / `ConversationManager` / `ThreadTree` / lease-refresh modules were retired in v3 Phase C.7 — the supervised global Monty service owns turn sequencing, with instance startup/transport in `brassclaw_reborn_composition`.
- Memory document system (`memory`): `MemoryStore`, `RetrievalEngine`, `SkillTracker`.
- Workspace mounts (`workspace`): `MountBackend`, `ProjectMounts`/`WorkspaceMounts`, `ProjectMountFactory`.
- `ReliabilityTracker` (per-action EMA success/latency) and the prompt templates in `prompts/*.md` (loaded via `include_str!`).

## Do Not Move In Here

- A dependency on the main `brassclaw` crate, product transport/channels, or UI behavior — the engine is testable in isolation.
- Safety logic (sanitization, leak detection): applied at the `EffectExecutor` adapter boundary in the host, not here.
- Provider-specific LLM auth or concrete `Store`/database backends: those are host bridge adapters behind the traits.
- Deletion of LLM output. Thread messages, steps, and events are never deleted; in-memory `Store` HashMaps are a cache that evicts to bound RAM, but `load_thread`/`load_steps`/`load_events` must fall back to the database.
- Secrets, raw host paths, backend error details, and unredacted user content in errors, events, snapshots, logs, or docs.

## Validation

- Fast local check: `cargo test -p brassclaw_engine`
- Lint: `cargo clippy -p brassclaw_engine --all-targets -- -D warnings`
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`
- After changing a trait surface (`LlmBackend`/`Store`/`EffectExecutor`), add caller-level tests in the host bridge adapters that implement them.

## Agent Notes

- Thread state transitions go through `ThreadState::can_transition_to()`; terminal states are `Done` and `Failed`.
- Tier-1 CodeAct follows the RLM pattern: context-as-variables (not attention input), recursive `llm_query()`, and compact output metadata between steps. These historical helper APIs are migration surfaces. Ordinary v3 execution uses the global supervised service, shared task accounts and live resource settings; old helper constants are not global defaults. Monty 1.0 has no allocation-count limit.
- Installed-but-unauthed provider tools are direct-callable: the auth preflight raises an `Authentication` gate at execute time and the OAuth callback resumes the parked VM. Tools needing user-driven setup (`NeedsSetup`, `Inactive`, `AvailableNotInstalled`) are surfaced under `Activatable Integrations`; the model cannot enable them itself.
- The v1 learning-missions system (`MissionManager`, `ensure_learning_missions()`, `fire_on_system_event()`, `start_event_listener()`) and `Mission`/`MissionId`/`MissionCadence`/`MissionStatus` types were deleted in the v3 H.5 obsolescence cleanup (O2.2/O2.3) — dormant v1 routines-reborn code, never live-active. Knowledge extraction is now handled by the v3 recipe/skill/tool/validation system (Phases H.6+). `ThreadType::Mission` is retained as dormant API.
- Keep multi-line prompt templates in `prompts/*.md`, never inline as Rust string constants.
