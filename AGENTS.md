# Agent Rules

> **Primary Rule — read this first:**
> When a user asks you to add, change, or fix a capability in BrassClaw, your first answer
> is a **Recipe** — not Rust code. The component library (Recipes + PythonCode + Skills +
> ToolSkills) is where almost all behaviour lives. New Rust code is only warranted when a
> genuinely new system-level Tool is needed that no existing Tool can provide. When in doubt,
> ask: "Can this be done by wiring existing tools in a Recipe?" If yes, write the Recipe.

## Purpose and Precedence

`AGENTS.md` is the quick-start routing map for AI coding agents entering the codebase. It is not the full architecture spec. Read the relevant subsystem spec before changing a complex area. When a crate spec exists, treat it as authoritative.

Start with these deeper docs as needed:

- `CLAUDE.md`
- `crates/brassclaw_reborn_cli/AGENTS.md`
- `crates/brassclaw_reborn/CLAUDE.md`
- `crates/brassclaw_reborn_composition/CLAUDE.md`
- `crates/brassclaw_agent_loop/CLAUDE.md`
- `crates/brassclaw_llm/CLAUDE.md`
- `crates/brassclaw_reborn_webui_ingress/CLAUDE.md`
- `tests/e2e/CLAUDE.md`

## Architecture Mental Model

BrassClaw Reborn is organized in five conceptual layers:

- **Products** own UX and surface-level composition. They wire together loops, capabilities, and host access for a specific deployment shape (CLI, web, daemon). Products do not implement agent logic directly.
- **Loops** own agent behavior. They manage planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, and completion. A loop is the unit of agentic execution. Product code must not implement a second loop or bypass the loop runner.
- **Kernel** owns authority. It controls trust decisions, secret resolution, safety policy enforcement, sandboxing, capability grants, and session identity. Kernel boundaries are not negotiable from product or loop code.
- **Infrastructure** — Shared services: LLM providers, Postgres persistence, embeddings, extensions, and observability. Lives in `crates/`.
- **Component Library** — Recipes, Skills, ToolSkills, PythonCode snippets, and ExtensionCatalogues stored in Postgres. **This is where most new capabilities are added.** No crate change required to add a Recipe, PythonCode snippet, or Skill. See `builtin_bootstrap.rs` for how first-party components are seeded.

New Reborn work that genuinely requires new infrastructure belongs in `crates/`. New *capabilities* belong in the Component Library first.

## Orchestrator-First, LLM-Minimal Design (Mandatory)

**The orchestrator IS the execution engine. Rust makes tools available. The LLM
is consulted ONLY when a task requires creative reasoning, composition, or
irreversible decisions the user must confirm. Everything else is Tier 0.**

This principle governs all Recipe, Skill, PythonCode, and ToolSkill authoring.

### Global Monty lifecycle (binding target architecture)

**Exactly one global Monty orchestrator starts during system startup and stays
alive in the background for the lifetime of the BrassClaw instance.** It starts
after migrations, component seeding and integrity verification, before turn
workers, trigger producers and ingress are enabled. Readiness requires a live
orchestrator waiting for work; merely constructing a driver or seeding Python
code does not satisfy startup.

Chat messages and other admitted inputs are work items delivered to this
already-running orchestrator. A turn is a bounded task, not a new global VM or
OS process. Completing or cancelling a turn must not terminate Monty. While
idle it awaits work without polling or consuming LLM tokens. Only instance
shutdown or a supervised fatal-runtime recovery replaces the global VM.

Conversation history, task state, replies, signals, tool bindings and execution
authority remain associated with explicit conversation/run/message IDs. Global
orchestration never means one shared chat history or a shared approval grant.
Waiting for approval, auth or a child run must leave the orchestrator able to
process the events needed to resume that task. Rust owns transport, VM hosting,
durable admission and kernel enforcement; Python/Recipes own task sequencing.

**Implementation gap:** current `PersistentMontyDriver` creates a VM lazily per
`TurnScope` in `MontySessionRegistry`. This is existing code, not the target
architecture. Follow `simplified_v3.md` Phase 3a for the cutover. This lifecycle
contract supersedes older per-input/per-conversation lifecycle descriptions in
crate docs and plans; it does not override kernel or Recipe authoring rules.

### Turn Execution Flow — binding target

Every user input travels one of two paths within the already-running global
orchestrator. **Understand this before authoring anything.** The lifecycle
cutover is specified in `simplified_v3.md` Phase 3a.

