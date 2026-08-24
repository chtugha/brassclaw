# Subplan — Phase H.8: extract engine orchestrator `pub` fns + delete dormant Model A prior-knowledge path

Parent plan: `saved_plan_to_v3.md` → Phase H.8 (lines ~5786–5870) + FIND-NEW-PASS12-01/02 +
FIND-NEW-PASS13-01.
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase H step `9d94d6cb-3a45-47cc-8e0f-85203c936652`,
immediately after it. Sibling of `subplan_problem_stepH_of_saved_plan_to_v3.md` (the Phase H
umbrella) and `subplan_problem_stepH5_obsolescence_of_saved_plan_to_v3.md` (the O1–O5 cleanup
that preceded this).

---

## 1. Why this subplan exists

Phase H.6 added the two turns-native DTOs (`PriorKnowledgeBundle` + `TierZeroReply`) and Phase
H.7 added the `OrchestratorLookup` trait + `LoopOrchestratorPort` accessor + the
`recipe_hint` field on `LoopPromptBundleRequest`. That plumbing has **no engine backing yet**:
the composition `OrchestratorLookup` impl (H.12) must call two engine `pub` fns that do not
exist. H.8 extracts them.

Concurrently, the user's **Q1=delete** decision (locked before this subplan) retires the
**dormant Model A prior-knowledge path**: the private Monty handler
`handle_assemble_prior_knowledge`, its `__assemble_prior_knowledge__` dispatch arm, and the
Python `pkr = __assemble_prior_knowledge__(...)` call in `default.py` step-0. Model A is
dormant (production turns run on the agent loop; confirmed in H.5 O3 —
`brassclaw_reborn/src` non-test + `brassclaw_product_workflow` have zero refs to
`execute_orchestrator`/`ExecutionLoop`). The prior-knowledge assembly moves entirely to the
Model B/C Rust path (`assemble_prior_knowledge_with_hint`, called by the composition
`OrchestratorLookup::run_step_zero`).

Q1=delete cascades: removing the Python call makes the rest of the `default.py` step-0
prior-knowledge block dead (`pkr` undefined), which obsoletes the 7 G.8 `run_python_step0`
harness tests, and deleting the handler renders six helpers + the legacy MemoryDoc JSON
formatter dead. This subplan records the full cascade and the obsolescence-clean resolution
(delete the dead code; re-home the behavioural assertions as Rust unit tests on the new `pub`
fn — behavior moves with the code to its new Rust home, nothing silenced).

## 2. Design decisions (locked by the user before implementation)

- **Q1 (handler fate) = DELETE.** Delete `handle_assemble_prior_knowledge` + the
  `__assemble_prior_knowledge__` dispatch arm + the Python `__assemble_prior_knowledge__`
  call + the now-dead `default.py` step-0 PK block. Extract the `pub` fn for Model B/C only.
- **Q2 (`PkrAssemblyResult` shape) = reduced 9-field struct (plan-literal).** Fields:
  `orchestrator_content: String`, `matched_component_ids: Vec<String>`,
  `override_prompt_creation: bool`, `action_short_circuit: bool`,
  `action_component_id: Option<String>`, `action_name: Option<String>`,
  `disambiguation: bool`, `candidates: Vec<serde_json::Value>`, `tier_zero: bool`.
  Routing for Model B/C comes from `RetrievalTurnResult` (H.4), **not** from this struct —
  the `action_*`/`disambiguation`/`tier_zero` fields are vestigial under Q2 (kept because the
  plan-literal 9 fields were locked).
- **Gap3 (`execute_tier_zero_channel` signature) = complete it.** The plan signature omits
  `llm` + `event_tx`; both are required because the fn runs each PythonCode step via
  `execute_code` (which needs `llm`) and broadcasts step events (which needs `event_tx`),
  mirroring `handle_execute_code_step` (orchestrator.rs:1084). Final signature:
  `execute_tier_zero_channel(thread, orchestrator_content, rust_context, effects, leases,
  policy, gate_controller, llm, event_tx) -> Result<TierZeroChannelResult, EngineError>`.
- **`recipe_hint` shape = Option C.** `recipe_hint: Option<serde_json::Value>`; `Some(v)` →
  `v` is a serialized `Vec<ComponentItem>` (the orchestrator-channel items RecipeStage
  stashed). `None` → no stash, fresh `retrieval_source.fetch_for_turn(...)`.
  - **Some-branch** (Tier-1 assemble-only, no second fetch): deserialize `Vec<ComponentItem>`
    → `format_orchestrator_content` → `orchestrator_content`; `matched_component_ids` from
    item ids; **Solution Override** detected from `item.override_prompt_creation` (a real
    `ComponentItem` field) — exactly one override item → verbatim body + `override=true`;
    `action_*`/`disambiguation`/`tier_zero` default false (those route upstream via
    `run_tier_zero`/disambiguation UX, never reach `run_step_zero`). Defaults are **correct**,
    not a stub — `run_step_zero` (Tier 1) is only called for the assemble case.
  - **None-branch** (fresh retrieval, full arm logic): `fetch_for_turn` → match on
    `FetchForTurnResult::{Components, Disambiguation, ActionShortCircuit, SplitResult}` →
    `PkrAssemblyResult`. No source / error → empty result (degrade).
