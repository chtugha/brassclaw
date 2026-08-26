# Subplan — Phase H problem resolution (`saved_plan_to_v3.md`)

Parent plan: `saved_plan_to_v3.md` → Phase H (`lines 5440–6565`) + §0.9 pseudocode
(`lines 1260–1389`) + §5 Tier-0 diagram.
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase H step `9d94d6cb-3a45-47cc-8e0f-85203c936652`.

---

## 1. Why this subplan exists — the Phase H two-runtime scope

Phase H is the largest architectural phase in the plan: it activates **Tier-0
deterministic recipe execution** (no LLM) and **Tier-1 guided execution** across
**two parallel runtimes**:

- **Model A** — the *production* engine `ExecutionLoop` / Monty `default.py` path
  (`loop_engine.rs::run` → `execute_orchestrator` → `default.py::run_loop`). This
  is what drives real turns today. Phase H adds the `default.py` step-0
  `tier_zero` early-return branch + `execute_recipe_orchestrator_channel` +
  `_parse_orchestrator_channel_steps`, plus the engine→composition outcome-
  recording bridge.
- **Model B/C** — the *agent-loop* `DefaultExecutorPipeline` / `CanonicalAgentLoopExecutor`
  target-state (`canonical.rs`), where `RecipeStage`, `PromptStage`,
  `AssistantReplyStage`, and the new `TierZeroExecutionStage` live. This is
  **test-only until switchover** (no product surface drives it yet), but the plan
  specifies its full target shape: three `brassclaw_turns`-native types, a new
  `LoopOrchestratorPort`, `RecipeStep::TierZero`/`ActionExecuted`, a restructured
  `PostRecipeOutcome`, two new engine `pub` fns, and the composition host impl.

Grounding the codebase revealed that **Phase E.0 already implemented most of H3 +
H4** (the agent-loop retrieval + user-text plumbing). This subplan records the
E.0-already-done state, the four design decisions answered by the user before
implementation (§3), and the exact 13-substep implementation sequence (§4). Each
substep is committed + pushed individually before the next begins.

---

## 2. Grounding findings (confirmed against current code)

### 2.1 What Phase E.0 already delivered (H3 + H4 — NOT redone by Phase H)

- **`LoopExecutionState`** (`state.rs:108–124`) already carries
  `last_user_text: Option<String>` AND `last_retrieval_result:
  Option<RetrievalTurnResult>`. The latter carries `rust_items` +
  `orchestrator_items` as `serde_json::Value` + the routing booleans
  (`tier0_eligible`, `llm_call_required`, `routing_meta`).
- **`LoopContextPort::resolve_message_text`** (`host.rs:799`, default returns
  `Err(Unimplemented)`) + composition `SkillActivationMessageTextResolver`
  (`retrieval_lookup_impl.rs:280–320`, reads raw accepted-message body via
  `peek_message_text`, NOT the redacted `safe_summary`).
- **`LoopRetrievalPort`** + `retrieval_lookup()` accessor + `NoRetrieval` default
  (`host.rs:2130–2142`) — the 14th supertrait port. Composition
  `RetrievalLookup` impl exists (`retrieval_lookup_impl.rs`).
- **`consume_drainable_inputs`** returns a 4th `Option<LoopMessageRef>`;
  `InputStage::drain` resolves it via `resolve_message_text` and stores into
  `state.last_user_text` (`input.rs:131–240`).
- **`RecipeStage::process`** (`recipe.rs:65–136`) already calls
  `fetch_for_turn` + stashes into `state.last_retrieval_result` — **producer-
  only**, always returns `RecipeStep::Continue` (Tier-2 fall-through).

→ **Phase H does NOT re-implement the retrieval port, user-text plumbing, or the
  `RecipeStage` producer fetch.** It consumes them.

### 2.2 What Phase E already added to the engine routing types

- **`TurnRoutingSignals`** (`retrieval_source.rs:108–137`):
  `{override_prompt_creation, matched_component_ids, variant_label, step_link,
   llm_call_required, wilson_lower, tier0_eligible}`.
- **`FetchForTurnResult`** (`retrieval_source.rs:148–184`):
  `Components(Vec<ComponentItem>)` / `Disambiguation(Vec<IntentCandidate>)` /
  `ActionShortCircuit{component_id,name}` /
  `SplitResult{rust_items, orchestrator_items, routing, instruction:
   Option<BuildInstruction>}`.

### 2.3 What already exists in the engine Python path

- **`handle_assemble_prior_knowledge`** (`orchestrator.rs:2601`) already emits the
  full §0.9 routing dict from the `SplitResult` arm (`orchestrator.rs:2739`):
  `orchestrator_content` (prose block) + `formatted_content` (alias) +
  `tier_zero: !routing.llm_call_required` + `override_prompt_creation` +
  `matched_component_ids` + `action_short_circuit` + `disambiguation` +
  `candidates` + `active_skills` + `rust_items` + `variant_label` + `step_link`
  + `wilson_lower` + `llm_call_required` + `tier0_eligible`.
  → **Model A's `tier_zero` PKR signal is already wired into the Python `pkr`
    dict.** What's MISSING is the `default.py` step-0 `tier_zero` early-return
  *branch* (currently a comment at `default.py:1094`: "deferred to Phase H (Q-G1)")
  + `execute_recipe_orchestrator_channel` + `_parse_orchestrator_channel_steps`.
- **`__execute_code_step__` IS registered** (`orchestrator.rs:581` dispatch →
  `handle_execute_code_step` at `:1007`). → Model A Tier-0 PythonCode execution
  can use it directly (fresh Monty state per step).