```
User Input
    │
    ▼
Global Orchestrator (Monty) — already running since system startup
    │
    ▼
Intent-Matching System  (resolve_intent / fetch_for_turn)
    │
    ├─── MATCH ──────────────────────────────────────────────────────────────────┐
    │                                                                            │
    │  Composition system fetches the matched Recipe                            │
    │      │                                                                     │
    │      ▼                                                                     │
    │  IBS (Instruction-Building-System)                                         │
    │  step_link + StepDescriptions JSONB  →  build_instruction()               │
    │  →  BuildInstruction { rust_steps, orchestrator_steps }                   │
    │      │                                                                     │
    │      ▼                                                                     │
    │  Orchestrator executes steps in order:                                     │
    │    channel:"rust"         → bind ToolSkill into Monty namespace            │
    │    channel:"orchestrator" → PythonCode: result = host.<tool>(...)          │
    │                              ↳ Rust Tool executes, returns result          │
    │    channel:"orchestrator" → (Tier 1 only) LLM step with recipe context     │
    │    channel:"orchestrator" → host.post_reply(answer="...") → user           │
    │                                                                            │
    │  History saved → task completes; global Monty awaits further work.         │
    │                                                                            │
    └─── NO MATCH ───────────────────────────────────────────────────────────────┘
         │
         ▼
     Orchestrator assembles LLM prompt (Tier 2 / Non-Matching-Mode):
         1. User's question
         2. Conversation history for this turn
         3. base-prompt prefix  (added by Kohai — precompiled from the entire
            component library: tools, recipes, skills, descriptions; ~250k–1M tokens)
         │
         ▼
     LLM answers over the assembled prompt → posted to user
         │
         ▼
     Sempai intercepts the completed turn →
     proposes new Recipe + intent examples →
     Q1 (auto audit) → Q2 (human approval) →
     next identical request runs at Tier 0 (zero LLM)
```

**What this means for authoring:**
- **Match path = Recipe.** Every capability on the Match path lives in a Recipe. To add behaviour, add a Recipe — not Rust.
- **IBS is the compiler.** It reads the Recipe's `step_descriptions` JSONB at match time and produces the `BuildInstruction`. Ephemeral — never stored. You never call IBS directly; you author correct `step_descriptions`.
- **Two-step tool invocation — always in this order:** `channel:"rust"` binds the ToolSkill (makes `host.<tool>` available), then `channel:"orchestrator"` PythonCode calls it (`result = host.<tool>(...)`). One does nothing without the other.
- **No-Match path = base-prompt.** The base-prompt is compiled from the component library by Kohai. It is not hardcoded. Adding components grows what the LLM knows in Non-Matching-Mode.
- **Sempai closes the loop.** Tier-2 turns that work well become Tier-0 recipes after Q1+Q2. The library grows with use.

### Component Roles — What Each Type IS

| Component | What it is | What it is NOT |
|-----------|-----------|----------------|
| **Tool** (class 0) | Rust callable registered in the host namespace. Executes one operation and returns. | An autonomous runner; it only executes when the Orchestrator calls `host.<tool>(...)` |
| **ToolSkill** (class 13) | Rust-side binding descriptor for IBS: which Tool to bind, param schema, preconditions, error policy. Lives in `reborn_tool_skills`. | Orchestrator instructions; it carries no prose for the Orchestrator, only metadata for IBS |
| **PythonCode** (class 22) | A Python code snippet stored in the component library. The Orchestrator runs it as a step. A tool-calling snippet calls `host.<tool>(...)` exactly once; a pure-logic snippet makes zero tool calls. | An executor — the Orchestrator is the executor; the PythonCode snippet is what gets run |
| **Skill** (class 1–3) | Orchestrator-facing **prose** describing a task pattern — loaded as LLM context in Tier-1 steps. Not executable. | A ToolSkill; Skills carry no Rust binding metadata and are never referenced in `channel:"rust"` steps |
| **Recipe** (class 21) | A turn template — like RNA. Carries `step_descriptions` JSONB listing component UUIDs in sequence. IBS reads it to assemble the `BuildInstruction` the Orchestrator executes. | A program; the Recipe is a template, IBS assembles the runnable structure from it |
| **ExtensionCatalogue** (class 23) | Domain overview: `task_groups[]` pointing to recipe names | A re-documentation of individual components it owns |

**ToolSkill vs Skill — do not conflate:**
- **ToolSkill** (class 13): consumed by **IBS** to prepare a Rust Tool binding. Referenced in `channel:"rust"` steps. No prose.
- **Skill** (class 1–3): consumed by the **Orchestrator** as LLM context. Referenced in `channel:"orchestrator"` steps. No binding metadata.

**Rust never executes autonomously.** `channel: "rust"` pre-loads a ToolSkill binding into
the execution context so the orchestrator knows which tool is available. The tool is
invoked ONLY by a `channel: "orchestrator"` PythonCode step calling `host.<tool>(...)`.
There is no other execution path. A rust step without a matching orchestrator PythonCode
step is a **Q1 hard error** (§tier0-orchestrator-channel Rule 2).

### The Two-Channel Execution Model

```
channel: "rust"           → pre-loads the ToolSkill binding (does NOT execute — availability only)
channel: "orchestrator"   → PythonCode calls host.<tool>(...) to ACTUALLY run the tool
```

Every `rust` step MUST be immediately followed by a matching `channel: "orchestrator"`
PythonCode step. One PythonCode snippet = exactly one `host.<tool>(...)` call.
The orchestrator **never** calls Rust directly — it always goes through `host.<tool>(...)`.

### Tier Decision Hierarchy

