# Subplan — Step C.5: Basic-mode orchestrator script

> Parent: `./saved_plan_to_v3.md` Step C (C.1–C.7). Sequenced after **C.4.5
> (COMPLETE)** and before **C.6 (production driver switch)**. Per saved_plan
> line 5948-5965: "C.5 — Basic-mode orchestrator script. The built-in Phase-1
> harness (receive input → `host.resolve_intent` → dispatch). Match →
> `host.compose_orchestrator` + run the assembled recipe program (Tier-0 calls /
> Tier-1 fallback prior-knowledge + `host.kohai_complete` Kohai-mediated LLM
> call). No-match → prompt-assembly recipe + `host.kohai_complete` + answer.
> Answer → `host.post_reply` tool → `host-save-history` recipe → kohai/sempai."

## Grounding (verified live source, 2026-09-03)

- **Host-callable surface** (`executor/orchestrator.rs` `execute_orchestrator`
  dispatch :634-862). WIRED: `FINAL`, `__llm_complete__`, `__check_signals__`,
  `__emit_event__`, `__save_checkpoint__`, `__transition_to__`, `__retrieve_docs__`,
  `__check_budget__`, `__log_budget_warning__`, `__get_reduction_rules__`,
  `__get_actions__`, `__list_skills__`, `__record_skill_usage__`, `__regex_match__`,
  `__validate_component__`, `__fetch_component__`, `__resolve_component_by_name__`;
  C.1 `host.*`: `resolve_intent`(:766), `post_reply`(:776), `fetch_component`,
  `resolve_component_by_name`, `validate_component`, `check_signals`,
  `regex_match`, `skill_list`; C.4.5.17 `run_program`(:822), `compose_orchestrator`
  (:845); C.3 dynamic-tool fallthrough (:856).
- **NOT wired (the C.5 gap):** `host.kohai_complete` (only a comment at :764).
  `host-save-history` / `host-non-match-llm-answer` are seeded **Recipes (cl.21)**,
  not host fns — the script reaches them via `compose_orchestrator`+`run_program`.
  `__assemble_prior_knowledge__` is RETIRED (H8.4) → pub fn
  `assemble_prior_knowledge_with_hint` (:1974); per fork C, prior-knowledge is
  embedded in `compose_orchestrator`'s returned `ComposedProgram` (via `tier`).
- **Driver reality:** `execute_orchestrator` (:520) is Model-A / DORMANT — sole
  caller `ExecutionLoop::run` (loop_engine.rs:514); `ThreadManager::with_*` have
  no external callers. The `host.*` arms + `host` namespace are dormant here and
  **move to the C.6 driver fn**; C.7 deletes `execute_orchestrator`. C.5 does NOT
  touch the driver — it authors the script + wires the one missing handler so the
  script is complete; the script is exercised end-to-end in **C.6** (fork D).
- **Orchestrator script = `orchestrator:main` (cl.10, protected, self-modifiable).**
  Compiled-in v0 = `DEFAULT_ORCHESTRATOR = include_str!("../../orchestrator/default.py")`
  (:71); `ORCHESTRATOR_TITLE = "orchestrator:main"` (:74). Loaded by
  `load_orchestrator_from_docs(&system_docs, allow_self_modify)` (:351) — DB
  override via shared doc when `ORCHESTRATOR_SELF_MODIFY=true`, else compiled-in v0.
- **Current `default.py` (1637 ln) = OLD Model-A** (retired
  `__execute_action__`/`__execute_actions_parallel__`/`__assemble_prior_knowledge__`).
  Sibling `segment_reduction.py`. C.5 authors the NEW v3 script; C.7 deletes
  default.py.
- **Injected orchestrator vars** (`build_orchestrator_inputs` :3014): `context`
  (list of prior msgs `{role,content,action_name,action_call_id,action_calls}`),
  `goal` (str), `actions` (empty — loaded via `__get_actions__`), `state`
  (persisted_state), `config` (dict: max_iterations, budgets, prompt_budget_tokens,
  step_count, …). The turn's user input = the last user message in `context`.
- **`host.kohai_complete` contract** (seed `seed_host_kohai_complete` :1095):
  `host.kohai_complete(prompt={chat_history, user_query, prefix_placeholder})` →
  wraps the existing `brassclaw_interceptor` ingress (Kohai: save prompt → optional
  Sempai optimize → swap placeholder for provider prefix → call provider LLM via
  `first_party_tools/http` → save answer → return answer). **"Wiring only, no new
  logic."** Tool `effect_type=write`; `capability_id="host.kohai_complete"`.
- **Seeded recipe names** (C.2): `host-save-history` (27.4), `host-assemble-prior-
  knowledge` (27.10.1), `host-non-match-llm-answer` (27.10.2). The script resolves
  them by name via `host.resolve_component_by_name(name, 21)` → UUID, then
  `host.compose_orchestrator(component_id=uuid, step_link=<variant>, user_input=…)`.

## Locked forks (user, 2026-09-03)

