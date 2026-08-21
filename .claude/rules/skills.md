---
paths:
  - "crates/brassclaw_skills/**"
---
# Skills System

`SKILL.md` files extend the agent's prompt with domain-specific instructions. Each skill is a YAML frontmatter block (metadata, activation criteria, required tools) followed by a markdown body injected into the LLM context.

Skills in the v3 Reborn stack are stored in the database (`brassclaw_skills` crate, `PgSkillStore`) scoped to `(tenant_id, user_id, agent_id)`. They are also importable from bundled `.md` files at boot time via `crates/brassclaw_reborn_composition/src/skill_import.rs`.

## Trust Model

| Trust Level | Source | Tool Access |
|-------------|--------|-------------|
| **Trusted** | Agent-scoped DB-stored skills (user-owned) | All tools available to the agent |
| **Installed** | Downloaded from registry or URL, stored with provenance metadata | Read-only tools only (no shell, file write, HTTP) |

## SKILL.md Format

```yaml
---
name: my-skill
version: 0.1.0
description: Does something useful
activation:
  patterns:
    - "deploy to.*production"
  keywords:
    - "deployment"
  exclude_keywords:
    - "rollback"
  tags:
    - "devops"
  max_context_tokens: 2000
requires:
  bins: [docker, kubectl]
  env: [KUBECONFIG]
---

# Skill instructions here...
```

Only the top-level `requires:` block is supported. The legacy nested shape
`metadata.openclaw.requires` is unsupported and ignored by the current parser.

## Selection Pipeline (`crates/brassclaw_skills/src/selector.rs`)

1. **Gating** (`gating.rs`) — Check binary/env/config requirements; skip skills whose prerequisites are missing
2. **Scoring** — Deterministic scoring: keywords (10/5 pts, cap 30) + patterns (20 pts, cap 40) + tags (3 pts, cap 15). `exclude_keywords` veto (score = 0 if any present). Pattern (regex) scoring is gated on a runtime config flag; when disabled, regex activation contributes 0 and only keywords/tags/explicit mentions can select a skill.
3. **Budget** — Select top-scoring skills within `SKILLS_MAX_TOKENS` prompt budget
4. **Attenuation** — Minimum trust across active skills determines tool ceiling; installed skills lose dangerous tools

## Skill Tools

First-party tools registered in `crates/brassclaw_host_runtime/src/first_party_tools/` handle:
- `skill_list` — List all discovered skills with trust level and status
- `skill_search` — Search registry for available skills
- `skill_install` — Install a skill from raw SKILL.md content or registry
- `skill_install_url` — Fetch and install a skill from an HTTPS raw SKILL.md, ZIP bundle, or supported GitHub repository/tree URL
- `skill_remove` — Remove an installed skill