0. **Rust gate (ask this before anything else):** Does this task require a new system-level capability not provided by any existing Tool? If **no** → author a Recipe that calls existing Tools. Do not write Rust. Only if a genuinely new primitive is needed should you proceed to write a new Rust Tool, and even then it must be accompanied by a full set of components (Recipe + PythonCode snippet + ToolSkill + Leaf Skill) seeded in `builtin_bootstrap.rs`.
1. **Tier 0 first**: Can the task be done deterministically with known inputs? → Author a Tier-0 Recipe with a PythonCode snippet the Orchestrator will run. This is the default target.
2. **Split by variant**: Each distinct invocation pattern gets its own recipe + intent examples. Three narrowly-scoped Tier-0 recipes beat one Tier-1 recipe that asks the LLM to pick a path.
3. **Tier 1 only when necessary**: LLM involvement ONLY for creative content, user-composed inputs, or confirmation of irreversible actions.
4. **One leaf skill per approach**: A leaf skill describes exactly one approach to one tool. If a tool has 3 common usage patterns, author 3 leaf skills — not one monolithic skill that bundles them. A skill should never describe multiple tool calls.
5. **10+ intent examples per recipe**: More examples = better routing precision. Cover both command-style inputs and natural language.

### PythonCode Snippet Pattern (Canonical Tier-0 body)

```python
# Channel: orchestrator | Class: 22 | No I/O, no imports except stdlib, no network.
# IBS bakes {{vars.slotN}} values into the body text before execution — they arrive
# as literals, not placeholders. Tools are first-class callables in the Monty namespace.
result = host.tool_name(param="{{vars.slot0}}")
```

**VM symbols available in every PythonCode body:**

| Symbol | Purpose |
|--------|---------|
| `host.<tool>(...)` | Call a registered host tool/capability as a first-class callable (the rust-channel step binds it into the Monty namespace) |

> **Retired intrinsics (never use):** `__execute_action__`, `__execute_code_step__`,
> `__execute_actions_parallel__`, `__check_budget__`, `__emit_event__` — all retired in v3.
> Any PythonCode body that uses these will fail Q1. Call `host.<name>(...)` directly.
> See `builtin_stuff_v3.md` Step 27 for the full migration record.

**Required:** the body must assign `result = <value>` before returning.
**Forbidden:** `import os`, `import subprocess`, `exec(`, `eval(`, `open(` — scanned at Q1.

**Step isolation invariant:** each PythonCode step runs with a **fresh empty state dict `{}`**. A step does NOT see state mutations from previous steps. If step B needs data produced by step A, redesign: either combine both operations into one self-contained PythonCode body, or model the data handoff through template variables (`{{vars.name}}`).

One PythonCode step = exactly one `host.<tool>(...)` call. Pure-logic helpers (zero tool calls) are valid. Never combine two independent tool dispatches into one PythonCode block.

### What Forces Tier 1

- Content composition (write_file, apply_patch, user-composed shell commands)
- Ambiguous intent where the LLM must choose between distinct alternatives
- Irreversible operations benefiting from LLM confirmation
- User-supplied strings that must be validated before tool dispatch
- Conditional logic where step B depends on the runtime output of step A in a way that cannot be pre-determined (split into two Tier-0 recipes instead where possible)

### Q1 Hard Errors (enforced on all authored components)

- **Rule 1**: Tier-0 `orchestrator_steps` may ONLY contain PythonCode (class 22). Skill bodies are orchestrator-facing prose — they are not executable and must not be placed in recipe steps.
- **Rule 2**: If `llm_call_required == false` AND `rust_steps` has tool bindings, then `orchestrator_steps` MUST contain ≥1 PythonCode UUID. A rust-only Tier-0 recipe is rejected.
- **Rule 3**: One PythonCode snippet = exactly one **independent** `host.<tool>(...)` call. Multiple **independent** dispatches (tool A and tool B are separately useful, their outputs do not flow directly into each other) require separate PythonCode snippets — one per tool call.

  **Exception — dependent sequential chain:** If tool B's input is the direct runtime output of tool A (B literally cannot run without A's result), both calls may share one PythonCode body. This is valid because they execute in a single `host.run_program` context and share local scope. The pair must form a single logical unit (e.g. sweep → store, read → transform). Example:
  ```python
  bundle_parts = host.sweep_validated_components(user_id="{{vars.slot0}}", project_id="{{vars.slot1}}")
  result = host.store_prefix_bundle(
      user_id="{{vars.slot0}}", project_id="{{vars.slot1}}",
      bundle=bundle_parts["bundle"], generation_ms=bundle_parts["generation_ms"]
  )
  ```
  Two `host.*` calls, one body — valid because `bundle_parts["bundle"]` is the direct input to the store call. Do **not** use this exception to bundle unrelated tool calls for convenience.
- **Rule 4**: A leaf skill should describe exactly one tool usage pattern. Avoid bundling multiple tool calls or approaches into one skill body.
- **§shell-guard**: Any Recipe using `builtin.shell` is `llm_call_required: true`. **Always. No shell command is ever Tier 0**, regardless of whether the command string is fixed or user-supplied. Known-safe commands (e.g. `cargo build`) may be Tier 1 at high confidence, never Tier 0.
- **§spawn_subagent-guard**: Any Recipe referencing `builtin.spawn_subagent` is `llm_call_required: true`. Always.
- **§no-snippet**: Step type `snippet` in `step_descriptions` is rejected. Use `text` (WebUI annotation, no runtime emission) or `component` (loads a component body).
- **§body-scan**: PythonCode bodies are scanned at Q1 for `import os`, `import subprocess`, `exec(`, `eval(`, `open(`, and similar patterns — hard rejection on any match.
- **§channel-isolation**: A ToolSkill UUID must never appear in `orchestrator_steps`. A Skill UUID must never appear in `rust_steps`. Channels must not overlap.

