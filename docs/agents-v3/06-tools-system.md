# 06 — Tools System

> **Subsystem:** Tools (class 0) — the Rust execution layer's capability-bound handlers. A Tool
> row is the durable, DB-stored definition that links the orchestrator's references back to a
> registered Rust capability (`builtin.read_file`, …). Opaque to the LLM.
> **Grounded in:** `crates/brassclaw_pg/migrations/V030__reborn_tools.sql`,
> `crates/brassclaw_capabilities/` (`CapabilityHost`, obligations, conformance), `crates/brassclaw_host_runtime/src/first_party_tools/`,
> `saved_plan_to_v3.md` §0.16 + Phase L (V057).

## 1. Purpose

A **Tool** (class 0) is the lowest layer of the v3 component hierarchy: a Rusty-only execution
capability with a JSON-Schema param spec and a declared side-effect class. The orchestrator never
sees a Tool's body — Tools are **excluded from all retrieval queries** and are opaque to the LLM.
The orchestrator reaches a Tool **indirectly**: a recipe's `rust_steps` reference a **ToolSkill**
(class 13) which binds, via `tool_name`/`tool_id`, to a Tool row; the Rust execution layer resolves
that row to its registered handler through `capability_id`. The DB table `reborn_tools` is the
durable, scope-isolated, validation-gated home for these definitions; the 23 first-party builtin
tools are registered in Rust code and seeded into the table by the Phase L bootstrap.

## 2. Location

- **Migration:** `crates/brassclaw_pg/migrations/V030__reborn_tools.sql` (live, class 0).
- **Capability host / approval / obligations:** `crates/brassclaw_capabilities/` — `host.rs`
  (`CapabilityHost`), `requests.rs`, `obligations.rs`, `conformance.rs`, `error.rs`,
  `tool_registry.rs`. Contracts: `docs/reborn/contracts/{capabilities,capability-access,approvals,run-state}.md`.
- **First-party tool handlers:** `crates/brassclaw_host_runtime/src/first_party_tools/`
  (provider ID `"builtin"`); param schemas from Rust schema structs (`schemas.rs`).
- **Production store:** `crates/brassclaw_reborn_composition/src/pg_tool_store.rs` (+ facade).
- **Seeder (Phase L):** `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`
  (`seed_builtin_components`); hand-authored bodies in `crates/brassclaw_engine/prompts/builtin/`.
- **Plan:** §0.16 (builtin bootstrap), §0.16.1 (recipe list), Phase L (V057 `capability_id` +
  `source='system'`), §capability-id Q1 rule.

## 3. Data model — `reborn_tools` (V030, class 0)

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `tenant_id`,`user_id`,`agent_id`,`project_id` | TEXT NOT NULL | scope tuple |
| `name` | TEXT NOT NULL | `^[a-z0-9]…[a-z0-9._-]*…$`, 1–128; unique per scope |
| `description` | TEXT NOT NULL | 1–1024 |
| `param_schema` | JSONB | JSON Schema for the tool's input params |
| `param_template` | JSONB | concrete parameter template for structured invocation |
| `effect_type` | TEXT default 'read' | `read`\|`write`\|`exec`\|`network`\|`mixed` — side-effect class (drives approval gating) |
| `preconditions` | TEXT | optional precondition expression evaluated before invocation |
| `error_handling` | TEXT | Rusty-level error-handling policy description |
| `class_code` | SMALLINT default 0 | `CHECK = 0` |
| `prompt_uid` | BIGINT | sequence → deterministic assembly order |
| `consumer_tags` | TEXT[] default '{}' | always `{00:rusty}` + `05:validator` until validated |
| `source` | TEXT default 'authored' | `CHECK ... ('authored','extracted','migrated','imported')` — **no `'system'` until V057** |
| `validation_status` | TEXT default 'pending' | pending/upgrade_queued/auto_failed/auto_passed/validated/review_requested/rejected/garbage |
| `validation_errors` | TEXT[] | (centralised to `reborn_validation_queue` in Phase N/V059) |
| `review_feedback`,`review_attempts`(INT),`rejected_at`,`queue_code` | | (centralised in Phase N) |
| lineage | | `similarity_parent_id`,`replaces_id`,`parent_version`,`content_hash`,`last_audit_at`,`audit_failure_count`,`parent_mission_id` |
| `capability_id` | TEXT | **(V057/Phase L — does not exist today)** `builtin.read_file`, etc. |