- **`format_orchestrator_content`** (`orchestrator.rs:3003–3017`) produces
  `## [{heading}: {name}]\n{effective_content}` blocks joined by `\n\n`.
  `heading` = `StepContextSpec::from_class_code(class_code).heading()` — the
  Capitalized category label. **Exact heading strings** (`instruction_builder.rs:276–295`):
  `Skill`, `Spec`, `Recipe`, `PythonCode`, `Catalogue`, `Annotation`,
  `Extension`, `Orchestrator`, `Plan`, `Summary`, `Action`, `Docu`, `Lesson`,
  `Issue`, `Note`, `Scaffold`, `Tool`, `Component`. Class 13 (ToolSkill) and
  class 11 (reserved) → `None` (skipped, never in `orchestrator_content`).
  → This is the exact block format `_parse_orchestrator_channel_steps` (H.1) must
    parse: split on `\n\n`, each block matches `## [{Heading}: {name}]` then the
    body is the remainder. Heading-only block (empty `effective_content`) →
    `## [{Heading}: {name}]` with no body line.

### 2.4 The outcome-recording gap (the real Phase H gap for Model A)

- **`record_recipe_outcome(recipe_id, success)`** is a `RecipeLookup` trait
  method (`recipe_lookup.rs:104–108`) + composition impl (`recipe_library.rs:115`
  → `self.recorder.record_recipe(project_id, "default", recipe_id, success)`).
- **It is NOT called from the engine `ExecutionLoop` / `loop_engine.rs` today.**
  Verified — no hook. `execute_orchestrator` returns
  `Result<OrchestratorResult, EngineError>` where `OrchestratorResult{outcome:
  ThreadOutcome, tokens_used}` (`orchestrator.rs:64–69`) carries NO `recipe_id`.
  `ExecutionLoop::run` (`loop_engine.rs:413`) calls `execute_orchestrator` at
  `:471` and returns `Result<ThreadOutcome, EngineError>`.
- → The Q-H3 outcome-recording bridge must surface `recipe_id` + Tier-0 outcome
  up through `execute_orchestrator`'s return so composition can call
  `record_recipe_outcome`. `TurnRoutingSignals` has no `recipe_id` field — so
  **recipe_id surfacing is a real gap** that H.4 must close (possibly via a
  nested subplan if the plumbing proves large).

### 2.5 What is ABSENT and Phase H must create (Model B/C)

- **`RetrievalTurnResult`** exists (turns-native, added E.0). **`PriorKnowledgeBundle`
  + `TierZeroReply` are NOT defined** (`host.rs` grep → 0). FIND-P9-03.
- **`LoopOrchestratorPort` + `NoOrchestrator`** — absent. The 15th supertrait port
  + blanket-impl `where`-clause update (FIND-NEW-PASS12-05).
- **`LoopPromptBundleRequest.recipe_hint`** — absent (`host.rs:1012–1022`).
- **Engine `pub` fns `assemble_prior_knowledge_with_hint` +
  `execute_tier_zero_channel`** + `PkrAssemblyResult` / `TierZeroChannelResult`
  structs — absent. `handle_assemble_prior_knowledge` must be refactored to
  delegate (FIND-NEW-PASS12-01/02).
- **`RecipeStep::TierZero` / `RecipeStep::ActionExecuted`** — absent
  (`recipe.rs:55–59` only has `Continue`).
- **`RecipeStage` consumer dispatch** — currently producer-only (always
  `Continue`); must branch on `tier0_eligible && !llm_call_required`.
- **`canonical.rs` `PostRecipeOutcome` restructuring + `TierZeroExecutionStage`** —
  absent (`canonical.rs:90–96` only handles `Continue`).
- **`state.recipe_hint` + `state.recipe_rust_context`** — absent (E.0 stashed
  into `last_retrieval_result`; H.9 migrates per Q-H1).
- **Composition `LoopOrchestratorPort` impl** + `build_prompt_bundle` reading
  `recipe_hint` — absent.

---

## 3. Design decisions (answered by user before implementation)

**Q-H1 (state stash shape):** E.0 stashed retrieval as
`state.last_retrieval_result: Option<RetrievalTurnResult>` (carries
`rust_items`+`orchestrator_items` as `serde_json::Value` + routing booleans). The
plan says ADD `recipe_hint: Option<serde_json::Value>` +
`recipe_rust_context: Vec<serde_json::Value>` and migrate from
`last_retrieval_result`.
→ **Decision: Add `recipe_hint` + `recipe_rust_context`, migrate from
  `last_retrieval_result` (plan-literal).** `recipe_hint` = the
  `orchestrator_items` `serde_json::Value`; `recipe_rust_context` = the
  `rust_items` array.

**Q-H2 (scope + ordering):** Phase H specifies BOTH Model A (production
`default.py` `tier_zero` branch + `execute_recipe_orchestrator_channel` +
`_parse_orchestrator_channel_steps`) AND Model B/C (agent-loop target state).
→ **Decision: Both; Model A first (production Tier 0), then Model B/C
  (target-state stages).**

**Q-H3 (Model-A Tier-0 outcome recording):** The engine `ExecutionLoop` has no
`record_recipe_outcome` / Wilson hook today. Plan tests require Tier-0
success/failure → `record_recipe_outcome` → wilson_lower/tier updated.
→ **Decision: Add an engine→composition outcome-recording bridge so
  `execute_orchestrator`'s caller records the Tier-0 outcome (engine carries
  recipe_id + success/error up through `OrchestratorResult`).**

**Q-H4 (Tier-0 success/failure definition):**
`execute_recipe_orchestrator_channel` returns `{outcome:'success', result}` or
`{outcome:'error', message}`.
→ **Decision: `outcome == 'success'` → success; `'error'` → failure (sole
  signal).**

---

## 4. Implementation sequence (13 substeps, one-at-a-time, commit+push each)