### Recipe Tier Lifecycle — LLM as One-Time Cost

**The LLM is a one-time cost. Recipes are the permanent return.**

Each user-facing operation goes through exactly one of three tiers per turn:

| Tier | Trigger | LLM call | Token cost |
|------|---------|----------|------------|
| **0** | Recipe matched, `llm_call_required: false` | ❌ Never | Zero |
| **1** | Recipe matched, `llm_call_required: true` | ✅ Guided by recipe context | Low |
| **2** | No recipe match (Non-Matching-Mode) | ✅ Full reasoning | Full |

**Tier 2 is the seed.** The first time a user asks something new, no recipe matches. The LLM reasons through it (Tier 2). The **Sempai interceptor** watches every Tier-2 turn, evaluates the outcome, and proposes new Recipes + intent examples. Those proposals enter the **validation queue** (Q1 automated → Q2 human review). Once validated, the recipe is live — every future match for that intent pattern costs zero LLM calls.

**Pre-seeded extensions skip the discovery cost.** An extension authored as `source: "system"` (via `builtin_bootstrap.rs`) bootstraps directly to `validated` state. All operations are Tier 0 from day one, without waiting for the system to encounter them.

**The Sempai continues growing the library at runtime.** Novel combinations the author didn't anticipate — e.g. "list tasks filtered by assignee" for a task-management extension — emerge from Tier-2 turns, get proposed by the Sempai, and graduate to Tier 0 after Q1+Q2. The library compounds with use.

**Step_descriptions structure (canonical — matches `builtin_stuff_v3.md`):**
```json
[
  { "step_id": "step-0", "type": "component", "channel": "orchestrator",
    "include": ["<uuid:skill-X>"], "label": "Load skill X as LLM context" },
  { "step_id": "step-1", "type": "llm",
    "label": "LLM reasons / composes (Tier-1 only)" },
  { "step_id": "step-2", "type": "component", "channel": "rust",
    "include": ["<uuid:ts-tool-Y>"], "label": "Pre-load ToolSkill binding" },
  { "step_id": "step-3", "type": "component", "channel": "orchestrator",
    "include": ["<uuid:pc-exec-Y>"], "label": "Execute: host.tool_y(...)" }
]
```

Valid `type` values: `component` (fetch+route a component), `llm` (LLM turn — Tier-1 only), `text` (WebUI annotation, never emitted to runtime), `snippet` (rejected at Q1 — promotes to `component` after Q1+Q2).

**Posting output without an LLM:** use `host.post_reply(answer="...")` via `ts-host-post-reply` + `pc-host-post-reply`. This is the correct pattern for fixed-text Tier-0 responses. `builtin.echo` is diagnostic-only and must not be used in user-facing recipes.

### Extension Authoring Reference

**Start here for any new capability.** Extension component stacks (Tools, ToolSkills, PythonCode, Leaf Skills, Domain Skills, Recipes, ExtensionCatalogues) are fully specified in:
- `builtin_stuff_v3.md` — complete list of all built-in v3 capabilities, tool signatures, and ToolSkill/PythonCode templates
- `tomedo_v3.md` — tomedo EMR integration: full reference implementation of an extension component stack
- `docs/plans/zencoder-extension-plan.md` — **best starting point**: Zencoder REST API extension with complete annotated `step_descriptions` JSONB, intent examples, and bootstrap seeding pattern

When adding any new capability, read `docs/plans/zencoder-extension-plan.md` first — it is the worked example closest to the authoring workflow you will follow.

## Adding a New Capability (Start Here)

**Work through this hierarchy top-down. Stop at the first level that solves the problem — do not skip ahead.**

| Level | What to do | What gets created |
|-------|-----------|-------------------|
| **1** | Add intent examples to an existing `RecipeVariant` | Rows in `reborn_intent_inputs` only — zero new components |
| **2** | Add a new `RecipeVariant` to an existing `Recipe` | One variant + intent examples — reuses existing PythonCode snippets and ToolSkills by UUID |
| **3** | Reference an existing PythonCode snippet in the new variant's `step_descriptions` | No new PythonCode row — slot the existing UUID into `channel:"orchestrator"` step |
| **4** | Reference an existing ToolSkill in the new variant's `channel:"rust"` step | No new ToolSkill row — slot the existing UUID |
| **5** | Author new component rows in `builtin_bootstrap.rs` or extension seeder | New Postgres rows — no new Rust code |
| **6** | Write a new Rust Tool, then do level 5 | Only when no existing Tool provides the primitive needed |

**Before doing anything:** search `builtin_stuff_v3.md` and the existing component library for existing PythonCode snippets, ToolSkills, and Leaf Skills that already cover what you need. Reuse by UUID reference first.

**If you reach level 5, author components in this order** (Recipe-first — define what you want, then fill in what it needs):