Unique: `(scope, name)`. Indexes: `consumer_tags` GIN, scope+status, scope+class+uid (assembly
order), partial `WHERE validation_status='validated'` on (scope, name) — the capability-surface
read (`validation_status='validated' AND NOT ('05:validator' = ANY(consumer_tags))`). `set_updated_at()`
trigger. (`review_attempts` is `INT` here vs `SMALLINT` in `reborn_recipes` — V059 populate needs a
`::INT` cast, SCHEMA-01.)

### `capability_id` (V057, Phase L.1)

```sql
ALTER TABLE reborn_tools ADD COLUMN IF NOT EXISTS capability_id TEXT;
CREATE INDEX reborn_tools_capability_id_idx
    ON reborn_tools (scope…) WHERE capability_id IS NOT NULL;
```
Links a Tool row back to the Rust capability registry (`builtin.read_file`, …) without fragile
name-search lookups. The Rust execution layer uses it when resolving a Tool UUID to its registered
handler. V057 also adds `'system'` to the `source` CHECK on `reborn_tools`, `reborn_tool_skills`,
and `reborn_skills` (and adds a fresh CHECK on `reborn_recipes`, which had none — FIND-P6-02).

### Relationship to ToolSkill (class 13) and the capability registry

- A **ToolSkill** binds to a Tool via `tool_name` (denormalized) / `tool_id` (UUID). The ToolSkill
  is the rust-channel body the IBS emits; the Tool is the handler it ultimately invokes.
- The **capability registry** (`brassclaw_capabilities::tool_registry`) holds the Rust handlers
  keyed by `capability_id`; the DB Tool row is the durable, scope-isolated, validated counterpart.
- `effect_type` is mapped from the handler's `EffectKind`; `param_schema` from the Rust schema
  structs. A Tool carries **no prompt text** for Monty/LLM — Rusty-only.

## 4. Behavior / flow

1. **Builtin registration:** the 23 first-party tools are registered in Rust
   (`first_party_tools/`, provider `"builtin"`). The DB table is live but holds **zero builtin
   rows** today — the orchestrator receives no authored prior knowledge about when to use `grep`
   vs `read_file`, when `shell` requires approval, etc.
2. **Phase L bootstrap:** `seed_builtin_components(pool, scope)` (idempotent — only if the scope
   has no existing builtin components) inserts, per group: one ExtensionCatalogue; one Tool row
   per `builtin.X` (`capability_id="builtin.X"`, `effect_type` from `EffectKind`,
   `param_schema` from `schemas.rs`, `source="system"`, `validated`); one ToolSkill per tool;
   task-level Skills; PythonCode helpers; Recipes (with `step_descriptions` + an IBS
   `build_instruction` pre-flight that panics in debug on `IbsError`). Intent examples are seeded
   with the correct `step_link`. Q2 is bypassed for system-authored components (Q1 still runs
   inside the seeder; Q1 errors are a build-time/CI bug, not a runtime failure).
3. **At turn time (v3):** a recipe `Match` → the IBS emits `rust_steps[]` with ToolSkill UUIDs +
  `ToolBinding { tool_id, tool_name, params, error_policy }`. `RecipeStage` applies them to the
  Rust execution context; the Rust layer resolves the Tool (via `capability_id`) to its handler
  and dispatches through `CapabilityHost` (authorization → obligations → approval gate → invoke).
4. **Approval / auth:** `CapabilityHost` is the **single caller-facing authority path** — no
  parallel dispatch, no dispatch before authorization/obligations/approval gates. `effect_type`
  drives the gate: `read` tools run freely; `write`/`exec`/`network`/`mixed` require the approval
  flow. `shell` and `spawn_subagent` carry extra safety invariants (below).
5. **Validation:** an authored Tool enters `pending` with `05:validator`; Q1 auto-validation + Q2
  manual review graduate it to `validated` (the `05:validator` tag is removed). System tools skip Q2.

### Safety invariants (Q1-enforced)

- **§shell-guard:** any Recipe whose rust channel references `builtin.shell` **must** have
  `llm_call_required: true` — open-ended shell is never Tier 0. Known-safe commands may be Tier 1
  at high Wilson, never Tier 0 without explicit allowlisting. The shell ToolSkill body must include
  an explicit approval-gate description.