Per the user's hard rule: **no batching, no parallelizing, no skipping, no stubs.**
Each substep: implement → fmt → clippy (both configs where relevant) → tests →
commit (explicit-pathspec guard; never stage `tomedo_v3.md`/`whatsapp_v3.md`)
→ push `origin/main` → mark Zenflow substep progress → continue immediately.
`CARGO_TARGET_DIR=/Users/ollama/brassclaw-target` is set on every build command.

### Model A — production engine Python path (H.1–H.5)

**H.1 — `_parse_orchestrator_channel_steps` helper in `default.py`** (+ Monty unit
test). Parses `orchestrator_content` (the `## [{Heading}: {name}]\n{body}` prose
block format from `format_orchestrator_content`, §2.3) back into a list of step
dicts `[{kind, name, body}]` where `kind` is the heading label (`"PythonCode"`,
`"Skill"`, …). Split on `"\n\n"`; each block: first line must match
`## [LABEL: NAME]` → `kind=LABEL`, `name=NAME`; remaining lines = `body` (empty
if heading-only). Unknown/non-`## [...]` blocks → error. Monty-safe: string
`+`/`str()`, `in`, `.split()`, `len()`, `.startswith()`, index slicing — NO
f-strings, NO `.format()`, NO `re`. Test: feed a known
`## [PythonCode: greet]\nprint("hi")` block, assert the parsed list.

**H.2 — `execute_recipe_orchestrator_channel` helper in `default.py`** (+ unit
tests). The Tier-0 no-LLM channel executor. Input: `orchestrator_content` string
(from `pkr`). Flow: call H.1 to parse steps; for each step: only `kind ==
"PythonCode"` steps execute via `__execute_code_step__(body, {})` (fresh Monty
state per step — ISOLATION INVARIANT; the `{}` empty kwargs means no shared
vars between steps). `kind == "ToolSkill"` (Rust-channel, should never appear in
orchestrator_content per §2.3) or unknown/empty → return
`{outcome:"error", message: ...}`. Collect each PythonCode step's return value;
on first error return `{outcome:"error", message: str(e)}`. On all-success:
`{outcome:"success", result: <last step result>}` (per Q-H4 — `outcome` string
is the sole signal). Monty-safe throughout. Tests: single PythonCode step
returns success; a step raising an error returns `{outcome:"error"}`; a non-
PythonCode kind returns error; empty steps return error.

**H.3 — `default.py` step-0 `tier_zero` early-return branch** (+ unit test). In
`run_loop` step-0, inside `if isinstance(pkr, dict):`, as a **sibling of**
`action_short_circuit` / `disambiguation` / `override_prompt_creation` /
`orchestrator_content` (a NEW leading `if pkr.get("tier_zero"):` branch, BEFORE
`__llm_complete__` and NOT via `override_prompt_creation`). When
`pkr.get("tier_zero")` is truthy AND `pkr.get("orchestrator_content")`: call
H.2 `execute_recipe_orchestrator_channel(pkr["orchestrator_content"])`; on
`outcome == "success"` → `__transition_to__("completed", "tier-0 recipe
executed")` + `return complete_result(state, "completed", response=result)`;
on `outcome == "error"` → fall through to Tier-2 (LLM) — a Tier-0 failure does
NOT abort the turn, it degrades to a normal LLM call (so the user still gets a
reply). Removes the `default.py:1094` "deferred to Phase H (Q-G1)" comment.
Test: step-0 harness with a `pkr` carrying `tier_zero=True` +
`orchestrator_content` of a simple PythonCode step → run_loop returns
`completed` with the step's output and never calls `__llm_complete__`.

**H.4 — engine→composition Tier-0 outcome-recording bridge** (resolves Q-H3). The
hard part. Goal: after a Tier-0 run, composition calls
`record_recipe_outcome(recipe_id, success)`. Sub-problem (§2.4): `recipe_id` is
not surfaced today. **The plumbing proved large (engine `types/event.rs` +
`memory/retrieval_source.rs` + `executor/orchestrator.rs` + composition event
listener) so H.4 SPAWNED the nested subplan
`./docs/agents-v3/subplan_problem_stepH4_of_saved_plan_to_v3.md` (Zenflow nested
subplan substep; H4.1–H4.8 one-by-one). Two design questions answered before
implementation: Q-H6 (mixed mechanism — success via
`complete_result(extra={"tier_zero_outcome":{recipe_id,success:true}})` +
failure via the `recipe_tier_zero_failed` event read from `thread.events`) and
Q-H7 Architecture A via A2 (event-based — typed
`RecipeTierZeroStarted`/`Succeeded`/`Failed` `EventKind` variants carrying
`recipe_id` + a composition event listener calls
`PgRecipeLibrary::record_recipe_outcome(recipe_id, success)` fire-and-forget;
`OrchestratorResult.tier_zero_outcome` still populated from extra/event for
tests; no duplicated Wilson SQL, no new engine `brassclaw_turns` dep).
`recipe_id` surfacing: add `recipe_id: Option<String>` to
`TurnRoutingSignals`, populate at both `fetch_recipe_split_result` construction
sites, surface it into the `pkr` dict, and emit it on the Tier-0 events.
`record_recipe_outcome` is a fire-and-forget best-effort call (errors logged at
`debug!`, never break the turn).

**H.5 — Model A composition integration test.** Tier-0 recipe in the DB:
`wilson_lower >= 0.70`, `validation_status = 'validated'`, `llm_call_required =
false`, a `step_link` whose IBS compiles to a single PythonCode step whose body
returns a constant. Drive `execute_orchestrator` (via the composition
`ThreadManager` / `loop_engine` test path) → assert: orchestrator runs the
channel, NO LLM call made, returns `completed` with the PythonCode result, and
`record_recipe_outcome(recipe_id, true)` was invoked (verify via a test
recorder / spy on the `RecipeLookup` impl). Skip-if-no-docker (real Postgres).
This is the Model A acceptance test.

