# Subplan — Step C.6 Kohai LLM-path + component re-architecture

> Parent: `./subplan_problem_stepC6_production_driver_switch_of_saved_plan_to_v3.md`
> slice 4d. **Precedes slice 4d-3** (composition construction of
> `PersistentMontyDriver`), because the re-architecture retires the
> `Arc<dyn LlmBackend>` dep the 4c driver holds + reworks the orchestrator's
> host-call surface. Sequenced after slice 4c (SHIPPED `f2083d0d`).

## Why this sub-step exists

Slice 4d grounding discovered that `PersistentMontyDriver` (4c) holds a raw
`Arc<dyn LlmBackend>` for `MontySession::drive_to_yield`
(`orchestrator.rs:1212` `deps.llm.complete(...)`), but composition's production
runtime has **no real `LlmBackend`** — only `TierZeroLlmGuard` (always-errors)
+ the real `HostManagedModelGateway` (a different trait). The α/β/γ fork posed
for this was **overthinking**: the user pointed out the **Kohai** was missed.

**The Kohai already exists (shipped C.5):** `host.kohai_complete` →
`KohaiPort::complete(prompt={chat_history, user_query, prefix_placeholder}, ctx,
llm)` (`crates/brassclaw_engine/src/executor/kohai_port.rs:103`) drives the
forensic-packet lifecycle + the provider-prefix swap + the provider call, returns
answer+usage. `PgKohaiPort` (`crates/brassclaw_reborn_composition/src/pg_kohai_port.rs`)
implements the routing path (Sempai audit gated on a Sempai sink being wired).

**The corrected Kohai flow (user, 2026-09-04):**
1. Orchestrator composes the prompt from the Recipe **and adds the
   prefix-placeholder**; sends it to Kohai (`host.kohai_complete`).
2. Kohai **saves** the prompt (for later component/intent creation).
3. Kohai replaces the prefix, branching on a connected Sempai:
   - **Sempai connected:** placeholder filled with a *precompiled* prefix for the
     Sempai optimization pass; Sempai optimizes the volatile prompt and returns it
     **without** the prefix; Kohai saves the optimized prompt beside the original;
     Kohai adds the **real** Prefix (bound to that placeholder) and sends to the
     Provider LLM.
   - **No Sempai:** Kohai saves the prompt, adds the real Prefix, sends to the
     Provider LLM.
4. Kohai receives the Answer, **saves it beside its prompt**, returns the Answer
   to the Orchestrator.

`handle_llm_complete`/`__llm_complete__` (the raw `LlmBackend` arm,
`orchestrator.rs:731`/`1133`) **retires as a Rust host tool** — the LLM HTTP
call is performed by Kohai (wrapping the provider gateway).

## Locked decisions (user, 2026-09-04)

