# 13 — Python Orchestrator (`default.py`)

> **Subsystem:** The **self-modifiable Python execution loop** that is the
> engine (Model A) outer turn driver. `default.py` runs inside the Monty VM,
> injected with Rust host functions, and is the thing that — today, in
> production — owns turn sequencing, prior-knowledge assembly, the LLM call
> (`__llm_complete__`), code/action execution, gate/approval pauses, and
> completion. It is the *alternative* to the agent-loop stage pipeline (see
> `12-agent-loop.md`); the two are the substance of the `DRIVER-GAP`.
> **Grounded in:** `crates/brassclaw_engine/orchestrator/default.py` (1472
> lines), `crates/brassclaw_engine/src/executor/orchestrator.rs` (the Rust
> host-function dispatch: `execute_orchestrator` at 444,
> `handle_assemble_prior_knowledge` at 2552, `handle_retrieve_docs` at 2488,
> `handle_llm_complete` at 795), `MESSAGE_FLOW_AND_PLAN_AUDIT.md` §2.2,
> `saved_plan_to_v3.md` Phase H item 3b + DRIVER-GAP/DRIVER-GAP-MODEL-A
> (lines 202, 229) + FIND-P9-02 / FIND-NEW-PASS12-01/02/03.

## 1. Purpose

`default.py` is introduced in its own header comment as *"the self-modifiable
execution loop. It replaces the Rust `ExecutionLoop::run()` with Python that
can be patched at runtime by the self-improvement Mission."* In the engine
(Model A) execution model, **Python is the outer loop**: `run_loop(context,
goal, actions, state, config)` iterates steps, assembles prior knowledge,
calls the LLM, executes code/actions, handles pauses, and returns an outcome
dict via `FINAL(result)`. There is no Rust stage pipeline on this path —
`__llm_complete__` (host function → `handle_llm_complete` at
`orchestrator.rs:795`) is the LLM call, called directly from Python.

This matters for the user's Task 3 description: the "message → orchestrator →
intent → recipe or LLM" flow the user described is, in production today, the
flow **inside this Python `run_loop`** (specifically the step-0 block), not
the agent-loop stage pipeline. The intent system is reached via
`__assemble_prior_knowledge__` → `PostgresSource::fetch_for_turn` →
`resolve_intent` (see `11-retrieval-system.md`), and the "recipe" vs "LLM
prompt" fork is the step-0 decision that the v3 plan refactors.

## 2. Location

- **Python source:** `crates/brassclaw_engine/orchestrator/default.py` — the
  whole script (1472 lines). Loaded and executed as a Monty VM program by
  `execute_orchestrator` (`orchestrator.rs:444`, `pub async fn`).
- **Rust host-function dispatch:** `crates/brassclaw_engine/src/executor/orchestrator.rs`
  - `execute_orchestrator` (444) — runs the **entire** Python script from
    scratch; takes `thread`, `llm`, `effects`, `leases`, `policy`,
    `signal_rx`, … (13+ params). The only externally-callable engine entry.
  - `handle_assemble_prior_knowledge` (2552) — **private** `async fn`; the
    `__assemble_prior_knowledge__` host handler (see `11-retrieval-system.md`).
  - `handle_retrieve_docs` (2488) — **private** `async fn`; the
    `__retrieve_docs__` host handler (legacy MemoryDoc path — dead shim, see
    §4.2).
  - `handle_llm_complete` (795, dispatched at 563) — the `__llm_complete__`
    host handler.
- **Host functions available to `default.py`** (header comment lines 7-29):
  `__llm_complete__`, `__execute_code_step__`, `__execute_action__`,
  `__execute_actions_parallel__`, `__check_signals__`, `__emit_event__`,
  `__save_checkpoint__`, `__transition_to__`, `__retrieve_docs__`,
  `__check_budget__`, `__get_actions__`, `__list_skills__`,
  `__record_skill_usage__`, `__regex_match__`, `__validate_component__`,
  `__assemble_prior_knowledge__`, `__get_reduction_rules__`,
  `__set_active_skills__`, `__log_budget_warning__`.