> **Obsolete/Skipped (v3 Phase H.5 obsolescence subplan O1–O5).** Grounding
> overturned the §1 claim that Model A "drives real turns today": production
> turns run on the agent loop, not the engine Python runtime, so Model A is
> dormant/never-built. H.5 was re-scoped to the obsolescence subplan
> `./subplan_problem_stepH5_obsolescence_of_saved_plan_to_v3.md` (O1–O5):
> missions purged, the `brassclaw_gateway` crate deleted, the Model A Python
> `tier_zero` branch removed (O3), and the reusable H.4 pieces re-documented
> as reused by Model B/C (O4). H.5 itself is **skipped**; Phase H resumes at
> H.6 (Model B/C). The severed engine Python runtime wrapper + H.1/H.2 Python
> fns are deleted in a later subplan opened after H.8.

### Model B/C — agent-loop target state, test-only (H.6–H.13)

**H.6 — `PriorKnowledgeBundle` + `TierZeroReply` turns types**
(`brassclaw_turns/src/run_profile/host.rs`). The two structs from FIND-P9-03
(plan lines 5486–5504), `#[derive(Debug, Clone, serde::Serialize,
serde::Deserialize)]`, `pub` fields exactly as specified. `RetrievalTurnResult`
already exists.

**H.7 — `LoopOrchestratorPort` + `NoOrchestrator` + supertrait/blanket update +
`LoopPromptBundleRequest.recipe_hint`** (`host.rs`). The port trait (plan lines
5619–5640) with `run_step_zero` + `run_tier_zero`. `NoOrchestrator` default impl
returning `None` (mirror `NoRetrieval`/`NoRecipeLookup`). Add as the **15th**
supertrait (after `LoopRetrievalPort`); update BOTH the supertrait declaration
AND the blanket `impl<T> AgentLoopDriverHost for T where ...` `where`-clause
(FIND-NEW-PASS12-05). Add `recipe_hint: Option<serde_json::Value>` (with
`#[serde(default)]`) to `LoopPromptBundleRequest` (`host.rs:1012`) + update ALL
construction sites (grep `LoopPromptBundleRequest`).

**H.8 — engine `pub` fns + structs + `handle_assemble_prior_knowledge` refactor**
(`orchestrator.rs`). Add `pub struct PkrAssemblyResult` (plan lines 5681–5691) +
`pub struct TierZeroChannelResult` + `pub async fn assemble_prior_knowledge_with_hint`
(plan lines 5670–5677) + `pub async fn execute_tier_zero_channel` (plan line 5704+,
the Tier-0 channel executor embodying the `execute_recipe_orchestrator_channel`
logic as a Rust library fn — drives `__execute_code_step__`-equivalent fresh-state
Python execution per step). Refactor `handle_assemble_prior_knowledge` to delegate
to `assemble_prior_knowledge_with_hint(.., None)` and `json_to_monty` the result.
Re-export from the crate. +7 unit tests (per plan).

**H.9 — `state.rs` `recipe_hint` + `recipe_rust_context` + `RecipeStage` migration
+ SEC-02 clear** (`brassclaw_agent_loop`). Add the two fields (Q-H1);
`initial_for_run` sets `None`/`Vec::new()`. `RecipeStage::process` migrates: on
`Ok(Some(result))`, populate `state.recipe_hint = Some(result.orchestrator_items)`
+ `state.recipe_rust_context = result.rust_items` (as the plan-literal split),
replacing the `state.last_retrieval_result = Some(result)` stash. Keep
`last_retrieval_result` populated too (E.0 artifact — documented upgrade, not
deleted) OR migrate cleanly per the plan-literal (decide during H.9; if unclear,
ask). SEC-02: clear `recipe_hint`/`recipe_rust_context` at the top of the next
turn (one-shot consume semantics — `run_step_zero`/`run_tier_zero` consume them).

**H.10 — `RecipeStep::TierZero` / `RecipeStep::ActionExecuted` +
`RecipeStage` consumer dispatch** (`recipe.rs`). Add the two variants (carrying
`state`). `RecipeStage::process` branches: `tier0_eligible && !llm_call_required`
→ `RecipeStep::TierZero` (consumes `recipe_hint` + `recipe_rust_context` for the
Phase-H `LoopOrchestratorPort::run_tier_zero`); `llm_call_required` → `RecipeStep::Continue`
(Tier-1; `recipe_hint` stays for `run_step_zero`); else `Continue` (Tier-2).

**H.11 — `canonical.rs` `PostRecipeOutcome` restructuring +
`TierZeroExecutionStage`** (`canonical.rs`). `PostRecipeOutcome` carries the
`RecipeStep` outcome; `DefaultExecutorPipeline::execute_family` handles
`RecipeStep::TierZero` by calling `ctx.host.run_tier_zero(..)` (via
`LoopOrchestratorPort`) → `TierZeroReply` → `AssistantReplyStage` emits the text
directly, no `PromptStage`/`ModelStage`. New `TierZeroExecutionStage` encapsulates
the `run_tier_zero` call + reply handling.

**H.12 — composition `LoopOrchestratorPort` impl + `build_prompt_bundle` reads
`recipe_hint`** (`brassclaw_reborn_composition`). `run_step_zero` → delegates to
engine `assemble_prior_knowledge_with_hint` (H.8) with the stashed `recipe_hint`;
`run_tier_zero` → delegates to engine `execute_tier_zero_channel` (H.8).
`build_prompt_bundle` (the composition `LoopPromptPort` impl) reads
`request.recipe_hint` and prepends the assembled `orchestrator_content` to the
prompt for Tier-1.