1. **Recipe** (class 21) — define the `RecipeVariant`: intent examples, `step_descriptions` JSONB with placeholders for component UUIDs, `variable_patterns` if needed
2. **PythonCode snippet** (class 22) — the code snippet the Orchestrator will run at the relevant step; calls `host.<tool>(param=value)` exactly once per tool-calling snippet; assign `result = ...`
3. **ToolSkill** (class 13) — binding descriptor for IBS: which Tool to bind, param schema, preconditions, error policy. Referenced by the `channel:"rust"` step in the Recipe's `step_descriptions`
4. **Leaf Skill** (class 1) — one prose description per usage pattern, for LLM context in Tier-1 steps. Not executable — the Orchestrator reads it, does not run it
5. **ExtensionCatalogue** (class 23) — update `task_groups[]` if this belongs to an existing domain

Fill in the UUID references in the Recipe's `step_descriptions` as you create each component.

**Verify:** intent resolves at Class 1 or 2 confidence → Q1 passes → executes at Tier 0 (zero LLM calls for a Tier-0 recipe).

## Where to Work

| Area | Location |
|------|----------|
| **New user-facing capability (first stop)** | Author Recipe + ToolSkill + PythonCode + Leaf Skill in `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`. **No new crate or Rust function needed** unless a genuinely new system-level primitive is required. |
| brassclaw CLI binary | `crates/brassclaw_reborn_cli/` |
| Reborn runtime and driver registry | `crates/brassclaw_reborn/` |
| Composition and wiring | `crates/brassclaw_reborn_composition/` |
| Config resolution and profiles | `crates/brassclaw_reborn_config/` |
| Agent loop driver | `crates/brassclaw_agent_loop/` |
| LLM providers and routing | `crates/brassclaw_llm/` |
| Skills system | `crates/brassclaw_skills/` — v3 DB-backed store (`db-store` feature), validation helpers (always compiled). **Do not** consume the `v1-types` or `v2-compat` feature gates from new code — they are migration-importer and legacy bridge paths only. New first-party skills: `PgSkillStore::insert(NewPgSkill { ... })` in `builtin_bootstrap.rs`. |
| Security, safety, prompt injection | `crates/brassclaw_safety/` |
| WebUI v2 server (React SPA) | `crates/brassclaw_webui_v2/`, `crates/brassclaw_webui_v2_static/` |
| WebUI ingress / gateway adapter | `crates/brassclaw_reborn_webui_ingress/` |
| Extensions lifecycle | `crates/brassclaw_extensions/` |
| Host runtime shell access | `crates/brassclaw_host_runtime/` (in-kernel capability host + runtime dispatcher; sandboxed subprocess execution via `services/process_executor` and `sandbox_process/`; first-party tools under `first_party_tools/`) |
| Embeddings | `crates/brassclaw_embeddings/` |
| Recipe-Skill-Tool library | `crates/brassclaw_engine/src/memory/` (types, matcher, validator, similarity), `crates/brassclaw_reborn_composition/src/recipe_store.rs` + `recipe_library.rs` (REST store + loop adapter), `crates/brassclaw_turns/src/run_profile/recipe_lookup.rs` (trait). Recipes use `RecipeVariant` + `step_link` + `StepDescriptions` JSONB + optional `variable_patterns` — read §0.3/§0.4/§0.5 of `saved_plan_to_v3.md` before touching. |
| IBS (Instruction-Building-System) | `crates/brassclaw_engine/src/memory/ibs.rs` + `crates/brassclaw_engine/src/types/ibs.rs` (`build_instruction`, `BuildInstruction`, `IbsRecipeStep`, `ToolBinding`, `ErrorPolicy`). Compiles `step_link` + `StepDescriptions` → `BuildInstruction` at intent-match time. **Never stored** — ephemeral per call, memoised in-process. |
| Component catalog (class codes 4–23) | `crates/brassclaw_engine/src/memory/retrieval_source.rs` (`PostgresSource`, `fetch_for_turn`, `FetchForTurnResult::SplitResult`/`ActionShortCircuit`, `class_code_to_table` — the single source of truth for class→table dispatch; the full class→table table is in `CLAUDE.md` §Component Catalog and is regression-tested against the code). Tables: `reborn_extensions_unified` (4–9, extension packages) + `reborn_specs/tool_skills/plans/summaries` (12–15) + `reborn_actions` (**16**, not 11 — class 11 is unallocated) + `reborn_docus` (17) + `reborn_lessons/issues/notes` (18–20) + `reborn_recipes` (21) + `reborn_python_code` (22, Phase B) + `reborn_extension_catalogues` (23, Phase C). Classes 10 (Orchestrator) and 50 (Scaffold) are **not** separate tables — they live in `reborn_skills`, filtered by `class_code`, alongside classes 1–3. All components carry `dependency_registry JSONB` (Phase J). `reborn_component_catalog` (`crates/brassclaw_pg/migrations/V084__reborn_component_catalog_view.sql`) is a read-only Postgres VIEW — not a table — that `UNION ALL`s all 14 prompt-bearing class tables (excluding `reborn_tools`, class 0) for ad hoc/Settings-API querying; it applies no per-request scope filtering, callers add their own `WHERE`. |
| Settings API / WebUI catalog tabs | `crates/brassclaw_reborn_composition/src/pg_settings_listing.rs` (`PgSettingsListingService::list`, single parameterised query backing every `GET /api/settings/{type}` tab — Skills, Tool Permissions, Actions, Extensions, Orchestrators, Scaffolds, Recipes, ToolSkills, PythonCode, ExtensionCatalogues; a genuinely missing table fails loud with `SettingsListingError::MissingTable`, never silently empty), `crates/brassclaw_product_workflow/src/settings.rs` + `reborn_services.rs` (`RebornServicesApi` trait methods), `crates/brassclaw_webui_v2/src/{descriptors,handlers,router}.rs` (routes). Frontend: `crates/brassclaw_webui_v2_static/.../settings-schema.js` + `settings-tabs.js` (sidebar sections: Runtime Config / Component Catalog / Security & Governance / Access & Ops) + per-tab `*-tab.js` files. The runtime tool-permission list ("Tool Permissions") is UI-distinct from the class-code Skill/ToolSkill catalog tabs — do not conflate them. Note: the old v1 SKILL.md plugin installer ("Skill Packages") UI was removed; the Skills tab now shows `reborn_skills` DB rows only. |
| Validation queue | `reborn_validation_queue` table (V051, Phase A.5). Four-state pipeline: **Q1** `auto` (orchestrated, sandboxed LLM audit) → **Q2** `manual` (operator review — human-only, never automated) → **Q3** `revision` (automated revision by class-09 extension, if flagged) → **Q4** `rejection` (rejected; retained for `q4_retention_days` then wiped). All non-builtin components must pass Q1+Q2. `source='system'` builtins are **exempt** — they insert as `validated` directly. **Recovery from Q4 rejection:** read the Q1 audit output, fix the component (check for forbidden symbols, wrong class codes, missing `result =`, channel isolation violations), and re-submit. Do **not** rewrite the capability as Rust because a recipe was rejected — fix the recipe. |
| Builtin bootstrap seeder | `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs` (Phase L). Seeds full v3 component stack (Tools + ToolSkills + Skills + PythonCode + Recipes + ExtensionCatalogues) for all 23 first-party tools at boot, if not already present. Idempotent. Orchestrator and system prompt components (`orchestrator:main`, `codeact_preamble`, `codeact_postamble`, `failure_explanation`, `compaction_summarizer_fresh`, `sempai_audit`, `subagent:direction:*`) are seeded here and carry `content_checksum` (SHA-256 hex) for boot-time integrity verification. |
| Seeding boot chain | `crates/brassclaw_reborn_composition/src/webui.rs` — all `seed_*`, `run_boot_integrity_check`, `run_content_integrity_check`, and `init_*` OnceLock calls live here in strict order. Driver port wiring (`PgOrchestratorCodePort`, etc.) lives separately in `runtime.rs`. |
| Global Orchestrator lifecycle (target) | `simplified_v3.md` Phase 3a: move the shared boot prerequisites out of WebUI-only construction, start one global Monty before workers/ingress, route work through its bounded inbox and retain task-local context. Current transition sites: composition `runtime.rs`, `persistent_monty_driver.rs`, `session_registry.rs`; engine `executor/orchestrator.rs`, `orchestrator/basic_mode.py`; Reborn `turn_runner.rs`. |
| Content integrity check | `crates/brassclaw_reborn_composition/src/content_integrity.rs` (`run_content_integrity_check`) — SHA-256 prose checksum verification for `source='system'` rows. Called at boot after seeding. Hard error on mismatch. Distinct from `boot_integrity.rs` (Phase N queue-consistency check). |
| `BootedDb` newtype | `crates/brassclaw_reborn_composition/src/booted_db.rs` — type-level proof that migrations completed. All seeding and integrity functions accept `&BootedDb`, not raw `Arc<PgPool>`. |
| `brassclaw repair` command | `crates/brassclaw_reborn_cli/src/commands/repair.rs` — force-reseeds all `source='system'` rows using `ON CONFLICT DO UPDATE`. Run after a binary update that changes system prompt content. |
| `OrchestratorCodePort` | `crates/brassclaw_engine/src/executor/orchestrator_code_port.rs` — engine-side port for loading the class-10 orchestrator body. Impl: `PgOrchestratorCodePort` in `crates/brassclaw_reborn_composition/src/pg_orchestrator_code_port.rs` (gated `postgres+skills-db`). Driver construction wiring in `runtime.rs`. |
| BasicPromptStore / prefix | `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs` (Phase K.1). Stores the operator-editable base-prompt prefix; regenerated via `regenerate_prefix`. |
| Intent system | `crates/brassclaw_engine/src/memory/intent_system.rs` (`resolve_intent`, 4-class classifier, `record_disambiguation_choice`), `reborn_intent_inputs` table (V028 + V058 variable-template columns). Intent expressions support `%` slot markers for variable capture (Phase M). |
| Monty VM settings | `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs` (`PgMontyVmSettingsStore`, reads/writes `reborn_monty_vm_settings` V034 migration) |
| User chat preferences | `crates/brassclaw_reborn_composition/src/pg_user_preference_store.rs` (`PgUserPreferenceStore`, `reborn_user_preferences` V035 migration) |
| Legacy MemoryDoc migration (v1→v3, read-only concern) | `crates/brassclaw_reborn_composition/src/component_import.rs` (`run_component_import` — migrates old v1 `brassclaw_memory_docs` rows into class-specific tables at first boot on old databases). **Do not add new components via `brassclaw_memory_docs`** — author them directly in `builtin_bootstrap.rs` or the extension seeder. |
| Interceptor / Sempai-Kohai | `crates/brassclaw_interceptor/` (Sempai/Kohai review loop, persona, base-prompt assembly, `SempaiProposalSink`, `SempaiReviewOutcome`). Sempai auto-creates **all** component types (not just recipes) — proposals enter the validation queue at `'pending'`. Wired in composition via `InterceptorConfigService`. |

