---
paths:
  - "crates/brassclaw_skills/**"
---
# Skills

Read [recipe.md](../../recipe.md) before v3 component work. A Skill (classes
1–3) is one reusable Tool usage: orchestrator-facing prose plus explicitly
associated executable PythonCode. ToolSkills (class 13) are Rust-side IBS
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