**H.13 — Model B/C tests + final verification.** Unit tests for each new stage +
the composition `LoopOrchestratorPort` impl (mock host driving `RecipeStage` →
`TierZeroExecutionStage` → `AssistantReplyStage`). Final verification: fmt +
`cargo clippy --all --benches --tests --examples --all-features -- -D warnings` +
`cargo test` (both configs). Mark the Phase H Zenflow step `9d94d6cb` Completed.
Mark this subplan Zenflow substep Completed.

---

## 5. Verification + status (updated as substeps complete)

- H.0 — Done. This subplan doc written; Zenflow subplan substep created; parent
  Phase H step `9d94d6cb` marked InProgress; `saved_plan_to_v3.md` reference
  blockquote inserted. (Setup only — no production code.)
- H.1 — Done. `_parse_orchestrator_channel_steps(orchestrator_content)` helper in
  `default.py` (helpers region, after `_set_active_skills_from_matched_ids`).
  Parses the `format_orchestrator_content` prose block format back into
  `[{kind, name, body}]`: splits on `"\n\n"`, each block's first line must
  `startswith("## [")` + `endswith("]")`, inner = `first[4:len(first)-1]`,
  `inner.split(": ", 1)` → `kind`/`name` (maxsplit preserves names containing
  `": "`), `body = "\n".join(lines[1:])` (`""` for heading-only). `None`/empty
  → `[]`. Raises `ValueError` on a non-heading first line or missing `": "`
  separator (H.2 catches → `{outcome:"error"}`). Monty-safe (`str.split` w/
  maxsplit, `startswith`, `endswith`, `len`, slicing, `+`/`str()`, for-loop +
  `.append()`, newline-join; NO f-strings/`.format()`/`re`). Monty unit test
  `phase_h1_parse_orchestrator_channel_steps` via `eval_python_int` (single
  block, multi-block, heading-only, empty, None, malformed-heading-raises,
  missing-separator-raises, name-with-`": "`-preserved). Verified:
  `python3 ast.parse` clean; fmt clean; clippy clean both configs; test passes
  both configs (1 passed, 684 default / 695 skills-db filtered — 0 regressions).
  Committed+pushed (this commit).
- H.2 — Done. `execute_recipe_orchestrator_channel(orchestrator_content)` helper
  in `default.py` (helpers region, immediately after
  `_parse_orchestrator_channel_steps`). The Tier-0 no-LLM channel executor.
  Flow: (1) parse via H.1 — a parse failure is caught and →
  `{outcome:"error",message:str(exc)}`; (2) empty steps →
  `{outcome:"error",message:"no orchestrator channel steps to execute"}`; (3) for
  each step, only `kind=="PythonCode"` runs via `__execute_code_step__(body,{})`
  (fresh Monty state per step — ISOLATION INVARIANT; the `{}` kwargs means no
  vars are shared between steps; bodies see only IBS-baked-in `{{vars.slot0}}`
  literals, §0.20.3 — NO runtime `vars` dict). A non-PythonCode `kind` (Skill /
  ToolSkill / unknown) → `{outcome:"error",message:"tier-0 channel step is not
  PythonCode: <kind>"}` and the executor is NOT called for that step; (4) a step
  fails when `__execute_code_step__` raises (hard RuntimeError, caught by
  `except Exception as exc`) OR `step_result.get("had_error")` is truthy
  (internal SyntaxError/runtime error) OR `step_result.get("pending_gate") is
  not None` (Q-H5gate — a gate pause is binary error: degrades to Tier-2 LLM,
  which owns gate handling) →
  `{outcome:"error",message:<reason>}`; on first failure the channel STOPS
  (later steps never run); (5) all-success →
  `{outcome:"success",result:<reply text>}` where the reply text is extracted
  from the LAST step's result (Q-H5result): `final_answer` (from `FINAL("...")`,
  the established RLM reply pattern) → else `return_value` (stringified via
  `str()` if not a `str`) → else captured `stdout` → else `""`. `outcome` is the
  SOLE signal (Q-H4): `"success"` → H.3 returns it as the completed reply (no
  LLM); `"error"` → H.3 falls through to Tier-2 (a Tier-0 failure degrades to a
  normal LLM call so the user still gets a reply). Monty-safe (`str`, `+`, `in`,
  `.get()`, `len()`, `.append()`, `isinstance`, `try/except Exception as exc:
  str(exc)`, `is not None`; NO f-strings/`.format()`/`re`).
  **Signature reconciliation with plan §0.9 / FIND-P9-02:** the plan's pseudocode
  specifies `execute_recipe_orchestrator_channel(pkr, goal, state)`; this
  implementation uses `execute_recipe_orchestrator_channel(orchestrator_content)`
  (single arg). Rationale: `goal` is NOT needed at runtime (the IBS already
  substituted `{{vars.slot0}}` into the bodies before the channel runs,
  §0.20.3 — bodies are self-contained); passing the turn `state` would VIOLATE
  the fresh-state-per-step ISOLATION INVARIANT (sharing turn state across Tier-0
  steps would be a correctness bug). So the subplan signature supersedes the
  plan pseudocode, which predates the grounding that established isolation. The
  plan §0.9 pseudocode is left as-is (it is the design intent); the
  implementation refines the signature for correctness. H.3 calls
  `execute_recipe_orchestrator_channel(pkr["orchestrator_content"])`.
  **Two design questions answered by the user before H.2 implementation
  (extending Q-H4):**
  - **Q-H5gate** — a Tier-0 PythonCode step that pauses on an approval gate
    (`pending_gate` set, not a hard error, not success): per Q-H4's binary
    signal, treat as `{outcome:"error"}` → degrade to Tier-2 LLM (the LLM path
    owns full gate handling; the recipe is recorded as a failure). NOT a third
    outcome; NOT success.
  - **Q-H5result** — the success dict's `result` (used as the chat reply via
    H.3 `response=result`): `final_answer` (FINAL) → else `return_value`
    (stringified if not str) → else `stdout` → else `""`. (Chosen over
    raw-`return_value`-only, which would be Null for the standard FINAL-based
    reply pattern, and over the full step dict, which is not a chat reply.)
  **Tests:** new `run_python_tier0_channel` test harness (sibling of
  `run_python_step0`) drives the helper through Monty with a MOCKED
  `__execute_code_step__` (queued result dicts + optional raise-on-step) so the
  branching/extraction logic is unit-tested without the full engine stack; it
  also returns the `__execute_code_step__` call count. 12 Monty unit tests:
  single-step FINAL success; single-step return_value success; stdout fallback;
  non-str return_value stringified; `had_error` → error; raised RuntimeError →
  error; `pending_gate` → error (degrade); non-PythonCode kind → error +
  executor NOT called (calls==0); empty steps → error + calls==0; malformed
  content → error + calls==0; multi-step last-result wins (calls==2); multi-step
  first-error stops (calls==1). Verified: `python3 ast.parse` clean; `cargo fmt`
  applied; clippy clean both default + skills-db configs; 12 tests pass both
  configs (default: 697 total = 685 + 12; skills-db: 708 total = 696 + 12; 0
  regressions).