When a task touches only `crates/` there is no longer a v1 `src/` tree — all v1 code was removed in Phase 6.

## Subagent and Loop Rules

In v3, a subagent's behaviour is defined by the **Recipe it receives** — not by configuring loop driver code. The Orchestrator running the subagent executes the Recipe's steps. To change what a subagent does, change its Recipe; do not modify loop driver or executor Rust code.

- Subagent spawn creates and wires child runs only. It must not implement a second agent loop.
- Child planning, execution, capability calls, checkpointing, gates, retries, and completion must go through the existing loop runner/driver/executor path.
- Host-trusted trigger ingress is sealed by trigger-worker-owned request minting plus private conversation-owned trusted inbound construction.
- Product adapters, product workflow, first-party capabilities, and host-runtime handlers must use untrusted inbound requests and must not mint `TrustedInboundTurnRequest` or call trusted trigger submitter factories.

## Repo-Wide Coding Rules

- No `.unwrap()` or `.expect()` in production code. They are acceptable in tests and for truly infallible invariants (e.g., compiled-in literals, regexes) with a safety comment.
- Keep clippy clean with zero warnings: `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`.
- Prefer `crate::` for cross-module imports. `super::` is fine in tests and intra-module refs.
- Use strong types and enums over stringly-typed control flow when the shape is known.
- Use `thiserror` for error types in `error.rs`. Map errors with context: `.map_err(|e| SomeError::Variant { reason: e.to_string() })?`.
- No `pub use` re-exports unless exposing to downstream consumers.
- Comments for non-obvious logic only.
- **Do not introduce new `include_str!()` constants for behavioural prompts or scripts in production code paths.** All prompt bodies are seeded as `source='system'` DB rows via `builtin_bootstrap.rs` and loaded at boot via `OnceLock` accessors (see seeding boot chain in `webui.rs`). `include_str!()` is only permitted in `builtin_bootstrap.rs` seed constants and in `#[cfg(test)]` modules. The `.md` and `.py` source files in `crates/brassclaw_engine/prompts/`, `crates/brassclaw_loop_support/prompts/`, and `crates/brassclaw_reborn/src/subagent/directions/` remain on disk as seed sources and test references — never as compiled-in runtime fallbacks.
- `info!` and `warn!` output appears in the REPL and corrupts the terminal UI. Use `debug!` for internal diagnostics. Background tasks must never use `info!`.

