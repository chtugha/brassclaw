# 06 — Tools System (the Rust Executioner)

> **Subsystem:** Tools (class 0) — the **Rust Executioner**: the capability-bound handler layer
> that runs **only** what the Orchestrator (Monty) calls. A Tool row is the durable, DB-stored
> definition that links an orchestrator `host.*` / `builtin.*` reference back to a registered Rust
> handler. Opaque to the LLM. This doc is **f5 — the Rust-Executioner-System**, including the
> mandatory orchestrator-command-driven execution rule and its exceptions.
> **Grounded in:** `crates/brassclaw_pg/migrations/V030__reborn_tools.sql` + `V071__reborn_tools_syntax.sql`,
> `crates/brassclaw_capabilities/` (`CapabilityHost`, obligations, conformance),
> `crates/brassclaw_host_runtime/src/first_party_tools/` + `services/process_executor` + `sandbox_process/`,
> `crates/brassclaw_engine/src/executor/dynamic_tool_port.rs` (`DynamicToolPort`),
> `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs` + `pg_tool_store.rs`,
> `saved_plan_to_v3.md` §0.16, Steps C.2/C.3/C.4.5.

## 1. Purpose — the Executioner (muscle)

BrassClaw Reborn is split into an **Orchestrator** (Monty/Python — the brain; sole sequencing
authority) and an **Executioner** (Rust — the muscle). A **Tool** (class 0) is the Executioner's
unit of work: a Rusty-only execution capability with a JSON-Schema param spec and a declared
side-effect class. The Executioner **does no sequencing, no planning, no prompt assembly** — it
dispatches a single tool invocation per Orchestrator call and returns.

Tools are **opaque to the LLM** — excluded from all retrieval queries. The Orchestrator reaches a
Tool in two ways:
1. **Builtin `host.*` / `builtin.*` calls** — the first-party handlers registered in Rust
   (`first_party_tools/`, provider `"builtin"`), dispatched directly.
2. **Composed `rust_directives`** — a matched recipe's ToolSkills (class 13) are composed into the
   `ComposedProgram.rust_directives`; the C.3 `DynamicToolLoader` (cdylib) applies them so the
   Executioner can dispatch the resulting tools. This application is a C.5/C.6 wiring concern
   (deferred); the directives are **carried** in the composed program until then.

The DB table `reborn_tools` is the durable, scope-isolated, validation-gated home for these
definitions; `capability_id` (V071) links each row to its Rust handler.

## 2. Location

- **Migration:** `V030__reborn_tools.sql` (live, class 0) + `V071__reborn_tools_syntax.sql`
  (drops `cdylib_artifact_path`, adds `capability_id TEXT NOT NULL DEFAULT ''`).
- **Capability host / approval / obligations:** `crates/brassclaw_capabilities/` — `host.rs`
  (`CapabilityHost`), `requests.rs`, `obligations.rs`, `conformance.rs`, `error.rs`,
  `tool_registry.rs`. Contracts: `docs/reborn/contracts/{capabilities,capability-access,approvals,run-state}.md`.
- **First-party tool handlers:** `crates/brassclaw_host_runtime/src/first_party_tools/`
  (provider `"builtin"`): `echo`, `http`, `http_output`, `json`, `memory`, `model_visible_output`,
  `shell` (+`shell_core`), `skill_management`, `skill_url_install`, `spawn_subagent`, `time`,
  `trigger_management`. Param schemas from Rust schema structs (`schemas.rs`). Sandboxed subprocess
  execution via `services/process_executor` + `sandbox_process/`.
