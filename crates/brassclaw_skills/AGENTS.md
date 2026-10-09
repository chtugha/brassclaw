# Agent Map — brassclaw_skills

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

- No crate-local `CLAUDE.md` exists yet; use this map plus the skills rules below.
- Read `Cargo.toml` for actual dependencies and feature flags (`db-store`).
- Use these sources of truth before changing behavior:
- `.claude/rules/skills.md`
- `CLAUDE.md`
- `docs/reborn/contracts/extensions.md`

## What This Crate Owns

- Skill data types (`types`), content/name/credential validation (`validation`), the class-code component taxonomy (`component_type`), and the `reborn_skills` reader/writer (`db_store`, feature `db-store`).
- V2 engine skill types (`v2`): `V2SkillMetadata`, `CodeSnippet`, `SkillMetrics`, `SkillRevision`/`SkillRepairRecord` — serialized into `MemoryDoc.metadata` by the engine crate.
- Crate-local public API, tests, and fixtures needed to prove that ownership.

## Do Not Move In Here

- Prompt execution, tool authorization, extension runtime dispatch, credential handling, channel UI, or ClawHub server behavior.
- The removed v1 SKILL.md subsystem: filesystem install/remove, host-FS registry discovery, the remote catalog, or the gating/scoring/selection pipeline. Skills are v3 DB components.
- Secrets, raw host paths, backend error details, and unredacted user content in errors, events, snapshots, logs, or docs.

## Validation

- Fast local check: `cargo test -p brassclaw_skills`
- Feature-shape check after `db_store` changes: `cargo test -p brassclaw_skills --all-features`
- Boundary check after dependency/API changes: `cargo test -p brassclaw_architecture`

## Agent Notes

- Skill injection must stay deterministic: `db_store` returns rows ordered by `(class_code, prompt_uid)` for a byte-identical, KV-cache-friendly prompt prefix.
- `escape_skill_content` is enforced at insert time and must stay idempotent.
- Add caller-level tests when validation or store changes affect prompt assembly.