## Database Rules

- All persistence uses Postgres. In-memory backends are acceptable for unit tests only.
- Treat bootstrap config, DB-backed settings, and encrypted secrets as distinct layers; do not collapse them.
- Do not break config precedence, bootstrap env loading, DB-backed config reload, or post-secrets LLM re-resolution.

## Security Invariants

- Review any change touching listeners, routes, auth, secrets, sandboxing, approvals, or outbound HTTP with a security mindset.
- Do not weaken bearer-token auth, webhook auth, CORS/origin checks, body limits, rate limits, allowlists, or secret-handling guarantees.
- Treat Docker containers and external services as untrusted.
- Session, thread, and turn state matters. Submission parsing happens before normal chat handling.
- Skills are selected deterministically. Tool approval and auth flows are special paths and must not be mixed into normal chat history.
- Persistent memory is the workspace system, not just transcript storage.

### Capability Lease Authority Invariants (`brassclaw_authorization`)

Capability leases are authority-bearing records — the rules below are not style preferences:

1. **`PgCapabilityLeaseStore` mutations must be atomic.** Every `revoke`, `claim`, and `consume` must run inside a single Postgres transaction with `SELECT … FOR UPDATE`. Never split the read and write across separate connections or pool checkouts. A TOCTOU gap here is a double-authority bug.
2. **`UPDATE` predicates must include `user_id`.** Reading is scoped to `(id, tenant_id, user_id)`; writing must use the same triple. Omitting `user_id` from the `WHERE` clause allows mutations to cross user boundaries.
3. **`consume` must call `ensure_consumable` and decrement `max_invocations`.** Directly setting `Consumed` without these steps grants additional invocations on multi-use leases and bypasses the unclaimed-fingerprint guard.
4. **`FilesystemCapabilityLeaseStore` must not silently downgrade `CasExpectation::Version` to `Any`.** The indexed-projection fallback (stripping `entry.indexed` for byte-only backends) is acceptable. Downgrading the CAS version expectation is not — it removes the cross-process ordering guarantee. If the backend cannot provide versioned CAS, fail closed.
5. **`LocalFilesystem` is not an accepted backend for authority-bearing stores.** Production wires `InMemoryBackend` (local-dev, under `/tenants`) and `PostgresRootFilesystem` (hosted). Tests that exercise mutation paths must use `InMemoryBackend`, not `LocalFilesystem`.
6. **`issue` must not return success when zero rows were inserted.** `ON CONFLICT DO NOTHING` silently absorbs duplicate-key conflicts; check `rows_affected == 1` before returning the lease to the caller.