- H.3 — Done. `default.py` step-0 `tier_zero` early-return branch wired into
  `run_loop` step-0 inside `if isinstance(pkr, dict):` as a NEW `elif
  pkr.get("tier_zero") and pkr.get("orchestrator_content"):` sibling, placed
  AFTER the `action_short_circuit` block and BEFORE `disambiguation` /
  `override_prompt_creation` / `orchestrator_content` (matching the §0.9 Model
  A ordering: action_short_circuit → tier_zero → disambiguation → override →
  orchestrator_content). The stale "tier_zero dispatch is deferred to Phase H
  (Q-G1)" comment was removed and replaced with the H.3-done explanation.
  Branch flow: on `tier_zero` + non-empty `orchestrator_content` →
  `__emit_event__("recipe_tier_zero_started", recipe=...)` +
  `__transition_to__("running", "recipe tier-0 execution")` + call
  `execute_recipe_orchestrator_channel(pkr.get("orchestrator_content", ""))`
  (H.2, single-arg — see the H.2 signature-reconciliation note); on
  `outcome == "success"` → `__transition_to__("completed", "tier-0 recipe
  executed")` + `return complete_result(state, "completed",
  response=tier0_result.get("result", ""))` (early return, NO
  `__llm_complete__`); on `outcome == "error"` (Q-H4 sole signal) →
  `__emit_event__("recipe_tier_zero_failed", recipe=..., message=...)` +
  `__transition_to__("prompting", "tier-0 recipe failed -> tier-2")` + fall
  through to the Tier-2 `__llm_complete__` path (un-augmented
  `working_messages` — the failed `orchestrator_content` is NOT re-injected as
  prior knowledge; degrade is a PLAIN Tier-2 call, not a Tier-1 guided call,
  exactly mirroring the `action_short_circuit` unresolved fall-through). The
  guard requires BOTH `tier_zero` AND non-empty `orchestrator_content`: a
  `tier_zero` pkr with no channel content has nothing deterministic to run, so
  the elif is skipped and step-0 falls through to a plain Tier-2 LLM call
  (documented "treated as a failure, degrades to Tier-2, NOT skipped"). No
  `else` arm was added (matches the existing if/elif chain shape — an
  unrecognised pkr falls through to Tier-2 by design). Monty-safe (no
  f-strings/`.format()`/`re`). **Tests:** 3 new `run_python_step0` integration
  tests (the harness's 4th arg `code_step_result` mocks `__execute_code_step__`
  for the H.2 channel executor): (1) `phase_h3_tier_zero_success_returns_
  completed_no_llm` — single PythonCode FINAL("hello") success → outcome
  `completed`, response `hello`, `llm_complete_called == false`, events
  include `recipe_tier_zero_started` (not `_failed`), transitions include
  `running`→`completed`; (2) `phase_h3_tier_zero_error_degrades_to_tier2_llm`
  — `had_error` step → `llm_complete_called == true`, response `done` (LLM
  mock, NOT the failed channel), events include both `recipe_tier_zero_started`
  + `recipe_tier_zero_failed`, transition `prompting`, and the failed
  `orchestrator_content` is NOT in the LLM messages; (3)
  `phase_h3_tier_zero_without_orchestrator_content_falls_through` — `tier_zero`
  with no `orchestrator_content` → `llm_complete_called == true`, no
  `recipe_tier_zero_started` event. Added `recording_has_transition` +
  `recording_has_event` helpers. Verified: `python3 ast.parse` clean; fmt
  clean; clippy clean both default + skills-db; 3 tests pass both configs
  (default: 700 total = 697 + 3; skills-db: 711 total = 708 + 3; 0
  regressions).
