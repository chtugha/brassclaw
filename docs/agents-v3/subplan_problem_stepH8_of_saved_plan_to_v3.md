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
- H8.2 — Done. Implemented `pub async fn assemble_prior_knowledge_with_hint`
  (Option C, user lock) in `orchestrator.rs`: Some-branch deserializes
  `Vec<ComponentItem>` from `recipe_hint` (map_err → `EngineError::InvalidInput`
  on deser fail) → `assemble_pkr_from_items`; None-branch builds `ComponentScope`
  from `thread` (tenant_id/user_id/agent_id/project_id) → `source.fetch_for_turn`
  → `assemble_pkr_from_fetch`, with no-source / fetch-error →
  `empty_pkr_assembly_result()` (degrade). Three helpers: `assemble_pkr_from_items`
  (Solution Override detect — exactly 1 `override_prompt_creation` item → verbatim
  body + override=true; else `format_orchestrator_content` + full id list, routing
  fields default false), `assemble_pkr_from_fetch` (faithful projection of every
  `FetchForTurnResult` arm: Components / Disambiguation / ActionShortCircuit /
  SplitResult with `tier_zero = !routing.llm_call_required`), and
  `empty_pkr_assembly_result` (all-empty / all-false). Re-exported
  `assemble_prior_knowledge_with_hint` from `executor/mod.rs` (plan §5877) so
  `brassclaw_reborn_composition` can import it. No unwrap/expect. Verified:
  `cargo check` + `cargo clippy --all-targets -- -D warnings` clean both configs
  (default + `--features skills-db`); `cargo test -p brassclaw_engine` GREEN
  (599 default / 610 skills-db); staged-only state re-verified clean. Committed
  `6884fea0`, pushed to `origin/main`.
- H8.3 — Done. Implemented `pub async fn execute_tier_zero_channel`
  (locked Gap3 signature, 9 args, `#[allow(clippy::too_many_arguments)]` per the
  5x existing orchestrator.rs precedent) in `orchestrator.rs`: parses
  `orchestrator_content` via the Rust port `parse_orchestrator_channel_steps`
  → `Vec<OrchestratorChannelStep> {kind, name, body}` (the port of Python
  `_parse_orchestrator_channel_steps`; `EngineError::InvalidInput` on malformed
  heading / missing `: ` separator); only `kind == "PythonCode"` steps run via
  `execute_code` with a FRESH `ThreadExecutionContext` per step,
  `persisted_state = {}`, `capability_policies = &[]` (ISOLATION invariant);
  first failure (`failure.is_some()` / `need_approval.is_some()` gate pause /
  `execute_code` `Err`) or parse failure or empty step list or non-PythonCode
  step → `empty_tier_zero_channel_result()` degrade (mirrors Python
  `outcome:"error"` → Tier-2). All-success → reply text from the last step via
  `extract_tier_zero_reply_text` (`final_answer` → `return_value` stringified
  → `stdout` → `""`; JSON `null` return = no return value). Step events
  broadcast via `event_tx` (mirror `handle_execute_code_step`); `thread: &Thread`
  so events NOT pushed to `thread.events` (agent-loop owns its state). Two
  forced-by-signature points documented prominently: (1) `matched_component_ids`
  returned empty — locked signature has no identity arg + `orchestrator_content`
  is prose names (not UUIDs); composition H.12 supplies
  `TierZeroReply.matched_component_ids` from stashed `recipe_hint`; (2)
  `rust_context` reserved (`_rust_context`, unused) for the future ToolSkill-
  binding pre-load (plan TIER0-GAP step 1); not consumed in the per-step loop
  (ISOLATION + Python reference `__execute_code_step__(body, {})` passes no
  rust_context); wired when `recipe_rust_context` gains a producer (H.9/H.10).
  Re-exported `execute_tier_zero_channel` from `executor/mod.rs` (plan §5877).
  Verified: `cargo check` + `cargo clippy --all-targets -- -D warnings` clean
  both configs (default + `--features skills-db`); `cargo test -p
  brassclaw_engine` GREEN (599 default / 610 skills-db); staged-only state
  re-verified clean. Committed `b55e9102`, pushed to `origin/main`.
