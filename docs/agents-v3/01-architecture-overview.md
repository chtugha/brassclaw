# 01 — Architecture Overview & Three-Layer Model

> **Current execution boundary — 2026-10-22:** ordinary PostgreSQL/skills-db
> startup retains one `GlobalMontyOwner`; `GlobalMontyDriver` transports work to
> that service. `global_mode.py` sequences pinned retained preload exports and
> typed result handoffs. `basic_mode.py`, the old composer/step-body examples and
> per-chat bootstrap observations below describe legacy paths. Qualified source
> support, open migration/approval/recovery work and the owning plans are in
> [the current status index](../implementation-status.md). Historical line numbers are inspection anchors,
> not current acceptance evidence. Reuse existing implementations.


## Current prefix ownership and selection — 2026-10-26

Follow the [shared selection/pair-design contract](../reborn/model-token-limits.md#prefix-selection-pair-design-and-assembly--binding-target-2026-10-26)
and [Prefix implementation owner](../plans/prefix_v3_upgrade.md). A Recipe,
Orchestrator or role selects a specific prefix; without one, base-prompt is the
regular default, including normal No-Match, not a Tier-2-only restriction.
Unavailable explicit selections/conflicts block rather than silently becoming
base. Shared knowledge/composites require an explicit content contract.

Design for formats/limits belongs to the provider/model pair; role content rules
and finished role prefixes are distinct. Same-pair Kohai/Sempai use the same
accepted design as the generator. Generate the fixed prefix specifically for
that pair. Kohai first formats only the dynamic Orchestrator prompt for the final
target pair. A connected/enabled Sempai corrects that already formatted prompt,
using its own generated reviewer prefix/target-pair design, and returns the final
dynamic prompt. Kohai then inserts the intended finished prefix unchanged exactly
once before it, without reformatting the return, and checks the complete request.
Every manual limit editor links to the existing provider revision/CAS endpoint
and shared query. Historical snapshots are not current mutable settings.
Provider facts may be retrieved through supported APIs/documentation or supplied
manually with provenance; no provider host/template/service edits are an option.

The old store, placeholder, server-template and prewarm walkthrough below describes
historical implementation/evidence where it differs. It is not current serving
qualification. Generation and prefill are separate; cached tokens are not promised
free, permanently resident or reusable without current backend-specific evidence.
Runtime cutover/remaining acceptance stays open in the implementation index.

## Binding Recipe architecture (v3)

Read [recipe.md](../../recipe.md) before authoring or changing components. This contract
supersedes older examples below where they conflict; it specifies the target,
not completed runtime or database functionality.

- Rust Tools supply primitives; many ToolSkills describe their IBS bindings;
  many Skills explain one Tool usage and have associated executable PythonCode;
  many small PythonCode components provide reusable executable building blocks.
  Recipes tell the orchestrator how to use them to fulfill task goals. Prefer
  explicit reusable steps, not fewer steps or specialized Rust workflow Tools.
- Each Recipe component step references exactly one stable component UUID.
  PythonCode may internally compose smaller PythonCode components; this is not
  a multi-component Recipe step. Keep all independent Tool calls in separate
  execution steps; the existing direct dependent-chain exception still applies.
- IBS/composition reads the newest activated, approved versions at task start
  from one consistent catalogue snapshot and pins exact UUID/version/checksum
  references in BuildInstruction, including nested dependencies. Recipes carry
  no version numbers. Execution, child steps, waits and resumption retain that
  selection; do not look up latest again during the task.
- Approved versions are immutable. Changes create new versions; authored
  versions pass Q1 and human Q2 before activation. Replacement neither deletes
  nor invalidates originals used by running/suspended tasks. Current global
  Tool policy is checked independently before every dispatch.
- Inputs and results are typed data. Use the exact input-reference grammar and
  step-local binding convention in recipe.md. Runtime values never become
  Python source. Monty owns each task's intermediate results; unrelated tasks
  and attempts stay isolated, including across child execution and waits.
- Rust-channel ToolSkill binding executes nothing and grants no permission.
  Orchestrator-channel PythonCode calls `host.<tool>(...)`. Only an actual
  No-Match enters Tier 2; errors or begun Recipe failures never replay there.

The legacy composer still contains source-substitution and historical step-body
paths. Ordinary PostgreSQL/skills-db execution uses immutable retained selections,
typed inputs, checked single-component steps and preloaded exports on the global
Monty service. Full-library migration, general pure approval, dynamic occurrences
and durable recovery remain open; see the current implementation status index.

> **Subsystem:** Whole-system architecture and the v3 target message flow.
> **Audience:** Engineers, reviewers, and the LLM base-prompt (via the conversion mechanism).
> **Grounded in:** `AGENTS.md`, `CLAUDE.md`, `crates/` structure, `MESSAGE_FLOW_AND_PLAN_AUDIT.md`,
> `saved_plan_to_v3.md` (overview + Working Rules).

## 1. Purpose

BrassClaw Reborn is a secure, local-first AI assistant built on the **IronClaw Reborn**
architecture. It targets **7B–14B LLMs within 8,192-token context windows** and is implemented as a
workspace of approximately **70 Rust crates** under `crates/`. The v3 effort ("Recipe System
Finalisation — v3", `saved_plan_to_v3.md`) closes the gap between the original vision and the
current implementation by adding an intent-driven recipe system, an instruction builder, a
multi-kind skills system, PythonCode components, a Sempai-Kohai prompt optimizer, and vLLM
prefix-cached base prompts.

This document is the map: it states the layering, the v3 target message flow, and where each new
subsystem lives. Sibling docs in `docs/agents-v3/` expand each subsystem.

## 2. Location

- **Routing map / rules:** `AGENTS.md`, `CLAUDE.md`, `crates/AGENTS.md`, `crates/*/CLAUDE.md`.
- **Plan:** `saved_plan_to_v3.md` (phases A–N), `saved_plan_to_v3_review.md` (18 findings),
  `MESSAGE_FLOW_AND_PLAN_AUDIT.md` (flow audit), `Goals_pre_v3_review.md` (two-goals audit).
- **Runtime crates (representative):**
  - Products / surfaces: `crates/brassclaw_reborn_cli/` (binary), `crates/brassclaw_webui_v2/` +
    `brassclaw_webui_v2_static/` (React SPA), `crates/brassclaw_reborn_webui_ingress/` (gateway).
  - Loops: `crates/brassclaw_agent_loop/` (stage pipeline — the live turn driver),
    `crates/brassclaw_engine/` (engine `ExecutionLoop` + Monty VM `execute_orchestrator`
      — the Orchestrator: the Python brain that sequences a composed program).
  - Kernel: `crates/brassclaw_safety/`, `crates/brassclaw_secrets/`, `crates/brassclaw_trust/`,
    `crates/brassclaw_authorization/`, `crates/brassclaw_process_sandbox/`, `crates/brassclaw_capabilities/`.
  - Composition: `crates/brassclaw_reborn_composition/` (`factory.rs`, `runtime.rs`).
  - Persistence: `crates/brassclaw_pg/` (pool + migrations `V001`–`V075`),
    `crates/brassclaw_embedded_postgres/`.
  - LLM / embeddings: `crates/brassclaw_llm/`, `crates/brassclaw_embeddings/`.
  - Skills / extensions / interceptor: `crates/brassclaw_skills/`, `crates/brassclaw_extensions/`,
    `crates/brassclaw_interceptor/`.

## 3. Data model — the layering

`AGENTS.md` defines **three conceptual layers**; `CLAUDE.md` adds a fourth shared-services layer.

| Layer | Owns | May not |
|-------|------|--------|
| **Products** | UX and surface-level composition for a deployment shape (CLI, web server, daemon). Wire together loops, capabilities, host access. | Implement agent logic directly. |
| **Loops** | Agent behavior: planning, tool dispatch, turn sequencing, approval gates, checkpointing, retries, completion. A loop is the unit of agentic execution. | Be bypassed by product code; spawn a second loop. |
| **Kernel** | Authority: trust decisions, secret resolution, safety policy, sandboxing, capability grants, session identity. | Have its boundaries overridden from product or loop code. |
| **Infrastructure** (shared services) | LLM providers, Postgres persistence, embeddings, skills, extensions, observability. | — |

**Subagent rule:** subagent spawn creates and wires child runs only; it must not implement a second
agent loop. Child planning/execution/capabilities/checkpointing/gates/retries/completion go through
the existing loop runner/driver/executor.

**Persistence:** all persistence uses Postgres; in-memory backends are acceptable for unit tests
only. Postgres is mandatory in production (Goal 2; see `Goals_pre_v3_review.md`).

## 4. Behavior / flow — the v3 message flow

BrassClaw Reborn v3 is an **Orchestrator / Executioner** system:

- **Orchestrator** (Monty / Python — the brain): recipe- and intent-driven; assembles every LLM
  prompt; calls tools via `host.<tool>(...)`; runs composed code via `host.run_program`. It is the
  sole sequencing authority — nothing in Rust decides step order.
- **Executioner** (Rust — the muscle): precompiled Tools + ToolSkills; executes on call; no
  sequencing, no planning. The Rust side is a library the Orchestrator drives, not a second loop.

### 4a. Global Monty lifecycle (binding — simplified_v3.md / AGENTS.md)

**Exactly one `GlobalMontyOwner`** starts during system startup and stays alive for the lifetime of
the BrassClaw instance. It starts after migrations, component seeding and integrity verification,
before turn workers and ingress are enabled. `GlobalMontyDriver` delivers admitted turns as bounded
tasks to that already-running service. **A turn is a bounded task, not a new global VM or OS
process.** Completing or cancelling a turn does not terminate Monty. While idle Monty awaits work
without polling or consuming LLM tokens. Only instance shutdown or supervised fatal-runtime
recovery replaces the global VM — never a parallel VM or per-chat fallback.

The ordinary `skills-db` startup already owns `global_monty_startup.rs`, `GlobalMontyOwner` and
`GlobalMontyDriver` before turn workers and trigger producers. Remaining durable recovery,
resource and platform acceptance is governed by `simplified_v3.md` Phase 3a and
`path_to_v3.5.md` V35-01, V35-24, V35-27, V35-29.

### 4b. Turn execution — two paths

**Path 1 — Live today:** the canonical Rust stage pipeline (`DefaultExecutorPipeline` via
`PlannedDriver` → `CanonicalAgentLoopExecutor::execute_family`). `RecipeStage` runs
`fetch_for_turn` → IBS → `SplitResult`. Tier-0 dispatches through `TierZeroExecutionStage` →
`PgOrchestratorLookup` → `TierZeroOrchestrator::run_tier_zero` without an LLM. Tier-1 injects
prior-knowledge context and calls the LLM. No Python VM is involved on this live path today.

**Path 2 — Target (V35-01/05, simplified_v3.md Phase 3a):** the full global Monty VM host-call
path where `host.compose_orchestrator → host.run_program` drives each step. The composition
system and IBS are already built for this path; the engine Monty VM (`ExecutionLoop::run →
execute_orchestrator`) is dormant in production until the remaining acceptance gates are met.

```
User enters a message in the chat
  │  (WebUI v2 / gateway → bearer-token auth → turn submission)
  ↓
GlobalMontyDriver delivers the turn to the already-running GlobalMontyOwner
  ↓
intent match → canonical stage pipeline (live path) OR full Monty VM path (target)
  ↓
host.resolve_intent(scope, user_text)
  ├── Match → {component_id, step_link, ...}
  │     ↓
  │   IBS: build_instruction(step_link, step_descriptions, variable_patterns)
  │     → BuildInstruction { rust_steps[], orchestrator_steps[],
  │                          variable_patterns[], llm_call_required }
  │     ↓ [live path: turns PgOrchestratorLookup bridge]
  │   host.compose_orchestrator(component_id, step_link, user_input)   [target path]
  │     → ComposedProgram { skills[], steplist[], rust_directives[],
  │                          variables{}, assembled_program, tier }
  │     ├── Tier 0 (live today): TierZeroOrchestrator → direct reply, no LLM
  │     ├── Tier 0 (target): Monty iterates steplist, calls host.<tool>(...), no LLM
  │     └── Tier 1: prompt assembly → InterceptorStage (Sempai) → ModelStage (LLM)
  │
  └── NoMatch → Tier 2: Orchestrator assembles LLM prompt (base-prompt prefix +
        per-turn body from chat history + selected memories/components)
  ↓
Result / answer / clarifying question returned to the chat
```

The retired `default.py` / `__execute_action__` step-machine (Model A) is gone — the Orchestrator
no longer "performs the recipe's steps one by one" via a fixed Python script; it composes a recipe
into the predefined structure and runs each step's `executable_code` through `host.run_program`.
The composer never runs anything and never bakes a single program string.

## 5. Relations — how the new subsystems fit

| Subsystem | Doc | Role in the flow |
|-----------|-----|------------------|
| Intent System | `02-intent-system.md` | Classifies the message; returns a recipe match or NoMatch |
| Recipe System | `03-recipe-system.md` | The matched unit of orchestration (steps + what each step needs) |
| IBS | `04-ibs.md` | Builds instructions from a recipe step; splits rust vs orchestrator channels |
| Skills System | `05-skills-system.md` | Classic / Tool / Orchestrator skills + ExtensionCatalogues (the doc namespace) |
| Tools System | `06-tools-system.md` | Capability-bound tools the Rust executor runs |
| PythonCode System | `07-pythoncode-system.md` | Orchestrator-channel executable Python (class 22) |
| Actions System | `08-actions-system.md` | Step-by-step orchestrator instructions (class 16) |
| Sempai-Kohai | `09-sempai-kohai.md` | Memorize/optimize/finalize prompts; pre-send review; idle self-optimization |
| Prefix / Base-Prompt | `10-prefix-base-prompt.md` | Pre-compiled prefix cache + `base-prompt` placeholder substitution |
| Retrieval System | `11-retrieval-system.md` | `fetch_for_turn` (intent) vs `fetch_for_consumer` (keyword UNION ALL) |
| Agent Loop | `12-agent-loop.md` | The stage pipeline that drives a turn |
| Orchestrator (Monty) | `13-orchestrator-default-py.md` | The Python Orchestrator (brain): intent → compose → iterate steplist → `host.run_program` per step. (`default.py` retired.) |
| Validation Queue | `14-validation-queue.md` | Q1/Q2 graduation + Wilson scoring for new components |
| Component Catalog | `15-component-catalog.md` | Unified tables (class 12–20) + new 22/23 |
| Kernel / Composition | `16-kernel-composition.md` | Authority + wiring |
| WebUI v2 / Prefix Tab | `17-webui-prefix-tab.md` | Authoring + prefix management UI |

## 6. Status — shipped vs. pending (v3)

**Shipped (verified, on `origin/main`):**
- **Phases A–G** complete: IBS (`instruction_builder.rs`, `types/ibs.rs`), `step_descriptions` +
  `variants` on recipes (V050); `reborn_validation_queue` (V051); `reborn_python_code` class 22
  (V052); `reborn_extension_catalogues` class 23 (V053); `step_link` on `reborn_intent_inputs`
  (V054); `PostgresSource` wired into composition; `SplitResult`/`ActionShortCircuit`; scope-bug
  fix + fallback preservation; dead `__retrieve_docs__` removed; disambiguation UX wired.
- **Phase H.0–H.12.5** complete: `last_user_text`, `TierZeroExecutionStage`, host ports
  (`LoopRetrievalPort`/`LoopOrchestratorPort`/`LoopContextPort`), Tier-0/Tier-1 retrieval, the
  `brassclaw_gateway` crate deletion (H.5 obsolescence), active-skills dormancy removal (H.8).
- **Step B → C.1–C.4** complete (order locked B→C→A): dual-nature recipe syntax; builtin host.*
  seed (C.2, `host.resolve_intent`/`host.compose_orchestrator`/`host.post_reply`/`host.fetch_component`/
  `host.resolve_component_by_name`/`host.validate_component`/`host.check_signals`/`host.kohai_complete`/
  `host.save_history`/`host.assemble_prior_knowledge`/`host.non_match_llm_answer`); the Model-A
  retirement (`default.py`/`__execute_action__`/`__execute_actions_parallel__` + the
  `loop_engine::tests`/`runtime::manager::tests` mods deleted); cdylib dynamic loading (C.3,
  `DynamicToolLoader` + `DynamicToolPort`); security settings (C.4, V068 + WebUI panel).
- **Phase C.4.5 (common component syntax)** C.4.5.0–C.4.5.17 complete: per-class DB-structure
  standardisation (V066–V075 — legacy columns dropped across all `reborn_*` component tables);
  placeholder grammar (`{{vars.NAME}}`/`{{user_input}}`/`{{component_name}}`) + Q1 gates per class;
  the **composition system = the IBS** (C.4.5.17): `composition.rs` (`ComposedProgram`/
  `compose_program`/`ComponentResolver`), `host.compose_orchestrator(component_id, step_link,
  user_input)` → `{ok, program:{skills, steplist, rust_directives, variables, assembled_program,
  tier}}`, `host.run_program` (nested `execute_code`), the engine `CompositionPort` trait +
  composition `PgCompositionPort` impl. Migrations through **V075**.
- **DB-less mode removed**: Postgres is always used (embedded or external). No in-memory fallback.

**Pending:**
- **Global Monty VM activation (path_to_v3.5.md V35-01, V35-05; simplified_v3.md Phase 3a):**
  activate the engine Monty VM host-call path (`PgCompositionPort` → `ExecutionLoop`/`execute_orchestrator` +
  `rust_directives` via `DynamicToolLoader`) as the full production path. Until then the live
  Tier-0/Tier-1 path runs through the turns `PgOrchestratorLookup` bridge. V35-05 owns
  retained selection / typed preload / export selection acceptance.
- **Dynamic occurrence support (path_to_v3.5.md V35-13/14):** checksummed branches, item
  bindings and repeated selected usages in the flow schema / IBS / qualifier.
- **Durable recovery, resource and platform acceptance (V35-24, V35-27, V35-29):** governed by
  simplified_v3.md Phase 3a.
- **Final cleanup:** Model-A, unused legacy code after the global Monty activation.

(Migration numbers per the plan's Round-2 correction; see `saved_plan_to_v3.md` header table.)

## 7. LLM-relevant summary

BrassClaw Reborn is a ~70-crate Rust agent system targeting 7B–14B LLMs in 8k context. It is
layered **Products / Loops / Kernel / Infrastructure** and is an **Orchestrator / Executioner**
system: a Python Orchestrator (Monty) is the sole sequencing authority; a Rust Executioner runs
precompiled tools on `host.<tool>(...)` calls with no sequencing of its own. A turn flows: chat →
bearer-token auth → `GlobalMontyDriver` delivers the turn to the already-running `GlobalMontyOwner`
as a bounded task (not a new VM) → canonical stage pipeline → (match → `RecipeStage`/IBS →
Tier 0: `TierZeroOrchestrator` runs deterministically, no LLM; Tier 1: LLM prompt assembly with
injected context; or no-match → Tier 2: full LLM over assembled context). The full Monty VM path
(`host.compose_orchestrator` → `host.run_program`) is the activation target (path_to_v3.5.md
V35-01/V35-05; simplified_v3.md Phase 3a); the live Tier-0/Tier-1 path today runs through the
turns `PgOrchestratorLookup` bridge. The IBS is the composition system. A multi-kind skills system
(Classic/Tool/Orchestrator/ExtensionCatalogue) is carried as a first-class array the Orchestrator
consults while stepping. A Sempai-Kohai prompt optimizer runs idle and pre-send; vLLM
prefix-cached base prompts hold the head. Postgres is mandatory; in-memory is unit-test only.
Validation gates (Q1 structural auto / Q2 human manual) score components by Wilson lower bound,
with a graduation trigger that upserts Monty-VM cursor rows.