- **C.5-A = new `orchestrator/basic_mode.py`**, compiled-in via `include_str!` as
  the new `DEFAULT_ORCHESTRATOR` (replaces the `default.py` const); `default.py`
  deleted in **C.7**. DB override (orchestrator:main shared doc) still works for
  self-modify.
- **C.5-B = wire `host.kohai_complete` Rust handler in C.5** (script + its one
  missing dependency land complete together).
- **C.5-C = `compose_orchestrator`'s returned `ComposedProgram` embeds
  prior-knowledge (via `tier`); no-match is a separate `compose_orchestrator`
  call — no new host fns.** Script only calls `resolve_intent`,
  `compose_orchestrator`, `run_program`, `kohai_complete`, `post_reply`
  (+ `resolve_component_by_name` for recipe UUIDs).
- **C.5-D = combine C.5 verification with C.6.** C.5 lands script + handler
  (with narrow handler unit tests); the script's end-to-end "drives a turn"
  verification is done in **C.6** when the driver runs it. Keep going into C.6
  without stopping.

## Realization fork (user-locked 2026-09-03) — FULL flow

The interceptor ingress (Kohai→Sempai→provider-prefix→http→save) does NOT exist
as a callable today — `host.kohai_complete` is net-new logic. User locked
**FULL**: build the complete flow now. Grounding confirmed the pieces:

- `ForensicPacket::new(run_id, iteration, CapturedPrompt)` → `AwaitingKohai` →
  `InterceptorStore::save` (PgInterceptorStore → `brassclaw_forensic_packets`).
- Optional Sempai: `with_sempai_review(SempaiReviewOutcome)` → `SempaiReviewed`
  (SempaiProposalSink — `NoopProposalSink`/`PgSempaiProposalSink`).
- Provider prefix chunk: `get_system_bundle(store, user_id, project_id) -> String`
  (`pg_basic_prompt_store.rs:285`, with `minimal_base_prompt_fallback`) — the
  System-message prefix to swap in for `prefix_placeholder`.
- Provider call: `LlmBackend::complete` (the existing ModelStage/"Kohai" call
  path — `handle_llm_complete` :962). Reused inside the port; Monty drives it via
  the host fn (satisfying "Monty, not the Rust agent-loop, drives the LLM"). The
  "via first_party_tools/http" in the seed is the conceptual route — `LlmBackend`
  reaches the provider over http internally.
- Close: `with_kohai_response(text, usage)` → `Complete` → `InterceptorStore::save`.

**Layering:** the interceptor store + basic-prompt store live in
`brassclaw_reborn_composition` (downstream of engine), so the handler (engine)
reaches them via a new engine port — the same pattern as `CompositionPort`
(C.4.5.17). The engine's `LlmBackend` is passed INTO the port call (the port impl
does not own an LLM backend).

## Slices (one-by-one; clippy green both configs + commit + push each)

- **Slice 1a — engine `KohaiPort` trait + types.** New `executor/kohai_port.rs`:
  `trait KohaiPort: Send + Sync { async fn complete(&self, prompt: serde_json::Value,
  ctx: KohaiCallCtx, llm: &dyn LlmBackend) -> Result<KohaiAnswer, KohaiPortError>; }`
  + `KohaiCallCtx { run_id, iteration, user_id, project_id, tenant_id }` +
  `KohaiAnswer { content: String, usage: {input_tokens, output_tokens, cost_usd} }`
  + `KohaiPortError`. Registered in `executor/mod.rs`. No impl yet (no callers).
  clippy green both configs.
- **Slice 1b — wire `KohaiPort` into the engine + dispatch arm + handler
  skeleton.** `ExecutionLoop` (`loop_engine.rs`) + `execute_orchestrator` param
  (`orchestrator.rs`) carry `kohai_port: Option<Arc<dyn KohaiPort>>` (like
  `composition_port`). Add `"kohai_complete" if call.method_call =>` arm (:~804)
  → `handle_kohai_complete(&args[1..], thread, llm, kohai_port)`. Handler: parse
  `prompt` kwarg + build `KohaiCallCtx` from thread; `None` port →
  `{ok:false, error:"kohai_unavailable"}`; `Some` → `port.complete(prompt, ctx,
  &**llm)` → `{ok:true, answer, usage}`. Narrow unit tests (MockKohaiPort +
  no-port degrade) mirroring `compose_orchestrator` handler tests (:3677+).
  clippy green both configs.
- **Slice 1c — `PgKohaiPort` impl in composition (the FULL flow).** New
  `pg_kohai_port.rs`: `complete` does (1) build `CapturedPrompt` from the prompt
  dict (messages from `chat_history`+`user_query`; segments; token accounting);
  (2) `ForensicPacket::new` → `PgInterceptorStore::save` [AwaitingKohai]; (3) if
  a Sempai is configured (rerouting), run the Sempai audit (LLM call to the
  Sempai model) → `with_sempai_review` → save [SempaiReviewed] — start with
  routing (no Sempai) + add Sempai behind the same path; (4) resolve provider
  prefix via `get_system_bundle(user_id, project_id)` → swap `prefix_placeholder`;
  (5) build final messages (system prefix + chat_history + user_query) →
  `llm.complete()`; (6) `with_kohai_response(text, usage)` → save [Complete];
  (7) return `KohaiAnswer`. `#[cfg(feature="skills-db")]` on the DB struct; pure
  helpers ungated + unit tests both configs. Registered in composition `lib.rs`.
  clippy green both configs.