- H8.4 — Done. Deleted the dormant Model A prior-knowledge path:
  `handle_assemble_prior_knowledge` + the `__assemble_prior_knowledge__`
  dispatch arm + `assemble_from_component_items` + `assemble_component_strings`
  + `skill_provenance_for_items` + `format_prior_knowledge_for_llm` +
  `doc_type_class_code` + `CLASS_CODE_*` consts (orchestrator.rs 9922→8566,
  −1356). Removed the `default.py` step-0 PK block (`pkr = __assemble_prior_
  knowledge__(...)` + the `if isinstance(pkr, dict):` branches +
  `_set_active_skills_from_matched_ids` line). Rewrote `phase_f7` tests #1–#5/#7
  to call `assemble_prior_knowledge_with_hint` and assert on `PkrAssemblyResult`
  fields; repurposed+renamed #2. Updated doc comments referencing the retired
  symbols in 8 files (orchestrator.rs module doc, loop_engine.rs
  `retrieval_source` field+builder docs, instruction_builder.rs, manager.rs,
  retrieval_source.rs, db_skill_loader.rs, thread.rs, lib.rs). Verified:
  `cargo check` + `cargo clippy -p brassclaw_engine --all-targets -- -D
  warnings` clean both configs (default + `--features skills-db`); `cargo test
  -p brassclaw_engine` GREEN (587 default / 598 skills-db, 0 failed). H8.4a
  nested substep folded into this commit (see below).
- H8.4a — Done (nested; see
  `subplan_problem_stepH8_4_active_skills_obsolescence_of_saved_plan_to_v3.md`).
  User decision Q-active_skills=B (obsolete, superseded by the orchestrated
  Sempai validation system): deleted the entire dormant `active_skills`
  provenance mechanism — D1 `__set_active_skills__` dispatch arm, D2
  `skill_activated` event match arm, D3 `handle_set_active_skills` fn
  (orchestrator.rs 8566→8526, −40); D5 `ActiveSkillProvenance` struct + D6
  `ACTIVE_SKILLS_METADATA_KEY` const + D7 `set_active_skills`/`active_skills`
  fns + D8 roundtrip test (thread.rs 702→641, −61); D10 `fetch_skill_provenance_
  by_ids` fn (db_skill_loader.rs 442→360, −82); D12 `_set_active_skills_from_
  matched_ids` helper (default.py 1670→1637, −33); D14 `pg_rig` module + D15
  `skill_codeact_persists_active_skill_provenance` docker test
  (engine_v2_skill_codeact.rs 1213→1003, −210); D13 `mission_skill_repair.md`
  prompt (rm). Surgical edits D4/D9/D11 (imports + re-exports) + DC1/DC2
  (retrieval_source.rs doc comments → orchestrator-channel identity set).
  Two orphaned-`DocId`-import cascades fixed (thread.rs top-level + test-module).
  Deferred cascades DOCUMENTED not deleted: `SkillActivated` pub event variant
  in `brassclaw_common` (serialization-breaking) + `test_skill_oauth_flow.py`
  e2e assertion (separate OAuth scenario). Verified: `cargo clippy
  --all-targets` clean both configs; `cargo test` GREEN (586 default / 597
  skills-db, 0 failed — the 1 deleted docker-gated test that skipped locally is
  gone as expected). Committed with H8.4.
