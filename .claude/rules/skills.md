---
paths:
  - "crates/brassclaw_skills/**"
---
# Skills

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


Read [recipe.md](../../recipe.md) before v3 component work. A Skill (classes
1–3) is one reusable Tool usage: orchestrator-facing prose plus an explicitly associated preloadable
Python function interface with exports, signatures and dependencies. ToolSkills (class 13) are Rust-side IBS
binding descriptors, not Skill prose. Build a large reusable PythonCode library;
Recipes instruct the orchestrator how to fulfill tasks with these components.

Each Recipe component step references one UUID. Internal PythonCode composition
is allowed and must be validated/version-pinned transitively. IBS selects the
newest activated approved versions at task start and pins them in BuildInstruction;
Recipes do not carry version numbers. Approved versions are immutable; authored
replacements pass Q1 and human Q2 and do not change or invalidate old task
snapshots. Keep runtime values as typed data and preserve task result flow;
current source substitution/fresh-step execution are implementation gaps.

Skill prose currently lives in `reborn_skills`, code in `reborn_python_code`;
classes 10 (Orchestrator) and 50 (Scaffold) share the prose table but are not new
Skill usage types. Existing scope columns are storage/legacy implementation,
not new v3 feature-role authorization requirements.

## The v1 SKILL.md Subsystem Is Gone

`SKILL.md` files, the `/skills` and `/system/skills` install roots, the host-FS registry,
the remote catalog, the gating/scoring/selection/attenuation pipeline, the
`skill_list` / `skill_search` / `skill_install` / `skill_install_url` / `skill_remove`
first-party tools, the `skill_install` / `skill_remove` lifecycle commands, the "Skill
Packages" WebUI tab, and the repo-root `skills/` directory have all been removed.

Do not reintroduce a filesystem skill-loading path. Authoring a skill means inserting a
class-1/2/3 row through the component pipeline (Q1 automated → Q2 human review), or
seeding it as `source = "system"` in `builtin_bootstrap.rs`.

## What This Crate Still Owns

- `types` / `v2` — `SkillManifest`, `LoadedSkill`, `V2SkillMetadata`, and related data
  structures.
- `validation` — name validation, `escape_skill_content`, credential-spec validation,
  safe relative-path normalization.
- `db_store` (feature `db-store`) — the `reborn_skills` reader/writer used by
  `brassclaw_engine`'s `db_skill_loader`.
- `component_type` — the class-code component taxonomy.

## Content Safety

Every skill body is passed through `escape_skill_content` on insert
(`DbSkillStore::insert`). The injection wrapper in `db_skill_loader` applies it again as
defence in depth; the function is idempotent for content with no raw `<skill` tags.

## Validation

- `cargo test -p brassclaw_skills`
- `cargo test -p brassclaw_skills --all-features` after touching `db_store`
- `cargo test -p brassclaw_architecture` after dependency or public-API changes