- **§spawn_subagent:** the ToolSkill must document that a child cannot exceed parent scope,
  budget inheritance, and the authorization model; any Recipe using it is Tier 1
  (`llm_call_required: true`).
- **§capability-id:** a `source="system"` Tool row must have a non-empty `capability_id` matching
  `^[a-z0-9_-]+\.[a-z0-9_.]+$`; `capability_id` is **optional** for user-authored custom tools.

## 5. Relations

- **ToolSkills** (`05`): the rust-channel body that binds to a Tool by `tool_name`/`tool_id`.
- **IBS** (`04`): `rust_steps` carry `ToolBinding`s that resolve to Tools; the IBS never fetches a
  Tool into the orchestrator channel (class 0 is excluded from retrieval).
- **Recipe System** (`03`): a recipe's `rust_steps` reference ToolSkills → Tools.
- **Kernel / Capabilities** (`16`): `CapabilityHost` is the authority path for invoke/resume/spawn;
  approvals, obligations, and run-state are kernel-owned.
- **Validation Queue** (`14`): Q1/Q2 graduation; `source='system'` bypasses Q2; the five per-table
  queue columns move to `reborn_validation_queue` in Phase N.

## 6. Status — today vs. v3

**Today:**
- `reborn_tools` (V030) is live and structurally ready but holds **zero builtin rows**.
- **No `capability_id` column** — the Rust layer cannot robustly resolve a Tool UUID to its
  handler (V057 adds it).
- `source` CHECK has **no `'system'`** — the Phase L seeder cannot insert system Tools today
  (V057 adds it).
- The 23 first-party handlers exist in Rust (`first_party_tools/`); `CapabilityHost`, obligations,
  conformance, and the approval gate all exist and are contract-governed.
- The five per-table queue columns (`validation_errors`, `review_feedback`, `review_attempts`,
  `rejected_at`, `queue_code`) still live on `reborn_tools` (Phase N centralises them).

**v3 plan adds:**
- **Phase L.1 (V057):** `capability_id TEXT` + index; `'system'` added to the `source` CHECK on
  `reborn_tools`/`reborn_tool_skills`/`reborn_skills` (+ a fresh CHECK on `reborn_recipes`).
- **Phase L.2:** `builtin_bootstrap.rs` `seed_builtin_components` — seeds ≈ 23 Tools (+ 23
  ToolSkills, 12–15 Skills, 4–5 PythonCode, 18–20 Recipes, 5 ExtensionCatalogues) across the 5
  domain groups, all `source="system"`/`validated`, with `capability_id="builtin.X"`. Hand-authored
  bodies live as `include_str!()` markdown in `crates/brassclaw_engine/prompts/builtin/`.
- **Phase N (V059):** drop the five per-table queue columns from `reborn_tools` (and re-index its
  decoder) in favor of `reborn_validation_queue`.
- **Phase E:** `fetch_for_consumer` continues to exclude class 0 from the UNION ALL (Tools are
  never retrieved into a prompt); `class_code_to_table` keeps the `10 | 50 => reborn_skills` arm.

## 7. LLM-relevant summary

A Tool (class 0, `reborn_tools` V030) is the Rust execution-layer handler: `name`, `param_schema`
(JSON Schema), `param_template`, `effect_type` (`read`/`write`/`exec`/`network`/`mixed` — drives the
approval gate), `consumer_tags` always `{00:rusty}`+`05:validator` until validated. Tools are
opaque to the LLM and excluded from retrieval; the orchestrator reaches them only via a ToolSkill's
`tool_name`/`tool_id`. `capability_id` (V057/Phase L) links a row to its Rust handler
(`builtin.read_file`) without name-search. The 23 first-party tools are Rust-registered
(`first_party_tools/`, provider `"builtin"`) but the table holds zero builtin rows today; the
Phase L seeder (`seed_builtin_components`, idempotent, `source="system"`/`validated`, Q2 bypassed)
fills it. `CapabilityHost` is the single authority path (auth → obligations → approval → invoke).
Q1 enforces §shell-guard (shell ⇒ `llm_call_required=true`, never Tier 0), §spawn_subagent (Tier 1),
and §capability-id (system rows need a valid `capability_id`; authored rows may omit it). Phase N
moves the five queue columns to `reborn_validation_queue`.
