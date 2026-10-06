# Agent Map — brassclaw_reborn_composition

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
  - `crates/brassclaw_reborn/AGENTS.md`
  - `crates/brassclaw_reborn_config/AGENTS.md`
  - `crates/brassclaw_host_runtime/AGENTS.md`
  - `crates/brassclaw_turns/AGENTS.md`

## What This Crate Owns

- Facade-shaped production composition root for Reborn.
- Top-level factories that expose `HostRuntime`, `TurnCoordinator`, readiness, runtime/profile inputs, and LLM catalog wiring: `RebornServices`/`build_reborn_services` (`factory`), `RebornBuildInput`/`RebornBuildError`, and the feature-gated LLM catalog resolvers (`llm_catalog`).
- The `RebornRuntime` conversation-level facade (`RebornRuntime`/`build_reborn_runtime`, `AssistantReply`, `ConversationId`, `RebornRuntimeError`) and its runtime inputs (`RebornRuntimeInput`/`RebornRuntimeIdentity`, `TurnRunnerSettings`/`PollSettings`, heartbeat/poll-interval defaults).
- Product-live adapter wiring (`product_live_adapters`): `ProductLivePlannedRuntimeAdapters`, capability authority/IO/model-route settings, `capability_allowlist`, `visible_capability_request_for_run`; and the WebUI facade (`webui`).
- Production and migration-dry-run profile validation for required handles (`profile`, `readiness`).

## Do Not Move In Here

- Root `brassclaw` crate or `src/` module dependencies.
- Lower substrate handles in public facade APIs.
- Legacy bridge modes without accepted migration contract.
- Live v1/product traffic routing; callers must opt into explicit Reborn adapters.
- Low-level policy internals owned by service crates.

## Validation

- Fast local check: `cargo test -p brassclaw_reborn_composition`
- Run profile/runtime tests when composition/profile behavior changes.
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`
- Run `scripts/reborn-e2e-rust.sh` for production wiring changes.

## Agent Notes

- Keep composition facade small and explicit.
- Fail closed on local-only or missing required handles in production/migration-dry-run profiles.
- Add readiness checks near the composed dependency they validate.
