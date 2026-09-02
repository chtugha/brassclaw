# Subplan — Problem Step C of `saved_plan_to_v3.md` (REFRAMED — Option 2)

> **SUPERSEDES the "Model A retirement" framing.** The user correction
> (2026-09-02): the H.12 production path (`execute_tier_zero_channel`'s Rust
> `for step in &steps { execute_code(step.body) }`) is "Rust running the show,
> using Python only to execute each step separately" — **architecturally wrong**.
> Neither current model fits the philosophy:
> - **Model A** (dead): Python `run_loop` outer loop (**good**) but *LLM-driven*
>   (**bad** — the LLM drives).
> - **H.12 Model B/C** (shipped): Rust agent-loop + per-step Python sandbox
>   (**bad** — Rust drives).
> - **Target (Option 2, LOCKED):** Monty (Python) is THE one long-persisting
>   main process for the whole turn, **recipe/intent-driven** (not LLM-driven);
>   the Rust agent-loop stage pipeline is **retired as the driver entirely**;
>   Rust = **pure host** (capabilities/stores/retrieval/LLM-as-helper/sandbox).
>   Rust must not do step sequencing.
>
> Sequenced **after B** (DONE) and **before A** (reshaped H.12.6). This reframe
> reworks the shipped **H.10–H.12** production path, not just dead code.

## Goal

Make **Monty (Python) the production main process**: one Monty VM session runs
the whole turn — input → intent (Rust host) → match: fetch+run recipe
orchestrator → no-match: LLM-prompt-assembly + LLM helper → answer → history →
kohai/sempai. Retire the Rust agent-loop stage pipeline
(`RecipeStage`/`TierZeroExecutionStage`/`PromptStage`/`ModelStage`/
`CapabilityStage`/`AssistantReplyStage` in `canonical.rs`) **as the driver**.
Rust becomes the host: intent, recipe fetch+assembly, capability dispatch,
stores, retrieval, LLM backend, sandbox, chat-post, history-save.

## Grounding (verified, live source)

### Current production driver (to retire)

- `TurnRunnerWorker` (`brassclaw_reborn/turn_runner`) → `AgentLoopDriverRunRequest`
  (`brassclaw_turns::run_profile::driver`) → `loop_driver_host`
  (`brassclaw_reborn`) → `DefaultExecutorPipeline::execute`
  (`brassclaw_agent_loop/src/executor/canonical.rs:20`) — the stage pipeline.
- `RecipeStage` branches `TierZero` (→ `TierZeroExecutionStage` →
  `AssistantReplyStage`) or `Continue` (→ `PromptStage` → `ModelStage` →
  `CapabilityStage` → `AssistantReplyStage`).
- The H.12 bridge (`orchestrator_lookup_impl.rs`) wires `orchestrator_lookup`
  → `TierZeroOrchestrator` → `execute_tier_zero_channel` (Rust per-step loop) /
  `assemble_prior_knowledge_with_hint` (pure Rust). **This is the Rust-driving
  pattern being retired.**

### Monty host-function surface (orchestrator.rs `__host_call__` dispatch, :641-790)

**Already present:** `__llm_complete__`, `__execute_action__` /
`__execute_actions_parallel__`, `__execute_code_step__`, `__retrieve_docs__`,
`__fetch_component__` / `__resolve_component_by_name__` / `__validate_component__`,
`__list_skills__` / `__record_skill_usage__`, `__get_actions__`, `__emit_event__`
/ `__save_checkpoint__` / `__transition_to__` / `__check_signals__`,
`__check_budget__` / `__log_budget_warning__` / `__get_reduction_rules__`,
`__regex_match__`, (and `__assemble_prior_knowledge__`).

**Missing for the basic-mode orchestrator (to add):** intent resolution
(`__resolve_intent__` → `intent_system.rs::resolve_intent`), recipe fetch +
orchestrator-assembly (`__fetch_recipe__` / `__compose_orchestrator__` →
composition recipe store + the splitter that separates rust vs orchestrator
parts), chat-post (`__post_reply__`), history-save (`__save_history__`).

### Keep (Rust host substrate)