- **Slice 2 — author `orchestrator/basic_mode.py`.** The v3 basic-mode script:
  receive input (last user msg in `context`) → `host.resolve_intent(user_input=)`
  → dispatch. Match → `host.compose_orchestrator(component_id, step_link,
  user_input)`; if `program.ok`: iterate `program.steplist` calling
  `host.run_program(step.executable_code)` per step (variable substitution is
  server-side in `compose_orchestrator`); consult `program.skills` for exact tool
  usage. No-match → resolve `host-non-match-llm-answer` recipe UUID via
  `host.resolve_component_by_name("host-non-match-llm-answer", 21)` →
  `compose_orchestrator`+`run_program` (the recipe internally calls
  `host.kohai_complete`); ultimate fallback → direct `host.kohai_complete`.
  Answer → `host.post_reply(text=answer)` → resolve `host-save-history` recipe →
  `compose_orchestrator`+`run_program` → `FINAL({state, answer, …})`. Idiomatic
  Monty 0.0.16 subset (no exec/eval/compile; no `re`; dicts/lists/strs; for-loops;
  host.* calls). `host.check_signals()` between phases.
- **Slice 3 — swap `DEFAULT_ORCHESTRATOR` + verification.** Replace
  `include_str!("../../orchestrator/default.py")` → `include_str!("../../orchestrator/basic_mode.py")`.
  Keep `default.py` on disk (C.7 deletes it). Both configs clippy green. Mark C.5
  done; commit + push. **Continue into C.6** (production driver switch — move
  `host.*` arms + `host` namespace from `execute_orchestrator` to the new
  cross-turn-persistent driver fn; replace canonical.rs stage pipeline as driver;
  end-to-end verify the script drives a turn).

## Status

[x] C.5 slice 1a DONE (2026-09-03 — engine `KohaiPort` trait + `KohaiCallCtx`/
    `KohaiAnswer`/`KohaiUsage` types + `KohaiPortError` in `executor/kohai_port.rs`,
    registered in `executor/mod.rs`; mirrors `CompositionPort`; both configs
    clippy-clean, 0 warnings). [x] 1b DONE (2026-09-03 — wired `kohai_port:
    Option<Arc<dyn KohaiPort>>` into `ExecutionLoop` (`loop_engine.rs`: struct
    field + constructor + `with_kohai_port` builder + call-site arg) +
    `execute_orchestrator` (`orchestrator.rs`: signature param +
    `"kohai_complete" if call.method_call =>` dispatch arm + `handle_kohai_complete`
    async fn: parse `prompt` kwarg/positional, validate `is_object`,
    None-port → `{ok:false,error:"kohai_unavailable"}`, invalid-prompt →
    `{ok:false,error:"invalid prompt: missing or not a dict"}`, build `KohaiCallCtx`
    from thread (run_id/iteration/user_id/project_id/tenant_id),
    `port.complete(prompt,ctx,&**llm)` → `{ok:true,answer,usage}`/err;
    5 `MockKohaiPort` unit tests (no-port, missing-prompt, non-dict-prompt,
    mock-success, port-failure); both configs clippy-clean + 10 handler tests green).
    [x] 1c DONE (2026-09-03 — `PgKohaiPort` FULL routing flow impl in composition
    `pg_kohai_port.rs`: parse prompt dict (chat_history + user_query +
    prefix_placeholder) → resolve per-scope provider prefix via
    `get_system_bundle` (infallible fallback) → build final messages + forensic
    `CapturedPrompt` → `ForensicPacket::new` + `InterceptorStore::save`
    [AwaitingKohai] → `ThreadMessage`s + `LlmBackend::complete(force_text)` →
    `with_kohai_response` + save [Complete] → return `KohaiAnswer`; Sempai path
    deferred (routing first, Sempai behind the same path when a sink is wired).
    Trait sig refined to `complete<'a>(&'a self, …, llm: &'a dyn LlmBackend) ->
    Future + 'a` (unifies `&self`+`llm` — the impl must drive the borrowed LLM
    inside the future, unlike `compose`'s `'static`). `#[cfg(feature="postgres")]`
    on the DB struct/impl; pure helpers + 10 unit tests ungated (both configs).
    Registered in composition `lib.rs`. Both configs clippy-clean (engine +
    composition) + 10 tests green). [ ] slice 2 — `basic_mode.py`.
    [ ] slice 3 — `DEFAULT_ORCHESTRATOR` swap + both-configs green. Then C.6.