- H8.5 — Done. The H8.4 commit already deleted the obsolete tests
  (`assemble_prior_knowledge_returns_both_surfaces`, `format_prior_knowledge_
  raw_and_formatted_are_structurally_distinct`, the `format_prior_knowledge_
  for_llm` test block, `phase_g1_active_skills_emitted_in_every_arm`), the
  `run_python_step0` helper + its 6 G.8 `step0_*` tests, and the
  `Step0Recording`/`kwargs_to_json`/`class_code_arg` helpers — AND rewrote
  `phase_f7` #1–#5/#7 to call `assemble_prior_knowledge_with_hint` and assert
  on `PkrAssemblyResult` fields (re-homing the routing assertions:
  `action_short_circuit`, `disambiguation`, `orchestrator_content` prose
  format, Components arm, SplitResult, retrieve_docs flat list, scope
  tenant/agent). User decision Q-H8.5=A1: H8.5's additive work = fill only the
  genuine gaps as new Rust unit tests on the H8.2/H8.3 fns (orchestrator.rs
  `mod tests`, after the phase_f7 group): (1) `phase_h8_5_solution_override_
  assembles_verbatim_single_item` — the `assemble_pkr_from_items` Solution
  Override arm (exactly 1 `override_prompt_creation` item → verbatim
  `effective_content` + `override_prompt_creation=true` + single-item identity
  set), exercised via the `recipe_hint` Some-branch; (2) `phase_h8_5_split_
  result_tier_zero_inverts_llm_call_required` — the `tier_zero` flag on the
  SplitResult arm (`!routing.llm_call_required`), both polarities
  (`llm_call_required=true⇒tier_zero=false`, `=false⇒tier_zero=true`) directly
  on `assemble_pkr_from_fetch`; (3) `phase_h8_5_no_source_or_failing_source_
  degrades_to_empty_pkr` — the None-branch degrade (`retrieval_source=None` +
  a `FailingRetrievalSource` whose `fetch_for_turn` errors → both return
  `empty_pkr_assembly_result()`); (4) `phase_h8_5_recipe_hint_some_branch_
  assembles_stashed_items_without_refetch` — the Some-branch assembles stashed
  items via `assemble_pkr_from_items` WITHOUT re-fetching (proven by passing a
  `FailingRetrievalSource`: any re-fetch would degrade to empty). Added the
  `FailingRetrievalSource` test helper. User decision Q-H8.5=B1: the G.8
  "injection" assertions that `assemble_prior_knowledge_with_hint` structurally
  cannot test (N-1 prompt injection, `__llm_complete__` fall-through,
  action-procedure execution, events/transitions, outcome shaping) are
  agent-loop/integration concerns for a FUTURE composition H.12 / agent-loop
  integration-test tier and are NOT re-homed here; the "legacy-shims-not-called"
  + "active_skills forwarded" ones are structurally moot (both deleted in
  H8.4/H8.4a). Verified: `cargo clippy -p brassclaw_engine --all-targets --
  -D warnings` clean both configs (default + `--features skills-db`); `cargo
  test -p brassclaw_engine` GREEN (+4 tests vs H8.4a). Committed `53d38fdc`.
- H8.6 — Done. Final verify + ship. `cargo check --workspace --all-targets`
  GREEN: every library crate compiles clean — including
  `brassclaw_reborn_composition` (which imports the H8.2/H8.3 re-exports
  `assemble_prior_knowledge_with_hint` / `execute_tier_zero_channel` from
  `executor/mod.rs`) and `brassclaw_engine`. So the H8.1–H8.5 re-exports +
  deletions (`ActiveSkillProvenance` from `lib.rs`, `fetch_skill_provenance_
  by_ids` from the `db_skill_loader` re-export, the retired Model A PK fns)
  cause **zero** downstream breakage. `cargo clippy --workspace --all-targets
  -- -D warnings` GREEN (whole workspace, incl. the user's prefix-cache WIP +
  the harness completion below). `cargo clippy -p brassclaw_engine
  --all-targets -- -D warnings` clean both configs (default + `--features
  skills-db`); `cargo test -p brassclaw_engine` GREEN both configs (590
  default / 601 skills-db, 0 failed). `cargo fmt` clean.
  //
  Harness completion (user decision Q-H8.6: "you fix the harness!", revising
  the earlier C): the first workspace `--all-targets` check failed on ONE root
  error — `error[E0063]: missing field 'system_bundle_source' in initializer
  of DefaultPlannedRuntimeParts` at `tests/support/reborn/harness.rs:857`. This
  was pre-existing, incomplete, **working-tree-only user prefix-cache WIP**
  (`system_bundle_source` is not on HEAD): the user added the
  `system_bundle_source: Option<Arc<dyn SystemBundleSource>>` field to
  `DefaultPlannedRuntimeParts` across the stack and updated 8 test call sites
  to set `system_bundle_source: None,`, but missed this harness. H.8 completed
  that one spot — added `system_bundle_source: None,` after
  `skill_context_source: None,` (mirroring the user's 8 other call sites). The
  harness fix is **left UNCOMMITTED in the working tree**: on `origin/main`
  `DefaultPlannedRuntimeParts` does NOT yet have the `system_bundle_source`
  field (it is part of the user's uncommitted prefix-cache WIP), so committing
  the harness fix alone would break main (referencing a non-existent field).
  The user must commit the harness fix together with their prefix-cache WIP
  (which defines the field). H.8 did NOT sweep the rest of the user's
  prefix-cache WIP into a commit (the `001dbee7` lesson).
  //
  H8.1–H8.5 committed: `74f54c6b`, `6884fea0`, `b55e9102`, `53d38fdc`
  (H8.4+H8.4a), `d80f05e4` (H8.5). H8.6 is verify-only (no H.8 code changes);
  its subplan §5 update is committed separately. **Phase H.8 complete.**