- H.4 — Done. Spawned the nested subplan `./docs/agents-v3/subplan_problem_stepH4_of_saved_plan_to_v3.md` (Zenflow nested subplan step `fa9fb137`) for `recipe_id` surfacing + the engine→composition Tier-0 outcome-recording bridge. H4.1–H4.7 implemented one-by-one (`b84a6197`→`edc0ab95`): `RecipeTierZeroStarted`/`Succeeded`/`Failed` `EventKind` variants; `handle_emit_event` dispatch arms; `TurnRoutingSignals.recipe_id`+`recipe_name` surfaced end-to-end into the pkr dict; `default.py` tier_zero branch emits the events + the success `tier_zero_outcome` extra stamp; `TierZeroOutcome`+`OrchestratorResult.tier_zero_outcome`+`build_tier_zero_outcome`; composition `RecipeOutcomeListener` (event→`record_recipe_outcome` projection, spy-tested). H4.8 final-verification spawned its OWN nested subplan `./docs/agents-v3/subplan_problem_stepH4_8_of_saved_plan_to_v3.md` (Zenflow substep `d48d5809`) to fix a pre-existing Phase-G.1 test regression (`skill_codeact_persists_active_skill_provenance` red since `e7c2ce31` — `ThreadManager` did not plumb a pg_pool into `ExecutionLoop`); S1–S4 done (`701f4194`, `b01129ef`+`5426cce0`, `ad07906a`, `0f78d80d`) — `ThreadManager::with_pg_pool` plumbing + test migrated to testcontainer-pg + `skills-db` + skip-if-no-docker. Verified: fmt clean; engine clippy `-D warnings` clean both configs; full default `cargo test` GREEN; full `--features skills-db` `cargo test` GREEN. One outside-H4.8 blockage remains (user's uncommitted `basic_prompt_store` WIP blocks the full-workspace `cargo clippy --all ... -D warnings` gate — documented in the H4.8 subplan §7, not H4.8's). Zenflow `fa9fb137` + `d48d5809` marked Completed.
- H.5 — Obsolete/Skipped (O1–O5 done; see `subplan_problem_stepH5_obsolescence_of_saved_plan_to_v3.md`). Model A dormant/never-built; superseded by H.6–H.13 (Model B/C).
- H.6 — Done. Added `crates/brassclaw_turns/src/run_profile/orchestrator_lookup.rs`
  with the two DTOs the plan requires — `PriorKnowledgeBundle` (`orchestrator_content:
  String`, `matched_component_ids: Vec<String>`, `override_prompt_creation: bool`) +
  `TierZeroReply` (`text: String`, `matched_component_ids: Vec<String>`) — both
  `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`, turns-native (no
  engine types), with field docs + a "Reused by Model B/C (H.5 O4)" module-doc note.
  Registered `pub mod orchestrator_lookup;` in `mod.rs` + re-exported the two DTOs.
  Followed the H4 precedent (DTOs in a dedicated `*_lookup.rs`, NOT `host.rs`) over the
  plan-literal `host.rs` placement (Q-H6-1). Verified: clippy `doc_lazy_continuation`
  fix (module-doc leading `+` read as a list marker); fmt clean; clippy clean; 131
  tests GREEN (+2 serde round-trip). Committed `9b460442` + pushed.
- H.7 — Done. (1) `orchestrator_lookup.rs`: added `#[async_trait] pub trait
  OrchestratorLookup: Send + Sync` with `async fn run_step_zero(.., recipe_hint:
  Option<&serde_json::Value>) -> Option<PriorKnowledgeBundle>` + `async fn
  run_tier_zero(.., recipe_hint: &serde_json::Value, recipe_rust_context:
  &serde_json::Value) -> Option<TierZeroReply>` (returns `Option`, degrade-gracefully —
  no error enum, mirroring `RecipeTierZeroFailed`→Tier-2) + trait tests (`StubOrchestrator`
  impl, `StubOrchestratorHost` exposing it, `no_orchestrator_returns_none`, compile-time
  supertrait-reachability proof). (2) `host.rs`: added `pub trait LoopOrchestratorPort:
  Send + Sync { fn orchestrator_lookup(&self) -> Option<&dyn OrchestratorLookup>; }`
  (accessor port, Q-H6-1 H4-precedent) + `pub struct NoOrchestrator; impl
  LoopOrchestratorPort for NoOrchestrator` returning `None` (mirror `NoRetrieval`) +
  added `+ LoopOrchestratorPort` as the **15th supertrait** to BOTH the
  `AgentLoopDriverHost` declaration AND the blanket `impl<T> AgentLoopDriverHost for T
  where ...` where-clause. (3) `LoopPromptBundleRequest`: added `#[serde(default,
  skip_serializing_if = "Option::is_none")] pub recipe_hint: Option<serde_json::Value>`
  + **dropped `Eq`** from the derive (kept `PartialEq` — `serde_json::Value` is not
  `Eq`). (4) `mod.rs`: re-exported `LoopOrchestratorPort`, `NoOrchestrator`,
  `OrchestratorLookup`. (5) Added `recipe_hint: None,` to **all 74**
  `LoopPromptBundleRequest { ... }` construction literals across 14 files (robust
  brace-matching Python script — inserts before the matching close brace, fixing the
  prior off-by-one; +7 sites the original bulk-edit missed in `prompt.rs` +
  `planning_context.rs` + `thread_loop_support_contract.rs` + `agent_loop_host_contract.rs`).
  (6) Added `LoopOrchestratorPort` impls to all 7 `AgentLoopDriverHost` implementors
  (forced by the 15th supertrait, blanket-impl complete): `RebornLoopDriverHost`
  (production — new `orchestrator_lookup` field + `with_orchestrator_lookup` builder on
  the factory + host-build clone + impl, mirroring H4 `retrieval_lookup`; H.12 wires
  the composition engine-backed impl) + `MockHost` (field+impl) + `ResumePayloadHost`
  (delegate to inner) + `StubHost`/`ForbiddenResumeHost`/`RecordingAgentLoopHost`/
  `MockAgentLoopDriverHost` (None impls). The `MockHost` builder is deferred to H.13
  (would be dead-code until tests use it). Verified: fmt clean (incl. the 2 user-WIP
  `loop_driver_host.rs` files — hunk-filtered at staging); clippy clean across turns,
  agent_loop, hooks, loop_support, reborn, composition; tests GREEN (turns 131 /
  agent_loop 355 / reborn 440 / loop_support 2 / hooks).