- Monty VM `scripting::execute_code` + the `__host_call__` dispatch table.
- `Store` / `Thread` / `PgThreadEngineStore`; `EffectExecutor` / `LlmBackend`
  traits (H.12 production impls: `TierZeroEffectExecutorBuilder` /
  `TierZeroLlmGuard`); capability dispatch; retrieval sources; intent system;
  recipe/component stores + validators; the formatters
  (`format_orchestrator_content` / `assemble_pkr_from_items` /
  `parse_orchestrator_channel_steps`).
- `types/recipe.rs` (kept + extended by B). First-party Rust capability handlers.

### Retire / rework

- The Rust agent-loop stage pipeline **as the driver** (`canonical.rs::execute` +
  the stages). Stage *logic* (prompt assembly, capability dispatch) is reused as
  **host fns** Monty calls — not as a Rust-driven pipeline.
- `execute_tier_zero_channel`'s Rust per-step loop → Monty runs the assembled
  recipe orchestrator script as **one continuous Python program** that sequences
  its own steps.
- `assemble_prior_knowledge_with_hint` (pure Rust) → Monty assembles prior
  knowledge by calling Rust retrieval/fetch host fns (or kept as a host fn
  `__assemble_prior_knowledge__` Monty invokes).
- `execute_orchestrator` / `ExecutionLoop` / `ThreadManager` /
  `brassclaw_engine::runtime` — dead test-only code; delete.
- The H.12 `LoopOrchestratorPort` / `orchestrator_lookup` bridge — reworked
  (Monty calls Rust host fns directly; no agent-loop bridge needed).

## Locked sub-decisions (confirmed 2026-09-02)