- **Engine dispatch port:** `crates/brassclaw_engine/src/executor/dynamic_tool_port.rs`
  (`DynamicToolPort` — `is_loaded` + `invoke`; the cdylib *load* lives downstream in
  `brassclaw_host_runtime`'s `DynamicToolLoader`).
- **Production store:** `crates/brassclaw_reborn_composition/src/pg_tool_store.rs` (+ facade).
- **Builtin seed (shipped, C.2):** `crates/brassclaw_reborn_composition/src/seed_builtin_host.rs`
  seeds the `host.*` meta-tools idempotently at boot (`source="system"`, `validated`,
  `capability_id` set). The Phase-L `~85–90 component` library across 5 ExtensionCatalogues is the
  starter set recipes compose from.

## 3. Data model — `reborn_tools` (V030 + V071, class 0)

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
| `source` | TEXT default 'authored' | includes `'system'` (system rows skip Q2) |
| `validation_status` | TEXT default 'pending' | pending/upgrade_queued/auto_failed/auto_passed/validated/review_requested/rejected/garbage |
| `capability_id` | TEXT NOT NULL DEFAULT '' | **(V071, shipped)** the Rust dispatch id, e.g. `builtin.shell`, `host.compose_orchestrator` |
| lineage | | `similarity_parent_id`,`replaces_id`,`parent_version`,`content_hash`,`last_audit_at`,`audit_failure_count`,`parent_mission_id` |

`cdylib_artifact_path` was **dropped in V071** (it reversed V067): Tools are ready-to-run handlers
addressed by `capability_id`, not on-disk artifact paths. V071 back-fills `capability_id = name`
for any pre-existing rows. Unique: `(scope, name)`. Indexes: `consumer_tags` GIN, scope+status,
scope+class+uid (assembly order), partial `WHERE validation_status='validated'` on (scope, name).
`set_updated_at()` trigger.

### `capability_id` (V071, shipped)

Links a Tool row to its Rust handler (`builtin.read_file`, `host.compose_orchestrator`, …) without
fragile name-search lookups. The Executioner uses it when resolving a Tool UUID / `host.*` call to
its registered handler. For the seeded `host.*` meta-tools `capability_id` equals the call name.

### Relationship to ToolSkill (class 13) and the capability registry

- A **ToolSkill** binds to a Tool via `tool_name` (denormalized) / `tool_id` (UUID). The ToolSkill
  is the rust-channel body the composition system emits; the Tool is the handler it ultimately
  invokes.
- The **capability registry** (`brassclaw_capabilities::tool_registry`) holds the Rust handlers
  keyed by `capability_id`; the DB Tool row is the durable, scope-isolated, validated counterpart.
- `effect_type` is mapped from the handler's `EffectKind`; `param_schema` from the Rust schema
  structs. A Tool carries **no prompt text** for Monty/LLM — Rusty-only.

## 4. Behavior / flow

1. **Builtin registration:** the first-party tools are registered in Rust (`first_party_tools/`,
   provider `"builtin"`); the `host.*` meta-tools are seeded into `reborn_tools` at boot (C.2,
   `source="system"`, `validated`, `capability_id` set).
2. **At turn time (Matching-Mode):** `host.resolve_intent` → `host.compose_orchestrator` returns
   `{ok, program}`. `program.rust_directives` carry the recipe's ToolSkill→Tool bindings for the
   cdylib loader (C.3 `DynamicToolLoader`); `program.steplist` steps carry `executable_code` Monty
   runs via `host.run_program`. As Monty iterates the steplist and issues `host.<tool>(...)` calls,
   the Executioner dispatches each through `CapabilityHost`:
   **authorization → obligations → approval gate → invoke**.
3. **Approval / auth:** `CapabilityHost` is the **single caller-facing authority path** — no
   parallel dispatch, no dispatch before authorization/obligations/approval gates. `effect_type`
   drives the gate: `read` tools run freely; `write`/`exec`/`network`/`mixed` require the approval
   flow. `shell` and `spawn_subagent` carry extra safety invariants (below).
4. **Validation:** an authored Tool enters `pending` with `05:validator`; Q1 auto-validation + Q2
   manual review graduate it to `validated` (the `05:validator` tag is removed). System tools skip
   Q2 (Q1 still runs inside the seeder; Q1 errors are a build-time/CI bug, not a runtime failure).

## 4a. The orchestrator-command-driven execution rule + exceptions (f5)

**Rule.** Every Executioner action originates from an Orchestrator `host.*` call. The Rust
Executioner runs **only** what the Orchestrator calls — builtins (`host.*` + first-party
`builtin.*`) and cdylib tools loaded per `rust_directives`. **Rust does not sequence steps, pick
recipes, or assemble prompts.** There is no Rust-side step machine; the retired `default.py` /
`__execute_action__` step machine was deleted in C.1.

**Exceptions** — a host call / execution that may proceed (or be refused) without an explicit
orchestrator step command, because it is safety-critical enforcement owned by the kernel /
Executioner, not the Orchestrator:

1. **Stop-signal honoring.** A `host.check_signals()` call (or an out-of-band stop) is honored
   immediately by the Executioner / loop runner — an in-flight tool is interrupted and the run
   winds down. The Orchestrator does not get to veto a stop.
2. **Budget hard-stops.** The Monty VM wall-clock / token / USD budget (`loop_engine.rs`
   `max_duration_override`, `__check_budget__`) is enforced by the Executioner side; when
   exhausted, execution is halted mid-step regardless of the steplist's remaining steps.
3. **Sandbox refusal.** `services/process_executor` + `sandbox_process/` may **refuse** a `shell` /
   subprocess invocation that violates sandbox policy (path allowlist, egress, resource limits).
   The refusal is returned to the Orchestrator as a tool error; the Orchestrator cannot override
   the sandbox to force execution.
4. **Approval-gate enforcement.** The `CapabilityHost` authorization → obligations → approval gate
   may **block** a `write`/`exec`/`network`/`mixed` tool pending user approval — again surfaced as
   a tool error / pending-approval state, not bypassable by the Orchestrator.

These four are the **only** autonomous Executioner actions. They never produce agentic work; they
only gate, stop, or refuse. Everything else is Orchestrator-command-driven.

### Safety invariants (Q1-enforced)

- **§shell-guard:** any recipe whose rust channel references `builtin.shell` **must** have
  `llm_call_required: true` — open-ended shell is never Tier 0. Known-safe commands may be Tier 1
  at high Wilson, never Tier 0 without explicit allowlisting. The shell ToolSkill body must include
  an explicit approval-gate description.
- **§spawn_subagent:** the ToolSkill must document that a child cannot exceed parent scope, budget
  inheritance, and the authorization model; any recipe using it is Tier 1
  (`llm_call_required: true`).
- **§capability-id:** a `source="system"` Tool row must have a non-empty `capability_id` matching
  `^[a-z0-9_-]+\.[a-z0-9_.]+$`; `capability_id` is **optional** for user-authored custom tools.

## 5. Relations

- **ToolSkills** (`05`): the rust-channel body that binds to a Tool by `tool_name`/`tool_id`.
- **IBS / Composition** (`04`): `rust_directives` carry the ToolSkill→Tool bindings the
  `DynamicToolLoader` applies; composition never fetches a Tool into the orchestrator channel
  (class 0 is excluded from retrieval).
- **Orchestrator** (`13`): the sole sequencing authority; the Executioner boundary and
  command-driven rule are defined there and enumerated here (§4a).
- **Recipe System** (`03`): a recipe's `rust_steps` reference ToolSkills → Tools.
- **Kernel / Capabilities** (`16`): `CapabilityHost` is the authority path for invoke/resume/spawn;
  approvals, obligations, and run-state are kernel-owned.
- **Validation Queue** (`14`): Q1/Q2 graduation; `source='system'` bypasses Q2.

## 6. Status — shipped vs. pending

**Shipped:**
- `reborn_tools` (V030) is live; `capability_id TEXT NOT NULL DEFAULT ''` + the drop of
  `cdylib_artifact_path` shipped in V071. `'system'` is in the `source` CHECK.
- The first-party handlers exist in Rust (`first_party_tools/`); `CapabilityHost`, obligations,
  conformance, and the approval gate all exist and are contract-governed; sandboxed subprocess
  execution via `services/process_executor` + `sandbox_process/`.
- The `host.*` meta-tools are seeded at boot (C.2, `seed_builtin_host.rs`).
- The engine `DynamicToolPort` (`is_loaded` + `invoke`) is wired; the cdylib `DynamicToolLoader`
  + ABI contract shipped in C.3 (slice 2–4).

**Pending:**
- **C.5/C.6 driver wiring:** the engine Monty VM `execute_orchestrator` host-call path — and
  therefore `host.compose_orchestrator` / `PgCompositionPort` / `host.run_program` — is constructed
  and unit-tested but **inert in production** until the C.5/C.6 driver (the active production path
  is the TURNS `PgOrchestratorLookup` bridge). The driver also activates `rust_directives`
  application via the `DynamicToolLoader`.
- **Phase-L full library:** the `~85–90 component` builtin library across 5 ExtensionCatalogues is
  the starter set recipes compose from (the `host.*` meta-tool seed is shipped; the full
  `builtin.X` task library is Phase L).
- **WebUI (Phase K.1):** Tool authoring UI for user-authored custom tools.

## 7. LLM-relevant summary

A Tool (class 0, `reborn_tools` V030 + V071) is the **Rust Executioner's** unit of work:
`name`, `param_schema` (JSON Schema), `param_template`, `effect_type`
(`read`/`write`/`exec`/`network`/`mixed` — drives the approval gate), `capability_id` (V071 — the
Rust dispatch id, e.g. `builtin.shell`), `consumer_tags` always `{00:rusty}`+`05:validator` until
validated. Tools are opaque to the LLM and excluded from retrieval; the Orchestrator reaches them
via `host.*`/`builtin.*` calls and composed `rust_directives`. **The orchestrator-command-driven
rule:** every Executioner action originates from an Orchestrator `host.*` call — Rust does not
sequence, pick, or assemble. **Exceptions** (safety-critical, kernel-owned, never agentic):
stop-signal honoring, budget hard-stops, sandbox refusal, approval-gate enforcement. `CapabilityHost`
is the single authority path (auth → obligations → approval → invoke). Q1 enforces §shell-guard
(shell ⇒ `llm_call_required=true`, never Tier 0), §spawn_subagent (Tier 1), and §capability-id
(system rows need a valid `capability_id`). C.5/C.6 activates the composition host-calls +
`rust_directives` application in production; Phase L fills the full builtin task library.
