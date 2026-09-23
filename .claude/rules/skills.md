---
paths:
  - "crates/brassclaw_skills/**"
---
# Skills

A Skill is a **v3 component** (class codes 1/2/3) — orchestrator-facing prose that
describes one task pattern. Skills live in `reborn_skills`, scoped to
`(tenant_id, user_id, agent_id, project_id)`, and are injected into the base prompt by
`PgBasicPromptStore`. Classes 10 (Orchestrator) and 50 (Scaffold) share the same table
and are distinguished only by `class_code`.

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
