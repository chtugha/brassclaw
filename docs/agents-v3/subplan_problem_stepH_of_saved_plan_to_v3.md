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
not surfaced today. Approach: extend `OrchestratorResult` (`orchestrator.rs:64`)
to carry `tier_zero_outcome: Option<TierZeroOutcome>` where
`TierZeroOutcome{recipe_id: String, success: bool}`; the engine populates it
when `default.py` returns a Tier-0 `completed` (recipe_id from the matched
recipe — surfaced from `handle_assemble_prior_knowledge`'s `SplitResult` arm
which has `routing.matched_component_ids`; the *recipe* id is the recipe
component itself, available in the `SplitResult` via the routing/recipe row —
verify the exact source during H.4; if it needs DB re-fetch or a new field on
`TurnRoutingSignals`, **write a nested subplan
`subplan_problem_stepH4_of_saved_plan_to_v3.md`** and execute it before
resuming H.4). Composition's `loop_engine` caller reads
`result.tier_zero_outcome` and calls `RecipeLookup::record_recipe_outcome`. If
the plumbing touches `loop_engine.rs` / `manager.rs` widely, do it via the nested
subplan. `record_recipe_outcome` is a fire-and-forget best-effort call (errors
logged at `debug!`, never break the turn).

**H.5 — Model A composition integration test.** Tier-0 recipe in the DB:
`wilson_lower >= 0.70`, `validation_status = 'validated'`, `llm_call_required =
false`, a `step_link` whose IBS compiles to a single PythonCode step whose body
returns a constant. Drive `execute_orchestrator` (via the composition
`ThreadManager` / `loop_engine` test path) → assert: orchestrator runs the
channel, NO LLM call made, returns `completed` with the PythonCode result, and
`record_recipe_outcome(recipe_id, true)` was invoked (verify via a test
recorder / spy on the `RecipeLookup` impl). Skip-if-no-docker (real Postgres).
This is the Model A acceptance test.

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
- H.2 — Pending.
- H.3 — Pending.
- H.4 — Pending (may spawn nested subplan `subplan_problem_stepH4_of_saved_plan_to_v3.md`
  if `recipe_id` surfacing proves large).
- H.5 — Pending.
- H.6 — Pending.
- H.7 — Pending.
- H.8 — Pending.
- H.9 — Pending.
- H.10 — Pending.
- H.11 — Pending.
- H.12 — Pending.
- H.13 — Pending.