## Testing Rules

- Add the narrowest tests that validate the change: unit tests for local logic, integration tests for runtime/DB/routing behavior, E2E or trace coverage for gateway, approvals, extensions, or other user-visible flows.
- Test through the caller, not just the helper. When a predicate/classifier/transform helper gates a side effect (HTTP, DB write, OAuth flow, UI mutation, tool execution) and has any wrapper or computed input between it and that side effect, a unit test on the helper alone is not sufficient regression coverage. Add a test that drives the actual call site at the integration tier or higher.
- Mocks of multi-arg runtime APIs must capture every argument the production caller passes.

## Key Environment Variables

**Bootstrap tier** (fixed set, read before the DB starts — set in the systemd unit's `Environment=` block):

| Variable | Purpose |
|----------|---------|
| `BRASSCLAW_REBORN_HOME` | Reborn state root (default: `~/.brassclaw/reborn`) |
| `BRASSCLAW_RUNTIME_PROFILE` | Per-invocation capability policy: `local_dev` (default), `local_safe`, `local_yolo`, `hosted_safe`, etc. — see `brassclaw runtime-profile list`. Controls the security resolver only; does **not** affect which storage backend is used (Postgres is always used). Setting `BRASSCLAW_REBORN_PROFILE` (old composition-profile name) is a hard startup error. |
| `BRASSCLAW_REBORN_LOG` | Log filter for Reborn runtime (e.g., `brassclaw=debug`) |
| `BRASSCLAW_PG_URL` | External Postgres URL. Optional for single-host local deployments (embedded Postgres is used when absent). Required for all non-local `BRASSCLAW_RUNTIME_PROFILE` values. |
| `BRASSCLAW_EMBEDDED_PG_PORT` | Override embedded Postgres port (default: 5434) |
| `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` | Override embedded Postgres listen addresses (default: `127.0.0.1`). Set to `0.0.0.0` for LAN access. First-boot only (written to `postgresql.conf` by `initdb`). |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Path to master-key file; set only when using passphrase-wrapped ceremony |

**Operator-trusted tier** (data-driven, read by configured name after the DB is up — set in `secrets.env` via `EnvironmentFile=`):

The *names* of these env vars are stored in `brassclaw_config`; the *values* are read from the environment at runtime and never persisted. Includes: `BRASSCLAW_REBORN_WEBUI_TOKEN`, `BRASSCLAW_REBORN_WEBUI_USER_ID`, provider API keys, OAuth secrets, trigger auth tokens.

## Build and Test

> **Mandatory:** Every `cargo build`/`test`/`clippy`/`check` **must** set
> `CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` (NVMe) — never build in-place on the
> slow external repo drive. **Before** compiling, check free space on that volume and clean
> it if it is too full:
>
> ```bash
> df -h /Users/ollama/brassclaw-target          # check before every compile
> # If Avail < 15 GB or Capacity > 90%, clean first:
> CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo clean
> # Then run the actual command with the target dir set:
> CARGO_TARGET_DIR=/Users/ollama/brassclaw-target cargo <build|test|clippy|check> ...
> ```
>
> The NVMe target dir accumulates multi-GB artifacts and can fill the 228 GB volume
> mid-build, starving/corrupting the run — the space check + clean is mandatory, not optional.

```bash
# Build the Reborn binary with WebUI v2
cargo build --release --bin brassclaw 

# Format
cargo fmt

# Lint a specific crate (zero warnings)
cargo clippy -p <crate_name> --all-targets -- -D warnings

# Lint everything
cargo clippy --all --benches --tests --examples --all-features -- -D warnings

# Unit tests for a specific crate
cargo test -p <crate_name>

# All unit tests
cargo test

# Integration tests (requires PostgreSQL)
cargo test --features integration
```

## Before Finishing

- Confirm whether behavior changes require updates to specs, API docs, or `CHANGELOG.md`.
- Run the most targeted tests and clippy checks that cover the change.
- Re-check security-sensitive paths when touching auth, secrets, network listeners, sandboxing, or approvals.
- Keep the final diff scoped to the task. Avoid unrelated file churn.
- **Capability check:** If behaviour was added or changed — is it expressed as a Recipe + PythonCode, or did it end up as Rust logic that belongs in a Recipe? Rust-only behaviour changes are incomplete unless a genuinely new system primitive was required.
- **Component set check:** If a new Rust Tool was written — do its Recipe + PythonCode snippet + ToolSkill + Leaf Skill + ≥10 intent examples all exist in `builtin_bootstrap.rs`? A Tool with no Recipe is unreachable at Tier 0.