- **One sub-step** — do all points together, then wire 4d (no split/deferral).
- **post_reply = stay a Rust host tool** (only Rust writes to the chat window).
- **`LlmBackend` retires from the host path** — `KohaiPort::complete` switches
  its provider-call param from `&'a dyn LlmBackend` to the
  `HostManagedModelGateway` (composition's real provider); no adapter.
- Directives: (1) LLM via `host.kohai_complete`; retire `__llm_complete__`/
  `LlmBackend` host dep. (2) `fetch_component`/`resolve_component_by_name` stay
  as tools. (3) `assemble_prior_knowledge` → seeded Recipe (no-Prefix fallback);
  drop `retrieve_docs`/`get_reduction_rules`. (4) `compose_orchestrator` rewrite
  (Rust part reduced) + Recipe/Component structure update. (5) `post_reply` stays
  a tool. (6) `memory_write` → seeded Recipe (SQL-saving tool, same store as
  Kohai prompt saves).

## Grounding (verified live source, 2026-09-04)

- `KohaiPort` trait `kohai_port.rs:103`; `complete<'a>(&'a self, prompt: Value,
  ctx: KohaiCallCtx, llm: &'a dyn LlmBackend) -> Future<KohaiAnswer>`. Doc says
  the impl drives forensic-packet lifecycle + provider-prefix swap +
  `LlmBackend::complete`; `LlmBackend` is passed IN from the engine handler.
- `PgKohaiPort` `pg_kohai_port.rs`: routing impl; `complete_with_stores(prompt,
  ctx, llm: &dyn LlmBackend)` (:253/258); parses prompt dict → resolves provider
  prefix via `get_system_bundle` → builds messages → `LlmBackend::complete
  (force_text)`. Sempai audit gated on a Sempai sink (future).
- `__llm_complete__(messages, actions, config)` arm `orchestrator.rs:731` →
  `handle_llm_complete` (:1133); `deps.llm.complete` at :1212 (the
  `host.non_match_llm_answer` arm's LLM call).
- `compose_orchestrator` arm `orchestrator.rs:888` → `handle_compose_orchestrator`
  (:1843) → `ComposedProgram { skills, steplist, rust_directives, variables, ... }`
  (`crate::memory::composition`: `ComposedProgram`/`ComposedStep`/`RustDirective`/
  `SkillRef`). Comment :885: "rust_directives is a C.5/C.6 concern (deferred)."
- `Recipe`/`RecipeStep`/`RecipeVariant` in `crates/brassclaw_engine/src/types/recipe.rs`
  (:130/151/176).
- Arms to drop: `__retrieve_docs__` (:761 → `handle_retrieve_docs` :1546),
  `__get_reduction_rules__` (:779 → `handle_get_reduction_rules` :2821).
- `post_reply` arm :838 → `handle_post_reply` :1452 (STAYS).
- `assemble_prior_knowledge_with_hint` (:2189) — pub library fn (H8.2 replacement
  for the dormant `handle_assemble_prior_knowledge`); the logic to convert into a
  no-Prefix-fallback Recipe.
- Composition gateway holders: `sempai_gateway: Option<Arc<dyn
  HostManagedModelGateway>>` (`runtime.rs:3301`) = the Sempai reviewing LLM;
  `gateway_for_runtime`/`model_gateway_override` (:4728/5544/`runtime_input.rs:303`)
  = the Kohai working provider. `PgKohaiPort` can hold the Kohai gateway Arc
  directly.

## Slices (one-by-one; both configs clippy-clean + commit + push each)

- **K1+K2 — DONE (`314645bd`/`bd5d92e9`).** KohaiPort→gateway + route LLM via
  `host.kohai_complete` + retire `__llm_complete__`.** (a) `KohaiPort::complete` drops the `llm` param;
  `PgKohaiPort` holds `Arc<dyn HostManagedModelGateway>` (Kohai gateway) +
  `Option<Arc<dyn HostManagedModelGateway>>` (Sempai gateway) + the stores; the
  provider call uses the gateway (`stream_model`). (b) The engine
  `host.kohai_complete` handler stops passing `llm`. (c) The orchestrator's
  LLM-calling arm (`host.non_match_llm_answer`) calls `host.kohai_complete`
  (compose prompt + placeholder → Kohai → answer) instead of `deps.llm.complete`.
  (d) Delete `__llm_complete__`/`handle_llm_complete` + the `Arc<dyn LlmBackend>`
  dep from `drive_to_yield`/`execute_orchestrator`/`MontySession`. (e) Update
  `basic_mode.py` + tests.
- **K3 — DONE (`a3ecad97`).** Drop `__retrieve_docs__`/`__get_reduction_rules__`.
  Deleted both arms + `handle_retrieve_docs`/`handle_get_reduction_rules` +
  `load_reduction_rules` + 4 tests + `make_rule_doc`/`REDUCTION_RULES_TEST_LOCK`;
  renamed `drive_to_yield`'s `retrieval` param → `_retrieval`. Kept
  `invalidate_reduction_rules_cache` + `REDUCTION_RULE_CACHE` (composition
  `reduction_rules_store`/webui still call the flush API). `basic_mode.py` had no
  calls; the lone `__get_reduction_rules__()` call left in `default.py` is in
  orphaned legacy code retired wholesale in C.7.
- **K6 — DONE.** `handle_compose_orchestrator` rewritten to a thin delegate:
  `port.compose(&scope, component_id, &step_link, &user_input)` → returns
  `{ok, program}` (Rust part significantly reduced; `ComponentPort::compose` owns
  the assembly). `ComposedProgram`/`ComposedStep`/`RustDirective`/`SkillRef`
  (`memory/composition.rs`) + `Recipe`/`RecipeStep`/`RecipeVariant`
  (`types/recipe.rs`) carry the new architecture shape.
- **K4 — DONE.** The `__assemble_prior_knowledge__` orchestrator arm is retired
  (Phase H8.4) and replaced by the `pub` `assemble_prior_knowledge_with_hint`
  library call (§3.13/§3.14). The no-Prefix fallback is now a **seeded Recipe
  component**: `seed_host_group` (Pass 6 in `builtin_bootstrap.rs`) seeds
  `pc-host-fallback-prior-knowledge` (class 22, pure-logic formatter — minimal
  system-context preamble + the Orchestrator MCP Server catalogue, Phase V) +
  `host-assemble-prior-knowledge` (class 21, single-orchestrator-step,
  Tier-0-eligible via `mark_recipe_tier0`) and explicitly graduates the recipe to
  `validation_status='validated'` via `RecipeValidationStatusUpdate` (the first
  recipe-graduation call in the bootstrap — `seed_recipe`/`mark_recipe_tier0` do
  NOT graduate, and `resolve_component_by_name`'s SEC-01 filter requires
  `validated`). `basic_mode.py` `_non_match_answer` composes+runs it
  (step_link `"0:1-0:E"` — the seeded variant key, NOT `"default"` which
  `match_variant` exact-matches and would never hit) and injects the bundle as
  `prior_knowledge` into the Kohai-mediated prompt. The integration-test counts
  in `tests/builtin_bootstrap_seed.rs` updated (83→84 PythonCode, 110→111
  Recipes, 378→380 total). Both configs clippy-clean; 706 lib tests pass.
  **DEFERRED to a follow-up slice:** `host-non-match-llm-answer` +
  `host-save-history` — both are 2-orchestrator-step recipes (assemble+call /
  format+write) that the isolated-`run_program` model breaks
  (`handle_run_program` uses a fresh `ThreadExecutionContext` + `persisted_state
  = {}` per call, so step 1 cannot see step 0's data); they need the
  single-step-collapse design resolved first. The full `builtin-host` catalogue
  (host.* Tool/ToolSkill/PythonCode/leaf-skill wrappers) is also deferred —
  host.* verbs are hardcoded Monty intrinsics today, not seeded Tool rows.
- **K5 — DONE.** `memory_write` orchestrator arm retired (no `handle_memory_write`/
  `__memory_write__` in `orchestrator.rs`); seeded as Recipes in
  `builtin_bootstrap.rs` Pass 3 — `recipe_memory_write` (+ `_log`/`_main`/`_patch`
  variants) driving `ts-memory-write` + `pc_exec_memory_write`/`pc_exec_memory_patch`
  via `host.memory_write(...)`.
- **K7 — DONE (verify-only).** `post_reply` remains an active orchestrator arm
  (`handle_post_reply` in `orchestrator.rs`); no change required.
- **K8 — DONE.** Both configs clippy-clean (skills-db workspace green after the
  `retrieval_source.rs` `doc_type_to_class_code` `#[allow(dead_code)]` unblock in
  commit `24ea6a5c`); Phase L integration test `tests/builtin_bootstrap_seed.rs`
  passes; re-architecture sub-step complete.

## Out of scope (explicit)

- The future Sempai idle-time self-optimization sweep.
- The future MCP bridge.
- C.7 deletions (`execute_orchestrator` / `default.py` / `ExecutionLoop` /
  `ThreadManager` / `brassclaw_engine::runtime`) — separate step.
- Local e2e (C6-4=C: CI/Docker only).