- **Context variables** injected by Rust before execution (lines 31-36):
  `context` (prior messages), `goal` (thread goal), `actions`, `state`
  (persisted state dict), `config` (thread config dict).
- **Plan:** Phase H item 3b (`execute_recipe_orchestrator_channel` spec),
  DRIVER-GAP / DRIVER-GAP-MODEL-A (lines 202, 229), FIND-P9-02,
  FIND-NEW-PASS12-01/02/03, FIND-P9-15 (crate boundary),
  FIND-P7-15 (return-shape evolution).

## 3. Data model

### `run_loop(context, goal, actions, state, config) -> dict`

Returns an **outcome dict** via `FINAL(result)` (line 1470-1471). The shape
is `complete_result(state, outcome, response=None, error=None, extra=None)`
(line 650): `{outcome, state, response, error, …}`. Outcomes: `completed`,
`stopped`, `max_iterations`, `error`, `gate_paused`, `need_approval`,
`need_authentication`.

### `working_messages` (the mutable transcript)

`ensure_working_messages(state, context)` (line 150) initializes
`state["working_messages"]` from `context` on first call and returns the live
list. All prompt assembly (`insert_as_user_message_at_n_minus_1`,
`append_message`) mutates this list. `compact_if_needed` and
`_reduce_prompt` may replace it wholesale (the return value must be
captured — the comment at 1078-1080 warns that discarding it "loses the
trimmed prefix").

### `state` (persisted across steps/turns)

`state.setdefault("history", [])`, `state.setdefault("compaction_count", 0)`,
plus per-step keys: `state["step_{n}_return"]`, `state["last_return"]`,
`state[r.get("action_name")]` (action outputs), `state["active_skill_ids"]`,
`state["skill_snippet_names"]`, `state["_obligation_nudge_count"]`,
`state["_obligation_resolved"]`. Checkpoints persist `{nudge_count,
consecutive_errors, consecutive_action_errors, compaction_count,
obligation_nudge_count}`.

### `__assemble_prior_knowledge__` return shape (today)

`{content, formatted_content, override_prompt_creation,
matched_component_ids}` (lines 24-29):
- `content` — raw PKC text (Rust-internal; action dispatch + KV fingerprint;
  never sent to the LLM).
- `formatted_content` — LLM-readable JSON (sent to `working_messages`).
- `override_prompt_creation` — if true, `formatted_content` becomes the
  complete prompt (Solution Override path).
- `matched_component_ids` — UUIDs of matched components.

> **FIND-P7-15:** the v3 plan extends this to also carry `action_short_circuit`,
> `tier_zero`, `action_component_id`, `action_name`, `disambiguation`,
> `candidates` — the `PkrAssemblyResult` shape (see `12-agent-loop.md` §6).

## 4. Behavior

### 4.1 The main loop (`run_loop`, line 932)

`for step in range(step_count, max_iterations)` (default 30 iterations;
`max_consecutive_errors` default 5, `None` = no limit). Per iteration:

1. **Signals** — `__check_signals__()`; `"stop"` → `complete_result(state,
   "stopped")`; `{"inject": msg}` → append a User message.
2. **Budget** — `__check_budget__()`; **tokens** is *soft telemetry only*
   (the post-assembly reduction pipeline shrinks the prompt, never aborts);
   **time** and **cost** are hard-stops (`completed`).
3. **Step-0 block** (`if step == 0`, §4.2) — prior-knowledge assembly +
   action short-circuit + skill registration.
4. **Post-assembly reduction** (line 1061) — if
   `estimate_context_tokens(working_messages) > prompt_budget`, fetch
   `__get_reduction_rules__()` and run `_reduce_prompt`; emit
   `prompt_over_budget` telemetry if still over.
5. **Compaction** — `compact_if_needed(state, config)` (line 1098); may
   summarize history via `__llm_complete__(summary_messages, None,
   {"force_text": True})` (line 537).
6. **LLM call** — `response = __llm_complete__(working_messages, actions,
   None)` (line 1103); emit `step_started`/`step_completed` with usage.
7. **Response handling** by `response.get("type", "text")`:
   - **`text`** (line 1111): append Assistant message; `extract_final` →
     `FINAL()` → `complete_result(state, "completed", final_answer)`. Else
     execution-obligation nudge (if `require_action_attempt` and the model
     replied text-only with actions available and unresolved obligation,
     bump `_obligation_nudge_count` < `max_obligation_nudges` and `continue`).
     Else plain text → `complete_result(state, "completed", text)`.
   - **`code`** (line 1141): mark `_obligation_resolved`; append the code as
     a fenced `repl` block; `__execute_code_step__(code, state)`; persist
     `return_value`/action outputs; `format_output` → append as User;
     `extract_final` in code output → completed. Else **gate pause**
     (`pending_gate`/`need_approval` with `gate_paused`) → checkpoint +
     `waiting` + `{outcome: "gate_paused", …}`. Else legacy
     `need_approval` → `need_authentication` or `need_approval` with
     checkpoint. Else `had_error` → bump `consecutive_errors`, abort at
     `max_consecutive_errors`.
8. **Loop end** — `max_iterations` → `complete_result(state, "max_iterations")`.

### 4.2 The step-0 block (the three calls)

`if step == 0:` (line 994) runs once, before the first LLM call:

**(a) Prior-knowledge assembly** — `__assemble_prior_knowledge__(goal,
token_budget, "02")` (line 997; `prior_knowledge_token_budget` default 100000):
- **Solution Override** (`pkr["override_prompt_creation"]`, line 999):
  `working_messages = [{"role": "User", "content": pkr["formatted_content"]}]`
  — the formatted PKC becomes the stable KV-cache base.
- **Normal Assembly** (`pkr["formatted_content"]`, line 1003):
  `insert_as_user_message_at_n_minus_1(working_messages, pkr["formatted_content"])`
  — inject formatted prior knowledge at N-1 so the KV-cache stable prefix
  (identities/skills/memory, priorities 1-5) is never disturbed.
- Always `insert_volatile_context_at_n_minus_1(working_messages)` (line
  1008) — **a no-op placeholder today** ("wired in Phase 5.2b", line 204);
  under Override this is the only non-PKC message the model sees.

**(b) Action short-circuit shim (DEAD)** — lines 1010-1028: a second
`__retrieve_docs__(goal, 5)` pass that looks for `metadata["class_code"] ==
16` and, if found, `execute_action_procedure(doc, goal, state)` and returns
immediately (no `__llm_complete__`). **This shim never fires in production**
because `__retrieve_docs__` returns MemoryDocs whose metadata does **not**
carry `class_code` (the legacy MemoryDoc path has no class-code field). The
comment block at 1010-1017 explains it is a "Pre-Phase-5 fallback" until
`__assemble_prior_knowledge__` surfaces Action IDs via
`matched_component_ids`. Phase G replaces the call; Phase K.3 removes the
comment artefact. (§0.9 Problem 1 — see `08-actions-system.md`.)

**(c) Skill registration** — `all_skills = __list_skills__()`;
`active_skills = select_skills(all_skills, goal, max_candidates=3,
max_tokens=6000)` (line 1032); build `active_skill_payload` (doc_id, name,
version, snippet_names, `force_activated: False`) **without**
list-comprehensions-with-if (Monty-safe §conventions); `__set_active_skills__`;
emit `skill_activated`; store `state["active_skill_ids"]` +
`state["skill_snippet_names"]`. Note: skills are assembled into the stable
system-prompt prefix by **Rust** (`InstructionBundleBuilder` priority 2);
Python only registers which are active for tracking/event emission.

### 4.3 `execute_action_procedure(action_doc, goal, state)` (no-LLM, line 901)

The deterministic class-16 Action executor. §3.11 dispatch flow steps 5-8:
`prior_knowledge_content` is given to `default.py`; it recognises class_code
16 and stops further prompt creation; performs the Action directly; the
Action's return value becomes the turn result. Implementation: `scope_vars
= {"goal": goal}`; `_execute_action_steps(action_doc, scope_vars, 0,
[0])`; if no explicit `return` → `complete_result(state, "completed",
"Action completed.")`; if `"error"` → `complete_result(state, "error", …)`;
else `complete_result(state, "completed", result["result"])`. Returns
directly from `run_loop` **without calling `__llm_complete__`** — this is
the no-LLM pattern that v3 Tier 0 generalises.

### 4.4 Compaction & reduction (helpers)

- `_reduce_prompt(messages, rules, budget_tokens)` (line 469) — applies
  reduction rules (`truncate`/`summarize`/`drop`/`priority`/`history_compact`)
  built by `make_*_rule` factories; **returns a new list** for
  `history_compact` (must be captured).
- `compact_if_needed(state, config)` (line 489) — when
  `estimate_context_tokens` exceeds `COMPACTION_THRESHOLD_DEFAULT` (0.85)
  of the context limit, summarizes older history via an LLM call.

## 5. Relations

- **Retrieval** (`11-retrieval-system.md`) — `__assemble_prior_knowledge__`
  → `PostgresSource::fetch_for_turn` → `resolve_intent`; `__retrieve_docs__`
  → legacy `retrieve_context` (dead shim).
- **Skills** (`05-skills-system.md`) — `__list_skills__` + `select_skills`
  + `__set_active_skills__` registration; Rust assembles the bodies.
- **Actions** (`08-actions-system.md`) — `execute_action_procedure` is the
  no-LLM class-16 executor; the dead `__retrieve_docs__`+`class_code==16`
  shim; v3 `action_short_circuit` replaces it.
- **IBS** (`04-ibs.md`) — the `formatted_content`/`content` split is the PKC
  two-surface design (§3.13/§3.14); KV-cache discipline.
- **Prefix/base-prompt** (`10-prefix-base-prompt.md`) — the
  `base-prompt` placeholder substitution is *not* in `default.py` today
  (Phase K.1, interceptor-side); the Solution Override `working_messages`
  reset is the closest existing analogue.
- **Sempai-Kohai** (`09-sempai-kohai.md`) — **not present in `default.py`
  today**: Model A has no stage pipeline, so there is no `InterceptorStage`
  on this path. The interceptor runs only on the agent-loop (Model B/C)
  path. This is part of the `DRIVER-GAP`.
- **Agent loop** (`12-agent-loop.md`) — `default.py` is the Model A driver;
  the stage pipeline is Model B/C. The v3 transition retires `__llm_complete__`
  in favor of `ModelStage`.

## 6. Today vs. v3

| Aspect | Today | v3 (Phase H) |
|---|---|---|
| Outer turn driver | `run_loop` in `default.py` (Model A) — Python is the outer loop | agent-loop `DefaultExecutorPipeline` (Model B/C); `ModelStage` owns the LLM call; `run_loop` reduced to step-0 + Tier-0 |
| LLM call | `__llm_complete__(working_messages, actions, None)` (line 1103) | `ModelStage` (agent-loop); `__llm_complete__` retired on the agent-loop path |
| `__assemble_prior_knowledge__` return | `{content, formatted_content, override_prompt_creation, matched_component_ids}` | + `action_short_circuit`, `action_component_id`, `action_name`, `tier_zero`, `disambiguation`, `candidates` (`PkrAssemblyResult`) |
| Action detection | dead `__retrieve_docs__`+`class_code==16` shim (never fires) | `action_short_circuit: true` + `action_component_id` from `__assemble_prior_knowledge__`; `__fetch_component__(id, 16)` + `execute_action_procedure` |
| Tier 0 (no-LLM recipe) | **does not work** — `override_prompt_creation: true` only swaps `working_messages` and *falls through* to `__llm_complete__`; the only pre-LLM return is the dead shim | dedicated `tier_zero: true` pkr signal (emitted when `SplitResult.llm_call_required == false`); new `if pkr.get("tier_zero"): return execute_recipe_orchestrator_channel(...)` early-return branch in step-0, sibling of `action_short_circuit`, before `__llm_complete__`; generalises `execute_action_procedure` no-LLM pattern from class-16 Actions to Tier-0 Recipes |
| `execute_recipe_orchestrator_channel` | does not exist | new Python helper (Model A path): extract `orchestrator_content`, run PythonCode formatter bodies, call `__execute_action__` on the Rust executioner via ToolSkill bindings; **Tier-0 recipes must be PythonCode-only** (Skill bodies are LLM prose, not executable — Q1 rule) |
| `insert_volatile_context_at_n_minus_1` | no-op placeholder ("wired in Phase 5.2b") | wired |
| `__retrieve_docs__` shim | present (dead) | removed (Phase K.3) — registration + body + comment artefact; `__assemble_prior_knowledge__` surfaces Action IDs directly |
| `recipe_hint` stash/unstash | n/a (no `LoopExecutionState`) | **Model B/C only**: the stage extracts `recipe_hint` from state, passes it as a parameter to `run_step_zero` (the composition host calls the new `pub` `assemble_prior_knowledge_with_hint`, **not** `handle_assemble_prior_knowledge` — it is private and takes `&[MontyObject]`); the stage clears `state.recipe_hint` *after* `run_step_zero` returns (the Python handler has no `&mut state` access — FIND-P9-15) |
| Composition-host Tier-0 entry | n/a | new `pub` `execute_tier_zero_channel(...)` Rust function (the composition host calls this, **not** the Python `execute_recipe_orchestrator_channel` which is internal to the VM — FIND-NEW-PASS12-02/03) |
| Interceptor | not on this path (Model A has no stage pipeline) | `InterceptorStage` on the agent-loop path; `base-prompt` substitution Phase K.1 |

> **The user's Task 3 description vs. `default.py` today.** The user's flow
> ("message → orchestrator → intent → recipe or LLM") is **architecturally
> correct in shape** but the intent-system fork is the step-0 block here,
> and today it does **not** return a "recipe" — `__assemble_prior_knowledge__`
> returns prior-knowledge components, not a recipe step-list. The
> `recipe`/`tier_zero`/`action_short_circuit` distinction is the v3
> refactoring of this step-0 block. The "base prompt" KV-cache does not exist
> yet (the Solution Override `working_messages` reset is the closest
> analogue). See `MESSAGE_FLOW_AND_PLAN_AUDIT.md` §2.2 for the full
> three-way comparison.

## 7. LLM summary (for prompt injection)

`default.py` is the self-modifiable Python execution loop that is the engine
(Model A) outer turn driver — Python is the outer loop, calling the LLM via
`__llm_complete__`. `run_loop(context, goal, actions, state, config)` iterates
up to `max_iterations` steps: check signals/budget, then on step 0 call
`__assemble_prior_knowledge__` (prior-knowledge assembly → Solution Override
or Normal Assembly at N-1), run the dead `__retrieve_docs__`+class-16
action short-circuit shim (never fires), and register active skills via
`__list_skills__`/`select_skills`/`__set_active_skills__`. Each step then
reduces/compacts the prompt and calls `__llm_complete__`; responses are
`text` (`FINAL()` extraction, obligation nudge, or completion) or `code`
(`__execute_code_step__`, gate/approval/auth pauses, error tracking).
`execute_action_procedure` runs class-16 Actions deterministically with no
LLM — the pattern v3 Tier 0 generalises. Today the intent-system fork lives
in step 0 but returns prior-knowledge components, not a recipe; Tier 0 does
**not** work (`override_prompt_creation: true` still calls the LLM; the only
pre-LLM return is the dead shim). Phase H refactors step 0 to carry
`action_short_circuit`/`tier_zero`/`recipe_hint` signals, adds the
`execute_recipe_orchestrator_channel` Python helper (Model A, PythonCode-only
Tier-0) and two new `pub` Rust functions (`assemble_prior_knowledge_with_hint`,
`execute_tier_zero_channel`) for the agent-loop (Model B/C) path, and retires
`__llm_complete__` in favor of `ModelStage` once the agent loop becomes the
driver. The `base-prompt` placeholder substitution is not in `default.py`
today (Phase K.1, interceptor-side).
