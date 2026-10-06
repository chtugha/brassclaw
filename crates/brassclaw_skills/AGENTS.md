# Agent Map — brassclaw_skills

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
