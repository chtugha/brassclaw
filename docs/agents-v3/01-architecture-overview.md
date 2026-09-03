# 01 — Architecture Overview & Three-Layer Model

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

```
User enters a message in the chat
  │  (WebUI v2 / gateway → bearer-token auth → turn submission)
  ↓
Orchestrator (Monty) receives the message
  ↓
host.resolve_intent(scope, user_text)
  ├── Match → {component_id, step_link, ...}
  │     ↓
  │   host.compose_orchestrator(component_id, step_link, user_input)
  │     → the IBS (composition system) composes the recipe + variant into the
  │       predefined structure { skills[], steplist[], rust_directives[],
  │       variables{}, assembled_program, tier }:
  │         • skills     — first-class array Monty consults while stepping
  │                         (exact tool usage; the steplist need not repeat it);
  │         • steplist   — [{step_id, instructions, executable_code, tool_bindings}];
  │         • rust_directives — cdylib load directives for the Executioner
  │                         (applied by the C.3 DynamicToolLoader; CARRIED, not
  │                         executed by the Orchestrator);
  │         • variables  — {{vars.NAME}} slots bound from user_input.
  │     ↓
  │   Monty iterates steplist: consults skills for exact tool usage, then runs
  │   each step's executable_code via host.run_program (Monty 0.0.16 has no
  │   exec/eval/compile — a host callable is the only way to run a dynamic code
  │   string). Tools are invoked as host.<tool>(...) and executed by Rust.
  │     ├── Tier 0 (no LLM): direct reply (Sempai-Kohai may finalize the surface).
  │     └── Tier 1 (LLM-guided): Orchestrator assembles the prompt →
  │         InterceptorStage (Sempai review) → ModelStage.
  │
  └── NoMatch → the Orchestrator builds an LLM prompt (head + body)
        ├── body = chat message + history + selected memories/components
        ├── head = the "base prompt" (pre-compiled, in the vLLM KV-cache).
        │   Composition inserts a single "base-prompt" line; the Sempai-Kohai
        │   system substitutes the real pre-compiled content at the very end of
        │   prompt creation, just before tokenizing.
        └── If the base prompt is not yet pre-compiled, Sempai-Kohai substitutes
            a short minimal-context part.
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

**Pending (v3):**
- **C.4.5.18–C.4.5.19**: this doc pass (bring 01–17 to the shipped Orchestrator/Executioner +
  composition=IBS architecture) + both-configs green + mark C.4.5 done.
- **C.5/C.6**: the basic-mode Orchestrator script + the driver that activates the engine Monty VM
  host-call path (wires `PgCompositionPort` into `ThreadManager` + applies `rust_directives` via
  the `DynamicToolLoader`). Until C.5/C.6 the engine host-call path is constructed + unit-tested
  but inert in production; the live Tier-0/Tier-1 path runs through the turns
  `PgOrchestratorLookup` bridge.
- **C.7**: final Model-A cleanup. **Phase A (reshaped H.12.6)** after C.

(Migration numbers per the plan's Round-2 correction; see `saved_plan_to_v3.md` header table.)

## 7. LLM-relevant summary

BrassClaw Reborn is a ~70-crate Rust agent system targeting 7B–14B LLMs in 8k context. It is
layered **Products / Loops / Kernel / Infrastructure** and is an **Orchestrator / Executioner**
system: a Python Orchestrator (Monty) is the sole sequencing authority; a Rust Executioner runs
precompiled tools on `host.<tool>(...)` calls with no sequencing of its own. A turn flows: chat →
Orchestrator → `host.resolve_intent` → (match → `host.compose_orchestrator` composes the recipe
into `{skills, steplist, rust_directives, variables, assembled_program, tier}`; Monty iterates
`steplist`, consults `skills`, runs each step's `executable_code` via `host.run_program`; or
no-match → the Orchestrator builds an LLM prompt with a pre-compiled `base-prompt` prefix + a
per-turn body). The IBS is the composition system. A multi-kind skills system
(Classic/Tool/Orchestrator/ExtensionCatalogue) is carried as a first-class array the Orchestrator
consults while stepping. A Sempai-Kohai prompt optimizer runs idle and pre-send; vLLM
prefix-cached base prompts hold the head. Postgres is mandatory; in-memory is unit-test only. The
four validation queues (Q1 auto / Q2 manual / Q3 revision / Q4 rejection) gate new components,
scored by Wilson, with a graduation trigger that upserts Monty-VM cursor rows.