- **D-C1 (Monty session granularity) = CROSS-TURN PERSISTENT.** One Monty VM
  session lives across the whole conversation (the literal "one long-persisting
  process"). Adds VM state + lifetime/liveness management; the basic-mode
  orchestrator must be **resumable across turns** (idle between turns, woken on
  new input) rather than start→exit per turn. State checkpointing already exists
  (`__save_checkpoint__`/`__transition_to__`); cross-turn persistence layers a
  VM-session handle keyed by conversation on top.
- **D-C2 (basic-mode orchestrator source) = RESTRUCTURE `default.py`.** Keep the
  host-call idioms (the Python→Rust intrinsic call sites); drop the LLM-driven
  `run_loop` outer loop + the dead Model-A step-0; add an intent/recipe-driven
  structure. `default.py` becomes the built-in basic-mode orchestrator.
- **D-C2-addition (SUPERSEDED — HostSkill interface dropped):** the
  `__host_call__` surface is NOT wrapped in a uniform HostSkill trait. Instead
  tools are **first-class callables in the Monty namespace** — recipe PythonCode
  calls `host.<name>(…)` directly; **no `__execute_action__` string-intrinsic,
  no `__execute_code_step__`** (Model-A relics); `__execute_actions_parallel__`
  → a Python helper. **The Monty namespace IS the tool registry** (bind=load,
  call=execute, unbind=unload at task end). Host capabilities register like any
  first-party tool; the 23-arm `__host_call__` `match` retires into the registry.
  Bare Rust helpers (`resolve_intent`, the fetch/split formatters) are dissected
  into registered `host.resolve_intent` / `host.compose_orchestrator` tools. The
  future MCP bridge hits the same registry — no Python intrinsic needed.
- **Two Tool Systems (LOCKED):** built-in Tools+ToolSkills precompiled into the
  binary; kohai/sempai-minted Tools+ToolSkills compiled as **separate cdylib
  crates**, **loaded dynamically on demand by a recipe** (`dlopen`), bound into
  the same namespace, **unloaded at main-process task end**.
- **Runtime security (LOCKED — mode-driven, no per-call babysitter):**
  **Matching-Mode (Q2+ validated component): ALL security OFF** (execute as
  intended, incl. outbound HTTP; no wrapper, no sensitive-tool self-scoping);
  **Non-Matching-Mode (LLM involved): wrapper ON**; **Q1 never accessible**
  (Queue-System + SEC-01 → only Q2+ returned); **WebUI global security panel**
  (per-layer operator toggles); LLM-path policy = **bind-time namespace
  filtering**; Matching-Mode bypasses it.

## Proposed steps (after sub-decisions; one-by-one; commit+push each)

Mirrors the C.1–C.7 block in `./saved_plan_to_v3.md` (Step C). Summary:

- **C.1 — Tool registry + first-class callables.** Replace the `__host_call__`
  23-arm `match` (`orchestrator.rs:641-801`) with a tool registry; bind host
  capabilities as first-class Monty-namespace callables (`host.<name>(…)`
  directly from recipe PythonCode). Retire `__execute_action__` +
  `__execute_code_step__`; reduce `__execute_actions_parallel__` to a Python
  helper. Dissect `intent_system::resolve_intent` + the fetch/split formatters
  into registered `host.resolve_intent` / `host.compose_orchestrator` tools.
- **C.2 — Reclassify the 23 host calls (per `builtin_stuff_v3.md` Step 27).**
  Register the **8 net-new `host.*` Tools** (resolve_intent, compose_orchestrator
  [rewrite], post_reply [A1: Tool], fetch_component, resolve_component_by_name,
  validate_component, check_signals, kohai_complete) + reuse builtin.memory_write /
  first_party_tools/http / builtin.skill_list / pc-regex-match;
  non-match-llm-answer (Kohai-mediated) / save-history / assemble-prior-knowledge
  (fallback, no retrieval verbs) → Orchestrator Recipes over existing tools (no
  new Rust); **DROP** retrieve_docs + get_reduction_rules; **RETIRE**
  host.llm_complete (+ `handle_llm_complete`/`LlmBackend`) + 7 stage-machinery
  verbs (Q-D) + the per-call `handle_execute_action` wrapper (security is
  mode-driven).
- **C.3 — Two Tool Systems: cdylib dynamic loading.** Built-in tools stay
  precompiled; add the cdylib load/unload path (`dlopen`) for
  kohai/sempai-minted Tools+ToolSkills — bound into the same namespace on demand
  by a recipe, unloaded at main-process task end.
- **C.4 — Mode-driven security + WebUI panel.** Matching-Mode all-off (Q2+
  validated components execute as intended); Non-Matching-Mode wrapper-on;
  bind-time namespace filtering for the LLM path; add the global
  security-settings WebUI panel (per-layer toggles).
- **C.5 — Basic-mode orchestrator script.** Built-in Phase-1 harness (receive
  input → `host.resolve_intent` → dispatch). Match → `host.compose_orchestrator`
  + run assembled recipe program (Tier-0 calls / Tier-1 fallback prior-knowledge
  + `host.kohai_complete` Kohai-mediated LLM call). No-match → prompt-assembly
  recipe + `host.kohai_complete` + answer. Answer → `host.post_reply` tool →
  `host-save-history` recipe → kohai/sempai.
- **C.6 — Production driver switch.** Replace `TurnRunnerWorker` → agent-loop
  stages with `TurnRunnerWorker` → one cross-turn persistent Monty session
  (D-C1) running the basic-mode orchestrator. Retire `canonical.rs` stage
  pipeline as the driver; reuse stage logic as host fns.
- **C.7 — Retire dead Model-A code + verify both configs green.** Delete
  `execute_orchestrator`/`ExecutionLoop`/`ThreadManager`/
  `brassclaw_engine::runtime` + Model-A engine tests; rework the H.12
  `orchestrator_lookup` bridge (Monty calls host fns directly). Both configs
  (default + `--features skills-db`); `CARGO_TARGET_DIR=/Users/ollama/brassclaw-
  target` on every build; `df -h` first — `cargo clean` (scoped `-p` ok) if
  Avail<15GB or >90%. Mark C done; commit + push; proceed to **A**.

## Out of scope (explicit)

- The Monty VM (`scripting.rs`), the host-call dispatch, `Store`/`Thread`,
  `EffectExecutor`/`LlmBackend` traits, intent system, recipe/component stores,
  formatters, `types/recipe.rs`, first-party Rust capability handlers — all kept.
- The future MCP bridge (LLM tool calls routed through Monty) — future work.
