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
  - Loops: `crates/brassclaw_agent_loop/` (stage pipeline — skeleton today),
    `crates/brassclaw_engine/` (engine `ExecutionLoop` + Monty VM — current production driver).
  - Kernel: `crates/brassclaw_safety/`, `crates/brassclaw_secrets/`, `crates/brassclaw_trust/`,
    `crates/brassclaw_authorization/`, `crates/brassclaw_process_sandbox/`, `crates/brassclaw_capabilities/`.
  - Composition: `crates/brassclaw_reborn_composition/` (`factory.rs`, `runtime.rs`).
  - Persistence: `crates/brassclaw_pg/` (pool + migrations `V001`–`V049`),
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

## 4. Behavior / flow — the v3 target message flow

This is the flow the v3 plan builds toward (the user's mental model, verified in
`MESSAGE_FLOW_AND_PLAN_AUDIT.md` §3):

```
User enters a message in the chat
  │  (WebUI v2 / gateway → bearer-token auth → turn submission)
  ↓
Orchestrator receives the message
  ↓
Intent system: resolve_intent(pool, scope, user_text)
  ├── Match → returns a recipe (component id + class code + step_link)
  │     ↓
  │   Orchestrator performs/orchestrates the recipe's steps one by one.
  │   A recipe carries, per step, exactly what is needed: skills, tools, Python
  │   code, LLM prompt contents, and instructions for the orchestrator and the
  │   Rust executor (what to preload, how to execute, whether an LLM call is
  │   needed and how its prompt is built).
  │     ├── Tier 0 (no LLM): rust_items applied; orchestrator_items stashed →
  │     │   direct reply (Sempai-Kohai may still finalize the prompt surface).
  │     └── Tier 1 (LLM-guided): orchestrator_items injected as context →
  │         PromptStage → InterceptorStage (Sempai review) → ModelStage.
  │
  └── NoMatch → an LLM prompt is built (head + body)
        ├── body = chat message + history + selected memories/components
        │   (formatted for the LLM)
        ├── head = the "base prompt" (pre-compiled, sitting in the vLLM KV-cache).
        │   During composition only a single line "base-prompt" is inserted;
        │   the Sempai-Kohai system replaces that placeholder with the real
        │   pre-compiled base-prompt content at the very end of prompt creation,
        │   just before tokenizing.
        └── If the base prompt is not yet pre-compiled, Sempai-Kohai substitutes a
            short minimal-context part (the LLM computes ~200 tokens/s, so a
            full un-precompiled base prompt is not feasible per turn).
  ↓
Result / answer / clarifying question returned to the chat
```

The current codebase differs from this target in three big ways (all v3 work):
1. The intent system is **not in the production loop** today (`RecipeStage` is a stub; `PostgresSource`
   is implemented but not wired; production uses `RamSource` keyword retrieval).
2. The recipe/IBS/two-channel delivery does not exist yet.
3. The base-prompt KV-cache store does not exist yet.

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
| Orchestrator (default.py) | `13-orchestrator-default-py.md` | The Python step-0 prior-knowledge assembly + tool execution |
| Validation Queue | `14-validation-queue.md` | Q1/Q2 graduation + Wilson scoring for new components |
| Component Catalog | `15-component-catalog.md` | Unified tables (class 12–20) + new 22/23 |
| Kernel / Composition | `16-kernel-composition.md` | Authority + wiring |
| WebUI v2 / Prefix Tab | `17-webui-prefix-tab.md` | Authoring + prefix management UI |

## 6. Status — today vs. v3

**Today (pre-v3, verified):**
- Production turn driver is the engine `ExecutionLoop::run` (`crates/brassclaw_engine/src/executor/loop_engine.rs`),
  which calls `execute_orchestrator` (Python `default.py`) directly with **no stage pipeline**.
- The agent-loop `DefaultExecutorPipeline` (`crates/brassclaw_agent_loop/src/executor/canonical.rs`)
  is a **skeleton** — no product surface drives it. This is the "DRIVER-GAP" (see `12-agent-loop.md`).
- Retrieval is `RamSource` (keyword-over-postgres via `PgMemoryDocStore`); `PostgresSource` exists
  but is not wired (`manager.rs:383`).
- No IBS, no `step_descriptions`, no `SplitResult`, no `reborn_basic_prompt_store`, no
  `reborn_validation_queue`. RecipeStage always returns `RecipeStep::Continue` (Tier 2).
- Two goals audit: Goal 1 (no installation profiles) fully done; Goal 2 (no postgres-less design)
  production path done (fail-hard); the residual postgres-less **test build** is Step 13 (deferred,
  needs e2e execution capability).

**v3 plan adds (phases A–N, `saved_plan_to_v3.md`):**
- Phase A (V050): IBS (`instruction_builder.rs`), `types/ibs.rs`, `step_descriptions` on recipes.
- Phase A.5 (V051): `reborn_validation_queue` table (hoisted so the queue exists from day one).
- Phase B (V052): `reborn_python_code` (class 22). Phase C (V053): `reborn_extension_catalogues` (class 23).
- Phase D (V054): `step_link` on `reborn_intent_inputs`. Phase E.0: wire `PostgresSource` in composition.
- Phase E: `SplitResult`/`ActionShortCircuit` variants. Phase F: scope-bug fix + fallback preservation.
- Phase G: remove dead `__retrieve_docs__` shim; wire disambiguation UX. Phase H / H.0: `last_user_text`,
  `TierZeroExecutionStage`, host ports (`LoopRetrievalPort`, `LoopOrchestratorPort`, `LoopContextPort`).
- Phase J (V055): intent examples + dependency registry. Phase K.1 (V056): `reborn_basic_prompt_store`.
- Phase L.1 (V057): `capability_id` on tools + builtin bootstrap. Phase M.1 (V058): template matching.
- Phase N.1 (V059): populate + DROP legacy queue columns + boot-integrity UNION ALL.

(Migration numbers per the plan's Round-2 correction; see `saved_plan_to_v3.md` header table.)

## 7. LLM-relevant summary

BrassClaw Reborn is a ~70-crate Rust agent system targeting 7B–14B LLMs in 8k context. It is
layered **Products / Loops / Kernel / Infrastructure**. A turn flows: chat → orchestrator →
intent system → (match → recipe steps, or no-match → LLM prompt with a pre-compiled
`base-prompt` prefix + per-turn body). The v3 plan adds an intent-driven recipe system with an
instruction builder that splits work into a Rust-executor channel and an orchestrator channel,
a multi-kind skills system (Classic/Tool/Orchestrator/ExtensionCatalogue), PythonCode components,
a Sempai-Kohai prompt optimizer that runs idle and pre-send, and vLLM prefix-cached base prompts.
Production today uses the engine `ExecutionLoop` + Python `default.py` with `RamSource` keyword
retrieval; the agent-loop stage pipeline and intent-driven `PostgresSource` are dormant and are
activated by v3 Phases E.0/H. Postgres is mandatory; in-memory is unit-test only. The four
validation queues (Q1 auto / Q2 manual / Q3 revision / Q4 rejection) gate new components, scored
by Wilson, with a graduation trigger that upserts Monty-VM cursor rows.