- H.8 — Done. Nested subplan `subplan_problem_stepH8_of_saved_plan_to_v3.md` (Zenflow nested
  substep `e7fd874b-cc3a-4dd8-a5eb-8fb3f86d301a`, now Completed). 5 design decisions locked by
  the user: Q1=**delete** the dormant Model A PK path (NOT the subplan §4 H.8 line's
  "refactor `handle_assemble_prior_knowledge` to delegate" — deletion supersedes that wording);
  Q2=reduced 9-field `PkrAssemblyResult`; Gap3=complete `execute_tier_zero_channel` signature
  with `llm`+`event_tx` + implement fully (port `_parse_orchestrator_channel_steps` to Rust, run
  PythonCode steps via `execute_code`); `recipe_hint`=Option C (serialized `Vec<ComponentItem>`);
  cleanup=full. Internal substeps H8.1–H8.6 all Done (subplan §5): H8.1 `74f54c6b` (structs),
  H8.2 `6884fea0` (`assemble_prior_knowledge_with_hint`), H8.3 `b55e9102`
  (`execute_tier_zero_channel`), H8.4+H8.4a `53d38fdc` (delete dormant Model A PK path +
  obsolete `active_skills` provenance — nested subplan
  `subplan_problem_stepH8_4_active_skills_obsolescence_of_saved_plan_to_v3.md`), H8.5 `d80f05e4`
  (4 additive G.8 re-home unit tests + `FailingRetrievalSource`), H8.6 `b49c4195` (workspace
  verify-only; the harness `system_bundle_source` gap was completed in the working tree and left
  uncommitted for the user's prefix-cache WIP — on `origin/main` `DefaultPlannedRuntimeParts`
  does not yet have that field). Workspace `cargo check --all-targets` + `cargo clippy
  --workspace --all-targets -- -D warnings` GREEN; engine tests GREEN both configs (590 default
  / 601 skills-db). The H8.2/H8.3 `pub` fns are re-exported from `executor/mod.rs` for H.12 to
  consume. **Phase H.8 complete; H.9 is next.**
- H.9 — Done. `state.rs` (`brassclaw_agent_loop`): replaced the E.0 interim
  `last_retrieval_result: Option<RetrievalTurnResult>` field with the plan-literal
  split — `#[serde(default)] pub recipe_hint: Option<serde_json::Value>` (the
  `orchestrator_items`) + `#[serde(default)] pub recipe_rust_context:
  Vec<serde_json::Value>` (the `rust_items` array). **Q-H9-1 = clean-migrate**
  (remove `last_retrieval_result`, do NOT keep it alongside — it was E.0's
  temporary intermediate, and the plan target-state lists only `last_user_text` /
  `recipe_hint` / `recipe_rust_context`); **Q-H9-2 = `Vec<serde_json::Value>`**
  for `recipe_rust_context` (plan code-block line 5993 + SEC-02 `vec![]` shorthand
  win over the prose "typed as `serde_json::Value`" — accepted the lossy
  `as_array().cloned().unwrap_or_default()` at produce + `Value::Array(vec.clone())`
  conversion at the H.11/H.12 consume). `initial_for_run` sets `None` / `Vec::new()`;
  dropped the now-unused `RetrievalTurnResult` import; updated the `Eq`-not-derived
  doc comment to reference `recipe_hint` / `recipe_rust_context`. `executor/recipe.rs`:
  `RecipeStage::process` clears `recipe_hint` / `recipe_rust_context` at the TOP of
  every call (SEC-02 — stale-stash replay guard for checkpoint-resumed turns) before
  the lookup/user_text guard; on `Ok(Some(result))` stashes `recipe_hint =
  Some(result.orchestrator_items.clone())` + `recipe_rust_context =
  result.rust_items.as_array().cloned().unwrap_or_default()` (mirroring the H.7
  `OrchestratorLookup` trait's `serde_json::Value` consume shape). The routing booleans
  (`tier0_eligible` / `llm_call_required`) are NOT stashed — H.10 branches on them
  inline. `Ok(None)` / `Err` leave the stash empty (SEC-02 clear already ran). Still
  returns `RecipeStep::Continue` (H.9 is producer-only; consumer dispatch is H.10).
  Module doc E.0 → H.9. `executor/tests.rs`: migrated the 5 `RecipeStage` integration
  assertions (success case asserts the exact plan-literal split derived from
  `expected`; the 4 fall-through cases assert `recipe_hint.is_none()` +
  `recipe_rust_context.is_empty()`); migrated the `state.rs` round-trip test to
  `last_user_text_and_recipe_hint_and_recipe_rust_context_round_trip_through_json`.
  Doc-comment-only updates in `orchestrator.rs` (`brassclaw_engine`) +
  `retrieval_lookup.rs` (`brassclaw_turns`) — the routing notes + field docs now
  reference `recipe_hint` / `recipe_rust_context` (H.9) + inline `RecipeStage`
  branching (H.10) instead of the removed `last_retrieval_result`. Verified:
  `cargo clippy -p brassclaw_agent_loop --all-targets -- -D warnings` GREEN;
  `cargo test -p brassclaw_agent_loop` full suite GREEN (6 migrated recipe tests
  confirmed: 5 `recipe_stage_*` + the state round-trip); `cargo clippy -p
  brassclaw_engine -- -D warnings` GREEN. Committed+pushed `2cd8eaec`. **H.10 is
  next.**
- H.10 — Pending.
- H.11 — Pending.
- H.12 — Pending.
- H.13 — Pending.