- **Cleanup = full.** Remove the whole `default.py` step-0 PK block; delete `run_python_step0`
  + its G.8 tests; re-home their routing/injection assertions as Rust unit tests on
  `assemble_prior_knowledge_with_hint`. No stubs/placeholders.

## 3. Grounding (confirmed against current code at HEAD `911fe52e`)

- `handle_assemble_prior_knowledge` — orchestrator.rs:2718–2926, private, dispatched at
  :788–798. RetrievalSource path (Components/Disambiguation/ActionShortCircuit/SplitResult
  arms) + legacy `RetrievalEngine::retrieve_context` fallback.
- Helpers that become dead under Q1+Q2 (verified single-caller):
  - `assemble_from_component_items` (:3195) — only caller :2766. Returns `ExtFunctionResult`.
  - `assemble_component_strings` (:3252) — only caller :3230. Produces raw `content` (NOT in
    the reduced 9-field struct) + matched_ids.
  - `skill_provenance_for_items` (:3149, `#[cfg(feature="skills-db")]`) — only callers
    :2763/:2847. Produces `active_skills` (NOT in the reduced struct).
  - `format_prior_knowledge_for_llm` (:3319, `pub(crate)`) — only production caller :2910
    (the legacy fallback). Plus its own unit tests (:8631–8715, :8822). Crate-private → no
    out-of-crate callers (grep-verified).
  - `doc_type_class_code` (:3288) + `CLASS_CODE_*` consts (:3276–3284) — only caller is the
    formatter (:3326) + the consts serve `doc_type_class_code`.
- `format_orchestrator_content` (:3127) + `step_context_label` (:3109) — KEPT (the new fn
  uses them to produce `orchestrator_content`).
- `execute_code` — scripting.rs:519, `pub async fn`, signature
  `(code, thread: &Thread, llm, effects, leases, policy, context: &ThreadExecutionContext,
  capability_policies: &[PolicyRule], persisted_state: &serde_json::Value) -> Result<
  CodeExecutionResult, EngineError>`. `CodeExecutionResult` (scripting.rs:230) carries
  `return_value`, `stdout`, `final_answer: Option<String>`, `failure: Option<…>` (replaces
  former `had_error`), `need_approval: Option<ThreadOutcome>` (gate pause), `events`.
- `thread_execution_context` — thread_context.rs:18, `pub(crate)`.
- Python reference for `execute_tier_zero_channel` — `execute_recipe_orchestrator_channel`
  (default.py:1081–1189) + `_parse_orchestrator_channel_steps` (default.py:1024–1078).
  KEPT as spec/reference (deleted in the final cleanup after H.8 per its own docstring).
- G.8 `run_python_step0` harness — orchestrator.rs:4932 + `Step0Recording`/`kwargs_to_json`/
  `class_code_arg` helpers (:4879–4925); used by 6 step-0 tests (:5146, :5207, :5267, :5313,
  :5372, :5452). All assert pkr-driven step-0 behaviour → obsolete under Q1.
- Direct `handle_assemble_prior_knowledge` unit tests — `assemble_prior_knowledge_returns_
  both_surfaces` (:8771, exercises the legacy MemoryDoc fallback), `phase_f7_assemble` helper
  (:8942) + #1–#5/#7 (:8992/:9043/:9063/:9095/:9141/:9361), `phase_g1_active_skills_
  emitted_in_every_arm` (:9191, asserts `active_skills` — not in the reduced struct).
- `ComponentItem` + `IntentCandidate` derive `Serialize`/`Deserialize` (retrieval_source.rs:38,
  intent_system.rs:152). `FetchForTurnResult`/`TurnRoutingSignals` derive only `Debug`/`Clone`
  (not needed under Option C — the fn consumes `Vec<ComponentItem>` + the in-memory
  `FetchForTurnResult` arms, no extra serde derives).
- Re-export point — `executor/mod.rs` has `pub mod orchestrator;` + `pub use loop_engine::
  ExecutionLoop;`; add `pub use orchestrator::{assemble_prior_knowledge_with_hint,
  execute_tier_zero_channel, PkrAssemblyResult, TierZeroChannelResult};`.

## 4. Substep sequence (each committed + pushed individually before the next begins)

- **H8.1** — Add `PkrAssemblyResult` + `TierZeroChannelResult` `pub struct`s to
  `orchestrator.rs` (9-field + 2-field per §2); re-export from `executor/mod.rs`. No logic.
  Verify: `cargo check -p brassclaw_engine` (default + `--features skills-db`).
- **H8.2** — Implement `assemble_prior_knowledge_with_hint` `pub async fn` per §2 Option C.
  Extract the arm logic from `handle_assemble_prior_knowledge` into a shared
  `fn assemble_pkr_from_fetch(result: FetchForTurnResult) -> PkrAssemblyResult` (used by the
  None-branch) + `fn assemble_pkr_from_items(items: &[ComponentItem]) -> PkrAssemblyResult`
  (override-detect + format + matched_ids + routing-defaults-false; used by the Some-branch
  AND the Components arm). `format_orchestrator_content` reused. Verify: `cargo check` both
  configs.
- **H8.3** — Implement `execute_tier_zero_channel` `pub async fn` per §2 Gap3. Port
  `_parse_orchestrator_channel_steps` to Rust (`parse_orchestrator_channel_steps(content:
  &str) -> Result<Vec<OrchestratorChannelStep>, EngineError>`); run each `PythonCode` step via
  `execute_code` (fresh `ThreadExecutionContext` per step, `persisted_state = {}`,
  `capability_policies = &[]`); first failure (`failure.is_some()` / `need_approval.is_some()`
  gate pause) → degrade to empty `TierZeroChannelResult` (mirroring the Python
  `outcome:"error"`→Tier-2 degradation); all-success → reply text from the last step
  (`final_answer` → `return_value` stringified → `stdout` → `""`). Broadcast step events via
  `event_tx` (mirror `handle_execute_code_step`). Verify: `cargo check` both configs.
- **H8.4** — Delete the dormant Model A PK path: `handle_assemble_prior_knowledge` + the
  `__assemble_prior_knowledge__` dispatch arm (:788–798) + `assemble_from_component_items` +
  `assemble_component_strings` + `skill_provenance_for_items` + `format_prior_knowledge_for_llm`
  + `doc_type_class_code` + `CLASS_CODE_*` consts. Remove the `default.py` step-0 PK block
  (the `pkr = __assemble_prior_knowledge__(...)` call + the `if isinstance(pkr, dict):`
  branches + `_set_active_skills_from_matched_ids` line; keep `insert_volatile_context_at_
  n_minus_1` no-op only if still used elsewhere — verify). Update doc comments referencing
  `__assemble_prior_knowledge__` (orchestrator.rs:18 module doc, loop_engine.rs:134/218
  `retrieval_source` field docs). Verify: `cargo check` both configs.
- **H8.5** — Test fallout: delete obsolete tests (`assemble_prior_knowledge_returns_both_
  surfaces` :8771, `format_prior_knowledge_raw_and_formatted_are_structurally_distinct` :8822,
  the `format_prior_knowledge_for_llm` test block :8631–8715, `phase_g1_active_skills_
  emitted_in_every_arm` :9191, `run_python_step0` + its 6 G.8 tests + the
  `Step0Recording`/`kwargs_to_json`/`class_code_arg` helpers if now-unused). Rewrite phase_f7
  #1–#5/#7 to call `assemble_prior_knowledge_with_hint` and assert on `PkrAssemblyResult`
  fields. Re-home the G.8 routing/injection assertions (orchestrator_content injected at N-1,
  action_short_circuit, disambiguation, override, legacy-shims-not-called) as Rust unit tests
  on `assemble_prior_knowledge_with_hint`. Verify: `cargo test -p brassclaw_engine` both
  configs GREEN.
- **H8.6** — Final verify + ship: `cargo fmt`; `cargo clippy -p brassclaw_engine --all-
  targets -- -D warnings` (default + `--features skills-db`); `cargo test -p brassclaw_engine`
  both configs; `cargo check --workspace --all-targets` (no downstream breakage from the
  re-exports / deletions). Selective-stage (the user's concurrent prefix-cache WIP is
  co-mingled in `loop_driver_host.rs` etc. — hunk-filter to stage only H.8 files). Commit +
  push to `origin/main`. Mark the Zenflow H.8 substep + the Phase H subplan §5 H.8 entry
  Done.

## 5. Status tracker

- H8.1 — Done. Added `PkrAssemblyResult` (9-field) + `TierZeroChannelResult`
  (2-field) `pub struct`s to `orchestrator.rs` in a dedicated Phase-H.8 section
  (after `format_orchestrator_content`), both `#[derive(Debug, Clone, PartialEq)]`,
  with field-level docs recording the Q2 vestigial-routing note. Re-exported both
  from `executor/mod.rs` (`pub use orchestrator::{PkrAssemblyResult,
  TierZeroChannelResult};`). No logic. Verified: `cargo check -p brassclaw_engine`
  clean both configs (default + `--features skills-db`); `cargo fmt` clean.
- H8.2 — Pending.
- H8.3 — Pending.
- H8.4 — Pending.
- H8.5 — Pending.
- H8.6 — Pending.
