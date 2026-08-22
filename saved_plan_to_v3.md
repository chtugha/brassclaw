# Recipe System Finalisation Plan — v3

> **Status:** Draft — for review before implementation begins.
> **Scope:** Closes all architectural gaps identified in the Vision vs. Implementation analysis.
> **No code changes are made by this document.**
> **Next migration:** V050 (current highest: V049 — V047–V049 were used for LLM provider and session migrations unrelated to this plan).
>
> **⚠️ MIGRATION NUMBER CORRECTION (two rounds):**
>
> **Round 1:** The original plan assumed V047 as the next migration.
> Three migrations were added since: `V047__llm_providers_is_builtin.sql`,
> `V048__seed_builtin_providers.sql`, `V049__session_threads_version.sql`.
>
> **Round 2 (Decision 2):** `reborn_validation_queue` table creation was split out of
> Phase N and inserted as **V051 (Phase A.5)** so the queue exists from day one.
> This shifts every subsequent migration by +1.
>
> | Original | Round 1 | Round 2 (final) | Phase |
> |----------|---------|-----------------|-------|
> | V047 | V050 | V050 | Phase A |
> | *(new)* | *(new)* | **V051** | **Phase A.5** (Decision 2 — queue table) |
> | V048 | V051 | V052 | Phase B |
> | V049 | V052 | V053 | Phase C |
> | V050 | V053 | V054 | Phase D |
> | V051 | V054 | V055 | Phase J |
> | V052 | V055 | V056 | Phase K.1 |
> | V053 | V056 | V057 | Phase L.1 |
> | V054 | V057 | V058 | Phase M.1 |
> | V055 | V058 | **V059** | Phase N.1 (populate + DROP only) |

---

## Working Rules

**Critical constraints for implementation:**
- Do implementations **one by one**, never batch or parallelize.
- After each phase is fully resolved: mark it **[DONE]** in this plan, commit + push to `origin/main`, then continue.
- Address everything encountered, even if out of scope or pre-existing — **never suppress/silence**.
- If a stub needs replacement: implement fully, trace the logic to all impacted locations **before** changing code.
- If a fix is complex: write a `subplan_stepX_of_v3.md` or `plan_stub_stepX_v3.md`, execute it, then resume the original step.
- Never delete upgrades found that aren't in the plan — **document and repair** them instead.
- **Do NOT run `git stash`** — commit everything, always.

---

## Codebase Audit Pass — Corrections Applied (Review Passes 2 + 3 + 5 + 7 + 8 + 9 + 10 + 12 + 13 + 14; all inline fixes applied)

The following issues were found by reading the live codebase and corrected directly in this plan. Each is tagged with a marker so implementers can grep for them.

### Pass 10 findings (full plan + complete codebase re-read — new issues found)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-P10-01` | **CRITICAL** | Phase D / `resolve_intent` SELECT list | Phase D adds `step_link: Option<String>` and `component_name: String` to `IntentResolution::Match`. But the live `resolve_intent` query at `intent_system.rs:340-356` only selects **5 columns**: `id, component_id, component_class_code, input_class, score`. The Phase D SQL change is specified as adding `step_link` and `COALESCE(a.name,'') AS component_name` — this is correct in prose (§0.8) but the plan never gives the exact new column positions for the `row.get(N)` calls in `IntentResolution::Match` construction. With the LEFT JOIN and two extra columns, the `rows[0].get(N)` indices for `component_id`, `component_class_code`, `step_link`, and `component_name` must be specified exactly. Old layout: `row.get(1)=component_id, row.get(2)=component_class_code`. New layout: `row.get(1)=component_id, row.get(2)=component_class_code, row.get(5)=step_link` (after keeping id/component_id/class/input_class/score and appending step_link=5, component_name=6). Phase D must include this index table explicitly, or the implementer will guess wrong and produce a silent misread. | Phase D "Files to modify" updated with the exact new SELECT column positions and `row.get(N)` index for each new field. |
| `FIND-P10-02` | **CRITICAL** | §3 Open Questions table / cache key formula | Open Question #2 in §3 says the memoisation cache key is `sha256(step_link + "\|" + sorted_include_uuids.join(","))`. This is the **OLD circular key** that was already corrected in DESIGN-02 / §0.7 to `sha256(step_link + "\|" + sha256(step_descriptions_json) + "\|" + sha256(variable_patterns_sorted_json))`. The §3 table still has the wrong formula, creating a contradiction with §0.7 and the Phase N.3 implementation. A future implementer reading §3 without §0.7 would implement the circular (broken) key. | §3 Open Question #2 answer updated to reference the corrected key formula from DESIGN-02 / §0.7. |
| `FIND-P10-03` | **HIGH** | Phase I / `GenericComponent` vs `class_code` field conflict | Phase I says "class 22 validation must use `Generic(GenericComponent<'a>)` where `GenericComponent` carries `{ name, content, class_code }`" — but the live `GenericComponent` struct (confirmed: `component_validator.rs:62–67`) has exactly 3 fields: `{ name, description, content }` — NO `class_code`. Meanwhile Phase C COMP-04 proposes adding `extra: Option<serde_json::Value>` (not `class_code`). These two proposals are inconsistent. An implementer following Phase I would add `class_code: i32` to `GenericComponent`, but Phase C COMP-04 says to add `extra`. The correct resolution: **Phase C COMP-04 takes precedence** — add `extra: Option<serde_json::Value>` to `GenericComponent` for the task_groups extensibility needed by class 23. For class 22 (PythonCode), the validator only needs `name`, `description`, and `content` — the 3 existing fields suffice. The Phase I claim that `GenericComponent` needs `class_code` is wrong: the validator dispatch arm (`22 =>`) already knows it's class 22 because it's in the class-22 arm of the match. `class_code` is known from the dispatch, NOT needed in the payload. Remove `class_code` from the Phase I `GenericComponent` description. | Phase I description corrected: `GenericComponent` does NOT need `class_code` (it's implicit from the dispatch arm). Phase I uses the existing 3-field struct for class 22; the 4th field `extra: Option<serde_json::Value>` added by Phase C COMP-04 is used for class 23 (`task_groups`). The two are now consistent. |
| `FIND-P10-04` | **HIGH** | Phase H / `LoopPromptBundleRequest` field count mismatch | Phase H item 6 / FIND-11 says the current `LoopPromptBundleRequest` struct has 7 fields and the test `support.rs:340` site must add `recipe_hint: None`. The confirmed struct (lines **986-996** of `host.rs`; an earlier cite "978-1005" was off) has exactly 7 fields: `mode, context_cursor, surface_version, capability_view, checkpoint_state_ref, max_messages, inline_messages`. Struct addition confirmed correct. No error. However, the plan's FIND-19 note includes a grep instruction that is correct and must be followed. ✅ Confirmed — no fix needed beyond the existing note. | Line cite corrected from "978-1005" to "986-996". Confirmed correct. |
| `FIND-P10-05` | **MEDIUM** | Phase D / `resolve_intent` Rust `Match` construction — `score` column position shifts | After Phase D adds the LEFT JOIN and two extra columns (`step_link`, `component_name`) to the SELECT, the `score` field moves from `row.get(4)` to still be at `row.get(4)` IF the two new columns are appended at the END of the SELECT (after score). But if they are inserted BEFORE score (e.g. after component_class_code), then `input_class` and `score` shift right. The safe approach: append `step_link` and `component_name` as columns 5 and 6 (zero-indexed), AFTER the existing 5 columns. Then `id=0, component_id=1, component_class_code=2, input_class=3, score=4, step_link=5, component_name=6`. The disambiguation and disambiguation-related paths also read columns by position — they must not shift. | Phase D "Files to modify" updated with the explicit column-append-last rule and the exact index assignments. |
| `FIND-P10-06` | **MEDIUM** | Phase I / `GenericComponent` in `validate_by_class` dispatch — `description` field availability | When Phase I adds a class-22 arm to `validate_by_class`, it uses `Generic(GenericComponent { name, description, content })`. All three fields are present in the current struct. For PythonCode (class 22), `description` exists on `reborn_python_code` (same column shape as specs). For class 23, `description` exists and `overview_doc` maps to `content`. This is consistent. ✅ Confirmed — no issue. | No change needed. Confirmed consistent. |
| `FIND-P10-07` | **LOW** | `class_label` in `intent_system.rs` — confirmed exact labels | Full confirmed label set (lines 254-265+): `0 => "tool"`, `1 => "skill_rusty"`, `2 => "skill_monty"`, `3 => "skill_llm"`, `4 => "extension_worker"`, `5 => "extension_cron"`, `6 => "extension_trigger"`, `7 => "extension_webhook"`, `8 => "extension_plan"`, `9 => "extension_revision"`. Phase B/C must add `22 => "python_code"` and `23 => "extension_catalogue"` — lower_snake_case consistent with all other entries. Confirmed correct. | No change. Confirmed accurate. |

### Pass 14 findings (full codebase re-verification — live source cross-check of every prior pass claim)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-NEW-PASS14-01` | **LOW** | Phase E / `fetch_for_consumer` PERF tag naming | The plan consistently uses `PERF-03` to refer to the 12-sub-select UNION ALL in `fetch_for_consumer`. The LIVE code comment inside `retrieval_source.rs` tags this optimisation as `PERF-05` (not `PERF-03`). The sub-select COUNT (12 today → 14 after adding classes 22/23) is correct in the plan. Only the internal code-comment tag label differs. No code behaviour is affected; this is a naming inconsistency between the plan's tag vocabulary and the code's own comment vocabulary. When implementers add the two new sub-selects they should use the existing `// PERF-05` comment convention already present in the file, not `// PERF-03`. | Plan note added: `PERF-03` in plan text ≡ `PERF-05` in the live source comments. Implementers adding the class-22/23 UNION ALL arms must label them `// PERF-05 (class 22)` and `// PERF-05 (class 23)` to match the existing convention. No plan phase body changes required. |
| `FIND-NEW-PASS14-02` | **LOW** | All phases / `execute_orchestrator` parameter count | `FIND-NEW-PASS12-01` informally calls `execute_orchestrator` a "13+ parameter" function. The confirmed live signature (`orchestrator.rs:444-461`) has **16 parameters**: `code, thread, llm, effects, leases, policy, signal_rx, event_tx, retrieval, store, platform_info, gate_controller, persisted_state` (13 unconditional) + `pg_pool` (feature-gated `skills-db`) + `retrieval_source, max_duration_override` (2 unconditional). Total unconditional: 15; total including feature-gated: 16. The "13+" notation is not wrong but any implementer adding a new parameter must use 17 as the next `$N` placeholder for Rust positional binds, and must add the feature-gate annotation if the parameter is pool-related. | Plan note clarified: `execute_orchestrator` has 15 unconditional params + 1 feature-gated (`pg_pool`). When adding `assemble_prior_knowledge_with_hint` as a new `pub fn`, it does NOT need to mirror `execute_orchestrator`'s full signature — it is a new standalone Rust entry point, not a wrapper. |
| `FIND-NEW-PASS14-03` | **LOW** | Phase A / `instruction_builder.rs` and `types/ibs.rs` modules | The plan specifies creating `crates/brassclaw_engine/src/memory/instruction_builder.rs` (new module) and `crates/brassclaw_engine/src/types/ibs.rs` (new module). Both are confirmed **absent** from the live codebase: `memory/mod.rs` has 11 modules (none named `instruction_builder`); `types/mod.rs` has 12 modules (none named `ibs`). Both must be created by Phase A. ✅ Plan is correct — no change needed. Confirmed absent, creation is required. | No change needed. Confirmed both modules are absent and must be created as the plan specifies. |
| `FIND-NEW-PASS14-04` | **MEDIUM** | Phase H / `execute_orchestrator` call-site in `loop_engine.rs` — `retrieval_source` parameter ordering | `execute_orchestrator` at `orchestrator.rs:459` takes `retrieval_source: Option<&Arc<dyn RetrievalSource>>` as the **15th unconditional parameter** (after `persisted_state` and `pg_pool`). The `loop_engine.rs` call site that Phase E.0 modifies to pass a `PostgresSource` must pass it in the **correct positional slot**. Any mistake here is a silent compile error (same `Option<&Arc<dyn RetrievalSource>>` type). Phase E.0 "Files to modify → `loop_engine.rs`" must specify the positional slot explicitly: replace `retrieval_source: None` with `retrieval_source: Some(&pg_retrieval_source)` at the 15th argument position. | Phase E.0 implementation note updated: when replacing the `None` at the `retrieval_source` argument site in `loop_engine.rs`, confirm it is argument slot 15 (0-indexed: 14). The existing `with_retrieval_source()` builder method at `loop_engine.rs:219` is the safe wiring path — prefer it over positional patching. |
| `FIND-NEW-PASS14-05` | **HIGH** | Phase H / `LoopContextPort` — confirmed single method, `resolve_message_text` is entirely new | `LoopContextPort` at `host.rs:778-784` has **exactly one method**: `load_loop_context`. The `resolve_message_text` method (FIND-28) does NOT exist on this trait yet — it must be added as a new default method with `Err(BrassPanic::Unimplemented)` body. The plan is correct, but no audit pass had explicitly confirmed that the method is 100% absent (vs possibly existing with a different signature). Confirmed absent. Any host implementation that tries to call `resolve_message_text` before Phase H adds it will fail to compile. ✅ Plan is correct. | No change. Confirmed `resolve_message_text` is entirely new and must be added by Phase H exactly as specified by FIND-28. |
| `FIND-NEW-PASS14-06` | **MEDIUM** | All Phases / `memory/mod.rs` and `types/mod.rs` — new modules not yet registered | `crates/brassclaw_engine/src/memory/mod.rs` currently declares 11 modules. `crates/brassclaw_engine/src/types/mod.rs` currently declares 12 modules (no `ibs`). Every phase that creates a new `.rs` file under these directories must also add the corresponding `pub mod <name>;` line to the relevant `mod.rs`. The plan body phases include "add to `mod.rs`" instructions, but they must be followed precisely: forgetting the `mod.rs` line is the #1 cause of "file created but unused" Rust compile silences. This is a documentation reminder, not a new bug. ✅ Plan already specifies `mod.rs` additions per phase. | No change. Confirmed: the plan's per-phase `mod.rs` additions are mandatory and must not be omitted. Each phase's "Files to modify" checklist must include the `mod.rs` entry. |

### Pass 13 findings (final pass — remaining stubs, wording precision, and test coverage gaps)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-NEW-PASS13-01` | **MEDIUM** | Phase H Tests section (line ~5446) | The Phase H Tests section was **missing unit tests** for the two new `pub` library functions `assemble_prior_knowledge_with_hint` and `execute_tier_zero_channel` introduced by FIND-NEW-PASS12-01/02. Integration tests alone are insufficient: both functions have significant conditional branches (`recipe_hint: Some` vs `None`; empty vs valid `orchestrator_content`; PythonCode vs Skill body) that must be exercised at the unit level without a full integration stack. | Added 7 unit tests for the two new functions to the Phase H Tests section, covering: (1) `assemble_prior_knowledge_with_hint` with `recipe_hint: None` (fetch_for_turn path), (2) with `Some(stashed)` skipping fetch, (3) returning `tier_zero: true` when `llm_call_required: false`; (4) `execute_tier_zero_channel` happy-path, (5) empty `orchestrator_content` error guard, (6) Skill body error guard, (7) regression: `handle_assemble_prior_knowledge` dispatch arm still passes after refactor. |
| `FIND-NEW-PASS13-02` | **LOW** | Phase H §4 / canonical.rs pseudocode block / comment at line ~5292 | The inline comment inside the `PostRecipeOutcome::TierZero` pseudocode block (line 5292) still said "delegating to the engine orchestrator's no-LLM entry point" — vague and inconsistent with FIND-NEW-PASS12-02 which named the concrete function `execute_tier_zero_channel`. An implementer reading only this block (without cross-referencing H5) would still not know what function to call. | Updated the comment to explicitly say "by calling `execute_tier_zero_channel(thread, orchestrator_content, rust_context, ...)`" with a cross-reference to FIND-NEW-PASS12-02 and a NOT note distinguishing it from the Python `execute_recipe_orchestrator_channel`. |
| `FIND-NEW-PASS13-03` | **LOW** | §5 Tier 1 turn-flow diagram (line ~7002) | The Tier 1 diagram said "PromptStage does NOT clear state.recipe_hint (COMP-03 — Python step-0 clears it)". This is **wrong wording**: the STAGE clears it (after `run_step_zero` returns), NOT "Python step-0". The Python handler has no `&mut state` access (see FIND-P9-15). This is a documentation precision issue: a reader following only the §5 diagram would incorrectly believe that clearing is the handler's responsibility, defeating the stash/unstash protocol. | Fixed to: "PromptStage does NOT clear state.recipe_hint (COMP-03 — the STAGE clears it AFTER run_step_zero returns; Python step-0's handler has no &mut state access — see FIND-P9-15)." |
| `FIND-NEW-PASS13-04` | **MEDIUM** | Phase H §4 / `execute_recipe_orchestrator_channel` PythonCode example (line ~5573) | The example PythonCode body used `vars["path"]` as if there is a runtime `vars` dict available in the Python scope. **This is wrong.** Per §0.20.2 (line 2301-2308): `No goal, pkr, context` — orchestrator-layer globals are NOT injected into step scope. Per §0.20.3 (line 2317-2334): `{{vars.slot0}}` substitution is done by the IBS at assembly time (before the body runs), not at runtime. A body that reads `vars["path"]` at runtime will get a `NameError`. The authored recipe template has `"{{vars.slot0}}"` as a literal placeholder; the IBS replaces it with the extracted value and the RUNTIME body sees the literal string. The example `vars["path"]` directly contradicts §0.20.3 and would mislead recipe authors. | Replaced the example with a comment block explaining: (1) there is NO runtime `vars` dict; (2) IBS substitution happens before the body runs; (3) the authored template uses `{{vars.slot0}}`; (4) at runtime, the body sees the literal value already baked in. Added `FIND-NEW-PASS13-04` marker to the fix. |

### Pass 12 findings (full plan re-read + deep live source verification of Phase H engine boundary — new issues found)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-NEW-PASS12-01` | **CRITICAL** | Phase H §H.0 H5 / `LoopOrchestratorPort::run_step_zero` implementation | The plan says the composition host implements `run_step_zero` by "delegating to the engine orchestrator's `handle_assemble_prior_knowledge` path (orchestrator.rs:2574+)". This is **critically underspecified and cannot be implemented as stated.** Verified from live source: `handle_assemble_prior_knowledge` (orchestrator.rs:2552) is a **private `async fn`** — NOT `pub`, NOT callable outside `orchestrator.rs`. It takes `args: &[MontyObject]` (Monty VM argument array), NOT a `recipe_hint: Option<serde_json::Value>` parameter. The only externally-callable entry point into the engine's Python execution is `execute_orchestrator` (orchestrator.rs:444, `pub async fn`) — which takes 13+ parameters including `thread: &mut Thread`, `llm: &Arc<dyn LlmBackend>`, `effects`, `leases`, `policy`, `signal_rx`, etc., and runs the **entire Python orchestrator script from scratch** as a full VM execution. There is **no mechanism** today for the composition host to call "just the `__assemble_prior_knowledge__` handler" in isolation. The plan's stash/unstash protocol (Phase H §5) describes the correct LOGIC (what `handle_assemble_prior_knowledge` should do when `recipe_hint` is passed as a parameter), but gives no guidance on HOW the composition host invokes it. The **LoopOrchestratorPort** bridge is the right design — it simply needs a concrete implementation path specified. **Three options (ranked):** **(A — RECOMMENDED)** Create a new `pub` Rust function in `brassclaw_engine` — e.g. `pub async fn assemble_prior_knowledge_with_hint(pool, scope, goal, token_budget, sender_class_code, recipe_hint: Option<serde_json::Value>) -> Result<PkrAssemblyResult, EngineError>` — that is the pure Rust logic of `handle_assemble_prior_knowledge` without the Monty VM wrapper. The composition host calls this directly from its `run_step_zero` impl. The `handle_assemble_prior_knowledge` Python handler becomes a thin wrapper calling this new function. **(B)** Run a minimal Python VM for step-0: have `execute_orchestrator` accept a `step_zero_only: bool` flag that causes `default.py` to `return` after step 0. This couples the engine Python and the Rust port interface — fragile. **(C)** Have `ExecutionLoop` expose a `run_step_zero_only()` method that runs the full orchestrator but halts it after the step-0 block returns (via a signal or special pkr key). Also fragile. **Option A is cleanly correct and must be specified in Phase H.** | Phase H §H.0 H5 and Phase H §5 (stash/unstash) updated: add explicit note that the composition host's `run_step_zero` implementation calls a **new `pub` function** `brassclaw_engine::executor::orchestrator::assemble_prior_knowledge_with_hint(...)` (not `handle_assemble_prior_knowledge` directly). Phase H "Files to modify" gains a new entry: `crates/brassclaw_engine/src/executor/orchestrator.rs` — add `pub async fn assemble_prior_knowledge_with_hint(...)` as a library API, refactor `handle_assemble_prior_knowledge` to call it internally. Similarly, `run_tier_zero` must call a new `pub async fn execute_tier_zero_channel(...)` that embodies the `execute_recipe_orchestrator_channel` logic as a Rust library function (not a Python helper). The Python `execute_recipe_orchestrator_channel` helper in `default.py` is still needed for the Model A (engine path) production route — but the composition host's `LoopOrchestratorPort` impl uses the Rust library function directly. |
| `FIND-NEW-PASS12-02` | **MEDIUM** | Phase H §H.0 H5 / `run_tier_zero` implementation | Same gap as FIND-NEW-PASS12-01, for `run_tier_zero`. The plan says the composition host implements `run_tier_zero` by calling "a new no-LLM entry point on the engine orchestrator that runs the skill/PythonCode channel against the pre-loaded rust execution context". No such entry point is specified. `execute_recipe_orchestrator_channel` is a Python function in `default.py` — the composition host cannot call it. Phase H must specify a new `pub async fn execute_tier_zero_channel(pool, scope, thread, orchestrator_items, rust_context, ...) -> Result<TierZeroChannelResult, EngineError>` in `brassclaw_engine::executor::orchestrator`. This function IS callable from composition and embodies the Tier-0 execution logic. | Phase H §H.0 H5 updated: add explicit `pub async fn execute_tier_zero_channel(...)` spec as the composition host entry point for `run_tier_zero`. The Python `execute_recipe_orchestrator_channel` remains on the engine path (Model A) but is NOT what the composition host calls. |
| `FIND-NEW-PASS12-03` | **MEDIUM** | Phase H §3b / `execute_recipe_orchestrator_channel` Python helper spec | The `execute_recipe_orchestrator_channel` specification (Phase H item 3b) is correct for the **engine (Model A) path** — it is a Python helper in `default.py` that runs the orchestrator channel when `pkr["tier_zero"]` is true. This is the ONLY mechanism for Model A (current production). The plan correctly notes it. However, Phase H does NOT call out that this Python helper is NOT reachable from the composition host's `LoopOrchestratorPort` implementation — it is internal to the Python VM execution. The plan says "composition host implements `run_tier_zero` by ... driving the Rust executioner via the loaded skills" but never says HOW (no Rust function to call). With FIND-NEW-PASS12-01 resolved (add `assemble_prior_knowledge_with_hint` and `execute_tier_zero_channel`), this becomes consistent: Model A uses the Python helper, Model B/C (agent-loop) uses the new Rust library functions. ✅ Already fixed by FIND-NEW-PASS12-01 and FIND-NEW-PASS12-02. | No additional change needed — covered by FIND-NEW-PASS12-01/02. |
| `FIND-NEW-PASS12-04` | **LOW** | Phase H §H.0 H3 / `LoopContextPort::resolve_message_text` default impl | Confirmed: `LoopContextPort` at `host.rs:778-784` has exactly ONE method: `load_loop_context`. FIND-28 requirement for a default `Err(Unimplemented)` body on `resolve_message_text` is confirmed as mandatory. ✅ Plan is correct. | No change needed. Confirmed accurate. |
| `FIND-NEW-PASS12-05` | **LOW** | `AgentLoopDriverHost` supertrait count | Confirmed: `host.rs:2185-2201` lists exactly 13 ports: `LoopRunInfoPort, LoopContextPort, LoopPromptPort, LoopInputPort, LoopModelPort, LoopCapabilityPort, LoopTranscriptPort, LoopCheckpointPort, LoopProgressPort, LoopCompactionPort, LoopCancellationPort, LoopRecipePort, LoopInterceptorPort`. Phase H adds `LoopRetrievalPort` as the 14th and `LoopOrchestratorPort` as the 15th. The `AgentLoopDriverHost` blanket impl at `host.rs:2204-2220` must also be updated to add the two new trait bounds. The plan correctly specifies adding to the supertrait list but does NOT mention updating the blanket impl. Any host that already implements all 13 must also implement the 14th and 15th (with `NoRetrieval`/`NoOrchestrator` default bodies). The blanket impl at line 2204 MUST also add the two new bounds. | Phase H §H.0 (port addition steps for H4 and H5) updated with explicit note: the `impl<T> AgentLoopDriverHost for T where T: ... + Send + Sync {}` blanket impl at `host.rs:2204` must also have `+ LoopRetrievalPort + LoopOrchestratorPort` added to its `where` clause. |

### Pass 11 findings (full plan + live codebase read of every referenced source file — new issues found)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-NEW-AUDIT-01` | **LOW** | Phase B/C / `intent_system.rs` tests | The plan describes `class_label` in `intent_system.rs` as returning a `String`. Confirmed at line 254: `pub fn class_label(class_code: i32) -> String`. Test assertions in Phase B/C must use `== "python_code".to_string()` not `== "python_code"` (a `&str` literal). Actually `"python_code"` compares fine against a `String` in Rust — no correction needed for the tests. But the return type being `String` (not `&'static str`) means the `class_label` calls at line 396 (`class_label: class_label(component_class_code)`) inside `IntentCandidate` construction allocate a new `String` per candidate. This is existing code, not a new issue. ✅ No fix needed. | No change — confirmed correct. |
| `FIND-NEW-AUDIT-02` | **MEDIUM** | Phase B/C / FIND-P5-01 / `recipe_store.rs:861-882` full label map | Prior passes mentioned specific labels but never provided the complete live map. Full verified map (Pass 11): `0=>"Tool"`, `1=>"Skill (Rusty)"`, `2=>"Skill (Monty)"`, `3=>"Skill (LLM)"`, `4..=9=>format!("Extension (class {code})")`, `10=>"Orchestrator"`, **`12=>"Document"` (NOT "Spec")**, `13=>"Guide"`, `14=>"Reference"`, `15=>"Note"`, `16=>"Action"`, `17=>"Template"`, `18=>"Snippet"`, `19=>"Config"`, `20=>"Workflow"`, `21=>"Recipe"`, `50=>"Scaffold"`. Class 12's label in `recipe_store.rs` is `"Document"` — different from `interceptor_config_service.rs` which uses `"Spec"` for class 12. No test in the plan asserts class 12 for `recipe_store.rs`, so no test correction is needed, but the FIND-P5-01 description in Phase C has been updated to include the full verified arm mapping including the class 12 discrepancy. | FIND-P5-01 note in Phase C updated with the full verified arm map and the `12 => "Document"` clarification. |
| `FIND-NEW-AUDIT-03` | **CRITICAL** | Phase E / `class_code_to_table` helper — missing `10 \| 50` arm | The proposed `class_code_to_table` helper in Phase E was MISSING the `10 \| 50` arm. Verified from live `fetch_component_by_id` (lines 573–627): `10 \| 50 => Some(("reborn_skills", "COALESCE(NULLIF(prior_knowledge_content,''), body)"))`. Classes 10 (Orchestrator) and 50 (Scaffold) resolve to `reborn_skills`. Extracting the helper without this arm would silently break retrieval for Orchestrator and Scaffold components after Phase E. **This is a silent regression.** | Phase E `class_code_to_table` helper specification updated to include the mandatory `10 \| 50` arm with inline comment. Tagged `FIND-NEW-AUDIT-06` in Phase E body. |
| `FIND-NEW-AUDIT-04` | **HIGH** | Phase E / `fetch_for_consumer` UNION ALL — confirmed arm count | Full read of `fetch_for_consumer` (lines 283–441 of `retrieval_source.rs`) confirms: 12 sub-selects covering skills, extensions_unified, actions, specs, tool_skills, plans, summaries, docus, lessons, issues, notes, recipes. The plan's PERF-03 count (12 → 14 after adding classes 22/23) is exactly correct. No class 10/50/12 sub-selects exist in the UNION ALL (those classes route through `reborn_skills` in `fetch_component_by_id` but are NOT separate arms in the UNION ALL). The `ORDER BY class_code ASC, prompt_uid ASC` order is confirmed. ✅ Plan is correct. | No change needed. Confirmed accurate. |
| `FIND-NEW-AUDIT-05` | **HIGH** | Phase G / `default.py` step-0 block — confirmed live code | Full read of `default.py` lines 994–1060 (Pass 11). The current step-0 block IS exactly: (1) `pkr = __assemble_prior_knowledge__(...)`, (2) `override_prompt_creation` / `formatted_content` branch, (3) `insert_volatile_context_at_n_minus_1`, (4) the dead `docs = __retrieve_docs__(goal, 5)` shim block with comment "Pre-Phase-5 fallback" (lines 1010–1028), (5) `all_skills = __list_skills__()` and `select_skills()` block (lines 1031–1060). The plan's Phase G description of what to remove is exactly correct. The `execute_action_procedure` at line 901 is confirmed present. The `__llm_complete__` call is at line 1103 (plan says 1103). ✅ No correction needed. | No change needed. Confirmed accurate. |
| `FIND-NEW-AUDIT-06` | **MEDIUM** | Phase H / `LoopExecutionState` — confirmed last field line | `spawn_subagent_hint: Option<String>` is at line 102 of `state.rs` (confirmed). The struct has exactly the fields listed in the plan (lines 47–103). No `last_user_text`, `recipe_rust_context`, or `recipe_hint` fields exist. Phase H appends after line 102. ✅ No correction needed. | No change needed. Confirmed accurate. |
| `FIND-NEW-AUDIT-07` | **MEDIUM** | Phase H / `canonical.rs` — confirmed exhaustive match | Lines 94-96 of `canonical.rs` confirm the single-variant exhaustive match: `state = match self.recipe.process(ctx, RecipeInput { state }).await? { RecipeStep::Continue { state: next } => *next, }`. Adding `TierZero` and `ActionExecuted` variants WILL cause a compile error until canonical.rs is restructured. Plan's CANONICAL-01 finding is correct. ✅ No correction needed. | No change needed. Confirmed accurate. |
| `FIND-NEW-AUDIT-08` | **MEDIUM** | Phase E.0 / `manager.rs` spawn path — confirmed `TODO(Phase K)` at line 377 | Lines 377-383 confirm the `TODO(Phase K)` comment and `RamSource` usage. The `with_retrieval_source()` call is at line 400 (plan says this). The `ThreadManager` struct (lines 34-61) has NO `pg_pool` field — Phase E.0 adds it. Struct has `llm, effects, store, capabilities, leases, policy, lease_planner, tree, running, completed, event_tx, gate_controller, max_duration_secs` — 13 fields. No `retrieval_source_override` or `pg_pool` field exists yet. ✅ Plan is correct. | No change needed. Confirmed accurate. |
| `FIND-NEW-AUDIT-09` | **LOW** | Migration sequence — confirmed current highest migration is V049 | Directory listing confirms highest migration is `V049__session_threads_version.sql`. Next migration is V050. Plan's "Next migration: V050" header is correct. ✅ Confirmed. | No change needed. |

### Pass 9 findings (full plan + codebase cross-read — new issues found)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-P9-01` | **CRITICAL** | Phase A.5 / §0.18 / `gate1_pass` crate-boundary | `gate1_pass` is `pub(crate)` on `ValidationQueueStore` in `brassclaw_reborn_composition`. BUT `ComponentValidator` lives in `brassclaw_engine` (a separate crate). `brassclaw_engine` CANNOT call `pub(crate)` methods from another crate. The Q1 orchestration sequence must therefore live entirely in `brassclaw_reborn_composition`: call `ComponentValidator::validate_by_class` (pure fn, importable cross-crate) → call `gate1_pass` or `gate1_fail` in the same crate. Add `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs` with function `run_q1_validation(pool, scope, component_id, class_code, payload, config, queue_store)` that owns the cross-crate orchestration. Never call `gate1_pass` from `brassclaw_engine`. | Phase A.5 "Files to create" updated: add `q1_orchestrator.rs`. Phase I updated: new Q1 rules go in `component_validator.rs` (engine, pure logic); composition `q1_orchestrator.rs` orchestrates the call. |
| `FIND-P9-02` | **CRITICAL** | Phase H item 3b / `execute_recipe_orchestrator_channel` unspecified | This Python function is referenced 6+ times but never defined. Specification: `def execute_recipe_orchestrator_channel(pkr, goal, state)`: (1) extract `orchestrator_content` from `pkr` (pre-assembled Skill+PythonCode block); (2) for each Skill/PythonCode step in `orchestrator_content` in order: interpret the step instructions, call `__execute_action__` on the Rust executioner using ToolSkill bindings (already in execution context from `RecipeStage`); (3) run PythonCode formatter bodies; (4) return `{"result": formatted_output, "outcome": "success"}` or `{"outcome": "error", "message": ...}`. **DESIGN RISK:** Skill bodies are narrative LLM instructions, not formal programs — interpreting them without an LLM is fragile. **Recommended Q1 constraint:** Tier-0 recipes (`llm_call_required: false`) MUST have ONLY `PythonCode` components in `orchestrator_steps`, never `Skill` components. This makes Tier 0 deterministic: PythonCode is executable Python, not LLM prose. Add this as a hard Q1 rule in Phase I §shell-guard. | Phase H item 3b + §0.9 updated with specification. Phase I §shell-guard extended with Tier-0 PythonCode-only rule. |
| `FIND-P9-03` | **CRITICAL** | Phase H / `RetrievalTurnResult`, `PriorKnowledgeBundle`, `TierZeroReply` never defined | All three `brassclaw_turns`-native types are referenced but never given struct definitions. Specs: `pub struct RetrievalTurnResult { pub tier0_eligible: bool, pub llm_call_required: bool, pub rust_items: serde_json::Value, pub orchestrator_items: serde_json::Value, pub routing_meta: serde_json::Value }`. `pub struct PriorKnowledgeBundle { pub orchestrator_content: String, pub matched_component_ids: Vec<String>, pub override_prompt_creation: bool }`. `pub struct TierZeroReply { pub text: String, pub matched_component_ids: Vec<String> }`. Define all three in `crates/brassclaw_turns/src/run_profile/host.rs` alongside the port traits. | Phase H §H.0 updated with full struct definitions. |
| `FIND-P9-04` | **CRITICAL** | Phase E / `fetch_components_by_ids` (PERF-02) — never specified | Spec: `async fn fetch_components_by_ids(pool: &PgPool, scope: &ComponentScope, ids_by_class: &[(Uuid, i32)]) -> Result<Vec<ComponentItem>, RetrievalSourceError>`. Group by `(table, content_expr)` from the class-code match; for each group: `SELECT ... FROM {table} WHERE id = ANY($1) AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5 AND validation_status='validated' AND '05:validator' != ALL(consumer_tags)`. Same security invariant as `fetch_component_by_id` (literals only, never user input). | Phase E "Files to modify" updated with full specification. |
| `FIND-P9-05` | **CRITICAL** | Phase A.5 `ValidationQueueStore::approve` — transaction not specified | `approve` touches two tables (`reborn_validation_queue` DELETE + component table UPDATE). Must be one transaction. Flow: (1) BEGIN; (2) UPDATE {component_table} SET validation_status='validated' WHERE id=$component_id AND scope; (3) DELETE FROM reborn_validation_queue WHERE component_id=$1 AND scope (trigger fires here); (4) COMMIT. Dispatch on `component_class` to find the target table using the same class→table map as `fetch_component_by_id`. Unknown class → return error before transaction. | Phase A.5 `ValidationQueueStore::approve` spec updated with transaction requirement, dispatch on component_class, and ordering (UPDATE before DELETE). |
| `FIND-P9-06` | **CRITICAL** | Phase G `call_action` Option A — data migration never specified | Phase G recommends Option A (add `action_id: UUID` to `call_action` step defs) but never provides the migration SQL, ambiguity handling, or fallback. Migration (run at Phase G deploy, not a Flyway migration): `UPDATE reborn_actions SET steps = (SELECT jsonb_agg(CASE WHEN step->>'type'='call_action' AND step->>'action' IS NOT NULL THEN step || jsonb_build_object('action_id', (SELECT id::text FROM reborn_actions a2 WHERE a2.name=step->>'action' AND a2.tenant_id=a1.tenant_id AND a2.user_id=a1.user_id AND a2.agent_id=a1.agent_id AND a2.project_id=a1.project_id LIMIT 1)) ELSE step END) FROM jsonb_array_elements(steps) step) FROM reborn_actions a1 WHERE a1.id=reborn_actions.id`. Ambiguous/unresolvable: leave `action_id` null; at runtime null falls back to Option B (`__resolve_component_by_name__`). Post-migration audit: `SELECT id, steps FROM reborn_actions WHERE steps @> '[{"type":"call_action"}]'::jsonb AND steps @> '[{"type":"call_action","action_id":null}]'::jsonb`. | Phase G "Files to modify" updated with migration SQL, fallback, and audit query. |
| `FIND-P9-07` | **HIGH** | Phase A.5 V051 `UNIQUE` constraint column order | Phase A.5 DDL: `UNIQUE (component_id, tenant_id, user_id, agent_id, project_id)`. §0.18: `UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)`. Scope-first is more efficient (queries always filter scope first). | Phase A.5 DDL corrected to scope-first ordering matching §0.18. |
| `FIND-P9-08` | **HIGH** | Phase A.5 V051 `state` comment — wrong label for state 2 | DDL comment says `2=Q2_pending`. Should be `2=Q1_passed`. State 2 = Gate 1 PASSED, awaiting Q2 manual review. Writing "Q2_pending" implies the Q2 reviewer sets it — that is the OPPOSITE of the security invariant (only Gate 1 writes state 2). | V051 DDL comment corrected to `1=Q1_queue, 2=Q1_passed, 3=rejected, 4=deletion_candidate`. |
| `FIND-P9-09` | **HIGH** | Phase A `PgRecipe::is_tier0_eligible()` fix — `has_validation` check not applicable | `Recipe::is_tier0_eligible()` checks `has_validation` (validation hook wired). `PgRecipe` has no `validation` field (no `RecipeValidation` column on `reborn_recipes`). The plan's proposed fix (`is_deliverable() && tier ∈ {mature,candidate} && wilson_lower >= 0.70`) is correct for `PgRecipe`. The validation-hook guard lives in `TurnRoutingSignals` (from `fetch_for_turn`). Plan is correct as stated. | No change. Confirmed correct. |
| `FIND-P9-10` | **HIGH** | Phase J stale "V054 intent_examples no-op" references | After Decision 2 V-number shifts, Phase J still mentions "V054's ADD COLUMN intent_examples as NO-OP". Post-shift this is V055. All "V054 intent_examples" references must read "V055 (NO-OP omitted per FIND-12)". | Phase J body updated. |
| `FIND-P9-11` | **HIGH** | Phase N.1 V059 SQL comment says "abort V058" | Should say "abort V059". | Phase N.1 comment corrected. |
| `FIND-P9-12` | **MEDIUM** | Phase M.2 test list missing trailing-`%` case | `parse_template("search for %")` → `Some(("search for ", ""))` (prefix-anchored, suffix empty) is missing from tests. | Test added to Phase M.2. |
| `FIND-P9-13` | **MEDIUM** | Phase B/C `recipe_store.rs` display label note | Labels are display labels not class names (`13 => "Guide"`, `18 => "Snippet"`, etc.). Implementers must not confuse display labels with class identifiers. `22 => "PythonCode"` and `23 => "Catalogue"` are correct single-word additions. | Note added for implementer clarity. |
| `FIND-P9-14` | **MEDIUM** | Phase L V057 `DROP CONSTRAINT IF EXISTS reborn_recipes_source_check` is a no-op | V033 has no source CHECK on `reborn_recipes`. The IF EXISTS prevents an error but this is a silent no-op. | V057 comment updated to document this. |
| `FIND-P9-15` | **MEDIUM** | Phase H §5 `handle_assemble_prior_knowledge` reads `state.recipe_hint` — crate boundary violation | `brassclaw_engine` does not depend on `brassclaw_agent_loop`. The handler CANNOT read `LoopExecutionState.recipe_hint`. Correct model: the stage reads `state.recipe_hint` and passes it as a parameter to `run_step_zero`; the composition host passes it to the engine handler as an argument. The handler receives `recipe_hint: Option<serde_json::Value>` as a parameter — never reads `LoopExecutionState`. Stage clears `state.recipe_hint = None` AFTER `run_step_zero` returns. | Phase H §5 corrected throughout. All "handler checks state.recipe_hint" replaced with "stage extracts recipe_hint from state, passes as parameter; handler receives as argument; stage clears after return." |
| `FIND-P9-16` | **LOW** | Phase M.2 suffix logic verified correct | `rsplitn(2, '%').next()` correctly yields the text after the last `%`. Tests confirmed correct. | No change. |

### Pass 8 findings (full `Thread` struct read — new critical issue)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-P8-01` | **CRITICAL** | Phase F / `Thread` struct | See full entry in Pass 7 findings table (added in line order). Summary: `Thread` has no `tenant_id` or `agent_id` field. Two stubs at `orchestrator.rs:2586-2590` and `3142-3145` must both be fixed. Phase F adds the fields via a builder method `with_tenant_agent`, greps all `Thread::new` sites, and fixes both scope construction stubs. | Phase F security fix section updated with confirmed field absence, second stub location, and exact implementation steps using builder pattern. |

### Pass 7 findings (deep live-source re-read — new issues found, not in any prior pass)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-P7-01` | **CRITICAL** | Phase G / Phase H / §0.9 / entire plan | `default.py` is referenced throughout the plan as `crates/brassclaw_engine/src/executor/default.py`. This path **does not exist**. The actual path is `crates/brassclaw_engine/orchestrator/default.py`. The file is embedded in Rust via `include_str!("../../orchestrator/default.py")` at `orchestrator.rs:55`. Every plan reference to `crates/brassclaw_engine/src/executor/default.py` must be `crates/brassclaw_engine/orchestrator/default.py`. | All Phase G and Phase H "Files to modify" references to `default.py` corrected to the real path. |
| `FIND-P7-02` | **CRITICAL** | Phase G / `execute_action_by_id` | The plan says Phase G must implement a **new** `execute_action_by_id` helper in `default.py`. This is WRONG: `execute_action_procedure(action_doc, goal, state)` already exists at `orchestrator/default.py:901` and does exactly this job — it executes an Action document deterministically without an LLM call. The `action_short_circuit` branch (Phase G step-0 new code) must call `execute_action_procedure`, NOT a new function with a different name. The difference: the Phase G branch receives `pkr["action_component_id"]` (a UUID) and `pkr["action_name"]`, so it first needs to call `__fetch_component__(uuid, 16)` to get the action doc, THEN pass it to `execute_action_procedure`. The plan's pseudocode is semantically correct but incorrectly names this step "implement `execute_action_by_id`" when what is needed is a thin wrapper that fetches by UUID and delegates to the existing `execute_action_procedure`. | Phase G "NEW FUNCTION REQUIREMENT" note updated: do NOT create a new `execute_action_by_id` function. The new Phase G step-0 branch does: `(1) action_doc = __fetch_component__(pkr["action_component_id"], 16); (2) return execute_action_procedure(action_doc, goal, state)`. The existing `execute_action_procedure` handles everything including the deterministic no-LLM execution. |
| `FIND-P7-03` | **CRITICAL** | Phase F / `handle_assemble_prior_knowledge` scope fix | The plan marks the `tenant_id`/`agent_id` scope fix as "[resolved pre-v3]". But reading the LIVE code at `orchestrator.rs:2586-2590`, the fix applied was: `tenant_id: thread.user_id.clone()` and `agent_id: "default".to_string()` — both are still stubs. The code comment says "Phase 1 stub" and "v3 Phase F will tighten this". The plan's "Review note" saying this was resolved and the deeper `Thread` struct work deferred to Phase F is **correct** — but the note then says "aligned with the documented sibling convention" giving the impression the fix is complete. It is NOT: `tenant_id` is still set to `thread.user_id` (not a real tenant_id), and `agent_id` is `"default"`. Phase F MUST fix this for multi-tenant correctness when `PostgresSource` is live. **The `Thread` struct (`types/thread.rs`) must be verified to confirm whether `tenant_id` and `agent_id` fields exist before Phase F can source real values.** | Phase F "Files to modify" updated: add explicit note that the pre-v3 fix only replaced `"default"` tenant_id with `thread.user_id` (still wrong for multi-tenant) and `agent_id` is still `"default"`. Phase F must either (a) add `tenant_id` and `agent_id` to `Thread` or (b) pass them through the call chain. The `Thread` struct must be read in Phase F before implementing. |
| `FIND-P7-04` | **HIGH** | Phase A / `RECIPE_SELECT` and `decode_recipe_row` | The plan's `FIND-P6-04` re-index table is **confirmed correct** against live code. `RECIPE_SELECT` at `pg_recipe_store.rs:208-217` selects exactly 31 columns (indices 0-30). `decode_recipe_row` at `pg_recipe_store.rs:219-252` uses positional `row.get(0)` through `row.get(30)` exactly as the plan describes. `id` is at 0, `updated_at` is at 30. Phase A appends at 31/32/33. Phase N re-index table is fully accurate. No correction needed — confirmed. |  Confirmed accurate. No change. |
| `FIND-P7-05` | **HIGH** | Phase A / `NewPgRecipe` INSERT | The plan says the Phase A INSERT will grow from 13 to 16 columns. Live code at `pg_recipe_store.rs:261-283` confirms: INSERT currently lists 13 columns (`tenant_id, user_id, agent_id, project_id, name, description, trigger, steps, prior_knowledge_content, override_prompt_creation, consumer_tags, intent_examples, source`) with `VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)`. After Phase A adds `step_descriptions`, `variants`, `dependency_registry`, it becomes 16 columns with `$14`, `$15`, `$16`. This confirms FIND-21. The `NewPgRecipe` struct at `pg_recipe_store.rs:147-162` also lacks these three fields — must be added. | Confirmed accurate. FIND-07 / FIND-21 are correct. No change. |
| `FIND-P7-06` | **HIGH** | Phase B / `interceptor_config_service.rs::class_label` actual values | The plan says `1 => "Skill"` — confirmed from live code at `interceptor_config_service.rs:67`. NOT `"Skill (Rusty)"` (that is `recipe_store.rs`'s version). The plan's FIND-20 table for `interceptor_config_service.rs` shows `"Skill"` for class 1 — CORRECT. But the existing code does NOT have arms for classes 2, 3, 4–8 (all fall to `_ => "Component"`) — confirmed. Phase B/C must add `22 => "PythonCode"` and `23 => "Catalogue"` only. | Confirmed accurate. |
| `FIND-P7-07` | **HIGH** | Phase B/C / `recipe_store.rs::class_label` live labels | Live code at `recipe_store.rs:861-882` confirms: `13 => "Guide"`, `14 => "Reference"`, `15 => "Note"`, `17 => "Template"`, `18 => "Snippet"`, `19 => "Config"`, `20 => "Workflow"`, `21 => "Recipe"`, `50 => "Scaffold"`. Also `4..=9 => format!("Extension (class {code})")`. The plan's description is correct. Phase B/C add `22 => "PythonCode".to_string()` and `23 => "Catalogue".to_string()` which fits the single-word pattern (like `"Tool"`, `"Action"`, `"Recipe"`). | Confirmed accurate. |
| `FIND-P7-08` | **HIGH** | Phase E / `fetch_for_consumer` sub-select count | The plan says the UNION ALL has 12 sub-selects (PERF-03 corrected count). Live code at `retrieval_source.rs:283-441` confirms: skills, extensions_unified, actions, specs, tool_skills, plans, summaries, docus, lessons, issues, notes, recipes = **12**. Plan is correct. Adding classes 22 and 23 brings it to 14. | Confirmed accurate. |
| `FIND-P7-09` | **MEDIUM** | Phase H / `LoopExecutionState` last field | The plan says `spawn_subagent_hint: Option<String>` at line 102 is the last field. Live code at `state.rs:102` confirms this. Phase H appends the three new fields after `spawn_subagent_hint`. | Confirmed accurate. |
| `FIND-P7-10` | **MEDIUM** | Phase D / `seed_intent_input` current signature | Live `intent_system.rs:462-505` confirms: current 7-param signature with INSERT that uses 11 columns / 10 placeholders (`VALUES ($1..$8,1,$9,$10)` where score=1 literal). The plan's FIND-NEW-03 spec is **exactly correct**. `step_link` is the 8th param, INSERT becomes 12 columns / 11 placeholders, ON CONFLICT gains `step_link = EXCLUDED.step_link`. | Confirmed accurate. |
| `FIND-P7-11` | **MEDIUM** | Phase H / `PgRecipe::is_tier0_eligible()` incomplete check | Live code at `pg_recipe_store.rs:140-142` confirms: only checks `is_deliverable() && matches!(self.tier.as_str(), "mature" \| "candidate")`. The `wilson_lower >= 0.70` guard is MISSING. FIND-05 is **correct** and the fix must land in Phase A (not Phase E) as specified. The one-line fix: `self.is_deliverable() && matches!(self.tier.as_str(), "mature" \| "candidate") && self.wilson_lower >= 0.70`. | Confirmed accurate. Fix should be in Phase A as plan directs. |
| `FIND-P7-12` | **MEDIUM** | Phase N / `V027` skills `source` CHECK | `V027__reborn_skills.sql:91` confirms `CHECK (source IN ('authored', 'extracted', 'migrated', 'imported'))` — no `'system'`. V057 MUST add `'system'` to this constraint. Also confirmed: `V033__reborn_recipes.sql:113` has `source TEXT NOT NULL DEFAULT 'authored'` with **no CHECK constraint** at all — FIND-P6-02 is correct that V057 must add one. | Confirmed accurate. |
| `FIND-P7-13` | **MEDIUM** | Phase G / `call_action` uses `__retrieve_docs__` | Live code at `orchestrator/default.py:844` confirms: `nested_docs = __retrieve_docs__(nested_name, 1)`. Phase G must replace this with `__fetch_component__(action_uuid, 16)`. However — the `call_action` step receives the nested action by **name** (`step_def.get("action", "")`), not by UUID. The replacement requires knowing the UUID of the nested action. This is a genuine design gap: `call_action` in the existing Action steps schema references actions by name. **Phase G must specify how `call_action` gets the UUID** — either (a) during Action seeding/authoring, all `call_action` steps must have UUIDs pre-resolved and stored as `action_id` alongside `action`, or (b) Phase G adds a `__resolve_component_by_name__(name, class_code)` host function. Option (a) is cleaner (data migration of existing Action steps) but requires touching the Action schema. Option (b) is a targeted stop-gap. The plan's current statement "UUID sourced from the BuildInstruction step" is wrong for `call_action` which is inside Action steps, NOT in a BuildInstruction. | Phase G "Files to modify" updated: the `call_action` nested lookup replacement is MORE COMPLEX than the plan states. `call_action` step defs reference actions by name. Phase G must either (a) require action authors to add `action_id: UUID` to `call_action` step defs (data migration of V029 Action rows at Phase G deploy) and then use `__fetch_component__(action_id, 16)`, or (b) add `__resolve_component_by_name__(name, 16)` as a host bridge. Option (a) is recommended. Phase G must include the data migration of existing `call_action` steps to add UUID references alongside the name. The plan's "UUID sourced from the BuildInstruction step" comment is incorrect for `call_action` — these are inside Action steps (class 16), not Recipe BuildInstructions. |
| `FIND-P7-14` | **LOW** | Phase F / `assemble_from_component_items` shape | Live code at `orchestrator.rs:2680-2721` confirms: the override branch (line 2686-2691) returns `formatted_content` as a **prose string** (`item.effective_content`). The normal assembly branch (line 2717-2721) returns `formatted_content` as a **JSON string** `{"prior_knowledge": [...], "matched_components": [...]}`. The plan's FINDING F description is confirmed accurate. Phase F must change normal assembly to also produce a prose string. | Confirmed accurate. FINDING F is correct. |
| `FIND-P7-15` | **LOW** | §0.9 / `default.py` `__assemble_prior_knowledge__` return shape | The existing `default.py:24-28` comment (registered host functions list) shows `__assemble_prior_knowledge__` currently returns `{content, formatted_content, override_prompt_creation, matched_component_ids}` — exactly 4 fields. The v3 plan extends this to also carry `action_short_circuit`, `tier_zero`, etc. This is documented correctly. | Confirmed accurate. |
| `FIND-P8-01` | **CRITICAL** | Phase F / `Thread` struct has NO `tenant_id` or `agent_id` fields | Reading the live `crates/brassclaw_engine/src/types/thread.rs` (245 lines, full file): the `Thread` struct (line 212) has `id`, `goal`, `title`, `thread_type`, `state`, `project_id`, `user_id`, `parent_id`, `config`, `messages`, `internal_messages`, `events`, `capability_leases`, `metadata`, `created_at`, `updated_at`, `completed_at`, `step_count`, `total_tokens_used`, `total_cost_usd` — **NO `tenant_id` field, NO `agent_id` field**. The code comment at `orchestrator.rs:2575-2579` already says "Phase 2+ / v3 Phase F will tighten this once the full 4-tuple is threaded through `Thread`". So the plan's Phase F instruction to "add `tenant_id` and `agent_id` to `Thread`" is confirmed as the right path — and it is non-trivial: `Thread::new` signature must change (adds 2 required params or uses builder pattern), **every `Thread::new` call site in the codebase** must be updated, and `#[serde(default)]` is required on both fields for checkpoint compatibility. A second stub is also present at `orchestrator.rs:3142-3145` (the `__list_skills__` / `scope_from_thread_ids` helper) — Phase F must fix BOTH stubs, not just the `handle_assemble_prior_knowledge` one. | Phase F section updated: (1) confirmed `Thread` has no `tenant_id`/`agent_id` — this is the primary work of Phase F; (2) documented the SECOND stub at `orchestrator.rs:3142-3145` that must also be fixed; (3) added explicit note that `Thread::new` signature change requires a codebase-wide grep for all construction sites. |

### Pass 6 findings (full codebase re-read — new issues found, not in any prior pass)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-P6-01` | **CRITICAL** | Phase E.0 | `ExecutionLoop` already has `.with_pg_pool(pool: Arc<brassclaw_pg::PgPool>)` at `loop_engine.rs:207` (under `#[cfg(feature = "skills-db")]`). The plan correctly says "add `pg_pool` field to `ThreadManager`" but fails to mention that the pool must be **feature-gated** (`#[cfg(feature = "skills-db")]`) because `brassclaw_pg` is an optional dep in `brassclaw_engine/Cargo.toml` (`skills-db = ["dep:brassclaw_pg", ...]`). A build without `skills-db` will fail to compile if `ThreadManager` holds an `Arc<brassclaw_pg::PgPool>` without the gate. Additionally, `ExecutionLoop::with_pg_pool` already exists and already flows into the spawn path — the correct E.0 fix is: (1) add `#[cfg(feature = "skills-db")] pg_pool: Option<Arc<brassclaw_pg::PgPool>>` to `ThreadManager`, (2) in the spawn path (`manager.rs:382-383`), if `self.pg_pool.is_some()` then call `.with_retrieval_source(Arc::new(PostgresSource::new(pool.clone())))` INSTEAD of the RamSource line. The existing `.with_retrieval_source()` call at line 400 remains; the only change is which source is passed. | Phase E.0 "Files to modify" updated: feature gate requirement added; exact spawn-path change described (replace `RamSource` line 382-383 with conditional; keep `.with_retrieval_source()` at line 400 unchanged). |
| `FIND-P6-02` | **CRITICAL** | Phase B/C / V052/V053 + Phase L (V057) | Phase L's `builtin_bootstrap.rs` inserts Recipes with `source = 'system'`. V057 adds `'system'` to the CHECK constraint on `reborn_tools`, `reborn_tool_skills`, and `reborn_skills` — but **NOT** `reborn_recipes`. The `reborn_recipes` table (V033:113) has `source TEXT NOT NULL DEFAULT 'authored'` with no CHECK constraint — so `'system'` happens to work today because there's no CHECK. However, for correctness and documentation V057 should explicitly allow `'system'` on `reborn_recipes` too. Additionally, the new Phase B/C tables `reborn_python_code` and `reborn_extension_catalogues` (V052/V053) will also need `source = 'system'` for Phase L's seeder — these new tables must include `'system'` in their own `source` CHECK constraint at creation time (V052/V053). **Do NOT wait for V057 to allow it.** (V-numbers updated per Decision 2: was V051/V052/V056.) | V057 updated to also add `'system'` to `reborn_recipes` CHECK constraint (for documentation/correctness). Phase B/C migration instructions updated: V052 and V053 must include `CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'))` on their `source` columns from day one. |
| `FIND-P6-03` | **CRITICAL** | Phase A / `types/recipe.rs` import | `RecipeVariant` in `types/recipe.rs` imports `VariablePattern` from `crate::memory::instruction_builder`. The direction `types → memory` is unusual (normally memory imports from types). The circular dependency risk was analysed and the clean solution adopted: **`VariablePattern` (and the `ToolBinding` / `ErrorPolicy` types it logically groups with) is moved to a new `crates/brassclaw_engine/src/types/ibs.rs` module** — a sibling of `recipe.rs` in the `types/` directory. Both `instruction_builder.rs` (memory) and `types/recipe.rs` then import from `types::ibs`, which has no reverse dependency on either. This is the correct dependency direction (`types → (nothing)`, `memory → types`). **`VariablePattern` does NOT live in `instruction_builder.rs`.** | Phase A fully updated: `types/ibs.rs` new file listed under "Files to create"; `FIND-NEW-01` note revised to reflect that `VariablePattern`/`ToolBinding`/`ErrorPolicy` move to `types/ibs.rs`, NOT `instruction_builder.rs`; `instruction_builder.rs` imports from `crate::types::ibs`; `types/recipe.rs` imports from `crate::types::ibs` (same crate, no cycle); `types/mod.rs` gains `pub mod ibs`. |
| `FIND-P6-04` | **CRITICAL** | Phase A + Phase N | `RECIPE_SELECT` currently reads 31 columns (indices 0–30). Phase A appends 3 columns at indices 31, 32, 33. Phase N later DROPS 5 columns from the middle (`validation_errors`=22, `review_feedback`=23, `review_attempts`=24, `rejected_at`=25, `queue_code`=26). After the drops, the original 5 fields at indices 22–26 are gone, shifting `source`=27→22, `content_hash`=28→23, `created_at`=29→24, `updated_at`=30→25, and the Phase A fields `step_descriptions`=31→26, `variants`=32→27, `dependency_registry`=33→28. `decode_recipe_row` must be completely re-indexed in Phase N. **The plan's Phase N.4 says "re-index `decode_recipe_row`" but does not provide the exact new index mapping.** This must be explicit or the implementer will produce a silent off-by-one. | Phase N.4 "Files to modify" updated: explicit re-index table provided, showing every `row.get(N)` call's new index value after the 5 drops. |
| `FIND-P6-05` | **CRITICAL** | Phase D / LEFT JOIN security | `resolve_intent` SQL extended in Phase D to LEFT JOIN `reborn_actions` for `component_name`. The LEFT JOIN MUST include scope filters: `AND a.tenant_id = $1 AND a.user_id = $2 AND a.agent_id = $3 AND a.project_id = $4` — without these, a match on `component_id` could return the Action name from a different tenant's row (cross-scope leakage). The plan's §0.8 SQL snippet DOES show these filters but they are never called out as a **security requirement** — an implementer who simplifies the JOIN may silently drop them. | Phase D "Files to modify" updated: add explicit security note — "the LEFT JOIN on `reborn_actions` MUST include all 4 scope parameters; omitting them is a cross-tenant information-leakage bug." |
| `FIND-P6-06` | **HIGH** | Phase A / `types/recipe.rs` + `instruction_builder.rs` | The plan does not specify where `VariablePattern` is deserialized from JSONB into a typed `Vec<VariablePattern>`. `PostgresSource::fetch_for_turn` (Phase E) fetches the `step_descriptions` JSONB column and calls `build_instruction(step_link, step_descriptions, variable_patterns)`. The `variable_patterns` are stored nested inside each `RecipeVariant` in the `variants` JSONB column — not as a top-level column. Phase E must deserialize `variants` → `Vec<RecipeVariant>` → find the matching variant → extract `variable_patterns`. This deserialization step is entirely unspecified in Phase E's "Files to modify" section. | Phase E "Files to modify" updated: add step "2a. Deserialize `variants` JSONB column → `Vec<RecipeVariant>`, find variant matching `step_link` (or use the matching variant's `variable_patterns`), pass to `build_instruction`." |
| `FIND-P6-07` | **HIGH** | Phase B / `interceptor_config_service.rs::class_label` | The function at `interceptor_config_service.rs:65` is an incomplete stub: it covers only `0, 1, 9, 10, 12, 13, 14, 15, 16, 18, 19, 20, 21, 50` — classes 2, 3, 4–8, 11, 17 all fall through to `"Component"`. This is pre-existing technical debt. Phase B/C only adds 22 and 23 to this function, which is correct. But an implementer who reads this function will be surprised that it's incomplete. | Pre-existing stub noted in Phase B/C "Files to modify": "Note: `interceptor_config_service.rs::class_label` is a stub that omits classes 2, 3, 4–8, 11, 17 (pre-existing debt). Do not fix those gaps in Phase B/C — add only 22 and 23 as specified." |
| `FIND-P6-08` | **HIGH** | Phase N / V059 populate SQL | The example SQL `INSERT INTO reborn_validation_queue ... SELECT ... 1,  -- class_code FROM reborn_skills` hardcodes class_code as `1`. But `reborn_skills` has class_code values 1, 2, OR 3 (skill_rusty/monty/llm). The per-table arm must read `class_code::SMALLINT` from the source table, not a hardcoded literal. Similarly, `reborn_extensions_unified` has class codes 4–9. For tables with a single fixed class code (e.g., `reborn_recipes` = 21, `reborn_actions` = 16, `reborn_tools` = 0) a literal is correct. For multi-class-code tables (skills, extensions) the actual column value must be used. (V-number updated per Decision 2: was V058, now V059.) | Phase N.1 V059 populate SQL example updated: `class_code` expression corrected to `class_code::SMALLINT` for variable-class tables (`reborn_skills`, `reborn_extensions_unified`) and literal for fixed-class tables. |
| `FIND-P6-09` | **MEDIUM** | Phase E.0 / `brassclaw_engine` feature gates | The plan says `PostgresSource::new(pool.clone())` is built in the spawn path under `#[cfg(feature = "skills-db")]`. The entire `PostgresSource` struct and its impl are already `#[cfg(feature = "skills-db")]`. The plan's ARCH-01/ARCH-02 notes reference this but never explicitly state that the `ThreadManager` pool field AND the spawn-path conditional BOTH require the feature gate. Without explicit instruction, an implementer may add the field unconditionally and hit a compile error in non-skills-db builds. | Phase E.0 "Concrete changes required" Option A updated: `#[cfg(feature = "skills-db")]` gate required on both the `pg_pool` field and the `PostgresSource` construction in the spawn path. |
| `FIND-P6-10` | **MEDIUM** | Phase A / `Recipe::from_metadata` | `Recipe::from_metadata` deserializes the whole `Recipe` struct from a `serde_json::Value`. After Phase A adds `variants`, `step_descriptions`, `dependency_registry` with `#[serde(default)]`, existing DB rows that store the old metadata (without these fields) will deserialize with defaults — that's correct. But existing metadata stored as `MemoryDoc.metadata` (the legacy `StoreBackedRecipeStore` path) may not have these fields. The `#[serde(default)]` handles this silently. This is not a bug but must be noted as intentional. | Phase A "Files to modify" (types/recipe.rs section) updated: note that `#[serde(default)]` on all three new fields is required for backward compatibility with legacy `MemoryDoc.metadata` round-trips via `Recipe::from_metadata`. |

### Pass 5 findings (full source read — new issues found)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `FIND-P5-01` | **HIGH** | Phase B/C / FIND-20 | `recipe_store.rs:861` `class_label` function uses *completely different* display labels from what the plan assumes: `13 => "Guide"`, `14 => "Reference"`, `15 => "Note"`, `17 => "Template"`, `18 => "Snippet"`, `19 => "Config"`, `20 => "Workflow"` — these are user-facing display labels, not class-name labels. The plan says add `22 => "PythonCode".to_string()` and `23 => "Catalogue".to_string()` — these are consistent with the single-word entries (`"Tool"`, `"Action"`, `"Recipe"`) in that function, not the descriptive entries (`"Skill (Rusty)"`, `"Guide"`). Actual Phase B/C change is correct as stated but the test assertion wording in the plan ("title-case style") is imprecise. | Phase B/C test assertions confirmed correct. Note added in FIND-20 that `recipe_store.rs` is a display-label function and the values are per the existing pattern (some descriptive, some name-based). |
| `FIND-P5-02` | **HIGH** | Phase E.0 / ARCH-02 | `ThreadManager::new` is called in 8 places: `manager.rs:1132/1160/1192` (test helpers/internal), `mission.rs:3916/4250`, `conversation.rs:856/925/1041`. Phase E.0 must add `pg_pool: Option<Arc<brassclaw_pg::PgPool>>` to the `ThreadManager` struct AND update all 8 construction sites to pass `None` (or the pool for the composition path). A `with_pg_pool` builder method is sufficient for the composition injection — the test construction sites pass `None` explicitly. The plan only mentions `mission.rs` and `conversation.rs` but misses the 3 `manager.rs` internal test-helper construction sites. | Phase E.0 "Files to modify" updated with all 8 `ThreadManager::new` call sites. |
| `FIND-P5-03` | **HIGH** | Phase A / §0.3 | `RecipeVariant` struct is defined in two places in the plan: (1) in "Files to create" (instruction_builder.rs section) — NOT actually listed, only `VariablePattern` is mentioned; (2) in "Files to modify" (types/recipe.rs section) — two slightly different definitions appear: the first adds `step_link: Option<String>` with `name` field, the second adds `variant_key: String` with `label` field. These are inconsistent. The canonical definition must be the `types/recipe.rs` one (stored in DB). | Reconciled: `RecipeVariant` in `types/recipe.rs` uses `variant_key: String` (the v2/legacy human label), `step_link: Option<String>`, `intent_examples: Vec<String>`, `variable_patterns: Vec<VariablePattern>`. `VariablePattern` is in `instruction_builder.rs`. No `label` field — use `variant_key` only. Phase A "Files to modify" canonical definition retained; the earlier non-canonical definition in the `instruction_builder.rs` section removed. |
| `FIND-P5-04` | **MEDIUM** | Phase H / FIND-18 | The `consume_drainable_inputs` return type (confirmed as `Result<(bool, Vec<LoopInputAckToken>, Option<LoopCancelledReasonKind>), AgentLoopExecutorError>`) would become `Result<(bool, Vec<LoopInputAckToken>, Option<LoopCancelledReasonKind>, Option<LoopMessageRef>), AgentLoopExecutorError>` under Option A. The plan's FIND-18 description is correct but never states the exact new return type signature. | Phase H item 2 "Option A" now shows the exact new return type. |
| `FIND-P5-05` | **MEDIUM** | §5 / Tier 1 diagram | The Tier 1 turn-flow diagram in §5 carries a `⚠️ FIND-16 FIX` note saying "the diagram is in the wrong order" but keeps the wrong diagram "for backward reference." An implementation plan should not contain known-incorrect diagrams. An implementer who only reads the diagram (not the note) will implement the wrong execution order. | §5 Tier 1 diagram redrawn with the correct order (PromptStage calls run_step_zero → Python step-0 → pkr returned → LLM called). The old wrong diagram is removed. |
| `FIND-P5-06` | **MEDIUM** | Phase E / `fetch_for_turn` | When `class_code == 16` (Action) match fires in Phase E, the current code (`retrieval_source.rs:516-525`) calls `fetch_component_by_id(pool, scope, component_id, 16)` and returns `Components([item])`. Phase E adds `ActionShortCircuit` but must detect class-16 BEFORE calling `fetch_component_by_id`. The plan implies this correctly but does not explicitly state the detection must happen at the `class_code` dispatch level immediately after `resolve_intent` returns, BEFORE the `fetch_component_by_id` call. | Phase E "Files to modify" now explicitly states: add class-16 detection IMMEDIATELY after `resolve_intent` returns `Match`, before the `fetch_component_by_id` call. The existing call that goes to `fetch_component_by_id` for class-16 must be replaced by `ActionShortCircuit` return. |
| `FIND-P5-07` | **LOW** | Phase A / `RECIPE_SELECT` | The plan says `RECIPE_SELECT` currently selects 31 columns ending at `updated_at` (index 30). Confirmed: `decode_recipe_row` reads indices 0–30 (31 fields), with `created_at` at index 29 and `updated_at` at index 30. The three new columns appended at indices 31/32/33 are correct as stated. | Confirmed correct. No change needed. |
| `FIND-P5-08` | **LOW** | Phase N.4 | The plan says `recipe_matcher.rs` "references `validation_errors` in some paths — audit required." Actual required fix: after N.4 column drops, `recipe_matcher.rs` must be checked at compile time — the compiler will catch any struct field references that no longer exist after the Rust struct fields are removed. The "audit required" is satisfied by compiling with zero warnings after the Phase N changes. | Phase N.4 updated to note that the compiler catches all Rust-struct-level references; the "audit required" is the `cargo check` step, not a manual audit. |


### Pass 3 findings (deep cross-check — verified against full source)

| Tag | Location | Finding | Fix Applied |
|-----|----------|---------|-------------|
| `FIND-17/28` | Phase H §H0 H3 | `LoopContextPort` (host.rs:778) has only ONE method (`load_loop_context`) and is a **required** trait — always present in `AgentLoopDriverHost` supertrait (line 2187). Adding `resolve_message_text` without a default impl breaks ALL existing host implementors. | Phase H item 2 updated: `resolve_message_text` must have a `Err(Unimplemented)` default body in the trait definition |
| `FIND-18/25` | Phase H item 2 | `consume_drainable_inputs` (input.rs:154) is a **pure free function** with no `ctx`. There is no "drain function". The matching drain-mode inputs (lines 169–173) do `consumed_len += 1; continue` — `message_ref` is NEVER captured. | Phase H item 2 rewritten with Option A (return last ref from free function) / Option B (pre-scan in `InputStage::process`) |
| `FIND-19` | Phase H item 6 | `LoopPromptBundleRequest` (host.rs:987) has 7 fields confirmed. Test mock at `support.rs:340` is one known construction site. All construction sites need `recipe_hint: None`. | Phase H item 6 updated with grep command and known site list |
| `FIND-20` | Phase B, C | Three `class_label` copies have DIFFERENT label styles: `intent_system.rs` = lowercase snake_case, `interceptor_config_service.rs` = `&'static str` title-case, `recipe_store.rs` = `String` title-case. | Phase B+C "Files to modify" and test assertions updated with correct labels per copy |
| `FIND-21` | Phase A | INSERT at `pg_recipe_store.rs:261–283` uses 13 columns (`$1`–`$13`). After Phase A additions → 16 columns (`$1`–`$16`). Off-by-one causes tokio-postgres panic. | FIND-07 note extended with verified column counts |
| `FIND-22` | Phase E.0 | `factory.rs` has ZERO direct references to `ThreadManager`/`MissionManager`/`ConversationManager`. Engine managers are built via `services.host_runtime_for_local_testing()`. | Phase E.0 Option A/B step 4 updated: "trace the builder API — no direct ThreadManager call in factory.rs" |
| `FIND-23` | Phase D | `retrieval_source.rs:516–519` inline destructures `IntentResolution::Match { component_id, component_class_code }` — will compile-fail after Phase D adds `step_link`. | Phase D "call sites" section updated with exact line and required binding |
| `FIND-26` | §2 table | Migration table showed old filename `V054__reborn_skills_intent_examples.sql`. | §2 table row updated: `V055__reborn_dependency_registry.sql` (V-number updated per Decision 2; was `V054` after Round 1, now `V055` after Round 2). |
| `FIND-27` | Phase H item 3 | Pseudocode code fence still contained `retrieval_source.fetch_for_turn(...)`. FIND-10 fixed the note but not the fence itself. | Pseudocode corrected to `ctx.host.fetch_for_turn(context, ...)` |
| `FIND-08` (addendum) | Phase E | Clarified that `FetchForTurnResult` / `TurnRoutingSignals` / `ActionShortCircuit` / `SplitResult` are **greenfield new types** (not extensions of existing types). | FIND-08 addendum added in Phase E "Files to modify" |

### Pass 4 findings (5-subagent full re-review — each verified against live source)

| Tag | Severity | Location | Finding | Fix Applied |
|-----|----------|----------|---------|-------------|
| `DRIVER-GAP-MODEL-A` | **CRITICAL** | Phase H / §0.3 / §0.9 / §5 / DRIVER-PREREQ | A prior draft of the DRIVER-GAP resolution claimed Tier 0 "works today" by reusing `override_prompt_creation: true` to skip `__llm_complete__`. FALSE: `default.py:998-1008` only swaps `working_messages` and FALLS THROUGH to `__llm_complete__` (`:1103`); the only pre-`__llm_complete__` return is the DEAD `__retrieve_docs__`+`class_code==16` shim (`:1018-1027`; §0.9 Problem 1 — never fires). So Tier 0 does NOT work today. | 7 plan locations corrected: a DEDICATED `tier_zero: true` pkr signal (NOT `override_prompt_creation` — that is the Solution-Override LLM path) emitted by `handle_assemble_prior_knowledge` (`orchestrator.rs:2552`) when `SplitResult.llm_call_required == false`, + a NEW `if pkr.get("tier_zero"): return execute_recipe_orchestrator_channel(...)` early-return branch in `default.py` step-0 (sibling of `action_short_circuit`, before `__llm_complete__`). Genuinely NEW wiring landing in Phase H. |
| `PARAM-GAP` | **CRITICAL** | §0.17.1 / Phase M.3 | The `resolve_intent` three-path SQL wrote `ANY($7)` (skipping `$6`) and `CASE ... WHEN $8/$9/$10` (10 params, off-by-one). `tokio_postgres` binds positionally → runtime `parameters ... but statement requires ...` crash. | §0.17.1 SQL renumbered to `ANY($6)` + CASE `$7/$8/$9` = 9 params, matching live `intent_system.rs:339-367`. Added a bind-arg order table + carry-forward note in Phase M.3. |
| `VARPAT-COL-GAP` | **CRITICAL** | Phase A / V050 / §2 | `variants` JSONB had NO migration anywhere in V050–V058, yet Phase A persists `variants` and Phase M.5 depends on `variable_patterns` nested in `variants`. A SELECT on a non-existent column is a hard runtime SQL error. | V050 now creates ALL three Phase A store columns (`step_descriptions`, `variants`, `dependency_registry`) on `reborn_recipes`. §2 table row updated. |
| `DEPREG-TIMING-GAP` | **CRITICAL** | Phase A / V050 / V055 | `dependency_registry` on `reborn_recipes` is not created until V055 (Phase J.2, was V054 before Decision 2), but Phase A's store round-trips it from V050 → between V050 and V055 the SELECT reads a non-existent column. | Folded into the V050 fix (V050 creates `dependency_registry` on recipes); V055's recipe line is `IF NOT EXISTS` → idempotent no-op. V055 comment updated. |
| `FIND-NEW-01` | **CRITICAL** | Phase A | IBS types (`BuildInstruction`, `RecipeStepType`, `StepOwner`) were listed in BOTH `instruction_builder.rs` (create) and `types/recipe.rs` (modify) — ambiguous duplicate home; `ToolBinding`/`ErrorPolicy` only in `types/recipe.rs` but belong with `IbsRecipeStep`. **UPDATED (Decision 1):** `VariablePattern`, `ToolBinding`, and `ErrorPolicy` are DATA-MODEL types (persisted in JSONB / used in the Recipe model) — they belong in `types/`, not `memory/`. The clean home is a new `crates/brassclaw_engine/src/types/ibs.rs` module. The IBS BUILDER types (`BuildInstruction`, `RecipeStepType`, `StepOwner`, `IbsRecipeStep`, `IbsError`, `DependencyExpr`, `DependencyNode`, `StepDescriptionEntry`, `StepContextSpec`) remain in `instruction_builder.rs` as the sole home for build-phase logic. Summary of type homes after Decision 1: `types/ibs.rs` = `VariablePattern` + `ToolBinding` + `ErrorPolicy`; `types/recipe.rs` = `RecipeVariant` + three `Recipe` fields; `memory/instruction_builder.rs` = all builder / error / IBS-output types. | Phase A fully updated: `types/ibs.rs` new file added to "Files to create"; `instruction_builder.rs` "Files to create" block revised (removes `VariablePattern`/`ToolBinding`/`ErrorPolicy`, adds import from `crate::types::ibs`); `types/recipe.rs` "Files to modify" block revised (imports `VariablePattern` from `crate::types::ibs`, not `crate::memory::instruction_builder`); `types/mod.rs` gains `pub mod ibs`. |
| `FIND-NEW-02` | HIGH | §0.4 | The BuildInstruction structure diagram labelled both channel steps `RecipeStep`, but Phase A renames the IBS per-step struct to `IbsRecipeStep` (collision with v2 `RecipeStep`). | Diagram corrected to `IbsRecipeStep` (both channels) + FIND-NEW-02 callout. |
| `FIND-NEW-03` | HIGH | Phase D | "Update `seed_intent_input` to accept and store `step_link`" was underspecified — no signature, column list, placeholder count, or ON CONFLICT clause. | Fully specified: 8th param `step_link: Option<&str>`, INSERT 12 cols / 11 placeholders (`$11`), `ON CONFLICT ... DO UPDATE SET ... step_link = EXCLUDED.step_link`, all callers must pass new arg. Verified against `intent_system.rs:463-505`. |
| `EXT-NAME` | HIGH | Phase J.2 / §2 / N.4 | The extensions table is `reborn_extensions_unified` (`V032:57`), but the J.2 list + N.4 prose used the bare `reborn_extensions` → V055 `ALTER TABLE` / V059 `DROP COLUMN` would fail (`relation does not exist`). (V-numbers updated per Decision 2.) | Both bare refs corrected to `reborn_extensions_unified` + EXT-NAME callout in J.2. |
| `SCHEMA-01` (incomplete) | MEDIUM | Phase N.4 | The SCHEMA-01 note named only `reborn_recipes` as SMALLINT; actually 10 tables use SMALLINT (extensions_unified, recipes, specs, tool_skills, plans, summaries, docus, lessons, issues, notes) and 3 use INT (skills, actions, tools). | Note expanded with the full verified split; cleanest fix `COALESCE(review_attempts::INT, 0)` on EVERY arm (uniform; `INT::INT` no-op). Populate SQL example updated. |
| `J.3-XREF` | LOW | Phase J.3 | J.3 `resolve_dependencies` fetches class-22/23 deps via `fetch_component_by_id`/`class_label`, but didn't cross-reference where those 22/23 arms land. | J.3 cross-reference note added (Phase E arms + Phase B/C `class_label`). |
| `PGRECIPE-LINE` | LOW | Phase A | `PgRecipe` struct cited as "line ~110"; verified at `pg_recipe_store.rs:91`. | Cite corrected to ~91. |
| `HANDLE-ASSEMBLE-LINE` | LOW | Phase H | `handle_assemble_prior_knowledge` def verified at `orchestrator.rs:2552` (dispatched at :563, defined at :795 was a prior conflation). | Cite made consistent (2552) across the DRIVER-GAP corrections. |

### Pass 2 findings (first deep-read pass)

| Tag | Location | Finding | Fix Applied |
|-----|----------|---------|-------------|
| `ARCH-01` | Phase E.0 | Plan said "`ExecutionLoop::with_retrieval_source` is internal-only, not callable from composition" but didn't clarify that `with_retrieval_source` already EXISTS on `ExecutionLoop` (at `loop_engine.rs:219`) and is already called at `manager.rs:400`. The task is to add the override field to `ThreadManager`, not `ExecutionLoop`. | Phase E.0 "Files to modify" rewritten with verified method location |
| `ARCH-02` | Phase E.0 | Plan stated injection site is `crates/brassclaw_reborn_composition/src/runtime.rs`. `ThreadManager` does NOT appear there — it is instantiated in `mission.rs` / `conversation.rs`. | Phase E.0 now explicitly warns the injection site claim is wrong; implementer must trace `ThreadManager::new()` calls |
| `SCHEMA-01` | Phase N.4 | `reborn_recipes` (V033) uses `SMALLINT` for `review_attempts`, but `reborn_skills` (V027) uses `INT`. V059 populate uses `COALESCE(review_attempts, 0)` — needs `::INT` cast for SMALLINT tables. (V-number updated per Decision 2: was V058, now V059.) | Added as a note under N.4 struct audit section |
| `RETRIEVAL-01` | §0.11 / K.3 | Plan stated `doc_type_weight(DocType)` was "already removed in Step 12 pass." It was NOT — file `retrieval_dbless.rs` still exists and contains the function (lines ~76–88). `doc_type_weight_by_class(i32)` (the i32 variant) does not exist. | §0.11 review note corrected; K.3 deletion instructions updated |
| `PERF-03` | Phase E | Plan said `fetch_for_consumer` has "currently 9 sub-selects." Verified: it has **12** (reborn_skills, reborn_extensions_unified, reborn_actions, reborn_specs, reborn_tool_skills, reborn_plans, reborn_summaries, reborn_docus, reborn_lessons, reborn_issues, reborn_notes, reborn_recipes). Adding classes 22/23 → 14. | PERF-03 count corrected |
| `CANONICAL-01` | Phase H §4 | The canonical.rs `RecipeStep::Continue` match is exhaustive (single variant). This will be a compile error when Phase H adds `TierZero` and `ActionExecuted`. | Added verification note under COMP-02 |
| `FINDING-A` | Phase D | `record_disambiguation_choice` line reference was "436" — verified actual body is at lines 433–455. Return statement at line 451. | Updated to exact verified line numbers |
| `RECIPE-SELECT` | Phase A | Plan cited wrong "line 117/147/208/219" for PgRecipe/NewPgRecipe/RECIPE_SELECT/decode_recipe_row. Verified: PgRecipe starts at ~110, NewPgRecipe at ~145, RECIPE_SELECT at ~208, decode_recipe_row at ~219. | Phase A "Files to modify" updated with verified locations + instruction to append new columns at END of RECIPE_SELECT (not middle) |
| `SQLX-01` | Phase N.3 | Cache-check code example used `sqlx::query_scalar!` macro. The codebase uses `tokio-postgres` / `deadpool-postgres`, not sqlx. | N.3 code block replaced with correct `pool.get()` + `client.query_opt()` pattern |
| `TIER0-GAP` | Phase H / Q14 | Plan does not specify how the Python scripting engine is "kicked" in Tier 0 (when CapabilityStage normally reacts to model output, not invents it). | **✅ RESOLVED** — Option 1 chosen: a new `TierZeroExecutionStage` inserted between `RecipeStage` and `AssistantReplyStage` in `canonical.rs` invokes the Python orchestrator in no-LLM mode via the new `LoopOrchestratorPort` host port. `CapabilityStage` is NOT bent (it keeps its model-output assumption). See Phase H.0 §H5 and the §5 Tier 0 diagram |
| `DRIVER-GAP` | Phase H / §0.3 / §5 | **Root cause behind TIER0-GAP.** The production turn driver is the engine `ExecutionLoop::run` (`loop_engine.rs:413`), which runs the Python `execute_orchestrator` directly with NO stage pipeline — Python (Monty) IS the outer loop and calls the LLM itself via `__llm_complete__` (`default.py:1103` → `handle_llm_complete`, dispatched at `orchestrator.rs:563`, defined at `orchestrator.rs:795`). The agent-loop `DefaultExecutorPipeline::execute` (`canonical.rs`) — where `RecipeStage`/`PromptStage`/`ModelStage`/`CapabilityStage` live — is a SKELETON: no product surface calls it (`DefaultExecutorPipeline`/`execute_family` appear only inside `brassclaw_agent_loop`). `brassclaw_agent_loop` does NOT depend on `brassclaw_engine`, and `__assemble_prior_knowledge__` exists ONLY in `brassclaw_engine`. So the plan's RecipeStage↔Python-step-0 stash/unstash (Phase H item 5) and the §5 Tier 0 diagram ("Python runs `default.py` step 0" inside the agent loop) both assume a RecipeStage-then-Python unification that does NOT exist. **The plan was silently mixing two execution models:** (A) `ExecutionLoop`/Monty = Python is the outer loop, calls `__llm_complete__`; (B) `DefaultExecutorPipeline`/agent_loop = no Python, `ModelStage` calls the LLM. | **✅ RESOLVED — model selection made explicit, both paths covered during migration.** **Engine path (A, current production):** ⚠️ Corrected — a prior draft wrongly claimed Tier 0 "works today" by reusing `override_prompt_creation: true` to skip `__llm_complete__`. That is FALSE: `override_prompt_creation: true` (`default.py:998-1008`) only swaps `working_messages` and FALLS THROUGH to `__llm_complete__`; the only pre-`__llm_complete__` return today is the DEAD `__retrieve_docs__`+`class_code==16` shim (`default.py:1018-1027`; §0.9 Problem 1 — never fires). So Tier 0 does NOT work today. Phase H adds a DEDICATED `tier_zero: true` pkr signal (NOT `override_prompt_creation` — that is the Solution-Override LLM path) emitted by `handle_assemble_prior_knowledge` when `SplitResult.llm_call_required == false`, plus a NEW `if pkr.get("tier_zero"): return execute_recipe_orchestrator_channel(...)` early-return branch in `default.py` step-0 (sibling of the `action_short_circuit` return, before `__llm_complete__`), generalising the `execute_action_procedure` (`default.py:901`) no-LLM pattern from class-16 Actions to Tier-0 Recipes. This is genuinely NEW wiring landing in Phase H. **Agent-loop path (B/C, target state):** `LoopOrchestratorPort` (15th `AgentLoopDriverHost` port, implemented by `brassclaw_reborn_composition` — the only crate depending on both `brassclaw_engine` and `brassclaw_agent_loop`) + `TierZeroExecutionStage` bridge agent-loop stages to the engine orchestrator (`run_step_zero` Tier 1; `run_tier_zero` Tier 0). Active after the agent-loop becomes the driver (`DRIVER-PREREQ`). **LLM-call ownership (v3 target):** once the agent-loop is the driver, `ModelStage` owns the Tier 1+ LLM call and the Python `__llm_complete__` loop is retired (Python reduced to step-0 prior-knowledge + Tier-0 no-LLM execution). During migration both mechanisms coexist: engine path serves production (Tier 0 via `tier_zero` once Phase H lands it), agent-loop stages are test-only until switchover (B/C). |
| `TIER0-ICS` | §5 Turn Flow | The Tier 0 turn flow incorrectly showed `InterceptorStage` running normally. Per COMP-07, InterceptorStage MUST be skipped in Tier 0 (it would open a ForensicPacket without a model call to close it). | Tier 0 flow diagram updated |
| `DOCTYPE-B` | §0.11 FINDING B §3 | "weight dispatch function itself is already gone" — wrong. `doc_type_weight(DocType)` still exists. Only the i32-keyed variant doesn't. | FINDING B item 3 corrected |
| `LOOPSTATE` | Phase H §1 | Added explicit verification that `LoopExecutionState` has NO `last_user_text`, `recipe_rust_context`, or `recipe_hint` fields currently. | Phase H state.rs addition note augmented |
| `HOSTPORTS` | Phase H §H.0 | Added exact verified list of all 13 current `AgentLoopDriverHost` supertrait ports, with instruction to add `LoopRetrievalPort` as 14th entry after `LoopInterceptorPort`. | Phase H H.0 retrieval port note updated |

---

## 0. Architecture Vision (Canonical Reference)

### 0.1 Component Hierarchy — Bottom to Top

Reading bottom-up is the correct direction. Every higher layer is composed *of* the layers below it.

```
┌─────────────────────────────────────────────────────────────────┐
│  ExtensionCatalogue (class 23)                                  │
│  Domain overview. task_groups[] → recipe names. Never re-docs.  │
├─────────────────────────────────────────────────────────────────┤
│  Recipe (class 21)                                              │
│  Primary intent target. One RecipeVariant per distinct intent.  │
│  Each variant owns: intent_examples[], step_link formula,       │
│  variable_patterns[], and StepDescriptions (the authoring       │
│  source from which the BuildInstruction is assembled by the IBS)│
├─────────────────────────────────────────────────────────────────┤
│  Skill (classes 1–3)    │  PythonCode (class 22) [NEW]          │
│  Orchestrator instruct. │  Python utilities / inline instruct.  │
│  for using one Rust tool│  for the orchestrator. Not full Skill.│
├─────────────────────────────────────────────────────────────────┤
│  ToolSkill (class 13)                                           │
│  Rust-layer only. param schema, preconditions, error handling.  │
│  The orchestrator never reads ToolSkill bodies directly.        │
├─────────────────────────────────────────────────────────────────┤
│  Tool (class 0)                                                 │
│  Rust execution layer only. No prompt text. Opaque to the       │
│  orchestrator. Excluded from all retrieval queries.             │
└─────────────────────────────────────────────────────────────────┘
```

**Runtime Extensions (classes 4–9)** remain as-is: MCP servers, Rusty capabilities,
Monty plans, LLM prompt templates. They are NOT documentation containers.

**ExtensionCatalogues (class 23)** are the documentation namespace. Separate class,
separate table, separate concern.

---

### 0.2 ExtensionCatalogue — Correct Design

An ExtensionCatalogue does **not** re-document commands. Every component it owns already
documents itself. The Catalogue draws the **bigger picture**:
> "This catalogue covers local file management. Its Recipes handle these task groups..."

| Section | Content |
|---------|---------|
| `name` | Catalogue identifier |
| `version` | Semver-like label |
| `description` | One-paragraph summary for LLM fallback context |
| `task_groups[]` | `{ group_name, summary, recipe_ids[] }` |
| `child_component_ids[]` | All owned component UUIDs (any class) for lineage |
| `intent_index[]` | Audit-only — never seeded into `reborn_intent_inputs` |

---

### 0.3 Recipe — Correct Design

A Recipe is a **complete turn script**. It is the primary intent target.

**Important — current vs. target state:**  
The live `Recipe` struct in `crates/brassclaw_engine/src/types/recipe.rs` is the
**v2 design**: `RecipeStep { skill: String, tool: String, params, description }`.
There is no `RecipeVariant`, `BuildInstruction`, `StepDescription`, or `step_link`.
Phase A establishes the v3 types. The existing `trigger` + `steps` fields are
**preserved** as the Tier-1 / Tier-2 fallback so old Recipes continue to work.

#### How a Recipe works (v3 complete flow)

```
Author:
  1. Author writes StepDescriptions in WebUI (YAML-structured, human-readable).
  2. Each intent expression gets a step_link pointing into StepDescriptions.

Intent match (runtime):
  1. RecipeStage (agent_loop) calls fetch_for_turn(scope, user_text, budget, "02"):
       a. resolve_intent(user_text) → Match { recipe_id, class_code:21, step_link }
       b. Fetch step_descriptions JSONB + variable_patterns + wilson_lower + tier
       c. IBS: build_instruction(step_link, step_descriptions, variable_patterns)
              → BuildInstruction { rust_steps[], orchestrator_steps[] }
       d. Apply {{vars.name}} substitution
       e. fetch_component_by_id for each UUID in rust_steps → rust_items
       f. fetch_component_by_id for each UUID in orchestrator_steps → orchestrator_items
       g. Return FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }

  ── Tier 0 (routing.tier0_eligible = true, llm_call_required = false): ────
  Two execution models (see DRIVER-GAP / Phase H.0 §H5 MODEL SELECTION):

  Model A — ExecutionLoop/Monty (CURRENT PRODUCTION; no RecipeStage exists):
  2. Python step-0 calls __assemble_prior_knowledge__ → fetch_for_turn → SplitResult.
     When routing.llm_call_required == false, the handler returns a DEDICATED
     `tier_zero: true` signal (NEW — do NOT reuse `override_prompt_creation`; see
     Phase H item 3b + §0.9 v3 step-0) plus the orchestrator_items as
     `orchestrator_content` and the rust_items pre-applied to the execution context.
  3. Python step-0 sees `tier_zero: true` → NEW early-return branch (parallel to the
     `action_short_circuit` return) runs the recipe's orchestrator channel
     (skills + PythonCode) deterministically against the pre-loaded rust execution
     context and returns WITHOUT calling `__llm_complete__` (the
     `execute_action_procedure` "return before __llm_complete__" pattern,
     `default.py:901`, generalised to Tier-0 Recipes). ⚠️ This is NEW wiring:
     today `override_prompt_creation: true` does NOT skip `__llm_complete__`
     (default.py:998-1008 only swaps `working_messages` and falls through), and the
     only pre-`__llm_complete__` return today is the dead `__retrieve_docs__` class-16
     shim (§0.9 Problem 1). Tier 0 on the engine path lands when Phase H adds the
     `tier_zero` signal + this early-return branch — NOT "today".

  Model B/C — DefaultExecutorPipeline/agent_loop (TARGET STATE; skeleton today):
  2. RecipeStage applies rust_items to Rust execution context (silently).
     PromptStage, InterceptorStage, and ModelStage are SKIPPED.
  3. TierZeroExecutionStage calls ctx.host.run_tier_zero(...) (LoopOrchestratorPort
     bridge to the engine orchestrator's no-LLM entry point) with the stashed
     orchestrator_items; AssistantReplyStage emits the result.

  ── Tier 1 (routing.tier0_eligible = false OR llm_call_required = true): ──
  2. RecipeStage stores rust_items → state.recipe_rust_context.
     RecipeStage stores orchestrator_items → state.recipe_hint.
  3. Executor applies rust_items before Python script starts.
  4. Python step 0 calls __assemble_prior_knowledge__: handler returns the
     pre-stashed orchestrator_items as orchestrator_content (no second fetch).
  5. LLM is called; guided by the orchestrator_content recipe hint.
     (Model A: Python calls __llm_complete__. Model B/C: ModelStage calls the LLM.)
```

> **✅ Review note (pre-v3 audit) — intent-driven retrieval is dormant in production today —
> RESOLVED by Phase E.0:** the dormancy described below is exactly what Phase E.0 (the
> zeroth E-family step) closes — it wires `PostgresSource` into the composition path
> (`ThreadManager::with_retrieval_source`) before any phase consumes `fetch_for_turn`'s new
> variants, so the "must happen before or during Phase H" recommendation below is satisfied
> (pulled forward to E.0, before Phase E). The "deferred to Phase K" framing is now stale:
> Phase K.3 is pure deletion (E.0 already wired the backend). Original detail retained:
> The flow above assumes `PostgresSource::fetch_for_turn` is the live retrieval backend. It is
> **not**. In production the engine wires `RamSource` (`crates/brassclaw_engine/src/runtime/manager.rs:383`,
> with a `TODO(Phase K)` comment added by `Goals_pre_v3_review.md` Step 8). `PostgresSource` exists and
> is correct (`#[cfg(feature = "skills-db")]`, UNION ALL query in `retrieval_source.rs`), but it is only
> exercised in tests — the composition layer never calls `with_retrieval_source(PostgresSource)`. As a
> result `resolve_intent` / `record_disambiguation_choice` / `SplitResult` are **never reached** in a
> live turn until Phase K swaps `RamSource` out. Phases A–H can land the types and the IBS, but no turn
> will actually take the Recipe/Tier-0/Tier-1 path until `PostgresSource` is wired. **Ordering
> consequence:** wiring `PostgresSource` into `manager.rs` (composition) is a prerequisite for the
> Phase H runtime behaviour to be observable, and must happen **before or during Phase H**, not be
> deferred to Phase K. Phase K's "remove `RamSource`" step must be ordered *after* the
> `PostgresSource` wiring sub-task, or the production retrieval path breaks (also recorded under
> `Goals_pre_v3_review.md` Step 14).

#### Mandatory shape

| Field | Content |
|-------|---------|
| `name` | Recipe identifier (e.g. `local-files-reading`) |
| `description` | One-sentence summary |
| `category` | Task group → `ExtensionCatalogue.task_groups[].group_name` |
| `step_descriptions JSONB` | Array of StepDescriptionN (YAML text + parsed fields) |
| `variants[]` | One or more `RecipeVariant` entries |
| `trigger` / `steps` | **Kept** — v2 fallback path |

#### Intent Variants

Each variant:
- Owns its own intent expressions (rows in `reborn_intent_inputs`)
- Carries a `step_link` specifying which StepDescription ranges to compile
- The `BuildInstruction` is computed at runtime by the IBS from StepDescriptions — **never stored as a blob**

**Example — Recipe `local-files-reading`:**

```
variant: ls-l
  intents: ["ls -l", "show me all files", "list files", "show local directory files"]
  step_link: "0:0-0:E"               ← all of StepDescription0

variant: ls-la
  intents: ["ls -la", "show all files including hidden", "list all files"]
  step_link: "0:0-0:30+1:0-1:E"     ← SD0 steps 0..30, then all of SD1

variant: ls-other-dir
  intents: ["list files of the /tmp directory", "show files in {{vars.dir}}"]
  variable_patterns: [{ name: "dir", pattern: r"of the (?P<dir>[/\w.-]+)" }]
  step_link: "0:0-0:31+2:0-2:E"     ← SD0 steps 0..31, then all of SD2
```

---

### 0.4 BuildInstruction — Two-Channel Design

> **Key design principle — three parties, one artefact:**
>
> | Party | Role |
> |-------|------|
> | **Human author** (WebUI) | Writes `StepDescriptions` — YAML-structured, readable. Never touches a `BuildInstruction`. |
> | **IBS** (Instruction-Building-System) | Sole producer. Compiles `StepDescriptions` → `BuildInstruction` at intent-match time. Never stores the result — it is ephemeral per-call (memoised in-process; see §0.7). |
> | **Rust executor** (`RecipeStage`) | Reads `rust_steps[]` only. Applies ToolSkill UUIDs and ToolBindings to the Rust execution context. Never touches orchestrator content. |
> | **Orchestrator** (`handle_assemble_prior_knowledge`) | Reads `orchestrator_steps[]` only. Serialises component bodies into `orchestrator_content`. Never touches rust channel content. |
>
> The two runtime readers (**Rust executor** and **Orchestrator**) each see exactly one channel.
> Neither reader sees the other channel. The IBS is the sole bridge.
>
> **BuildInstructions are never stored** — not in the DB, not in session state.
> The IBS compiles them on demand from the `step_descriptions` JSONB column plus the
> resolved `step_link` formula. In-process memoisation (§0.7) eliminates the per-call cost
> for repeated identical intents without requiring persistence.

#### Why two channels, not three

Earlier drafts described a three-section design (RetrievalEngine / Orchestrator / Rust).
The v3 design simplifies: `fetch_steps` is eliminated as a separate section. The IBS
directly emits `rust_steps[]` and `orchestrator_steps[]`, each containing `IbsRecipeStep`
entries with UUIDs. `PostgresSource::fetch_for_turn` calls `fetch_component_by_id` for
each UUID immediately after IBS compilation.

> **No `fetch_by_instruction` method.**
> There is no `RetrievalSource` method named `fetch_by_instruction` or similar.
> The IBS runs synchronously *inside* `fetch_for_turn`, not as a separate retrieval pass.
> `fetch_for_turn` calls `build_instruction(...)`, then immediately calls
> `fetch_component_by_id` for every UUID the IBS emitted.
> The result is `FetchForTurnResult::SplitResult` with two pre-fetched item lists.
> Any design that adds a `fetch_by_instruction` method to `RetrievalSource` is wrong.

#### Two readers, two typed channels

**Channel R — Rust (`rust_steps[]`)**  
Steps with `knowledge: "rust"` or `"both"`.  
Contains: ToolSkill UUIDs + ToolBinding params + ErrorPolicy.  
Applied silently to the Rust execution context by `RecipeStage`. Never forwarded to the orchestrator.

**Channel O — Orchestrator (`orchestrator_steps[]`)**  
Steps with `knowledge: "orchestrator"` or `"both"`.  
Contains: Skill UUIDs and PythonCode UUIDs. PythonCode component bodies ARE the
orchestrator instructions — authored with the correct content and formatting.
`type: "text"` steps are authoring annotations only (WebUI documentation); they have
no runtime emission.
Serialized into `orchestrator_content` by the v3 `handle_assemble_prior_knowledge` in `orchestrator.rs`.

#### 0.4.1 ToolBinding + ErrorPolicy

```rust
/// Binding from a rust-channel IBS step to a specific tool invocation.
/// Persisted in the `step_descriptions` JSONB column (inside rust-channel IbsRecipeStep).
/// `tool_id` is the UUID of the Tool (class 0) row; `tool_name` is denormalized for
/// runtime __execute_action__ calls without a DB round-trip. `params` carries the
/// parameter values with {{vars.name}} substitution placeholders.
///
/// ⚠️ FIND-AUDIT-10: This is the canonical ToolBinding definition. The `types/ibs.rs`
/// "Files to create" block in Phase A MUST match this exactly (tool_id + tool_name + params
/// + error_policy). An earlier draft of types/ibs.rs omitted tool_name and params — that
/// was wrong; both are required for runtime dispatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolBinding {
    /// UUID of the Tool (class 0) row — used by the Rust execution layer for capability dispatch.
    pub tool_id: uuid::Uuid,
    /// Denormalized tool name (e.g. "read_file"). Needed for __execute_action__ calls
    /// without an extra DB fetch. Must match the registered capability name.
    pub tool_name: String,
    /// Parameter values for this tool call. {{vars.name}} substitution applied before use.
    pub params: serde_json::Value,
    pub error_policy: ErrorPolicy,
}

/// ⚠️ FIND-AUDIT-11: This is the canonical ErrorPolicy definition.
/// The `types/ibs.rs` "Files to create" block in Phase A MUST match this exactly.
/// An earlier draft of types/ibs.rs used { Propagate, Retry { max_attempts: u8 },
/// Fallback { message: String } } — that variant set is wrong; use the definitions below.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    /// Fail the turn immediately — hard error, no retry.
    #[default]
    Fail,
    /// Ignore the error and continue — the orchestrator receives an empty result.
    Ignore,
    /// Retry up to max_attempts times before falling through to Fail.
    Retry { max_attempts: u32 },
    /// On error, jump to the step with id step_id within the same BuildInstruction.
    Fallback { step_id: String },
}

// Default: Fail. Implemented via `#[derive(Default)]` + `#[default]` on `Fail`
// (FIND-IBS-06) — semantically identical to the hand-written
// `impl Default for ErrorPolicy { fn default() -> Self { ErrorPolicy::Fail } }`
// that an earlier draft showed, and clippy-clean under `derivable_impls` which
// the repo's zero-warning rule mandates.
```

#### Structure

```
BuildInstruction
├── llm_call_required: bool           ← false for Tier-0/Actions, true for Tier-1+
├── variable_patterns[]               ← applied before any channel is read
├── basic_prompt_section_refs[]       ← navigation hints into cached basic-prompt (no re-fetch)
│
├── rust_steps[]                      ← CHANNEL R: Rust execution layer reads this only
│   └── IbsRecipeStep { step_id, knowledge: Rust/Both,
│                       include: Vec<Uuid>,          ← ToolSkill UUIDs
│                       tool_bindings: Vec<ToolBinding> }
│
└── orchestrator_steps[]              ← CHANNEL O: serialized into orchestrator_content
    └── IbsRecipeStep { step_id, knowledge: Orchestrator/Both,
                        step_type: Text | Component,
                        include: Vec<Uuid>,          ← Skill / PythonCode UUIDs
                        info: Option<String> }       ← WebUI annotation only; NOT emitted to orchestrator
```

> **⚠️ FIND-NEW-02 — per-step struct is `IbsRecipeStep`, not `RecipeStep`.** Phase A
> renames the IBS per-step struct from `RecipeStep` to `IbsRecipeStep` to avoid a
> name collision with the existing v2 `RecipeStep { skill, tool, params, description }`
> in `types/recipe.rs` (see Phase A "Files to create"). This diagram previously
> labelled both channel steps `RecipeStep`; corrected to `IbsRecipeStep`. The v2
> `RecipeStep` (the Tier-1/Tier-2 fallback) is unrelated and stays in `types/recipe.rs`.

**Invariant:** Channels must not overlap.  
A ToolSkill UUID must never appear in `orchestrator_steps`.  
A Skill UUID must never appear in `rust_steps`.  
An orchestrator step never references a ToolSkill. A rust step never references a Skill.  
Runtime content for the orchestrator lives in component bodies loaded by `type: "component"` steps — not in `type: "text"` step `info` fields. Step type and component class are orthogonal.

#### Complete example — Recipe `local-files-reading`, variant `ls-la`

```
BuildInstruction for variant: ls-la
  (intent: "show all files including hidden in /tmp")

llm_call_required: false   ← Tier 0: skip LLM, execute directly
variable_patterns:
  - { name: "dir", pattern: r"in (?P<dir>[/\w.-]+)" }

# ── CHANNEL R: Rust ─────────────────────────────────────────────────────
rust_steps:
  - step_id: "r-toolskill-ls"
    knowledge: Rust
    include: ["<uuid:toolskill-ls>"]
    tool_bindings:
      - tool_name: "ls"
        params: { flags: "-la", dir: "{{vars.dir}}" }
        error_policy: { policy: "fail" }

# ── CHANNEL O: Orchestrator ──────────────────────────────────────────────
orchestrator_steps:
  - step_id: "o-context"
    knowledge: Orchestrator
    step_type: Text
    info: |
      Task performed by orchestrator only. No LLM prompt created.
      Rust receives: ToolSkill "ls" + Tool "ls".
      Orchestrator receives: Skill "ls" + PythonCode "ls-result-handler".
      Orchestrator uses the skill to instruct the Rust executioner.
      Rust executes ls and returns stdout. Orchestrator formats output for chat.

  - step_id: "o-skill-ls"
    knowledge: Orchestrator
    step_type: Component
    include: ["<uuid:skill-ls>"]

  - step_id: "o-pythoncode-ls"
    knowledge: Orchestrator
    step_type: Component
    include: ["<uuid:pythoncode-ls>"]
    # The PythonCode component body is the formatted instruction — no separate formatter needed.
```

---

### 0.5 StepDescription Authoring Layer

**StepDescription is the single human-editable source of truth** for what a Recipe does.
It serves two audiences simultaneously:

- **Human / WebUI editor:** A YAML-structured, readable description of every step a
  component performs after an intent match. Editable in the WebUI component page.
- **IBS (Instruction-Building-System):** The authoritative source from which the
  two-channel `BuildInstruction` is assembled at intent-match time. The IBS reads
  StepDescriptions directly — no intermediate format.

StepDescriptions are stored as a JSONB column `step_descriptions` on `reborn_recipes`
(added in V050). Each element of the JSONB array holds **two representations** of the
same StepDescription, kept in sync on every WebUI save:

```json
[
  {
    "desc_idx": 0,
    "label": "base path (ls -l, current directory)",
    "yaml_source": "steps:\n  - stepnumber: 1\n    knowledge: orchestrator\n ...",
    "steps": [
      {
        "stepnumber": 1,
        "knowledge": "orchestrator",
        "goal": "Provide task context",
        "content": "Information explaining the task",
        "type": "text",
        "info": "Task performed by orchestrator only...",
        "include": [],
        "dependencies": ""
      }
    ]
  }
]
```

- **`yaml_source`** — the raw YAML string as typed by the author. Preserved verbatim.
  Used by the WebUI renderer (syntax-highlighted YAML editor). Never read by the IBS.
- **`steps`** — the pre-parsed structured array. Used exclusively by the IBS.
  Written by the WebUI on save: parse `yaml_source` → produce `steps` array.

The IBS never parses YAML at runtime — it reads the pre-parsed `steps` array directly.
YAML parsing happens exactly once, at WebUI save time, before Q1 runs. If `yaml_source`
fails to parse (malformed YAML), the save is rejected before Q1 with a parse error shown
inline in the WebUI. The `steps` array is therefore always consistent with `yaml_source`.

#### Mandatory fields per step

| Field | Type | Meaning |
|-------|------|---------|
| `stepnumber` | int | 1-based ordinal position within this StepDescription's step sequence |
| `knowledge` | `"orchestrator" \| "rust" \| "both"` | Which runtime channel reads this step |
| `goal` | string | What this step accomplishes (human-readable) |
| `content` | string | Short description of step content |
| `type` | `"text" \| "component" \| "snippet"` | Determines IBS treatment (see below) |

#### Optional fields per step

| Field | Type | Meaning |
|-------|------|---------|
| `info` | text | Human-readable documentation about what this step does. Visible in the WebUI component page to help the author understand the step's purpose. **Not emitted to the orchestrator at runtime.** Orchestrator instructions are delivered by `type: "component"` steps — the body of the referenced component (Skill, PythonCode, or any other orchestrator-channel class) is what the orchestrator receives. |
| `include` | UUID[] | Component UUIDs needed at this step. IBS emits a fetch for each UUID. |
| `codesnippet` | text | Inline Python code. On WebUI save: creates a PythonCode component (class 22) with `validation_status='pending'` and enters the Q1 queue (the queue exists from Phase A.5 / V051). Step greyed out until Q1+Q2 pass; promoted to `type: "component"` with the new UUID on Q2 pass. **Q1/Q2 gate logic is a Phase N capability** (V059). Until Phase N lands, only system-seeded (Phase L) or operator-validated PythonCode is usable in `type:"component"` steps; the V059 boot-integrity check and gate logic complete the promotion flow when Phase N lands. |
| `dependencies` | string | Traversal expression into this step's component's `dependency_registry` (see §0.19). E.g. `"1[all], 5[2,6], 17[3, 7[1,4]]"`. Resolved at fetch time by `fetch_for_turn`. Absent or empty string = no dependencies. |

#### Step types

| Type | IBS behaviour |
|------|--------------|
| `text` | Authoring annotation only. No component fetch. No runtime emission — `type: "text"` steps produce nothing in `orchestrator_content`. They exist solely for WebUI readability: documenting what a step does, why it is here, what the author should know. |
| `component` | Emits a fetch for each UUID in `include`. Routes item to rust or orchestrator channel based on `knowledge`. |
| `snippet` | WebUI-only authoring shortcut. **IBS refuses to assemble** a BuildInstruction while any step has this type — it returns `IbsError::UnpromotedSnippet`. The step must be promoted to `type: "component"` after the created PythonCode passes Q1+Q2. |

> **`type: "text"` steps and the IBS:** The IBS produces no output for `type: "text"` steps.
> They are pure WebUI annotations. Runtime content reaches the orchestrator exclusively via
> `type: "component"` steps: the body of the referenced component — whether a Skill (class 1–3),
> PythonCode (class 22), or any other orchestrator-channel class — is what the orchestrator
> receives. Step type and component class are orthogonal: the step type determines IBS
> handling; the component class and the step's `knowledge` field determine channel routing.
> A `type: "text"` step with no `info` is a Q1 **warning** (undocumented step), not an error.

#### Multi-StepDescription pattern (variants)

- `StepDescription0` — base, most common use-case (the full shared prefix)
- `StepDescription1` — variant 1 (individual part only; SD0 provides the shared prefix via step_link)
- `StepDescription2`, `StepDescription3`, ...

Each `desc_idx` (0-based) maps to an element in the `step_descriptions` JSONB array.

#### Example — Recipe `local-files-reading`, StepDescription0 (partial)

```yaml
# desc_idx: 0  — base path (ls -l, current directory)
steps:
  - stepnumber: 1
    knowledge: orchestrator
    goal: Provide task context
    content: Information explaining the task
    type: text
    info: |
      Task performed by orchestrator only. No LLM prompt created.
      Rust receives: ToolSkill "ls" + Tool "ls".
      Orchestrator receives: Skill "ls" + PythonCode "ls-result-handler".
      Orchestrator uses the skill to instruct the Rust executioner.
      Rust executes ls and returns stdout. Orchestrator formats output for chat.

  - stepnumber: 2
    knowledge: rust
    goal: Provide ToolSkill
    content: ToolSkill "ls"
    type: component
    include: ["<uuid:toolskill-ls>"]

  - stepnumber: 3
    knowledge: orchestrator
    goal: Provide Skill
    content: Skill "ls"
    type: component
    include: ["<uuid:skill-ls>"]

  - stepnumber: 4
    knowledge: orchestrator
    goal: Provide execution instructions
    content: PythonCode "ls-result-handler"
    type: component
    include: ["<uuid:pythoncode-ls>"]
    dependencies: "0[all]"        # load all of pythoncode-ls's declared dependencies
    info: |
      Final step. PythonCode tells the orchestrator how to invoke the skill,
      pass flags to Rust, and format output for the chat window.
```

#### WebUI interaction

- Component page → **Step Descriptions section**: all steps shown, editable on click.
- Dropdown fields for `knowledge` and `type`; free text for `goal`, `content`, `info`.
- `include` field: UUID autocomplete over known component names.
- `step_link` per intent: shown inline, editable with live syntax validation.
- `codesnippet` field: on save → new PythonCode component created → sent to Q1.
  **Security:** Snippet submission requires an authenticated session with `component:write`
  permission. Unauthenticated and read-only sessions must not be able to submit snippets.
  The Q1 injection scan is the technical backstop; ACL is the first line of defense.
  - While pending: step greyed out in WebUI.
  - If Q1 fails: snippet field cleared, PythonCode removed.
  - If Q1+Q2 pass: step promoted to `type: "component"` with the new UUID; parent Recipe re-queued to Q1.

#### StepContextSpec — typed context for each step's IBS output

When the IBS compiles a `BuildInstruction`, each orchestrator-channel step has a **context type**
(`StepContextSpec`) that determines how `handle_assemble_prior_knowledge` formats its body
into `orchestrator_content`. The context type is inferred from the component's `class_code`
after `fetch_component_by_id` returns — authors do not set it manually.

**Class-code → StepContextSpec mapping** (computed at fetch time, not stored):

| `class_code` | Class | `StepContextSpec` | Formatter heading |
|---|---|---|---|
| 1–3 | Skill | `Skill` | `## [Skill: {name}]` |
| 12 | Spec | `Spec` | `## [Spec: {name}]` |
| 13 | ToolSkill | *(never in orchestrator channel)* | — |
| 21 | Recipe | `Recipe` | `## [Recipe: {name}]` |
| 22 | PythonCode | `PythonCode` | `## [PythonCode: {name}]` |
| 23 | ExtensionCatalogue | `Catalogue` | `## [Catalogue: {name}]` |
| *(type: "text" step)* | *(no component fetch)* | `Annotation` | *(nothing emitted)* |

```rust
/// Describes the kind of content emitted to the orchestrator for one step.
/// Inferred from the component's class_code in handle_assemble_prior_knowledge.
/// `Annotation` is assigned when the step type is "text" (no component involved).
pub enum StepContextSpec {
    Skill,           // class 1–3
    Spec,            // class 12
    Recipe,          // class 21 (nested recipe reference)
    PythonCode,      // class 22
    Catalogue,       // class 23
    Annotation,      // type: "text" step — never emitted; WebUI-only
}
```

The formatter in `handle_assemble_prior_knowledge` iterates `orchestrator_items`, derives
`StepContextSpec` from each item's `class_code`, and emits a labelled block:

```
## [Skill: ls]
<skill body>

## [PythonCode: ls-result-handler]
<pythoncode body>
```

This makes `orchestrator_content` self-describing. Authors do not need to add type headers
to their PythonCode bodies or Skill bodies — the formatter generates them from class_code.

**Invariant — ToolSkill is never in `orchestrator_items`.**
`fetch_component_by_id` for a ToolSkill UUID (class 13) called from an orchestrator-channel
step is a Q1 hard error. ToolSkills are Rust-channel-only. If a class 13 UUID appears in
`orchestrator_steps[].include`, the IBS or Q1 must catch it before it reaches the formatter.

`StepContextSpec` is a **derived type** — computed once per component fetch, never stored.
It is not part of the `StepEntry` JSONB. It exists only in the formatter code path inside
`handle_assemble_prior_knowledge` when iterating over `orchestrator_items`.

---

### 0.6 Intent-Link Formula (`step_link`)

Every intent row in `reborn_intent_inputs` carries a `step_link` TEXT column encoding
which steps to assemble. This single field replaces the previously separate `variant_key`
+ `link_formula` columns: it is more expressive (encodes shared prefixes without
duplicating steps) and is the **direct input to the IBS** — no secondary lookup needed.

#### Notation

```
step_link = "{desc_idx}:{start}-{desc_idx}:{end}" [+ "+" more segments]

  {desc_idx} = StepDescription index (0 = base, 1 = first variant, …)
               Always 0-based index into the step_descriptions JSONB array.
  {start}    = step number (1-based, matching the stepnumber field), or 0 (sentinel = first step)
  {end}      = step number (1-based), or E (sentinel = last step)
  +          = concatenate segments in order
```

> **Indexing invariant:** `stepnumber` inside StepDescriptions is always 1-based.
> Formula start/end refer to `stepnumber` values (1-based), except `0` which is a
> sentinel meaning "first step in the sequence regardless of numbering gaps".
> `0:0-0:E` and `0:1-0:E` both mean "all steps of StepDescription0".

#### Examples

| Formula | Meaning |
|---------|---------|
| `0:0-0:E` | All steps of SD0 (single-variant component) |
| `0:0-0:30+1:0-1:E` | SD0 steps 0–30 (shared prefix), then all of SD1 (individual part) |
| `0:0-0:31+2:0-2:E` | SD0 steps 0–31, then all of SD2 |
| `0:0-0:30+1:0-1:11+3:0-3:E` | SD0 steps 0–30, SD1 steps 0–11, all of SD3 |

#### Storage

**Migration V054** (**was V053 before Decision 2**): `ADD COLUMN step_link TEXT CHECK (length(step_link) <= 4096)` to `reborn_intent_inputs`.

**`step_link` is nullable.** Existing rows after V054 have `step_link IS NULL`. The IBS
treats a NULL `step_link` as a legacy intent — it skips IBS compilation and falls through
to the existing `fetch_component_by_id` path unchanged. Only rows seeded after Phase D
carry a non-NULL `step_link`.

**`step_link` replaces `variant_key`.** There is no `variant_key` column. New variants
are authored with `step_link` from the start.

```
| intent_expression              | component_id  | step_link               |
|--------------------------------|---------------|-------------------------|
| "ls -l"                        | <recipe-uuid> | "0:0-0:E"               |
| "show all files including ..."  | <recipe-uuid> | "0:0-0:30+1:0-1:E"      |
| "list files of the /tmp dir"   | <recipe-uuid> | "0:0-0:31+2:0-2:E"      |
```

---

### 0.7 Instruction-Building-System (IBS)

The IBS **compiles** human-editable StepDescriptions into machine-optimized `BuildInstruction`
structs at intent-match time. It is the sole producer of BuildInstructions. BuildInstructions
are never hand-authored or pre-stored.

> **Why assemble on match rather than pre-store?** Component UUIDs in `include` fields can
> be updated (a PythonCode component is revised and re-validated). Pre-stored BuildInstructions
> would require a cascade rebuild on every component update. On-match assembly always reads
> current, validated UUIDs with zero staleness risk. Hot-path memoisation (keyed on
> `sha256(step_link + sorted_include_uuids)`, evicted on validation-status change) eliminates
> the cost for repeated identical intents.

#### Location

- New module: `crates/brassclaw_engine/src/memory/instruction_builder.rs`
- **Pure Rust, no async, no DB calls.**
- Called by `PostgresSource::fetch_for_turn` after an intent match resolves to a Recipe (class 21).
- Exposed as `crate::memory::instruction_builder::build_instruction`.

#### Assembly algorithm

```
fn build_instruction(
    step_link:          &str,
    step_descriptions:  &[StepDescriptionEntry],  // from JSONB
    variable_patterns:  &[VariablePattern],
) -> Result<BuildInstruction, IbsError>

1. Parse step_link → Vec<StepRange>  (e.g. [(desc_idx:0, 0..=30), (desc_idx:1, 0..=E)])
2. For each StepRange:
     Select steps[start..=end] from step_descriptions[desc_idx]
     Append to ordered step list
3. For each step in the ordered list:
     type == "text"      → no runtime emission; step is WebUI annotation only; skip
     type == "component" → emit component fetch step; route by knowledge; emit UUIDs from include
     type == "snippet"   → return Err(IbsError::UnpromotedSnippet)
4. Validate:
     step numbers must be monotonically increasing within each StepDescription
     rust-channel steps must have type:component with non-empty include
     all include UUIDs must parse as valid UUID v4
     S7 guard: if any rust_steps emit tool_bindings, orchestrator_steps must contain ≥1 skill_id
       (Tier 1: Skill body is read by LLM to direct the executor)
       S7-extension for Tier 0: if llm_call_required==false AND rust_steps has tool_bindings,
       orchestrator_steps must contain ≥1 PythonCode UUID (class 22) — not a Skill UUID, because
       Skill bodies require an LLM interpreter. A Tier-0 recipe with tool_bindings and empty
       orchestrator_steps is a Q1 hard error (§tier0-orchestrator-channel Rule 2).
     dependency expressions: parse each step's `dependencies` string into DependencyExpr tree
       → parse errors are hard IBS errors (IbsError::InvalidDependencyExpr)
       → out-of-range indices are not checked here (registry is in DB; checked at Q1)
5. Partition:
     rust_steps[]         ← steps where knowledge ∈ {"rust", "both"}
     orchestrator_steps[] ← steps where knowledge ∈ {"orchestrator", "both"}
6. Attach parsed DependencyExpr to each RecipeStep that declared a dependencies field.
   The IBS does NOT resolve UUIDs — it only parses the expression into a typed tree.
   Resolution (recursive DB fetching) happens in fetch_for_turn (§0.19).
7. Return BuildInstruction { rust_steps, orchestrator_steps,
                              variable_patterns, basic_prompt_refs,
                              llm_call_required }
```

> **⚠️ FIND-IBS-01 — empty-include rule is a Q1 invalidation, NOT an IBS error.**
> Assembly step 4 lists "rust-channel steps must have type:component with
> non-empty include". Rationale for the requirement: `type: "component"`
> semantically means "emit a fetch for each UUID in `include`" (§0.5 step
> types) — an empty `include` contradicts the declared type (the step says it
> will fetch components but lists none). This is a **structural authoring
> defect**, not a runtime semantic. The IBS runs at intent-match time **after**
> the component has passed Q1 validation; it assumes Q1-validated input.
> Therefore the empty-include rule is enforced at **Q1** (`ComponentValidator`,
> Phase I): on failure it **invalidates the component and routes it to the
> review-queue for repair** (§0.23.5). The IBS does **NOT** add a new
> `IbsError` variant for it — the canonical `IbsError` enum (§0.7 Errors) is
> unchanged. If a structurally-defective row somehow reaches the IBS, it is a
> Q1 gate failure (the validation gate missed it), not an IBS compile error.
> The IBS still enforces the other step-4 rules that are pure compile-time
> checks: monotonic stepnumbers (FIND-IBS-03), valid include UUIDs (already
> typed `Uuid`), the S7 guard, and dependency-expression parsing.
>
> **⚠️ FIND-IBS-02 — `build_instruction` takes an `llm_call_required` param.**
> `BuildInstruction.llm_call_required` must be set, but the IBS does not know
> the recipe's tier (that is a function of `wilson_lower`, owned by the caller).
> Resolution (collaborative Q&A): `build_instruction` gains a fourth param
> `llm_call_required: bool`; the caller (`fetch_for_turn`, Phase E.0) computes
> it from the recipe's tier/wilson and passes it in. The IBS stores it directly
> into `BuildInstruction`. The **Tier-0 class-22 S7-extension** ("if
> `llm_call_required==false` AND rust has tool_bindings, orchestrator must
> contain ≥1 PythonCode UUID (class 22)") remains a **Q1** check — the IBS
> cannot distinguish a Skill UUID from a PythonCode UUID (class_code is resolved
> at fetch time per §0.5, UUIDs are opaque to the IBS). The IBS enforces only
> the basic S7 guard ("rust tool_bindings present → orchestrator_steps must
> contain ≥1 step with non-empty include").
>
> **⚠️ FIND-IBS-03 — monotonic check scope is ALL `StepDescriptionEntry`.**
> Assembly step 4 "step numbers must be monotonically increasing within each
> StepDescription" is validated over **every** `StepDescriptionEntry` provided
> in the slice, not only those referenced by the parsed `step_link` ranges.
> This catches malformed unreferenced variants too (never suppress a defect).
>
> **⚠️ FIND-IBS-04 — `IbsRecipeStep.step_id` is synthesized.** `StepEntry` has
> no `step_id` field; `IbsRecipeStep.step_id` (used for `IbsError` attribution)
> is synthesized as `format!("{desc_idx}:{stepnumber}")` — stable, unique, no
> authoring change.

#### LLM-formatted orchestrator content

After assembly, `handle_assemble_prior_knowledge` in `orchestrator.rs` renders
orchestrator_steps into a human+LLM-readable block (`orchestrator_content` in the
`__assemble_prior_knowledge__` result):

```
## Task: {recipe.name} — variant: {variant_label}

Step 1 [orchestrator — text]:
  This task is performed by the orchestrator only. The Rust execution layer
  receives ToolSkill "ls"…

Step 3 [orchestrator — skill]:
  Skill "ls" (UUID: uuid-of-ls-skill) loaded.
  [skill body content]

Step 4 [orchestrator — python_code]:
  PythonCode "ls-result-handler" (UUID: uuid-of-pythoncode) loaded.
  [pythoncode body content]
  Final step: use the skill to call Rust, format output for chat window.
```

#### Memoisation

- **Key:** `sha256(step_link + "|" + sha256(step_descriptions_json) + "|" + sha256(variable_patterns_sorted_json))`

  > **⚠️ DESIGN-02 — original key `sorted_include_uuids` was circular; replaced with `step_descriptions_hash`:**
  > The original key formula was `sha256(step_link + "|" + sorted_include_uuids.join(",") + "|" + sha256(variable_patterns_json))`.
  > This is circular: `sorted_include_uuids` requires the IBS to have already compiled the
  > `BuildInstruction` to know which UUIDs it emitted — so the key can only be computed
  > AFTER doing the work, not BEFORE checking the cache.
  >
  > **Correct key:** `sha256(step_link + "|" + sha256(serde_json::to_string(&step_descriptions).unwrap()) + "|" + sha256(serde_json::to_string(&sorted_variable_patterns).unwrap()))`.
  > This is fully computable from the Recipe row BEFORE IBS compilation. It is correctly
  > invalidated by any StepDescription change (the step_descriptions hash changes) or any
  > variable_patterns change (the variable_patterns hash changes). The step_link embeds
  > which steps are selected; the step_descriptions hash covers any step content changes.
  > No UUID enumeration step is needed — the UUID set is implied by step_descriptions content.
  >
  > **PERF-01 / COMP-06 remain valid:** `variable_patterns` must still be in the key.
  > Sort `variable_patterns` by `name` before hashing for stability under authoring order changes.

- **Eviction triggers (all must be monitored):**
  1. Any `include`d component's `updated_at` changes (via `last_graduation_at` scope cursor — §0.18)
  2. The Recipe's own `updated_at` changes (StepDescription edited in WebUI, or variable_patterns changed)
- **Cache miss:** safe at high concurrency — compilation is pure computation (no HTTP, no DB).
  Concurrent misses compile redundantly; last writer wins the cache slot (idempotent).

#### Errors

```rust
pub enum IbsError {
    UnpromotedSnippet { step_id: String },
    InvalidUuid { step_id: String, value: String },
    StepOrderViolation { desc_idx: usize, stepnumber: u32 },
    UnknownDescIdx { desc_idx: usize },
    ParseError { formula: String, reason: String },
    S7Violation,  // rust tool_bindings present but no orchestrator skill_ids
    InvalidDependencyExpr { step_id: String, reason: String },
}
```

#### Interface

```rust
// crates/brassclaw_engine/src/memory/instruction_builder.rs

pub fn build_instruction(
    step_link:         &str,
    step_descriptions: &[StepDescriptionEntry],
    variable_patterns: &[VariablePattern],
    llm_call_required: bool,            // FIND-IBS-02: caller passes tier-derived bool
) -> Result<BuildInstruction, IbsError>;

pub fn parse_step_link(step_link: &str) -> Result<Vec<StepRange>, IbsError>;

pub fn parse_dependency_expr(expr: &str) -> Result<DependencyExpr, IbsError>;
```

No trait, no async. Called synchronously inside `fetch_for_turn`.

---

### 0.8 `fetch_for_turn` Upgrade — SplitResult and ActionShortCircuit

#### Current state (grounded in code)

`PostgresSource::fetch_for_turn` (in `retrieval_source.rs`) already calls:
- `resolve_intent(pool, scope, query)` → `IntentResolution::Match { component_id, class_code }`
- `fetch_component_by_id(uuid)` on a match

`IntentResolution::Match` currently has only `{ component_id: Uuid, component_class_code: i32 }`.
`FetchForTurnResult` currently has only `Components(Vec<ComponentItem>)` and
`Disambiguation(Vec<IntentCandidate>)`.

> **✅ Review note (pre-v3 audit) — `PostgresSource` is correct but not the live backend —
> RESOLVED by Phase E.0:** the "not wired" state below is closed by Phase E.0, which wires
> `PostgresSource` into the composition path before Phase E. The "see … Phase K" reference
> below is now stale — wiring was pulled forward to E.0 (Phase K.3 is pure deletion).
> `IntentResolution::Match`'s lack of `step_link` (greenfield for Phase D) is unaffected.
> Original detail retained:
> The "Current state" above is accurate, but note that `PostgresSource` is **not wired** in the
> production engine — `manager.rs:383` constructs `RamSource` (keyword retrieval over
> `PgMemoryDocStore`, postgres-backed but non-intent). So although the `fetch_for_turn` extension
> described here is implemented against `PostgresSource`, nothing calls it in a live turn today.
> `ActionShortCircuit` and `SplitResult` therefore do not exist *and* cannot be reached until
> `PostgresSource` is wired (see §0.3 review note and Phase K). `IntentResolution::Match` today has
> exactly `{ component_id, component_class_code }` (verified `intent_system.rs` — no `step_link`),
> confirming Phase D's addition is greenfield.

#### Extended `FetchForTurnResult`

```rust
pub enum FetchForTurnResult {
    /// No-match UNION ALL path or non-recipe intent match (existing behaviour unchanged).
    Components(Vec<ComponentItem>),

    /// Multiple near-equal intent candidates — surface disambiguation UX.
    Disambiguation(Vec<IntentCandidate>),

    /// Intent matched an Action (class 16) — execute directly, no LLM.
    ActionShortCircuit { component_id: Uuid, name: String },

    /// Intent matched a Recipe (class 21) with a step_link.
    /// Two channels pre-fetched and ready for delivery.
    SplitResult {
        rust_items:         Vec<ComponentItem>,   // ToolSkill bodies — Rust only
        orchestrator_items: Vec<ComponentItem>,   // Skill + PythonCode bodies
        routing:            TurnRoutingSignals,
        // ⚠️ UPGRADE (Phase E.4, Q-E4-5 → option B): carries the compiled
        // BuildInstruction (with `{{vars.name}}`-substituted tool_bindings +
        // per-step structure) so Phase H's RecipeStage/TierZeroExecutionStage
        // consumer gets everything without re-compiling. Deviates from the
        // original §0.8 shape above (which had no `instruction` field) and
        // from FIND-P9-03's RetrievalTurnResult. `Some` on a successful
        // compile, `None` on the build_instruction soft-fail. Full rationale
        // + the matched RetrievalTurnResult.instruction (serde_json::Value,
        // decoupled) in docs/agents-v3/subplan_problem_stepE_of_saved_plan_to_v3.md §7.5.
        instruction:        Option<BuildInstruction>,
    },
}

pub struct TurnRoutingSignals {
    pub override_prompt_creation: bool,
    pub matched_component_ids:    Vec<String>,  // orchestrator-channel UUIDs (for _set_active_skills)
    pub variant_label:            String,
    pub step_link:                String,
    pub llm_call_required:        bool,
    /// Wilson lower-bound from the matched Recipe row (for metrics / logging).
    pub wilson_lower:             f64,
    /// Pre-computed Tier 0 eligibility check.
    /// TRUE only when ALL of: tier ∈ {mature, candidate}, wilson_lower ≥ 0.70,
    /// validation_status = 'validated', AND validation ≠ None (hook wired).
    ///
    /// > **Discrepancy note:** `PgRecipe::is_tier0_eligible()` in `pg_recipe_store.rs`
    /// > only checks `is_deliverable() && tier ∈ {mature, candidate}` — it OMITS the
    /// > wilson_lower ≥ 0.70 guard. The v3 `TurnRoutingSignals` must use the full
    /// > `Recipe::is_tier0_eligible()` check from `types/recipe.rs` (which includes
    /// > the Wilson and validation-hook guard), NOT the stripped `PgRecipe` method.
    /// > Phase E must compute this correctly when building `TurnRoutingSignals`.
    pub tier0_eligible:           bool,
}
```

#### Updated `IntentResolution::Match`

```rust
// In intent_system.rs — add step_link AND component_name fields:
Match {
    component_id:         Uuid,
    component_class_code: i32,
    step_link:            Option<String>,  // None for legacy / non-variant intents
    /// Component name, populated for class-16 Actions so ActionShortCircuit can
    /// carry it without a second DB fetch. Empty string for non-Action matches.
    /// See FIND-P5-06 — resolved by adding this field.
    component_name:       String,
}
```

> **⚠️ FIND-P5-06 resolution — `component_name` in `IntentResolution::Match`:**
> `ActionShortCircuit { component_id, name }` needs the Action's human-readable name.
> The cleanest implementation avoids a second DB query by adding `component_name` to
> the match result, populated via a subquery in `resolve_intent`:
> ```sql
> SELECT ii.id, ii.component_id, ii.component_class_code, ii.input_class, ii.score,
>        COALESCE(a.name, '') AS component_name
> FROM reborn_intent_inputs ii
> LEFT JOIN reborn_actions a
>        ON a.id = ii.component_id AND ii.component_class_code = 16
>        AND a.tenant_id = $1 AND a.user_id = $2 AND a.agent_id = $3 AND a.project_id = $4
> WHERE ...
> ```
> Non-action matches have `component_name: ""` (empty string — harmless; never accessed).
> Phase D adds the field to `IntentResolution::Match`; Phase E consumes it in the
> `ActionShortCircuit` return. All destructure sites that add `step_link` must also
> add `component_name` (or bind it as `component_name: _` if unused at that site).

Update all match sites in `retrieval_source.rs` and `orchestrator.rs` that destructure
`IntentResolution::Match { component_id, component_class_code }` to also bind `step_link`
and `component_name`. Non-IBS paths treat `None` step_link as a legacy match.

#### Updated `fetch_for_turn` flow

```
fetch_for_turn(scope, query, token_budget, consumer_tag):

  1. resolve_intent(pool, scope, query)
       → Match { component_id, class_code, step_link }

          a. class_code == 16 (Action):
               → return FetchForTurnResult::ActionShortCircuit { component_id, name }

          b. class_code == 21 (Recipe) AND step_link.is_some():
               i.   Fetch Recipe row → step_descriptions JSONB + variable_patterns
               ii.  IBS: build_instruction(step_link, step_descriptions, variable_patterns)
                         → BuildInstruction { rust_steps[], orchestrator_steps[] }
               iii. Apply {{vars.name}} substitution (captured from user_text)
               iv.  Fetch ComponentItems for UUIDs in rust_steps → rust_items
                    Fetch ComponentItems for UUIDs in orchestrator_steps → orchestrator_items

                    > **⚠️ PERF-02 — batch UUID fetch required:**
                    > Calling `fetch_component_by_id` once per UUID is O(N) individual DB
                    > queries. A recipe with 6 steps = 6 round-trips before returning.
                    > Phase E MUST implement a batched helper:
                    > `fetch_components_by_ids(pool, scope, &[(Uuid, i32)]) -> Vec<ComponentItem>`
                    > using `WHERE id = ANY($ids) AND tenant_id = $1 … AND validation_status = 'validated'`
                    > per-table. The IBS groups UUIDs by channel already, so two batched
                    > fetches (one per channel) replace N per-UUID queries on the hot path.

               → return FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }

          c. step_link.is_none() (legacy intent or non-recipe class):
               → Existing fetch_component_by_id path → Components([item]) (unchanged)

       → Disambiguation { candidates }:
               → return FetchForTurnResult::Disambiguation(candidates)

       → NoMatch:
               → fetch_for_consumer (UNION ALL) → return Components(broad_scan)
```

**No `reborn_pending_rust_context` transient table.** `rust_items` from `SplitResult`
are delivered directly into the Rust execution context by `RecipeStage` (Phase H).
There is no DB round-trip for rust channel delivery.

**No `fetch_by_instruction` method.** The IBS is called synchronously inside
`fetch_for_turn`. There is no separate `RetrievalSource` method for executing a
BuildInstruction.

---

### 0.9 The Current Step-0 Problem and v3 Solution

#### Current step-0 (three calls, two problems)

```python
# Current default.py step 0 — three separate calls:
pkr        = __assemble_prior_knowledge__(goal, token_budget, "02")  # PRIMARY: merged blob
docs       = __retrieve_docs__(goal, 5)                              # SHIM: dead Action detect
all_skills = __list_skills__()                                       # extra round-trip
active_skills = select_skills(all_skills, goal, ...)                 # re-selection
```

**Problem 1 — dead Action detection shim:**  
`__retrieve_docs__` at step-0 uses the legacy `RetrievalEngine::retrieve_context`
(MemoryDoc path). It returns `{type, title, content}` with **no `class_code`** in the
metadata. The check `metadata.get("class_code") == 16` (line 1022 of `default.py`)
therefore **never fires**. This is a known named bug.

**Problem 2 — redundant skills round-trip:**  
`__list_skills__()` → `select_skills()` does no scoring (takes first N in budget).
With a BuildInstruction, the IBS already selected the exact Skills for this turn by UUID.
The re-selection step is unnecessary.

**Problem 3 — mixed blob:**  
`__assemble_prior_knowledge__` returns one merged `formatted_content` blob. Skill bodies,
PythonCode, ToolSkills — all go to the orchestrator together. There is no channel separation.

#### v3 step-0: single call

> **Important — which function is upgraded:**  
> `__retrieve_docs__` is the **legacy** function. It calls the old `RetrievalEngine::retrieve_context`
> (MemoryDoc path), returns a flat list `[{type, title, content}]`, and knows nothing about
> `class_code`, intent resolution, or the component class system. It is the dead shim in the
> current step-0.  
> `__assemble_prior_knowledge__` is **already** the intent-capable path. It calls
> `PostgresSource::fetch_for_turn`, handles `FetchForTurnResult::Components` and
> `Disambiguation`, and returns `{content, formatted_content, override_prompt_creation,
> matched_component_ids}`. This is the function that v3 upgrades — not `__retrieve_docs__`.  
> After the v3 upgrade, `__assemble_prior_knowledge__` handles everything in one call.
> The dead `__retrieve_docs__` shim at step-0 is removed (Phase G). The `__retrieve_docs__`
> host function registration is removed unconditionally in Phase K — there is no compatibility
> window. Any custom orchestrator calling it must be updated before Phase K ships.

The three-call block collapses to one call. The upgraded `__assemble_prior_knowledge__`
handles everything — intent resolution, IBS compilation, channel split, Action routing.

```python
# v3 default.py step 0 — single call:
if step == 0:
    token_budget = config.get("prior_knowledge_token_budget", 100000) if isinstance(config, dict) else 100000
    pkr = __assemble_prior_knowledge__(goal, token_budget, "02")

    if isinstance(pkr, dict):
        if pkr.get("action_short_circuit"):
            __emit_event__("action_started", action_name=pkr.get("action_name", ""))
            __transition_to__("running", "action execution")
            # FIND-P7-02: do NOT call a new execute_action_by_id function — it doesn't
            # exist and must NOT be created. Use the existing execute_action_procedure
            # (default.py:901) which already executes an Action doc without an LLM call.
            # First fetch the action document by UUID, then pass it to the existing executor.
            action_doc = __fetch_component__(pkr["action_component_id"], 16)
            action_result = execute_action_procedure(action_doc, goal, state)
            __transition_to__("completed", "action completed")
            return action_result

        # Tier-0 Recipe (class 21, llm_call_required == false) — NEW no-LLM
        # early return. This is NOT the same as `override_prompt_creation`
        # (Solution Override, an LLM path — see below). `tier_zero` is a
        # dedicated signal emitted by `handle_assemble_prior_knowledge` when
        # `fetch_for_turn` returns `SplitResult { llm_call_required: false }`.
        # It runs the recipe's orchestrator channel (skills + PythonCode)
        # deterministically against the pre-loaded rust execution context and
        # returns WITHOUT calling `__llm_complete__` — exactly the
        # `execute_action_procedure` "return before __llm_complete__" pattern,
        # generalised from class-16 Actions to Tier-0 Recipes. See §0.3 Model A
        # and Phase H item 3b. NOTE: today (pre-Phase-H) this branch does not
        # exist and `override_prompt_creation: true` does NOT skip
        # `__llm_complete__` — it only swaps `working_messages` (default.py:998)
        # and falls through to the LLM. The class-16 short-circuit that exists
        # today is the dead `__retrieve_docs__` shim (§0.9 Problem 1). So this
        # `tier_zero` branch is genuinely NEW wiring landing in Phase H.
        if pkr.get("tier_zero"):
            __emit_event__("recipe_tier_zero_started", recipe=pkr.get("recipe_name", ""))
            __transition_to__("running", "recipe tier-0 execution")
            tier0_result = execute_recipe_orchestrator_channel(pkr, goal, state)
            __transition_to__("completed", "recipe tier-0 completed")
            return tier0_result

        if pkr.get("disambiguation"):
            return handle_disambiguation(pkr["candidates"], state)

        # Solution Override (§3.13) — LLM path: the override content becomes
        # the whole user message. This is NOT a no-LLM Tier-0 path (Tier 0
        # returns early via `tier_zero` above). Control FALLS THROUGH to
        # `__llm_complete__` with the override as the user message — this is
        # the existing default.py:998-1008 behaviour, unchanged.
        if pkr.get("override_prompt_creation"):
            working_messages = [{"role": "User",
                                  "content": pkr.get("orchestrator_content", "")}]
        elif pkr.get("orchestrator_content"):
            insert_as_user_message_at_n_minus_1(working_messages,
                                                pkr["orchestrator_content"])

    # Volatile context injected separately — never mixed with prior knowledge.
    insert_volatile_context_at_n_minus_1(working_messages)

    # Active-skill tracking using matched UUIDs — no __list_skills__ round-trip.
    _set_active_skills_from_matched_ids(pkr.get("matched_component_ids", []), state)

    # REMOVED in v3:
    # - docs = __retrieve_docs__(goal, 5)       ← dead Action-detection shim (Phase G)
    # - all_skills = __list_skills__()          ← IBS already selected Skills by UUID
    # - active_skills = select_skills(...)      ← no longer needed
```

#### What `__assemble_prior_knowledge__` returns in v3

The existing return shape is **extended** (not replaced) to carry the new v3 routing signals.
Existing `{content, formatted_content, override_prompt_creation, matched_component_ids}`
fields are preserved for backward compatibility with custom orchestrators.

> **⚠️ FINDING F — `formatted_content` in the current codebase is structured JSON, not flat string:**
> `assemble_from_component_items` (orchestrator.rs:2674) currently produces `formatted_content`
> as a JSON string: `{"prior_knowledge": [...], "matched_components": [...]}` — not a prose
> string. The current `§0.9` description of `formatted_content` as a "formatted content blob"
> may suggest prose. In v3, `formatted_content` becomes an alias for `orchestrator_content`,
> which IS a formatted prose string (the StepContextSpec-headed block). Phase F must
> explicitly change the shape: `formatted_content` transitions from a JSON-encoded object
> to a prose string (= `orchestrator_content`). Custom orchestrators that currently parse
> `formatted_content` as JSON will break if not warned. The Phase F migration note must
> document this shape change. In the return dict, set:
> ```python
> result["formatted_content"]  = orchestrator_content_string   # NEW: prose string
> result["orchestrator_content"] = orchestrator_content_string # also new field
> ```
> Any code that does `json.loads(pkr["formatted_content"])` will break — it must be
> updated to use `pkr["orchestrator_content"]` or `pkr["formatted_content"]` as a string.
> Document this in the Phase F change notes and the public changelog.
>
> **✅ Review note (pre-v3 audit) — FINDING F verified against current code — RESOLVED:**
> the imprecise "2674" line cite is corrected below (the JSON `.to_string()` is at ~2710; the
> prose override at ~2677), and Phase F is instructed to reference both branches when
> documenting the shape change. This is a documentation-precision fix — no code change needed.
> Original detail retained:
> Confirmed in `orchestrator.rs`: the normal-assembly branch builds
> `serde_json::json!({"prior_knowledge": entries, "matched_components": matched_ids}).to_string()`
> (the JSON string, ~line 2706–2710), while the single-override/Action branch sets
> `formatted_content` to the **prose** `item.effective_content` (~line 2677). So `formatted_content`
> is shape-polymorphic today: JSON for multi-component, prose for the override path. The plan's
> line cite "orchestrator.rs:2674" is within the function but the JSON `.to_string()` is at ~2710;
> Phase F should reference both branches when documenting the shape change.

```python
{
    # EXISTING fields (preserved, shape changed in v3):
    "content":                  str,   # Raw PKC — Rust dispatch / KV-cache fingerprint only
    "formatted_content":        str,   # ⚠️ CHANGED in v3: was JSON object; now prose string alias for orchestrator_content
    "override_prompt_creation": bool,

    # EXTENDED in v3 — orchestrator channel content:
    # Skill bodies, PythonCode bodies, and any other orchestrator-channel component bodies.
    # type:text step info fields are NOT included — they are WebUI annotations only.
    # ToolSkill bodies NEVER appear here (Rust channel is delivered silently by RecipeStage).
    "orchestrator_content": str,

    # v3 routing signals (new):
    "action_short_circuit":  bool,
    "action_component_id":   str,   # UUID (when action_short_circuit is true)
    "action_name":           str,
    "disambiguation":        bool,
    "candidates":            list,

    # Active-skill tracking (extended):
    "matched_component_ids": list,  # orchestrator-channel UUIDs (Skills + PythonCode)
                                    # passed to _set_active_skills_from_matched_ids;
                                    # no __list_skills__() + select_skills() round-trip
}
```

The Rust channel (ToolSkills, ToolBindings) is applied to the Rust execution context
**inside `handle_assemble_prior_knowledge`, silently**. It never crosses to the
orchestrator's `working_messages`. `formatted_content` is an alias for `orchestrator_content`
in v3 — both are set to the same value. Custom orchestrators that already check
`pkr["formatted_content"]` continue to work unchanged.

#### `call_action` nested lookup migration

`call_action` in `default.py` (line 844) currently calls `__retrieve_docs__(nested_name, 1)` to
look up an Action by name. This is a search-by-name — fragile and hits the legacy path.

**v3 replacement:** a new host function `__fetch_component__(uuid, class_code)` calls
`fetch_component_by_id` directly with the UUID from the BuildInstruction step.

```python
# Old (line 844):
action_docs = __retrieve_docs__(nested_name, 1)
# New:
action_item = __fetch_component__(action_uuid, 16)
```

`__retrieve_docs__` is the **dead legacy function** — it returns a flat `[{type, title, content}]`
list with no class_code awareness. It must not appear in v3 default.py at all.
`__retrieve_docs__` registration is removed unconditionally in Phase K (no compatibility window).

---

### 0.10 Current Turn Pipeline (Actual Code)

```
1.  CheckpointStage     — cancel-check
2.  BudgetStage         — token/iteration budget check
3.  InputStage          — drain pending user input → LoopExecutionState
4.  RecipeStage         — [STUB] always falls through (structural debt in recipe.rs)
5.  PromptStage         — assemble LLM prompt from history + prior_knowledge
6.  InterceptorStage    — Sempai review of outgoing prompt (if connected)
7.  ModelStage          — LLM call (Kohai)
8.  ReplyAdmissionStage — validate/admit model response
9.  AssistantReplyStage — emit response to user
10. CapabilityStage     — if response contains tool calls: execute, loop back
11. StopStage           — check for loop termination
12. ExitStage           — clean exit
```

**Critical gap:** `RecipeStage` (step 4) always falls through to Tier 2. Phase H closes this.
**Verified:** `recipe.rs` (85 lines, full file read) confirms the stage always returns
`RecipeStep::Continue` with a structural debt comment: "user text is not yet accessible
from `LoopExecutionState` at this pipeline position." The `RecipeStep` enum has exactly
ONE variant: `Continue { state: Box<LoopExecutionState> }`. Phase H adds `TierZero` and
`ActionExecuted`.

`LoopExecutionState` has no `last_user_text` field. Added in Phase H via `InputStage`.

**Prior-knowledge assembly** happens inside `PromptStage` (step 5) via the orchestrator's
`__assemble_prior_knowledge__` call (step 0 in `default.py`), which calls
`PostgresSource::fetch_for_turn` and handles the full split and channel delivery internally.
The legacy `__retrieve_docs__` (MemoryDoc path) is NOT called in v3 step-0.

> **⚠️ DRIVER-GAP cross-reference:** the sentence above assumes the agent-loop `PromptStage`
> and the engine Python `__assemble_prior_knowledge__` share one turn. They do NOT today —
> the production driver is the engine `ExecutionLoop::run` (stage-free); the agent-loop
> `DefaultExecutorPipeline` (where `PromptStage` lives) is a skeleton with no Python access,
> and `brassclaw_agent_loop` cannot import `brassclaw_engine`. Phase H.0 §H5 resolves this with
> a `LoopOrchestratorPort` host port (composition-bridged): Tier 1 step-0 is reached via
> `run_step_zero`, and Tier 0 via `run_tier_zero` (the `TierZeroExecutionStage` kick). Until
> the agent-loop pipeline is wired as the driver (DRIVER-PREREQ), this flow is test-only.

---

### 0.11 Normal Assembly — No-Match Path (UNION ALL weights)

| Class | Label | Weight |
|-------|-------|--------|
| 50 | Scaffold | 0.55 |
| 10 | Orchestrator | 0.52 |
| 12 | Spec | 0.50 |
| 0 | Tool | 0.50 |
| 1–3 | Skills | 0.45 |
| 4–9 | Extensions | 0.42 |
| **22** | **PythonCode** | **0.42** |
| 13 | ToolSkill | 0.40 |
| 18 | Lesson | 0.40 |
| 21 | Recipe | 0.38 |
| **23** | **ExtensionCatalogue** | **0.38** |
| 16 | Action | 0.35 |
| 14 | Plan | 0.30 |
| 17 | Docu | 0.25 |
| 19 | Issue | 0.20 |
| 15 | Summary | 0.10 |
| 20 | Note | 0.05 |

Bold rows are new additions for v3.

> **✅ Review note (pre-v3 audit) — `doc_type_weight_by_class(i32)` does not exist — RESOLVED
> (obsolete, do not implement). CORRECTION: `doc_type_weight(DocType)` DOES still exist in
> `retrieval_dbless.rs` but is separate and takes the deprecated enum, not an i32:**
> The function `doc_type_weight_by_class(i32)` (the i32-keyed variant) does not exist. However,
> `retrieval_dbless.rs` still contains `doc_type_weight(DocType) -> f64` (lines ~76–88), which
> takes the **deprecated** `DocType` enum. This function remains as part of `RamSource`'s keyword
> scoring. It already covers `DocType::Recipe => 0.35` and `DocType::ToolSkill => 0.40` but has
> no `PythonCode` or `ExtensionCatalogue` variants — because `DocType` is frozen (§0.11 FINDING B).
> Classes 22 and 23 cannot be added to `doc_type_weight` (frozen enum). **This is consistent with
> the plan's direction:** when `RamSource` is deleted in Phase K.3, `doc_type_weight(DocType)`
> (along with `extract_keywords` and `keyword_match_score`) is moved to `retrieval_source.rs` as
> private helpers or deleted entirely. `PostgresSource::fetch_for_consumer` orders by
> `(class_code ASC, prompt_uid ASC)` — no weight function used. Classes 22/23 sort automatically
> once rows exist.
>
> The weight table above is retained here **for historical/authoring intent only** (it shows
> the *relative priority* the original design wanted new classes to have). It must NOT be
> read as an instruction to add match arms to any weight function — `doc_type_weight_by_class`
> does not exist, and `doc_type_weight(DocType)` uses a frozen deprecated enum. Phases B and C
> below have been corrected to drop the "add weight arm" sub-steps; classes 22/23 only need a
> `class_label` arm (§0.11 FINDING B below) and the `fetch_for_consumer` / `fetch_component_by_id`
> arms — ordering is automatic via `class_code ASC`.

> **⚠️ FINDING B — `DocType` is `#[deprecated]` — DO NOT add new variants:**
> The `DocType` enum in `crates/brassclaw_engine/src/types/memory.rs` is annotated
> `#[deprecated(since = "0.1.0")]`. Adding `PythonCode` or `ExtensionCatalogue` variants
> to it would extend a deprecated type and contradict the migration direction.
>
> **Phase B/C action:** Adding classes 22 and 23 requires:
> 1. **Do NOT add `DocType::PythonCode` or `DocType::ExtensionCatalogue`** to `types/memory.rs`.
>    The `DocType` enum is frozen. All new class-code dispatch uses integers only.
> 2. ~~Adding integer match arm `22 => 0.42` in `doc_type_weight_by_class(i32)`~~ — **OBSOLETE**:
>    `doc_type_weight_by_class(i32)` does not exist. The only weight function in
>    `retrieval_dbless.rs` is the enum-keyed `doc_type_weight(DocType)`, which accepts only
>    existing `DocType` variants and cannot be extended with frozen-enum entries. No arm to add.
> 3. ~~Adding enum variant arm `DocType::PythonCode` / `DocType::ExtensionCatalogue` to
>    `doc_type_weight(DocType)`~~ — **OBSOLETE**: `DocType` is frozen; adding variants to it is
>    explicitly prohibited by FINDING B. `retrieval_dbless.rs` and `doc_type_weight(DocType)` are
>    scheduled for full deletion in Phase K.3 together with `RamSource`. Skip this sub-step.
> No action is needed to make classes 22/23 sort correctly — `ORDER BY class_code ASC,
> prompt_uid ASC` already places them deterministically once rows exist in their tables.

---

### 0.12 Actions — LLM-Bypass

Actions (class 16) already default to `override_prompt_creation = true` in V029.
Their `steps` JSONB encodes 13 step types and is **executed directly by the orchestrator
without going through the IBS**. The IBS applies to Recipes (class 21) only.

In v3, an Action intent match returns `FetchForTurnResult::ActionShortCircuit` — no
BuildInstruction, no IBS compilation, no prior-knowledge assembly. The `__assemble_prior_knowledge__`
return dict carries `action_short_circuit: true` + `action_component_id`. The Python
step-0 block fetches the action document via `__fetch_component__(action_component_id, 16)`,
then calls `execute_action_procedure(action_doc, goal, state)` and returns immediately.
(`execute_action_procedure` already exists at `default.py:901` — do NOT create a new
`execute_action_by_id` function. See FIND-P7-02.)

Do not confuse the Action override mechanism (`override_prompt_creation`) with the
Recipe Tier-0 mechanism (`llm_call_required: false`). They are separate paths.

---

### 0.13 KV-Cache / LMCache-Aware Design

**Basic-prompt:** Pre-assembled `InstructionBundle` stored in `reborn_basic_prompt_store`
(V056 — **was V055 before Decision 2**). Manual trigger only. Stale when any component passes Gate 2 (Q2 approval).

**Compile flow (operator, via Phase K.1 Prefix Tab):**
The operator opens Settings → Prefix Tab, sees the `base-prompt` prefix row (the only one today), and clicks **Generate / Regenerate**. This single action:
1. Assembles the bundle from all validated components (`do_reassemble` logic: for each component table, `SELECT … WHERE validation_status='validated' AND NOT ('05:validator' = ANY(consumer_tags)) ORDER BY class_code, prompt_uid ASC LIMIT 1000`; rows rendered as `## {class_code}:{prompt_uid}  {label}  "{name}"\n\n{content}`; a literal Sempai Response Schema JSON block is appended).
2. Stores the result in `reborn_basic_prompt_store` (per `(tenant_id, user_id, agent_id, project_id)` scope), computing `fingerprint = sha256(bundle_content)`, setting `is_stale = false`, `assembled_at = now()`.
3. Ships the bundle to the Sempai LLM as a single `System` message via `HostManagedModelRequest` → `gateway.stream_model` (model profile `"sempai_model"`), warming its KV cache.
4. Stores `prewarm_last_at` in the row on success.

This replaces the two-button Reassemble + Pre-warm flow in the Interceptor tab (see Phase K.1 §UI migration).

**Placeholder substitution at turn time (Phase K.1 wiring):**
The per-turn prompt carries a single `base-prompt` placeholder line while being composed. The Sempai-Kohai system (see `09-sempai-kohai.md`, `10-prefix-base-prompt.md`) replaces that placeholder with the real base-prompt content **at the very end of prompt creation**. If the base prompt was not precompiled (absent from `reborn_basic_prompt_store` or `is_stale = true` and the scope has no fresh entry), the Sempai-Kohai system emits a **short minimal-context prompt-part** with only the most necessary information instead — because the LLM computes only ~200 new tokens/s while cached prefix tokens are free.

**Staleness:**
Any component Q2 graduation → `mark_stale(scope)` called on `PgBasicPromptStore` → the prefix row's `is_stale = true`. The Prefix Tab surfaces this as the **Regenerate** button lighting up. This is side effect 4 of Q2 approval (§0.15).

**BuildInstruction patch rules:**
- Must NOT repeat content already in the stored basic-prompt.
- `basic_prompt_section_refs` carries navigation hints (pointers, not content):
  e.g. `"→ see §ls-skill in basic-prompt"` — the LLM already has the body from KV-cache.
- Target patch size: < 4k tokens (fast new-token computation).
- Orchestrator patch: PRIORITY 2 (instruction snippets) in the InstructionBundle.
- Memory: PRIORITY 3 (memory snippets).
- Rust context: delivered directly by `RecipeStage`, not in the bundle at all.

---

### 0.14 Interceptor System

Saves each turn's composition plan (`BuildInstruction` orchestrator_steps + routing signals,
not the basic-prompt content). If Sempai is connected: reviews the outgoing prompt before
shipping to Kohai. Can flag patterns for Recipe creation.

> **⚠️ Revised/extended by §0.23 (v3 direction).** The interceptor is extended to:
> (1) **Sempai auto-creates ALL component types** — not just recipes — via a generalised
> `SempaiReviewOutcome` + `SempaiProposalSink` (§0.23.6); (2) the kohai prompt store
> (`PgInterceptorStore`/`PromptSegment`) captures **component UUID references** and keeps
> packets for **6 weeks max** (§0.23.7); (3) an **idle self-improvement sweep** (idle ≥ 2h
> AND after 15:00 local, once/day) reassembles prompts + chat history and asks the Sempai
> for component creation/upgrades → Q1 (§0.23.8). Implementation folds into **Phase K**
> (§0.23.11).

---

### 0.15 Validation System — Two-Gate Pipeline

> **⚠️ Revised by §0.23.2.** Q1 is **upgraded from pure-Rust to orchestrated**: the
> `component_validator.rs` logic is split into v3 components and Q1 runs a sandboxed
> agent-loop orchestrator instance. There is **no permanent deterministic floor** (even
> injection/schema checks become orchestrated test components over time); the
> calculated risk is accepted and reduces itself via self-improvement (§0.23.8).
> `component_validator.rs` is **retired at Phase N**. The **state-2 invariant is
> retained**: only the Q1 orchestrator writes `state = 2`. The text below is the
> pre-revision reference; see §0.23.2 for the authoritative upgraded design.

**Gate 1 (Q1 — automatic):** Injection scan, schema conformance, S7 guard, cross-references.
Implemented in `component_validator.rs`. On pass: queue row transitions to state 2 (via
`gate1_pass` — callable only from the validator). On fail: queue row stays at state 1
with updated `validation_errors`.

**Gate 2 (Q2 — manual):** WebUI review. On approve: queue row deleted (graduation event),
component `validation_status` set to `'validated'`. On reject: queue row transitions to
state 3, `counter` incremented, `review_feedback` populated.

**The queue and the status are separate state machines (§0.18):**
- While a component is in `reborn_validation_queue` (states 1–4) it is pre-validation.
  Its `validation_status` on the component table is `'pending'` or similar.
- Once the queue row is deleted (Q2 approval), the component is post-validation.
  Its `validation_status = 'validated'` is the sole retrieval gate.

**Q2 approval drives four side effects:**
1. Queue row deleted → `last_graduation_at` bumped on scope cursor (via DB trigger).
2. Component `validation_status` set to `'validated'`.
3. SplitResult memo-cache for this scope evicted on next cache hit (via `last_graduation_at` check — §0.18).
4. `PgBasicPromptStore::mark_stale(scope)` called — sets `is_stale = true` on the `reborn_basic_prompt_store` row for the scope, signalling the Prefix Tab to show the Regenerate button (§0.13, Phase K.1).

---


### 0.16 Builtin Tool Bootstrap

> **⚠️ Further revised by §0.23.3 / §0.23.9.** Phase L now **also seeds the
> trusted-root validation system** alongside the builtin-tool stack: one pre-trusted
> Extension per class + four category main-Recipes per class (each calling
> sub-recipes) + one basic Recipe per class + one formatter PythonCode per class. The
> validation system is itself built from v3 components (trusted root + evolvable via
> Sempai→Q1→Q2). All `source='system'` components — builtins **and** validation-system
> trusted root — graduate via the automated-but-auditable Q2 (Phase P.0), no bypass.
> See §0.23.3 (component shape), §0.23.4 (formatter/`formatted_content`), §0.23.9
> (ordering: L seeds the trusted root right before Phase N's orchestrated Q1).

> **⚠️ REVISED by Answer 2 to the doc-conversion review + Phase P.0.** The
> bypass pattern described below (builtins seeded with
> `validation_status='validated'`, Q2 skipped) is **superseded**: nothing
> ever bypasses Q1+Q2. Builtins now enter `reborn_validation_queue` at
> `'pending'`, run Q1, and graduate via an **automated-but-auditable Q2**
> (the seeder/automation is the recorded Q2 actor — never a silent skip).
> `source='system'` is provenance only and never gates validation. The text
> below is retained as the pre-revision reference; Phase P.0 implements the
> revision (also see Open Questions #8 and #12, both superseded).

#### Ground truth

All 23 first-party builtin tools are registered purely in Rust code
(`crates/brassclaw_host_runtime/src/first_party_tools/`), under provider ID `"builtin"`.
The DB tables `reborn_tools` (V030), `reborn_tool_skills` (V037), and `reborn_skills` (V027)
are live and structurally ready, but contain **zero rows for builtins** today. The
orchestrator receives no authored prior knowledge about when to use `grep` vs `read_file`,
what memory_search expects, or when shell requires approval.

`reborn_tools` currently has no column linking a DB row back to its registered capability ID
(`"builtin.read_file"`). A `capability_id TEXT` column is needed (V057 — was V056 before Decision 2) to avoid fragile
name-search lookups when the Rust execution layer needs to resolve a Tool row to its handler.

#### What gets generated

The builtin bootstrap generates the full v3 component stack for all 23 tools, grouped into
**5 ExtensionCatalogues** by cognitive domain (not one per tool):

| Catalogue | Tools covered | Recipes (approx) |
|-----------|---------------|------------------|
| `builtin-filesystem` | read_file, write_file, list_dir, glob, grep, apply_patch | 6–8 |
| `builtin-network` | http, http.save | 3–4 |
| `builtin-memory` | memory_search, memory_write, memory_read, memory_tree | 2 |
| `builtin-process` | shell, spawn_subagent, trigger_create/list/remove | 5–6 |
| `builtin-management` | skill_list/install/remove, echo, time, json | 4–5 |

For each tool the bootstrap generates:
- **Tool (class 0):** One row per `builtin.X` capability. `capability_id = "builtin.X"`,
  `effect_type` mapped from EffectKind, `param_schema` from Rust schema structs in
  `schemas.rs`, `source = "system"`.
- **ToolSkill (class 13):** One per tool. Hand-authored `content` with: the exact
  `tool_name`, annotated `param_schema`, preconditions, error handling, and critical safety
  notes (especially for `shell`, `apply_patch`, `spawn_subagent`).
- **Skill (class 1–3):** **Task-level, not tool-level.** The filesystem group gets 4–5
  Skills covering task patterns (e.g. "find files" = glob + grep combined in one Skill
  body), not 6 trivial single-tool Skills. Utilities (`echo`, `time`, `json`) get
  PythonCode helpers instead of Skills (see Grain rule below).
- **PythonCode (class 22):** For utility helpers that are sub-orchestrator patterns rather
  than standalone capabilities: `json-query-helper`, `time-format-helper`, `patch-formatter`
  (for apply_patch result formatting).
- **Recipe (class 21):** Task-level, multi-variant where the cognitive grain demands it.
  `builtin-edit-file` gets 3 variants; `builtin-http-fetch` gets 3. See §0.16.1 for the
  full recipe list.
- **ExtensionCatalogue (class 23):** One per domain. `overview_doc` describes the domain
  model, not individual tools.

#### Grain rule — Skill vs PythonCode

Use a **Skill** when: the orchestrator needs narrative instructions for a task pattern
that spans one or more tools — a complete capability description.  
Use **PythonCode** when: the component is a utility helper used inside another Recipe's
orchestrator channel, not a standalone capability.

`echo`, `time`, `json` → PythonCode helpers.  
All filesystem, network, memory, skill-management, trigger patterns → Skills.

#### Validation at bootstrap

All generated components are inserted with `source = "system"` and
`validation_status = "validated"` (bypassing Q2 for system-authored components; Q1 still
runs internally inside the seeder). This prevents the boot state from depending on a human
completing Q2 before the agent can use its own core tools.

`"system"` is a new allowed value for the `source` column on `reborn_tools`,
`reborn_tool_skills`, and `reborn_skills` (V056 adds it to the CHECK constraints).
Q1 errors in the seeder content are a build-time bug, not a runtime failure mode — they
must be caught in CI.

#### Shell + spawn_subagent safety invariants

Two invariants encoded in ToolSkill bodies and enforced at Q1:

1. **`builtin.shell`:** The shell ToolSkill `content` must include an explicit
   approval-gate description. Any Recipe whose rust channel references `builtin.shell`
   **must** have `llm_call_required: true` — enforced as a Q1 rule (see Phase I §shell-guard).
   Open-ended shell cannot be Tier 0. Known-safe commands (e.g. `cargo build`) may be
   Tier 1 at high Wilson score, but never Tier 0 without explicit allowlisting.

2. **`builtin.spawn_subagent`:** The spawn_subagent ToolSkill must document: child cannot
   exceed parent scope, budget inheritance, authorization model. Any Recipe using it must
   be Tier 1 (`llm_call_required: true` enforced at Q1 — same rule as shell).

#### §0.16.1 Full builtin Recipe list (target)

| Recipe name | Variants | Tier | ToolSkills in rust channel |
|-------------|----------|------|---------------------------|
| `builtin-read-file` | 2 (by path, by glob) | 0 | read_file |
| `builtin-write-file` | 2 (create, overwrite) | 1 | write_file |
| `builtin-list-dir` | 2 (current dir, named dir) | 0 | list_dir |
| `builtin-find-files` | 3 (by name, by ext, by pattern) | 0 | glob |
| `builtin-search-content` | 3 (literal, regex, in dir) | 0 | grep |
| `builtin-edit-file` | 3 (targeted edit, refactor, fix-line) | 1 | read_file + apply_patch |
| `builtin-http-fetch` | 3 (GET, POST, with headers) | 1 | http |
| `builtin-http-download` | 1 | 1 | http.save |
| `builtin-remember` | 1 | 0 | memory_write |
| `builtin-recall` | 1 | 0 | memory_search |
| `builtin-run-shell` | 2 (known-safe cmd, open-ended) | 1 (always) | shell |
| `builtin-spawn-subagent` | 2 (generic task, named procedure) | 1 (always) | spawn_subagent |
| `builtin-create-trigger` | 1 | 1 | trigger_create |
| `builtin-list-triggers` | 1 | 0 | trigger_list |
| `builtin-remove-trigger` | 1 | 1 | trigger_remove |
| `builtin-list-skills` | 1 | 0 | skill_list |
| `builtin-install-skill` | 1 | 1 | skill_install |
| `builtin-remove-skill` | 1 | 1 | skill_remove |

**Total: ~23 Tools + 23 ToolSkills + 12–15 Skills + 4–5 PythonCode + 18–20 Recipes + 5 ExtensionCatalogues ≈ 85–90 components.**  
All inserted at boot if the scope has no existing builtin components (idempotent).

---

### 0.17 Variable Intent Templates

#### The problem

`resolve_intent` matches `input_text = $query` — exact string equality. A Recipe variant
whose execution depends on a runtime value (a path, a filename, a search pattern) cannot
be expressed as a single intent row. Without a variable mechanism the author must
pre-register every possible value as a separate row, which is impossible.

#### The `%` slot marker

Intent expressions authored on Recipe variants (and Skills with `intent_examples`) may
contain `%` as a **positional slot marker**. `%` means "any sequence of tokens may appear
here". The author controls where variability is allowed; the rest of the expression is
literal and anchors the match.

```
# Literal expression (no slot) — stored and matched exactly as today:
"list files of the current directory"

# Template expressions (contain %):
"show me all files in the % directory"
"show me all files in the directory %"
"read the file at %"
"search for % in %"
"edit % and change %"
```

`%` is purely an authoring and matching marker. After a template matches, the values
captured in each `%` slot are extracted from the user text and passed to the
`variable_patterns` extraction step (or auto-extracted from template segments when
`variable_patterns` is absent — see §0.17.3).

`variable_patterns` and `%` are **separate concerns**:
- `%` drives **matching** — does this user text structurally fit this template?
- `variable_patterns` drives **extraction** — what is the value of each slot?

`variable_patterns` becomes optional for simple single-slot cases where auto-extraction
from template segments is unambiguous (see §0.17.3).

#### Terminology

| Term | Meaning |
|------|---------|
| **literal expression** | Intent text with no `%` — stored and matched exactly (existing path) |
| **template expression** | Intent text containing one or more `%` slots |
| **template_prefix** | The literal text before the first `%` in a template |
| **template_suffix** | The literal text after the last `%` in a template |
| **anchor** | A non-empty `template_prefix` or non-empty `template_suffix` |

---

### 0.17.1 Matching — Three-Path Dispatch

Template matching uses PostgreSQL's `LIKE` operator with the stored template as the
**pattern** and the user text as the **value**:

```sql
'show me all files in the /tmp directory'
  LIKE
'show me all files in the % directory'
-- → TRUE  (PostgreSQL native, no Rust pre-processing needed)
```

This is the reverse of the usual `LIKE` use — the pattern is stored in the DB, the
concrete value is the incoming query. PostgreSQL supports this natively.

Because plain sequential scanning of all template rows is too slow at scale, matching
is pre-filtered using computed anchor columns and targeted indexes. Three index paths
cover all valid templates:

**Path 0 — Exact match (existing, unchanged):**
```
input_text = $user_text
Uses the existing B-tree index on (scope, input_text, input_class).
```

**Path 1 — Prefix-anchored template (`template_prefix != ''`):**
```
template_prefix = "show me all files in the "
User text must start with this prefix.
Pre-filter: $user_text LIKE (template_prefix || '%')
Full check:  $user_text LIKE input_text
Uses B-tree index on (scope, template_prefix).
```

**Path 2 — Suffix-anchored template (`template_prefix = ''`, `template_suffix != ''`):**
```
Leading-% case: "% directory", "% in /tmp"
template_suffix = " directory"
User text must end with this suffix.
Pre-filter (reverse trick): reverse($user_text) LIKE (reverse(template_suffix) || '%')
Full check:  $user_text LIKE input_text
Uses functional B-tree index on (scope, reverse(template_suffix)).
```

**Path 3 — Dual-anchored template (`template_prefix != ''` AND `template_suffix != ''`):**
```
"search for % in the % directory"
prefix = "search for ", suffix = " directory"
Uses the prefix index as primary pre-filter (more selective),
suffix check eliminates remaining false candidates before full LIKE.
Fastest path — two anchors eliminate nearly all non-matching rows.
```

**Blocked — no anchor (`template_prefix = ''` AND `template_suffix = ''`):**
```
"% in %", "% %", "%"
Q1 hard error. Never reaches the DB.
```

The combined SQL for `resolve_intent` evaluates all four paths in a single query:

```sql
WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
  AND input_class = ANY($6)
  AND (
    -- Path 0: exact match (existing path, unchanged)
    input_text = $5

    -- Path 1: prefix-anchored template
    OR (
        is_template = true
        AND template_prefix != ''
        AND $5 LIKE (template_prefix || '%')
        AND $5 LIKE input_text
    )

    -- Path 2: suffix-anchored template (leading-% case)
    OR (
        is_template = true
        AND template_prefix = ''
        AND template_suffix != ''
        AND reverse($5) LIKE (reverse(template_suffix) || '%')
        AND $5 LIKE input_text
    )
  )
ORDER BY
  CASE WHEN input_text = $5 THEN 0 ELSE 1 END,   -- exact match always beats template
  CASE input_class WHEN $7 THEN 0 WHEN $8 THEN 1 WHEN $9 THEN 2 ELSE 3 END,
  score DESC
LIMIT 30
```

> **⚠️ PARAM-GAP — parameter binding order MUST match the live `resolve_intent`**
> (verified `intent_system.rs:339-367`). The placeholders bind positionally in
> `tokio_postgres`/`deadpool-postgres` — a numbering gap or off-by-one is a runtime
> `parameters ... but statement requires ...` error, not a compile error. The 9 bind
> args MUST be passed in EXACTLY this order (same as the current production query):
>
> | `$N` | bind arg | source |
> |------|----------|--------|
> | `$1` | `tenant_id`   | `scope.tenant_id` |
> | `$2` | `user_id`     | `scope.user_id` |
> | `$3` | `agent_id`    | `scope.agent_id` |
> | `$4` | `project_id`  | `scope.project_id` |
> | `$5` | `query`       | the raw user text (no Rust normalisation) |
> | `$6` | `order_vec`   | the `input_class = ANY($6)` array (Vec<i16>) |
> | `$7` | `order[0]`    | CASE `WHEN $7 THEN 0` |
> | `$8` | `order[1]`    | `WHEN $8 THEN 1` |
> | `$9` | `order[2]`    | `WHEN $9 THEN 2` |
>
> A prior draft of this block wrote `ANY($7)` (skipping `$6`) and `CASE ... WHEN $8/$9/$10`
> (10 params, off-by-one) — that would crash at runtime. Fixed here to `$6` (ANY) +
> `$7/$8/$9` (CASE) = 9 params, matching the live code. The only structural change vs
> the current query is replacing the hard `AND input_text = $5` filter with the
> `AND (input_text = $5 OR <template paths>)` OR-group, and prefixing the ORDER BY with
> the `CASE WHEN input_text = $5 THEN 0 ELSE 1 END` exact-match tiebreaker. The
> parameter set and order are otherwise unchanged.

Dual-anchored templates (Path 3) are caught by Path 1 (prefix pre-filter fires, then
full `LIKE` validates the suffix naturally). No separate path 3 branch is needed in SQL.

---

### 0.17.2 New Columns and Indexes on `reborn_intent_inputs`

**Migration V058** (**was V057 before Decision 2**) adds three columns and two indexes:

```sql
-- V058__reborn_intent_inputs_template.sql  (was V057 before Decision 2)

ALTER TABLE reborn_intent_inputs
  ADD COLUMN is_template      BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN template_prefix  TEXT,   -- literal text before first %;  NULL for literals
  ADD COLUMN template_suffix  TEXT;   -- literal text after last %;    NULL for literals

-- Path 1: prefix-anchored templates
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_prefix_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, template_prefix)
    WHERE is_template = true AND template_prefix != '';

-- Path 2: suffix-anchored templates (reverse trick for leading-% case)
CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_suffix_rev_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, reverse(template_suffix))
    WHERE is_template = true AND template_prefix = '' AND template_suffix != '';
```

**Existing rows** (literal expressions) are unaffected: `is_template = false`,
`template_prefix = NULL`, `template_suffix = NULL`. The existing exact-match index
and all existing query paths are unchanged.

**Seeding a template row:**

```rust
fn seed_template_intent(expression: &str) -> (String, String, String) {
    // expression = "show me all files in the % directory"
    let prefix = expression.split('%').next().unwrap_or("").to_string();
    let suffix = expression.split('%').last().unwrap_or("").to_string();
    let suffix = if expression.contains('%') && suffix != prefix { suffix } else { String::new() };
    // → prefix = "show me all files in the "
    // → suffix = " directory"
    // input_text stored as the template string with % intact
    (expression.to_string(), prefix, suffix)
}
```

`input_text` stores the template string as-is (with `%`). The UNIQUE constraint
`(scope, input_text, input_class, component_id)` therefore naturally deduplicates
templates: two identical template expressions for the same component are one row.

---

### 0.17.3 Post-Match Value Extraction

Once a template row matches, the variable values must be extracted from the user text.

**Auto-extraction from template segments (no `variable_patterns` needed):**

Split the template on `%` → literal segments. Find each segment's position in the user
text in order. The substring between consecutive segments is the captured slot value.

```
template:  "show me all files in the % directory"
segments:  ["show me all files in the ", " directory"]
user_text: "show me all files in the /tmp directory"

Match segment[0] at position 0..25 → OK
Match segment[1] from right → " directory" ends at position 41
Slot[0] value = user_text[25..31] = "/tmp"
```

For multiple slots:
```
template:  "search for % in %"
segments:  ["search for ", " in ", ""]
user_text: "search for TODO in /src"

Slot[0] = "TODO"   (between segment[0] and segment[1])
Slot[1] = "/src"   (after segment[1] to end of string)
```

Auto-extraction is sufficient for most builtin Recipe variants. `variable_patterns`
is used when:
1. The extracted value needs **validation** (e.g. must start with `/`)
2. The extracted value needs **transformation** (e.g. strip quotes)
3. There are **overlapping templates** where the auto-extraction is ambiguous
4. The slot name matters for `{{vars.name}}` substitution (auto-extract assigns
   positional names `vars.slot0`, `vars.slot1`; `variable_patterns` assigns semantic names)

When `variable_patterns` is present, it runs after auto-extraction as a refinement:
the auto-extracted value is validated against the pattern. If it fails, the match is
demoted (not rejected — the template still matched; extraction just gets the raw value).

**Positional names:** Auto-extracted slots are named `slot0`, `slot1`, `slot2`, ...
in left-to-right order. `{{vars.slot0}}` in ToolBinding `params` references the first
slot. Authors who want semantic names (`{{vars.dir}}`, `{{vars.pattern}}`) add a
`variable_patterns` entry that maps the positional regex capture to the named variable.

---

### 0.17.4 Q1 Validation Rules for Templates

| Rule | Condition | Severity |
|------|-----------|----------|
| **No-anchor error** | `template_prefix = ''` AND `template_suffix = ''` (e.g. `"% in %"`, `"%"`) | **Hard error** — template too permissive; add literal text around each `%` |
| **Leading-`%` warning** | `template_prefix = ''` AND `template_suffix != ''` (e.g. `"% directory"`) | **Warning** — valid and indexed, but imprecise; consider adding a word before `%` |
| **Adjacent slots** | Two `%` with no literal text between them (e.g. `"search % %"`) | **Hard error** — adjacent slots are unextractable; separate them with literal text |
| **Dangling `variable_patterns`** | A `variable_patterns` entry whose `name` does not appear as `{{vars.name}}` in any ToolBinding `params` | **Warning** — pattern defined but never used |
| **Missing template** | `{{vars.slot0}}` used in ToolBinding `params` but expression has no `%`, and no `variable_patterns` | **Hard error** — variable referenced but no source defined |

---

### 0.17.5 Authoring in WebUI

In the intent expression field, `%` is rendered as a styled token (highlighted chip, not
plain text) so authors can see at a glance which parts of the expression are slots.

The field shows live feedback:
- **Green anchor indicator:** "Prefix anchor: `show me all files in the `" — anchored, fast.
- **Yellow anchor indicator:** "Suffix anchor only: ` directory`" — valid, leading-`%` warning shown.
- **Red indicator:** "No anchor — add literal text around `%`" — hard error, cannot save.

---

### 0.18 Validation Queue — Pre-Validation Lifecycle

> **⚠️ Revised by §0.23.2 + §0.23.5.** Two changes: (1) Q1 is now **orchestrated**
> (not deterministic/no-LLM) — see §0.23.2; the state-2 "only Gate 1 writes it"
> invariant is retained. (2) The **"non-overlapping states" invariant below is
> revised for upgrades**: a validated live row **stays validated and keeps serving
> retrieval** while a queue row carries an upgrade copy in the new `proposed_payload
> JSONB` column; Q2 approval overwrites the live row, Q2 rejection discards the copy.
> `validation_status='upgrade_queued'` is **not** set on the live row. For **new**
> components the original invariant (component `'pending'` + queue row, not served)
> is unchanged. See §0.23.5 for the authoritative upgrade model.

#### Two separate state machines

The validation system has two distinct phases, each with its own authoritative state:

```
Component created / edited
        │
        ▼
┌─────────────────────────────────────┐
│     reborn_validation_queue         │   PRE-VALIDATION
│                                     │   All components not yet manually approved
│  state 1 — Q1 queue                 │   live here. Erased on manual approval.
│  state 2 — Q1 passed (Gate 1 only) │
│  state 3 — rejected (back to fix)  │
│  state 4 — deletion candidate      │
│  counter  — rejection count         │
└─────────────────────────────────────┘
        │
        │  Manual approval (Q2) → row DELETED from queue
        ▼
┌─────────────────────────────────────┐
│  validation_status on component     │   POST-VALIDATION
│  table (existing, unchanged)        │   'validated' = active, trusted, in retrieval
│                                     │   'upgrade_queued' = re-entering queue
│  'validated' / 'upgrade_queued'     │   This system is untouched by this design.
└─────────────────────────────────────┘
```

The two systems do not overlap. A component row is either in the queue (not yet
manually approved) OR it has a `validation_status` that reflects its post-approval
runtime identity. It cannot be in both states simultaneously.

**Every component that is not yet manually validated must have a row in `reborn_validation_queue`.**  
A component with no queue row and `validation_status != 'validated'` is an inconsistent state
— detected and reported by an integrity check that runs at boot.

---

#### The queue states

| State | Value | Meaning | Who can write it |
|-------|-------|---------|-----------------|
| Q1 queue | 1 | Submitted, awaiting Gate 1 (automatic) validation | Application layer |
| Q1 passed | 2 | Gate 1 passed; awaiting Q2 manual review | **Gate 1 only** — never the application layer |
| Rejected | 3 | Q2 reviewer rejected; author must revise and resubmit | Q2 reviewer action |
| Deletion candidate | 4 | Too many rejections or manually condemned; awaiting cleanup | System (counter threshold) or Q2 reviewer |

**State 2 is the security invariant.** No API endpoint, no application-layer code path,
no direct SQL can set `state = 2`. Only the internal Gate 1 validator function transitions
a row to state 2 after a clean Q1 result. This is enforced at the application layer
(the only write path for state 2 is inside the validator) and documented as an
inviolable rule — any code that sets `state = 2` outside the validator is a security bug.

#### The rejection counter

`counter` starts at 0 on row insert. It increments by 1 each time a component is rejected
(state 2 → state 3, or state 3 → state 1 after author resubmits and is rejected again).
It never resets. It is a permanent rejection history for this component version.

When `counter` reaches a configurable threshold (default: 3), the queue system
automatically promotes the row to state 4 (deletion candidate) without requiring a
Q2 reviewer action. This prevents perpetually-stuck components from clogging the queue.

#### Table shape

```sql
CREATE TABLE reborn_validation_queue (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Scope — all reads and writes filter on the full tuple.
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    agent_id        TEXT        NOT NULL,
    project_id      TEXT        NOT NULL,

    -- The component this row tracks.
    component_id    UUID        NOT NULL,
    component_class SMALLINT    NOT NULL,   -- class_code; for WebUI filtering

    -- Lifecycle state. 1=Q1_queue 2=Q1_passed 3=rejected 4=deletion_candidate.
    -- State 2 may only be written by the Gate 1 validator.
    state           SMALLINT    NOT NULL DEFAULT 1
        CHECK (state IN (1, 2, 3, 4)),

    -- Permanent rejection count. Never resets. Increments on each rejection.
    counter         INT         NOT NULL DEFAULT 0,

    -- Human-readable feedback from Q2 reviewer (populated on rejection).
    review_feedback TEXT,

    -- Q1 error messages (populated on Q1 fail, cleared on Q1 pass).
    validation_errors TEXT[]    NOT NULL DEFAULT '{}',

    -- Timestamps
    submitted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- One queue row per component at any time.
    UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)
);

CREATE INDEX reborn_validation_queue_scope_state_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, state);

CREATE INDEX reborn_validation_queue_scope_class_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, component_class);

-- Partial index: state 4 (deletion candidates) for cleanup job.
CREATE INDEX reborn_validation_queue_deletion_idx
    ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id)
    WHERE state = 4;
```

#### What moves to the queue table from component tables

The following columns currently exist on every component table and describe
**pre-validation** lifecycle — they belong in the queue and are removed
from component tables (see Phase N):

| Removed column | Moved to queue as | Notes |
|----------------|-------------------|-------|
| `queue_code TEXT` | `state SMALLINT` | Queue state replaces the text queue_code |
| `review_attempts INT` | `counter INT` | Same concept, renamed and centralised |
| `review_feedback TEXT` | `review_feedback TEXT` | Moved to queue |
| `rejected_at TIMESTAMPTZ` | `updated_at TIMESTAMPTZ` on queue row | Queue row's `updated_at` serves this purpose |
| `validation_errors TEXT[]` | `validation_errors TEXT[]` | Moved to queue; cleared on Q1 pass |

**Columns that stay on component tables** (post-validation runtime identity):
- `validation_status TEXT` — `'validated'` is the retrieval gate. All retrieval queries
  (`WHERE validation_status = 'validated'`) continue to work unchanged.
- All content columns, reward columns, lineage columns — unchanged.

The net result: every component table loses 4–5 columns. The validation lifecycle
is managed entirely by the queue while the component is pre-validated, and entirely
by `validation_status` once it graduates.

#### Cache invalidation via queue graduation

When a component is manually approved (Q2 pass), its queue row is deleted. This
deletion event is the authoritative cache invalidation signal.

A companion column `last_graduation_at TIMESTAMPTZ` is added to the scope-level
settings table (or a new lightweight `reborn_scope_cursors` table — one row per scope):

```sql
-- Added to existing reborn_monty_vm_settings or a new reborn_scope_cursors table:
last_graduation_at TIMESTAMPTZ;
-- Updated by a trigger on reborn_validation_queue DELETE.
```

The SplitResult cache (§0.7 memoisation) checks `last_graduation_at` on every hit.
If it is newer than the cache entry's `cached_at`: discard all cache entries for this
scope. One sub-millisecond PK read. No TTL required as primary mechanism — eviction
is exact and event-driven.

This resolves Open Question 2 completely: the cache eviction mechanism is queue
graduation events, not polling `updated_at` or TTL expiry.

---

### 0.19 Dependency Registry

#### Every component owns a flat dependency registry

Every component table gains a `dependency_registry JSONB` column. It is a flat,
zero-indexed array of entries — each entry names one component this component depends on:

```json
[
  { "idx": 0, "component_id": "<uuid:pipe-skill>",     "class_code": 1,  "label": "pipe-skill" },
  { "idx": 1, "component_id": "<uuid:json-helper>",    "class_code": 22, "label": "json-helper" },
  { "idx": 2, "component_id": "<uuid:toolskill-read>", "class_code": 13, "label": "toolskill-read" }
]
```

- `idx` is the positional index used in traversal expressions on StepDescription steps.
- `label` is human-readable, shown in the WebUI registry editor.
- `class_code` drives channel routing (orchestrator vs rust) during traversal.
- The registry is flat — sub-dependencies are declared on the referenced components
  themselves, not nested here.

The `dependency_registry` is authored in the WebUI component page, editable as a table.
It is part of the component's validated content — changes require re-entry into Q1.

---

#### Traversal expressions on StepDescription steps

A step's `dependencies` field is a **traversal expression** that walks the registry tree
selectively. The expression is a comma-separated list of traversal nodes.

**Traversal node syntax:**

```
<idx>               — load component at index <idx>; no sub-dependencies
<idx>[all]          — load component at index <idx>; recursively load ALL of its
                      dependency_registry entries and ALL of their sub-dependencies
                      (full transitive closure from this node)
<idx>[<n>,<m>,...]  — load component at index <idx>; from its registry load only
                      indices <n>, <m>, ... (no further recursion unless nested)
<idx>[<n>, <m>[all], <p>[<q>,<r>]]
                    — mixed: index <n> (no sub-deps), <m> full transitive, <p> with
                      selective sub-indices <q> and <r>
```

**Example:**

```yaml
- stepnumber: 3
  knowledge: orchestrator
  type: component
  include: ["<uuid:skill-file-editing>"]
  dependencies: "1[all], 5[2,6], 17[3, 7[1, 4]]"
```

Resolution of `1[all], 5[2,6], 17[3, 7[1,4]]` against `skill-file-editing`'s registry:

```
1[all]
  → load registry[1] of skill-file-editing → <uuid:pipe-skill>
  → load ALL of pipe-skill's dependency_registry, recursively
      → pipe-skill.registry[0] → <uuid:tokenizer>  (no further deps)
      → pipe-skill.registry[1] → <uuid:buffer>
          → buffer.registry[0] → <uuid:allocator>  (leaf)
      → ... (full transitive closure)

5[2,6]
  → load registry[5] of skill-file-editing → <uuid:formatter>
  → load formatter.registry[2] → <uuid:indent-helper>  (no sub-deps)
  → load formatter.registry[6] → <uuid:escape-helper>  (no sub-deps)

17[3, 7[1,4]]
  → load registry[17] of skill-file-editing → <uuid:validator>
  → load validator.registry[3] → <uuid:schema-checker>  (no sub-deps)
  → load validator.registry[7] → <uuid:type-coercer>
      → load type-coercer.registry[1] → <uuid:int-parser>   (no sub-deps)
      → load type-coercer.registry[4] → <uuid:float-parser> (no sub-deps)
```

---

#### Resolution algorithm (at fetch time, not pre-embedded)

The traversal expression is stored as a string in the StepDescription JSONB. The IBS
parses it into a typed `DependencyExpr` tree at compile time (pure Rust, no DB). The
**actual component fetching** happens in `fetch_for_turn` after IBS compilation, as
`fetch_component_by_id` calls per resolved UUID.

```
resolve_dependencies(
    root_component_id: Uuid,
    expr: &DependencyExpr,
    pool: &PgPool,
    visited: &mut HashSet<Uuid>,    // deduplication + cycle guard
) -> Vec<ComponentItem>

For each node in expr:
  1. Look up root_component.dependency_registry[node.idx]
       → (dep_uuid, dep_class_code)
  2. If dep_uuid ∈ visited → skip (already collected or cycle)
  3. visited.insert(dep_uuid)
  4. fetch_component_by_id(dep_uuid) → ComponentItem
  5. Route by dep_class_code → orchestrator or rust channel
  6. If node has sub-expression:
       If sub-expression == All:
           fetch dep_component.dependency_registry (one DB read)
           for each entry: resolve_dependencies(dep_uuid, All, pool, visited)
       Else:
           resolve_dependencies(dep_uuid, sub_expr, pool, visited)
  7. Collect result
```

**Deduplication:** `visited` is shared across the entire `fetch_for_turn` call for a
given turn. A component UUID collected by any step's dependency traversal is not
fetched again — regardless of which step triggered it.

**Eager-loading rule:** On the first occurrence of a Skill UUID in the assembled step
list, all dependencies declared on that step are resolved immediately. On subsequent
steps that reference the same Skill UUID: the Skill itself is already in `visited` →
skipped. Its dependencies were already loaded at the first occurrence.

**Cycle protection:** The `visited` set prevents infinite recursion. If component A
depends on B and B depends on A (or any longer cycle), the second encounter of either
UUID is skipped silently. Q1 detects and rejects cycles statically (see below).

---

#### KV-cache interaction

In steady state, the basic-prompt prefix already contains the bodies of all commonly-used
Skills, ToolSkills, and PythonCode helpers. The dependency traversal is a graph walk
over components that are **already in the LLM's context**. For each resolved dependency
UUID, the IBS checks whether that UUID appears in `basic_prompt_section_refs` — if so,
it emits a section reference instead of re-injecting the body. The token cost of
dependency resolution in steady state is therefore near zero: the graph walk happens in
Rust (fast), the components are already in the KV-cache prefix (no token cost).

New or recently-validated components not yet in the prefix do incur a full body
injection into the per-turn patch. This is the transient case — it resolves after the
next basic-prompt rebuild.

---

#### Q1 validation rules for dependency_registry and traversal expressions

| Rule | Condition | Severity |
|------|-----------|----------|
| **Self-reference** | Registry entry points to the same component's own UUID | Hard error |
| **Invalid UUID** | Registry entry UUID does not resolve to any known component in this scope | Hard error |
| **Out-of-range index** | Traversal expression references an index that does not exist in the component's registry | Hard error |
| **Cycle detection** | Traversal expression (with `[all]`) would follow a dependency cycle | Hard error (static DFS from the traversal root) |
| **Adjacent `[all]` depth** | `[all]` on a component whose own registry also contains `[all]` entries — warn author of potential large transitive closure | Warning |
| **Unparseable expression** | `dependencies` string fails the traversal expression parser | Hard error (parse error message included) |

Cycle detection at Q1 is a **static DFS** starting from the component being validated,
following all `[all]` and explicit sub-expressions, using the current state of all
referenced components' registries. Any back-edge in the DFS is a cycle → hard error on
the component that closes the cycle.

---

#### Relationship to `required_skills` (Phase J)

**`required_skills` on the `reborn_skills` table does not exist.** It was a previous
design that placed dependency declarations on the Skill component itself. Under the
dependency registry model, dependencies are declared:
1. On the component's own `dependency_registry` (what it depends on)
2. On the StepDescription step via the traversal expression (how deep to follow)

Phase J is replaced by the dependency registry implementation (see revised Phase J below).

---

### §0.20 Recipe Authoring Rules (`§recipe-authoring-rules`)

These rules govern how Recipes and their PythonCode components must be written. They are
enforced at Q1 validation time (Phase I) and documented here so authors understand the
architectural constraints before authoring.

#### §0.20.1 Step isolation invariant (DESIGN-DECISION-01)

**PythonCode steps in `orchestrator_steps` are isolated execution units.**

Each PythonCode body is executed by `execute_recipe_orchestrator_channel` with a **fresh
empty state dict `{}`**. A step does NOT see any state mutations from previous steps.

**Why:** The orchestrator is the information broker. If step 2 needs data produced by step 1,
the recipe must be designed so that:
1. The orchestrator reads the recipe's `step_descriptions` and knows what each step produces.
2. Step 2's PythonCode body calls `__execute_action__` directly to obtain the data it needs
   — it does not read a shared `state` key set by step 1.
3. If the orchestrator itself needs to pass context between steps (e.g. step 1 extracts a
   path, step 2 uses that path), this must be modelled in the recipe as a step that the
   **orchestrator** executes (an Action or a pkr-level variable), not a state side-effect.

**Consequence for recipe design:**

| Anti-pattern (WRONG) | Correct pattern |
|----------------------|-----------------|
| Step 1 sets `state["extracted_path"] = result`; Step 2 reads `state["extracted_path"]` | Both steps are self-contained; the orchestrator assembles pkr with `{{vars.path}}` from the template match, which is available to every step as a template variable |
| Step 1 calls `__execute_action__("read_file", ...)` and stores result; Step 2 post-processes it | Combine both operations into a single PythonCode step body that calls read_file and formats the result in one atomic step |
| Step 2 depends on Step 1 having run a particular tool | Model as a single PythonCode body that runs both tool calls in sequence and formats the combined result |

**Single-step is preferred for Tier-0 recipes.** Most Tier-0 recipes should have exactly
ONE PythonCode step in `orchestrator_steps` that: (1) calls `__execute_action__`, (2) handles
the result, (3) formats and assigns `result`. Multi-step is only needed when two genuinely
independent capabilities (different tools, different result shapes) must both contribute to
the output. In that case each step is self-contained per the isolation invariant.

#### §0.20.2 PythonCode body contract

A PythonCode body is a Python code string executed by `__execute_code_step__` in the Monty VM.
It has access to:

| Symbol | Source | Notes |
|--------|--------|-------|
| `__execute_action__(name, params)` | VM host function | Call a registered tool/capability |
| `__execute_actions_parallel__(calls)` | VM host function | Parallel tool calls (list of `{name, params}`) |
| `__check_budget__()` | VM host function | Check remaining time/token budget |
| `__emit_event__(kind, **data)` | VM host function | Emit a structured event |
| No `state` from previous steps | — | See isolation invariant above |
| No `goal`, `pkr`, `context` | — | These are orchestrator-layer globals; NOT injected into step scope |

**Required:** The body must assign `result = <some value>` before returning. The caller
reads `vm_result["return_value"]` as the step output. A body that never assigns `result`
produces an empty string in `result_parts`.

**Forbidden:** All patterns listed in the shell-injection scan (FIND-AUDIT-12 / Phase B):
`import os`, `import subprocess`, `exec(`, `eval(`, `open(`, etc.

#### §0.20.3 Template variable availability

Template variables (`{{vars.name}}`, extracted by `extract_template_slots` in Phase M) are
substituted into `orchestrator_content` by the IBS **before** `execute_recipe_orchestrator_channel`
is called. By the time any PythonCode step body runs, variables have already been baked into
the step body text (the IBS applies `{{vars.name}}` → literal value substitution when
formatting `orchestrator_content`). The PythonCode body therefore sees literal values, not
template placeholders.

**Example** — a recipe with `step_link = "0:0-0:E"` and intent `"read the file at %"`:
- User says: `"read the file at /tmp/foo.txt"`
- Template match extracts `slot0 = "/tmp/foo.txt"`
- IBS substitutes `{{vars.slot0}}` → `"/tmp/foo.txt"` in the PythonCode body text
- The PythonCode body sees: `tool_output = __execute_action__("read_file", {"path": "/tmp/foo.txt"})`
- Not: `tool_output = __execute_action__("read_file", {"path": "{{vars.slot0}}"})`

This means the PythonCode body does NOT need to parse template variables at runtime — they
are already resolved.

#### §0.20.4 Q1-enforced recipe design rules

| Rule | Condition | Source |
|------|-----------|--------|
| No Skill in `orchestrator_steps` for Tier-0 | `llm_call_required: false` + Skill UUID in `orchestrator_steps` | Phase I §tier0-orchestrator-channel Rule 1 |
| PythonCode required when tool_bindings present | `llm_call_required: false` + non-empty `tool_bindings` + empty `orchestrator_steps` | Phase I §tier0-orchestrator-channel Rule 2 |
| Shell/spawn tools require Tier 1 | `builtin.shell` or `builtin.spawn_subagent` in `rust_steps` + `llm_call_required: false` | Phase I §shell-guard |
| PythonCode body scan | `import os`, `exec(`, `open(`, etc. in body content | Phase I + FIND-AUDIT-12 |
| All step `include` UUIDs must be valid UUID v4 | Non-parseable UUID string | Phase I |
| No `snippet`-type steps | Step type = `snippet` in step_descriptions | Phase I |
| Each PythonCode step is self-contained | Steps must not read state from other steps | Architecture (§0.20.1) — not mechanically Q1 checkable, enforced by documentation |

### §0.21 Global Token-Budget Kill Switch (user item, Answer 5 of the doc-conversion review)

> **Subsystem:** A single operator-facing toggle that, when disabled, makes
> **token budgets play no role anywhere in the code** — not used in any
> decision or function. When re-enabled, every token budget is enforced
> again exactly as today.
> **Grounded in:** the live token-budget consumers —
> `crates/brassclaw_product_workflow/src/settings.rs:48`
> (`prior_knowledge_token_budget: u32`, default `100_000` at `:193`, update
> DTO `Option<u32>` at `:64`),
> `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs`
> (`PgMontyVmSettingsStore` persists it to `reborn_monty_vm_settings`,
> read `:83`, upsert `:142-190`),
> `crates/brassclaw_engine/src/executor/orchestrator.rs:2844`
> (`handle_check_budget` — the `__check_budget__` VM host fn reading
> `thread.config.max_tokens_total` / `max_duration` / `max_budget_usd`),
> `crates/brassclaw_engine/src/types/thread.rs` (`ThreadConfig` token/time/
> USD caps), `crates/brassclaw_agent_loop/src/token_budget.rs`
> (`TokenBudgetTracker` + `estimate_tokens`), `fetch_for_turn(scope, query,
> token_budget, consumer_tag)` (retrieval budget, §0.11),
> `crates/brassclaw_skills/src/registry.rs` (skill budget),
> `crates/brassclaw_interceptor/src/packet.rs` (interceptor packet budget),
> and the per-request output `max_tokens` across `crates/brassclaw_llm/*`.

**Concept.** A single per-scope boolean `token_budgets_enabled` (default
`true`), persisted on `reborn_monty_vm_settings` (the existing per-scope
settings table — V034 — that already holds `prior_knowledge_token_budget`,
so no new table). A `TokenBudgetPolicy { enabled: bool }` resolver reads it
once at turn start and is threaded into every consumer. When `false`, every
"over budget?" / "how many tokens remain?" / "drop/truncate on budget?"
decision returns *no limit / infinite / do not drop*.

**Exact boundary — what the switch controls (token budgets only).** When
disabled, the following become no-ops / unlimited:

| Consumer | File:line | Behaviour when disabled |
|----------|-----------|--------------------------|
| prior-knowledge injection cap | `settings.rs:48` + `fetch_for_turn` budget | `prior_knowledge_token_budget` ignored; the full assembled prior-knowledge blob is injected (no truncation). `fetch_for_turn` receives `usize::MAX`. |
| message-selection token tracker | `token_budget.rs` (`TokenBudgetTracker`) | `remaining()` = `usize::MAX`; `would_exceed` always `false`; messages are never dropped on token-budget grounds. |
| `__check_budget__` tokens | `orchestrator.rs:2844` | `tokens_remaining` = `u64::MAX` — the orchestrator's "stop on token exhaustion" branch never fires on token grounds. |
| `ThreadConfig.max_tokens_total` | `types/thread.rs` | enforcement skipped when disabled. |
| skill budget | `skills/registry.rs` | unlimited. |
| interceptor packet budget | `interceptor/packet.rs` | unlimited. |
| LLM per-request output `max_tokens` | `brassclaw_llm/*` | set to the provider's documented maximum output (or omitted when the provider treats absence as max) — generation is not token-truncated. |

**What the switch does NOT control (explicitly out of scope).** Time budget
(`max_duration` / `time_remaining_ms`) and USD budget (`max_budget_usd` /
`usd_remaining`) are **separate resource limits, not token budgets**; they
remain enforced. `handle_check_budget`'s `time_remaining_ms` and
`usd_remaining` fields are unchanged by this switch. They remain as
cost/runaway backstops.

**Policy type.** `TokenBudgetPolicy { enabled: bool }` lives in
`brassclaw_reborn_composition` (co-located with `PgMontyVmSettingsStore`).
`enabled()` returns the bool. The single substitution point every consumer
uses is `cap_or_unlimited(cap: usize) -> usize` → returns `cap` when
enabled, `usize::MAX` when disabled. Consumers call this instead of reading
their cap directly, so the switch is one read at turn start + one helper —
no scattered `if enabled` ladders.

**Per-scope, not cross-tenant.** The setting rides the existing
`reborn_monty_vm_settings` scope (tenant + agent, `user_id`/`project_id`
per-call — the same scope as `prior_knowledge_token_budget`). "Everywhere
in the whole code" means everywhere for this agent's execution. A
cross-tenant global is **not** the design; the existing per-scope settings
page is the surface.

**Safety.** Disabling token budgets removes a cost/runaway guard. The
toggle is operator-only (the bearer-authenticated Monty-VM settings
endpoint), its writes are logged, and time + USD limits stay enforced as
backstops. The WebUI toggle carries help text stating the cost implication.

**Implementation:** Phase O, migration `V060` (see §2). No other phase
depends on it; it is additive and independently shippable.

### §0.22 Doc-Conversion Mechanism (auto doc→DB conversion for the base-prompt prefix; user repeat item 4)

> **Subsystem:** Automatically converts each `docs/agents-v3/*.md` into an
> LLM-optimized form, stores it in the DB, keeps it fresh on change, and
> injects it into the base-prompt prefix (+ per-turn retrieval). Built **as
> v3 agent artifacts** (Recipe + Skills + Tools + PythonCode + Action), **not
> Rust code** — the agent operates on itself through the same component
> catalog + execution paths it uses for every other task.
> **Implementation:** Phase P (prerequisites Phase P.0 + P.1). No new
> migration (reuses live V040; see §2).
> **Grounded in:** the 17 per-system docs `docs/agents-v3/01..17-*.md` (each
> carries a §7 "LLM-summary (machine-convertible)" section),
> `15-component-catalog.md` (class 17 = Docu),
> `crates/brassclaw_pg/migrations/V040__reborn_docus.sql` (Docu schema),
> `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs`
> (`COMPONENT_TABLES:47`, `class_label:65`, `do_reassemble:204`),
> `crates/brassclaw_reborn_composition/src/component_import.rs`
> (`content_hash` idempotency), `10-prefix-base-prompt.md` +
> `17-webui-prefix-tab.md` (prefix caching, `reborn_basic_prompt_store` V056,
> `mark_stale`-on-graduation), `14-validation-queue.md` (Q1/Q2),
> `08-actions-system.md` (class-16 Action no-LLM `execute_action_procedure`),
> `03-recipe-system.md` (Tier-0/1/2), `07-pythoncode-system.md` (class 22),
> `05-skills-system.md` / `06-tools-system.md`, and §0.13 / §0.16 / §0.18 /
> Phase K.1 of this plan. Full design: `docs/agents-v3/DOC_CONVERSION_MECHANISM_DESIGN.md`.

#### §0.22.1 Goal & scope

The mechanism (user repeat item 4): (1) **convert** each `docs/agents-v3/*.md`
into an LLM-optimized (token-efficient, prompt-ready) form; (2) **insert** it
into the DB; (3) **auto-update** the stored converted docs when source docs
change; (4) make converted docs **injectable into an LLM prompt** if needed or
selected; (5) make the **prefix prompts (base prompt) contain** these converted
+ optimized docs; (6) **run on change events only** — a source
`docs/agents-v3/*.md` file change on disk, or a `reborn_docus` row change in the
DB (Answer 4). **No idle-time loop, no scheduled cadence, no boot trigger.**
(7) Built as v3 agent artifacts, not Rust code.

The Sempai-Kohai idle-time self-optimization loop (item 7, `09-sempai-kohai.md`)
is **not** in the refresh loop. Its only role: when the `by-llm-compress`
variant runs and a Sempai is connected, the Sempai reviews the compression
prompt before shipment; without a Sempai, only the deterministic `by-extract`
variant runs.

#### §0.22.2 Storage — `reborn_docus` (class 17), store BOTH versions (Answer 1)

Class 17 = Docu is the catalog's reserved home for reference documentation
(`15-component-catalog.md`) — not a Spec (12), Note (20), or ExtensionCatalogue
(23). `reborn_docus` (V040) already has every column needed:

| Column | Use |
|--------|-----|
| `name` | stable key = source doc slug, e.g. `agents-v3::02-intent-system` (`UNIQUE(tenant,user,agent,project,name)`) |
| `content` | per-row text — source row = unconverted markdown; converted row = LLM-optimized text (`do_reassemble` reads the converted row) |
| `prior_knowledge_content` (SCH-02) | richer form for the per-turn retrieval path (`PostgresSource`) |
| `class_code` | `17` (CHECK `= 17`) |
| `prompt_uid` | sequence — stable base-prompt ordering key |
| `consumer_tags` | `{03:llm}` (and `{02:orchestrator}` where relevant) — **never `05:validator`** on a graduated row or `do_reassemble` excludes it |
| `validation_status` | `'validated'` **only after full Q1+Q2 graduation** (Answer 2); upsert always sets `'pending'`; no bypass path |
| `source` | provenance label only (`'system'`/`'authored'`/`'migrated'`); no `source` CHECK on `reborn_docus`, so `'system'` is allowed with no migration. **Never gates validation** (Answer 2) |
| `content_hash` | SHA-256 of the source `.md` — the staleness key (mirrors `component_import.rs`) |
| lineage (`similarity_parent_id`, `replaces_id`, `parent_version`, `last_audit_at`, `audit_failure_count`) | links source → converted row; versions the converted doc across regenerations |

**Storing both versions (Answer 1)** — two `reborn_docus` rows per doc, linked
by lineage, no schema change:

| Row | `name` convention | `content` | `source` | `validation_status` | role |
|-----|-------------------|-----------|----------|---------------------|------|
| **source** | `agents-v3::02-intent-system` (slug) | unconverted source markdown | `'system'`/`'authored'` | Q1+Q2 → `'validated'` | auditable original; `content_hash` computed from this |
| **converted** | `agents-v3::02-intent-system::llm` | LLM-optimized text | `'system'` | Q1+Q2 → `'validated'` | what `do_reassemble` reads into the base prompt |

The converted row points at the source row via `similarity_parent_id`
(`replaces_id` on re-conversion), carrying `parent_version`. A re-conversion
writes a new converted-row version (lineage); the active converted row is the
one with `validation_status='validated'`. Both rows pass Q1+Q2 independently.

**The one composition prerequisite (not a migration).** `reborn_docus` is **not**
in `COMPONENT_TABLES` (`interceptor_config_service.rs:47`) and `class_label`
(`:65`) has no `17` arm (falls to `_ => "Component"`), so `do_reassemble`
(`:204`) does not read Docu rows today. For converted docs to flow into the
base prompt (item 4.5), add `("reborn_docus", 17)` to `COMPONENT_TABLES` and
`17 => "Docu"` to `class_label`. This is an **additive Rust const edit in
composition**, not SQL — no data movement, no row breakage (consistent with how
`reborn_orchestrators`/`reborn_scaffolds` are already listed and gracefully
skipped when absent). This is the single piece of "host" code the mechanism
needs; everything else is v3 artifacts.

#### §0.22.3 The conversion + no per-doc token budget (Answer 5)

Each source doc follows the 7-section convention (§1 Purpose · §2 Location ·
§3 Data model · §4 Behavior · §5 Relations · §6 Today vs v3 · **§7 LLM-summary
(machine-convertible)**). §7 was authored specifically to be machine-convertible,
so the conversion is largely **deterministic** (extract §7 + header metadata)
with an **optional LLM-assisted compression** pass.

The converted form stored in `reborn_docus.content` matches `do_reassemble`'s
render format exactly:
```
## 17:{prompt_uid}  Docu  "{name}"

<doc-slug> · <one-line description>
<§7 LLM-summary verbatim, or LLM-compressed>
```
so `do_reassemble` just concatenates it with the other validated components,
ordered by `(class_code, prompt_uid)`.

**No per-doc token budget (Answer 5).** Docs are not budget-limited; the
deterministic §7 extract is used verbatim, and the LLM compression pass runs
for clarity/injection-safety, **not** to hit a token ceiling. Whether any token
budget applies anywhere in the codebase is governed by the global kill switch
(§0.21 / Phase O).

#### §0.22.4 The v3 artifacts + the generic-DB-tool rule (Answer 3)

**Recycling — the v3 composition principle (applies to every future recipe,
not just this one).** Skills should be as small as practical — at best the
description of ONE tool usage — so they can be reused in many recipes. Tools
too: one concern each. A Recipe is a *composition* of already-existing Skills +
Tools; prefer reusing a library part over authoring a new one. Never bake a
whole procedure into one fat skill — split it into leaves the library can
recycle. Two skill grains coexist (`05-skills-system.md` §3): **leaf**
Orchestrator Skills (one tool / one pythoncode — the reusable unit, user case
(a)) and **domain** Orchestrator Skills (span tools — the bigger picture that
*references* leaves by name, never duplicates their tool instructions; user
case (b); one per mechanism).

**Tool vs Skill — the generic-DB-tool rule (Answer 3).** A Tool (class 0) is
the opaque Rust capability that touches Postgres (the kernel boundary); keep it
**maximally generic**. **One generic DB-reading (and DB-writing) Tool
suffices** — not a per-purpose Tool per read/write/stale-mark. The per-purpose
specificity lives in **Skills** (one per reading/writing approach) and in
**sub-recipes** that tell the orchestrator how to use each skill to call Rust
to read from / write to the DB in a certain way. The single DB Tool is recycled
by every future "sync" recipe; only the skills/sub-recipes differ.

**The artifact set for this mechanism:**

| Kind | Class | Channel | What it is here |
|------|------|---------|-----------------|
| Action | 16 | orchestrator (no IBS, no LLM) | `doc-sync` — deterministic driver, composes leaves |
| Recipe | 21 | orchestrator (IBS); routes sub-steps | `doc-convert` — per-doc converter, composes leaves |
| Orchestrator Skill (leaf) | 1-3 | orchestrator | one-tool-each reusable leaves |
| Orchestrator Skill (domain) | 1-3 | orchestrator | `doc-convert-method` — the one doc-specific overview |
| ToolSkill | 13 | **rust** | executor-facing param schema, one per Tool |
| Tool | 0 | **rust** | Rust capability; the only kind that touches Postgres |
| PythonCode | 22 | orchestrator | pure-logic helpers, one concern each, no I/O |
| ExtensionCatalogue | 23 | namespace | `doc-sync` namespace + `overview_doc` |

**Channel rule.** The IBS splits a Recipe's `include` list into
`orchestrator_items` (Skill + PythonCode) and `rust_items` (ToolSkill). The
orchestrator never calls a Tool directly and never holds a DB handle; it drives
the executor, which calls the Tool (guided by its ToolSkill). So a DB write is
always a **Tool + ToolSkill**, never a PythonCode. The LLM prompt is authored
**in the Recipe's `type: llm` step**, built *from* the Skill body — the Skill
is the reusable method, not the prompt.

**Leaf library (§4.1 of the design doc)** — each entry one-purpose and reusable
beyond `doc-convert`. Per the generic-DB-tool rule, the three DB-access shapes
are **one generic Tool + three leaf skills** (not three Tools):

- **Leaf Orchestrator Skills (1-3):** `file-list` (glob), `file-read` (read_file),
  `hash-compute` (sha256 PythonCode), `hash-compare` (hash_changed PythonCode),
  `db-read-hash` (`component_db` op=read_hash), `db-upsert-docus`
  (`component_db` op=upsert, table=reborn_docus — always `pending`, §0.22.7),
  `db-mark-prefix-stale` (`component_db` op=mark_stale), `markdown-section`
  (markdown_section PythonCode), `component-header-render`
  (format_component_header PythonCode), `prompt-compress` (`__llm_complete__` —
  reusable for ANY compression; no token budget, Answer 5).
  (`token-estimate` is removed — no per-doc token budget.)
- **PythonCode (22):** `sha256`, `hash_changed`, `markdown_section`,
  `format_component_header` — pure logic, one concern each, no I/O.
- **Tool (0):** `read_file`/`glob`/`memory_*` reused as-is; **`component_db`**
  `(op ∈ {read_hash, read_row, upsert, mark_stale}, table, scope, name, fields?)`
  — the one generic DB Tool. `upsert` does `INSERT … ON CONFLICT (scope,name) DO
  UPDATE` and **always sets `validation_status='pending'`**; `mark_stale` wraps
  `PgBasicPromptStore::mark_stale` (Phase K.1). One ToolSkill (13) covers the
  uniform `op` surface; per-`op` nuances live in the leaf skills + sub-recipes.

**Domain Skill — `doc-convert-method` (case b).** DB-stored Classic skill
(`05-skills-system.md` 5.1), the one doc-specific overview: the §7 source
shape; the pipeline (`file-read` → `markdown-section` → [`prompt-compress` if
noisy] → `component-header-render` → `db-upsert-docus`); the converted-form
render; the extract-vs-compress rule (compress when the §7 extract is noisy or
quotes injection payloads — **no token budget**); "never invent facts — only
compress what is in the source"; "quote injection payloads only as fenced,
escaped code" (so Q1 passes). `source='system'` (provenance); goes through
Q1+Q2 — no bypass (Answer 2).

**Recipe — `doc-convert` (class 21).** Converts one doc; steps `include` leaf
UUIDs + the domain skill. Step 1: include `doc-convert-method` (domain). Step 2:
`file-read` + `read_file` ToolSkill → read `{path}`. Step 3: `markdown-section`
(extract §7) + `hash-compute` (content_hash). Step 4 (`by-llm-compress` only,
`type: llm`): `__llm_complete__` using `prompt-compress`'s rubric; prompt
assembled from the domain skill + §7 text; Sempai reviews before shipment when
connected. Step 5: `component-header-render` → `## 17:…` header. Step 6 (rust):
`db-upsert-docus` + `component_db` ToolSkill (op=upsert) → executor writes
**both** rows (source + converted), each `validation_status='pending'`, linked
by lineage. Variants: `by-extract` = steps 1,2,3,5,6 (Tier 0, no LLM);
`by-llm-compress` = steps 1,2,3,4,5,6 (Tier 1).

**Action — `doc-sync` (class 16).** `execute_action_procedure`, no LLM: (1)
`file-list` + glob → list `docs/agents-v3/*.md`; (2) per doc `file-read` →
source; (3) `hash-compute` → content_hash; (4) `db-read-hash` (op=read_hash) →
stored hash; `hash-compare` → changed? skip unchanged (mirrors
`component_import.rs`); (5) changed → run `doc-convert` `by-extract` inline →
`db-upsert-docus` writes both rows (pending); if §7 needs compression, **enqueue**
`by-llm-compress` (run when a Sempai is connected; not idle-time); (6) if any
changed → `db-mark-prefix-stale` (op=mark_stale) → light up the Prefix Tab
regenerate button; (7) report (N scanned, M changed, stale=yes/no).

**ExtensionCatalogue — `doc-sync` (class 23).** Groups the doc-specific parts
(domain skill, `db-upsert-docus`/`db-mark-prefix-stale` leaves over the one
`component_db` Tool, Recipe, Action) under the `doc-sync` namespace with an
`overview_doc`. The general-purpose leaves live in the matching *builtin*
catalogue and are only referenced. `source='system'`; goes through Q1+Q2 — no
bypass (Answer 2; revises the §0.16 / `17-webui-prefix-tab.md` bootstrap-bypass
pattern — see Phase P.0).

#### §0.22.5 The refresh loop — event-driven (file / DB change only, Answer 4)

**No auto-refresh, no idle-time loop, no cadence, no boot trigger.** The
Kohai/Sempai system is not in the refresh loop. (1) **File-change trigger:** a
watcher on `docs/agents-v3/*.md` fires `doc-sync` when a source doc changes on
disk. (2) **DB-change trigger:** a `reborn_docus` row change (doc edited via
the WebUI Docs section, §0.22.7, or a Sempai-proposed re-compression graduating)
fires `doc-sync` for the affected slug. (3) **Staleness-driven, not
cadence-driven:** O(N) hash compares on the changed set only; `content_hash` is
the staleness key; no periodic full sweep. (4) **Base-prompt invalidation:** on
any change, the Action calls `db-mark-prefix-stale` (op=mark_stale); the Prefix
Tab shows stale; regeneration re-runs `do_reassemble` (now reading the freshly
converted Docu rows, prerequisite §0.22.2) and re-prewarms the KV cache. (5)
**No per-turn cost:** conversion is event-time, not per-turn; per-turn prompts
carry only the `base-prompt` placeholder + a <4k patch (§0.13).

#### §0.22.6 Injection paths (how converted docs reach a prompt)

Three paths, all already in the architecture: (1) **Base prompt (bulk) — the
prefix:** once `reborn_docus` is in `COMPONENT_TABLES`, `do_reassemble` reads
every validated Docu row into the base-prompt bundle; the Prefix Tab compiles
it (item 4.5). (2) **Per-turn retrieval (selected):**
`PostgresSource::fetch_for_turn` returns intent-relevant Docu rows for a turn's
prior-knowledge assembly (item 4.4). (3) **Section pointers (navigational):**
for docs not in the base prompt, `basic_prompt_section_refs` (§0.13) carries
pointers — the LLM already has the body from the KV cache, so the per-turn patch
only references it.

#### §0.22.7 Validation — nothing bypasses Q1+Q2 (Answer 2) + WebUI Docs section

Per `14-validation-queue.md` and Answer 2 — **nothing ever bypasses Q1+Q2:**
- **Every converted doc goes through the full Q1+Q2 queue.** Upsert sets
  `validation_status='pending'` → Q1 (Gate 1) → Q2 (review) → `'validated'`.
  This includes the agent's own `docs/agents-v3/*.md` conversions,
  system-authored docs, and any Sempai-proposed re-compression. There is **no
  `source='system'` Q2-bypass path**; `source` is provenance only and never
  gates validation. (The earlier "system bypass Q2" pattern is removed — Answer 2.)
- **Q1 (Gate 1, automatic)** runs on every converted doc
  (`component_validator.rs`): structural check + injection scan. A converted doc
  that accidentally contains an injection pattern (e.g. the source documents
  prompt-injection and §7 quotes a payload) fails Q1 — the converter sanitizes
  (the domain skill + `prompt-compress` both carry "quote injection payloads
  only as fenced, escaped code").
- **Q2 (review)** graduates the doc. For system-authored/builtin docs this is an
  **automated-but-auditable** Q2 graduation recorded in the queue (never a silent
  skip) — this requires the validation-system extension (Phase P.0); until it
  exists, system-authored conversions cannot be marked `validated` and will not
  reach the base prompt. For operator-authored/Sempai-proposed conversions, Q2
  is the human reviewer. Q2 approval → `db-mark-prefix-stale` (op=mark_stale) →
  Prefix Tab regenerate.
- **WebUI Docs section (Answer 2).** A new WebUI section lists `reborn_docus`
  rows (source + converted, with validation status) and allows manual editing.
  **Saving an edited doc sends it to the validation queue again**
  (`validation_status='pending'`, enqueued to `reborn_validation_queue`) — it
  never writes `validated` directly. Mirrors the existing validation-queue tab
  pattern
  (`./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/validation-queue-tab.js`).
- **Lineage.** A re-conversion that changes content writes a new row version via
  the lineage columns; the active row is the one with `validation_status='validated'`.

---

### 0.23 Sempai-Driven Auto-Creation & Self-Improving Orchestrated Validation

> **Subsystem:** The v3 direction that **brings the §4 "Out of Scope" self-improvement
> items INTO scope** and **upgrades Q1 from pure-Rust to orchestrated**. Four ideas,
> resolved by collaborative design Q&A (recorded inline):
> 1. **Sempai auto-creates ALL component types** (not just recipes) → Q1 queue.
> 2. **Q1 validates by running an orchestrator instance** that exercises the
>    component (security / performance / token-budget / v3-design adherence), with
>    Sempai help — the validation system is **itself built from v3 components**.
> 3. **The validation system is self-improving** — it sends validation prompts to the
>    kohai-sempai system; the kohai saves the prompts + Sempai answers (as references
>    to existing components) so the validation system's own components can be upgraded.
> 4. **Everything new — including new validation-system components — must be
>    validated** through Q1+Q2 (no bypass).
>
> **Implementation directive (Q7):** fold this functionality into the **already
> existing plan steps** for the validation system and the kohai-sempai system — do
> NOT create trailing phases Q/R/S. The folding map is §0.23.11.
>
> **Grounded in:** `crates/brassclaw_interceptor/` (Sempai/Kohai, `SempaiProposalSink`,
> `ForensicPacket`/`CapturedPrompt`/`PromptSegment`, `PgInterceptorStore`),
> `crates/brassclaw_reborn_composition/src/sempai_proposal_sink.rs`
> (`PgSempaiProposalSink` — recipes-only today),
> `crates/brassclaw_engine/src/memory/component_validator.rs` (pure-Rust Q1 today),
> `crates/brassclaw_host_runtime/` (`sandbox_process/`, `services/process_executor`,
> `first_party_tools/` — the sandboxed orchestrator runtime),
> `docs/agents-v3/09-sempai-kohai.md` + `14-validation-queue.md` +
> `15-component-catalog.md`, and §0.14 / §0.15 / §0.16 / §0.18 of this plan.

#### 0.23.1 Scope changes (§4 items brought into scope)

The following §4 "Out of Scope (Marked Postponed)" items are **now IN SCOPE**,
implemented per this section:

- ~~Full self-improvement pipeline (Interceptor-driven Recipe auto-creation)~~ →
  IN SCOPE (§0.23.6 + §0.23.8). Generalised from Recipe-only to **all component
  types** (tools, tool_skills, skills, extensions, python_code,
  extension_catalogues, recipes, **intents**).
- ~~Component self-creation wizard~~ → IN SCOPE (§0.23.6). The "wizard" is the
  Sempai auto-creation path itself; WebUI manual authoring remains as the
  operator-in-the-loop review/edit path (§0.23.5).
- ~~Automatic Sempai-driven prompt rewrites~~ → IN SCOPE (§0.23.8). The idle
  self-improvement sweep reassembles prompts + chat history and asks the Sempai
  for component-creation/upgrades; the existing inline rerouting prompt-rewrite
  (`adjusted_volatile_messages`) is unchanged.

#### 0.23.2 Q1 upgraded from pure-Rust to orchestrated

**Decision (Q1):** Q1 is upgraded. What was pure-Rust
(`ComponentValidator::validate_by_class`) is **split into v3 components** — tools,
skills, tool_skills, python_code, recipes, extensions — and Q1 **runs an orchestrator
instance** that executes the validation Recipe for the component's class.

**Runtime (Q1-runtime / N6 decision):** each Q1 validation is a **full sandboxed
agent-loop orchestrator run** via the existing `sandbox_process` /
`services/process_executor`, with **restricted capabilities**, a **per-validation
token budget** enforced, and **cannot mutate production state**. The validation
orchestrator may exercise the component under test (run a Recipe's steps, execute a
PythonCode body) inside the sandbox, but never persists outcomes to production tables.

**Security invariant — revised (Q1 + N1 decisions):**
- **State 2 remains a security invariant.** Only the Q1 orchestrator
  (`run_q1_validation` in composition, §0.23.9) may write `state = 2`. No API
  endpoint, no application-layer path, no direct SQL may set it. This is the
  retained boundary — it is about **who** writes state 2, not about Q1 being LLM-free.
- **No permanent deterministic floor.** Even injection-scan and schema-conformance
  **eventually become orchestrated test components**. The calculated risk (Q1 begins
  with LLM-based category checks) is accepted and **reduces itself rapidly**: the
  self-improvement loop (§0.23.8) creates deterministic test components that phase
  out LLM calls until LLM is not needed at all. There is no kept-around pure-Rust
  floor that self-improvement cannot reach.

> **⚠️ Revision of §0.15 / §0.18 wording.** §0.15 currently says Q1 is
> "Injection scan, schema conformance, S7 guard, cross-references … Implemented in
> `component_validator.rs`" and §0.18 says Q1 is "automatic, deterministic, no LLM."
> Both are **superseded by §0.23.2**: Q1 becomes orchestrated (LLM-assisted
> initially, deterministically self-improving over time). `component_validator.rs`
> is **retired at Phase N** (§0.23.9), not kept as a floor. The state-2
> "only Gate 1 writes it" invariant is retained verbatim.

#### 0.23.3 The validation system is built from v3 components (trusted-root + evolvable)

**Decisions (Q2 + Q6 + N7):** the validation system is itself a set of v3 components,
bootstrapped as a **trusted root** and **evolvable** via the same Sempai→Q1→Q2 path.

**Component shape:**
- **One pre-trusted Extension per component class** (classes 0, 1–3, 4–9, 12–23 — all
  retrievable classes). Each Extension carries: the **task description**, the
  **format** the component must conform to, the **token budget**, **what needs to be
  tested**, and **security concerns** for that class.
- **Four main Recipes per class** — one per test category
  (**security, performance, token budget, v3-design adherence**) — and **each main
  Recipe calls sub-recipes** (Q6). The sub-recipes perform the actual test steps
  (calling tools / skills / python_code, some hitting the kohai-sempai).
- **One basic Recipe per class** (the bootstrap recipe, Q2): at the start it does
  little more than **create a prompt for the kohai-sempai system** composed from:
  the Extension content (**LLM-formatted**, see §0.23.4), the recipe(s), the
  component under test, and a **query** to test for what the Extension's description
  asks. Over time, self-improvement replaces this LLM prompt with deterministic
  sub-recipes (§0.23.8).
- **One formatter PythonCode per class** (§0.23.4) — computes `formatted_content`.

**Validation matrix scope (N7):** all retrievable classes (0, 1–3, 4–9, 12–23), all 4
categories per class, seeded as **builtin validation components in Phase L (trusted
root)**. (~15 class groups × 1 Extension + 4 main-Recipes + sub-recipes + 1 formatter
PythonCode each.) Classes that do not conceptually need a category (e.g. a Note under
"performance") still receive a trivial recipe for that category, so the matrix is
uniform and self-improvement can fill it in deterministically.

**Evolvable (Q2):** if the Sempai creates a **new** validation-system component (a new
sub-recipe, a sharper security skill, etc.), that component **must also go through
Q1+Q2** like any other component — there is no bypass for validation-system
components. The trusted root is the seed; everything after is gated.

#### 0.23.4 `formatted_content` — persisted LLM-formatted version on every component

**Decision (Q2 + N3):** every component table gains a **`formatted_content TEXT`
column** holding the **LLM-formatted version** of the component, so validation
prompts (and base-prompt assembly) can be composed from pre-formatted content without
re-formatting at runtime.

- **Computed at save time** by the **per-class formatter PythonCode component**
  (§0.23.3), via a **lighter in-process sandboxed PythonCode executor** (N3 decision:
  sandboxed PythonCode run, **no full agent loop**) — cheaper than a Q1 orchestrator
  run. Re-computed on every content change.
- **Migration:** added to **all 13 component tables** in the **same all-tables
  migration that adds `dependency_registry`** — i.e. `V055__reborn_dependency_registry.sql`
  carries both `dependency_registry JSONB` **and** `formatted_content TEXT`
  (additive, `IF NOT EXISTS`). This folds the column into the existing Phase J.2
  step (Q7). For Recipe variants, a per-variant formatted view is derived at assembly
  time from the row `formatted_content` + variant `variable_patterns` (no extra
  column needed in Phase A; revisit if profiling shows reformatting cost).

> **⚠️ Build item (Phase J.2 / L):** the **lightweight in-process sandboxed
> PythonCode executor** must exist for save-time formatting. If no such executor
> exists today (the runtime only has the full sandboxed `process_executor` path),
> Phase J.2 builds it as a restricted single-component PythonCode runner
> (sandboxed, no orchestrator loop, no tool dispatch, no network) — used by the
> formatter and reusable by other single-PythonCode needs. Confirm against live
> source at Phase J.2 implementation.

#### 0.23.5 Upgrade model — validated-live + queue-copy + `proposed_payload` (revises §0.18)

**Decision (N2 + final upgrade-model confirmation):** editing a validated component
does **NOT** remove it from retrieval.

- The **live validated row stays `validation_status='validated'` and keeps serving
  retrieval** while the edit is pending.
- A **COPY** of the edited version is sent to the validation queue. The queue row
  carries the proposed new payload in a **new `proposed_payload JSONB` column**
  (set for upgrades; `NULL` for new-component submissions, where the component row
  itself is the payload at `'pending'`).
- On **Q2 approval (graduation)**: the `proposed_payload` is applied to the live
  component row (overwrite/upgrade), `validation_status` stays `'validated'`, the
  queue row is deleted (graduation trigger fires). For new components, graduation
  just flips the component row to `'validated'` and deletes the queue row.
- On **Q2 rejection**: the queue copy is discarded (queue row → state 3/4); the live
  row is untouched.

**Revision of §0.18 "non-overlapping states" invariant:** the invariant holds for
**new** components (component `'pending'` + queue row, not served). It is **revised
for upgrades**: a validated live row **can** coexist with a queue row carrying an
upgrade copy. `validation_status='upgrade_queued'` is **NOT set on the live component
row** (that would drop it from retrieval, which is exactly the regression the user
rejected); the pending upgrade is tracked **solely** by the queue row's state. The
queue's `UNIQUE (scope, component_id)` constraint still holds — one pending upgrade
per component at a time (concurrent edits are rejected while a copy is queued).

**Migration:** `proposed_payload JSONB` is added to `reborn_validation_queue` in
**V051** (Phase A.5, same migration that creates the table) — additive, nullable.

#### 0.23.6 Sempai auto-creates ALL component types (extends SempaiProposalSink + SempaiReviewOutcome)

**Decision (Q4):** the Sempai proposal path is generalised from recipes-only to **all
component types**: tools (0), tool_skills (13), skills (1–3), extensions (4–9),
python_code (22), extension_catalogues (23), recipes (21), and **intent examples**.

- `SempaiReviewOutcome` (`crates/brassclaw_interceptor/src/packet.rs`) gains
  `proposed_tools`, `proposed_tool_skills`, `proposed_skills`,
  `proposed_extensions`, `proposed_python_code`, `proposed_catalogues` alongside the
  existing `proposed_recipe_updates` / `proposed_intent_examples`. (Or a single typed
  `proposed_components: Vec<ComponentProposal>` with a class tag — chosen at Phase K
  implementation for least churn; the trait signature change is the load-bearing
  part.)
- `SempaiProposalSink::submit_proposals` (`proposal_sink.rs`) is generalised to
  accept all classes; `PgSempaiProposalSink` (composition) inserts into the correct
  class table via the class→table dispatch (same map `fetch_component_by_id` uses),
  each at `validation_status='pending'` + a queue row (state 1). Best-effort, never
  aborts the Kohai call (unchanged).
- **WebUI manual authoring remains** (Q4) as the operator-in-the-loop path: a manual
  create or edit sends the new/edited version into the same validation queue (new
  component → `'pending'` + queue row; edit of validated → copy + `proposed_payload`
  per §0.23.5). The production row is written/overwritten **only after Q2 success**.

> **⚠️ This resolves FIND-NEW-17's前提.** FIND-NEW-17 assumed a "WebUI save handler"
> that creates recipes directly. Under §0.23.6 there is **no direct production write
> on save** — saves enqueue to validation. Intent seeding therefore moves to
> **graduation** (§0.23.5 / Phase N), not raw save. See the FIND-NEW-17 revision
> note in Phase A.

#### 0.23.7 Kohai prompt store — component-UUID references + 6-week retention (extends interceptor)

**Decision (Q3 + N4):** the kohai stores prompts + their composing parts **as
references to already-existing components in the DB**, so prompts + chat history can
be reassembled and sent to the Sempai with a component-creation query during the idle
sweep (§0.23.8).

- **Extend `PgInterceptorStore` / `ForensicPacket` / `CapturedPrompt` / `PromptSegment`**
  (`crates/brassclaw_interceptor/`) so each `PromptSegment` captures the **component
  UUID** it came from (not just the string provenance like `"skill:ibm_bob_people"`).
  Prompts reassemble **by reference** (load the referenced component rows).
- **No new table** (N4). The interceptor packet store is the kohai prompt store.
- **Retention: 6 weeks max.** A retention sweep deletes packets older than 6 weeks
  (this is a packet-store TTL, distinct from the SplitResult memo-cache which stays
  event-driven / no-TTL per §0.18).

> **⚠️ Build item (Phase K):** extend the interceptor packet/segment schema with a
> component-UUID column and add the 6-week retention sweep. The UUID column is an
> additive ALTER **folded into `V056`** (Phase K's single migration), **not a separate
> `V062` file** — see the §0.23.10 ordering note for why (refinery applies migrations in
> strict ascending order and the embedded Postgres data dir is persistent across boots,
> so a `V062` landing in Phase K before `V057`–`V061` of Phases L–P.0 would silently skip
> those later lower-numbered migrations). Confirm the exact packet-store table/column
> shape against the live `PgInterceptorStore` schema at Phase K implementation. The
> retention sweep is a background job in composition (alongside the idle sweep machinery).

#### 0.23.8 Self-improvement sweep — idle ≥ 2h AND after 15:00

**Decisions (Q5 + N5):** the validation-system self-improvement loop runs as an
**in-process background task** with two gating conditions, **both** required:

1. The system has been **idle for ≥ 2 hours** ("idle" = no active agent turns and no
   in-flight LLM calls).
2. The current **server-local time is after 15:00**.

When both first hold, the sweep **runs once per day** (then waits until the next
day's eligible window). The sweep:

1. **Reassembles** recent prompts + chat history from the kohai prompt store
   (§0.23.7) **by component reference** (loads the referenced component rows).
2. **Sends all of it to the Sempai** with a **component-creation query** — asking for
   new recipes / tools / skills / **intents** / extensions / python_code / upgrades
   to existing components (including validation-system components, §0.23.3 evolvable).
3. **Saves the prompts + Sempai answers** (as component references + the resulting
   proposals) so the validation system accumulates evidence for future upgrades.
4. The Sempai's proposals enter **Q1** via `SempaiProposalSink` (§0.23.6) — best
   effort, never bypassing validation.

> **⚠️ Build item (Phase K):** the idle-detection (no active turns / no in-flight LLM
> calls for 2h) + the 15:00-local gate + the once/day cadence + the reassemble-and-
> query-Sempai execution. Implemented as a composition background task using the
> existing turn/LLM-call activity signals. Settings (idle threshold, start hour,
> enabled flag) are configurable in `reborn_monty_vm_settings` (V034) — additive
> columns, fold into Phase K. Default: idle=2h, start=15:00 local, enabled=true.

#### 0.23.9 Bootstrap ordering & ComponentValidator retirement

**Decision (final ordering confirmation):**

- **Phase L seeds the trusted-root validation system** (basic Extensions + basic
  Recipes + 4 category main-Recipes + sub-recipes + formatter PythonCode, per class
  — §0.23.3) **right before Phase N's orchestrated Q1**, alongside the existing
  builtin-tool stack. Phase L's `source='system'` components graduate via the
  automated-but-auditable Q2 (Phase P.0) — **no bypass** (Answer 2 retained).
- **Phase N implements orchestrated Q1** (`run_q1_validation` in a new
  `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`): loads the validation
  Recipe for the component's class, runs the **full sandboxed agent-loop orchestrator**
  (§0.23.2), collects the four category results, and on a clean result writes
  `state = 2` (the retained security invariant). `gate1_pass` / `gate1_fail` stay
  `pub(crate)` on `ValidationQueueStore`; the engine cannot call them (FIND-P9-01
  retained).
- **The pure-Rust `ComponentValidator` is retired at Phase N** (not kept as a floor).
  Between Phase A.5 and Phase N, components sit at `'pending'` with queue rows and Q1
  does not run (the existing documented limitation is unchanged). Phase N is the
  cutover: orchestrated Q1 goes live, `component_validator.rs` is removed.

> **⚠️ Note on the calculated risk.** Between Phase N (orchestrated Q1 live) and the
> self-improvement loop phasing out LLM, Q1 uses LLM-based category checks. This is
> the accepted calculated risk (Q1/N1 decisions). The risk window is bounded by the
> idle sweep's rate of producing deterministic test components (§0.23.8).

#### 0.23.10 Migration & data-model summary

| Migration | Change | Phase |
|-----------|--------|-------|
| `V051__reborn_validation_queue.sql` | **+ `proposed_payload JSONB`** (upgrade copy payload; nullable) alongside the table+indexes already planned. | A.5 |
| `V055__reborn_dependency_registry.sql` | **+ `formatted_content TEXT`** on all 13 component tables, alongside `dependency_registry JSONB`. File now carries two additive columns. | J.2 |
| `V056__reborn_basic_prompt_store.sql` (Phase K **single** migration — **folded**) | Phase K's one migration carries **all** Phase K additive DDL: `CREATE TABLE reborn_basic_prompt_store`; **+ component-UUID reference column(s) on the interceptor packet/segment store** (§0.23.7, enables reference-based prompt reassembly — confirm exact shape vs the live `PgInterceptorStore` schema at Phase K); **+ `reborn_monty_vm_settings` validation-improve cols** (`validation_idle_threshold_minutes INT NOT NULL DEFAULT 120`, `validation_improve_start_hour INT NOT NULL DEFAULT 15`, `validation_improve_enabled BOOLEAN NOT NULL DEFAULT true`, §0.23.8). **Not split into `V062`/`V063`** — see the ordering note below. | K |
| `V061__reborn_validation_queue_q2_actor.sql` (P.0, already noted) | `q2_actor` on the queue for automated-auditable Q2. | P.0 |

> **⚠️ Ordering note — why V062/V063 are folded into V056 (not separate files).**
> Refinery (`refinery::embed_migrations!`, `runner().run_async()` in
> `brassclaw_pg/src/migrations.rs`) applies migrations in **strict ascending version
> order**, and the embedded Postgres data dir is **persistent across boots**
> (`brassclaw_embedded_postgres/src/initdb.rs` — `run_initdb` skips silently if the data
> dir already exists and is non-empty). Phase K runs at sort_order 12, **before** Phases
> L–P.0 (sort_order 13–17) which own `V057`–`V061`. A separate `V062`/`V063` landing in
> Phase K would be applied before `V057`–`V061` even exist; when those lower-numbered
> migrations are later added, refinery would silently skip them (its "apply everything
> after the current max applied version" step sees `V062`/`V063` as the max, never
> reaching `V057`–`V061`) — a silent data-loss hazard on every persistent DB. Folding
> all Phase K additive DDL into the single `V056` keeps migration numbers strictly
> ascending with phase execution order. `V061` (Phase P.0, sort_order 17) stays separate
> — it is numerically after `V060` (Phase O, sort_order 16), so it is in order.

No new component class code is introduced (the validation-system components reuse
existing classes: Extensions 4–9, Recipes 21, Skills 1–3, ToolSkills 13, Tools 0,
PythonCode 22). Validation prompt/answer pairs are stored as **interceptor packets +
component references** (§0.23.7), not a new class.

#### 0.23.11 Plan-step folding map (Q7 — fold into existing steps, no trailing phases)

| New functionality | Folded into existing step | What that step now also does |
|--------------------|---------------------------|------------------------------|
| Queue `proposed_payload` column; upgrade-copy graduation logic | **Phase A.5** (queue table) + **Phase N** (graduation) | A.5: V051 adds `proposed_payload`. N: graduation applies `proposed_payload` to live row on Q2 approval (§0.23.5). |
| `formatted_content` column + per-class formatter PythonCode + light PythonCode executor | **Phase J.2** (dependency_registry all-tables migration) + **Phase L** (seeder) | J.2: V055 adds `formatted_content` + builds the light in-process PythonCode executor. L: seeds the per-class formatter PythonCode components. |
| Sempai auto-creates all types; `SempaiReviewOutcome` + `SempaiProposalSink` generalised; WebUI save → queue (no direct write) | **Phase K** (interceptor) | K: generalise the proposal sink + outcome to all classes; route WebUI saves to the queue (new components `'pending'`; edits → copy + `proposed_payload`). |
| Kohai prompt store: component-UUID refs + 6-week retention | **Phase K** (interceptor) | K: extend `PgInterceptorStore`/`PromptSegment` with component UUIDs (additive ALTER folded into `V056`, not a separate `V062` — see §0.23.10 ordering note); add the 6-week retention sweep. |
| Idle self-improvement sweep (≥2h idle + after 15:00, once/day) | **Phase K** (interceptor) | K: in-process background task + `reborn_monty_vm_settings` config cols; reassemble → Sempai → Q1. |
| Trusted-root validation system (Extensions + Recipes + formatters per class) | **Phase L** (builtin seeder) | L: seeds the validation-system trusted root alongside the builtin-tool stack (all via automated-auditable Q2, no bypass). |
| Orchestrated Q1 (`q1_orchestrator.rs`, sandboxed agent-loop run, state-2 invariant); retire `ComponentValidator` | **Phase N** (validation queue) | N: implements orchestrated Q1, removes `component_validator.rs`, wires graduation for new + upgrade-copy (§0.23.5). |
| Automated-auditable Q2 actor recording | **Phase P.0** | P.0: V061 `q2_actor`; the seeder/automation is the recorded Q2 actor for builtins incl. validation-system trusted root. |

> **Net effect on the phase list:** no new phases are appended. Phases A.5, J.2, K,
> L, N, P.0 each absorb the items above. Phase A is unaffected except for the
> FIND-NEW-17 revision (intent seeding moves to graduation / Phase N; Phase A still
> round-trips `variants` in the store so the data is preserved).

#### 0.23.12 Session summarization — considered and rejected (design decision)

**Background.** A prior design pass (the "Step 13" / Answer 3 idea) proposed an
automatic **session-summarize recipe** that would run on session completion: two
PythonCode leaves (LLM-assisted fact extractor + formatter) + a `memory_write`
ToolSkill, appending a durable summary to the daily memory log so future turns
could `memory_search` for prior context. The identified gap was real: the Kohai
packet store (`crates/brassclaw_interceptor/`) holds prompts + chat history as a
**forensic audit store** (and, after §0.23.7, as self-improvement evidence held by
component-UUID reference), but it is **not agent-queryable** via `memory_search` —
so there was no automatic mechanism writing durable, agent-retrievable session
records.

**Decision (collaborative Q&A): do NOT auto-produce a durable session record.**
Rely solely on the **agent's own `memory_write`** during the session for durable,
agent-queryable context. Rationale:

- An LLM-generated **prose summary** is lossy — it can drop or hallucinate detail.
  Auto-writing a lossy record into the workspace memory system risks corrupting the
  agent's recall with low-confidence content.
- The trusted path is the agent **deliberately** writing durable notes (decisions,
  facts, outcomes) via `memory_write` as it works — the same mechanism that already
  exists. The agent is in the best position to judge what is worth persisting.
- The Kohai packet store stays **forensic + self-improvement evidence**, intentionally
  **separate** from the memory system (Answer 3 invariant preserved). No store merge.

**No plan step is added for session summarization.** This subsection exists to
record the decision so a future pass does not re-propose the "Step 13" recipe as a
forgotten gap.

> **Reversal path (recorded preference, NOT to be implemented unless this decision
> is explicitly revisited).** If session summarization is ever brought back in, the
> agreed shape is: a **structured record** (decisions, files touched, outcomes, open
> questions) — **not** a lossy prose summary — **owned by the Kohai** (reusing its
> already-captured prompt+chat history by component-UUID reference per §0.23.7, so
> there is no double-capture), **writing to the memory system** via `memory_write`
> (the Kohai packet store stays separate), triggered **on session completion**, and
> landing in **Phase K** alongside the idle self-improvement sweep. It would be a
> builtin `source='system'` component graduating via the automated-auditable Q2
> (Phase P.0). This reversal path is documented only; it is out of scope for the
> current plan.

---

## 1. Implementation Phases

### Phase A — StepDescription Schema + IBS Core

**Status:** [ ] Pending

**Goal:** Define the StepDescription types, add `step_descriptions` JSONB to `reborn_recipes`,
implement the IBS as a pure-Rust module. This is Phase A because all later phases depend on it.

#### Files to create

- `crates/brassclaw_pg/migrations/V050__reborn_recipe_step_descriptions.sql`
  ```sql
  -- V050 carries ALL THREE v3 Recipe columns so the Phase A store round-trip
  -- (PgRecipe / NewPgRecipe / RECIPE_SELECT / decode_recipe_row / INSERT / UPDATE,
  -- which reads+writes all three at indices 31/32/33) is never orphaned.
  -- `dependency_registry` is added to the other 12 component tables in V055
  -- (Phase J.2, was V054 before Decision 2); V055's `reborn_recipes` line is
  -- `IF NOT EXISTS` → idempotent no-op here.
  ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS step_descriptions   JSONB;
  ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS variants            JSONB;
  ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
  ```

  > **⚠️ VARPAT-COL-GAP / DEPREG-TIMING-GAP — V050 must create all three columns
  > (not just `step_descriptions`).** A prior draft had V050 create only
  > `step_descriptions`. But Phase A's "Files to modify" adds `variants` +
  > `dependency_registry` to `PgRecipe`/`NewPgRecipe`/`RECIPE_SELECT`
  > (indices 31/32/33) and the FIND-07 INSERT/UPDATE writes all three. Two gaps
  > would otherwise result:
  > 1. **VARPAT-COL-GAP:** `variants` had NO migration anywhere in V050–V058, yet
  >    Phase A persists it and Phase M.5 depends on `variable_patterns` nested in
  >    `variants`. A SELECT on a non-existent column is a hard SQL error at runtime.
  > 2. **DEPREG-TIMING-GAP:** `dependency_registry` on `reborn_recipes` is not
  >    created until V055 (Phase J.2, was V054 before Decision 2), but Phase A's
  >    store round-trips it from V050. Between V050 and V055 the SELECT would read
  >    a column that does not exist.
  >
  > Fix: V050 creates all three columns on `reborn_recipes` atomically. `variants`
  > is recipe-specific (no other table has it). `dependency_registry` is also
  > created on the other 12 component tables in V055 (Phase J.2); V055's
  > `ALTER TABLE reborn_recipes ADD COLUMN IF NOT EXISTS dependency_registry JSONB`
  > is then an idempotent no-op (the column already exists from V050). This is the
  > lowest-churn, additive fix and preserves the plan's invariant that the store
  > round-trips every column from the phase that introduces the struct field.

- `crates/brassclaw_engine/src/types/ibs.rs` (**NEW — Decision 1 / FIND-P6-03 / FIND-NEW-01**)
  Home for the IBS **data-model** types that are persisted in JSONB / referenced by
  `RecipeVariant`. These are NOT builder types — they are the authoring model.
  Adding them to `types/ibs.rs` (a sibling of `recipe.rs`) ensures zero circular
  dependency: `types/ibs.rs` imports only `serde` + `uuid`; `memory/instruction_builder.rs`
  imports from `crate::types::ibs`; `types/recipe.rs` imports from `crate::types::ibs`.
  The direction is cleanly `memory → types` throughout.

  Types to define here:

  > **⚠️ FIND-AUDIT-10 + FIND-AUDIT-11 — these types MUST match the canonical definitions
  > in §0.4.1 exactly. The earlier draft of this section had TWO WRONG definitions:**
  > 1. `ToolBinding` was missing `tool_name: String` and `params: serde_json::Value` — both
  >    are required for runtime `__execute_action__` dispatch and `{{vars.name}}` substitution.
  > 2. `ErrorPolicy` used `{ Propagate, Retry { max_attempts: u8 }, Fallback { message: String } }`
  >    which is inconsistent with the §0.4.1 canonical definition
  >    `{ Fail, Ignore, Retry { max_attempts: u32 }, Fallback { step_id: String } }`.
  > **Use the definitions below (corrected to match §0.4.1).**

  ```rust
  use serde::{Deserialize, Serialize};

  /// Slot-variable refinement rule stored on a RecipeVariant.
  /// Persisted in the `variants` JSONB column of `reborn_recipes`.
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct VariablePattern {
      /// Slot name — e.g. "dir", "filename". Matches {{vars.NAME}} expressions.
      pub name: String,
      /// Optional regex applied after positional extraction to validate/transform the value.
      pub pattern: Option<String>,
      /// Human description of the slot's expected value (WebUI help text only).
      pub description: Option<String>,
  }

  /// Error-handling policy for a single ToolBinding.
  /// Persisted nested inside ToolBinding in rust-channel IbsRecipeStep tool_bindings.
  ///
  /// ⚠️ FIND-AUDIT-11: canonical definition — matches §0.4.1 exactly.
  /// Do NOT use Propagate/Retry{u8}/Fallback{message} — that was an earlier wrong draft.
  /// Default: Fail — via `#[derive(Default)]` + `#[default]` on `Fail` (FIND-IBS-06;
  /// clippy-clean under `derivable_impls`, semantically identical to a hand-written impl).
  #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
  #[serde(tag = "policy", rename_all = "snake_case")]
  pub enum ErrorPolicy {
      /// Fail the turn immediately — hard error, no retry.
      #[default]
      Fail,
      /// Ignore the error and continue — orchestrator receives an empty result.
      Ignore,
      /// Retry up to max_attempts times before falling through to Fail.
      Retry { max_attempts: u32 },
      /// On error, jump to the step with id step_id within the same BuildInstruction.
      Fallback { step_id: String },
  }

  /// Binding from a Rust-channel IBS step to a specific tool invocation.
  /// Persisted nested inside rust-channel IbsRecipeStep `tool_bindings`.
  ///
  /// ⚠️ FIND-AUDIT-10: canonical definition — matches §0.4.1 exactly.
  /// Do NOT use { tool_id, error_policy } only — tool_name and params are required.
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct ToolBinding {
      /// UUID of the Tool (class 0) row — used by the Rust layer for capability dispatch.
      pub tool_id: uuid::Uuid,
      /// Denormalized tool name (e.g. "read_file"). Needed for __execute_action__ calls.
      /// Must match the registered capability name in FirstPartyCapabilityRegistry.
      pub tool_name: String,
      /// Parameter values for this tool call. {{vars.name}} substitution applied before use.
      pub params: serde_json::Value,
      /// How to handle a tool invocation error.
      pub error_policy: ErrorPolicy,
  }
  ```

  > **⚠️ Decision 1 propagation:** any place in the existing plan that says
  > "`VariablePattern` lives in `instruction_builder.rs`" or imports
  > `crate::memory::instruction_builder::VariablePattern` must be updated to
  > `crate::types::ibs::VariablePattern`. The `instruction_builder.rs` "Files to create"
  > section below has been updated accordingly.

- `crates/brassclaw_engine/src/memory/instruction_builder.rs`
  New types (this module is the **sole home** for all IBS **builder / output** types — see
  FIND-NEW-01 updated. `VariablePattern`/`ToolBinding`/`ErrorPolicy` moved to `types/ibs.rs`
  per Decision 1 / FIND-P6-03; do NOT redefine them here):
  `StepDescriptionEntry`, `StepRange`, `StepOwner`, `RecipeStepType`,
  `IbsRecipeStep` (renamed from `RecipeStep` to avoid collision with the existing
  v2 `RecipeStep { skill, tool, params, description }` in `types/recipe.rs`),
  `BuildInstruction`, `IbsError`,
  `DependencyExpr`, `DependencyNode` (the parsed traversal tree — see §0.19),
  `StepContextSpec` (derived content type for orchestrator formatting — see §0.5).

  Import at top of `instruction_builder.rs`:
  ```rust
  use crate::types::ibs::{VariablePattern, ToolBinding, ErrorPolicy};
  ```

  **`StepDescriptionEntry` shape** (maps to one element of the `step_descriptions` JSONB array):
  ```rust
  pub struct StepDescriptionEntry {
      pub desc_idx:    usize,
      pub label:       String,
      pub yaml_source: String,              // preserved verbatim; never read by IBS
      pub steps:       Vec<StepEntry>,      // pre-parsed; IBS reads this only
  }

  pub struct StepEntry {
      pub stepnumber:   u32,
      pub knowledge:    StepOwner,          // Orchestrator | Rust | Both
      pub goal:         String,
      pub content:      String,
      pub step_type:    RecipeStepType,     // Text | Component | Snippet
      pub info:         Option<String>,     // WebUI annotation only; not emitted at runtime
      pub include:      Vec<uuid::Uuid>,    // component UUIDs
      pub tool_bindings: Vec<ToolBinding>,  // rust-channel tool calls (§0.4.1); empty for orchestrator-only steps
      pub dependencies: Option<String>,     // traversal expression string (§0.19)
  }
  ```

  > **⚠️ FIND-IBS-05 — `StepEntry` MUST carry `tool_bindings`.** §0.4.1 states
  > ToolBinding is "persisted in the `step_descriptions` JSONB column (inside
  > rust-channel IbsRecipeStep tool_bindings)", and the §0.7 S7 guard keys off
  > rust `tool_bindings`. The earlier StepEntry shape omitted the field — that
  > was a gap. Resolution (collaborative Q&A): add
  > `#[serde(default)] pub tool_bindings: Vec<ToolBinding>` to `StepEntry`.
  > Authors write concrete tool calls (`tool_id`/`tool_name`/`params`/`error_policy`)
  > per rust-channel step; the IBS passes them through to the compiled
  > `IbsRecipeStep.tool_bindings`. Empty for orchestrator-only steps. The
  > `ToolBinding`/`ErrorPolicy` types are imported from `crate::types::ibs`.

  New functions: `parse_step_link(&str) -> Result<Vec<StepRange>, IbsError>`,
  `parse_dependency_expr(&str) -> Result<DependencyExpr, IbsError>`, and
  `build_instruction(step_link, step_descriptions, variable_patterns, llm_call_required) -> Result<BuildInstruction, IbsError>` (FIND-IBS-02 adds the `llm_call_required` param).

  **`DependencyExpr` / `DependencyNode` types:**
  ```rust
  pub enum DependencySubExpr {
      All,                          // [all] — full transitive closure
      Selective(Vec<DependencyNode>), // [n, m[...], ...] — selective indices
  }

  pub struct DependencyNode {
      pub idx: usize,
      pub sub: Option<DependencySubExpr>, // None = load component only, no sub-deps
  }

  pub type DependencyExpr = Vec<DependencyNode>;
  ```

#### Files to modify

> **⚠️ FIND-P7-11 / FIND-05 — FIRST SUB-TASK of Phase A: fix `PgRecipe::is_tier0_eligible()`**
> This is a single-line bug fix with NO dependencies on any other Phase A work. Do it first,
> before adding columns or types, so it is never deferred.
>
> **File:** `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` at **line ~140**
>
> **Current code (WRONG — confirmed by live read, Pass 7):**
> ```rust
> pub(crate) fn is_tier0_eligible(&self) -> bool {
>     self.is_deliverable() && matches!(self.tier.as_str(), "mature" | "candidate")
> }
> ```
> **Fix:**
> ```rust
> pub(crate) fn is_tier0_eligible(&self) -> bool {
>     self.is_deliverable()
>         && matches!(self.tier.as_str(), "mature" | "candidate")
>         && self.wilson_lower >= 0.70
> }
> ```
> **Why:** Without the `wilson_lower >= 0.70` guard, any Recipe marked `"mature"` or
> `"candidate"` with ANY wilson score (including 0.0 — never used) would be silently
> eligible for Tier 0. The `PgRecipe` struct carries `wilson_lower: f64` at line ~114 — the
> check is one line. This is a dangerous silent escalation bug. Fix now; it is the exact
> same guard that `Recipe::is_tier0_eligible()` in `types/recipe.rs` correctly applies.
> After fixing, `PgRecipeLibrary::find_recipe` (line ~790) will set `tier0_eligible` correctly
> on `RecipeMatchDto`. See FINDING G in Phase H for why the v3 `RecipeStage` dispatch should
> use `TurnRoutingSignals` (from `fetch_for_turn`) rather than `RecipeMatchDto.tier0_eligible`,
> but this fix prevents the wrong value from propagating into ANY code that reads that field.

- `crates/brassclaw_engine/src/types/recipe.rs` — add the `RecipeVariant` authoring type
  and three new `Recipe` struct fields. **Do NOT add `BuildInstruction` /
  `RecipeStepType` / `StepOwner` / `ToolBinding` / `ErrorPolicy` here** — those IBS-domain
  types live solely in `instruction_builder.rs` (Files to create; FIND-NEW-01). This file
  only adds the Recipe *model* (what is authored + persisted), not the IBS *build* types.
  Add to `Recipe` struct (all `#[serde(default)]` so existing rows deserialise unchanged):
  ```rust
  #[serde(default)] pub variants: Vec<RecipeVariant>,
  #[serde(default)] pub step_descriptions: serde_json::Value,
  #[serde(default)] pub dependency_registry: serde_json::Value,  // per-component, see §0.19
  ```
  **`RecipeVariant` shape** (one entry per distinct intent — §0.3; stored in the
  `variants` JSONB column added by V050):
  > **⚠️ FIND-P5-03 — canonical definition is in the "Files to modify" section below.**
  > See the "RecipeVariant (canonical, persisted in `variants` JSONB)" block below the
  > naming-collision note. The definition here is the same but uses the authoritative field
  > names. The `name` field shown here equals `variant_key` in the canonical definition.
  ```rust
  use crate::types::ibs::VariablePattern;  // Decision 1: home is types/ibs.rs (NOT instruction_builder.rs)

  // See canonical definition with variant_key / step_link / intent_examples / variable_patterns
  // in the "Files to modify" block below (FIND-P5-03 canonical definition).
  // DO NOT implement two different shapes — use the canonical definition only.
  ```
  Note: `dependency_registry` is also added to `ToolSkill`, `Skill`, `PythonCode`,
  and all other component types that participate in dependency traversal. Each component
  owns its own flat indexed registry.

- `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs` — extend the **store
  round-trip** so the new `step_descriptions` / `variants` / `dependency_registry` columns
  are persisted AND loaded (otherwise the Phase A column is orphaned / write-only / never
  loaded).

  **Verified struct locations (read from source):**
  - `PgRecipe` struct starts at **line ~91** (verified `pg_recipe_store.rs:91`,
    `pub(crate) struct PgRecipe`; first field `id`); fields include
    `validation_errors: Vec<String>` (line ~116), `review_feedback: Option<String>`,
    `review_attempts: i16`, `rejected_at`, `queue_code`. Add `step_descriptions: serde_json::Value`,
    `variants: serde_json::Value`, `dependency_registry: serde_json::Value` fields.
  - `NewPgRecipe` struct starts at **line ~145** (the plan's "line 147" is approximately
    correct). Add the three fields so INSERT/UPDATE persist them.
  - `RECIPE_SELECT` constant at **line ~208** (verified): currently selects 31 columns ending
    at `updated_at` (index 30 in `decode_recipe_row`). Add the three new columns at the end of
    the SELECT list (indices 31, 32, 33) to avoid re-indexing all 31 existing positions.
  - `decode_recipe_row` at **line ~219** (verified): reads columns by positional index
    (row.get(0) through row.get(30)). Adding the three columns at the END of `RECIPE_SELECT`
    means appending `row.get(31)`, `row.get(32)`, `row.get(33)` — no re-indexing of existing
    positions needed if placed at the end. **Do NOT insert the new columns in the middle of
    `RECIPE_SELECT`** — that would require re-numbering all subsequent `row.get(N)` calls.
  - `RecipeValidationStatusUpdate` struct at **line ~170**: has `validation_errors`,
    `review_feedback`, `queue_code` — do NOT modify in Phase A (Phase N drops these).
  - The WebUI save path (create/update recipe) must pass `step_descriptions` through.
 > **⚠️ FIND-NEW-17 — REVISED by §0.23.6 (intent seeding moves to graduation).**
 > Under §0.23.6 there is **no direct production write on save** — a WebUI save
 > (or Sempai proposal) enqueues the component to validation; the production row is
 > written/overwritten **only after Q2 success**. Therefore the intent-seeding call
 > below is correct in *logic* but its **trigger point moves from "on save" to "on
 > Q2 graduation"** (Phase N, §0.23.5). Phase A does **not** seed intents; it only
 > round-trips `variants` in the store so the data is preserved until graduation
 > seeds it. The original block below is retained as the pre-revision reference:
 > **⚠️ FIND-NEW-17 — WebUI save path MUST also seed intent rows from `variants`:**
 > When a Recipe is saved with `variants` populated, the save handler (the composition
 > endpoint that calls `PgRecipeStore::insert` or `update`) MUST also iterate each
 > `RecipeVariant.intent_examples` and call `seed_intent_input(pool, scope, example, class,
 > recipe_id, 21, IntentSource::Seeded, variant.step_link.as_deref())` for each expression.
 > Without this, intent rows are never written and `resolve_intent` will not match the recipe.
 > This seeding step belongs in the **composition** WebUI-save handler (where the pool and
 > scope are available), NOT in `PgRecipeStore` itself (which is a pure store layer).
 > The same logic applies on update: re-seed all variants on every save (the ON CONFLICT
 > DO UPDATE clause in `seed_intent_input` makes it idempotent).
 This makes the column writable + loadable from Phase A. Whether `RecipeMatchDto`
  (line 799) exposes `step_descriptions` to the runtime `RecipeStage` is a **Phase H**
  decision (extend the DTO + `find_recipe` path, or route `RecipeStage` through
  `PostgresSource::fetch_for_turn`, which reads the column directly) — see the Phase H
  review note. Either way, the store must round-trip the column from Phase A so it is
  never orphaned.

  > **⚠️ FIND-07 — Phase A must also update the INSERT and UPDATE SQL statements:**
  > Adding `step_descriptions`/`variants`/`dependency_registry` to `PgRecipe` + `RECIPE_SELECT`
  > + `decode_recipe_row` is only half the work. The INSERT and UPDATE SQL must also be updated.
  > Before implementing, locate the exact SQL statements:
  > - **INSERT:** verified at `pg_recipe_store.rs:261–283` — INSERT statement currently uses
  >   13 columns (`tenant_id, user_id, agent_id, project_id, name, description, trigger, steps,
  >   prior_knowledge_content, override_prompt_creation, consumer_tags, intent_examples, source`)
  >   with parameters `$1`–`$13`. After adding the three new columns the INSERT becomes 16 columns
  >   with parameters `$1`–`$16`. The RETURNING clause (`RETURNING id`) stays unchanged. The
  >   parameter binding array must also add three entries for the new fields.
  > - **UPDATE:** there is an `update_validation_status` method at `pg_recipe_store.rs:384–420`
  >   that updates only `validation_status`, `validation_errors`, `review_feedback`, `queue_code`.
  >   This does NOT need `step_descriptions`. Look for a general recipe update/upsert statement
  >   (if one exists for the WebUI save path) and add `step_descriptions = $N`, `variants = $N+1`,
  >   `dependency_registry = $N+2` to the SET clause.
  > - **`NewPgRecipe`:** the three new fields (`step_descriptions`, `variants`, `dependency_registry`)
  >   must be added to this struct (all `Option<serde_json::Value>` with `None` as default for
  >   backwards compatibility when creating recipes without step_descriptions).
  > - **⚠️ FIND-21 — concrete INSERT parameter count:** The current INSERT has 13 columns
  >   (`$1`–`$13`). After adding `step_descriptions`, `variants`, `dependency_registry`, the
  >   parameter count MUST be exactly `$1`–`$16`. Off-by-one here causes a runtime panic
  >   (tokio-postgres will reject a mismatch between the column count and the params array length).
  >   Count before committing.
  > Failure to update the INSERT/UPDATE means the Phase A column is SELECT-able but never written —
  > it will always be NULL until the SQL is fixed, silently defeating the round-trip goal.

  > **⚠️ FIND-IBS-07 — `PgRecipe`/`NewPgRecipe` v3 fields are `Option<Value>`, NOT non-Option.**
  > The "Files to modify" text above says "Add `step_descriptions: serde_json::Value`,
  > `variants: serde_json::Value`, `dependency_registry: serde_json::Value` fields" (non-Option).
  > That is imprecise: V050 creates the columns with `ADD COLUMN IF NOT EXISTS ... JSONB` —
  > **NULLable, no default**. Existing (pre-V050) rows backfill to NULL. tokio-postgres rejects
  > decoding a NULL column into a non-Option `serde_json::Value` at runtime
  > ("a column was NULL but the Rust type is not Option"), which would break every
  > `get`/`list_all`/`fetch_validated` call on a legacy row. Resolution (implemented): the three
  > fields are `Option<serde_json::Value>` on **both** `PgRecipe` (read) and `NewPgRecipe` (write,
  > `None` = leave column NULL). This matches the existing pattern for the other NULLable JSONB
  > columns `trigger` and `intent_examples` (both `Option<Value>`); the non-Option `steps: Value`
  > is NOT NULL by schema. The engine `Recipe` struct fields stay non-Option `serde_json::Value`
  > with `#[serde(default)]` (NULL → `Value::Null` is a valid `Value`, no panic) — the store layer
  > maps `None` ↔ `Value::Null` at the conversion boundary (Phase H owns that conversion). This is
  > a forced correctness constraint from the NULLable migration, not a design preference.

  > **Naming note:** The existing `types/recipe.rs` already defines `RecipeStep { skill, tool,
  > params, description }` — a name-based v2 type. The IBS module introduces a **different**
  > `RecipeStep` type in `instruction_builder.rs` that uses UUIDs and channels. To avoid a
  > naming collision: the IBS type is named `IbsRecipeStep` in `instruction_builder.rs`; the
  > existing v2 `RecipeStep` in `types/recipe.rs` is NOT renamed (backward compatibility).

  > **⚠️ FIND-P5-03 — `RecipeVariant` has two inconsistent definitions in this plan.**
  > The earlier block (§0.3 "Intent Variants") shows a `name` field; the block immediately
  > above shows both a `name` field AND a `step_link: Option<String>`. The canonical
  > persisted struct must be consistent. Resolved: use the definition below. The `name`
  > field from the §0.3 example is the same as `variant_key`. There is NO separate `label`
  > field — `variant_key` is the only human-readable identifier. `step_link` is nullable
  > (`Option<String>`) because legacy Recipe rows have no step_link until Phase D re-seeds
  > their intent examples.

  `RecipeVariant` (canonical, persisted in `variants` JSONB):
  ```rust
  pub struct RecipeVariant {
      /// Human-readable variant identifier (e.g. "ls-la"). Used by WebUI only.
      pub variant_key: String,
      /// Direct IBS input — the step_link formula for this variant.
      /// None for legacy variants not yet migrated to v3 intent inputs.
      pub step_link: Option<String>,
      /// Intent expressions for this variant — seeded into reborn_intent_inputs on save.
      pub intent_examples: Vec<String>,
      /// Optional post-extraction refinement for slot values (§0.17.3).
      /// Empty = positional auto-extraction only.
      pub variable_patterns: Vec<VariablePattern>,
  }
  ```

  > **✅ FIND-P6-03 — RESOLVED (Decision 1): `types/ibs.rs` is the correct home.**
  > `RecipeVariant` in `types/recipe.rs` must `use crate::types::ibs::VariablePattern`.
  > The direction is cleanly `types/recipe.rs → types/ibs` (same module, no cross-module dep).
  > `instruction_builder.rs` also imports `crate::types::ibs::{VariablePattern, ToolBinding, ErrorPolicy}`.
  > There is NO dependency from `types/ibs.rs` back to `memory/` or `types/recipe.rs`.
  > The old option "if a cycle is found, move to types/ibs.rs" is now the primary path — the
  > cycle is pre-empted entirely by never putting `VariablePattern` in `instruction_builder.rs`.
  >
  > **⚠️ FIND-P6-10 — `#[serde(default)]` on all three new fields is required for backward compat.**
  > `Recipe::from_metadata` uses `serde_json::from_value`. Existing DB rows stored as
  > `MemoryDoc.metadata` (legacy `StoreBackedRecipeStore` path) do not have `variants`,
  > `step_descriptions`, or `dependency_registry`. Without `#[serde(default)]`, deserializing
  > these old rows will fail with "missing field". All three new fields MUST be:
  > `#[serde(default)] pub variants: Vec<RecipeVariant>` etc. This is already specified in the
  > Phase A instructions — this note reinforces it as a correctness requirement, not a style choice.

- `crates/brassclaw_engine/src/memory/mod.rs` — add `pub mod instruction_builder`

- `crates/brassclaw_engine/src/types/mod.rs` — add `pub mod ibs` (**Decision 1**)
  This exposes `crate::types::ibs::{VariablePattern, ToolBinding, ErrorPolicy}` to all
  modules in `brassclaw_engine`. No other changes to `types/mod.rs`.

> **✅ Review note (pre-v3 audit) — the engine `Recipe` struct is NOT the type the runtime
> `RecipeStage` consumes; the store round-trip is missing from Phase A's file list — RESOLVED:**
> `pg_recipe_store.rs` (PgRecipe struct + RECIPE_SELECT + decode_recipe_row + NewPgRecipe, with
> the positional re-indexing caveat) has been added to Phase A's "Files to modify" above so the
> column is writable + loadable from Phase A and never orphaned. The `RecipeMatchDto` exposure
> decision is deferred to Phase H as the note recommends. Original audit detail retained below:
> The runtime stage consumes `RecipeMatchDto` from `RecipeLookup::find_recipe`
> (`crates/brassclaw_reborn_composition/src/pg_recipe_store.rs:764`), which is built from the
> `PgRecipe` store row and today carries only `id, name, tier, wilson_lower, tier0_eligible,
> validation_kind, steps, match_score` (line 799) — `steps` here is the **v2** `serde_json::Value`
> from the `reborn_recipes.steps` column via `steps_to_dtos`. It does **not** carry
> `step_descriptions`, `variants`, `step_link`, or `variable_patterns`. Separately, `PgRecipe`
> (`pg_recipe_store.rs:117`), `RECIPE_SELECT` (line 208), `decode_recipe_row` (line 219, indexed),
> and `NewPgRecipe` (line 147) also have no `step_descriptions` field. So Phase A's addition of
> `step_descriptions`/`variants`/`dependency_registry` to the **engine** `Recipe` struct
> (`types/recipe.rs`) is greenfield, but unless `pg_recipe_store.rs` (SELECT + decode + insert) is
> also extended in the same phase, the column is orphaned (write-only / never loaded). Phase A's
> "Files to modify" does **not** list `pg_recipe_store.rs`. **Resolution required in Phase H:** the
> v3 Tier-0/Tier-1 dispatch reads `step_descriptions` straight from the recipe row via
> `PostgresSource::fetch_for_turn` (intent/IBS path, §0.3), which is a **different** path from
> `RecipeLookup::find_recipe`. Phase H must specify which path feeds `RecipeStage` (extend
> `RecipeMatchDto` + store round-trip, or route through `PostgresSource`), and must ensure the two
> paths do not both fire for the same turn. See the Phase H review note.

#### Tests

- Unit: JSONB round-trip: `StepDescriptionEntry` with `yaml_source` + `steps` serialises and deserialises correctly
- Unit: `yaml_source` field is preserved verbatim (not re-serialised from `steps`)
- Unit: `parse_step_link("0:0-0:E")` → single range, all steps
- Unit: `parse_step_link("0:0-0:30+1:0-1:E")` → two ranges, correct desc_idx and bounds
- Unit: `build_instruction` with `knowledge: rust` step → step only in `rust_steps`
- Unit: `build_instruction` with `knowledge: both` step → step in both channels
- Unit: `build_instruction` with `snippet`-type step → `IbsError::UnpromotedSnippet`
- Unit: step numbers non-monotonic within a StepDescription → `IbsError::StepOrderViolation`
- Unit: S7 guard: rust tool_bindings present, no orchestrator skill_ids → `IbsError::S7Violation`
- Unit: `parse_dependency_expr("1[all], 5[2,6], 17[3, 7[1,4]]")` → correct `DependencyExpr` tree
- Unit: `parse_dependency_expr("0")` → single node, no sub-expr
- Unit: `parse_dependency_expr("1[all]")` → node with `DependencySubExpr::All`
- Unit: `parse_dependency_expr("")` → empty vec (no dependencies)
- Unit: malformed expression `"1[all"` → `IbsError::InvalidDependencyExpr`
- Unit: `BuildInstruction`, `ToolBinding`, `ErrorPolicy`, `DependencyNode` serde roundtrips

---

### Phase A.5 — Validation Queue Table (Decision 2)

**Status:** [ ] Pending

> **§0.23.5 fold-in:** V051 also adds `proposed_payload JSONB` (nullable) to
> `reborn_validation_queue` — the upgrade-copy payload (§0.23.5). The
> `ValidationQueueStore` application layer carries `proposed_payload` through
> submit (set for upgrades, null for new-component submissions) and exposes it to
> graduation. The graduation *apply* logic itself lands in Phase N (§0.23.9).

**Goal:** Create `reborn_validation_queue` table (DDL + indexes only) and the
`ValidationQueueStore` application layer **before** Phase B creates classes 22/23.
This makes the queue available from the very first WebUI-authored save of any component class,
not just after Phase N.

> **Why here?** Decision 2 splits the original Phase N migration (V058 monolith) into two
> parts: V051 (table creation — this phase) and V059 (populate-from-existing + trigger +
> DROP legacy columns — Phase N). The only reason Phase A.5 exists as a separate phase
> from Phase A is that Phase N's column-drop work is not yet ready — the table must exist
> early but the data migration and DROP work stays in Phase N where it belongs.
>
> **What this phase does NOT do:** it does NOT populate queue rows from existing component
> tables (that is V059 / Phase N), it does NOT add `last_graduation_at` to the scope cursor
> (also Phase N), it does NOT drop legacy columns (also Phase N). Phase A.5 is table DDL + Rust store only.

#### Files to create

- `crates/brassclaw_pg/migrations/V051__reborn_validation_queue.sql`

  Full DDL (from §0.18 — copy the `CREATE TABLE reborn_validation_queue` block exactly):
  ```sql
  -- ⚠️ Decision 2: this file creates the table+indexes ONLY.
  -- Data migration (populate from component tables), graduation trigger, and
  -- legacy column DROPs are in V059__reborn_validation_queue_populate.sql (Phase N).
  CREATE TABLE reborn_validation_queue (
      id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      tenant_id        TEXT NOT NULL,
      user_id          TEXT NOT NULL,
      agent_id         TEXT NOT NULL,
      project_id       TEXT NOT NULL,
      component_id     UUID NOT NULL,
      component_class  SMALLINT NOT NULL,
      state            SMALLINT NOT NULL DEFAULT 1,
      -- state values: 1=Q1_pending, 2=Q1_passed (awaiting Q2 manual review), 3=rejected, 4=deletion_candidate
      -- ⚠️ FIND-P9-08: state 2 = Gate 1 PASSED. Only Gate 1 sets state 2 (pub(crate) gate1_pass).
      -- The Q2 reviewer approves from state 2 → approve() deletes the row (graduation).
      counter          INT NOT NULL DEFAULT 0,
      review_feedback  TEXT,
      validation_errors TEXT[] NOT NULL DEFAULT '{}',
      submitted_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
      -- ⚠️ FIND-P9-07: scope-first ordering matches all query patterns (queries always
      -- filter scope first, then component_id). component_id-first is wrong index order.
      UNIQUE (tenant_id, user_id, agent_id, project_id, component_id)
  );
  CREATE INDEX reborn_validation_queue_scope_state_idx
      ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, state);
  CREATE INDEX reborn_validation_queue_scope_class_idx
      ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id, component_class);
  -- Deletion candidate index — used by purge_deletion_candidates
  CREATE INDEX reborn_validation_queue_deletion_idx
      ON reborn_validation_queue (tenant_id, user_id, agent_id, project_id)
      WHERE state = 4;
  ```

  > **Cross-check with §0.18:** The full `CREATE TABLE` DDL in §0.18 of the plan is the
  > authoritative reference. Copy it exactly into this file. If there is any discrepancy
  > between the DDL here and §0.18, the §0.18 version takes precedence.

- `crates/brassclaw_reborn_composition/src/validation_queue.rs` — `ValidationQueueStore`
  **This store moves from Phase N.2 to Phase A.5 (Decision 2).** See Phase N.2 for the
  full `ValidationQueueStore` impl spec (`submit`, `gate1_pass`, `gate1_fail`, `reject`,
  `approve`, `list`, `purge_deletion_candidates`). Implement the full store here in Phase A.5;
  Phase N.2 becomes a no-op (already done) — mark it so when Phase N is reached.

  > **⚠️ FIND-P9-05 — `approve` must be ONE TRANSACTION across two tables:**
  > `approve` touches both `reborn_validation_queue` (DELETE) and the target component
  > table (UPDATE `validation_status = 'validated'`). These must be atomic. If the DELETE
  > succeeds but the UPDATE fails, the queue row is gone and the component stays `pending` —
  > a permanent orphan.
  >
  > **Required implementation:**
  > ```
  > BEGIN;
  > (1) Dispatch on component_class → resolve the target table using the same class→table
  >     map as fetch_component_by_id. Unknown class → return error BEFORE starting the
  >     transaction (no wasted BEGIN).
  > (2) UPDATE {component_table} SET validation_status = 'validated'
  >     WHERE id = $component_id AND tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND
  >     project_id=$4;
  >     If 0 rows updated → ROLLBACK + return error (component disappeared).
  > (3) DELETE FROM reborn_validation_queue
  >     WHERE component_id = $component_id AND tenant_id=$1 AND user_id=$2 AND agent_id=$3
  >     AND project_id=$4;
  >     The graduation trigger fires here, updating last_graduation_at.
  > COMMIT;
  > ```
  > **Ordering note:** UPDATE before DELETE so the trigger fires only after the component
  > row is already validated — no window where the queue row is deleted but the component
  > is still `pending`. Return `Ok(component_id)` for the caller.
  >
  > **⚠️ FIND-P9-01 — `approve` is `pub` but `gate1_pass` is `pub(crate)`. The Q1
  > orchestration (validate → gate1_pass/gate1_fail) MUST live in
  > `brassclaw_reborn_composition`, NOT in `brassclaw_engine`.** `ComponentValidator`
  > (`brassclaw_engine`) is a pure function importable cross-crate; it does not call
  > `gate1_pass`. The composition crate calls it and then calls `gate1_pass`. This
  > cross-crate wiring lives in `q1_orchestrator.rs` (see "Files to create" below).

- `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs` (**NEW — FIND-P9-01**)

  > **⚠️ FIND-P9-01 — `gate1_pass` is `pub(crate)` in `brassclaw_reborn_composition`.
  > `ComponentValidator` is in `brassclaw_engine` (a DIFFERENT crate).
  > `brassclaw_engine` CANNOT call `pub(crate)` methods from `brassclaw_reborn_composition`.
  > Therefore the Q1 orchestration sequence MUST live entirely inside
  > `brassclaw_reborn_composition`.**

  Function signature:
  ```rust
  pub async fn run_q1_validation(
      pool: &PgPool,
      scope: &ComponentScope,
      component_id: Uuid,
      class_code: i32,
      payload: ComponentPayload<'_>,       // from brassclaw_engine — importable cross-crate
      config: &ValidatorConfig,
      queue_store: &ValidationQueueStore,
  ) -> Result<Q1Outcome, Q1Error>
  ```

  Flow:
  1. Call `ComponentValidator::validate_by_class(class_code, payload, config)` — this is a
     pure function from `brassclaw_engine`; `brassclaw_reborn_composition` CAN call it
     (it depends on `brassclaw_engine`).
  2. If validation succeeds → call `queue_store.gate1_pass(scope, component_id, &[])`.
  3. If validation fails with errors → call `queue_store.gate1_fail(scope, component_id, &errors)`.
  4. Return `Q1Outcome::Passed` or `Q1Outcome::Failed { errors }`.

  `ComponentValidator::validate_by_class` must be `pub` (not `pub(crate)`) so
  `brassclaw_reborn_composition` can import it. Verify its visibility when implementing;
  adjust if needed.

  Wire `run_q1_validation` into the WebUI-save path for classes 22 and 23 (Phase B/C),
  and into the boot-integrity pass for all classes (Phase N).

#### Files to modify

- `crates/brassclaw_reborn_composition/src/lib.rs` (or the composition wiring file) — expose
  `ValidationQueueStore` to the composition layer so Phase B / Phase C can call
  `validation_queue_store.submit(scope, component_id, class_code)` on WebUI-save.

#### Tests

- Unit: `submit` inserts a queue row with `state = 1`
- Unit: `gate1_pass` transitions `state 1 → 2`; `gate1_fail` keeps `state = 1` and populates errors
- Unit: `reject` transitions `state 2 → 3` and increments `counter`
- Unit: `reject` when `counter >= threshold` → `state = 4`
- Unit: `approve` executes UPDATE + DELETE in one transaction; partial failures roll back atomically
- Unit: `approve` with unknown `component_class` → error returned before transaction begins
- Unit: `approve` with missing component row → ROLLBACK, queue row preserved, error returned
- Unit: `approve` deletes the queue row and returns `Ok(component_id)` on success
- Unit: `gate1_pass` is `pub(crate)` — confirm it is not callable from outside the composition crate
- Unit: `run_q1_validation` — valid payload → `gate1_pass` called with empty errors
- Unit: `run_q1_validation` — invalid payload → `gate1_fail` called with error list
- Integration: round-trip with actual Postgres — `submit` → `run_q1_validation` → `approve` → queue row deleted, component `validation_status = 'validated'`

---

### Phase B — PythonCode Component (class 22)

**Status:** [ ] Pending

**Goal:** New component class for Python code/instruction elements targeted at the orchestrator.

#### Files to create

- `crates/brassclaw_pg/migrations/V052__reborn_python_code.sql` (**was V051 before Decision 2**)
  `class_code = 22`. Default consumer tags: `{02:orchestrator, 05:validator}`.
  **Do NOT include** `queue_code`, `review_attempts`, `review_feedback`, `rejected_at`,
  or `validation_errors` columns — those five are centralised on `reborn_validation_queue`
  (§0.18 / V051). The table DOES carry `validation_status` (the post-validation
  gate, which STAYS on the component table — see §0.18). **`reborn_validation_queue` now
  exists from V051 (Phase A.5)** — a WebUI-authored PythonCode row can enter the queue
  immediately on save. The Phase B WebUI-save path MUST call
  `ValidationQueueStore::submit(scope, component_id, 22)` on component creation.
  The §0.5 snippet→Q1→Q2 promotion flow completes at Phase N (V059 + gate logic), but
  the queue row is created here from day one.

  > **⚠️ FIND-AUDIT-15 — The plan previously said "same column shape as V036__reborn_specs.sql"
  > without providing the actual DDL. This is dangerous: V036 was created in an earlier
  > migration pass and retrofitted by V046 to add `prior_knowledge_content` /
  > `override_prompt_creation`. V052 must be the FINAL authoritative shape — with ALL
  > solution-override columns already present at creation time (no V046-style retrofit needed),
  > WITHOUT the 5 queue-tracking columns (per §0.18), and WITH `prompt_uid` (required for
  > the `fetch_for_consumer` UNION ALL sub-select which casts `prompt_uid::bigint` for
  > every table arm). Missing `prompt_uid` would make the UNION ALL fail at runtime.
  > The complete canonical DDL for V052 is below.**

  ```sql
  -- V052__reborn_python_code.sql
  -- PythonCode component table for BrassClaw Reborn (Phase B, class 22).
  --
  -- Executable Python bodies for Tier-0 recipe orchestration.
  -- Source: 'authored' (user) or 'system' (seeded by builtin_bootstrap.rs).
  -- consumer_tags default: {02:orchestrator, 05:validator} until validated.
  --
  -- DESIGN NOTE (§0.18): Queue-tracking columns (queue_code, review_attempts,
  -- review_feedback, rejected_at, validation_errors) are NOT on this table —
  -- they are centralised on reborn_validation_queue (V051). This table carries
  -- validation_status only (the post-validation gate that STAYS on the component).
  -- dependency_registry is included here at creation (V055 retroactively adds it
  -- to the 13 older tables; new tables include it from day one — Phase J.2).

  CREATE SEQUENCE IF NOT EXISTS reborn_python_code_prompt_uid_seq;

  CREATE TABLE IF NOT EXISTS reborn_python_code (
      id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

      tenant_id               TEXT        NOT NULL,
      user_id                 TEXT        NOT NULL,
      agent_id                TEXT        NOT NULL,
      project_id              TEXT        NOT NULL,

      name                    TEXT        NOT NULL
          CHECK (length(name) BETWEEN 1 AND 256),
      description             TEXT        NOT NULL DEFAULT ''
          CHECK (length(description) <= 1024),
      content                 TEXT        NOT NULL DEFAULT '',

      -- Solution-override columns (§3.13/§3.14 — SCH-02).
      -- Already present at creation (no retrofit migration needed).
      prior_knowledge_content TEXT,
      override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

      -- class_code = 22 (PythonCode)
      class_code              SMALLINT    NOT NULL DEFAULT 22
          CHECK (class_code = 22),
      prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_python_code_prompt_uid_seq'),

      consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',

      intent_examples         JSONB,

      -- Post-validation gate (STAYS on component table — see §0.18 / FIND-AUDIT-15).
      -- Queue-tracking columns (queue_code, review_attempts, review_feedback,
      -- rejected_at, validation_errors) are NOT here — centralised on
      -- reborn_validation_queue (V051).
      validation_status       TEXT        NOT NULL DEFAULT 'pending'
          CHECK (validation_status IN (
              'pending', 'auto_passed', 'auto_failed', 'validated',
              'review_requested', 'rejected', 'garbage', 'upgrade_queued'
          )),

      -- See FIND-P6-02 / FIND-AUDIT-15: 'system' must be allowed from day one
      -- (Phase L seeds rows with source = 'system'; V057 only alters older tables).
      source                  TEXT        NOT NULL DEFAULT 'authored'
          CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system')),
      content_hash            TEXT,
      similarity_parent_id    UUID,
      replaces_id             UUID,
      parent_version          TEXT,
      last_audit_at           TIMESTAMPTZ,
      audit_failure_count     SMALLINT    NOT NULL DEFAULT 0,
      parent_mission_id       UUID,

      -- Dependency registry (§0.19 / Phase J.2). New tables include it at creation.
      dependency_registry     JSONB,

      created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

      CONSTRAINT reborn_python_code_pk PRIMARY KEY (id),
      CONSTRAINT reborn_python_code_scope_name_unique
          UNIQUE (tenant_id, user_id, agent_id, project_id, name)
  );

  -- Required indexes (PERF-03 / FIND-AUDIT-15):
  -- Without these, the UNION ALL sub-select in fetch_for_consumer degrades to seq-scan.
  CREATE INDEX IF NOT EXISTS reborn_python_code_scope_idx
      ON reborn_python_code (tenant_id, user_id, agent_id, project_id);
  CREATE INDEX IF NOT EXISTS reborn_python_code_scope_status_idx
      ON reborn_python_code (tenant_id, user_id, agent_id, project_id, validation_status);
  CREATE INDEX IF NOT EXISTS reborn_python_code_scope_uid_idx
      ON reborn_python_code (tenant_id, user_id, agent_id, project_id, prompt_uid);
  CREATE INDEX IF NOT EXISTS reborn_python_code_consumer_tags_gin_idx
      ON reborn_python_code USING GIN (consumer_tags);
  CREATE INDEX IF NOT EXISTS reborn_python_code_similarity_parent_idx
      ON reborn_python_code (similarity_parent_id)
      WHERE similarity_parent_id IS NOT NULL;
  CREATE INDEX IF NOT EXISTS reborn_python_code_replaces_idx
      ON reborn_python_code (replaces_id)
      WHERE replaces_id IS NOT NULL;

  CREATE TRIGGER reborn_python_code_updated_at
      BEFORE UPDATE ON reborn_python_code
      FOR EACH ROW EXECUTE FUNCTION set_updated_at();
  ```

- `crates/brassclaw_reborn_composition/src/pg_python_code_store.rs` — new store

#### Files to modify

- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  - Add class 22 to the `fetch_for_consumer` UNION ALL sub-select (new table arm with `reborn_python_code`).
  - Add class 22 arm to `fetch_component_by_id` (currently returns `None` for class 22).
  > **⚠️ FINDING C:** `fetch_for_consumer` (the UNION ALL fallback) and `fetch_component_by_id`
  > (the direct UUID lookup) are **separate functions**. Both must have a class 22 arm.
  > Currently neither has one — class 22 silently returns nothing on both code paths.
  > Phase B adds both arms.
- `crates/brassclaw_engine/src/memory/intent_system.rs` — add `22 => "python_code"` to `class_label`
  (lowercase snake_case — consistent with `intent_system.rs` style: e.g. `21 => "recipe"`)
  > Also update the doc-comment at `intent_system.rs:250–253` to include `22=python_code` in the
  > class-code legend (the comment lists 0–21 and 50 but stops at 21). Phase C adds the `23`
  > update to the same doc-comment.
  > **✅ Review note (pre-v3 audit) — RESOLVED (obsolete, do not implement):** the plan previously also instructed adding a
  > `22 => 0.42` arm to `doc_type_weight_by_class(i32)` in `retrieval_source.rs`. That
  > function no longer exists (removed in `Goals_pre_v3_review.md` Step 12) — both retrieval
  > backends now order by `(class_code ASC, prompt_uid ASC)`. This sub-step is dropped; see
  > §0.11 review note.
- `crates/brassclaw_engine/src/memory/component_validator.rs` — class 22 dispatch:
  name format, non-empty content, soft 10k token budget, shell-injection scan.
  > **⚠️ FIND-AUDIT-12 — "shell-injection scan" was unspecified in all prior plan drafts.**
  > An implementer cannot invent security rules from scratch. The CONCRETE rules are:
  >
  > **Hard errors (Q1 fail):**
  > - `import os` — direct OS module import (use `__execute_action__` instead)
  > - `import subprocess` — subprocess invocation (bypasses capability dispatch)
  > - `import sys` — sys-module access (interpreter manipulation risk)
  > - `import socket` — direct network socket access (bypasses host network controls)
  > - `import ctypes` — C foreign-function interface (native escape)
  > - `import importlib` — dynamic module loading (import whitelist bypass)
  > - `__import__(` — built-in dynamic import call (same risk as importlib)
  > - `exec(` — raw code execution inside PythonCode body (nested unsandboxed exec)
  > - `eval(` — unsafe expression evaluation (injection risk)
  > - `open(` — direct filesystem access (bypasses host file-access controls — use `__execute_action__("read_file", ...)` etc.)
  > - `compile(` — Python code compilation (code-object injection path)
  > - `__builtins__` — builtins manipulation attempt
  > - `globals()` or `locals()` — scope inspection for injection
  >
  > **Warnings (Q1 soft — flag, do not block):**
  > - `print(` — stdout writes (allowed but the VM captures stdout, not the host terminal; may be intentional for debug output)
  > - `input(` — interactive prompt (will hang in VM; likely a copy-paste error)
  >
  > **Implementation note:** Scan the raw `content` string before execution using simple substring
  > search (no AST required). False-positive rate is low because PythonCode bodies are authored
  > to call host functions (`__execute_action__`, `__check_budget__`, etc.), NOT OS/subprocess.
  > The scan is additive to Q1 checks — it does NOT replace them.
  >
  > **Test coverage (add to Phase I §shell-injection tests — see Phase I Tests section):**
  > - `import os` in content → Q1 hard error
  > - `import subprocess` in content → Q1 hard error
  > - `exec(` in content → Q1 hard error
  > - `open(` in content → Q1 hard error
  > - `__builtins__` in content → Q1 hard error
  > - `__execute_action__("read_file", {"path": path})` in content → Q1 pass (correct usage)
  > - `print("debug")` in content → Q1 warning only, not a hard error

  > **⚠️ FINDING E — `ComponentPayload` for class 22:** The existing `ComponentPayload` enum has
  > `ToolSkill(&'a ToolSkill)`, `Recipe(&'a Recipe)`, and `Generic(GenericComponent<'a>)`.
  > There is NO `PythonCode` variant. Class 22 validation must use `Generic(GenericComponent<'a>)`
  > where `GenericComponent` carries `{ name, description, content }` (confirmed 3-field shape —
  > FIND-P10-03). The `validate_by_class` dispatch adds a `22 =>` arm that reads from the
  > `Generic` payload. The `class_code` is implicit from the match arm; do NOT add it to the struct.
  > A new dedicated `ComponentPayload::PythonCode` variant may be added if richer validation is
  > needed, but the simpler path is to use `Generic`. The plan must not assume a `PythonCode`
  > variant exists — it does not yet.

> **⚠️ FIND-P6-07 — `interceptor_config_service.rs::class_label` is a pre-existing incomplete stub.**
> The function at `interceptor_config_service.rs:65` covers only classes 0, 1, 9, 10, 12, 13, 14,
> 15, 16, 18, 19, 20, 21, 50. Classes 2, 3, 4–8, 11, 17 fall through to `"Component"`. This is
> pre-existing technical debt — do NOT fix those gaps in Phase B. Only add the two new arms:
> `22 => "PythonCode"` and `23 => "Catalogue"` as specified.

> **⚠️ FIND-P6-02 — V052 (`reborn_python_code`) `source` column MUST allow `'system'` from day one.**
> (V-numbers updated per Decision 2: was V051/V052, now V052/V053.)
> Phase L's `builtin_bootstrap.rs` seeds PythonCode rows with `source = 'system'`. V057 adds
> `'system'` to `reborn_tools`, `reborn_tool_skills`, and `reborn_skills` — but V052 is created
> before V057. The `reborn_python_code` table must include `'system'` in its own source CHECK
> constraint at creation time:
> ```sql
> source TEXT NOT NULL DEFAULT 'authored'
>     CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system')),
> ```
> Similarly, `reborn_extension_catalogues` (V053) must include `'system'` in its `source` CHECK.
> Do NOT omit `'system'` from these tables expecting V057 to add it — V057 only alters the
> pre-existing tables. New table CREATE statements must include all allowed values up front.

> **Do NOT modify `crates/brassclaw_engine/src/types/memory.rs` (DocType enum).**
> `DocType` is `#[deprecated(since = "0.1.0")]` and frozen. Phase B uses class-code integers
> throughout. There is no `DocType::PythonCode`. See §0.11 note.

#### Tests

- Unit: `class_label(22) == "python_code"` (in `intent_system.rs` test — add to existing `class_label_known_codes` test fn)
- Unit: `interceptor_config_service::class_label(22) == "PythonCode"` (local copy — `&'static str` style, NOT snake_case — FIND-20; `class_label` there is private so the test lives in `#[cfg(test)] mod tests { use super::*; }` inside `interceptor_config_service.rs`)
- Unit: `class_label(22) == "PythonCode".to_string()` (in `recipe_store.rs` `#[cfg(test)] mod tests { use super::*; }` — `class_label` is a private `fn`, NOT `pub fn`; the test CANNOT be called as `recipe_store::class_label(22)` from outside the module. Place the assertion inside `recipe_store.rs` tests using `use super::*`. — DESIGN-ISSUE-02 / FIND-20)
- ~~Unit: `doc_type_weight_by_class(22) == 0.42`~~ — **removed**: function no longer exists (§0.11 review note)
- Integration: PythonCode row retrieved via `fetch_for_consumer` with consumer tag `02:orchestrator`
- Integration: PythonCode row retrieved via `fetch_component_by_id(uuid, 22)` (UUID lookup path)

---

### Phase C — ExtensionCatalogue Component (class 23)

**Status:** [ ] Pending

**Goal:** Documentation-container class that organises a capability domain.

#### Files to create

- `crates/brassclaw_pg/migrations/V053__reborn_extension_catalogues.sql` (**was V052 before Decision 2**)
  `class_code = 23`. Default consumer tags: `{02:orchestrator, 05:validator}`.
  **Do NOT include** `queue_code`, `review_attempts`, `review_feedback`, `rejected_at`,
  or `validation_errors` columns — those five are centralised on `reborn_validation_queue`
  (§0.18 / V051). The table carries `validation_status` only (which STAYS).
  **`reborn_validation_queue` now exists from V051 (Phase A.5)** — a WebUI-authored
  ExtensionCatalogue row can enter the queue immediately on save. The Phase C WebUI-save
  path MUST call `ValidationQueueStore::submit(scope, component_id, 23)` on component
  creation. The Q1/Q2 gate logic completing at Phase N (V059) is required before
  snippet→component promotion can run, but the queue row is created from day one.

  > **⚠️ FIND-AUDIT-16 — The plan previously listed only column names without providing actual DDL.
  > This is insufficient: ExtensionCatalogue has a DIFFERENT content layout from PythonCode
  > (no plain `content` column — `overview_doc` is the primary text field, `task_groups` and
  > `child_component_ids` are JSONB/UUID-array extras). It also requires `prompt_uid` (for
  > `fetch_for_consumer` UNION ALL which casts `prompt_uid::bigint` for every table arm), and
  > all solution-override columns at creation time (no retrofit needed), and `dependency_registry`
  > from day one (same as V052). The complete canonical DDL for V053 is below.**

  ```sql
  -- V053__reborn_extension_catalogues.sql
  -- ExtensionCatalogue component table for BrassClaw Reborn (Phase C, class 23).
  --
  -- Documentation-container that organises a capability domain.
  -- Primary text field: overview_doc (maps to effective_content in UNION ALL).
  -- Source: 'authored' (user) or 'system' (seeded by builtin_bootstrap.rs).
  -- consumer_tags default: {02:orchestrator, 05:validator} until validated.
  --
  -- DESIGN NOTE (§0.18): Queue-tracking columns are NOT on this table.
  -- dependency_registry is included here at creation (V055 retroactively adds it
  -- to the 13 older tables; new tables include it from day one — Phase J.2).

  CREATE SEQUENCE IF NOT EXISTS reborn_extension_catalogues_prompt_uid_seq;

  CREATE TABLE IF NOT EXISTS reborn_extension_catalogues (
      id                      UUID        NOT NULL DEFAULT gen_random_uuid(),

      tenant_id               TEXT        NOT NULL,
      user_id                 TEXT        NOT NULL,
      agent_id                TEXT        NOT NULL,
      project_id              TEXT        NOT NULL,

      name                    TEXT        NOT NULL
          CHECK (length(name) BETWEEN 1 AND 256),
      description             TEXT        NOT NULL DEFAULT ''
          CHECK (length(description) <= 1024),
      version                 TEXT        NOT NULL DEFAULT '1.0',

      -- Primary text content (maps to effective_content in UNION ALL via COALESCE):
      --   COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)
      overview_doc            TEXT        NOT NULL DEFAULT '',

      -- Structured extras (Phase C — accessed in validator via GenericComponent.extra).
      task_groups             JSONB       NOT NULL DEFAULT '[]',
      child_component_ids     UUID[]      NOT NULL DEFAULT '{}',
      intent_index            JSONB,                           -- audit-only, NOT indexed

      -- Solution-override columns (§3.13/§3.14 — SCH-02).
      -- Already present at creation (no retrofit migration needed).
      prior_knowledge_content TEXT,
      override_prompt_creation BOOLEAN    NOT NULL DEFAULT false,

      -- class_code = 23 (ExtensionCatalogue)
      class_code              SMALLINT    NOT NULL DEFAULT 23
          CHECK (class_code = 23),
      prompt_uid              BIGINT      NOT NULL DEFAULT nextval('reborn_extension_catalogues_prompt_uid_seq'),

      consumer_tags           TEXT[]      NOT NULL DEFAULT '{}',

      intent_examples         JSONB,

      -- Post-validation gate only (see §0.18 / FIND-AUDIT-16).
      validation_status       TEXT        NOT NULL DEFAULT 'pending'
          CHECK (validation_status IN (
              'pending', 'auto_passed', 'auto_failed', 'validated',
              'review_requested', 'rejected', 'garbage', 'upgrade_queued'
          )),

      -- See FIND-P6-02 / FIND-AUDIT-16: 'system' must be allowed from day one.
      source                  TEXT        NOT NULL DEFAULT 'authored'
          CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system')),
      content_hash            TEXT,
      similarity_parent_id    UUID,
      replaces_id             UUID,
      parent_version          TEXT,
      last_audit_at           TIMESTAMPTZ,
      audit_failure_count     SMALLINT    NOT NULL DEFAULT 0,
      parent_mission_id       UUID,

      -- Dependency registry (§0.19 / Phase J.2). New tables include it at creation.
      dependency_registry     JSONB,

      created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),

      CONSTRAINT reborn_extension_catalogues_pk PRIMARY KEY (id),
      CONSTRAINT reborn_extension_catalogues_scope_name_unique
          UNIQUE (tenant_id, user_id, agent_id, project_id, name)
  );

  -- Required indexes (PERF-03 / FIND-AUDIT-16):
  -- Without these, the UNION ALL sub-select in fetch_for_consumer degrades to seq-scan.
  CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_scope_idx
      ON reborn_extension_catalogues (tenant_id, user_id, agent_id, project_id);
  CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_scope_status_idx
      ON reborn_extension_catalogues (tenant_id, user_id, agent_id, project_id, validation_status);
  CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_scope_uid_idx
      ON reborn_extension_catalogues (tenant_id, user_id, agent_id, project_id, prompt_uid);
  CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_consumer_tags_gin_idx
      ON reborn_extension_catalogues USING GIN (consumer_tags);
  CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_similarity_parent_idx
      ON reborn_extension_catalogues (similarity_parent_id)
      WHERE similarity_parent_id IS NOT NULL;
  CREATE INDEX IF NOT EXISTS reborn_extension_catalogues_replaces_idx
      ON reborn_extension_catalogues (replaces_id)
      WHERE replaces_id IS NOT NULL;

  CREATE TRIGGER reborn_extension_catalogues_updated_at
      BEFORE UPDATE ON reborn_extension_catalogues
      FOR EACH ROW EXECUTE FUNCTION set_updated_at();
  ```

- `crates/brassclaw_reborn_composition/src/pg_extension_catalogue_store.rs` — new store

#### Files to modify

Same engine files as Phase B, but for class 23:
- `retrieval_source.rs`
  - Add class 23 arm to `fetch_for_consumer` UNION ALL sub-select (`reborn_extension_catalogues`, `effective_content = overview_doc`).
  - Add class 23 arm to `fetch_component_by_id`.
  > **⚠️ FINDING C (same as Phase B):** Both `fetch_for_consumer` AND `fetch_component_by_id`
  > need a class 23 arm. Neither has one today.
- `intent_system.rs` — `23 => "extension_catalogue"` in `class_label` (lowercase snake_case —
  consistent with `intent_system.rs` style; e.g. `21 => "recipe"`)
  > Also update the doc-comment at `intent_system.rs:250–253` to add `23=extension_catalogue`
  > to the class-code legend (Phase B added `22=python_code` to the same comment).

  > **⚠️ FIND-20 — THREE `class_label` copies have DIFFERENT label styles. Use the correct style
  > for each copy:**
  >
  > | File | Style | Class 22 arm | Class 23 arm |
  > |------|-------|-------------|-------------|
  > | `intent_system.rs:254` | lowercase snake_case | `22 => "python_code"` | `23 => "extension_catalogue"` |
  > | `interceptor_config_service.rs:65` | returns `&'static str`, mixed case | `22 => "PythonCode"` | `23 => "Catalogue"` |
  > | `recipe_store.rs:861` | display-label style `.to_string()` | `22 => "PythonCode".to_string()` | `23 => "Catalogue".to_string()` |
  >
  > **⚠️ FIND-P5-01 + FIND-NEW-AUDIT-02 — `recipe_store.rs` uses user-facing DISPLAY labels, not class-name labels:**
  > Verified `recipe_store.rs:861-882` (full read, Pass 11): this function uses descriptive display labels.
  > The complete current arm mapping is:
  > `0 => "Tool"`, `1 => "Skill (Rusty)"`, `2 => "Skill (Monty)"`, `3 => "Skill (LLM)"`,
  > `4..=9 => format!("Extension (class {code})")`, `10 => "Orchestrator"`,
  > `12 => "Document"` (**NOT "Spec"** — the label is "Document" here, distinct from `interceptor_config_service.rs` which returns `"Spec"`),
  > `13 => "Guide"`, `14 => "Reference"`, `15 => "Note"`, `16 => "Action"`, `17 => "Template"`,
  > `18 => "Snippet"`, `19 => "Config"`, `20 => "Workflow"`, `21 => "Recipe"`, `50 => "Scaffold"`.
  > The Phase B/C additions `22 => "PythonCode"` and `23 => "Catalogue"` fit the single-word pattern.
  > These display labels are NOT the same as the `intent_system.rs` class labels and are used
  > only for WebUI display. The test assertion (inside `recipe_store.rs`'s own `#[cfg(test)] mod tests { use super::*; }`)
  > `assert_eq!(class_label(22), "PythonCode".to_string())` is correct. The function is private — cannot be called as `recipe_store::class_label` from outside. (DESIGN-ISSUE-02 resolved.)
  >
  > The `interceptor_config_service.rs` copy also has its return type annotated as `&'static str`
  > (not `String`), so the match arms must use string literals `"PythonCode"`, not
  > `.to_string()`. Keep the return type and style consistent with each copy.
  > The `recipe_store.rs` copy returns `String` via `.to_string()` — use `"PythonCode".to_string()` there.
  >
  > **Type note — `class_code` parameter:** Both `recipe_store.rs:861` and
  > `interceptor_config_service.rs:65` take `u16` as the `class_code` parameter type.
  > The new arms are just `22 =>` and `23 =>` (Rust infers `u16` from the match context).
  > No cast or type annotation needed.
  > **✅ Review note (pre-v3 audit) — RESOLVED (obsolete, do not implement):** as with Phase B, the previously-planned `23 => 0.38`
  > arm on `doc_type_weight_by_class(i32)` in `retrieval_source.rs` is obsolete — that
  > function was already removed; see §0.11 review note. No weight arm to add.
- `component_validator.rs` — class 23: name format, non-empty `overview_doc`, ≥1 `task_group`,
  valid UUID syntax in `child_component_ids`

  > **⚠️ COMP-04 — `GenericComponent` cannot carry `task_groups` JSONB for class 23:**
  > `GenericComponent` (from `component_validator.rs`) has only `{ name, description, content }`.
  > For class 23 (`ExtensionCatalogue`), `content` maps to `overview_doc` (a text field) — that
  > part works. But the `≥1 task_group` validation rule requires accessing the `task_groups JSONB`
  > column, which `GenericComponent` cannot carry. Two options:
  > - **Option A (simpler):** extend `GenericComponent` with an optional `extra: Option<serde_json::Value>`
  >   field that the caller populates for class 23 with the parsed `task_groups` JSON. The validator
  >   checks `extra["task_groups"].as_array().map(|a| a.len() >= 1)`.
  > - **Option B (typed):** add `ComponentPayload::ExtensionCatalogue(&'a ExtensionCatalogueData)`
  >   with a small struct `{ name, overview_doc, task_groups }`. More explicit but requires a
  >   new payload type.
  > **Option A is recommended** as the minimal change that keeps `ComponentPayload` lean.
  > Phase C must pick one and spec the concrete `GenericComponent` extension.
  >
  > **⚠️ FIND-06 — Option A requires cascading changes to ALL existing `GenericComponent` constructors:**
  > Adding `extra: Option<serde_json::Value>` to `GenericComponent` (verified struct at
  > `component_validator.rs:62–67`: has exactly three fields `name`, `description`, `content`) will
  > require updating EVERY call site that constructs `GenericComponent { name, description, content }`
  > to also pass `extra: None`. This is a mandatory breaking change to the struct — not just
  > the validator dispatch. Option B avoids this structural change but adds a new payload type.
  >
  > **Known internal construction sites in `component_validator.rs` that MUST be updated:**
  > - `validate_skill_generic(g, config)` — constructs `GenericComponent` inline
  > - `validate_tool_generic(g, config)` — constructs `GenericComponent` inline
  > - `validate_extension(g, config)` — constructs `GenericComponent` inline
  > - `validate_generic(g, config)` — constructs `GenericComponent` inline
  > - Every `ComponentPayload::Generic(g)` construction site in the composition layer
  >
  > Run `grep -rn "GenericComponent {" crates/` before implementing to find all sites.
  > The Rust compiler will catch any missed site (struct update is non-exhaustive-safe since
  > `GenericComponent` is not `#[non_exhaustive]`, so missing the field is a hard compile error).

> **Do NOT modify `types/memory.rs` (DocType enum).** `DocType` is `#[deprecated]` and frozen.
> No `DocType::ExtensionCatalogue`. See §0.11 note.

#### Tests

- Unit: `class_label(23) == "extension_catalogue"` (in `intent_system.rs` test — add to existing `class_label_known_codes` test fn)
- Unit: `interceptor_config_service::class_label(23) == "Catalogue"` (local copy — `&'static str` style — FIND-20; placed inside `#[cfg(test)] mod tests { use super::*; }` within `interceptor_config_service.rs`)
- Unit: `class_label(23) == "Catalogue".to_string()` (in `recipe_store.rs` `#[cfg(test)] mod tests { use super::*; }` — same visibility note as class 22 above — DESIGN-ISSUE-02 / FIND-20)
- ~~Unit: `doc_type_weight_by_class(23) == 0.38`~~ — **removed**: function no longer exists (§0.11 review note)
- Integration: Catalogue with `task_groups` → retrieved with `overview_doc` as `effective_content` via `fetch_for_consumer`
- Integration: Catalogue retrieved via `fetch_component_by_id(uuid, 23)` (direct UUID lookup)

---

### Phase D — `step_link` Column on Intent Inputs

**Status:** [ ] Pending

**Goal:** Add `step_link` to `reborn_intent_inputs`; wire into `resolve_intent` and
`IntentResolution::Match`.

#### Files to create

- `crates/brassclaw_pg/migrations/V054__reborn_intent_inputs_step_link.sql` (**was V053 before Decision 2**)
  ```sql
  ALTER TABLE reborn_intent_inputs ADD COLUMN IF NOT EXISTS step_link TEXT
      CHECK (length(step_link) <= 4096);
  ```

#### Files to modify

- `crates/brassclaw_engine/src/memory/intent_system.rs`
  Add BOTH `step_link: Option<String>` AND `component_name: String` to `IntentResolution::Match`.
  (See §0.8 / FIND-P5-06 — `component_name` is needed for `ActionShortCircuit` in Phase E.)
  Update the resolution query to `SELECT ..., step_link, COALESCE(a.name, '') AS component_name FROM ...` with the LEFT JOIN shown in FIND-P5-06.
  Update `seed_intent_input` to accept and store `step_link`.

  > **⚠️ FIND-P10-01 / FIND-P10-05 — exact new column positions for `resolve_intent` SELECT.**
  > The live query (confirmed `intent_system.rs:340-356`) selects exactly 5 columns:
  > `id(0), component_id(1), component_class_code(2), input_class(3), score(4)`.
  > After Phase D adds the LEFT JOIN and two extra columns, append them at the END of the
  > SELECT (after `score`) — never insert them in the middle, which would shift all
  > existing `row.get(N)` calls. The new full SELECT column list and `row.get(N)` positions are:
  >
  > | Index | Column | Rust binding |
  > |-------|--------|-------------|
  > | 0 | `ii.id` | `row.get(0)` — row id (disambiguation key) |
  > | 1 | `ii.component_id` | `row.get(1)` — `component_id: Uuid` |
  > | 2 | `ii.component_class_code` | `row.get(2)` — `component_class_code: i32` |
  > | 3 | `ii.input_class` | `row.get(3)` — `input_class: i16` |
  > | 4 | `ii.score` | `row.get(4)` — `score: i32` |
  > | 5 | `ii.step_link` | `row.get(5)` — `step_link: Option<String>` (NEW) |
  > | 6 | `COALESCE(a.name, '')` | `row.get(6)` — `component_name: String` (NEW) |
  >
  > The disambiguation path (which also calls `row.get(0..4)`) is unaffected because the
  > new columns are appended. The `Disambiguation` arm in `IntentResolution` construction
  > does NOT read `step_link` or `component_name` — it uses only columns 0–4. Do NOT
  > insert columns 5/6 before column 4 (score).
  >
  > **SQL change (replace the current SELECT line in `resolve_intent`):**
  > ```sql
  > SELECT ii.id, ii.component_id, ii.component_class_code, ii.input_class, ii.score,
  >        ii.step_link,
  >        COALESCE(a.name, '') AS component_name
  > FROM reborn_intent_inputs ii
  > LEFT JOIN reborn_actions a
  >        ON a.id = ii.component_id
  >       AND ii.component_class_code = 16
  >       AND a.tenant_id  = $1
  >       AND a.user_id    = $2
  >       AND a.agent_id   = $3
  >       AND a.project_id = $4
  > WHERE ii.tenant_id = $1 AND ii.user_id = $2 AND ii.agent_id = $3 AND ii.project_id = $4
  >   AND ii.input_text = $5
  >   AND ii.input_class = ANY($6)
  > ORDER BY
  >   CASE ii.input_class WHEN $7 THEN 0 WHEN $8 THEN 1 WHEN $9 THEN 2 ELSE 3 END,
  >   ii.score DESC
  > LIMIT 30
  > ```
  > (Phase M will further extend this to add the template OR-paths, but Phase D is the
  > first to add the LEFT JOIN + new columns. Phase M's extension must keep `step_link` at
  > index 5 and `component_name` at index 6.)
  > **⚠️ FIND-NEW-03 — `seed_intent_input` extension is fully specified here (verified
  > against `intent_system.rs:463-505`).** The live function has 7 params and an INSERT
  > with 11 columns / 10 placeholders (`VALUES ($1..$8,1,$9,$10)` — `score` is the literal
  > `1`) and `ON CONFLICT (...) DO UPDATE SET source, needs_review, updated_at`. The
  > `step_link` extension is NOT just "accept and store" — three concrete edits:
  >
  > 1. **Signature** — add an 8th param:
  >    ```rust
  >    pub async fn seed_intent_input(
  >        pool: &brassclaw_pg::PgPool,
  >        scope: &IntentScope,
  >        input_text: &str,
  >        input_class: InputClass,
  >        component_id: Uuid,
  >        component_class_code: i32,
  >        source: IntentSource,
  >        step_link: Option<&str>,   // NEW — None for non-Recipe inputs; the variant's
  >                                   //        step_link for Recipe (class 21) variant intents
  >    ) -> Result<(), IntentSystemError>
  >    ```
  > 2. **INSERT** — add `step_link` to the column list AND a new placeholder `$11`:
  >    ```sql
  >    INSERT INTO reborn_intent_inputs
  >        (tenant_id, user_id, agent_id, project_id,
  >         input_text, input_class, component_id, component_class_code,
  >         score, source, needs_review, step_link)          -- 12 columns (was 11)
  >    VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,$9,$10,$11)          -- 11 placeholders (was 10)
  >    ON CONFLICT (tenant_id, user_id, agent_id, project_id,
  >                 input_text, input_class, component_id)
  >    DO UPDATE SET
  >        source       = EXCLUDED.source,
  >        needs_review = EXCLUDED.needs_review,
  >        step_link    = EXCLUDED.step_link,                 -- NEW
  >        updated_at   = now()
  >    ```
  >    The bind slice gains `&step_link` as the 11th arg (after `&source.needs_review()`).
  >    `step_link` is NOT part of the conflict key (the key is scope+input_text+
  >    input_class+component_id); it is updated via `SET` on re-seed.
  > 3. **All existing callers** must pass the new `step_link` arg — the compiler
  >    catches a missing arg (arity mismatch). Non-Recipe seeders pass `None`; the
  >    Recipe variant seeder (Phase D / Phase L seeder) passes the variant's
  >    `step_link` (so `resolve_intent` can return it for the IBS path). J.1's
  >    `auto_passed` → `seed_intent_input` wiring (Phase J) passes `None` for Skill
  >    intents. The WebUI Recipe save path (Phase A round-trip) re-seeds each
  >    variant's `intent_examples` with that variant's `step_link`.
  >
  > **Sequencing:** as with the `SELECT`, this INSERT edit requires V053 to have run
  > (the `step_link` column must exist) — see the sequencing invariant below.
  > **⚠️ FINDING A — `record_disambiguation_choice` also returns `Match` — verified against code:**
  > Confirmed at `intent_system.rs:433–455`: `record_disambiguation_choice` takes
  > `(pool, scope, row_id, component_id, component_class_code)` as parameters (no `step_link`)
  > and returns `Ok(IntentResolution::Match { component_id, component_class_code })` at line 451.
  > When `step_link: Option<String>` and `component_name: String` are added to the `Match`
  > variant, this return statement must be updated to include `step_link: None` AND
  > `component_name: String::new()`. This is the correct semantics: a user who clicked a
  > disambiguation button confirmed a `component_id` — the caller then re-fetches the recipe
  > row for its `step_link`. `step_link: None` here instructs the caller to use the legacy
  > `fetch_component_by_id` path, which for a Recipe match will fall through to the intent-less
  > retrieval path (acceptable post-disambiguation — the full IBS path with step_link is taken
  > on the _next_ turn when the user's text directly matches the intent).
  > `component_name: String::new()` for disambiguation is acceptable — the disambiguation
  > result is always a Recipe/Skill, never an Action (Actions are matched unambiguously).
  > Either way this is a **mandatory compile-time update site** — the compiler will catch it.

- All call sites that destructure `IntentResolution::Match { component_id, component_class_code }`:
  bind `step_link` as well. Non-IBS paths treat `None` as a legacy match (unchanged behaviour).
  Call sites include:
  - `retrieval_source.rs` — `fetch_for_turn` at **line 516–519** (primary path, inline destructure;
    currently `{ component_id, component_class_code }` — add `step_link: _` or `step_link` to bind)
  - `orchestrator.rs` — any call site that destructures `Match`
  - `intent_system.rs` — `record_disambiguation_choice` return statement (**FINDING A**)

  > **⚠️ FIND-P6-05 — Security: the LEFT JOIN on `reborn_actions` for `component_name` MUST
  > include all 4 scope parameters.** The SQL added in Phase D LEFT JOINs `reborn_actions a`
  > to populate `component_name` for class-16 matches. The JOIN condition MUST include
  > `AND a.tenant_id = $1 AND a.user_id = $2 AND a.agent_id = $3 AND a.project_id = $4`.
  > Without these, a `component_id` UUID that exists in a different tenant's `reborn_actions`
  > table would populate `component_name` with that other tenant's Action name — a cross-scope
  > information-leakage bug. The §0.8 SQL snippet already shows these filters correctly;
  > this note exists to flag it as a hard security requirement, not an optimization.
  > The full correct JOIN clause is:
  > ```sql
  > LEFT JOIN reborn_actions a
  >        ON a.id = ii.component_id
  >       AND ii.component_class_code = 16
  >       AND a.tenant_id = $1
  >       AND a.user_id   = $2
  >       AND a.agent_id  = $3
  >       AND a.project_id = $4
  > ```
  > Never simplify this to `ON a.id = ii.component_id` without the scope filters.

  > **⚠️ FIND-23 — `retrieval_source.rs::fetch_for_turn` line 516–519 is a mandatory compile-time
  > update site for Phase D:** verified by reading the code — `PostgresSource::fetch_for_turn`
  > (lines 515–519) contains:
  > ```rust
  > match resolve_intent(&self.pool, &intent_scope, query).await {
  >     Ok(IntentResolution::Match {
  >         component_id,
  >         component_class_code,
  >     }) => { ... }
  > ```
  > When `step_link: Option<String>` is added to `IntentResolution::Match`, this inline
  > destructure will fail to compile. Add `step_link` to the binding:
  > ```rust
  > Ok(IntentResolution::Match {
  >     component_id,
  >     component_class_code,
  >     step_link,
  > }) => { ... }
  > ```
  > The compiler will catch this — do not miss it. This is the same retrieval_source.rs
  > file that Phase E subsequently upgrades to dispatch on `step_link`. Phase D adds the
  > `step_link` binding; Phase E adds the dispatch logic using it.

> **Sequencing invariant:** The Rust code change (adding `step_link` to `IntentResolution::Match`
> and the `SELECT ... step_link` query) **requires V054 to have run first**. V054 adds the column.
> If the code change deploys before V054 runs, the `SELECT` will fail at runtime.
> Required order: run V054 migration → then deploy the code that reads `step_link`.
> This means Phase D migration (V054) and Phase D code change are a two-step deploy,
> not a single atomic deploy. Plan accordingly.

**Notes:**
- `step_link` replaces `variant_key`. No `variant_key` column is added to `reborn_intent_inputs`.
- `step_link` is nullable. Existing rows use the existing `fetch_component_by_id` path unchanged.
- The current `IntentResolution::Match` in the codebase has `{ component_id: Uuid, component_class_code: i32 }` — no `step_link`. All destructuring sites, **including `record_disambiguation_choice`**, must be updated simultaneously.

#### Tests

- Unit: intent row with `step_link` → `IntentResolution::Match { step_link: Some(...), component_name: "" }`
- Unit: intent row without `step_link` → `IntentResolution::Match { step_link: None, component_name: "" }` → existing path unchanged
- Unit: class-16 intent row → `IntentResolution::Match { component_class_code: 16, component_name: "daily-sync" }` — name populated from LEFT JOIN
- Unit: class-21 intent row → `IntentResolution::Match { component_class_code: 21, component_name: "" }` — empty name for non-Action matches
- Unit: `record_disambiguation_choice` → `IntentResolution::Match { step_link: None, component_name: "" }` — both new fields set correctly

---

### Phase E.0 — Wire `PostgresSource` in Composition (Prerequisite — resolves C2/C3/C4)

**Status:** [ ] Pending

> **🔗 SUBSTEP — Re-targeted E0-A (agent-loop adaptation):** Grounding proved the engine
> `ExecutionLoop`/`ThreadManager`/`execute_orchestrator`/`fetch_for_turn` path this section
> targets (`manager.rs:377–383`) is **dormant / test-only** — the live production turn
> driver is the **agent-loop** stack (`PlannedDriver`/`CanonicalAgentLoopExecutor`), which
> today runs with **no retrieval wired** (`LoopContextPort.memory_snippets` = `Vec::new()`;
> `RecipeStage::process` is a no-op stub because the pipeline never exposes raw user text).
> Per user decision (E0-A + Option 1, plan-ordered), E.0 is re-targeted to make
> `PostgresSource::fetch_for_turn` fire inside **live agent-loop turns** via the composition
> bridge, pulling `LoopRetrievalPort` (Phase H4) + user-text plumbing (Phase H3) forward.
> Full approach, faithful deviations (orphan-rule/dependency → mirror the `RecipeLookup`
> precedent via a new turns-layer `RetrievalLookup`/`MessageTextResolver` trait), exact edit
> sites, tests, and acceptance: **`docs/agents-v3/subplan_problem_stepE0_of_saved_plan_to_v3.md`**.
> Run that subplan's steps before resuming the remainder of this E.0 section. Routing booleans
> stay conservative (`llm_call_required=true`, `tier0_eligible=false`) until Phase E's
> `SplitResult`; the Tier-0/Tier-1 consumer stays at Phase H.

**Goal:** Make `PostgresSource` the **live** retrieval backend before any phase consumes its
new `FetchForTurnResult` variants. Today `manager.rs:377–383` constructs `RamSource` and
passes it to `with_retrieval_source(...)`; `PostgresSource` is exported from
`brassclaw_engine` but **never instantiated in the composition path** (verified — a
`TODO(Phase K)` marker sits at `manager.rs:377`). Phases E/F/G/H all assume the
intent-driven `fetch_for_turn` path is live; without E.0 it is dormant and those phases
ship correct-but-unreachable code. Phase K then becomes pure deletion (no wiring race).

This is the single most important ordering fix in the plan. It is scheduled as **E.0**
(zeroth step of the E family) so it lands before Phase E's `fetch_for_turn` upgrade and
before Phase H's `RecipeStage` dispatch (which consumes `SplitResult`/`ActionShortCircuit`
via the §H.0 `LoopRetrievalPort`). Cross-ref `Goals_pre_v3_review.md` Step 14's ordering
constraint: "Phase K must come AFTER the `PostgresSource` wiring sub-task; otherwise the
production retrieval path breaks." E.0 is that wiring sub-task, pulled forward.

#### Files to modify

> **⚠️ ARCH-01 — `ThreadManager` does NOT have `with_retrieval_source`; it lives on `ExecutionLoop`:**
> Verified in the codebase: `with_retrieval_source` is defined on `ExecutionLoop`
> (`crates/brassclaw_engine/src/executor/loop_engine.rs:219`) and is called at `manager.rs:400`
> when building the `ExecutionLoop` inside the `ThreadManager::spawn` path. `ThreadManager`
> itself has no such method and no `retrieval_source_override` field. The plan's framing
> that "the `ExecutionLoop::with_retrieval_source` at line 400 is internal-only, not
> callable from composition" is **correct** — but the solution must therefore be:
> add the override field + builder method to `ThreadManager` itself (so composition can call it
> before `spawn`), NOT to `ExecutionLoop` (which is built inside `spawn` and not
> accessible from outside the engine crate). This is exactly what Phase E.0 prescribes —
> the description below is correct; only the "(the `ExecutionLoop::with_retrieval_source`
> at line 400 is internal-only)" parenthetical has been clarified to confirm that
> `with_retrieval_source` already exists on `ExecutionLoop` and is already called
> inside `manager.rs:400` — but it is driven by an `Arc<dyn RetrievalSource>` that
> `ThreadManager` builds at spawn-time from its own fields. Phase E.0's task is to
> add `retrieval_source_override: Option<Arc<dyn RetrievalSource>>` to `ThreadManager`
> (line ~34) + a builder method, then use it in the spawn path at lines 377–383.

> **⚠️ ARCH-02 — `ThreadManager` is NOT instantiated in `crates/brassclaw_reborn_composition/src/runtime.rs`:**
> Verified by grepping all files in the composition crate: `ThreadManager` does NOT appear
> in `runtime.rs` or anywhere under `crates/brassclaw_reborn_composition/src/`. The engine's
> `ThreadManager` is instantiated in `crates/brassclaw_engine/src/runtime/mission.rs` and
> `conversation.rs`. The composition layer wraps `ConversationManager` (which internally
> creates `ThreadManager` instances). Therefore Phase E.0's injection point is NOT
> `crates/brassclaw_reborn_composition/src/runtime.rs` — it must be wherever
> `ThreadManager::new()` is called and the composition layer has access to the `pg_pool`.
> **Correct injection points to investigate before implementing:**
> 1. `crates/brassclaw_engine/src/runtime/mission.rs` — contains `ThreadManager` instantiation;
>    verify whether composition passes a `pg_pool` down to this layer.
> 2. `crates/brassclaw_engine/src/runtime/conversation.rs` — same question.
> 3. Alternatively: thread the `pg_pool` through from the composition layer's `RebornRuntime`
>    (or whichever composition-layer service owns the `PgPool`) down through the engine
>    initialization path to the `ThreadManager` construction site.
> The Phase E.0 implementer **must** trace the actual `ThreadManager::new()` call site in
> the composition + engine boundary, confirm where a `PgPool` is available, and wire
> `PostgresSource` there. The plan's assumption that the injection site is
> `crates/brassclaw_reborn_composition/src/runtime.rs` is **wrong** and must not be
> followed literally.

- `crates/brassclaw_engine/src/runtime/manager.rs` — `ThreadManager::new` (line 64) does
  NOT take a `pg_pool`, and lines 382–383 build `RamSource` internally from `self.store`.

  > **⚠️ FIND-02 FIX — composition does NOT hold `ThreadManager` directly:** Verified by
  > grepping the composition crate: `ThreadManager` is never constructed in
  > `brassclaw_reborn_composition`. It is constructed inside `brassclaw_engine`'s
  > `MissionManager` (`mission.rs:3916`) and `ConversationManager` (`conversation.rs:57`).
  > The composition layer holds these engine managers, not `ThreadManager` directly.
  > Therefore the injection chain must be:
  > `ThreadManager` ← `MissionManager` ← `ConversationManager` ← composition factory.
  >
  > **Recommended approach — inject `pg_pool` not `Arc<dyn RetrievalSource>` (FIND-14):**
  > Instead of a `retrieval_source_override: Option<Arc<dyn RetrievalSource>>` field,
  > add `pg_pool: Option<Arc<brassclaw_pg::PgPool>>` to `ThreadManager`. In the spawn path,
  > build `PostgresSource::new(pool.clone())` behind `#[cfg(feature = "skills-db")]` when
  > the pool is present; otherwise fall back to `RamSource`. This avoids the trait-object
  > wrap/unwrap cycle and keeps `PostgresSource` construction inside the engine where it belongs.
  >
  > **However**, if `Arc<dyn RetrievalSource>` override is preferred for testability, it is
  > also valid — just thread it through `MissionManager`/`ConversationManager` as well.

  **Concrete changes required:**

  Option A (pool injection — recommended):
  1. Add `pg_pool: Option<Arc<brassclaw_pg::PgPool>>` to `ThreadManager` (line ~34) +
     `with_pg_pool(Arc<brassclaw_pg::PgPool>) -> Self` builder.
  2. In spawn path (lines 377–383): if `self.pg_pool.is_some()` →
     `Arc::new(crate::memory::PostgresSource::new(pool.clone()))` else `RamSource`.
  3. Add `with_pg_pool` pass-through on `MissionManager` and `ConversationManager`.
  4. In composition factory (`factory.rs`): after constructing the engine runtime,
     call `.with_pg_pool(pg_pool.clone())` before the manager is used. The `pg_pool` is
     already available in the factory (it holds the Postgres pool for all DB operations).

  Option B (RetrievalSource override — if testability requires it):
  1. Add `retrieval_source_override: Option<Arc<dyn crate::memory::RetrievalSource>>` to
     `ThreadManager` (line ~34) + builder.
  2. In spawn path: `self.retrieval_source_override.clone().unwrap_or_else(|| RamSource...)`.
  3. Add pass-through on `MissionManager` and `ConversationManager`.
  4. Composition factory constructs `Arc::new(PostgresSource::new(pg_pool.clone()))` and
     passes it via the builder chain.

  > **⚠️ FIND-22 — `factory.rs` NEVER directly instantiates `ThreadManager`, `MissionManager`,
  > or `ConversationManager`:** Verified by grepping `factory.rs` — ZERO matches for those types.
  > The factory produces a `RebornServices` struct via `build_local_dev` and related functions.
  > The engine runtime is built inside `RebornServices` (the `host_runtime` field is populated via
  > `services.host_runtime_for_local_testing()` at line ~1040). `ThreadManager` is constructed
  > deep inside the engine, not in the composition layer.
  >
  > **Correct injection approach:**
  > - The `pg_pool` IS available in `factory.rs` (confirmed: `services.pg_pool` at line 257,
  >   set to `Some(Arc::clone(&pg_pool_arc))` at line 577).
  > - The injection must go through the **engine's builder API** — whatever method builds the
  >   engine runtime that eventually creates `ThreadManager` instances. Option A requires
  >   adding a `with_pg_pool` method to the engine runtime builder that threads down to
  >   `MissionManager::new` → `ConversationManager::new` → `ThreadManager::new`.
  > - The implementer must trace `services.host_runtime_for_local_testing()` (or equivalent
  >   `build_reborn_runtime` path in `factory.rs:542`) to find where `ThreadManager::new` is
  >   called, confirm the engine builder API exists or must be added, and wire the pool there.
  > - Step 4 in Option A above ("`In composition factory (factory.rs): call .with_pg_pool(pg_pool.clone())`")
  >   is the correct final wiring location — but the call will be on the **engine runtime builder
  >   object** found at that trace site, not on a `ThreadManager` directly visible in `factory.rs`.
  > - Do NOT assume there is a direct `ThreadManager` call in `factory.rs` — there is none.

  Either way: remove the `TODO(Phase K)` comment at `manager.rs:377` (satisfied by E.0).
  Keep `RamSource` importable (Phase K.3 deletes it).

  > **⚠️ FIND-P6-01 — `pg_pool` field and spawn-path conditional MUST be feature-gated:**
  > `brassclaw_pg` is an optional dependency (`skills-db = ["dep:brassclaw_pg", ...]`).
  > The new `ThreadManager` field MUST be `#[cfg(feature = "skills-db")] pg_pool: Option<Arc<brassclaw_pg::PgPool>>`.
  > The spawn-path conditional that builds `PostgresSource` MUST be inside `#[cfg(feature = "skills-db")]`.
  > The exact spawn-path change is:
  > ```rust
  > // manager.rs:375-383 — replace the store_for_retrieval line + unconditional RamSource with:
  > //
  > // ⚠️ FIND-NEW-11: store_for_retrieval is MOVED into RamSource::new() at line 383 in the
  > // original code. The conditional below has TWO branches that both need it.
  > // Clone it BEFORE the conditional so both branches compile.
  > let store_for_retrieval = Arc::clone(&self.store);
  > let store_for_retrieval_clone = Arc::clone(&store_for_retrieval);  // for RamSource fallback
  > let retrieval = crate::memory::RetrievalEngine::new(store_for_retrieval);
  >
  > #[cfg(feature = "skills-db")]
  > let retrieval_source: Arc<dyn crate::memory::RetrievalSource> = if let Some(pool) = &self.pg_pool {
  >     Arc::new(crate::memory::PostgresSource::new(Arc::clone(pool)))
  > } else {
  >     Arc::new(crate::memory::RamSource::new(store_for_retrieval_clone))
  > };
  > #[cfg(not(feature = "skills-db"))]
  > let retrieval_source: Arc<dyn crate::memory::RetrievalSource> =
  >     Arc::new(crate::memory::RamSource::new(store_for_retrieval_clone));
  > ```
  > The `.with_retrieval_source(retrieval_source)` call at line 400 is **unchanged** — it
  > already exists. The only change is WHAT source is passed. Do NOT add a second
  > `.with_retrieval_source()` call.
  > Also note: `ExecutionLoop::with_pg_pool()` already exists at `loop_engine.rs:207` and
  > is `#[cfg(feature = "skills-db")]`. It is NOT needed here — the `retrieval_source` is
  > built from the pool and passed via `.with_retrieval_source()` directly.

- **Files to modify in the composition layer (FIND-02):**
  - `crates/brassclaw_engine/src/runtime/manager.rs` — add `#[cfg(feature = "skills-db")] pg_pool` field + builder.
    **⚠️ FIND-P5-02 — ALL 8 `ThreadManager::new` call sites must be updated:**
    Verified by grep: `ThreadManager::new` is called at:
    - `manager.rs:1132`, `manager.rs:1160`, `manager.rs:1192` — internal test-helper
      factory functions. Add `pg_pool: None` here (no pool available in test helpers).
    - `mission.rs:3916`, `mission.rs:4250` — `MissionManager` construction functions.
      Thread the optional pool through the `MissionManager` builder/constructor too.
    - `conversation.rs:856`, `conversation.rs:925`, `conversation.rs:1041` —
      `ConversationManager` construction functions. Same: thread optional pool through.
    Every call site must be updated atomically or the code will not compile
    (`ThreadManager::new` arity changes). Do not miss the 3 `manager.rs` internal sites.
  - `crates/brassclaw_engine/src/runtime/mission.rs` — pass pool/source through to `ThreadManager::new` (at lines 3916 and 4250)
  - `crates/brassclaw_engine/src/runtime/conversation.rs` — same (at lines 856, 925, 1041)
  - `crates/brassclaw_reborn_composition/src/factory.rs` — call the builder with `pg_pool` after constructing the engine runtime. The `pg_pool` is already available here (`services.pg_pool` at line ~257, set to `Some(Arc::clone(&pg_pool_arc))` at line ~577).

#### Acceptance (must be verified live, not just unit-tested)

- A real turn's `__assemble_prior_knowledge__` takes the `RetrievalSource` arm
  (`orchestrator.rs:2574` `if let Some(source) = retrieval_source`) and calls
  `PostgresSource::fetch_for_turn` (not the legacy `retrieve_context` fallback at
  `orchestrator.rs:2620–2637`).
- `resolve_intent` is exercised on live user text (log/trace confirms the intent path).
- Tier-2 keyword fallback still works when `resolve_intent` returns `NoMatch` (Phase E's
  `fetch_for_turn` falls back to `fetch_for_consumer` inside `PostgresSource`).

> **✅ Review note (pre-v3 audit) — the dominant cross-cutting hazard is `PostgresSource`
> wiring spanning E/F/G/H/K (RESOLVED by this E.0 step):** verified fact —
> `manager.rs:383` wires `RamSource`; `PostgresSource` is never constructed in composition;
> `orchestrator.rs:2552–2637` calls `fetch_for_turn` only when the source is `Some`, else
> falls through to the legacy `retrieve_context` (MemoryDoc) block at `2620–2637`, which is
> the **actual production retrieval path today**. Consequences reconciled by E.0:
> (C2) Phase E edits target a dormant backend → E.0 makes it live first.
> (C3) Phase F must not delete the `retrieve_context` fallback until `PostgresSource` is
> wired → E.0 wires it, so Phase F's fallback-preservation note (below) holds and removal is
> deferred to Phase K. (C4) Phase K.3 must not delete `RamSource` before wiring → E.0
> wires first, so K.3 is split into wire-verify-then-delete (K.3 already-satisfied by E.0;
> K.3 becomes pure deletion). This E.0 step is the cleanest resolution of C2/C3/C4 and the
> transitive H4 dependency.

#### Tests

- Integration: boot the runtime with a `pg_pool`, assert `with_retrieval_source` was
  called with a `PostgresSource` (not `RamSource`) — e.g. a retrieval-source-type probe.
- Integration: a live turn with an intent-input row present takes the
  `PostgresSource::fetch_for_turn` path (trace/log assertion).
- Integration: a turn with no intent match still returns components via the
  `PostgresSource` keyword fallback (Tier 2 preserved).

---

### Phase E — `fetch_for_turn` Upgrade + `FetchForTurnResult::SplitResult`

**Status:** [ ] Pending

> **🔗 SUBSTEP — Component-class registry (FIND-IBS-02 gap):** the IBS emits
> `IbsRecipeStep.include: Vec<Uuid>` per step with **no per-UUID `class_code`**
> (FIND-IBS-02: "UUIDs are opaque to the IBS"), but `fetch_component_by_id`
> needs `class_code` to pick the class table, and no per-UUID class source
> exists in the data model (`reborn_intent_inputs` holds intent-matched
> components, not recipe-step includes). Per user decision (Q1→C then Q-F1→B),
> resolve each step UUID's class via a **real `reborn_components(id,
> class_code, scope)` registry table** kept in sync by triggers on all 14 class
> tables — new migration **V061** (a schema change beyond Phase E's original
> "no migration"; explicitly accepted as an upgrade). Full approach, the 7 user
> design decisions governing all of Phase E, the V061 migration design, and the
> ordered E.1–E.6 substeps: **`docs/agents-v3/subplan_problem_stepE_of_saved_plan_to_v3.md`**.
> Run that subplan's steps (E.1 registry first) before the remainder of this
> Phase E section. Note: the E0-A re-target means Phase E MUST also grow the
> composition `PgRetrievalLookup` bridge with `SplitResult` + `ActionShortCircuit`
> arms (the engine enum grows → the bridge `match` would be non-exhaustive) —
> replacing E.0's conservative booleans/unsplit items with real routing booleans
> and split channels.
>
> **🔗 SUB-SUBSTEP — embedded-PG pgvector build fix (encountered during E.1
> runtime verification):** the embedded-PG boot test failed on V000 `CREATE
> EXTENSION vector` because `build.rs::try_build_pgvector` failed on macOS
> arm64 (stale CI toolchain tokens baked in `Makefile.global` — `-isysroot
> MacOSX14.5.sdk`, brew ICU `-I`, `-Werror=unguarded-availability-new` — plus
> cargo's `PROFILE=debug` injected as a bare `debug` token via `ifdef PROFILE`)
> and wrote an empty `EMBEDDED_PGVECTOR_ARCHIVE` placeholder, so V000 aborts
> and V061 is never reached. This is a pre-existing infrastructure gap, not an
> E.1 bug. User decision (ask_user): do BOTH — fix compile-from-source
> (primary, `build.rs` sanitise `Makefile.global` + `.env_remove("PROFILE")`)
> AND add a runtime prebuilt-fetch fallback in `install_pgvector`
> (system/brew PG-16 → `BRASSCLAW_PGVECTOR_URL` → warn+Ok). Full approach +
> ordered P1/P2/P3 steps: **`docs/agents-v3/subplan_problem_stepE1_pgvector_of_saved_plan_to_v3.md`**.
> Run that subplan BEFORE completing E.1's runtime verification (V000–V061
> must apply via the embedded-PG boot test).

**Goal:** Wire the IBS into `PostgresSource::fetch_for_turn`. On a Recipe intent match
with a `step_link`, call the IBS, fetch component items for each channel, and return a
`SplitResult`. Handle Action match with `ActionShortCircuit`.

#### Files to modify

- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  - Extend `FetchForTurnResult` with `ActionShortCircuit` and `SplitResult` variants (§0.8).
  - Extend `TurnRoutingSignals` struct.
  - Update `PostgresSource::fetch_for_turn`:
    > **⚠️ FIND-P5-06 — class-16 detection must happen BEFORE `fetch_component_by_id`.** The
    > current code at `retrieval_source.rs:516-525` calls `fetch_component_by_id(pool, scope,
    > component_id, component_class_code)` for ALL match class codes, then returns
    > `Components([item])`. Phase E must restructure the dispatch to detect `class_code == 16`
    > IMMEDIATELY after `resolve_intent` returns, before ANY `fetch_component_by_id` call, and
    > return `ActionShortCircuit { component_id, name }` directly. The `name` for the
    > `ActionShortCircuit` must be fetched from the `reborn_actions` table (a separate
    > single-column `SELECT name FROM reborn_actions WHERE id = $1` query), NOT by calling
    > `fetch_component_by_id` (which returns the full component). Or alternatively, `resolve_intent`
    > could be extended to return `name` alongside `component_id` for class-16 matches.
    > The cleanest approach: add a `name: String` to `IntentResolution::Match` so the name
    > is available from the intent-resolution result itself (it is already in `reborn_intent_inputs`
    > via the component's `name` field — no separate fetch needed). Alternatively fetch it with
    > a lightweight targeted query. **Do NOT call `fetch_component_by_id` for a class-16 match
    > and then discard the result** — that is an unnecessary DB round-trip.
    1. After `resolve_intent` → `Match { class_code: 16 }`: return `ActionShortCircuit`.
    2. After `resolve_intent` → `Match { class_code: 21, step_link: Some(...) }`:
       - Fetch Recipe row's `step_descriptions` JSONB **and** `variants` JSONB columns.
       - **⚠️ FIND-P6-06 — Deserialize `variants` to find `variable_patterns` for the matched `step_link`:**
         `variable_patterns` are NOT a top-level column — they live inside each `RecipeVariant`
         in the `variants` JSONB. Deserialize `variants` → `Vec<RecipeVariant>` → find the variant
         whose `step_link` matches the `step_link` from `IntentResolution::Match` → extract
         `variable_patterns: Vec<VariablePattern>`. If no matching variant found, use an empty
         `vec![]` for `variable_patterns` (legacy recipe with no variants defined).
         This deserialization step MUST be in `fetch_for_turn`, NOT inside the IBS
         (the IBS is pure and has no DB access).
       - Call `instruction_builder::build_instruction(step_link, step_descriptions, variable_patterns)`.
       - Apply `{{vars.name}}` substitution using captures from `user_text`.
       - For each UUID in `rust_steps`: call `fetch_component_by_id` → `rust_items`.
       - For each UUID in `orchestrator_steps`: call `fetch_component_by_id` → `orchestrator_items`.
       - Return `SplitResult { rust_items, orchestrator_items, routing }`.
    3. After `resolve_intent` → `Match { step_link: None }`: existing `fetch_component_by_id` path (unchanged).
  - **Extend `fetch_component_by_id` match arm for new classes 22 and 23** (added in Phases B and C):
    the current `match component_class_code` in `retrieval_source.rs` has no arm for 22 or 23 —
    those class codes currently return `None` (empty vec). Phase E adds:
    ```rust
    22 => Some(("reborn_python_code",    "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    23 => Some(("reborn_extension_catalogues", "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)")),
    ```
    This is required before any Recipe step can reference a class 22 or 23 component UUID.
    > **Security note:** `fetch_component_by_id` uses `format!()` to interpolate the table
    > name and content expression into the SQL query string. This is safe **only because**
    > both values come from a `match` on `component_class_code` and are hard-coded `&'static str`
    > literals — never from user input. This pattern must NEVER be extended to accept
    > user-supplied table names or column expressions. The class code itself is an `i32`
    > from the DB, not from user input, so the dispatch is safe. Document this constraint
    > in a code comment above the match arm when implementing.

  - **⚠️ PERF-02 (Phase E implementation):** Replace the per-UUID `fetch_component_by_id`
    loop with a batched `fetch_components_by_ids` helper. Group the IBS output UUIDs by
    `(table, content_expr)` pair (the same grouping used by the match arm), then issue one
    `WHERE id = ANY($uuids) AND tenant_id = $1 … AND validation_status = 'validated'` query
    per group. This reduces O(N) round-trips to at most O(tables) — in practice 1–2 for most
    recipes. Re-use the same scope params and the same `format!()` pattern (same security
    invariant: literals only). **This is a Phase E requirement, not a future optimisation.**

    > **⚠️ FIND-P9-04 — `fetch_components_by_ids` is never specified. Full spec below.**

    ```rust
    /// Batch-fetches multiple components in O(tables) round-trips instead of O(N).
    /// Groups `ids_by_class` by (table, content_expr) using the same class→table match
    /// as `fetch_component_by_id`. Emits one `WHERE id = ANY($1) AND scope...` query per group.
    ///
    /// SECURITY: `table_name` and `content_expr` are ALWAYS `&'static str` literals from the
    /// class-code match arm — never user input. This is the same invariant as
    /// `fetch_component_by_id`. NEVER extend this function to accept user-supplied table names.
    ///
    /// ⚠️ FIND-NEW-10: Use tokio-postgres directly (pool.get() + client.query()).
    /// This codebase does NOT use sqlx — PgPool is brassclaw_pg::PgPool backed by
    /// deadpool-postgres / tokio-postgres. Do NOT use sqlx::query(), .bind(), ComponentItem::from_row().
    /// Build ComponentItems field-by-field using row.get(N) exactly as fetch_component_by_id does.
    #[cfg(feature = "skills-db")]
    async fn fetch_components_by_ids(
        pool:        &brassclaw_pg::PgPool,
        scope:       &ComponentScope,
        ids_by_class: &[(uuid::Uuid, i32)],  // (component_id, class_code) pairs
    ) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
        use tokio_postgres::types::ToSql;

        // 1. Group by (table_name, content_expr) using the same match arm as fetch_component_by_id.
        //    Unknown class codes are silently skipped (same behaviour as fetch_component_by_id
        //    returning None — let the caller handle missing items via the returned Vec length).
        let mut groups: std::collections::HashMap<(&'static str, &'static str), Vec<uuid::Uuid>>
            = std::collections::HashMap::new();
        for (id, class_code) in ids_by_class {
            if let Some((table, content_expr)) = class_code_to_table(*class_code) {
                groups.entry((table, content_expr)).or_default().push(*id);
            }
        }

        // 2. For each group: one SELECT with id = ANY($1) and all scope + validation filters.
        //    The WHERE clause replicates fetch_component_by_id's safety guarantees exactly:
        //    validation_status='validated' AND '05:validator' != ALL(consumer_tags).
        let client = pool
            .get()
            .await
            .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

        let mut results = Vec::new();
        for ((table, content_expr), ids) in &groups {
            let sql = format!(
                "SELECT id::text, class_code::int, prompt_uid::bigint,
                        name, COALESCE(description,'') AS description,
                        {content_expr} AS effective_content,
                        override_prompt_creation
                 FROM {table}
                 WHERE id = ANY($1)
                   AND tenant_id=$2 AND user_id=$3 AND agent_id=$4 AND project_id=$5
                   AND validation_status='validated'
                   AND '05:validator' != ALL(consumer_tags)"
            );
            let params: &[&(dyn ToSql + Sync)] = &[
                ids,  // Vec<Uuid> implements ToSql for = ANY($1) in tokio-postgres
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ];
            let rows = client
                .query(&sql, params)
                .await
                .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
            for row in rows {
                let id_str: &str = row.get(0);
                let id = id_str.parse::<uuid::Uuid>().unwrap_or_else(|_| uuid::Uuid::nil());
                results.push(ComponentItem {
                    id,
                    class_code: row.get(1),
                    prompt_uid: row.get(2),
                    name: row.get::<_, &str>(3).to_string(),
                    description: row.get::<_, &str>(4).to_string(),
                    effective_content: row.get::<_, &str>(5).to_string(),
                    override_prompt_creation: row.get(6),
                });
            }
        }
        Ok(results)
    }
    ```

    Extract the `class_code_to_table(code: i32) -> Option<(&'static str, &'static str)>` helper
    from the existing match arm in `fetch_component_by_id` so both functions share the same
    mapping — no duplication of the literal table/column mapping.

    > **⚠️ FIND-NEW-AUDIT-06 (CRITICAL) + MISSING-ARM GUARD — `class_code_to_table` MUST include `10 | 50` arm:**
    >
    > **Verified against live `fetch_component_by_id` (lines 573–627):** the existing function
    > contains `10 | 50 => Some(("reborn_skills", "COALESCE(NULLIF(prior_knowledge_content,''), body)"))`.
    > Classes 10 (Orchestrator) and 50 (Scaffold) are served from `reborn_skills`. An earlier
    > draft of the `class_code_to_table` helper omitted these two arms — that would be a **silent
    > regression**: after Phase E extracts the helper, classes 10 and 50 would return `None` where
    > they previously returned results. The correct helper body MUST include the `10 | 50` arm,
    > exactly as the live `fetch_component_by_id` does. Do NOT extract this helper without
    > copying the `10 | 50` arm verbatim from the source function.
    >
    > **Add this comment immediately above the wildcard arm in `class_code_to_table`:**
    > ```rust
    > fn class_code_to_table(code: i32) -> Option<(&'static str, &'static str)> {
    >     match code {
    >         0     => None, // Tool — no prompt text in component table
    >         1..=3 => Some(("reborn_skills",              "COALESCE(NULLIF(prior_knowledge_content,''), body)")),
    >         4..=9 => Some(("reborn_extensions_unified",  "COALESCE(prior_knowledge_content, description)")),
    >         // ⚠️ FIND-NEW-AUDIT-06: classes 10 (Orchestrator) and 50 (Scaffold) map to
    >         // reborn_skills — confirmed from live fetch_component_by_id (line 582-585).
    >         // MUST be present or Phase E silently loses retrieval for these classes.
    >         10 | 50 => Some(("reborn_skills",            "COALESCE(NULLIF(prior_knowledge_content,''), body)")),
    >         12 => Some(("reborn_specs",                   "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         13 => Some(("reborn_tool_skills",             "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         14 => Some(("reborn_plans",                   "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         15 => Some(("reborn_summaries",               "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         16 => Some(("reborn_actions",                 "COALESCE(prior_knowledge_content, description)")),
    >         17 => Some(("reborn_docus",                   "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         18 => Some(("reborn_lessons",                 "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         19 => Some(("reborn_issues",                  "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         20 => Some(("reborn_notes",                   "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         21 => Some(("reborn_recipes",                 "COALESCE(NULLIF(prior_knowledge_content,''), '')")),
    >         22 => Some(("reborn_python_code",             "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
    >         23 => Some(("reborn_extension_catalogues",    "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)")),
    >         // ⚠️ WHEN ADDING A NEW CLASS CODE: ADD A MATCH ARM HERE.
    >         // A missing arm silently returns None → fetch_for_turn produces an empty
    >         // SplitResult item list → the recipe executes without the component.
    >         // There is NO compile-time enforcement. Always add the arm AND a test.
    >         _ => None,
    >     }
    > }
    > ```
    > The full body is shown for clarity. The `⚠️ FIND-NEW-AUDIT-06` arm and the `⚠️` comment above
    > `_ => None` are both **mandatory** — do not omit either. When extracting the helper from the
    > existing `fetch_component_by_id`, verify every arm matches the source function exactly.

  > **⚠️ FIND-08 correction — `FetchForTurnResult`, `TurnRoutingSignals`, `ActionShortCircuit`, and
  > `SplitResult` are NEW types that do NOT yet exist:** Verified `retrieval_source.rs:90–96` —
  > `FetchForTurnResult` currently has exactly TWO variants: `Components` and `Disambiguation`.
  > Phase E CREATES all the new variants and `TurnRoutingSignals` from scratch. No existing type
  > needs to be "extended" — these are greenfield additions.

  - **⚠️ PERF-03 — UNION ALL growth with classes 22 and 23:**
    **Verified:** `fetch_for_consumer` currently has **12 sub-selects** (not 9 as previously
    stated): reborn_skills, reborn_extensions_unified, reborn_actions, reborn_specs,
    reborn_tool_skills, reborn_plans, reborn_summaries, reborn_docus, reborn_lessons,
    reborn_issues, reborn_notes, reborn_recipes. Adding classes 22 and 23 raises it to **14**.
    Each sub-select requires an index scan on a scope + consumer_tag filtered index.
    Verify that `reborn_python_code` and `reborn_extension_catalogues` have composite indexes
    on `(tenant_id, user_id, agent_id, project_id, validation_status)` and that
    `consumer_tags` has a GIN index. Without these, the two new arms degrade from index-scan to
    seq-scan on every `fetch_for_consumer` call. Document the required indexes in the Phase B/C
    migration files alongside the table CREATE.

> **✅ Review note (pre-v3 audit) — Phase E code is correct but unreachable in production until
> `PostgresSource` is wired — RESOLVED by the new Phase E.0 above:** E.0 wires
> `PostgresSource` as the live backend before Phase E, so the `SplitResult`/`ActionShortCircuit`
> variants are reachable at deploy time. Original audit detail retained below:
> All edits here target `PostgresSource::fetch_for_turn`, which is
> not the active retrieval backend (`manager.rs:383` wires `RamSource`; see §0.3 review note).
> The `SplitResult`/`ActionShortCircuit` variants and the IBS call are therefore dormant at
> deploy time — the unit/integration tests above will pass, but no live turn takes this path
> until the composition layer calls `with_retrieval_source(PostgresSource)`. Treat
> `PostgresSource` wiring as a Phase E/H prerequisite, not a Phase K item (Phase K only *removes*
> `RamSource`; it must not be the first phase that wires `PostgresSource`, or turns lose retrieval
> entirely between the `RamSource` removal and the wiring). Cross-ref `Goals_pre_v3_review.md`
> Step 14 ordering constraint.

#### Tests

- Unit: Recipe match with `step_link` → `SplitResult`; `rust_items` contain only ToolSkills; `orchestrator_items` contain only Skills and PythonCode
- Unit: `knowledge: both` step → UUID appears in both `rust_items` and `orchestrator_items`
- Unit: Action (class 16) match → `ActionShortCircuit { component_id, name }`
- Unit: match with `step_link: None` → existing `Components([single_item])` path unchanged
- Unit: `{{vars.dir}}` substitution applied in `orchestrator_items[].effective_content`
- Unit: `routing.wilson_lower` populated from Recipe row's `wilson_lower` field
- Integration: full intent match → correct channel split confirmed by asserting item class_codes

---

### Phase F — `handle_assemble_prior_knowledge` Upgrade (Rust handler)

**Status:** [ ] Pending

> **🔗 SUBSTEP — Phase F problem resolution (pre-E0-A-plan vs post-E0-A-codebase
> gap):** Phase F was written before the E0-A re-target. Grounding after E.0–E.6
> found four gaps: (1) the dormant engine `handle_assemble_prior_knowledge` already
> has all four `FetchForTurnResult` arms but its `SplitResult` arm ignores
> `rust_items`/`routing` and returns a plain dict (not the §0.9 routing dict); (2)
> `Thread` carries no `tenant_id`/`agent_id`, so the scope stubs at
> `orchestrator.rs:2586` + `:3177` use `thread.user_id`/`"default"` (cross-tenant
> leak now `PostgresSource` is wired); (3) the LIVE `build_component_scope`
> (`retrieval_lookup_impl.rs:131`) has the same `tenant_id = user_id` stub — but
> `LoopRunContext.scope` (`TurnScope`) already carries a real `tenant_id`; (4)
> `__fetch_component__` is not registered. Also: `brassclaw_engine` has no
> `brassclaw_turns` dep and the engine `Thread`/`spawn_*` API has zero
> composition/agent-loop callers, so the engine spawn path has no tenant/agent
> source (only the subagent child at `scripting.rs:1995` can inherit the parent's).
> The 5 user design decisions (Q-F1 dormant-handler §0.9 upgrade; Q-F2 fix BOTH
> live scope + `Thread` fields + both stubs; Q-F3 register `__fetch_component__`
> now; Q-F4 `RetrievalTurnResult.tier0_eligible` is the live signal / dormant
> `tier_zero` = `!llm_call_required`; Q-F5 engine spawn default-empty + subagent
> inherits + live via F.4) and the ordered F.1–F.7 substeps:
> **`docs/agents-v3/subplan_problem_stepF_of_saved_plan_to_v3.md`**.
> Run that subplan's steps (F.1 `Thread` fields first) before the remainder of
> this Phase F section.

> **🔗 SUBSTEP — Phase F.5 stub fix (orchestrator_content prose + `formatted_content`
> JSON→prose, FINDING F):** discovered while grounding Phase F tests (below). F.5
> (commit `46d64d31`) emitted `orchestrator_content` / `formatted_content` as the
> **JSON string** from `assemble_component_strings` (`{"prior_knowledge":[...],
> "matched_components":[...]}`), but plan §0.9 (line 780–786) mandates a **prose
> StepContextSpec-headed block** (`## [Skill: name]\n<body>`) and FINDING F (line
> 1334–1350) mandates `formatted_content` transition **JSON object → prose string =
> `orchestrator_content`** (a breaking shape change; built-in `default.py` is
> unaffected — it uses `formatted_content` as a string). Also plan test #4 (line
> 5216) requires the `Components` (no-match) arm to emit `orchestrator_content`
> containing **all** retrieved items, which F.5 did not touch. User design
> decisions (Q-F7-1 → ALL retrieved classes in the `Components` arm; Q-F7-2 →
> Capitalized category labels via a new `step_context_label` helper;
> class-13 ToolSkill always skipped — plan-specified) and the ordered
> SF5.1–SF5.5 substeps:
> **`docs/agents-v3/subplan_stub_stepF5_saved_plan_to_v3.md`**.
> Run that subplan's steps (SF5.1 the `StepContextSpec` enum + prose formatter
> first) before resuming F.7 (the Phase F tests).

**Goal:** Upgrade the Rust handler behind `__assemble_prior_knowledge__` to handle all
four `FetchForTurnResult` variants — including the new `SplitResult` and
`ActionShortCircuit` variants added in Phase E. Register `__fetch_component__`.
Fix the hardcoded `tenant_id: "default"` scope bug (see §below).

> **Clarification — which handler is upgraded:**
> `handle_retrieve_docs` calls `RetrievalEngine::retrieve_context` (legacy MemoryDoc path).
> It is **not** upgraded — it is removed unconditionally in Phase K (no compatibility window).
> `handle_assemble_prior_knowledge` already calls `fetch_for_turn` via the wired
> `RetrievalSource` trait object (`RamSource` today; `PostgresSource` after Phase E.0 wires
> it — see the C2/C3/C4 resolution). This is the handler that v3 extends to handle
> `SplitResult` and `ActionShortCircuit`. **Preserve the legacy `retrieve_context` fallback
> (`orchestrator.rs:2620–2637`) unchanged in Phase F** — it is the actual production
> retrieval path until E.0 wires `PostgresSource`, and remains a safety net for the
> `None`/error case afterwards. Its removal is a Phase K.3 action (see C4 resolution).

#### Files to modify

- `crates/brassclaw_engine/src/executor/orchestrator.rs`

  **`handle_assemble_prior_knowledge`:**  
  The existing handler already calls `retrieval_source.fetch_for_turn()` and handles
  `Components` and `Disambiguation`. Extend it to handle the two new variants:
  - `SplitResult`: format `orchestrator_items` into `orchestrator_content` (channel O);
    set `formatted_content = orchestrator_content` (backward compat alias);
    populate `action_short_circuit: false`, `disambiguation: false`;
    include `rust_items` serialized in the return dict under `"rust_items"` (for the
    caller — see note below);
    return extended routing dict (§0.9 shape).
    > **Important — rust_items delivery:** `handle_assemble_prior_knowledge` runs inside
    > the Python scripting engine and has NO access to the Rust execution context (the
    > tool-dispatch layer managed at `RecipeStage` level). The handler CANNOT "apply"
    > rust_items directly. Instead, `RecipeStage` (Phase H) calls `fetch_for_turn` BEFORE
    > the Python script starts. For Tier 1 (where Python does run), `RecipeStage` stores the
    > rust_items in the loop state and applies them to the execution context during that
    > pre-Python pass. When Python later calls `__assemble_prior_knowledge__`, the handler
    > returns the stashed orchestrator_content. The `"rust_items"` field in the dict is
    > informational only — the Python side never calls Rust tools directly. Do NOT add
    > rust_items application logic inside `handle_assemble_prior_knowledge`.
  - `ActionShortCircuit`: return `{ action_short_circuit: true, action_component_id, action_name,
    orchestrator_content: "", formatted_content: "", override_prompt_creation: false,
    matched_component_ids: [] }`.
  - `Components` (no-match UNION ALL): all items → `orchestrator_content` **and**
    `formatted_content` (both set) — existing behaviour, unchanged shape.
  - `Disambiguation`: existing behaviour. Return `{ disambiguation: true, candidates }`.

  Return value is always a dict. The Python side already guards `isinstance(pkr, dict)`
  from the existing `__assemble_prior_knowledge__` usage.

  **`handle_retrieve_docs` — no change in Phase F.** Phase K removes both the
  registration and the function body. Do not add logic to this handler.

  **Register `__fetch_component__(uuid: str, class_code: int)`:**  
  New host function. Handler calls `fetch_component_by_id(uuid, class_code)` directly.
  Returns a single item dict or `None`. Used by `call_action` nested lookups (§0.9).

#### Phase F security fix — hardcoded `tenant_id: "default"` in scope

> **Bug found (orchestrator.rs lines 2586–2590):** `handle_assemble_prior_knowledge`
> constructs `ComponentScope` with stubs (live code verified by Pass 7):
> ```rust
> ComponentScope {
>     tenant_id: thread.user_id.clone(), // ← STUB — user_id used as tenant_id (Phase 1)
>     user_id: thread.user_id.clone(),
>     agent_id: "default".to_string(),   // ← STUB — fixed string, not the real agent_id
>     project_id: thread.project_id.to_string(),
> }
> ```
> This means all intent lookups ignore the real tenant_id and agent_id. In a
> multi-tenant deployment, User A could match intents seeded by User B's tenant.
> Phase F MUST fix this: the scope must be constructed from the actual thread's
> tenant, agent, and project identities. The `Thread` struct must carry `tenant_id`
> and `agent_id` (verify if they already exist; if not, they must be added).
> This is a **correctness and isolation bug** — fix it as part of Phase F, not deferred.
>
> **⚠️ FIND-P7-03 — LIVE CODE CONFIRMED: scope fix is a REAL Phase F task, NOT pre-v3 done.**
> Reading `orchestrator.rs:2586-2590` in the live codebase:
> ```rust
> let scope = ComponentScope {
>     tenant_id: thread.user_id.clone(), // tenant_id stub (Phase 1)
>     user_id: thread.user_id.clone(),
>     agent_id: "default".to_string(),   // agent_id stub (Phase 1)
>     project_id: thread.project_id.to_string(),
> };
> ```
> The "pre-v3 code fix" referenced above only changed `tenant_id` from the literal `"default"`
> to `thread.user_id.clone()` — still wrong for multi-tenant (the user_id is not the tenant_id).
> `agent_id` is still `"default"`. Both are still stubs with a code comment saying Phase F
> will fix them. The `RamSource` ignores these so the stub is behavior-preserving today, but
> once Phase E.0 wires `PostgresSource` these values drive the real scope filter and will
> cause cross-user intent leakage if not corrected. Phase F MUST fix this.
>
> **Phase F implementation requirement:** Before implementing, read `Thread` struct in
> `crates/brassclaw_engine/src/types/thread.rs` to confirm whether `tenant_id`/`agent_id`
> fields exist. If they do not exist (likely — the pre-v3 audit noted they were absent):
> option (a) add them to `Thread` with `#[serde(default)]` for checkpoint compatibility, or
> (b) source them from the engine execution context. Whichever path is chosen must provide
> real, per-user values — NOT `user_id` as tenant_id, NOT `"default"` as agent_id.
>
> **⚠️ FIND-P8-01 — `Thread` struct confirmed to have NO `tenant_id` and NO `agent_id`.**
> Full read of `crates/brassclaw_engine/src/types/thread.rs:212–245` (Pass 8):
> The `Thread` struct has `user_id: String`, `project_id: ProjectId`, and 18 other fields —
> but **NO `tenant_id` field and NO `agent_id` field**. The code comment at
> `orchestrator.rs:2575–2579` already says "Phase 2+ / v3 Phase F will tighten this once
> the full 4-tuple is threaded through `Thread`". This is the primary work of Phase F.
>
> **SECOND STUB identified (FIND-P8-01 addendum):** `orchestrator.rs:3142-3145` —
> the `__list_skills__` handler ALSO builds a scope with stub values via
> `scope_from_thread_ids(&thread.user_id, &thread.user_id, "default", ...)`.
> Phase F MUST fix BOTH stubs:
> - `handle_assemble_prior_knowledge` at `orchestrator.rs:2586–2590`
> - `__list_skills__` / `scope_from_thread_ids` at `orchestrator.rs:3142–3145`
>
> **Exact Phase F implementation (Option A — recommended, confirmed correct path):**
> 1. Add to `Thread` in `types/thread.rs`:
>    ```rust
>    /// Tenant identifier. Added v3 Phase F. `#[serde(default)]` = "" for legacy threads.
>    #[serde(default)]
>    pub tenant_id: String,
>    /// Agent context identifier. Added v3 Phase F.
>    #[serde(default)]
>    pub agent_id: String,
>    ```
> 2. Use a **builder method** `with_tenant_agent(tenant_id, agent_id) -> Self` rather
>    than adding params to `Thread::new` — builder avoids breaking every call site at once.
>    Callers that do not set it get empty strings (correct default for legacy/test threads).
> 3. **Grep all call sites first:** `grep -rn "Thread::new" crates/` — update every
>    construction site in the composition path (where real tenant/agent are known) to call
>    `.with_tenant_agent(tenant_id, agent_id)`.
> 4. After the fields exist, fix BOTH stubs to use `thread.tenant_id` and `thread.agent_id`:
>    ```rust
>    let scope = ComponentScope {
>        tenant_id:  thread.tenant_id.clone(), // real (Phase F)
>        user_id:    thread.user_id.clone(),
>        agent_id:   thread.agent_id.clone(),  // real (Phase F)
>        project_id: thread.project_id.to_string(),
>    };
>    ```
>
> **Review note (pre-v3 audit) — original detail retained for traceability:**
> The hardcoded scope is at `orchestrator.rs:2575–2580` (the plan cites "line 2581"; the
> `ComponentScope { … }` literal spans 2575–2580, with `tenant_id: "default"` at 2576 and
> `agent_id: String::new()` at 2578). More importantly, the `Thread` struct
> (`crates/brassclaw_engine/src/types/thread.rs:212`) carries `user_id` and `project_id` but
> **has no `tenant_id` field and no `agent_id` field**. So the handler cannot source the real
> tenant/agent from `thread` today — that is *why* the literals are hardcoded. Phase F must
> either (a) add `tenant_id` and `agent_id` to `Thread` (touching `Thread::new`, every thread
> creator, and checkpoint serde via `#[serde(default)]`), or (b) source them from the
> turn/loop context that reaches this handler (verify what identity is available on the
> engine execution context at call time). Option (a) is the correct path per FIND-P8-01.
> Option (b) is now moot: `thread.tenant_id` / `thread.agent_id` are added by this phase.
> Either way this is larger than "construct the scope from the thread" implies,
> and is scoped in Phase F.
>
> **✅ Review note (pre-v3 audit) — the legacy `retrieve_context` fallback must survive Phase F — RESOLVED:** E.0 wires `PostgresSource` first (so the fallback is no longer the *primary* production path after E.0), and the Phase F "Clarification" + body above now explicitly instruct "preserve the `retrieve_context` fallback (`orchestrator.rs:2620–2637`) unchanged in Phase F; removal is a Phase K.3 action." Phase K.3 (see the C4 resolution below) is updated to explicitly delete this block (plus `handle_retrieve_docs` +, if no remaining callers, `retrieve_context` itself). Original audit detail retained below:
> The plan's §0.9 / Q7 framing (line ~2298, ~3682) says `handle_assemble_prior_knowledge`
> "already calls `fetch_for_turn` via `PostgresSource`". That is **imprecise** and risks an
> implementer dropping the wrong code. Verified at `orchestrator.rs:2552-2637`: the handler
> takes `retrieval_source: Option<&Arc<dyn RetrievalSource>>` (line 2556) and calls
> `source.fetch_for_turn(...)` (line 2582-2584) ONLY `if let Some(source) = retrieval_source`
> (line 2574). In production that trait object is `RamSource` (or `None`), **not**
> `PostgresSource` — `PostgresSource` is not wired (see §0.3 / Phase E notes). When the
> source is `None` or `fetch_for_turn` errors, control **falls through** to the legacy
> block at `orchestrator.rs:2620-2637`: `retrieval.retrieve_context(project_id, user_id,
> goal, LEGACY_MAX_DOCS)` (the `RetrievalEngine` / MemoryDoc path). **That fallback is the
> actual production retrieval path today** — it is what serves `__assemble_prior_knowledge__`
> until `PostgresSource` is wired (Phase E/H prerequisite). Phase F's instruction to "extend
> the handler to handle `SplitResult` and `ActionShortCircuit`" is **silent on this
> fallback**. Hazard: an implementer who restructures the handler around the four
> `FetchForTurnResult` variants (which all presuppose a live `PostgresSource`) could delete
> or orphan the `retrieve_context` block — breaking all retrieval before `PostgresSource`
> is wired. **Phase F must preserve the `retrieve_context` fallback unchanged.** Its removal
> belongs in Phase K, alongside `handle_retrieve_docs`: Phase K (line ~3011) says "remove
> `RetrievalEngine::retrieve_context` if it has no other callers" — but this fallback IS a
> second caller (the first being `handle_retrieve_docs` at line 2509), so Phase K must
> explicitly delete the `orchestrator.rs:2620-2637` block, not just the `__retrieve_docs__`
> registration. Net: Phase F = add variants, keep fallback; Phase K (after `PostgresSource`
> wiring) = remove `handle_retrieve_docs` AND the `retrieve_context` fallback AND, if no
> remaining callers, `retrieve_context` itself.

#### Tests

- Unit: `SplitResult` → `orchestrator_content` contains Skill bodies and PythonCode bodies; does NOT contain ToolSkill bodies; does NOT contain `type:text` step info text
- Unit: `SplitResult` → `formatted_content` equals `orchestrator_content` (alias preserved)
- Unit: `ActionShortCircuit` → `action_short_circuit: true`, `orchestrator_content: ""`
- Unit: `Components` (no-match) → `orchestrator_content` contains all items (baseline preserved)
- Unit: `Disambiguation` → `disambiguation: true` with candidates list
- Unit: `handle_retrieve_docs` remains untouched — still returns flat `[{type, title, content}]` list
- Unit: `ComponentScope` in `handle_assemble_prior_knowledge` uses correct tenant_id and agent_id from thread (not hardcoded "default")
- Integration: `__fetch_component__(uuid, 16)` → correct Action item returned
- Integration: two-tenant setup → tenant A's intents do NOT match for tenant B's thread

---

### Phase G — Python Step-0 Upgrade + `call_action` Migration

**Status:** [ ] Pending

> **Subplan (resolved BEFORE implementation):** grounding after Phase A–F
> revealed 11 gaps/forks (e.g. `__resolve_component_by_name__` + `handle_disambiguation`
> + `_set_active_skills_from_matched_ids` do NOT exist; `ActiveSkillProvenance.version`
> is u32 but `DbSkillRow.version` is a String → latent silent-fail bug; Monty v0.0.16
> supports try/except+async but NOT custom classes → Q-G5 uses a result-dict marker
> not a custom exception; `run_loop` is not unit-tested today → G.8 extends the Monty
> harness). 8 design questions answered (Q-G1 `tier_zero`→Phase H defer; Q-G2
> implement `handle_disambiguation`; Q-G3 Rust emits `active_skills` in pkr via new
> `fetch_skill_provenance_by_ids`; Q-G4 add `__resolve_component_by_name__` host fn;
> Q-G5 fall back to Tier-2 via Monty-safe result-dict marker; Q-G6 V062 Flyway
> migration overrides plan's "not Flyway"; Q-G7/Q-G8 folded). Full approach +
> substep sequence (G.1–G.8) + verification in
> `./docs/agents-v3/subplan_problem_stepG_of_saved_plan_to_v3.md`
> (Zenflow substep `24e3d17b-b36d-4fe9-ae19-0d9ab7e02d8e`). Execute G.1→G.8 one-by-one.

**Goal:** Remove the dead step-0 shim calls from `default.py` so it makes a single
`__assemble_prior_knowledge__` call (which is already the primary call at line 997).
Migrate `call_action` nested lookup to `__fetch_component__`.

> **What the current code does (lines 994–1032):**  
> 1. `pkr = __assemble_prior_knowledge__(goal, token_budget, "02")` — PRIMARY call (works)  
> 2. `docs = __retrieve_docs__(goal, 5)` — dead Action-detection shim (broken: `class_code`
>    never in metadata, bug known, documented in §0.9 Problem 1)  
> 3. `all_skills = __list_skills__()` + `select_skills(...)` — unnecessary round-trip
>    (IBS already selected Skills by UUID; §0.9 Problem 2)  
>
> Phase G removes items 2 and 3. The primary `__assemble_prior_knowledge__` call (item 1)
> stays. After Phase F upgrades the handler, `pkr` already carries `action_short_circuit`,
> `disambiguation`, and `orchestrator_content` — no shim needed.

#### Files to modify

- `crates/brassclaw_engine/orchestrator/default.py`
  - Remove the `docs = __retrieve_docs__(goal, 5)` block (lines ~1018–1028): dead shim, never fires.
  - Remove `all_skills = __list_skills__()` and `select_skills()` calls (lines ~1031–1050).
  - Extend the `pkr` dict handling after `__assemble_prior_knowledge__` to check the new
    v3 fields: `action_short_circuit`, `disambiguation`, `orchestrator_content` (as described in §0.9).
  - Add `_set_active_skills_from_matched_ids(pkr.get("matched_component_ids", []), state)` helper.
  - Replace `call_action` `__retrieve_docs__(nested_name, 1)` at line ~844 with
    `__fetch_component__(action_uuid, 16)`.
    > **⚠️ FIND-P7-13 + FIND-P9-06 — `call_action` references actions BY NAME, not by UUID.
    > Option A is chosen (recommended). Full migration SQL and audit query are below.**
    >
    > - **Option A (recommended — chosen):** Require authors to add `action_id: UUID` to
    >   `call_action` step defs. At Phase G deploy, run the data migration SQL below.
    >   At runtime: `action_doc = __fetch_component__(step_def["action_id"], 16)`.
    >   Unresolvable names → `action_id` stays null; runtime falls back to Option B path.
    > - **Option B (stop-gap, fallback for null action_id):** If `action_id` is null at
    >   runtime (unresolved at migration time), call `__resolve_component_by_name__(name, 16)`.
    >   Both paths must be implemented — Option A failures degrade gracefully.
    >
    > **Migration SQL (run at Phase G deploy, NOT a Flyway migration — data-only):**
    > ```sql
    > -- Resolve call_action step names to UUIDs in-place.
    > -- Unresolvable names leave action_id absent/null → fallback to name lookup at runtime.
    > UPDATE reborn_actions a1
    > SET steps = (
    >     SELECT jsonb_agg(
    >         CASE
    >             WHEN step->>'type' = 'call_action'
    >              AND step->>'action' IS NOT NULL
    >              AND step->>'action_id' IS NULL
    >             THEN step || jsonb_build_object('action_id',
    >                 (SELECT a2.id::text
    >                  FROM reborn_actions a2
    >                  WHERE a2.name     = step->>'action'
    >                    AND a2.tenant_id  = a1.tenant_id
    >                    AND a2.user_id    = a1.user_id
    >                    AND a2.agent_id   = a1.agent_id
    >                    AND a2.project_id = a1.project_id
    >                  LIMIT 1)
    >             )
    >             ELSE step
    >         END
    >     )
    >     FROM jsonb_array_elements(a1.steps) AS step
    > )
    > WHERE a1.steps @> '[{"type":"call_action"}]'::jsonb;
    > ```
    > **Post-migration audit** (run immediately after — find still-unresolved steps):
    > ```sql
    > SELECT a.id, a.name, step->>'action' AS unresolved_action_name
    > FROM reborn_actions a, jsonb_array_elements(a.steps) AS step
    > WHERE step->>'type' = 'call_action'
    >   AND (step->>'action_id' IS NULL OR step->>'action_id' = 'null');
    > ```
    > Review output and fix or accept each row before deploying Phase G.
    > The original plan statement "UUID sourced from the BuildInstruction step" is WRONG for
    > `call_action` — these are Action steps (class 16 internal steps), not BuildInstruction steps.
  - `pkr["formatted_content"]` remains supported (backward compat alias) — code that checks
    it continues to work. New code uses `pkr["orchestrator_content"]`.
  > **⚠️ FIND-P7-02 — `execute_action_by_id` does NOT need to be a new function.**
  > `execute_action_procedure(action_doc, goal, state)` already exists at
  > `orchestrator/default.py:901` and executes an Action document deterministically
  > without an LLM call. The Phase G `action_short_circuit` branch should:
  >
  > ```python
  > action_doc = __fetch_component__(pkr["action_component_id"], 16)
  > return execute_action_procedure(action_doc, goal, state)
  > ```
  >
  > Do NOT create a new `execute_action_by_id` function. The existing
  > `execute_action_procedure` is the correct execution path. Phase G only needs:
  > (1) the `__fetch_component__` host function registered in Phase F, and
  > (2) the `action_short_circuit` branch in step-0 to call the existing procedure.
  > The §0.9 pseudocode reference to `execute_action_by_id` must be read as calling
  > `execute_action_procedure` with the fetched doc.

#### Tests

- Unit: step-0 with upgraded pkr → `orchestrator_content` injected; `__list_skills__` NOT called; `__retrieve_docs__` shim NOT called
- Unit: pkr has `action_short_circuit: true` → `execute_action_procedure` called via UUID fetch, no LLM
- Unit: pkr has `disambiguation: true` → `handle_disambiguation` called
- Unit: no-match path → UNION ALL `orchestrator_content` injected (baseline preserved)
- Integration: `call_action` using `__fetch_component__` → correct Action fetched by UUID

> **Substep G-STUB (inserted while grounding G.8) — `action_short_circuit` / `call_action`
> executable-steps stub (Q-G-STUB1).** ✅ Done — commit `07165137`. G.5/G.6 wired the
> `action_short_circuit` + `call_action` paths to fetch an Action via `__fetch_component__` /
> `__resolve_component_by_name__` and hand the doc to `execute_action_procedure`, which reads
> `action.get("steps")` + `action.get("allowed_tools")`. But those handlers returned a
> component *view* dict with NO `steps`/`allowed_tools` keys — for class 16 `content` is the
> LLM-readable description (`COALESCE(prior_knowledge_content, description)`), NOT the
> executable `steps` JSONB — so `_execute_action_steps` got `steps = []` and silently ran zero
> steps (and the G.6 `fall_back_to_tier2` signal was never produced). This was the
> "written half-way and silenced" stub the task calls out. Subplan:
> `docs/agents-v3/subplan_stub_stepG_action_steps_of_saved_plan_to_v3.md`. Fix (Option A,
> minimal — `steps` + `allowed_tools` only, no `timeout_secs`): `ComponentItem` gains
> `steps` + `allowed_tools` `Option<serde_json::Value>` (class-16 only); `fetch_component_
> by_id` / `fetch_component_by_name` gained a class-16 projection (`steps`, `allowed_tools`
> for class 16; `NULL::jsonb` / `NULL::text[]` otherwise → uniform 9-column shape) + a shared
> `component_item_from_row` helper; the two handlers emit `steps` + `allowed_tools` when
> `Some`; the prompt-assembly constructors (RamSource legacy, broad-scan `Components`,
> batched `fetch_components_by_ids`) deliberately stay `None` (they build
> `orchestrator_content`, not an executable doc). Verified: fmt/clippy clean both configs;
> engine lib 678 default / 689 skills-db (0 failed both); composition integration tests
> `fetch_component_by_id_returns_action_steps` + `fetch_component_by_name_returns_action_
> steps` added (skip-if-no-docker, compile-validated). No migration changed (G.7 V062 boot
> verification stands). Executed before G.8; resume G.8 next.

---

### Phase H — RecipeStage: `last_user_text` + Tier 0/1 Dispatch

**Status:** [ ] Pending

**Goal:** Activate the RecipeStage stub so it dispatches correctly for Tier 0, Tier 1,
and falls through to Tier 2 on no match.

#### H.0 Host-port prerequisites (resolves H3 + H4)

`RecipeStage` lives in `brassclaw_agent_loop`, which (like `brassclaw_turns`) does **not**
depend on `brassclaw_engine`. Two things `RecipeStage` needs are only reachable through
`brassclaw_engine` or host-side state, so they MUST be exposed as new `brassclaw_turns`
host ports — mirroring the existing `LoopRecipePort::recipe_lookup` → `&dyn RecipeLookup`
pattern (host.rs:2081–2093). Add both before the `RecipeStage` dispatch body:

> **⚠️ FIND-P9-03 — `RetrievalTurnResult`, `PriorKnowledgeBundle`, and `TierZeroReply` are
> referenced throughout Phase H but never defined. All three are `brassclaw_turns`-native types.
> Define them in `crates/brassclaw_turns/src/run_profile/host.rs` alongside the port traits.**
>
> ```rust
> // ─── brassclaw_turns-native types (crate boundary: no engine types inside) ──────────────
>
> /// Returned by LoopRetrievalPort::fetch_for_turn. Carries the routing signals and
> /// pre-serialized component arrays. Uses serde_json::Value throughout — ComponentItem
> /// (brassclaw_engine) must NOT appear here.
> pub struct RetrievalTurnResult {
>     /// True when the matched recipe is mature/candidate + wilson_lower >= 0.70
>     /// + validated + validation hook wired. Full Tier-0 eligibility check.
>     pub tier0_eligible:    bool,
>     /// True when the recipe declares llm_call_required = true.
>     /// Tier 0 requires both tier0_eligible == true AND llm_call_required == false.
>     pub llm_call_required: bool,
>     /// Serialized Vec<ComponentItem> for the rust channel (ToolSkills, PythonCode helpers).
>     /// Applied to the Rust execution context before Python starts.
>     pub rust_items:        serde_json::Value,
>     /// Serialized Vec<ComponentItem> for the orchestrator channel (Skills, PythonCode).
>     /// Stashed in state.recipe_hint; consumed by Python step-0 handler.
>     pub orchestrator_items: serde_json::Value,
>     /// Routing metadata (recipe name, matched component UUIDs, variant label, etc.)
>     /// for telemetry and stash/unstash disambiguation.
>     pub routing_meta:      serde_json::Value,
> }
>
> /// Returned by LoopOrchestratorPort::run_step_zero. Carries the formatted
> /// prior-knowledge bundle that PromptStage / build_prompt_bundle injects into
> /// the LLM prompt for Tier 1. Plain string + metadata — no engine types.
> pub struct PriorKnowledgeBundle {
>     /// The assembled orchestrator_content block (Skills + PythonCode bodies, formatted).
>     /// This is what PromptStage prepends to the LLM context window.
>     pub orchestrator_content: String,
>     /// The UUIDs of matched components, for telemetry and record_recipe_outcome.
>     pub matched_component_ids: Vec<String>,
>     /// When true, the composition host chose to replace the entire prompt with this
>     /// content (Solution Override path). Normally false.
>     pub override_prompt_creation: bool,
> }
>
> /// Returned by LoopOrchestratorPort::run_tier_zero. The Tier-0 reply text to emit
> /// directly to the user, with no LLM call.
> pub struct TierZeroReply {
>     /// The formatted output text to emit as the assistant reply.
>     pub text: String,
>     /// The UUIDs of matched components that produced this reply, for Wilson scoring.
>     pub matched_component_ids: Vec<String>,
> }
> ```
>
> **Visibility:** all three are `pub` structs in `brassclaw_turns`. `brassclaw_agent_loop`
> (which uses them in `RecipeStage`, `canonical.rs`) depends on `brassclaw_turns` and can
> import them. `brassclaw_reborn_composition` (which implements the host ports) also depends
> on `brassclaw_turns`.
>
> **Serde:** all three should `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`
> so they can be logged and passed across the host boundary without friction.

**H4 — retrieval port (the `fetch_for_turn` host boundary).** `RetrievalSource` /
`PostgresSource` / `FetchForTurnResult` / `ComponentItem` all live in `brassclaw_engine`
(`memory/retrieval_source.rs`); `RecipeStage` cannot import them.
- `crates/brassclaw_turns/src/run_profile/host.rs` — add a new opt-in port:
  ```rust
  /// Intent-driven retrieval port. Hosts that wire `PostgresSource` implement
  /// this; `RecipeStage` calls it for Tier 0/1 dispatch. Hosts without a
  /// retrieval source inherit `NoRetrieval` and `RecipeStage` falls through to
  /// Tier 2 (no short-circuit).
  pub trait LoopRetrievalPort: Send + Sync {
      async fn fetch_for_turn(
          &self,
          context: &LoopRunContext,
          query: &str,
          token_budget: usize,
          sender_class_code: &str,
      ) -> Option<RetrievalTurnResult>;
  }
  ```
  `RetrievalTurnResult` is a **`brassclaw_turns`-native** type (NOT `FetchForTurnResult`):
  it carries the two routing booleans (`tier0_eligible`, `llm_call_required`) plus the
  rust/orchestrator item arrays as `serde_json::Value` — the same crate-boundary
  discipline already used for `state.recipe_rust_context` / `state.recipe_hint` (see the
  constraint note in item 1 below). Add `LoopRetrievalPort` to the `AgentLoopDriverHost`
  supertrait list (host.rs:2185–2201 — **verified: current supertrait list has 13 ports:
  LoopRunInfoPort, LoopContextPort, LoopPromptPort, LoopInputPort, LoopModelPort,
  LoopCapabilityPort, LoopTranscriptPort, LoopCheckpointPort, LoopProgressPort,
  LoopCompactionPort, LoopCancellationPort, LoopRecipePort, LoopInterceptorPort**;
  add `LoopRetrievalPort` as the 14th entry in the `+ Sync` block after `LoopInterceptorPort`)
  and a `NoRetrieval` default impl returning `None`
  (mirror `NoRecipeLookup`).
  > **⚠️ FIND-NEW-PASS12-05 — the `AgentLoopDriverHost` blanket `impl` MUST also be updated:**
  > `host.rs:2204-2220` contains `impl<T> AgentLoopDriverHost for T where T: LoopRunInfoPort + ... + LoopInterceptorPort + Send + Sync {}`. When `LoopRetrievalPort` and `LoopOrchestratorPort` are added to the supertrait, the `where` clause of this blanket impl MUST also gain `+ LoopRetrievalPort + LoopOrchestratorPort`. Without this update the blanket impl no longer covers all `AgentLoopDriverHost` implementors (the compiler would not auto-derive the `AgentLoopDriverHost` impl for types that DO implement all 15 ports). Update BOTH: (1) the supertrait declaration at line 2185 and (2) the blanket `impl` where-clause at line 2204.
  `RecipeStage::process` calls
  `ctx.host.fetch_for_turn(context, user_text, budget, "02")` — NOT a direct
  `PostgresSource` import. (The §0.3 pseudocode `retrieval_source.fetch_for_turn` is
  satisfied through this port.) This is the exact method H4 asked the plan to specify.
- `crates/brassclaw_reborn_composition/src/...` — implement `LoopRetrievalPort` by
  delegating to the wired `PostgresSource::fetch_for_turn` and serializing the engine
  `ComponentItem`s / `TurnRoutingSignals` into `RetrievalTurnResult`. (Requires Phase E.0
  wiring — see the C2/C3/C4 resolution; `PostgresSource` must be live before this port
  returns non-`None`.)

**H3 — user-text resolution (the `message_ref → text` host boundary).** `LoopInput::UserMessage
{ message_ref }` (host.rs:844) holds an **opaque ref**, not text. `consume_drainable_inputs`
(input.rs:154–210) only advances `input_cursor`; it never extracts text. The agent loop
**never holds raw user text** today — `LoopContextMessage` carries `safe_summary`
(sanitized; host.rs:727–737) and `LoopPromptBundle` carries `content_ref` (still a ref;
host.rs:938–942, 1004–1017); the host resolves content host-side under scope/policy
(host.rs:1001–1003). So `state.last_user_text` cannot be populated from the `LoopInput`
value alone.
- Resolve `message_ref → text` via a **host fetch**. The raw text already exists
  host-side: the composition layer records it keyed by `(scope, accepted_message_ref)`
  in `SkillActivationMessage { text, .. }` / `messages_by_run`
  (`brassclaw_first_party_extension_ports/src/activation.rs:254–272`, written from
  `runtime.rs:1036`), and it is live for the whole turn until `clear_accepted_message`
  (runtime.rs:1062). Expose it via a new `LoopContextPort` method (host.rs:779):
  ```rust
  async fn resolve_message_text(
      &self,
      context: &LoopRunContext,
      message_ref: &LoopMessageRef,
  ) -> Result<String, AgentLoopHostError>;
  ```
  This must return the **raw** accepted-message body — NOT `safe_summary` (sanitized
  `safe_summary` redacts credential-like tokens to `[redacted]` (host.rs:1170–1198),
  which would corrupt intent matching for `resolve_intent`). The composition layer
  implements it against `messages_by_run`.
- `InputStage` (`input.rs`): `consume_drainable_inputs` is a free function with no host
  access, so it cannot resolve the ref itself. Change it to **return the last consumed
  user-facing `message_ref`** (bind `LoopInput::UserMessage { message_ref }` /
  `FollowUp` / `Steering` in the drain-mode arm, input.rs:170–174, and return the last
  one). Then `drain` (which holds `ctx`) calls
  `ctx.host.resolve_message_text(context, &last_message_ref)` and stores the result in
  `state.last_user_text`. This is the exact call H3 asked the plan to specify.

**H5 — orchestrator port (the Python bridge — resolves DRIVER-GAP + TIER0-GAP).**
The plan's RecipeStage↔Python-step-0 stash/unstash (item 5) and the §5 Tier 0 diagram both
assume the agent-loop `RecipeStage` and the engine Python orchestrator run in the **same
turn** — `RecipeStage` stashes into `state`, then Python step-0 (`__assemble_prior_knowledge__`)
reads the stash. **This unification does not exist today.** Verified:
- The production turn driver is the engine `ExecutionLoop::run` (`loop_engine.rs:413`), which
  calls `execute_orchestrator` (Python `default.py`) directly with **no stage pipeline**.
- The agent-loop `DefaultExecutorPipeline::execute` (`canonical.rs`) — where `RecipeStage`,
  `PromptStage`, `ModelStage`, `CapabilityStage` live — is a **skeleton**: `DefaultExecutorPipeline`
  / `execute_family` appear **only** inside `brassclaw_agent_loop` (canonical.rs, pipeline.rs,
  tests). No product surface drives it.
- `brassclaw_agent_loop` does **not** depend on `brassclaw_engine` (Cargo.toml), and
  `__assemble_prior_knowledge__` exists **only** in `brassclaw_engine` (orchestrator.rs,
  loop_engine.rs, default.py, retrieval_source.rs). The agent loop cannot import it.

So a third host port is required — the **only** crate that can bridge the two is
`brassclaw_reborn_composition`, which depends on **both** `brassclaw_engine` and
`brassclaw_agent_loop` (Cargo.toml). Add:
- `crates/brassclaw_turns/src/run_profile/host.rs` — a new opt-in port, added to the
  `AgentLoopDriverHost` supertrait list as the **15th** entry (immediately after the
  `LoopRetrievalPort` added by H4 — verified current list has 13 ports ending at
  `LoopInterceptorPort` at host.rs:2198; H4 makes `LoopRetrievalPort` the 14th; this is 15th):
  ```rust
  /// Bridge from agent-loop stages to the engine Python orchestrator.
  /// Implemented only by composition (the sole crate depending on both
  /// brassclaw_engine and brassclaw_agent_loop). Hosts without an orchestrator
  /// inherit `NoOrchestrator`; Tier 0 then falls back to Tier 2 (no short-circuit)
  /// and Tier 1 step-0 runs the legacy in-orchestrator fetch (no stash).
  pub trait LoopOrchestratorPort: Send + Sync {
      /// Tier 1: run Python step-0 prior-knowledge assembly. Reads the stashed
      /// `recipe_hint` (one-shot consume) and returns the formatted prior-knowledge
      /// bundle that PromptStage / build_prompt_bundle injects. Does NOT call the LLM.
      async fn run_step_zero(
          &self,
          context: &LoopRunContext,
          recipe_hint: Option<&serde_json::Value>,
      ) -> Option<PriorKnowledgeBundle>;

      /// Tier 0: run the orchestrator channel (skills + PythonCode) with NO LLM.
      /// Consumes the stashed `recipe_hint` (orchestrator_items) + `recipe_rust_context`,
      /// drives the Rust executioner via the loaded skills, and returns the reply
      /// text for AssistantReplyStage. Returns None if no orchestrator is wired
      /// (RecipeStage must then fall back to Tier 2).
      async fn run_tier_zero(
          &self,
          context: &LoopRunContext,
          recipe_hint: &serde_json::Value,
          recipe_rust_context: &serde_json::Value,
      ) -> Option<TierZeroReply>;
  }
  ```
  `PriorKnowledgeBundle` and `TierZeroReply` are **`brassclaw_turns`-native** types (plain
  `serde_json::Value` / `String` payloads) — same crate-boundary discipline as
  `RetrievalTurnResult` (H4) and `state.recipe_hint` (item 1). Add a `NoOrchestrator` default
  impl returning `None` (mirror `NoRetrieval` / `NoRecipeLookup`).
- `crates/brassclaw_reborn_composition/src/...` — implement `LoopOrchestratorPort` by
  delegating to two **new `pub` library functions** in `brassclaw_engine`. See the "Files to
  modify" entry below for their exact signatures.
  > **⚠️ FIND-NEW-PASS12-01 + FIND-NEW-PASS12-02 — CRITICAL: the composition host CANNOT call
  > `handle_assemble_prior_knowledge` or `execute_recipe_orchestrator_channel` directly.**
  >
  > `handle_assemble_prior_knowledge` (orchestrator.rs:2552) is a **private `async fn`** with
  > signature `(args: &[MontyObject], thread: &Thread, ...)` — NOT `pub`, NOT externally
  > callable, NOT parameterised with `recipe_hint`. It is an internal Monty VM dispatch handler.
  > `execute_recipe_orchestrator_channel` is a Python function inside `default.py` — it is not
  > reachable from Rust at all from outside the VM.
  >
  > The `execute_orchestrator` entry point (orchestrator.rs:444, `pub async fn`) runs the
  > ENTIRE Python VM from scratch — it is not suitable for the focused step-0 or Tier-0
  > channel invocations needed here.
  >
  > **Required: two new `pub` functions in `brassclaw_engine::executor::orchestrator`:**
  >
  > **For `run_step_zero` (`LoopOrchestratorPort` Tier 1):**
  > ```rust
  > /// Pure-Rust prior-knowledge assembly. Replaces the `__assemble_prior_knowledge__`
  > /// Monty VM handler with a direct library call for the `LoopOrchestratorPort` bridge.
  > /// When `recipe_hint` is Some(v): uses the stashed orchestrator_items from `RecipeStage`
  > /// (no second `fetch_for_turn`). When None: calls `retrieval_source.fetch_for_turn`.
  > pub async fn assemble_prior_knowledge_with_hint(
  >     thread: &Thread,
  >     goal: &str,
  >     token_budget: usize,
  >     sender_class_code: &str,
  >     retrieval_source: Option<&Arc<dyn RetrievalSource>>,
  >     recipe_hint: Option<serde_json::Value>,  // stashed orchestrator_items from RecipeStage
  > ) -> Result<PkrAssemblyResult, EngineError>
  > ```
  > `PkrAssemblyResult` is a new `pub struct` in `brassclaw_engine`:
  > ```rust
  > pub struct PkrAssemblyResult {
  >     pub orchestrator_content: String,    // formatted Skill + PythonCode bodies
  >     pub matched_component_ids: Vec<String>,
  >     pub override_prompt_creation: bool,
  >     pub action_short_circuit: bool,
  >     pub action_component_id: Option<String>,
  >     pub action_name: Option<String>,
  >     pub disambiguation: bool,
  >     pub candidates: Vec<serde_json::Value>,
  >     pub tier_zero: bool,                 // true when llm_call_required == false
  > }
  > ```
  > The existing private `handle_assemble_prior_knowledge` becomes a thin wrapper:
  > ```rust
  > // Inside the `__assemble_prior_knowledge__` dispatch arm:
  > let result = assemble_prior_knowledge_with_hint(thread, &goal, token_budget,
  >     &sender_class_code, retrieval_source, None).await?;
  > return json_to_monty(&serde_json::to_value(&result)?);
  > ```
  > Composition host's `run_step_zero` calls `assemble_prior_knowledge_with_hint` with the
  > `recipe_hint` extracted from `state` by the stage.
  >
  > **For `run_tier_zero` (`LoopOrchestratorPort` Tier 0):**
  > ```rust
  > /// Run the Tier-0 orchestrator channel (PythonCode bodies + tool calls) without an LLM.
  > /// Embodies the `execute_recipe_orchestrator_channel` logic as a Rust library function.
  > /// The Python `execute_recipe_orchestrator_channel` helper in `default.py` is the
  > /// Model A (engine path) counterpart — both implement the same logic, one in Python
  > /// (for `ExecutionLoop`), one in Rust (for the `LoopOrchestratorPort` bridge).
  > pub async fn execute_tier_zero_channel(
  >     thread: &Thread,
  >     orchestrator_content: &str,    // formatted PythonCode bodies (from recipe_hint)
  >     rust_context: &serde_json::Value,  // pre-loaded ToolSkill bindings (from recipe_rust_context)
  >     effects: &Arc<dyn EffectExecutor>,
  >     leases: &Arc<LeaseManager>,
  >     policy: &Arc<PolicyEngine>,
  >     gate_controller: &Arc<dyn GateController>,
  > ) -> Result<TierZeroChannelResult, EngineError>
  >
  > pub struct TierZeroChannelResult {
  >     pub formatted_output: String,            // the final text to emit to the user
  >     pub matched_component_ids: Vec<String>,  // for Wilson score recording
  > }
  > ```
  > Composition host's `run_tier_zero` calls `execute_tier_zero_channel`. The Python helper
  > `execute_recipe_orchestrator_channel` in `default.py` continues to be used by the engine
  > path (Model A) when the full Python VM is running — the two implementations share the
  > same logic but live in different layers. Code-share can be via calling `__execute_code_step__`
  > (already a registered Monty host function) or by extracting a common Rust helper.
  >
  > **Files to modify (addition to Phase H "Files to modify"):**
  > - `crates/brassclaw_engine/src/executor/orchestrator.rs` — add `assemble_prior_knowledge_with_hint`
  >   and `execute_tier_zero_channel` as `pub async fn`s; refactor the private
  >   `handle_assemble_prior_knowledge` dispatch handler to delegate to the former; add
  >   `PkrAssemblyResult` and `TierZeroChannelResult` as `pub struct`s.
  > - `crates/brassclaw_engine/src/executor/mod.rs` (or engine `lib.rs`) — re-export
  >   `assemble_prior_knowledge_with_hint`, `execute_tier_zero_channel`, `PkrAssemblyResult`,
  >   `TierZeroChannelResult` so `brassclaw_reborn_composition` can import them.
  >
  > (Requires the rust execution context to be applied first — see item 4 / the
  > `TierZeroExecutionStage` below.)

**TIER0-GAP resolution — the kick mechanism (Option 1, chosen):** add a new
`TierZeroExecutionStage` to `canonical.rs`, inserted between `RecipeStage` and
`AssistantReplyStage`, invoked **only** on `PostRecipeOutcome::TierZero`. It:
1. Applies the stashed `recipe_rust_context` to the Rust execution context (the orchestrator
   port does this server-side, or the stage hands it through).
2. Calls `ctx.host.run_tier_zero(context, &recipe_hint, &recipe_rust_context)`.
3. On `Some(reply)` → emits via `AssistantReplyStage` (PromptStage/InterceptorStage/ModelStage/
   CapabilityStage all skipped).
4. On `None` (no orchestrator wired) → degrades to Tier 2: re-enter the normal
   NeedsPrompt path (or end the turn with a no-match). This keeps Tier 0 **opt-in** via the
   host port, matching the dormancy pattern.
`CapabilityStage` is **not** bent — it keeps its "react to model output" assumption and is
simply skipped in Tier 0. This is cleaner than Option 2 (synthetic signal into
`LoopCapabilityPort`), which would couple the capability port to Tier 0 routing.

> **⚠️ DRIVER-PREREQ + MODEL SELECTION — two execution models, both covered during migration.**
> The plan was silently mixing two runtimes. Verified against live code:
> - **Model A — `ExecutionLoop` / Monty (CURRENT PRODUCTION).** `loop_engine.rs:413` runs
>   `execute_orchestrator`; Python IS the outer loop and calls the LLM itself via
>   `__llm_complete__` (`default.py:1103` → `handle_llm_complete`, dispatched at
>   `orchestrator.rs:563`, defined at `orchestrator.rs:795`). `RecipeStage`/`ModelStage` do
>   not exist here. The engine has a deterministic no-LLM *pattern* —
>   `execute_action_procedure` (`default.py:901`, "returns without calling
>   `__llm_complete__`") for class-16 Actions — but it is NOT gated by
>   `override_prompt_creation`. Verified: the `override_prompt_creation` block
>   (`default.py:998-1008`) only swaps `working_messages` and FALLS THROUGH to
>   `__llm_complete__`; the only pre-`__llm_complete__` return is the dead
>   `__retrieve_docs__`+`class_code==16` shim (`default.py:1018-1027`; §0.9 Problem 1 — never
>   fires, the shim surfaces no `class_code`). `override_prompt_creation: true` is set by
>   `assemble_from_component_items` (`orchestrator.rs:2689`) for a Solution Override, which
>   is an LLM path, NOT a no-LLM path. So today NO deterministic no-LLM turn path actually
>   works in production.
> - **Model B/C — `DefaultExecutorPipeline` / agent_loop (TARGET STATE, skeleton today).**
>   `canonical.rs`; no Python access (`brassclaw_agent_loop` does not depend on
>   `brassclaw_engine`). `RecipeStage`/`ModelStage`/`TierZeroExecutionStage` live here.
>
> **Resolution — production Tier 0 lands via Model A once Phase H adds the `tier_zero`
> signal + early-return branch (NOT "today"); agent-loop stages are the target state active
> after switchover:**
> - **Phase H §A (engine path, production):** wire `fetch_for_turn` `SplitResult` with
>   `llm_call_required: false` so that `handle_assemble_prior_knowledge` returns a DEDICATED
>   `tier_zero: true` field (NEW — do NOT reuse `override_prompt_creation`; that is the
>   Solution-Override LLM path at `orchestrator.rs:2683-2691` and must stay an LLM path) plus
>   `orchestrator_content`. Python step-0 gets a NEW `if pkr.get("tier_zero"): return
>   execute_recipe_orchestrator_channel(...)` early-return branch (sibling of the
>   `action_short_circuit` return, placed before `__llm_complete__` at `default.py:1103`)
>   that runs the recipe's orchestrator channel (skills + PythonCode) deterministically
>   against the pre-loaded rust execution context — the `execute_action_procedure` pattern
>   generalised from class-16 Actions to Tier-0 Recipes. **⚠️ This is NEW wiring, not a
>   "no new kick" generalisation of something live:** the kick is the new `tier_zero`
>   early-return branch; today no such branch exists and Tier 0 does not work. Phase H's
>   engine-side work is: (1) make `handle_assemble_prior_knowledge` emit `tier_zero: true`
>   (NOT `override_prompt_creation`) when `routing.llm_call_required == false`; (2) add the
>   `tier_zero` early-return branch + `execute_recipe_orchestrator_channel` helper to
>   `default.py` step-0; (3) ensure it consumes the stashed `orchestrator_items`
>   (Tier-1 stash/unstash, item 5) and drives the Rust executioner via the loaded skills
>   without an LLM round-trip.
> - **Phase H §B/C (agent-loop path, target state):** `LoopOrchestratorPort` +
>   `TierZeroExecutionStage` (above). Until the agent-loop `DefaultExecutorPipeline` is wired
>   as the production driver (or the engine `ExecutionLoop::run` retired), this stage work is
>   exercised **only** in `brassclaw_agent_loop` tests — it does not affect production
>   traffic, which is served by the Model-A mechanism. This mirrors the C2
>   `PostgresSource`-dormant pattern. The switchover is a tracked prerequisite (adjacent to,
>   not part of, the recipe-system scope).
> - **LLM-call ownership (v3 target):** once the agent-loop is the driver, `ModelStage` owns
>   the Tier 1+ LLM call and the Python `__llm_complete__` loop is retired (Python reduced to
>   step-0 prior-knowledge + Tier-0 no-LLM execution). **During migration both mechanisms
>   coexist:** Model A serves production (Tier 0 via `tier_zero` once Phase H lands it);
>   Model B/C stages are test-only until switchover. The earlier draft "rejected" the
>   engine-side implementation — that was wrong: it would have left production with no Tier 0
>   until an indefinite switchover. The engine-side mechanism is retained and is the
>   production path. A subsequent draft then over-corrected by claiming Tier 0 "works today"
>   via `override_prompt_creation` — also wrong (that signal does not skip
>   `__llm_complete__`); corrected here to the dedicated `tier_zero` signal + new
>   early-return branch.

> **Note:** All three ports (`LoopRetrievalPort`, `resolve_message_text` via
> `LoopContextPort`, and `LoopOrchestratorPort`) are **Phase H prerequisites**, not Phase K
> afterthoughts. They are additive `brassclaw_turns` traits with `NoRetrieval` /
> `resolve_message_text`-returning-`None` / `NoOrchestrator` defaults, so existing host impls
> (tests) keep compiling. They MUST be added in the same phase as the `RecipeStage` dispatch
> body (item 3) and the `TierZeroExecutionStage` (item 4) — the dispatch pseudocode
> (`result = retrieval_source.fetch_for_turn(...)`, `user_text = state.last_user_text`) and
> the Tier 0 kick (`ctx.host.run_tier_zero(...)`) are unimplementable without them.

#### Files to modify

1. `crates/brassclaw_agent_loop/src/state.rs` — add to `LoopExecutionState`:
   **Verified:** `LoopExecutionState` (lines 47–100) currently has NO `last_user_text`,
   `recipe_rust_context`, or `recipe_hint` fields. All three must be added:
   ```rust
   /// Last user-visible input text; populated by InputStage on each drain.
   /// Required by RecipeStage for fetch_for_turn query. See recipe.rs module doc.
   #[serde(default)] pub last_user_text: Option<String>,
   /// Stashed rust_items from a Tier 1 SplitResult. Applied to the Rust execution
   /// context before the Python scripting engine starts each turn. Cleared after use.
   #[serde(default)] pub recipe_rust_context: Vec<serde_json::Value>,
   /// Stashed orchestrator_items hint from a Tier 1 SplitResult. PromptStage
   /// injects this before the UNION ALL scan. Cleared after use.
   #[serde(default)] pub recipe_hint: Option<serde_json::Value>,
   ```
   The struct currently ends with `pending_prose_conversion: Option<String>` (line 93),
   `content_cache: ContentCacheState` (line 98), and `spawn_subagent_hint: Option<String>`
   (line 102) — **verified by reading `state.rs:47–103`**. Append the three new fields
   **after `spawn_subagent_hint`** (the last existing field); the `#[serde(default)]` attribute
   ensures all existing checkpoint JSON payloads that lack these fields still deserialise
   without error.
   > **Crate boundary constraint:** `brassclaw_agent_loop` depends on `brassclaw_turns` but NOT
   > on `brassclaw_engine`. `ComponentItem` is defined in `brassclaw_engine`. Therefore
   > `recipe_rust_context` and `recipe_hint` CANNOT be typed as `Vec<ComponentItem>` or
   > `Vec<ComponentItem>` — doing so would create a forbidden crate dependency.
   > They are typed as `serde_json::Value` (pre-serialized at `RecipeStage` before being
   > stored in state). The executor deserializes them back to component data when
   > applying them to the execution context, using types from `brassclaw_turns` only.
   >
   > **⚠️ SEC-02 — stale recipe_hint survives checkpoint restore:**
   > `recipe_hint` and `recipe_rust_context` are serialized in `LoopExecutionState` via
   > `#[serde(default)]`. If the loop is checkpointed after `RecipeStage` sets the stash
   > but before `handle_assemble_prior_knowledge` consumes it, then restores from that
   > checkpoint on a retry, the stash is replayed — the handler will use a potentially
   > stale pre-fetched result. To mitigate: always clear both fields at the start of each
   > `RecipeStage::process` call (not just after consume), so a resumed turn re-fetches
   > fresh. Phase H must add `state.recipe_hint = None; state.recipe_rust_context = vec![];`
   > at the top of `RecipeStage::process` before doing anything else.

2. `crates/brassclaw_agent_loop/src/executor/input.rs` — populate `last_user_text` from
   drained input (the last user message text seen this turn).

   > **Codebase reality:** `consume_drainable_inputs` (input.rs line 154) currently processes
   > `LoopInput::UserMessage { .. }` and `LoopInput::Steering { .. }` as matching drain-mode
   > inputs, but **only advances `state.input_cursor`** — it never extracts the message text.
   > Phase H must also modify `consume_drainable_inputs` (or add a parallel extraction pass)
   > to capture the text from whichever `UserMessage`/`Steering` input was consumed.
   > The text is needed before `RecipeStage` runs; it must be in `state.last_user_text`
   > when `InputStage` returns `InputStep::Continue`.
   >
   > **⚠️ FIND-18 — `consume_drainable_inputs` is a PURE FUNCTION with no host access;
   > there is no `drain` function in `input.rs`:**
   > Verified by reading `input.rs:154–211`: `consume_drainable_inputs` is a free function
   > (`pub(super) fn consume_drainable_inputs(batch, mode, state)`) with NO `ctx` parameter.
   > The matching drain-mode inputs (lines 169–173) are handled as:
   > ```rust
   > if user_facing_input_matches_drain_mode(input, mode) {
   >     consumed_len += 1;
   >     drained = true;
   >     continue;   // <-- message_ref is NEVER captured here
   > }
   > ```
   > The `LoopInput::UserMessage { message_ref }` variant is matched by
   > `user_facing_input_matches_drain_mode` (a separate function at line 213) and then
   > the `consumed_len += 1; continue` branch runs — the `message_ref` is discarded.
   >
   > **Required Phase H restructuring of `InputStage::process`:**
   > The function holding `ctx` is `InputStage::process` (which receives
   > `ctx: StageContext<'_>`). The correct Phase H implementation is one of:
   > - **Option A (return last ref from free function):** Modify `consume_drainable_inputs`
   >   to also return `Option<LoopMessageRef>` (the last consumed user-facing input's ref).
   >   The caller (`InputStage::process`) then calls
   >   `ctx.host.resolve_message_text(context, &message_ref).await` and stores in
   >   `state.last_user_text`. This requires changing the return type of
   >   `consume_drainable_inputs`.
   >
   >   **⚠️ FIND-P5-04 — exact confirmed return type change:**
   >   Current return type (confirmed from `input.rs:158-163`):
   >   ```rust
   >   Result<(bool, Vec<LoopInputAckToken>, Option<LoopCancelledReasonKind>), AgentLoopExecutorError>
   >   ```
   >   New return type with Option A:
   >   ```rust
   >   Result<(bool, Vec<LoopInputAckToken>, Option<LoopCancelledReasonKind>, Option<LoopMessageRef>), AgentLoopExecutorError>
   >   ```
   >   The 4th tuple element is the last consumed user-facing `message_ref`.
   >   All callers of `consume_drainable_inputs` (there is exactly one: `InputStage::drain`
   >   in `input.rs`) must destructure the 4th element and call `resolve_message_text`.
   > - **Option B (parallel extraction in `InputStage::process`):** Before calling
   >   `consume_drainable_inputs`, scan the `batch.inputs` array in `InputStage::process`
   >   to find the last user-facing input ref, resolve it via the host, then call
   >   `consume_drainable_inputs` as before.
   >
   > **Option A is recommended** — it keeps the ref extraction inside the function that
   > already iterates the inputs. Do NOT add a "drain function" — it does not exist.
   > Do NOT call `ctx.host.resolve_message_text` from `consume_drainable_inputs` — it
   > has no `ctx`.
   >
   > **⚠️ FIND-04 — `resolve_message_text` composition implementation is missing from "Files to modify":**
   > The plan's §H.0 specifies adding `resolve_message_text` to `LoopContextPort` with a
   > default impl. But it does NOT list which composition file implements the non-default
   > (real) version that reads the raw text from `messages_by_run`. This is required.
   >
   > **⚠️ FIND-28 — `resolve_message_text` MUST have a default implementation in the trait body:**
   > Verified: `LoopContextPort` (host.rs:778–784) currently has only ONE method:
   > `load_loop_context`. It is a **required** trait — it is in the `AgentLoopDriverHost`
   > supertrait list (line 2187). Adding `resolve_message_text` WITHOUT a default body is a
   > **breaking change** — every existing `AgentLoopDriverHost` implementor (including all test
   > hosts in `support.rs`) must implement it. Phase H MUST provide a default implementation
   > directly in the `LoopContextPort` trait body:
   > ```rust
   > async fn resolve_message_text(
   >     &self,
   >     _context: &LoopRunContext,
   >     _message_ref: &LoopMessageRef,
   > ) -> Result<String, AgentLoopHostError> {
   >     Err(AgentLoopHostError::new(
   >         AgentLoopHostErrorKind::Unimplemented,
   >         "resolve_message_text not implemented by this host",
   >     ))
   > }
   > ```
   > Only the composition host (which has access to `messages_by_run`) overrides this default.
   > Test hosts inherit the default and return `Err(Unimplemented)`, which means
   > `state.last_user_text` stays `None` in tests — acceptable since test recipes can
   > seed `last_user_text` directly for unit testing.
   >
   > **Additional "Files to modify" for H3:**
   > - `crates/brassclaw_turns/src/run_profile/host.rs` — add `resolve_message_text` method
   >   to `LoopContextPort` trait **with a default impl** (see FIND-28 above).
   > - The composition host implementation file (the struct that implements
   >   `AgentLoopDriverHost` in `brassclaw_reborn_composition`) — implement
   >   `resolve_message_text` by reading from the composition-layer `messages_by_run` store
   >   (the `SkillActivationMessage { text }` keyed by `message_ref`, written at
   >   `activation.rs:254–272`, live until `clear_accepted_message` at `runtime.rs:1062`).
   >   The implementer must identify which composition struct implements the host trait and
   >   add `resolve_message_text` there. This is a concrete new file that must be listed.
   > - `crates/brassclaw_agent_loop/src/executor/input.rs` — modify `consume_drainable_inputs`
   >   to also return the last user-facing `LoopMessageRef` (Option A above). Then in
   >   `InputStage::process`, call `ctx.host.resolve_message_text(context, &ref).await`
   >   and store in `state.last_user_text`. There is NO `drain` function — `InputStage::process`
   >   is the function that holds `ctx` (see FIND-18).
   >
   > **⚠️ COMP-01 — `LoopInput` field names must be verified in `brassclaw_turns`:**
   > The plan writes `LoopInput::UserMessage { content, .. }` but the actual struct
   > definition lives in `brassclaw_turns` (not `brassclaw_agent_loop`). The field name
   > `content` is an assumption — it has NOT been verified by reading that crate's source.
   > Before implementing, read `brassclaw_turns/src/lib.rs` or wherever `LoopInput` is
   > defined and confirm the exact field name. The implementation must not guess.
   >
   > **✅ Review note (pre-v3 audit) — COMP-01 RESOLVED by reading the source — and the
   > H3 "specify which call" follow-up is RESOLVED in §H.0 above:** `LoopContextPort::resolve_message_text(context, message_ref)` reads the raw text from the composition
   > `messages_by_run` store; `consume_drainable_inputs` returns the last consumed ref and
   > `drain` resolves it. Original audit detail retained below:
   > `LoopInput` is defined in `crates/brassclaw_turns/src/run_profile/host.rs:843` and
   > re-exported from `lib.rs:56`. The variant is `UserMessage { message_ref: LoopMessageRef }`
   > — there is **no `content` field**; the payload is a `message_ref`, not text. `LoopMessageRef`
   > is an opaque newtype produced by the `loop_ref!` macro (`brassclaw_turns/src/ids.rs:242`,
   > string form `"msg:…"`); it is a reference, **not embedded message text**. The other
   > user-facing variants are identical: `FollowUp { message_ref }`, `Steering { message_ref }`.
   > Consequence for Phase H: `consume_drainable_inputs` (`input.rs:154`) cannot "extract the
   > text" from the `LoopInput` value alone — it only holds an opaque ref. To populate
   > `state.last_user_text`, Phase H must resolve the ref to its text via a host/turn API
   > (the accepted-message body keyed by `message_ref`), or capture the text earlier in the
   > pipeline where it is still available (e.g. the turn-submission path that mints the ref).
   > The plan's "capture the text from whichever UserMessage/Steering input was consumed"
   > understates this: the text is not in the `LoopInput`, so a host fetch or an upstream
   > capture is required. Specify exactly which call resolves `message_ref → text`.

3. `crates/brassclaw_agent_loop/src/executor/recipe.rs` — replace stub with full dispatch:

   > **Enum note:** The current `RecipeStep` enum has only `Continue { state }`. Phase H
   > adds `TierZero` and `ActionExecuted` variants. The internal enum is `RecipeStep`
   > (the type alias `RecipeStageOutcome` used in comments below maps to it).
   >
   > **RecipeLookup vs fetch_for_turn:** `ctx.host.recipe_lookup()` is backed by
   > `PgRecipeLibrary` (in `pg_recipe_store.rs`) — a real Postgres implementation that
   > queries `reborn_recipes` using trigger-based scoring (exact/keyword/pattern match).
   > It is NOT a dead v2 path; it is wired and used in production via `runtime.rs`.
   > However, it uses the old `trigger` JSONB scoring — not the intent-system (`resolve_intent`).
   > Phase H uses `PostgresSource::fetch_for_turn` (intent-driven) instead.
   > The `recipe_lookup()` port **must be kept wired and functional** — it provides the
   > outcome recording path (`record_recipe_outcome`) that updates wilson_lower/tier.
   > Phase H adds a parallel intent-driven lookup; both paths coexist during the v3 transition.
   > `record_recipe_outcome` must be called from the v3 path too (same Wilson update needed).
   >
   > **⚠️ FINDING G — `PgRecipeLibrary::find_recipe` uses STRIPPED `is_tier0_eligible`:**
   > `PgRecipeLibrary::find_recipe` (pg_recipe_store.rs:790) sets `tier0_eligible: recipe.is_tier0_eligible()`
   > on the returned `RecipeMatchDto`, using `PgRecipe::is_tier0_eligible()` — which only checks
   > `is_deliverable() && tier ∈ {mature, candidate}`. It **omits** the `wilson_lower >= 0.70`
   > guard and the validation-hook check that `Recipe::is_tier0_eligible()` in `types/recipe.rs`
   > performs. The v3 Tier 0 path in `RecipeStage` must NOT trust the `tier0_eligible` field
   > from `RecipeMatchDto` for the Tier 0 decision. Instead, compute `tier0_eligible` from
   > `TurnRoutingSignals` (populated by `fetch_for_turn` → Phase E, which uses the full check).
   > The `RecipeMatchDto.tier0_eligible` field is a v2 artefact with an incomplete check.
   > If `PgRecipeLibrary.find_recipe` is called anywhere for non-outcome-recording purposes
   > in Phase H, do not use its `tier0_eligible` result for Tier 0 routing.
   >
   > **⚠️ FIND-05 + NEW + FIND-P7-11: Fix `PgRecipe::is_tier0_eligible()` to prevent silent Tier-0 escalation.**
   > **Confirmed by live code read (Pass 7):** `pg_recipe_store.rs:140-142` currently is:
   > ```rust
   > pub(crate) fn is_tier0_eligible(&self) -> bool {
   >     self.is_deliverable() && matches!(self.tier.as_str(), "mature" | "candidate")
   > }
   > ```
   > The `wilson_lower >= 0.70` guard is MISSING. This is a dangerous bug — a recipe that
   > has never been used (wilson_lower = 0.0, tier = "mature" via some other path) would
   > be silently eligible for Tier 0. Fix: `self.is_deliverable() && matches!(self.tier.as_str(), "mature" | "candidate") && self.wilson_lower >= 0.70`. The `PgRecipe` struct carries `wilson_lower: f64` at line 114, so the check is one line.
   >
   > **⚠️ ORDERING: This fix belongs in Phase A** — single-line, no dependencies.
   > Do NOT defer to Phase E or later. Fix as the first sub-task of Phase A.
   >
   > **rust_items application:** `RecipeStage` runs at the agent loop level (above the
   > Python scripting engine) so it CAN apply rust_items to the Rust execution context
   > directly. For Tier 1, rust_items are stashed in `state.recipe_rust_context` and
   > applied by the executor before the Python script is invoked.
   >
   > **✅ Review note (pre-v3 audit) — resolves the Phase A "store round-trip" gap for the runtime
   > path — RESOLVED:** follow-up (1) (WebUI save path: `RECIPE_SELECT`/`decode_recipe_row`/
   > `NewPgRecipe`) is now in Phase A's "Files to modify" (see the H1 resolution); follow-up (2)
   > (H4: `RecipeStage` reaching `PostgresSource` across the crate boundary) is RESOLVED in §H.0
   > above via the new `LoopRetrievalPort` host port. Original audit detail retained below:
   > Because Phase H routes Tier 0/1 through `PostgresSource::fetch_for_turn` (intent/IBS),
   > which reads `step_descriptions` + `variable_patterns` **straight from the `reborn_recipes`
   > row** (Phase E step 2), the runtime never needs `step_descriptions` on `RecipeMatchDto` or on
   > the engine `Recipe` struct. That resolves the Phase A concern *for the dispatch path*. Two
   > follow-ups remain: (1) the **WebUI authoring/save path** (`PgRecipeStoreFacade`,
   > `pg_recipe_store.rs:861`) still needs to SELECT/decode/insert `step_descriptions` so authors
   > can write and re-load it — `RECIPE_SELECT`/`decode_recipe_row`/`NewPgRecipe` must be extended
   > (this was missing from Phase A's file list); (2) `RecipeStage::process` calls
   > `retrieval_source.fetch_for_turn` — confirm `RecipeStage` (in `brassclaw_agent_loop`) can
   > actually reach `PostgresSource`. The agent-loop crate does **not** depend on
   > `brassclaw_engine`, and `RetrievalSource`/`PostgresSource` live in `brassclaw_engine`. The
   > retrieval source is exposed to stages via the host port (`ctx.host.…`), not as a direct
   > engine import — Phase H must specify the host port method that exposes `fetch_for_turn` to
   > `RecipeStage` (the §0.3 flow assumes this exists; verify it does, or add it).

   > **⚠️ FIND-27 — pseudocode still uses wrong `retrieval_source.fetch_for_turn(...)` call:**
   > This is the call corrected by FIND-10. The pseudocode must use the host port.

   ```
   RecipeStage::process(state):
     user_text = state.last_user_text (return Continue if None)
     result = ctx.host.fetch_for_turn(context, user_text, budget, "02")
             // ↑ FIND-10/FIND-27 fix: NOT `retrieval_source.fetch_for_turn`
             // The host port (LoopRetrievalPort) is the only way to reach
             // fetch_for_turn from brassclaw_agent_loop.

     match result:
       SplitResult { rust_items, orchestrator_items, routing }:
         if routing.tier0_eligible && !routing.llm_call_required:
           // Tier 0: no LLM — tier0_eligible = tier∈{mature,candidate} + wilson≥0.70
           //                                    + validated + validation hook wired
           apply rust_items to Rust execution context (RecipeStage has direct access)
           stash orchestrator_items in state for PromptStage bypass
           return RecipeStep::TierZero { routing }       // NEW variant

         else:
           // Tier 1: inject hint, let LLM decide.
           stash rust_items in state.recipe_rust_context  // applied before Python starts
           stash orchestrator_items in state.recipe_hint
           return RecipeStep::Continue { state }

       ActionShortCircuit { component_id, name }:
         execute Action or stash for Python step-0 to handle
         return RecipeStep::ActionExecuted { component_id, name }  // NEW variant

       Components(_) | Disambiguation(_) | (no match):
         return RecipeStep::Continue { state }  // Tier 2 — unchanged
   ```

3b. `crates/brassclaw_engine/src/executor/orchestrator.rs` + `crates/brassclaw_engine/orchestrator/default.py` — **engine-path
   Tier 0 wiring (Model A, CURRENT PRODUCTION — see DRIVER-GAP / H5 MODEL SELECTION).** The
   agent-loop `RecipeStage` (item 3) does not run in production today; production is the
   engine `ExecutionLoop` where Python step-0 calls `__assemble_prior_knowledge__`. Phase H
   must therefore ALSO wire Tier 0 on the engine path so production gets Tier 0 before any
   agent-loop switchover. **⚠️ Corrected (prior draft was wrong):** a prior draft claimed
   Tier 0 "works today" by reusing the `override_prompt_creation: true` signal to skip
   `__llm_complete__`. That is FALSE against live code — `override_prompt_creation: true`
   (default.py:998-1008) only swaps `working_messages` and FALLS THROUGH to
   `__llm_complete__` (default.py:1103); it does NOT short-circuit. The only
   pre-`__llm_complete__` return today is the dead `__retrieve_docs__`+`class_code==16`
   shim (default.py:1018-1027; §0.9 Problem 1 — never fires because the shim surfaces no
   `class_code`). And `handle_assemble_prior_knowledge` (orchestrator.rs:2552) today
   returns only `{content, formatted_content, override_prompt_creation,
   matched_component_ids}` — no `action_short_circuit`, no `tier_zero`. So Tier 0 does
   NOT work today; Phase H must ADD it. Two changes:
   - In `handle_assemble_prior_knowledge` (orchestrator.rs:2552; the `SplitResult` arm is
     added by Phase E/F): when `fetch_for_turn` returns `SplitResult { routing, .. }` with
     `routing.llm_call_required == false`, return a pkr with a **DEDICATED `tier_zero: true`**
     field (NEW — do NOT set `override_prompt_creation`; that is the Solution-Override
     LLM path and must remain an LLM path), the `orchestrator_items` serialised as
     `orchestrator_content`, and `recipe_name` for telemetry. The `rust_items` are applied
     to the rust execution context server-side (the executor path that
     `execute_recipe_orchestrator_channel` reads). `matched_component_ids` carries the
     orchestrator-channel UUIDs. (Mirror the existing `action_short_circuit` pkr shape that
     Phase F wires for class-16 Actions — same "no-LLM early-return" family, separate field.)
   - In `default.py` step-0: add a NEW early-return branch (see the §0.9 v3 step-0
     pseudocode) — `if pkr.get("tier_zero"): return execute_recipe_orchestrator_channel(pkr,
     goal, state)` — placed alongside the `action_short_circuit` return and BEFORE the
     `__llm_complete__` call (default.py:1103).

     > **⚠️ FIND-P9-02 — `execute_recipe_orchestrator_channel` is referenced 6+ times but
     > never specified. Full specification below.**

     `execute_recipe_orchestrator_channel` is a NEW helper function in `default.py`,
     sibling of `execute_action_procedure` (default.py:901). Specification:

     ```python
     def execute_recipe_orchestrator_channel(pkr: dict, goal: str, state) -> dict:
         """
         Tier-0 execution: runs the recipe's orchestrator channel (Skills + PythonCode)
         against the pre-loaded Rust execution context WITHOUT an LLM call.

         Called when pkr["tier_zero"] is True. The orchestrator_items were already
         applied to the Rust execution context by handle_assemble_prior_knowledge
         (rust_items via RecipeStage/state.recipe_rust_context; orchestrator_items
         in pkr["orchestrator_content"] as the formatted block).

         Returns a complete_result dict with outcome="success" or outcome="error".
         """
         orchestrator_content = pkr.get("orchestrator_content", "")
         matched_ids = pkr.get("matched_component_ids", [])

         # Parse the orchestrator_content block into ordered steps.
         # The format is the IBS-produced section headers + bodies:
         #   ## [PythonCode: <name>]
         #   <python body>
         #
         # ⚠️ ARCHITECTURAL INVARIANT: The orchestrator is ALWAYS the supervisory layer.
         # It is always the PythonCode in orchestrator_steps that calls __execute_action__
         # on the Rust executor. The Rust channel (rust_steps) only pre-loads which ToolSkills
         # are available — it does NOT self-execute. Therefore orchestrator_content must NEVER
         # be empty when rust_steps has tool_bindings (enforced by Q1 Rule 2, S7-extension).
         # At runtime, an empty orchestrator_content means Q1 was bypassed — fail explicitly.
         #
         # ⚠️ FIND-P9-02 CONSTRAINT: orchestrator_steps may ONLY contain PythonCode (class 22).
         # Skill bodies are narrative LLM instructions; executing them without an LLM is undefined
         # behaviour. Enforced at Q1 time (Phase I). At runtime, non-PythonCode → hard error.

         if not orchestrator_content.strip():
             return {"outcome": "error",
                     "message": "Tier-0 recipe has no orchestrator channel content. "
                                "Add a PythonCode component to orchestrator_steps that "
                                "calls __execute_action__ and formats the result. "
                                "(Q1 Rule 2 / S7-extension should have caught this.)"}

         steps = _parse_orchestrator_channel_steps(orchestrator_content)
         result_parts = []

         for step in steps:
             if step["type"] == "python":
                 # ⚠️ FIND-AUDIT-07 / DESIGN-01 SECURITY — MUST use __execute_code_step__, NOT exec().
                 #
                 # `exec("...", {}, locals)` does NOT sandbox Python — code can escape via __builtins__.
                 # Q1 injection scan alone is not sufficient to make raw exec() safe.
                 #
                 # The CORRECT invocation is `__execute_code_step__(code, {})` — the existing
                 # Monty VM host function (default.py:9, registered at orchestrator.rs:580–582).
                 # This is the same sandbox that default.py itself runs in. The PythonCode body
                 # uses `__execute_action__` and other registered host functions — they are already
                 # available in the VM context without injection. This is identical in spirit to
                 # how execute_action_procedure runs Action steps, but uses the code-execution VM
                 # path instead of the JSONB step-dispatch path.
                 #
                 # ⚠️ ISOLATION INVARIANT (DESIGN-DECISION-01): Each PythonCode step receives a
                 # FRESH EMPTY state dict `{}`, NOT the shared orchestrator `state`.
                 #
                 # Rationale: Steps are executed by the orchestrator one at a time. If a step needs
                 # output from a previous step, that information must be explicitly provided by the
                 # ORCHESTRATOR (which reads recipe instructions and caches results accordingly) — it
                 # is NOT automatically available to the next PythonCode body. This is an architectural
                 # invariant: the recipe + orchestrator defines the data flow; PythonCode bodies are
                 # isolated execution units that communicate only through __execute_action__ results
                 # and their explicit return_value.
                 #
                 # Consequences for recipe authors:
                 #   - A PythonCode body CANNOT read state["last_result"] from a previous step.
                 #   - All inputs to a step must come from: (a) __execute_action__ calls inside the
                 #     body, or (b) params passed via pkr["orchestrator_content"] (assembled by IBS
                 #     from the recipe's component descriptions + template vars).
                 #   - If step 2 needs the output of step 1's tool call, the RECIPE must be authored
                 #     so that step 2's PythonCode body itself calls __execute_action__ with the
                 #     appropriate params — NOT reads from a shared state key.
                 #   - This matches the Recipe authoring model: each orchestrator_step is a
                 #     self-contained capability. Chaining is done by the orchestrator reading the
                 #     recipe's step_descriptions and constructing appropriate pkr content per step.
                 #
                 # See §recipe-authoring-rules for the full recipe design constraints.
                 #
                 # `__execute_code_step__` takes (code: str, state: dict) and returns a result dict:
                 #   {
                 #     "return_value": <the last expression value or None>,
                 #     "stdout": <captured stdout>,
                 #     "action_results": [ {action_name, output}, ... ],
                 #     "final_answer": <FINAL() content or None>,
                 #     "error": <error message or None>,
                 #   }
                 # The PythonCode body should `result = ...` assign its output. The caller reads
                 # result_dict["return_value"] (the assigned `result`) as the step output.
                 # If the body calls __execute_action__, those results appear in action_results.
                 try:
                     vm_result = __execute_code_step__(step["body"], {})  # fresh state per step — see ISOLATION INVARIANT above
                     if vm_result.get("error"):
                         return {"outcome": "error",
                                 "message": f"PythonCode step '{step['name']}' failed: {vm_result['error']}"}
                     # Prefer explicit return_value; fall back to concatenated stdout.
                     step_result = vm_result.get("return_value") or vm_result.get("stdout") or ""
                     result_parts.append(str(step_result))
                 except Exception as e:
                     return {"outcome": "error", "message": f"PythonCode step '{step['name']}' failed: {e}"}
             elif step["type"] == "toolskill":
                 # ToolSkill steps are pre-loaded into the Rust execution context by
                 # RecipeStage (state.recipe_rust_context). The orchestrator invokes them
                 # via __execute_action__ using the skill's binding params.
                 # (ToolSkill bodies are in the rust channel, not orchestrator channel —
                 # if a ToolSkill appears here, that is a Q1 violation.)
                 return {"outcome": "error",
                         "message": f"Skill/ToolSkill component '{step['name']}' in "
                                    f"orchestrator_steps is not allowed for Tier-0 recipes. "
                                    f"Only PythonCode is permitted (Phase I §shell-guard). "
                                    f"Promote recipe to Tier 1 or replace with PythonCode."}
             else:
                 return {"outcome": "error", "message": f"Unknown step type '{step['type']}'"}

         formatted_output = "\n".join(result_parts)
         return {
             "result": formatted_output,
             "outcome": "success",
             "matched_component_ids": matched_ids,
         }
     ```

     Helper `_parse_orchestrator_channel_steps(orchestrator_content)` parses the
     `## [PythonCode: <name>]` / `## [Skill: <name>]` block format produced by IBS and
     returns a list of `{"type": "python"|"toolskill", "name": str, "body": str}` dicts.
     This helper is also used in tests.

     Do NOT route Tier 0 through the `override_prompt_creation` branch — that would
     conflate the no-LLM Tier-0 path with the LLM Solution-Override path and break
     Solution Override.

   - Tier 1 on the engine path is unchanged: `tier_zero` false, `override_prompt_creation`
     false, Python calls `__llm_complete__` guided by `orchestrator_content` (current
     behaviour).

4. `crates/brassclaw_agent_loop/src/executor/canonical.rs` — **executor loop restructuring
   required**. The current dispatch at line 94 is:
   ```rust
   state = match self.recipe.process(ctx, RecipeInput { state }).await? {
       RecipeStep::Continue { state: next } => *next,
   };
   ```
   This is an exhaustive match. Adding `TierZero` and `ActionExecuted` variants causes a
   **compile error** until canonical.rs handles them. Restructure using an intermediate enum:

   > **⚠️ COMP-02 — `TurnRoutingSignals` crate boundary violation in `PostRecipeOutcome`:**
   > `TurnRoutingSignals` is defined in `brassclaw_engine`. `canonical.rs` is in
   > `brassclaw_agent_loop`, which does NOT depend on `brassclaw_engine`.
   > Putting `routing: TurnRoutingSignals` in `PostRecipeOutcome::TierZero` would be a
   > forbidden crate dependency. **Solution:** `PostRecipeOutcome::TierZero` must NOT carry
   > the full `TurnRoutingSignals` struct. Instead it carries only the primitive fields that
   > `canonical.rs` actually needs to make routing decisions — all of which are plain scalar
   > types that can be defined locally or re-exported through `brassclaw_turns`:
   > ```rust
   > TierZero {
   >     state:              Box<LoopExecutionState>,
   >     tier0_eligible:     bool,    // already in state.recipe_hint context
   >     llm_call_required:  bool,
   > }
   > ```
   > `RecipeStep` (in `recipe.rs`) also must NOT carry `TurnRoutingSignals`. The routing
   > signals that `canonical.rs` needs for its dispatch decision are at most two booleans.
   > All richer metadata (variant_label, wilson_lower, step_link, matched_component_ids)
   > are already serialized into `state.recipe_hint` by `RecipeStage` before returning.
   >
   > **⚠️ CANONICAL-01 — The current exhaustive `RecipeStep` match in `canonical.rs` will
   > not compile after Phase H adds `TierZero` and `ActionExecuted` variants:**
   > Verified at `canonical.rs:94-96`: the current match is:
   > ```rust
   > state = match self.recipe.process(ctx, RecipeInput { state }).await? {
   >     RecipeStep::Continue { state: next } => *next,
   > };
   > ```
   > This is an exhaustive match over the single `Continue` variant. The compiler will
   > emit an error the moment `TierZero` and `ActionExecuted` are added to `RecipeStep`.
   > Phase H MUST restructure `canonical.rs` at this point — the intermediate
   > `PostRecipeOutcome` enum described below is the correct approach. The `state = match`
   > assignment pattern CANNOT be reused for a non-homogeneous return (some variants want
   > to skip stages, not just produce a state). The restructuring is not optional.

   ```rust
   /// Produced by the RecipeStage dispatch inside canonical.rs.
   /// Determines which pipeline stages run after RecipeStage.
   enum PostRecipeOutcome {
       /// Normal path — PromptStage, InterceptorStage, ModelStage all run.
       NeedsPrompt(Box<LoopExecutionState>),
       /// Tier 0: rust_items applied, orchestrator_items stashed.
       /// PromptStage, InterceptorStage, AND ModelStage are ALL SKIPPED.
       /// Python scripting engine runs directly with stashed orchestrator context.
       ///
       /// ⚠️ COMP-07: InterceptorStage must ALSO be skipped. It runs between PromptStage
       /// and ModelStage in canonical.rs. The `PostRecipeOutcome::TierZero` arm must jump
       /// past the entire PromptStage + InterceptorStage + ModelStage block, not just
       /// past PromptStage and ModelStage individually. This must be explicit in the
       /// canonical.rs restructuring so InterceptorStage doesn't open a ForensicPacket
       /// for a turn that has no model call to close it.
       TierZero {
           state:             Box<LoopExecutionState>,
           tier0_eligible:    bool,
           llm_call_required: bool,
       },
       /// Action short-circuit: no LLM, no prompt, no Interceptor.
       /// Python step-0 receives pkr["action_short_circuit"] = true.
       ActionExecuted {
           state:        Box<LoopExecutionState>,
           component_id: Uuid,
           name:         String,
       },
   }
   ```

   The canonical loop becomes:
   ```
   let outcome = match recipe_step {
       RecipeStep::Continue { state }        => PostRecipeOutcome::NeedsPrompt(state),
       RecipeStep::TierZero { state, tier0_eligible, llm_call_required }
                                             => PostRecipeOutcome::TierZero { state, tier0_eligible, llm_call_required },
       RecipeStep::ActionExecuted { state, component_id, name }
                                             => PostRecipeOutcome::ActionExecuted { ... },
   };

   match outcome {
       PostRecipeOutcome::NeedsPrompt(state) => {
           // run PromptStage → InterceptorStage → ModelStage (unchanged)
       }
       PostRecipeOutcome::TierZero { state, .. } => {
           // SKIP PromptStage, InterceptorStage, ModelStage, AND CapabilityStage entirely
           // (no ForensicPacket opened; no model call made; CapabilityStage is NOT bent —
           //  it keeps its "react to model output" assumption and is simply not entered).
           //
           // TIER0-GAP resolution (Option 1, chosen — see §H.0 H5): the Python orchestrator
           // is "kicked" by a dedicated TierZeroExecutionStage inserted here, NOT by
           // CapabilityStage. CapabilityStage cannot kick Python — it has no model output to
           // react to in Tier 0. The stage calls the LoopOrchestratorPort host port:
           //
           //   let reply = ctx.host.run_tier_zero(
           //       context,
           //       &state.recipe_hint,          // stashed orchestrator_items (consumed)
           //       &state.recipe_rust_context,   // stashed rust_items (applied server-side)
           //   ).await;
           //   state.recipe_hint = None;          // one-shot consume
           //   state.recipe_rust_context = vec![];
           //   match reply {
           //       Some(tier_zero_reply) => AssistantReplyStage::emit(tier_zero_reply.text),
           //       None => /* NoOrchestrator host — degrade to Tier 2 */ NeedsPrompt(state),
           //   }
           //
           // The composition host implements run_tier_zero by calling
           // `execute_tier_zero_channel(thread, orchestrator_content, rust_context, ...)`
           // — the NEW pub library function in brassclaw_engine::executor::orchestrator
           // (see FIND-NEW-PASS12-02). It is NOT the Python helper
           // `execute_recipe_orchestrator_channel` (that runs inside the Python VM on the
           // engine/Model-A path only). execute_tier_zero_channel applies rust_context,
           // runs the PythonCode channel via __execute_code_step__, and returns formatted text.
           // PromptStage/InterceptorStage/ModelStage/CapabilityStage are ALL skipped.
       }
       PostRecipeOutcome::ActionExecuted { state, .. } => {
           // SKIP PromptStage, InterceptorStage, AND ModelStage entirely
           // Python script already handled the action in step-0
           // AssistantReplyStage emits the result
       }
   }
   ```

5. **Stash / unstash protocol — how RecipeStage and `handle_assemble_prior_knowledge` coordinate (Tier 1):**

   > This is the trickiest coordination point in the whole architecture. Both `RecipeStage`
   > (in the agent loop) and `handle_assemble_prior_knowledge` (inside the Python scripting
   > engine) would naively call `fetch_for_turn`. They must NOT both do a full IBS
   > compilation + component fetch — the stash/unstash protocol prevents the double-fetch.

   > **⚠️ FIND-P9-15 — crate-boundary correction: `handle_assemble_prior_knowledge` is in
   > `brassclaw_engine`, which does NOT depend on `brassclaw_agent_loop`. It CANNOT read
   > `LoopExecutionState.recipe_hint` directly. The correct flow is:**
   >
   > 1. The STAGE (`RecipeStage`, in `brassclaw_agent_loop`) reads `state.recipe_hint` and
   >    passes it as a parameter to `ctx.host.run_step_zero(context, recipe_hint.as_ref())`.
   > 2. The COMPOSITION HOST (`brassclaw_reborn_composition`) implements `run_step_zero`:
   >    it receives `recipe_hint: Option<&serde_json::Value>` as a function parameter and
   >    passes it down to the engine handler.
   > 3. `handle_assemble_prior_knowledge` (in `brassclaw_engine`) receives
   >    `recipe_hint: Option<serde_json::Value>` as a function argument — it NEVER reads
   >    `LoopExecutionState`. It sees the value because the composition host extracted it
   >    from state and passed it through the port boundary.
   > 4. The STAGE clears `state.recipe_hint = None` AFTER `run_step_zero` returns
   >    (not inside the handler — the handler has no `&mut state` access).
   >
   > Any description that says "handler checks state.recipe_hint" is WRONG — the handler
   > does not have access to state. The stage extracts, passes, and clears.

   **The protocol:**

   - **Tier 1 path in `RecipeStage`:**
     1. Calls `fetch_for_turn` → `SplitResult { rust_items, orchestrator_items, routing }`.
     2. Stores `orchestrator_items` serialized as `state.recipe_hint` (JSONB).
     3. Stores `rust_items` serialized as `state.recipe_rust_context` (JSONB).
     4. Returns `RecipeStep::Continue { state }` — does NOT skip PromptStage.

   - **Tier 1 path in the composition host's `run_step_zero`** (called via `LoopOrchestratorPort`):
     1. Receives `recipe_hint: Option<&serde_json::Value>` as a parameter (extracted from
        `state.recipe_hint` by the stage BEFORE the call).
     2. Calls the NEW public library function `brassclaw_engine::executor::orchestrator::assemble_prior_knowledge_with_hint(thread, goal, token_budget, sender_class_code, retrieval_source, recipe_hint.cloned())`.
        > **⚠️ FIND-NEW-PASS12-01 — DO NOT call `handle_assemble_prior_knowledge` directly:**
        > it is `async fn` (private), takes `args: &[MontyObject]` (not `recipe_hint`), and
        > is NOT callable from outside `orchestrator.rs`. The composition host calls
        > `assemble_prior_knowledge_with_hint` — the new `pub` library function that
        > `handle_assemble_prior_knowledge` is refactored to delegate to internally.
     3. `assemble_prior_knowledge_with_hint`: if `recipe_hint` is `Some(v)` → use the stashed value,
        skip `fetch_for_turn` entirely. Deserialize `v` back to `Vec<ComponentItem>` as
        `orchestrator_items`. Format → `orchestrator_content`. Return `PkrAssemblyResult`.
     4. Returns `PriorKnowledgeBundle { orchestrator_content, matched_component_ids, .. }`
        (converted from `PkrAssemblyResult` by the composition host).
   - After `ctx.host.run_step_zero(...)` returns, the STAGE clears:
     ```rust
     state.recipe_hint = None;       // one-shot consume
     state.recipe_rust_context = vec![];
     ```
     (The handler never clears state — it has no `&mut LoopExecutionState` access.)

   **In other words:** For Tier 1, `RecipeStage` is the actual fetcher. The Python handler
   just reads the stash PASSED AS A PARAMETER. There is no double-fetch, no second
   `resolve_intent`, no second IBS compilation.

   - **Tier 0 path:** `RecipeStage` returns `TierZero`. `PromptStage` and `ModelStage` are
     skipped. The Python script still runs (the scripting engine is not the LLM call), but
     it receives `pkr` from `__assemble_prior_knowledge__` which uses the `recipe_hint`
     parameter passed by the composition host (same stash/unstash pattern as Tier 1,
     just the PromptStage/ModelStage stages are absent).

   **Rust state type constraint (repeated for clarity):** `state.recipe_hint` and
  `state.recipe_rust_context` are typed as `serde_json::Value` — NOT `Vec<ComponentItem>`.
  `ComponentItem` is in `brassclaw_engine`; `LoopExecutionState` is in `brassclaw_agent_loop`
  which does NOT depend on `brassclaw_engine`. Serialization to `serde_json::Value` happens
  at `RecipeStage` before storing in state. Deserialization from `Value` happens in
  `assemble_prior_knowledge_with_hint` (which is in `brassclaw_engine` and CAN use `ComponentItem`).

  > **⚠️ DRIVER-GAP cross-reference (invocation — concrete via new `pub` library functions):**
  > the protocol above describes the **server-side** stash/unstash logic. The invocation path
  > goes through the `LoopOrchestratorPort` host port:
  > - Tier 1 step-0: `ctx.host.run_step_zero(context, state.recipe_hint.as_ref())`
  >   → composition host calls `assemble_prior_knowledge_with_hint(thread, goal, ..., recipe_hint.cloned())`
  >   (**NOT** `handle_assemble_prior_knowledge` — see FIND-NEW-PASS12-01).
  > - Tier 0: `ctx.host.run_tier_zero(context, &recipe_hint, &rust_context)` via the
  >   `TierZeroExecutionStage` → composition host calls `execute_tier_zero_channel(thread, ...)`
  >   (**NOT** `execute_recipe_orchestrator_channel` — that's a Python function — see FIND-NEW-PASS12-02).
  > The stash/unstash logic itself is unchanged — only the call boundary is the port.

6. **`PromptStage` / host `build_prompt_bundle`:** `PromptStage` calls
   `ctx.host.build_prompt_bundle(context_request)` — it does NOT call `fetch_for_consumer`
   directly. The recipe hint injection must happen **inside the host's `build_prompt_bundle`
   implementation** (in `brassclaw_turns/src/run_profile/prompt.rs` or the composition layer),
   not in `PromptStage` itself.
   If `PostRecipeOutcome::TierZero`, `PromptStage` and `ModelStage` are skipped entirely
   via the `PostRecipeOutcome` dispatch in `canonical.rs`.

   > **⚠️ FIND-11 — `build_prompt_bundle` does NOT receive `LoopExecutionState`; it receives**
   > **`LoopPromptBundleRequest`. The host cannot "read `state.recipe_hint`" from the request:**
   > `LoopPromptBundleRequest` (in `brassclaw_turns`) is the struct passed to `build_prompt_bundle`.
   > It does not contain `LoopExecutionState`. The host cannot access `state.recipe_hint` unless
   > the field is explicitly included in the request struct.
   >
   > **Concrete fix:** Add `recipe_hint: Option<serde_json::Value>` to `LoopPromptBundleRequest`
   > in `brassclaw_turns/src/run_profile/host.rs` (or wherever `LoopPromptBundleRequest` is
   > defined). In `PromptStage::process`, before calling `ctx.host.build_prompt_bundle(...)`,
   > copy `state.recipe_hint.clone()` into the request field. Do NOT clear `state.recipe_hint`
   > in `PromptStage` — Python step-0's handler clears it (one-shot consume, per COMP-03).
   >
   > **Phase H "Files to modify" must include:**
   > - `crates/brassclaw_turns/src/run_profile/host.rs` (line 987) — add `recipe_hint: Option<serde_json::Value>`
   >   to `LoopPromptBundleRequest`. This is a **breaking change** to the struct.
   >
   > **⚠️ FIND-19 — ALL `LoopPromptBundleRequest` construction sites must add `recipe_hint: None`:**
   > Verified construction sites:
   > - `crates/brassclaw_agent_loop/src/executor/tests/support.rs:340` — `NoInlineContextStrategy::plan_context_request` builds `LoopPromptBundleRequest { mode, context_cursor: None, surface_version: None, checkpoint_state_ref: None, max_messages: Some(16), inline_messages: Vec::new(), capability_view: None }` — must add `recipe_hint: None`.
   > - Any `ContextStrategy::plan_context_request` implementations in the composition layer — grep for `LoopPromptBundleRequest {` in `brassclaw_reborn_composition/src/` before implementing.
   > - Any other crate that constructs `LoopPromptBundleRequest` — run `grep -r "LoopPromptBundleRequest {" crates/` to find all sites.
   > All must add `recipe_hint: None` or the actual hint. Missing sites cause compile errors.
   >   This is a breaking change to the struct.
   > - `crates/brassclaw_agent_loop/src/executor/prompt.rs` — in `PromptStage::process`, set
   >   `request.recipe_hint = state.recipe_hint.clone()` before calling `build_prompt_bundle`.
   > - The composition host's `build_prompt_bundle` implementation — read `request.recipe_hint`
   >   and prepend the stashed orchestrator items to the bundle before the UNION ALL scan.
   >
   > **COMP-03 remains valid:** `build_prompt_bundle` reads the hint but must NOT clear it.
   > Only `assemble_prior_knowledge_with_hint` (called by the composition host in `run_step_zero`)
   > is the one-shot consumer — the stage clears `state.recipe_hint` AFTER `run_step_zero` returns.
   > The hint must be set into the request from `state.recipe_hint` and NOT cleared from state by PromptStage.

   > **⚠️ COMP-03 — recipe_hint consumed by BOTH PromptStage host AND the `run_step_zero` call:**
   > `PromptStage` calls `build_prompt_bundle` which reads `state.recipe_hint` (from the request)
   > to inject the hint into the LLM prompt. Then `PromptStage` calls `run_step_zero` which
   > ALSO reads `state.recipe_hint` (passing it to `assemble_prior_knowledge_with_hint`) and the
   > stage clears it (one-shot consume) after `run_step_zero` returns. There are two readers:
   >
   > - **PromptStage (via `build_prompt_bundle`):** reads `recipe_hint` from the request. Must NOT
   >   clear `state.recipe_hint` — `run_step_zero` still needs it.
   > - **PromptStage (via `run_step_zero` → `assemble_prior_knowledge_with_hint`):** reads, uses,
   >   and the STAGE clears `state.recipe_hint` AFTER `run_step_zero` returns.
   >
   > The protocol requires that `build_prompt_bundle` reads but does NOT consume the hint from state.
   > Only the stage itself (after `run_step_zero` returns) clears `state.recipe_hint`. This ordering constraint
   > must be explicitly stated in Phase H implementation notes: "read in PromptStage via request, clear
   > in stage after run_step_zero only". If both clear it, `run_step_zero` gets `None` and falls through
   > to a second `fetch_for_turn` — defeating the stash/unstash protocol.

#### Tests

- Unit: `last_user_text` populated by `InputStage` after draining input
- Integration: Tier 0 match (wilson ≥ 0.70, `llm_call_required: false`) → `PromptStage` and `ModelStage` skipped
- Integration: Tier 1 match (wilson < 0.70) → orchestrator hint injected, LLM called normally
- Integration: no match → falls through to full LLM (Tier 2 unchanged)
- Integration: Tier 0 success → `record_recipe_outcome(recipe_id, true)` called → wilson_lower updated
- Integration: Tier 0 failure → `record_recipe_outcome(recipe_id, false)` called → tier possibly downgraded
- Unit: `assemble_prior_knowledge_with_hint(thread, goal, budget, sender_class_code, retrieval_source, recipe_hint: None)` → calls `fetch_for_turn`, returns `PkrAssemblyResult` (**FIND-NEW-PASS13-01 — new pub fn must be unit-tested directly**)
- Unit: `assemble_prior_knowledge_with_hint(..., recipe_hint: Some(stashed_orchestrator_items))` → skips `fetch_for_turn`, deserialises stash, returns `PkrAssemblyResult { tier_zero: false, orchestrator_content: <formatted>, ... }`
- Unit: `assemble_prior_knowledge_with_hint(..., recipe_hint: Some(stashed_items), routing.llm_call_required: false)` → returns `PkrAssemblyResult { tier_zero: true }`
- Unit: `execute_tier_zero_channel(thread, orchestrator_content, rust_context, ...)` with valid PythonCode body → returns `TierZeroChannelResult { formatted_output: <text>, matched_component_ids: [...] }` (**FIND-NEW-PASS13-01**)
- Unit: `execute_tier_zero_channel` with empty `orchestrator_content` → returns error result (Q1 Rule 2 bypass guard — same check as `execute_recipe_orchestrator_channel`)
- Unit: `execute_tier_zero_channel` with Skill body in `orchestrator_content` → returns error (Tier-0 must be PythonCode only)
- Unit: `handle_assemble_prior_knowledge` (Python dispatch arm) still works end-to-end after refactor to delegate to `assemble_prior_knowledge_with_hint` (regression — must pass existing tests)

---

### Phase I — Q1 Validator Upgrades

**Status:** [ ] Pending

**File:** `crates/brassclaw_engine/src/memory/component_validator.rs`

> **⚠️ FINDING E — `ComponentPayload` enum has no `PythonCode` or `ExtensionCatalogue` variant:**
> The existing `ComponentPayload` enum in `component_validator.rs` has only three variants:
> `ToolSkill(&'a ToolSkill)`, `Recipe(&'a Recipe)`, and `Generic(GenericComponent<'a>)`.
> Phase I must dispatch classes 22 and 23 through `ComponentPayload::Generic`, NOT through
> a non-existent dedicated variant.
>
> **⚠️ FIND-P10-03 — `GenericComponent` struct shape clarification (corrects an inconsistency
> between Phase I and Phase C COMP-04):**
> The live `GenericComponent` struct (confirmed `component_validator.rs:62–67`) has exactly
> **3 fields**: `{ name: &'a str, description: &'a str, content: &'a str }` — NO `class_code`.
>
> A prior version of Phase I implied that `GenericComponent` needs a `class_code` field for
> the dispatcher to know which class it is handling. **This is wrong.** The validator dispatch
> arm already knows the class because it matched `22 =>` in the dispatch. The class code is
> implicit from the match arm — it does NOT need to be carried in the payload.
>
> **Resolution:** Phase I uses the existing 3-field `GenericComponent` for class 22 (PythonCode)
> — `name`, `description`, and `content` are all that is needed. The only shape extension
> to `GenericComponent` is the one mandated by Phase C COMP-04: add
> `extra: Option<serde_json::Value>` as a 4th field (for class 23's `task_groups` JSONB).
> That single 4th field is sufficient for all current and planned uses. Do NOT add `class_code`.
>
> Updated rule: the `GenericComponent` type carries `{ name, description, content, extra }`.
> The `extra` field is `None` for classes 22 and all other classes that don't need it; it
> is `Some(json!({ "task_groups": [...] }))` for class 23. If the `GenericComponent` shape
> does not yet carry `extra` (i.e. Phase C has not run), extend it as part of Phase C.
> Phase I validation arms rely on `extra` being present (added in Phase C before Phase I runs).

New dispatch cases:

| Class | Rules |
|-------|-------|
| 22 PythonCode | name format, non-empty content, soft 10k token budget, shell-injection scan (see FIND-AUDIT-12 in Phase B for concrete blocked patterns: `import os/subprocess/sys/socket/ctypes/importlib`, `__import__(`, `exec(`, `eval(`, `open(`, `compile(`, `__builtins__`, `globals()`, `locals()`) — dispatched via `ComponentPayload::Generic` |
| 23 ExtensionCatalogue | name format, non-empty `overview_doc`, ≥1 `task_group`, valid UUID syntax in `child_component_ids` — requires extended `GenericComponent` with `extra` field (see COMP-04 in Phase C) |
| 21 Recipe (StepDescriptions) | call `instruction_builder::build_instruction` as pre-flight; reject on any `IbsError` with the parse message; all `include` UUIDs parse as UUID v4; no `snippet`-type steps; step numbers monotonically increasing; S7 guard |
| 1–3 Skills | `intent_examples` entries ≤ 512 chars, capped at 20; `dependency_registry` entries must have valid UUID syntax and non-empty `label` |
| 16 Actions | `steps` JSONB validated against 13 known step types |

**The `link_formula` / `step_link` parse check uses the same `parse_step_link` function as
runtime.** Any parse error that would blow up at runtime becomes a Q1 error with the
full parse message. This is the primary correctness guard for formula authoring.

**§shell-guard — Recipes referencing `builtin.shell` or `builtin.spawn_subagent`:**
If any `rust_steps[].include` UUID resolves to a ToolSkill whose `tool_name` is
`"builtin.shell"` or `"builtin.spawn_subagent"`, the Recipe **must** have
`llm_call_required: true`. Q1 returns a hard error if `llm_call_required: false` and
either tool appears in the rust channel. This prevents open-ended shell/spawn from
accidentally becoming a Tier 0 path.

**§tier0-orchestrator-channel — Tier-0 recipes MUST have an orchestrator supervisor:**

> **⚠️ ARCHITECTURAL CORRECTION (user review):** A prior draft of this section claimed that
> "most Tier-0 recipes have empty `orchestrator_steps`" and that the Rust executor can run
> autonomously from ToolBindings alone. **This is wrong.** The orchestrator is always the
> supervisory layer. Even in Tier 0 it is the Python orchestrator (`execute_recipe_orchestrator_channel`)
> that calls `__execute_action__` on the Rust executor — not the Rust executor acting on its own.
> The Rust channel (`rust_steps`) pre-loads *which* ToolSkills are available and *with which
> params*, but it is the orchestrator's PythonCode body that actually issues the call.
> A recipe with `rust_steps` that has `tool_bindings` AND empty `orchestrator_steps` has
> a loaded gun that nobody fires. **The S7 guard must cover this case.**

**Architectural invariant — the orchestrator always supervises:**

```
user message
  → orchestrator (Python, default.py)
    → step 0: __assemble_prior_knowledge__ → intent match → recipe fetched
      [rust channel pre-loads ToolSkill context into Rust executor — silent]
      [orchestrator channel → orchestrator_content delivered to Python]
    → execute_recipe_orchestrator_channel(pkr, goal, state):
        PythonCode body calls __execute_action__(tool_name, params)
        → Rust executor runs tool with pre-loaded ToolSkill binding
        → result returned to orchestrator
        PythonCode formats result → assigns to `result`
    → orchestrator presents result to user
```

The Rust executor **never acts without an orchestrator call**. `rust_steps` pre-loads
context; `orchestrator_steps` (PythonCode) directs and presents.

**Design rationale (why Skill bodies cannot replace PythonCode in Tier 0):**
- In **Tier 1** (`llm_call_required: true`): the LLM reads the Skill body (narrative prose) from
  `orchestrator_content` and uses it as instructions to decide *if and how* to call tools.
  The LLM is the interpreter of "pass the directory path from the user's message to the ls tool."
- In **Tier 0** (`llm_call_required: false`): there is NO LLM. Nobody interprets the Skill body.
  `execute_recipe_orchestrator_channel` must act directly — it can `exec()` a PythonCode body
  but cannot "interpret" narrative prose without an LLM round-trip, defeating Tier 0's purpose.
  PythonCode is the deterministic replacement for the LLM's supervisory role.

**What a Tier-0 recipe's orchestrator PythonCode does:**

It is not just "post-processing." It is the full supervisory step:
1. Call `__execute_action__` with the tool name and the pre-extracted `vars` params.
2. Receive the Rust executor's result.
3. Format the result for the user.
4. Assign to `result`.

Example — `builtin-read-file` (Tier 0):
```python
# PythonCode body: "read-file-executor"
#
# ⚠️ FIND-NEW-PASS13-04 — There is NO runtime `vars` dict in the PythonCode body scope.
# Template variable substitution (Phase M / §0.20.3) is done by the IBS BEFORE
# execute_recipe_orchestrator_channel / execute_tier_zero_channel runs. The IBS replaces
# {{vars.slot0}} with the literal extracted value in the body TEXT at assembly time.
# By the time this body executes, the path is already baked in as a literal string.
# (See §0.20.3 — a body that accesses a `vars` dict at runtime will get a NameError.)
#
# The AUTHORED recipe body template (before IBS substitution):
#   tool_output = __execute_action__("read_file", {"path": "{{vars.slot0}}"})
#   result = tool_output
#
# What the PythonCode body looks like AT RUNTIME (after IBS substitution):
tool_output = __execute_action__("read_file", {"path": "/tmp/foo.txt"})  # literal, baked in by IBS
result = tool_output  # raw file content, or format it as needed
```

The ToolBinding in `rust_steps` pre-stages *that* `read_file` is available and *which params
pattern* it uses — but the PythonCode above is what actually runs it. Without the PythonCode,
the ToolBinding is never invoked.

> **⚠️ FIND-P9-02 + S7-EXTENSION — Q1 rules for Tier-0 orchestrator channel:**
>
> **Rule 1 (Skill forbidden):** `llm_call_required = false` AND any UUID in
> `orchestrator_steps[].include` resolves to class 1, 2, or 3 (Skill) → **Q1 hard error**:
> `"Tier-0 recipes may only reference PythonCode (class 22) in orchestrator_steps. Found
> Skill '{name}'. Either replace with PythonCode, or set llm_call_required: true."`.
>
> **Rule 2 (S7-extension — PythonCode required when rust_steps has tool_bindings):**
> `llm_call_required = false` AND `rust_steps` contains any step with non-empty
> `tool_bindings` AND `orchestrator_steps` is empty → **Q1 hard error**:
> `"Tier-0 recipe has tool_bindings in rust_steps but no PythonCode in
> orchestrator_steps. The orchestrator must supervise tool execution — add a PythonCode
> component that calls __execute_action__ and formats the result."`.
> *(Extension of the existing S7 guard for the Tier-0 case specifically.)*
>
> **Rule 3 (no tool_bindings → empty orchestrator_steps is valid):** If `rust_steps` has
> no `tool_bindings` (i.e. the Rust channel only pre-loads ToolSkill context but issues
> no specific parameterised call), then `orchestrator_steps` may be empty — the PythonCode
> in `orchestrator_steps` is only mandatory when there is a tool call to supervise. This
> edge case may not arise in practice for built-in recipes but is architecturally valid.
>
> **Scope:** `rust_steps` may contain ToolSkills freely — they are the pre-loaded execution
> context. Only `orchestrator_steps` component UUIDs are class-checked. Rule 2 enforces
> that a tool_binding always has an orchestrator supervisor.

**§capability-id — Tool rows from builtin bootstrap:**  
For class 0 (Tool) components with `source = "system"`, Q1 validates that `capability_id`
is non-empty and matches the pattern `^[a-z0-9_-]+\.[a-z0-9_.]+$` (e.g. `builtin.read_file`).
Tool rows without a `capability_id` that are authored (not system) pass without error —
`capability_id` is optional for user-authored custom tools.

**§template-rules — Intent expression templates (applies to all component classes):**  
Q1 runs `parse_template` against every intent expression in `intent_examples`. Rules:

| Condition | Severity |
|-----------|----------|
| `template_prefix = ''` AND `template_suffix = ''` — e.g. `"% in %"`, `"%"` | **Hard error** — no anchor; add literal text around each `%` |
| Two `%` with no literal text between them — e.g. `"search % %"` | **Hard error** — adjacent slots are unextractable |
| `template_prefix = ''` AND `template_suffix != ''` — e.g. `"% directory"` | **Warning** — leading-`%` is valid and indexed via suffix; consider adding a word before `%` for precision |
| `{{vars.name}}` in ToolBinding `params` but no `%` in any expression AND no `variable_patterns` for that name | **Hard error** — variable referenced but no source defined |
| `variable_patterns` entry whose `name` does not appear in any `{{vars.name}}` reference | **Warning** — pattern defined but never used |

#### Tests

- Unit: Recipe with `snippet`-type step → Q1 fail with `IbsError::UnpromotedSnippet`
- Unit: Recipe with unparseable UUID in `include` → Q1 fail
- Unit: Recipe with S7 violation → Q1 fail
- Unit: PythonCode with shell-injection pattern → Q1 fail
- Unit: PythonCode with >10k tokens → Q1 warn (soft limit)
- Unit: ExtensionCatalogue with empty `overview_doc` → Q1 fail
- Unit: Skill with `intent_examples` entry > 512 chars → Q1 fail
- Unit: valid StepDescriptions, valid `step_link` → Q1 pass
- Unit: §shell-guard: Recipe with `builtin.shell` ToolSkill in rust channel + `llm_call_required: false` → Q1 fail
- Unit: §shell-guard: Recipe with `builtin.spawn_subagent` + `llm_call_required: false` → Q1 fail
- Unit: §shell-guard: Recipe with `builtin.shell` + `llm_call_required: true` → Q1 pass
- Unit: §tier0-orchestrator-channel Rule 1: Recipe `llm_call_required: false` + Skill (class 1) in `orchestrator_steps` → Q1 hard error
- Unit: §tier0-orchestrator-channel Rule 1: Recipe `llm_call_required: false` + PythonCode (class 22) in `orchestrator_steps` → Q1 pass
- Unit: §tier0-orchestrator-channel Rule 2 (S7-extension): Recipe `llm_call_required: false` + non-empty `tool_bindings` in `rust_steps` + empty `orchestrator_steps` → Q1 hard error (orchestrator must supervise)
- Unit: §tier0-orchestrator-channel Rule 2 satisfied: Recipe `llm_call_required: false` + non-empty `tool_bindings` + PythonCode in `orchestrator_steps` → Q1 pass
- Unit: §tier0-orchestrator-channel Rule 3: Recipe `llm_call_required: false` + no `tool_bindings` in `rust_steps` + empty `orchestrator_steps` → Q1 pass (nothing to supervise)
- Unit: §tier0-orchestrator-channel: Recipe `llm_call_required: true` + Skill UUID in `orchestrator_steps` → Q1 pass (Tier 1 recipes may use Skills)
- Unit: §capability-id: Tool row `source = "system"`, empty `capability_id` → Q1 fail
- Unit: §capability-id: Tool row `source = "authored"`, no `capability_id` → Q1 pass (optional for user tools)

---

### Phase J — Skill `intent_examples` + Dependency Registry

**Status:** [ ] Pending

> **§0.23.4 fold-in (J.2):** V055 also adds `formatted_content TEXT` (nullable) to
> all 13 component tables — the persisted LLM-formatted version computed at save
> time by the per-class formatter PythonCode (§0.23.4). J.2 also builds the
> **lightweight in-process sandboxed PythonCode executor** (single-component run,
> no orchestrator loop, no tool dispatch, no network) that the formatters use; if
> no such executor exists today, J.2 creates it (confirm against live source). The
> per-class formatter PythonCode *components* themselves are seeded in Phase L.

**Note:** `required_skills` does not exist. Dependencies between components are expressed
via each component's `dependency_registry` JSONB and step-level traversal expressions (§0.19).
Phase J covers two concerns: (1) Skill intent_examples seeding, and (2) `dependency_registry`
column on all component tables.

#### J.1 Skill `intent_examples` seeding

Intent examples already live on `reborn_skills.intent_examples` (JSONB array of
`{input, class}` objects, `class` ∈ 1|2|3) — added in `V027__reborn_skills.sql:67` with a
GIN index at `V027:139–141`. They are authored via the **skill store** input structs
(`CreateSkillInput`/`UpdateSkillInput`, `db_store.rs:167`/`:207`), validated to the
`{input, class}` shape at `db_store.rs:348–371`. **Migration V055's `ADD COLUMN IF NOT
EXISTS intent_examples` would be a NO-OP** (the §2 table already flags this, and per
FIND-12 the no-op line has been removed from V055 entirely; the file is named
`V055__reborn_dependency_registry.sql`) — do not expect a column to be created.

The genuinely missing piece is **propagation into the intent table**: today
`reborn_skills.intent_examples` never reaches `reborn_intent_inputs`, so `resolve_intent`
cannot match skill intents. J.1 wires that propagation:

- **Do NOT add `intent_examples: Vec<String>` to `SkillManifest`** (`types.rs:127`).
  `SkillManifest` has no such field today, and `Vec<String>` is **shape-incompatible**
  with the existing DB column (which stores `{input, class}` objects, dropping the
  `class` 1|2|3 the schema/validator require). Keep intent examples **DB-only** via the
  skill store API (the existing authoring surface). (If SKILL.md-frontmatter authoring is
  later desired, it must use `{input, class}` objects — not `Vec<String>` — so the
  manifest ↔ DB round-trip is consistent; that is a separate, optional change, not J.1.)
- On skill `auto_passed` transition (Q1 auto-pass in the validation queue): call
  `seed_intent_input` (`intent_system.rs:462`, writes `reborn_intent_inputs`, the V028
  table `resolve_intent` queries) for each `{input, class}` intent expression, so skill
  intents become matchable. `seed_intent_input` is not called from `brassclaw_skills`
  today (only `intent_system.rs` / `pg_intent_inputs_store.rs` reference it) — this is
  the new wiring. **Ordering:** `auto_passed` is a queue state from the Phase N gate
  logic (V059); this wiring therefore lands after Phase N's queue/gate is in place (or,
  pre-Phase-N, hook the existing `validation_status='validated'` transition as the
  interim trigger).
- On skill wipe/delete: call `purge_component_inputs(component_id)` (remove its rows
  from `reborn_intent_inputs`).

> **✅ Review note (pre-v3 audit) — J.1 conflates three things; the migration is a no-op and the
> `SkillManifest` shape is wrong — RESOLVED:** the J.1 body above has been rewritten to match
> this audit: the migration is stated as a NO-OP, the `SkillManifest` `Vec<String>` change is
> dropped as shape-incompatible (intent examples stay DB-only via the store API), and the real
> wiring (`auto_passed → seed_intent_input` + `purge_component_inputs` on delete) is retained.
> Original audit detail retained below for traceability:
> 1. **Migration V055 `ADD COLUMN IF NOT EXISTS intent_examples` was originally planned but is a NO-OP (FIND-12 removed it).** `reborn_skills`
>    already has an `intent_examples JSONB NOT NULL DEFAULT '[]'` column — added in
>    `V027__reborn_skills.sql:67`, with a GIN index at `V027:139–141`. It stores an array of
>    `{input, class}` objects (the `class` is 1|2|3), **not** an array of strings. The §2 migration
>    table already flags V054 as a no-op; J.1 should state it explicitly so the implementer does
>    not expect a column to be created.
> 2. **`SkillManifest` (`crates/brassclaw_skills/src/types.rs:127`) has NO `intent_examples`
>    field** today. Intent examples are authored via the **skill store** input structs
>    (`CreateSkillInput`/`UpdateSkillInput` in `db_store.rs:167`/`:207`) as `intent_examples:
>    JsonValue`, validated to the `{input, class}` shape at `db_store.rs:348–371` and persisted at
>    `db_store.rs:463/505`. So intent examples are **DB-only metadata set through the store API**,
>    not a SKILL.md frontmatter field. Adding `intent_examples: Vec<String>` to `SkillManifest`
>    would (a) introduce a *new* SKILL.md authoring surface that doesn't exist today, and
>    (b) be **shape-incompatible** with the existing DB column — `Vec<String>` drops the `class`
>    1|2|3 field that the schema and validator require. Reconcile before implementing: either keep
>    intent examples DB-only (drop the `SkillManifest` change entirely) or, if SKILL.md
>    authoring is desired, use `{input, class}` objects (not `Vec<String>`) so the manifest ↔ DB
>    round-trip is consistent.
> 3. **The genuinely missing wiring is the propagation
>    `reborn_skills.intent_examples` → `reborn_intent_inputs`.** `seed_intent_input`
>    (`intent_system.rs:462`, writes to `reborn_intent_inputs`, the V028 table that
>    `resolve_intent` actually queries) is **not called from `brassclaw_skills` today** (verified:
>    the only references are `intent_system.rs` and `pg_intent_inputs_store.rs`). So skill intent
>    examples sit on the skill row but never reach the intent-inputs table — `resolve_intent`
>    cannot match them until J.1 wires the `auto_passed` → `seed_intent_input` call. This is the
>    correct core of J.1; the `SkillManifest`/`Vec<String>` addition is a separable authoring
>    question (point 2) and the migration is a no-op (point 1).

> **⚠️ FIND-12 — The no-op `intent_examples` line should be dropped from V055. The migration
> file should be named for its actual work: `V055__reborn_dependency_registry.sql`.** (V-number
> updated per Decision 2: was V054, now V055.)
> Including a NO-OP ALTER in a production migration file adds noise to migration history
> and confuses reviewers. Since `IF NOT EXISTS` makes it harmless, it won't break anything,
> but it should be removed for clarity. The migration's name should reflect its real content.

**Migration V055 (was V054 before Decision 2; file: `V055__reborn_dependency_registry.sql`):**
```sql
-- V055__reborn_dependency_registry.sql
-- Adds dependency_registry JSONB to all 13 component tables (§0.19 / Phase J.2).
-- Note: reborn_skills.intent_examples already exists (V027) — no-op omitted.
-- reborn_python_code (V052) and reborn_extension_catalogues (V053) include this
-- column at creation time — no ALTER needed here for those tables.
-- reborn_recipes ALREADY has dependency_registry from V050 (Phase A — see
-- VARPAT-COL-GAP / DEPREG-TIMING-GAP note); its line below is `IF NOT EXISTS`
-- → idempotent no-op, kept for the single-migration-covers-all-tables invariant.
```

#### J.2 `dependency_registry` column on all component tables

Add `dependency_registry JSONB` to every component table that participates in dependency
traversal. This is a nullable column — components with no declared dependencies have
`dependency_registry = NULL` or `[]`.

**Tables to add the column to** (one ALTER TABLE per table; can be a single migration):
`reborn_skills`, `reborn_tools`, `reborn_tool_skills`, `reborn_recipes`, `reborn_actions`,
`reborn_specs`, `reborn_plans`, `reborn_summaries`, `reborn_lessons`, `reborn_docus`,
`reborn_issues`, `reborn_notes`, `reborn_extensions_unified`.
> **⚠️ EXT-NAME — the extensions table is `reborn_extensions_unified`, not
> `reborn_extensions`.** Verified `V032__reborn_extensions_unified.sql:57`:
> `CREATE TABLE IF NOT EXISTS reborn_extensions_unified`. A prior draft of this list
> wrote the bare `reborn_extensions`, which would make the V055 `ALTER TABLE` (and the
> V059 `DROP COLUMN`) fail with `relation "reborn_extensions" does not exist`. Use the
> real name `reborn_extensions_unified` in every per-table ALTER/DROP. (The PERF-03
> sub-select list at line 67 and the populate UNION ALL at line 4801 already use the
> correct `reborn_extensions_unified` name.)

New tables (Phases B, C) include the column from creation: `reborn_python_code`,
`reborn_extension_catalogues`.

**Migration V055** (same file, additional statements — **was V054 before Decision 2**):
```sql
ALTER TABLE reborn_skills        ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_tools         ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_tool_skills   ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
ALTER TABLE reborn_recipes       ADD COLUMN IF NOT EXISTS dependency_registry JSONB;
-- ... repeated for all 13 tables
```

#### J.3 `resolve_dependencies` in `fetch_for_turn`

**File:** `crates/brassclaw_engine/src/memory/retrieval_source.rs`

After the IBS compiles a `BuildInstruction`, `fetch_for_turn` calls
`resolve_dependencies` for each `RecipeStep` that carries a non-empty `DependencyExpr`:

```rust
async fn resolve_dependencies(
    pool: &PgPool,
    root_component_id: Uuid,
    expr: &DependencyExpr,
    visited: &mut HashSet<Uuid>,
) -> Result<Vec<ComponentItem>, RetrievalSourceError>
```

Implements the algorithm from §0.19. Results are partitioned into rust/orchestrator
channels by `class_code` and merged into the corresponding `SplitResult` item lists.

> **Cross-reference (class 22/23 dependency fetches):** `resolve_dependencies` loads
> each dependency component via `fetch_component_by_id`, which dispatches on
> `class_code` to pick the table + content expression (the `table_and_content` match).
> Class 22 (`reborn_python_code`) and 23 (`reborn_extension_catalogues`) arms are
> added in **Phase E** (`retrieval_source.rs` — see the `22 => Some(("reborn_python_code",
> ...))` / `23 => Some(("reborn_extension_catalogues", ...))` arms), and the matching
> `class_label` arms (`22 => "python_code"`, `23 => "extension_catalogue"`) are added in
> **Phase B / Phase C** (`intent_system.rs`). A StepDescription `dependencies` traversal
> that resolves to a class-22/23 component therefore works only once Phase E + B/C have
> landed (J.3 runs after all of A–E by phase order, so this is satisfied). No new arm is
> added in J.3 itself — J.3 only consumes the Phase E/B/C arms.

#### Tests

- Unit: skill store `CreateSkillInput`/`UpdateSkillInput` `intent_examples` round-trips
  `{input, class}` objects (validated to the `{input, class}` shape at `db_store.rs:348–371`);
  a `Vec<String>`-shaped value is rejected (shape-incompatible — see J.1; `SkillManifest`
  has no `intent_examples` field)
- Unit: `intent_examples` entry > 512 chars → rejected by the store validator
- Integration: Skill with `intent_examples` → `auto_passed` calls `seed_intent_input` →
  resolves via `resolve_intent` (the J.1 wiring)
- Integration: `resolve_dependencies` with `"1[all]"` → full transitive closure fetched
- Integration: `resolve_dependencies` with `"5[2,6]"` → only indices 2 and 6 fetched, no sub-deps
- Integration: `resolve_dependencies` — UUID already in `visited` → skipped (deduplication)
- Integration: `resolve_dependencies` with cycle in registries → cycle node skipped (visited guard)
- Integration: dependency components routed to correct channel by class_code

---

### Phase K — BasicPromptStore + Prefix Tab + MCP Translation + Cleanup

**Status:** [ ] Pending

> **§0.23.6 + §0.23.7 + §0.23.8 fold-in:** Phase K also absorbs three §0.23 items:
> (1) **Sempai auto-creates all component types** — generalise `SempaiReviewOutcome`
> (`packet.rs`) and `SempaiProposalSink::submit_proposals` (`proposal_sink.rs`) beyond
> recipes to tools/tool_skills/skills/extensions/python_code/catalogues/intents;
> `PgSempaiProposalSink` inserts into the correct class table (class→table dispatch);
> route WebUI saves to the queue with **no direct production write** (new → `'pending'`
> + queue row; edit of validated → copy + `proposed_payload`, live row stays validated).
> (2) **Kohai prompt store** — extend `PgInterceptorStore`/`PromptSegment` to capture
> **component UUID references** (additive ALTER **folded into `V056`**, not a separate
> `V062` — confirm vs live schema; see §0.23.10 ordering note for the refinery
> ascending-order constraint) + add a **6-week retention sweep**. (3) **Idle
> self-improvement sweep** — in-process background task, idle ≥ 2h AND after 15:00
> local, once/day: reassemble prompts + chat history by reference → Sempai
> component-creation query → proposals enter Q1; config cols on
> `reborn_monty_vm_settings` (additive, **folded into `V056`**, not a separate `V063`).

#### K.1 BasicPromptStore + Prefix Tab (UI migration from Interceptor tab)

> **Grounded in:** `17-webui-prefix-tab.md`, `saved_plan_to_v3.md §0.13`, user item 8 (Prefix Tab),
> item 5.1 (SKILL.md export). This phase delivers: the `reborn_basic_prompt_store` table,
> `PgBasicPromptStore` facade, prefix-named routes, the Prefix Tab UI, migration of the
> Reassemble+Pre-warm ControlCard out of the Interceptor tab, `mark_stale` on Q2 graduation,
> the `base-prompt` placeholder substitution wiring, and the SKILL.md on-demand export endpoint.

##### K.1.1 Database migration

**Migration V056** (**was V055 before Decision 2**; file: `V056__reborn_basic_prompt_store.sql`).

> **⚠️ §0.23.7 + §0.23.8 fold-in — V056 is Phase K's SINGLE migration.** Beyond
> `reborn_basic_prompt_store` (below), V056 **also** carries: (a) the component-UUID
> reference column(s) on the interceptor packet/segment store (§0.23.7 — confirm exact
> shape against the live `PgInterceptorStore` schema at Phase K); (b) the
> `reborn_monty_vm_settings` validation-improve cols (§0.23.8:
> `validation_idle_threshold_minutes INT NOT NULL DEFAULT 120`,
> `validation_improve_start_hour INT NOT NULL DEFAULT 15`,
> `validation_improve_enabled BOOLEAN NOT NULL DEFAULT true`). These are **not** split
> into separate `V062`/`V063` files — refinery applies migrations in strict ascending
> order and the embedded PG data dir is persistent across boots, so a `V062`/`V063` in
> Phase K (sort_order 12) would silently skip `V057`–`V061` (Phases L–P.0). See §0.23.10
> ordering note. Append the ALTERs for (a) and (b) to this same `V056__…sql` file after
> the `CREATE TABLE` + indexes below.

```sql
CREATE TABLE reborn_basic_prompt_store (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     TEXT NOT NULL,
    -- user_id / agent_id / project_id are nullable (scoped to the authenticated caller;
    -- NULL means "any user / any agent / any project" for a tenant-wide entry).
    user_id       TEXT,
    agent_id      TEXT,
    project_id    TEXT,
    fingerprint   TEXT NOT NULL,   -- SHA-256 hex of bundle content
    bundle_json   JSONB NOT NULL,  -- rendered bundle string stored as JSON string value
    is_stale      BOOLEAN NOT NULL DEFAULT false,
    assembled_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    prewarm_last_at TIMESTAMPTZ,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- NULLS NOT DISTINCT requires Postgres 15+.  The codebase already requires
    -- Postgres 14+ (embedded PG is bundled). If Postgres 15 is not guaranteed,
    -- replace the nullable columns with NOT NULL DEFAULT '' and store empty string
    -- for "absent" scope component (simpler upsert, no NULL distinctness issue).
    -- Decision: use NOT NULL DEFAULT '' for all three scope columns to avoid
    -- UNIQUE NULL semantics entirely — this is the correct approach for a
    -- single-row-per-scope guarantee.
    CONSTRAINT reborn_basic_prompt_store_scope_unique
        UNIQUE (tenant_id, user_id, agent_id, project_id)
);
CREATE INDEX reborn_basic_prompt_store_scope_idx
    ON reborn_basic_prompt_store (tenant_id, user_id, agent_id, project_id);
```

> **⚠️ DDL correction — nullable scope columns + UNIQUE constraint:** The original design above uses nullable `user_id`/`agent_id`/`project_id` with a `UNIQUE` constraint, but Postgres treats `NULL ≠ NULL` in unique constraints — two rows with `(tenant_1, NULL, NULL, NULL)` would both satisfy the constraint. **The correct DDL is to declare all scope columns `TEXT NOT NULL DEFAULT ''`** and store empty string when the caller's scope component is absent. This is the same pattern `PgMontyVmSettingsStore` uses (it receives `user_id: &str` and `project_id: &str` per-call with empty-string fallbacks). The constraint name `reborn_basic_prompt_store_scope_unique` is used in the `ON CONFLICT` clause of the `store()` upsert.
>
> **Corrected DDL:**
> ```sql
> CREATE TABLE reborn_basic_prompt_store (
>     id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
>     tenant_id       TEXT NOT NULL DEFAULT '',
>     user_id         TEXT NOT NULL DEFAULT '',
>     agent_id        TEXT NOT NULL DEFAULT '',
>     project_id      TEXT NOT NULL DEFAULT '',
>     fingerprint     TEXT NOT NULL,
>     bundle_json     JSONB NOT NULL,
>     is_stale        BOOLEAN NOT NULL DEFAULT false,
>     assembled_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
>     prewarm_last_at TIMESTAMPTZ,
>     updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
>     CONSTRAINT reborn_basic_prompt_store_scope_unique
>         UNIQUE (tenant_id, user_id, agent_id, project_id)
> );
> CREATE INDEX reborn_basic_prompt_store_scope_idx
>     ON reborn_basic_prompt_store (tenant_id, user_id, agent_id, project_id);
> ```
> Use the corrected DDL in the migration file. The `PgBasicPromptStore` implementation passes `caller.user_id.as_str()`, `caller.agent_id.as_ref().map(|a| a.as_str()).unwrap_or("")`, and `caller.project_id.as_ref().map(|p| p.as_str()).unwrap_or("")` for the scope parameters.

> **Note:** `prewarm_last_at` is added here (not in the existing `brassclaw_config` key), since the new store owns the full compile state. The `bundle_json` JSONB column stores the rendered bundle string as a JSON string value (not an object), matching the existing `do_reassemble` output type.

##### K.1.2 `PgBasicPromptStore` facade

**File:** `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs` (new)

> **Pattern:** Follow `PgMontyVmSettingsStore` exactly: gated under `#[cfg(feature = "postgres")]`, constructed with fixed `tenant_id` + `agent_id` at composition time (not per-call), errors mapped through `thiserror` in `error.rs`, uses `brassclaw_pg::PgPool` + `deadpool_postgres` + `tokio_postgres` (not `sqlx`). `user_id` and `project_id` are passed as `&str` parameters per-call, consistent with how `PgMontyVmSettingsStore::get(user_id, project_id)` works.

```rust
/// A stored prefix entry.
pub struct BasicPromptEntry {
    pub id:              uuid::Uuid,
    pub fingerprint:     String,       // SHA-256 hex of bundle string
    pub bundle:          String,       // the rendered bundle string (stored in bundle_json JSONB)
    pub is_stale:        bool,
    pub assembled_at:    chrono::DateTime<chrono::Utc>,
    pub prewarm_last_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at:      chrono::DateTime<chrono::Utc>,
}

/// Composition-side Postgres-backed store for the pre-assembled base-prompt bundle.
/// One row per `(tenant_id, user_id, agent_id, project_id)` scope.
pub(crate) struct PgBasicPromptStore {
    pool:      Arc<PgPool>,
    tenant_id: String,
    agent_id:  String,
}

impl PgBasicPromptStore {
    pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>, agent_id: impl Into<String>) -> Self;

    /// Return the stored entry for this scope, or `None` if no prefix has been compiled yet.
    pub async fn get_for_scope(&self, user_id: &str, project_id: &str) -> Result<Option<BasicPromptEntry>, BasicPromptStoreError>;

    /// Upsert the compiled bundle for this scope.
    /// `fingerprint` = hex-encoded `sha256(bundle_bytes)` (computed before calling).
    /// Sets `is_stale = false`, `assembled_at = now()`, `updated_at = now()`.
    /// SQL: `INSERT INTO reborn_basic_prompt_store (...) VALUES (...) ON CONFLICT ON CONSTRAINT
    ///   reborn_basic_prompt_store_scope_unique DO UPDATE SET bundle_json=EXCLUDED.bundle_json,
    ///   fingerprint=EXCLUDED.fingerprint, is_stale=false, assembled_at=now(), updated_at=now()`
    pub async fn store(&self, user_id: &str, project_id: &str, fingerprint: &str, bundle: &str) -> Result<BasicPromptEntry, BasicPromptStoreError>;

    /// Set `is_stale = true`, `updated_at = now()` for the scope row.
    /// No-op (returns `Ok(())`) if no row exists for the scope.
    /// Called after every component Q2 graduation (side effect 4 of §0.15).
    /// SQL: `UPDATE reborn_basic_prompt_store SET is_stale=true, updated_at=now()
    ///       WHERE tenant_id=$1 AND user_id=$2 AND agent_id=$3 AND project_id=$4`
    pub async fn mark_stale(&self, user_id: &str, project_id: &str) -> Result<(), BasicPromptStoreError>;

    /// Record that a prewarm succeeded: `UPDATE ... SET prewarm_last_at=now(), updated_at=now()`.
    pub async fn record_prewarm(&self, user_id: &str, project_id: &str) -> Result<(), BasicPromptStoreError>;

    /// Delete the stored entry for this scope.
    pub async fn delete(&self, user_id: &str, project_id: &str) -> Result<(), BasicPromptStoreError>;
}
```

**`BasicPromptStoreError`** (in `error.rs`):
```rust
#[derive(Debug, thiserror::Error)]
pub enum BasicPromptStoreError {
    #[error("db pool: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("db query: {0}")]
    Pg(#[from] tokio_postgres::Error),
}
```

**Fingerprint computation:** `hex::encode(sha2::Sha256::digest(bundle.as_bytes()))`. Add `sha2` and `hex` as dependencies to `brassclaw_reborn_composition/Cargo.toml` if not already present (`sha2 = "0.10"`, `hex = "0.4"`).

**Wiring into `RebornInterceptorConfigService`:** Add a `pg_basic_prompt_store: Option<Arc<PgBasicPromptStore>>` field to `RebornInterceptorConfigService` (alongside `sempai_gateway`), added via a `with_basic_prompt_store(store)` builder. The store is constructed in `crates/brassclaw_reborn_composition/src/webui.rs` alongside `RebornInterceptorConfigService` itself, then passed in via the builder. `webui.rs` is the composition file that assembles `RebornServices` from all sub-services and stores — it is the correct place to instantiate `PgBasicPromptStore` and wire it in.

##### K.1.3 New prefix-named routes (Rust)

> The existing `/interceptor/reassemble` and `/interceptor/prewarm` routes are **removed** and replaced by the prefix-named routes below. The Interceptor tab keeps `GET /interceptor/config` and `POST /interceptor/config` (mode + persona), but loses the compile actions.

> **Pattern:** All descriptor builder functions follow the codebase pattern: private `fn X_descriptor() -> IngressRouteDescriptor` using the `descriptor(route_id, method, pattern, policy)` helper and the `mutation_policy(...)` / `read_policy(...)` helpers. Route IDs are `pub const WEBUI_V2_ROUTE_*: &str` constants. Route patterns use `{name}` curly-brace syntax (not Axum `:name` colon syntax) — e.g. `"/api/webchat/v2/prefixes/{name}/regenerate"`. All new route ID constants must be exported from `crates/brassclaw_webui_v2/src/lib.rs` alongside existing ones.

**Files to modify:** `crates/brassclaw_webui_v2/src/descriptors.rs`, `crates/brassclaw_webui_v2/src/handlers.rs`, `crates/brassclaw_webui_v2/src/lib.rs`, `crates/brassclaw_webui_v2/src/router.rs`

**`InterceptorConfigService` trait** (`crates/brassclaw_product_workflow/src/reborn_services/interceptor_config.rs`):
- **Remove** `reassemble_base_prompt` and `prewarm` methods from the trait.
- **Add** `list_prefix_entries` and `regenerate_prefix` methods to the trait:

```rust
/// List prefix cache entries for the caller's scope (Phase K.1).
async fn list_prefix_entries(
    &self,
    caller: WebUiAuthenticatedCaller,
) -> Result<PrefixListResponse, InterceptorConfigServiceError>;

/// Assemble + prewarm the named prefix (Phase K.1). Rate-limited to 1/min per caller.
async fn regenerate_prefix(
    &self,
    caller: WebUiAuthenticatedCaller,
    name: String,
) -> Result<PrefixRegenerateResponse, InterceptorConfigServiceError>;
```

**`InterceptorConfigServiceError`** — add `PrefixNotFound` variant (in `interceptor_config.rs`):
```rust
#[error("interceptor: prefix not found: {name}")]
PrefixNotFound { name: String },
```

**`map_interceptor_config_error`** — add a `PrefixNotFound` arm (maps to HTTP 404):
```rust
InterceptorConfigServiceError::PrefixNotFound { .. } => {
    crate::RebornServicesError::from_status(
        crate::RebornServicesErrorCode::NotFound,
        404,
        false,
    )
}
```

**New DTO `pub use` additions** in `crates/brassclaw_product_workflow/src/reborn_services.rs`, in the `pub use interceptor_config::{...}` block (currently at ~line 105–108): add `PrefixEntry`, `PrefixListResponse`, `PrefixRegenerateResponse` to the exported set.

**New route ID constants and pattern constants** (add to `descriptors.rs` alongside existing ones):
```rust
pub const WEBUI_V2_ROUTE_LIST_PREFIXES:    &str = "webui.v2.list_prefixes";
pub const WEBUI_V2_ROUTE_REGENERATE_PREFIX: &str = "webui.v2.regenerate_prefix";
pub const WEBUI_V2_ROUTE_EXPORT_SKILL:     &str = "webui.v2.export_skill";

pub const WEBUI_V2_PATTERN_LIST_PREFIXES:    &str = "/api/webchat/v2/prefixes";
pub const WEBUI_V2_PATTERN_REGENERATE_PREFIX: &str = "/api/webchat/v2/prefixes/{name}/regenerate";
pub const WEBUI_V2_PATTERN_EXPORT_SKILL:     &str = "/api/webchat/v2/skills/{id}/export";
```

**Descriptors to add** (`descriptors.rs`):
```rust
// Under "// Phase K.1 — Prefix cache routes." comment block.

fn list_prefixes_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_LIST_PREFIXES,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_LIST_PREFIXES,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}

fn regenerate_prefix_descriptor() -> IngressRouteDescriptor {
    // 1/min per-caller rate limit enforced both at descriptor level and in the service.
    descriptor(
        WEBUI_V2_ROUTE_REGENERATE_PREFIX,
        NetworkMethod::Post,
        WEBUI_V2_PATTERN_REGENERATE_PREFIX,
        mutation_policy(
            BodyLimitPolicy::NoBody,
            mutation_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProductWorkflow,
        ),
    )
}
```

Also add `export_skill_descriptor()` in K.1.7.

**`webui_v2_routes()` in `descriptors.rs`:** Add `list_prefixes_descriptor()` and `regenerate_prefix_descriptor()` to the `vec![]` in `webui_v2_routes()`. Remove `reassemble_interceptor_descriptor()` and `prewarm_interceptor_descriptor()` from the vec.

**Constants to remove** from `descriptors.rs` (both the `pub const` IDs and the `pub const` patterns and the private builder functions):
- `WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR`, `WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR`, `reassemble_interceptor_descriptor`
- `WEBUI_V2_ROUTE_PREWARM_INTERCEPTOR`, `WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR`, `prewarm_interceptor_descriptor`

**New route ID exports** (`crates/brassclaw_webui_v2/src/lib.rs`):
The existing `pub use descriptors::{...}` block in `lib.rs` contains `WEBUI_V2_ROUTE_PREWARM_INTERCEPTOR` and `WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR` (currently on lines 68–70 of that file, among many other exports). Remove both of those identifiers from the block and add the three new constants:
```rust
// Add to the pub use descriptors::{...} block:
WEBUI_V2_ROUTE_EXPORT_SKILL,
WEBUI_V2_ROUTE_LIST_PREFIXES,
WEBUI_V2_ROUTE_REGENERATE_PREFIX,
// Remove from the pub use descriptors::{...} block:
// WEBUI_V2_ROUTE_PREWARM_INTERCEPTOR    (line ~68)
// WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR  (line ~70)
```
The `is_webui_v2_llm_config_route_id` function export and all other exports in `lib.rs` are unchanged.

**`WebUiV2HttpError::internal`** (`crates/brassclaw_webui_v2/src/error.rs`): This method does **not** currently exist. Add it as a convenience constructor so the `export_skill` handler can propagate a `Response::builder()` error without `.unwrap()`:
```rust
impl WebUiV2HttpError {
    /// Construct an internal-server-error [`WebUiV2HttpError`] from a free-form
    /// message. Used only where the `From<RebornServicesError>` path is unavailable
    /// (e.g., response-builder failures).
    pub fn internal(msg: impl std::fmt::Display) -> Self {
        use brassclaw_product_workflow::{RebornServicesError, RebornServicesErrorCode};
        Self(RebornServicesError::from_status(
            RebornServicesErrorCode::InternalError,
            500,
            false,
        ))
    }
}
```
> **Note:** the `msg` parameter is intentionally discarded — `RebornServicesError` carries only the structured error code + HTTP status, not a free-form string. If the error type is later extended to carry a message, update accordingly.

**Handler routing** (`router.rs`): In `webui_v2_router_with_options`, in the `use crate::descriptors::{...}` import block at the top of `router.rs`:
- **Remove** from imports: `WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR`, `WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR`
- **Add** to imports: `WEBUI_V2_PATTERN_LIST_PREFIXES`, `WEBUI_V2_PATTERN_REGENERATE_PREFIX`, `WEBUI_V2_PATTERN_EXPORT_SKILL`

In the route list, remove the two existing routes (currently around lines 284–291 of `router.rs`):
```rust
// REMOVE these two:
.route(
    WEBUI_V2_PATTERN_REASSEMBLE_INTERCEPTOR,
    post(handlers::reassemble_interceptor),
)
.route(
    WEBUI_V2_PATTERN_PREWARM_INTERCEPTOR,
    post(handlers::prewarm_interceptor),
)
```
Replace the removed block with the three new prefix + export routes, placed in the same "Phase 5.5" comment block location:
```rust
// Phase K.1 — Prefix cache routes (replaces reassemble + prewarm).
.route(
    WEBUI_V2_PATTERN_LIST_PREFIXES,
    get(handlers::list_prefixes),
)
.route(
    WEBUI_V2_PATTERN_REGENERATE_PREFIX,
    post(handlers::regenerate_prefix),
)
.route(
    WEBUI_V2_PATTERN_EXPORT_SKILL,
    get(handlers::export_skill),
)
```

**Handlers to add** (`handlers.rs`):

```rust
/// `GET /api/webchat/v2/prefixes`
///
/// List prefix cache entries for the caller's scope.
pub async fn list_prefixes(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
) -> Result<Json<brassclaw_product_workflow::PrefixListResponse>, WebUiV2HttpError> {
    let response = state.services().list_prefix_entries(caller).await?;
    Ok(Json(response))
}

/// Path params for `POST /api/webchat/v2/prefixes/{name}/regenerate`.
#[derive(Debug, Deserialize)]
pub struct RegeneratePrefixPath {
    pub name: String,
}

/// `POST /api/webchat/v2/prefixes/{name}/regenerate`
///
/// Assemble + prewarm the named prefix. Rate-limited to 1/min per caller
/// (enforced in the service via in-memory rate-limit map).
pub async fn regenerate_prefix(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(RegeneratePrefixPath { name }): Path<RegeneratePrefixPath>,
) -> Result<Json<brassclaw_product_workflow::PrefixRegenerateResponse>, WebUiV2HttpError> {
    let response = state.services().regenerate_prefix(caller, name).await?;
    Ok(Json(response))
}
```

Remove handler functions `reassemble_interceptor` and `prewarm_interceptor`.

**DTOs** (define in `crates/brassclaw_product_workflow/src/interceptor_config.rs` alongside `InterceptorConfigSnapshot` — this is where all interceptor/prefix service types live):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixEntry {
    pub name:              String,           // e.g. "base-prompt"
    pub fingerprint:       Option<String>,   // SHA-256 hex, None if never assembled
    pub assembled_at:      Option<String>,   // RFC3339, None if never assembled
    pub prewarm_last_at:   Option<String>,   // RFC3339, None if never prewarmed
    pub is_stale:          bool,
    pub bundle_size_chars: Option<usize>,    // len of bundle string, None if never assembled
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixListResponse {
    pub prefixes: Vec<PrefixEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrefixRegenerateResponse {
    pub name:           String,
    pub fingerprint:    String,
    pub assembled_at:   String,   // RFC3339
    pub prewarm_last_at: String,  // RFC3339
}
```

> **Note:** Timestamps are `Option<String>` (RFC3339) rather than `Option<DateTime<Utc>>` for the same reason existing `InterceptorConfigSnapshot` uses `Option<String>` for `base_prompt_assembled_at` and `prewarm_last_at` (they are stored as strings in the DB / KV store and forwarded directly). This is consistent with the existing pattern.

`list_prefix_entries` returns a `PrefixListResponse` with today exactly one entry (`name = "base-prompt"`), synthesized from the `reborn_basic_prompt_store` row (or a zero-state entry with all `Option` fields `None` if no row exists yet). This design already supports future prefixes.

**`RebornServicesApi` trait additions** (in `crates/brassclaw_product_workflow/src/reborn_services.rs`, under the interceptor config section `// ── Interceptor configuration (Phase 5.5) ─────`):

```rust
/// List prefix cache entries for the caller's scope (Phase K.1).
async fn list_prefix_entries(
    &self,
    _caller: WebUiAuthenticatedCaller,
) -> Result<PrefixListResponse, RebornServicesError> {
    Err(interceptor_config::interceptor_config_unavailable())
}

/// Assemble + prewarm the named prefix (Phase K.1). Rate-limited to 1/min per caller.
async fn regenerate_prefix(
    &self,
    _caller: WebUiAuthenticatedCaller,
    _name: String,
) -> Result<PrefixRegenerateResponse, RebornServicesError> {
    Err(interceptor_config::interceptor_config_unavailable())
}
```

Also **remove** `reassemble_interceptor_base_prompt` and `prewarm_interceptor` from the `RebornServicesApi` trait (and from the `RebornServices` struct implementation in `reborn_services.rs`). There are **two** removal sites:
1. **Trait default methods** in the `RebornServicesApi` trait body — the `async fn reassemble_interceptor_base_prompt(...)` and `async fn prewarm_interceptor(...)` default-`Err` methods currently at ~lines 1195–1208 of `reborn_services.rs`.
2. **Concrete forwarding impls** on `RebornServices` — the `reassemble_interceptor_base_prompt` and `prewarm_interceptor` forwarding bodies that call `self.interceptor_config.as_ref().ok_or_else(...)?.<method>(caller).await`, currently at ~lines 3653–3675 of `reborn_services.rs`.

Both sites must be removed for the codebase to compile cleanly (the `InterceptorConfigService` trait will no longer define those methods).

##### K.1.4 `regenerate_prefix` service method (backend)

**File:** `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` (modify)

> **Rate-limit pattern:** The existing `RebornInterceptorConfigService` uses an in-memory `Arc<Mutex<HashMap<String, Instant>>>` per operation (one for `reassemble`, one for `prewarm`). The new `regenerate_prefix` replaces both, so it uses a single new `regenerate_rate_limit: RateLimitState` field (added alongside `reassemble_rate_limit` and `prewarm_rate_limit`, which are then **removed**). The existing `check_rate_limit` helper is reused unchanged.

The new `regenerate_prefix(caller, name)` method on `RebornInterceptorConfigService`:

1. **Name guard:** `match name.as_str() { "base-prompt" => {} _ => return Err(PrefixNotFound { name }) }`. Add `PrefixNotFound { name: String }` variant to `InterceptorConfigServiceError` in `crates/brassclaw_product_workflow/src/interceptor_config.rs`.
2. **Rate-limit check:** `self.check_rate_limit(&self.regenerate_rate_limit, &caller_id).await?` — same 60-second per-caller window as before.
3. **Assemble:** call `self.do_reassemble().await?` — unchanged. The assembly is per-table `SELECT prompt_uid, name, COALESCE(content,'') FROM {table} WHERE validation_status='validated' AND NOT ('05:validator'=ANY(COALESCE(consumer_tags, ARRAY[]::text[]))) ORDER BY prompt_uid ASC LIMIT 1000` (one query per existing table), with cross-table sort `parts.sort_by(|a,b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))` applied after collecting all rows. The per-table `ORDER BY prompt_uid ASC` and the post-query global sort by `(class_code, prompt_uid)` are both preserved exactly as today.
4. **Fingerprint:** `let fingerprint = hex::encode(sha2::Sha256::digest(assembled.as_bytes()));`
5. **Store:** Extract `let user_id = caller.user_id.as_str(); let project_id = caller.project_id.as_ref().map(|p| p.as_str()).unwrap_or(""); let agent_id = caller.agent_id.as_ref().map(|a| a.as_str()).unwrap_or("");` — note that `PgBasicPromptStore` has its own fixed `tenant_id` and `agent_id` from construction, but the `store(user_id, project_id, ...)` call uses the per-caller `user_id` and `project_id`. The `agent_id` dimension is fixed from construction (matching the interceptor service's own `tenant_id`). Call `pg_store.store(user_id, project_id, &fingerprint, &assembled).await`. This replaces the `save_config_key(...KEY_BASE_PROMPT...)` and `save_config_key(...KEY_BASE_PROMPT_ASSEMBLED_AT...)` calls — **remove both from the method and from the struct.**
6. **Prewarm:** exact same logic as the existing `prewarm()` method — load `assembled` (now from the store rather than the KV, since we just stored it), call `gateway.stream_model(request)` with the same `HostManagedModelRequest` shape (a single `System` message with `content = assembled`). On success: `pg_store.record_prewarm(user_id, project_id).await` (using the same locals from step 5).
7. Return `PrefixRegenerateResponse { name: "base-prompt".to_string(), fingerprint, assembled_at: entry.assembled_at.to_rfc3339(), prewarm_last_at: prewarm_at.to_rfc3339() }`.

**Remove from `RebornInterceptorConfigService`:**
- `reassemble_base_prompt` method
- `prewarm` method
- `reassemble_rate_limit` field
- `prewarm_rate_limit` field
- `KEY_BASE_PROMPT` and `KEY_BASE_PROMPT_ASSEMBLED_AT` and `KEY_PREWARM_LAST_AT` config key constants

**Add to `RebornInterceptorConfigService`:**
- `pg_basic_prompt_store: Option<Arc<PgBasicPromptStore>>` field
- `regenerate_rate_limit: RateLimitState` field
- `with_basic_prompt_store(store: Arc<PgBasicPromptStore>)` builder

**`build_snapshot` / `InterceptorConfigSnapshot`** (in `crates/brassclaw_product_workflow/src/interceptor_config.rs`):
Remove fields `base_prompt_assembled_at: Option<String>`, `base_prompt_size_chars: Option<usize>`, `components_since_rebuild: Option<u32>`, `prewarm_last_at: Option<String>` from `InterceptorConfigSnapshot`. After removal the struct has exactly: `mode: String`, `sempai_connected: bool`, `persona: String`. Also remove the corresponding `KEY_BASE_PROMPT` load from `build_snapshot` and `load_config`.

**`mark_stale` wiring in Q2 approval:**
The Q2 approve path goes through `update_component_validation_status` in `crates/brassclaw_reborn_composition` (the composition impl of the `RebornServicesApi` trait method). After the `validation_status = 'validated'` DB write, call `pg_basic_prompt_store.mark_stale(user_id, project_id).await` (log a `debug!` on error; do not propagate — stale marking is best-effort, a stale miss is safe, an error must not fail the graduation). This is side effect 4 of §0.15.

**`list_prefix_entries` service method:**
Add to `RebornInterceptorConfigService` a `list_prefix_entries(caller)` method (wired into the `InterceptorConfigService` trait or directly into `RebornServices`). It calls `pg_basic_prompt_store.get_for_scope(user_id, project_id)` and returns a `PrefixListResponse` with one `PrefixEntry` for `"base-prompt"`: either synthesized from the stored row or a zero-state entry when no row exists.

##### K.1.5 Placeholder substitution wiring (Sempai-Kohai)

**File:** the Sempai-Kohai prompt-assembly path — `crates/brassclaw_reborn_composition/src/interceptor_config_service.rs` or the module referenced in `09-sempai-kohai.md` and `10-prefix-base-prompt.md` as the prompt-creation endpoint.

At the very end of prompt creation (after the full turn prompt is assembled), the Sempai-Kohai system performs:

1. Call `pg_basic_prompt_store.get_for_scope(user_id, project_id)`.
2. **If a non-stale entry exists** (`is_stale = false`): replace the single `base-prompt` placeholder line in the assembled prompt with the stored `bundle` string. The placeholder line is a literal marker line that the assembly step inserts; the exact format is defined in `10-prefix-base-prompt.md`.
3. **If no entry exists or `is_stale = true`**: emit a **short minimal-context prompt-part** in place of the full bundle — a compact summary with only the most essential instruction headers, sufficient for the LLM to operate without the KV-cached content. The fallback does not error; it degrades gracefully.
4. The per-turn patch must NOT repeat content already present in the full base-prompt bundle (`basic_prompt_section_refs` contains navigation pointers, not content).

This wiring is the Phase K.1 completion of §0.13. No placeholder substitution code exists before K.1.

##### K.1.6 WebUI SPA changes

**Files to modify** (in `crates/brassclaw_webui_v2_static/static/js/`):

**`pages/settings/lib/settings-schema.js`:**
Add one entry to `SETTINGS_TABS` **after the `interceptor` entry** (between `interceptor` and `safety`):

```js
{ id: "prefix", labelKey: "settings.prefix", icon: "layers" },
```

> **Note:** The `labelKey` format in `settings-schema.js` is `"settings.{id}"` (e.g. `"settings.interceptor"`, `"settings.safety"`), NOT `"settings.tab.{id}"`. Match the existing format exactly. Total tabs after adding: 18.

**`app/routes.js`** (`crates/brassclaw_webui_v2_static/static/js/app/routes.js`):
Add a `prefix` entry to `SETTINGS_SUB_ROUTES` **after the `interceptor` entry**. In the current file (line 50), the `interceptor` entry immediately precedes the `safety` entry (line 51). Insert `prefix` between them so the order is:

```js
{ id: "interceptor", labelKey: "settings.interceptor", icon: "spark" },
{ id: "prefix",      labelKey: "settings.prefix",      icon: "layers" },
{ id: "safety",      labelKey: "settings.safety",      icon: "shield" },
```

No `hidden: true` — the prefix tab endpoints are real routes that land in K.1.

**`pages/settings/settings-page.js`:**
Add `import { PrefixTab } from "./components/prefix-tab.js";` to imports.
Add `prefix: html\`<${PrefixTab} />\`` to the `tabContent` object (after the `interceptor` entry).

**`pages/settings/lib/settings-api.js`:**
Add two new api functions after the existing interceptor functions. Remove `reassembleInterceptor()` and `prewarmInterceptor()` (their routes are gone):

```js
// Phase K.1 — Prefix cache routes.
export function fetchPrefixes() {
  return apiFetch("/api/webchat/v2/prefixes");
}
export function regeneratePrefix(name) {
  return apiFetch(`/api/webchat/v2/prefixes/${encodeURIComponent(name)}/regenerate`, {
    method: "POST",
  });
}
```

**`pages/settings/hooks/useInterceptor.js`:**
Remove `handleReassemble`, `handlePrewarm`, and `actionStatus` (reassemble/prewarm tracking state) from the hook. Remove the calls to `reassembleInterceptor()` and `prewarmInterceptor()`. The hook only needs to fetch config and provide `handleUpdate`.

**`pages/settings/components/interceptor-tab.js`:**
Remove the `ControlCard` component and its usage. Remove the `ControlCard`-related destructured values from `useInterceptor()` (`handleReassemble`, `handlePrewarm`, `actionStatus`). Remove the `StatusCard` fields for `base_prompt_assembled_at`, `base_prompt_size_chars`, `components_since_rebuild`, and `prewarm_last_at` — the `StatusCard` renders only `mode` and `sempai_connected` after this change. Update `InterceptorSkeleton` to remove the third skeleton card (was the ControlCard). The Interceptor tab retains: `StatusCard` (mode + Sempai connected badge), `PersonaCard` (persona textarea + save button).

**`pages/settings/components/prefix-tab.js`** (new file):

The Prefix Tab renders a list of prefix entries (today: one row, `"base-prompt"`). For each entry:

- **Name badge**: `entry.name` (e.g. `"base-prompt"`).
- **Status row**: `assembled_at` timestamp (formatted with `toLocaleString()`), `prewarm_last_at` timestamp, `bundle_size_chars` chars, `fingerprint` (truncated to 12 hex chars for display).
- **Stale indicator**: if `entry.is_stale === true`, show a warning badge `"Stale — components changed since last compile"`.
- **Generate / Regenerate button**: label is `"Generate"` if `entry.assembled_at == null`, otherwise `"Regenerate"`. On click: call `regeneratePrefix(entry.name)`, then refresh via `refetch()`. On HTTP 429 response: show inline `"Rate limited — try again in 1 minute"` without throwing. On other errors: show inline error message.
- **Loading state**: button shows spinner and is disabled while the request is in flight (prewarm can take several seconds).
- **Empty state**: if `prefixes` is empty, show `"No prefix entries found."` (should not happen in K.1 since the response always includes the `"base-prompt"` synthetic entry).

The component uses a `usePrefixes()` hook (`pages/settings/hooks/usePrefixes.js`, new file) backed by `fetchPrefixes()`. The hook fetches on mount and exposes `{ prefixes, isLoading, loadError, refetch }`.

**`i18n/en.js`** (`crates/brassclaw_webui_v2_static/static/js/i18n/en.js`):
The file uses `registerPack("en", { ... })`. The `"settings.interceptor": "Interceptor"` key is currently at line 185. Add the `"settings.prefix"` key on the line immediately after `"settings.interceptor"`:
```js
  "settings.interceptor": "Interceptor",
  "settings.prefix": "Prefix Cache",
```
**Other language packs** (`i18n/es.js`, `i18n/de.js`, `i18n/fr.js`, `i18n/ja.js`, `i18n/ko.js`, `i18n/zh-CN.js`, `i18n/pt-BR.js`, `i18n/hi.js`, `i18n/ar.js`, `i18n/uk.js`): Add `"settings.prefix"` with the locale-appropriate translation (or copy the English value `"Prefix Cache"` as a placeholder when no translation is available). Missing keys fall back to the key string itself, so this is not blocking, but all packs should be updated for completeness.

##### K.1.7 SKILL.md on-demand export (item 5.1)

> Classic Claude-style v3 skills are DB-stored parts (name, description, body, tool_name, param_schema, activation criteria) with **no physical `SKILL.md` file**. A `SKILL.md` can be exported via the WebUI on demand.

**New route:** `GET /api/webchat/v2/skills/{id}/export`

> **No `ContentType` field on `IngressRouteDescriptor`** — that API does not exist. The descriptor is a read route (bearer, ProjectionOnly). The response's `Content-Type: text/plain` and `Content-Disposition` headers are set directly in the handler.

**Descriptor** (add `export_skill_descriptor()` to `descriptors.rs`, following the `read_policy` pattern):

```rust
fn export_skill_descriptor() -> IngressRouteDescriptor {
    descriptor(
        WEBUI_V2_ROUTE_EXPORT_SKILL,
        NetworkMethod::Get,
        WEBUI_V2_PATTERN_EXPORT_SKILL,
        read_policy(
            read_rate_limit(),
            AuditTraceClass::UserAction,
            AllowedEffectPath::ProjectionOnly,
            StreamingMode::None,
        ),
    )
}
```

Add `export_skill_descriptor()` to `webui_v2_routes()`.

**Handler** (`handlers.rs`):

```rust
/// Path params for `GET /api/webchat/v2/skills/{id}/export`.
#[derive(Debug, Deserialize)]
pub struct ExportSkillPath {
    pub id: String,   // skill UUID as string; parse to Uuid in the handler
}

/// `GET /api/webchat/v2/skills/{id}/export`
///
/// Export a DB-stored v3 skill as an on-demand SKILL.md download.
/// Returns `Content-Type: text/plain` with a `Content-Disposition: attachment` header.
pub async fn export_skill(
    State(state): State<WebUiV2State>,
    Extension(caller): Extension<WebUiAuthenticatedCaller>,
    Path(ExportSkillPath { id }): Path<ExportSkillPath>,
) -> Result<axum::response::Response, WebUiV2HttpError> {
    let skill_md = state.services().export_skill_as_skill_md(caller, id).await?;
    let response = axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            "attachment; filename=\"SKILL.md\"",
        )
        .body(axum::body::Body::from(skill_md))
        .map_err(|e| WebUiV2HttpError::internal(format!("response build: {e}")))?;
    Ok(response)
}
```

> **No `.unwrap()` in production code.** The `Response::builder()` pattern above propagates the error through `WebUiV2HttpError::internal` instead. `WebUiV2HttpError::internal` must be added if it doesn't exist, or the appropriate existing error constructor used.

**`RebornServicesApi` trait addition** (in `crates/brassclaw_product_workflow/src/reborn_services.rs`, under the skills section):

```rust
/// Export a v3 DB-stored skill as a SKILL.md formatted string (on-demand, item 5.1).
async fn export_skill_as_skill_md(
    &self,
    _caller: WebUiAuthenticatedCaller,
    _skill_id: String,
) -> Result<String, RebornServicesError> {
    Err(RebornServicesError::from_status(
        RebornServicesErrorCode::InvalidRequest,
        501,
        false,
    ))
}
```

**Service method** (implement in `brassclaw_reborn_composition`):
`export_skill_as_skill_md(caller, skill_id)` parses `skill_id` as UUID (`uuid::Uuid::parse_str(&skill_id).map_err(...)?`), fetches the DB-stored skill row by `(tenant_id, user_id, skill_id)` scope, and renders it into the standard Anthropic SKILL.md format:

```
---
name: {skill.name}
description: {skill.description}
tool_name: {skill.tool_name}
param_schema: {skill.param_schema_json}
activation_criteria: {skill.activation_criteria}
---

{skill.body}
```

Returns `SkillNotFound` (mapped to HTTP 404 via `WebUiV2HttpError`) if the skill does not exist in scope. No file is written to disk.

**WebUI**: add a "Download SKILL.md" button to the skill detail/edit page. On click: `window.location.assign('/api/webchat/v2/skills/' + encodeURIComponent(skill.id) + '/export')` — use `location.assign` rather than `window.open` to avoid a popup blocker.

##### K.1.8 Tests

**Unit tests** (in `crates/brassclaw_reborn_composition/src/pg_basic_prompt_store.rs` or a sibling test module):
- `store(user_id, project_id, fingerprint, bundle)` → `get_for_scope` returns the entry, `is_stale = false`.
- `mark_stale` on an existing row → `is_stale = true`.
- `mark_stale` with no row → `Ok(())` (no error).
- `record_prewarm` → `prewarm_last_at` is set.
- Fingerprint is `sha256` hex — stable for identical bundles, different for changed bundles.

**Integration tests** (require Postgres, gate under `#[cfg(feature = "integration")]`):
- `regenerate_prefix(caller, "base-prompt")` → `reborn_basic_prompt_store` row written, `is_stale = false`, `fingerprint` correct, `prewarm_last_at` set.
- `regenerate_prefix(caller, "unknown-name")` → `InterceptorConfigServiceError::PrefixNotFound`.
- Q2 graduation (call `update_component_validation_status` → `validated`) → `pg_basic_prompt_store.mark_stale` called → row `is_stale = true` (verifies side effect 4 of §0.15).
- `regenerate_prefix` after stale mark → row written, `is_stale = false`.
- `list_prefix_entries` with no store row → `PrefixListResponse { prefixes: [PrefixEntry { name: "base-prompt", assembled_at: None, is_stale: false, ... }] }`.
- `list_prefix_entries` with stored row → entry reflects stored values.
- `regenerate_prefix` rate limit — second call within 60s → `InterceptorConfigServiceError::RateLimitExceeded`.
- Handler contract (`crates/brassclaw_webui_v2/tests/webui_v2_handlers_contract.rs`): the `StubServices` struct at ~lines 897–1092 implements `RebornServicesApi`. Make these changes:
  - **Remove** the `reassemble_interceptor_base_prompt` stub (currently ~lines 1063–1076) and the `prewarm_interceptor` stub (currently ~lines 1078–1091) entirely.
  - **Update** the `get_interceptor_config` stub (~line 1032) and `update_interceptor_config` stub (~line 1047) to use the trimmed `InterceptorConfigSnapshot` — remove the now-deleted fields `base_prompt_assembled_at`, `base_prompt_size_chars`, `prewarm_last_at`, `components_since_rebuild`:
    ```rust
    Ok(InterceptorConfigSnapshot {
        sempai_connected: false,
        mode: "routing".to_string(),
        persona: String::new(),
    })
    ```
  - **Add** three new stub methods:
    ```rust
    async fn list_prefix_entries(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<PrefixListResponse, RebornServicesError> {
        Ok(PrefixListResponse { prefixes: vec![] })
    }

    async fn regenerate_prefix(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _name: String,
    ) -> Result<PrefixRegenerateResponse, RebornServicesError> {
        Ok(PrefixRegenerateResponse {
            name: "base-prompt".to_string(),
            fingerprint: "abc123".to_string(),
            assembled_at: "2024-01-01T00:00:00Z".to_string(),
            prewarm_last_at: "2024-01-01T00:00:00Z".to_string(),
        })
    }

    async fn export_skill_as_skill_md(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _skill_id: String,
    ) -> Result<String, RebornServicesError> {
        Ok("---\nname: stub-skill\n---\n\nStub body.".to_string())
    }
    ```
  - **Add** `PrefixListResponse`, `PrefixRegenerateResponse` to the `use brassclaw_product_workflow::{...}` import block at the top of the test file (alongside the existing `InterceptorConfigSnapshot`, `UpdateInterceptorConfigRequest` imports).
- Handler contract: `GET /api/webchat/v2/prefixes` → 200 `{ "prefixes": [...] }`.
- Handler contract: `POST /api/webchat/v2/prefixes/base-prompt/regenerate` → 200.
- Handler contract: `GET /api/webchat/v2/skills/{id}/export` → 200, `Content-Type: text/plain`, `Content-Disposition: attachment; filename="SKILL.md"`.
- Descriptor contract (`crates/brassclaw_webui_v2/tests/webui_v2_descriptors_contract.rs`): add `Expected` entries for `WEBUI_V2_ROUTE_LIST_PREFIXES` (GET, `/api/webchat/v2/prefixes`, read rate limit 120/60, `ProjectionOnly`) and `WEBUI_V2_ROUTE_REGENERATE_PREFIX` (POST, `/api/webchat/v2/prefixes/{name}/regenerate`, mutation rate limit 60/60, `ProductWorkflow`) and `WEBUI_V2_ROUTE_EXPORT_SKILL` (GET, `/api/webchat/v2/skills/{id}/export`, read rate limit 120/60, `ProjectionOnly`); remove entries for `WEBUI_V2_ROUTE_REASSEMBLE_INTERCEPTOR` and `WEBUI_V2_ROUTE_PREWARM_INTERCEPTOR`; remove their `pub use` imports from the test's use block; update the count assertion if present.
- Integration: `POST /api/webchat/v2/interceptor/reassemble` → 404 (route removed).
- Integration: `POST /api/webchat/v2/interceptor/prewarm` → 404 (route removed).
- Integration: `GET /api/webchat/v2/interceptor/config` → 200 `{ "mode": "...", "sempai_connected": ..., "persona": "..." }` — no prefix fields.

#### K.2 MCP Translation Layer — External MCPs Only

**File:** `crates/brassclaw_extensions/src/mcp_translation.rs` (new)

> **Scope:** This translator handles **external third-party MCP servers only**.
> Builtin first-party tools (`builtin.*`) are seeded by the separate `builtin_bootstrap.rs`
> in Phase L — with hand-authored content through automated-but-auditable Q2 (Phase P.0).
> Do NOT run the MCP translator against builtin tools.

For each external MCP tool: generate Tool (class 0), ToolSkill (class 13), Skill (class 1), and
a skeleton Recipe (class 21) with auto-generated StepDescriptions:
- Step 1: `knowledge: orchestrator`, `type: text`, `info`: auto-generated task context (from MCP tool description)
- Step 2: `knowledge: rust`, `type: component`, `include`: [ToolSkill UUID]
- Step 3: `knowledge: orchestrator`, `type: component`, `include`: [Skill UUID]
- Default `step_link: "0:0-0:E"`

One ExtensionCatalogue (class 23) grouping all generated components.
All inserted with `validation_status = 'pending'` — external MCP content must go through Q1 + Q2.

**Why external MCPs are treated differently from builtins:**
- MCP content comes from untrusted third-party servers — must pass Q1 injection scan and Q2 manual review.
- Skill bodies are auto-generated stubs and need human review before becoming active.
- No `capability_id` is set — external MCPs are referenced by UUID, not by a registered capability name.
- `source = "imported"` (not `"system"`).

#### K.3 Cleanup

- Remove `__retrieve_docs__` handler registration from `orchestrator.rs` **now** (no
  compatibility window). It is the legacy MemoryDoc path, fully superseded by the
  v3-upgraded `__assemble_prior_knowledge__`. Any orchestrator still calling
  `__retrieve_docs__` must be updated before Phase K ships.
- Remove step-0 shim comment block from `default.py` (the `# Pre-Phase-5 fallback`
  comment block around the dead `__retrieve_docs__(goal, 5)` call — Phase G already
  removes the call itself; Phase K removes the comment artefact).
- Delete `crates/brassclaw_engine/src/memory/retrieval_dbless.rs`. The file still exists
  and contains three functions: `extract_keywords`, `keyword_match_score`, and
  `doc_type_weight(DocType)`. `extract_keywords` and `keyword_match_score` should be
  **moved** to `retrieval_source.rs` as private helpers (they remain useful for any keyword
  scoring on the `RamSource` path until Phase K.3 deletes `RamSource`). `doc_type_weight`
  takes the deprecated `DocType` enum — it is **deleted outright** (do not move it); the
  `DocType` enum is frozen and once `RamSource` is gone there is no caller.
  > **⚠️ CORRECTION to prior review note:** The previous note stated "`doc_type_weight(DocType)`
  > function was already removed in the same Step 12 pass." This is **WRONG** — verified by
  > reading the file: `retrieval_dbless.rs` still exists and still contains `doc_type_weight(DocType)`
  > (lines ~76–88) as of the current codebase. `doc_type_weight_by_class(i32)` (the i32-keyed
  > variant the review note was about) does not exist — that is the correct claim. The enum-keyed
  > `doc_type_weight(DocType)` is still present and must be deleted in Phase K.3.
- Delete `RamSource` from `retrieval_source.rs` and remove it from `mod.rs` exports.
  `RamSource` is the DB-less fallback; it has no role in a Postgres-only deployment.
  All unit tests that construct a `RamSource` directly are replaced by integration tests
  against `PostgresSource`. **Prerequisite satisfied by Phase E.0:** `PostgresSource` is
  already the live backend (E.0 wired it + removed the `TODO(Phase K)` marker), so this
  K.3 step is **pure deletion** — no wiring race, no "no retrieval backend" window. Verify
  E.0's acceptance criteria (live turns take `PostgresSource::fetch_for_turn`) still hold
  immediately before this deletion.
  > **✅ Review note (pre-v3 audit) — ORDERING HAZARD: do not delete `RamSource` until
  > `PostgresSource` is wired in production — RESOLVED by Phase E.0:** E.0 (added before
  > Phase E) wires `PostgresSource` as the live backend and removes the `TODO(Phase K)`
  > marker, so by Phase K `RamSource` is no longer the active backend — K.3 is pure
  > deletion, exactly the "wire-then-delete" split the note required (with the wire step
  > pulled forward to E.0). Original audit detail retained below:
  > `RamSource` is the *active* production
  > retrieval backend today (`manager.rs:383`, with the `TODO(Phase K)` from
  > `Goals_pre_v3_review.md` Step 8). Deleting it before the composition layer calls
  > `with_retrieval_source(PostgresSource)` leaves the engine with **no** retrieval backend
  > — every turn's `__assemble_prior_knowledge__` returns empty. Phase K.3 must be split
  > into two ordered sub-steps: (1) wire `PostgresSource` into `manager.rs` (composition),
  > ship and verify live turns take the intent path; (2) **then** delete `RamSource` +
  > `retrieval_dbless.rs`. This matches `Goals_pre_v3_review.md` Step 14's ordering
  > constraint and the §0.3 / Phase E review notes. If Phase K is also the first phase to
  > wire `PostgresSource`, the wiring sub-step must be done *and verified* before the
  > deletion sub-step in the same phase.
- Remove `handle_retrieve_docs` from `orchestrator.rs` (the function body, not just
  the registration). Remove `RetrievalEngine::retrieve_context` if it has no other
  callers after this deletion.
- **Delete the legacy `retrieve_context` fallback block in `handle_assemble_prior_knowledge`
  (`orchestrator.rs:2620–2637`).** This is the second caller of `retrieve_context` (the
  first being `handle_retrieve_docs`), preserved unchanged through Phase F per the C3
  resolution. With `PostgresSource` live (E.0) and `RamSource` deleted (previous bullet),
  the `if let Some(source) = retrieval_source` arm at `orchestrator.rs:2574` is always
  taken in production, so the `None`/error fallthrough to `retrieve_context` is dead.
  Delete the block; then `retrieve_context` has no remaining callers and is removed by
  the previous bullet. (Order: delete `handle_retrieve_docs` + this fallback block first,
  then remove `retrieve_context` itself.)
- Add deprecation notice to `__list_skills__`: no longer called from default step-0;
  remains callable for external/custom orchestrators.
- `__assemble_prior_knowledge__` is **not removed**. It is the primary prior-knowledge
  assembly function and stays as the canonical call in `default.py`.

#### Tests (K.1)

- Integration: `store` → `get_for_scope` returns bundle
- Integration: component `validated` → `is_stale = true`
- Integration: Interceptor prepends stored bundle before LLM shipment

#### Tests (K.2)

- Unit: MCP payload with 3 tools → 3 Tool + 3 ToolSkill + 3 Skill + 3 Recipe + 1 ExtensionCatalogue components created
- Integration: MCP install → components enter validation queue with `status = 'pending'`

---

### Phase L — Builtin Tool Bootstrap Seeder

**Status:** [ ] Pending

> **§0.23.3 + §0.23.9 fold-in:** Phase L also seeds the **trusted-root validation
> system** alongside the builtin-tool stack: one pre-trusted Extension per class +
> four category main-Recipes per class (security / performance / token-budget /
> v3-design-adherence, each calling sub-recipes) + one basic Recipe per class + one
> formatter PythonCode per class (§0.23.3). All `source='system'` components —
> builtins **and** the validation-system trusted root — graduate via the
> automated-but-auditable Q2 (Phase P.0), **no bypass** (Answer 2 retained). This
> seeding **must land right before Phase N's orchestrated Q1** (§0.23.9) so the
> validation components exist when orchestrated Q1 goes live.

**Goal:** Seed the full v3 component stack for all 23 builtin tools at first boot.
This is a separate concern from the Phase K MCP translator. The MCP translator targets
external third-party MCP servers (unknown shape, must enter Q1/Q2). The builtin bootstrap
targets the 23 first-party tools: content is hand-authored and quality-checked at
author time; all components go through Q1 + **automated-but-auditable Q2** (Phase P.0 —
the seeder is the recorded Q2 actor; no silent bypass). See §0.23.3 fold-in note above.

#### L.1 New migration: V057 (**was V056 before Decision 2**)

```sql
-- V057__reborn_tools_capability_id_and_system_source.sql
ALTER TABLE reborn_tools ADD COLUMN IF NOT EXISTS capability_id TEXT;
CREATE INDEX IF NOT EXISTS reborn_tools_capability_id_idx
    ON reborn_tools (tenant_id, user_id, agent_id, project_id, capability_id)
    WHERE capability_id IS NOT NULL;

-- Allow "system" as a source value (alongside existing: authored, extracted, migrated, imported)
ALTER TABLE reborn_tools
    DROP CONSTRAINT IF EXISTS reborn_tools_source_check,
    ADD CONSTRAINT reborn_tools_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
ALTER TABLE reborn_tool_skills
    DROP CONSTRAINT IF EXISTS reborn_tool_skills_source_check,
    ADD CONSTRAINT reborn_tool_skills_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
ALTER TABLE reborn_skills
    DROP CONSTRAINT IF EXISTS reborn_skills_source_check,
    ADD CONSTRAINT reborn_skills_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));

-- ⚠️ FIND-P6-02 + FIND-P9-14: reborn_recipes has NO source CHECK constraint in V033.
-- V033 has: source TEXT NOT NULL DEFAULT 'authored' (no CHECK clause).
-- A DROP CONSTRAINT IF EXISTS for reborn_recipes would be a SILENT NO-OP.
-- We do NOT issue a DROP here (nothing to drop). We only ADD the new constraint:
ALTER TABLE reborn_recipes
    ADD CONSTRAINT reborn_recipes_source_check
        CHECK (source IN ('authored', 'extracted', 'migrated', 'imported', 'system'));
-- Note: The DROP … IF EXISTS pattern used for reborn_tools/reborn_tool_skills/reborn_skills
-- above is correct for those tables (V027 has source CHECK on reborn_skills; etc.).
-- Do NOT copy that pattern for reborn_recipes — there is no prior constraint to drop.
```

`capability_id` links a `reborn_tools` row back to the Rust capability registry
(`"builtin.read_file"`, etc.) without fragile name-search. The Rust execution layer
uses this when resolving a Tool UUID to the registered handler.

#### L.0 Live capability layer — what already exists

The 23 `BuiltinFirstPartyTools` Rust capability handlers are **already fully
implemented** in `crates/brassclaw_host_runtime/src/first_party_tools/`. The
`capability_id` string constants are also already exported (e.g.
[`READ_FILE_CAPABILITY_ID`](crates/brassclaw_host_runtime/src/first_party_tools/mod.rs:68),
[`SHELL_CAPABILITY_ID`](crates/brassclaw_host_runtime/src/first_party_tools/shell.rs), etc.).

Phase L adds **zero new Rust capability logic**. It adds only the DB rows that
make these capabilities visible to the v3 orchestrator stack. Every
`capability_id` value in the Tool row seeder must be taken directly from the
live `*_CAPABILITY_ID` constant in `first_party_tools/` — never hard-coded as
a string literal — to stay in sync with the handler registry.

**Live capability-ID constants to use in the seeder:**

| `capability_id` value | Rust constant source |
|-----------------------|----------------------|
| `"builtin.read_file"` | `mod.rs::READ_FILE_CAPABILITY_ID` |
| `"builtin.write_file"` | `mod.rs::WRITE_FILE_CAPABILITY_ID` |
| `"builtin.list_dir"` | `mod.rs::LIST_DIR_CAPABILITY_ID` |
| `"builtin.glob"` | `mod.rs::GLOB_CAPABILITY_ID` |
| `"builtin.grep"` | `mod.rs::GREP_CAPABILITY_ID` |
| `"builtin.apply_patch"` | `mod.rs::APPLY_PATCH_CAPABILITY_ID` |
| `"builtin.shell"` | `shell.rs::SHELL_CAPABILITY_ID` |
| `"builtin.http"` | `http.rs::HTTP_CAPABILITY_ID` |
| `"builtin.http.save"` | `http.rs::HTTP_SAVE_CAPABILITY_ID` |
| `"builtin.json"` | `json.rs::JSON_CAPABILITY_ID` |
| `"builtin.time"` | `time.rs::TIME_CAPABILITY_ID` |
| `"builtin.echo"` | `echo.rs::ECHO_CAPABILITY_ID` |
| `"builtin.memory_search"` | `memory.rs::MEMORY_SEARCH_CAPABILITY_ID` |
| `"builtin.memory_write"` | `memory.rs::MEMORY_WRITE_CAPABILITY_ID` |
| `"builtin.memory_read"` | `memory.rs::MEMORY_READ_CAPABILITY_ID` |
| `"builtin.memory_tree"` | `memory.rs::MEMORY_TREE_CAPABILITY_ID` |
| `"builtin.skill_list"` | `skill_management.rs::SKILL_LIST_CAPABILITY_ID` |
| `"builtin.skill_install"` | `skill_management.rs::SKILL_INSTALL_CAPABILITY_ID` |
| `"builtin.skill_remove"` | `skill_management.rs::SKILL_REMOVE_CAPABILITY_ID` |
| `"builtin.trigger_create"` | `trigger_management.rs::TRIGGER_CREATE_CAPABILITY_ID` |
| `"builtin.trigger_list"` | `trigger_management.rs::TRIGGER_LIST_CAPABILITY_ID` |
| `"builtin.trigger_remove"` | `trigger_management.rs::TRIGGER_REMOVE_CAPABILITY_ID` |
| `"builtin.spawn_subagent"` | `spawn_subagent.rs::SPAWN_SUBAGENT_CAPABILITY_ID` |

**Content source for all 319 component bodies:** `builtin_stuff_v3.md` (Steps 1–26 + Step 14.x).
Every ToolSkill body, Leaf/Domain Skill body, PythonCode body, ExtensionCatalogue
`overview_doc`, and Recipe `step_descriptions` JSON is fully specified there. The
implementer transcribes each body into the corresponding `const &str` in
`builtin_bootstrap.rs` — no file I/O, no `include_str!()`, no external files.

**Current DB state:** migrations end at `V051`. Zero `reborn_tools`,
`reborn_tool_skills`, `reborn_python_code`, or `reborn_extension_catalogues` rows
exist for builtins. Phase L is gated on V052 (Phase B), V053 (Phase C), V055
(Phase J.2), V056 (Phase K), and V057 (Phase L.1) being applied first.

#### L.2 New file: `crates/brassclaw_reborn_composition/src/builtin_bootstrap.rs`

Seeder function: `pub async fn seed_builtin_components(pool: &PgPool, scope: &ComponentScope)`

**Structure:**
```
seed_builtin_components(pool, scope):
  if builtin components already exist for this scope → return (idempotent)
  
  for each builtin group (filesystem, network, memory, process, management):
    1. Insert ExtensionCatalogue row (validated, source=system)
    2. For each tool in group:
         Insert Tool row (capability_id = "builtin.X", source=system, validated)
         Insert ToolSkill row (tool_name = "builtin.X", source=system, validated)
    3. For each task-level Skill in group:
         Insert Skill row (body = hand-authored content, source=system, validated)
         Seed intent_examples into reborn_intent_inputs
    4. For each PythonCode helper in group:
         Insert PythonCode row (source=system, validated)
    5. For each Recipe in group (per §0.16.1):
         Insert Recipe row + step_descriptions JSONB
         Run IBS build_instruction as pre-flight (panics in debug builds on IbsError)
         Seed intent_examples into reborn_intent_inputs with correct step_link
         Insert Recipe row (source=system, validated)
```

**Hand-authored content** lives as named `const &str` literals **inline in
`builtin_bootstrap.rs`** — one constant per component body (ToolSkill, Skill,
PythonCode, ExtensionCatalogue `overview_doc`, Recipe `step_descriptions` JSON). The
full body text for all 319 components is already specified in `builtin_stuff_v3.md`
and is transcribed into these constants at implementation time.

**No `include_str!()` and no `prompts/builtin/` directory.** The `include_str!()` +
on-disk file pattern was the pre-v3 filesystem-skill approach (see Phase P.1 Step A —
now being deleted). Recreating it here for builtins would contradict the v3 principle
that **the DB is the sole runtime source of truth for all component content**. The
seeder writes to the DB once at first boot; the DB row is the live record from that
point on. The WebUI export button (Phase K.1 §K.1.7) produces a `SKILL.md`-formatted
file on demand from the DB row — that is the only "file" that ever needs to exist.

**Rust constant naming convention** (for readability inside the seeder file):

```rust
const TS_READ_FILE: &str = "...ToolSkill body for builtin.read_file...";
const TS_WRITE_FILE: &str = "...";
const SKILL_FILESYSTEM: &str = "...Domain Skill body...";
const CAT_FILESYSTEM_OVERVIEW: &str = "...ExtensionCatalogue overview_doc...";
// etc. — one const per component body, grouped by domain
```

**Called from** composition boot sequence, analogous to `component_import.rs`.

#### L.3 Content source

**No content files to create.** All 319 component bodies (30 ToolSkills, 62 PythonCode
bodies, 76 Leaf Skills, 9 Domain Skills, 95 Recipes with `step_descriptions` JSONB,
24 ExtensionCatalogue `overview_doc` strings, 23 Tool rows) are fully specified in
`builtin_stuff_v3.md` (Steps 1–26 + Final section). The implementer transcribes each
body from that document into the corresponding `const &str` in `builtin_bootstrap.rs`.

**Content registry** (inline `const &str` constants — all live in `builtin_bootstrap.rs`,
grouped by domain group matching the seeder insertion order):

| Constant name | Component | Source in builtin_stuff_v3.md |
|---------------|-----------|-------------------------------|
| `TS_SHELL_RUN` | ToolSkill `ts-shell-run` | Step 1.2 |
| `TS_READ_FILE` | ToolSkill `ts-read-file` | Step 2.2 |
| `TS_WRITE_FILE` | ToolSkill `ts-write-file` | Step 3.2 |
| `TS_LIST_DIR` | ToolSkill `ts-list-dir` | Step 4.2 |
| `TS_GLOB` | ToolSkill `ts-glob` | Step 5.2 |
| `TS_GREP` | ToolSkill `ts-grep` | Step 6.2 |
| `TS_APPLY_PATCH` | ToolSkill `ts-apply-patch` | Step 7.2 |
| `TS_HTTP_FETCH` | ToolSkill `ts-http-fetch` | Step 8.2 |
| `TS_HTTP_SAVE` | ToolSkill `ts-http-save` | Step 9.2 |
| `TS_MEMORY_SEARCH` | ToolSkill `ts-memory-search` | Step 10.2 |
| `TS_MEMORY_WRITE` | ToolSkill `ts-memory-write` | Step 11.2 |
| `TS_MEMORY_READ` | ToolSkill `ts-memory-read` | Step 12.2 |
| `TS_MEMORY_TREE` | ToolSkill `ts-memory-tree` | Step 13.2 |
| `TS_TIME_NOW` | ToolSkill `ts-time-now` | Step 14.2 |
| `TS_TIME_PARSE` | ToolSkill `ts-time-parse` | Step 14.6 |
| `TS_TIME_CONVERT` | ToolSkill `ts-time-convert` | Step 14.10 |
| `TS_JSON_QUERY` | ToolSkill `ts-json-query` | Step 15.2 |
| `TS_JSON_STRINGIFY` | ToolSkill `ts-json-stringify` | Step 15.6 |
| `TS_JSON_VALIDATE` | ToolSkill `ts-json-validate` | Step 15.7 |
| `TS_SKILL_LIST` | ToolSkill `ts-skill-list` | Step 16.4 |
| `TS_SKILL_INSTALL` | ToolSkill `ts-skill-install` | Step 16.5 |
| `TS_SKILL_REMOVE` | ToolSkill `ts-skill-remove` | Step 16.6 |
| `TS_TRIGGER_CREATE` | ToolSkill `ts-trigger-create` | Step 17.4 |
| `TS_TRIGGER_LIST` | ToolSkill `ts-trigger-list` | Step 17.5 |
| `TS_TRIGGER_REMOVE` | ToolSkill `ts-trigger-remove` | Step 17.6 |
| `TS_SPAWN_SUBAGENT` | ToolSkill `ts-spawn-subagent` | Step 18.2 |
| `TS_WEB_SEARCH` | ToolSkill `ts-web-search` | Step 20.1 |
| `TS_ECHO` | ToolSkill `ts-echo` | Step 19.2 |
| `SKILL_SHELL_RUN` … `SKILL_WEB_SEARCH` | 44 Leaf Skills (class 1) | Steps 1.3–20.4 |
| `SKILL_SHELL` … `SKILL_SUBAGENT` | 7 Domain Skills (class 2) | Steps 1.5, 7.x, 9.x.1, 13.x.3, 16.11, 17.11, 18.5 |
| `PC_EXEC_READ_FILE` … `PC_WEB_SEARCH_QUERY_BUILD` | 27 PythonCode bodies (class 22) | Steps 2.3–20.3 |
| `CAT_FILESYSTEM_OVERVIEW` … `CAT_MANAGEMENT_OVERVIEW` | 5 ExtensionCatalogue `overview_doc` strings | Steps 22–26 |
| Recipe `step_descriptions` JSONB | 46 Recipes (inline `serde_json::json!{}` literals) | Steps 1.6–20.5 |

> **Note:** Recipe `step_descriptions` are inline `serde_json::json!{}` macro
> literals in the seeder function body rather than `const &str` — they reference
> the UUID variables of already-inserted sibling components, which are local to
> the insertion scope. See `builtin_stuff_v3.md` §Final for the correct seeding
> order (ExtensionCatalogue → Tools → ToolSkills → PythonCode → Leaf Skills →
> Domain Skills → Recipes).

#### Tests

- Unit: seeder runs on empty DB → 85–90 component rows inserted
- Unit: seeder runs twice → idempotent (same row count, no duplicates)
- Unit: all inserted Recipes pass `build_instruction` pre-flight with no `IbsError`
- Unit: `builtin.shell` ToolSkill body contains "approval" text (safety content regression guard)
- Unit: `builtin.spawn_subagent` ToolSkill body contains "scope isolation" text
- Unit: every inserted Recipe with shell in rust channel has `llm_call_required = true`
- Integration: `resolve_intent("read the file at /tmp/foo.txt")` → matches `builtin-read-file` Recipe
- Integration: `resolve_intent("show me all files")` → matches `builtin-list-dir` or `builtin-find-files`
- Integration: Tool row `capability_id = "builtin.read_file"` → look up by `capability_id` returns correct UUID

---

### Phase M — Variable Intent Templates

**Status:** [ ] Pending

**Goal:** Add `%` slot marker support to intent expressions. Authors can write
`"show me all files in the % directory"` as an intent expression and `resolve_intent`
will match user text that fits the template. Value extraction is automatic from
template segments; `variable_patterns` remains optional refinement.

#### M.1 New migration: V058 (**was V057 before Decision 2**)

**File:** `crates/brassclaw_pg/migrations/V058__reborn_intent_inputs_template.sql`

```sql
ALTER TABLE reborn_intent_inputs
  ADD COLUMN is_template      BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN template_prefix  TEXT,
  ADD COLUMN template_suffix  TEXT;

CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_prefix_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, template_prefix)
    WHERE is_template = true AND template_prefix != '';

CREATE INDEX IF NOT EXISTS reborn_intent_inputs_template_suffix_rev_idx
    ON reborn_intent_inputs
    (tenant_id, user_id, agent_id, project_id, reverse(template_suffix))
    WHERE is_template = true AND template_prefix = '' AND template_suffix != '';
```

#### M.2 `seed_intent_input` upgrade

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs`

Extend `seed_intent_input` to detect `%` in `input_text` and populate the new columns:

```rust
pub fn parse_template(expression: &str) -> Option<(String, String)> {
    // Returns None if expression has no %, Some((prefix, suffix)) if it does.
    // prefix = text before first %, suffix = text after last %.
    // Adjacent-slot validation (two % with no literal between) done by caller (Q1).
    //
    // ⚠️ FIND-01 FIX: The earlier "suffix == expression" guard was dead code (the
    // early-return above prevents it from ever firing) and obscured the intent.
    // Replaced with a clear implementation.
    if !expression.contains('%') { return None; }
    // prefix = everything before the FIRST '%'
    let prefix = expression.splitn(2, '%').next().unwrap_or("").to_string();
    // suffix = everything after the LAST '%'
    // rsplitn(2, '%') yields the portion AFTER the last '%' first.
    let suffix = expression.rsplitn(2, '%').next().unwrap_or("").to_string();
    Some((prefix, suffix))
}
```

> **⚠️ FIND-01 NOTE — add test:** The test suite must include:
> - `parse_template("%")` → `Some(("".to_string(), "".to_string()))` — both anchors
>   empty → Q1 hard error (no-anchor rule).
> - `parse_template("% in %")` → `Some(("".to_string(), ""))` — prefix empty, suffix
>   empty (last `%` is at end) → Q1 hard error.
> - `parse_template("search for %")` → `Some(("search for ".to_string(), "".to_string()))` — prefix-anchored.
> - `parse_template("% directory")` → `Some(("".to_string(), " directory".to_string()))` — suffix-anchored.

`seed_intent_input` sets `is_template`, `template_prefix`, `template_suffix` from
`parse_template`. UNIQUE constraint already deduplicates on `(scope, input_text,
input_class, component_id)` so re-seeding is idempotent.

#### M.3 `resolve_intent` SQL upgrade

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs`

Replace the single `AND input_text = $5` predicate with the three-path query from
§0.17.1. The existing exact-match path (Path 0) is unchanged; Paths 1 and 2 are
new OR branches. `ORDER BY` gains the `CASE WHEN input_text = $5 THEN 0 ELSE 1 END`
tiebreaker so exact matches always outrank template matches for the same component.

Pass `$5 = raw user_text` (exact match) — no normalisation step in Rust needed.
PostgreSQL evaluates `$5 LIKE input_text` directly.

> **⚠️ PARAM-GAP (carry-forward from §0.17.1):** keep the existing 9-arg bind order
> exactly (`[tenant, user, agent, project, query, order_vec, order[0], order[1], order[2]]`).
> The new query only swaps the `AND input_text = $5` hard filter for the
> `AND (input_text = $5 OR <template paths>)` OR-group and prepends the exact-match
> ORDER BY tiebreaker — `$6` (ANY) and `$7/$8/$9` (CASE) stay in the same positions as
> the current `intent_system.rs:357-367` bind slice. See the §0.17.1 PARAM-GAP table.

#### M.4 Post-match extraction: `extract_template_slots`

**File:** `crates/brassclaw_engine/src/memory/intent_system.rs` (or new
`crates/brassclaw_engine/src/memory/template_extractor.rs`)

```rust
/// Given a matched template expression and the user text, extract slot values.
/// Returns a Vec of (slot_index, value) pairs in left-to-right order.
/// Slot names are "slot0", "slot1", ... unless overridden by variable_patterns.
pub fn extract_template_slots(
    template: &str,    // "show me all files in the % directory"
    user_text: &str,   // "show me all files in the /tmp directory"
) -> Vec<(String, String)>   // [("slot0", "/tmp")]
```

Algorithm: split `template` on `%` → literal segments. Find each segment left-to-right
in `user_text`. Each gap between consecutive segments is a slot value.

Called by `fetch_for_turn` (in `retrieval_source.rs`) after a template match resolves,
before `variable_patterns` validation/refinement. The extracted `(name, value)` pairs
feed the `{{vars.name}}` substitution step.

#### M.5 `variable_patterns` as optional post-extract refinement

When a template match occurs and `variable_patterns` is non-empty on the variant:
1. Auto-extract slot values via `extract_template_slots`.
2. For each `variable_patterns` entry: apply its regex to the auto-extracted value
   (not to the full `user_text`). If the regex matches a named group, that group's
   value replaces the positional slot name in the vars map.
3. If `variable_patterns` is empty: use positional names `slot0`, `slot1`, ...

This means an author can choose:
- **Simple case:** `"show me files in the % directory"` — no `variable_patterns`. Slot
  auto-extracted as `vars.slot0`. ToolBinding params reference `{{vars.slot0}}`.
- **Semantic case:** Same template + `variable_patterns: [{name: "dir", pattern: ...}]`.
  Auto-extract produces the raw value; the pattern validates and names it `vars.dir`.
  ToolBinding params reference `{{vars.dir}}`.

#### M.6 WebUI — template authoring feedback

In the intent expression input field:
- `%` characters rendered as a distinct chip/token (not plain text).
- Live feedback line shows computed prefix/suffix anchors and their classification
  (green = anchored, yellow = suffix-only leading-`%`, red = no-anchor blocked).
- On save: Q1 template rules run immediately; error/warning shown inline.

#### Files to create

- `crates/brassclaw_pg/migrations/V058__reborn_intent_inputs_template.sql` (**was V057 before Decision 2**)
- `crates/brassclaw_engine/src/memory/template_extractor.rs` (or inline in `intent_system.rs`)

#### Files to modify

- `crates/brassclaw_engine/src/memory/intent_system.rs`
  — `parse_template` helper
  — `seed_intent_input`: detect `%`, populate `is_template`/`template_prefix`/`template_suffix`
  — `resolve_intent`: three-path SQL query
- `crates/brassclaw_engine/src/memory/retrieval_source.rs`
  — `fetch_for_turn`: after template match, call `extract_template_slots` before `variable_patterns` refinement
- `crates/brassclaw_engine/src/memory/component_validator.rs` (Phase I)
  — template Q1 rules (adjacent slots, no-anchor, dangling patterns) — add here when Phase I runs

#### Tests

- Unit: `parse_template("show me files in the % directory")` → `Some(("show me files in the ", " directory"))`
- Unit: `parse_template("% directory")` → `Some(("", " directory"))`
- Unit: `parse_template("% in %")` → `Some(("", ""))` → Q1 rejects (no anchor)
- Unit: `parse_template("search for % in %")` → `Some(("search for ", ""))` — prefix-anchored, valid
- Unit: `parse_template("search for %")` → `Some(("search for ", ""))` — trailing `%`, prefix-anchored (FIND-P9-12)
- Unit: `parse_template("no slots here")` → `None`
- Unit: `extract_template_slots("show me files in the % dir", "show me files in the /tmp dir")` → `[("slot0", "/tmp")]`
- Unit: `extract_template_slots("search for % in %", "search for TODO in /src")` → `[("slot0", "TODO"), ("slot1", "/src")]`
- Unit: `extract_template_slots` with adjacent slots `"% %"` → empty / error (undefined behaviour blocked by Q1)
- Integration: `resolve_intent("show me all files in the /tmp directory")` → matches template row `"show me all files in the % directory"`
- Integration: `resolve_intent("show me all files in the /tmp directory")` — exact literal row present → ranks above template match for same component
- Integration: `resolve_intent("/tmp directory")` → matches suffix-anchored template `"% directory"` via reverse index
- Integration: slot values flow through to `{{vars.slot0}}` substitution in ToolBinding params

---

### Phase N — Validation Queue (Populate + Drop)

**Status:** [ ] Pending

> **§0.23.2 + §0.23.5 + §0.23.9 fold-in:** Phase N also (1) implements **orchestrated
> Q1** — a new `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`
> (`run_q1_validation`) that loads the validation Recipe for the component's class and
> runs a **full sandboxed agent-loop orchestrator** (existing `sandbox_process` /
> `process_executor`, restricted capabilities, per-validation token budget, cannot
> mutate production state); on a clean result it writes `state = 2` (the retained
> security invariant — only the Q1 orchestrator writes state 2; `gate1_pass`/
> `gate1_fail` stay `pub(crate)`). The pure-Rust `component_validator.rs` is **retired
> here** (not kept as a floor). (2) Wires **graduation for both paths**: new
> components flip to `'validated'` + delete queue row; **upgrade-copies** apply
> `proposed_payload` to the live row on Q2 approval (live row stayed validated+served
> until then), delete queue row. (3) **Seeds intents from graduated `variants`**
> (FIND-NEW-17 revised — intent seeding moves here from save-time, per §0.23.6).

**Goal:** Populate `reborn_validation_queue` from existing component tables; add the
`last_graduation_at` scope cursor; wire the graduation trigger; drop `queue_code`,
`review_attempts`, `review_feedback`, `rejected_at`, and `validation_errors` off the
13 component tables.

> **Decision 2:** `reborn_validation_queue` table and `ValidationQueueStore` were created
> in Phase A.5 (V051). Phase N only contains: Step 2 (populate), Step 3 (last_graduation_at),
> Step 4 (trigger), Step 5 (DROP). **Step 1 (CREATE TABLE) is NOT in V059 — the table
> already exists.** Phase N.2 (`ValidationQueueStore`) is also **already done** at Phase A.5 —
> mark it as [DONE] when you reach Phase N.

> **Pre-requisite awareness:** Phase N touches 13 component tables. Each column removal
> is a two-step migration: (2) populate the queue from the component columns (data migration),
> then (5) drop the now-redundant columns. Both steps are in V059, additive-first,
> destructive-second within one transaction to guarantee atomicity.

#### N.1 New migration: V059 (**was V058 before Decision 2**)

**File:** `crates/brassclaw_pg/migrations/V059__reborn_validation_queue_populate.sql`

```sql
-- Step 1: CREATE TABLE is NOT here — reborn_validation_queue was created in
-- V051__reborn_validation_queue.sql (Phase A.5). Do NOT re-create the table.

-- Step 2: populate from existing component table state
-- For every component that is NOT yet 'validated':
-- Map validation_status → state, review_attempts → counter, etc.
INSERT INTO reborn_validation_queue
    (tenant_id, user_id, agent_id, project_id,
     component_id, component_class, state, counter,
     review_feedback, validation_errors, submitted_at)
SELECT
    tenant_id, user_id, agent_id, project_id,
    id,
    -- ⚠️ FIND-P6-08: Use the actual class_code column value for tables with variable class codes
    -- (reborn_skills: 1/2/3; reborn_extensions_unified: 4-9).
    -- Use a literal for tables with a single fixed class code
    -- (reborn_recipes: 21, reborn_actions: 16, reborn_tools: 0, etc.).
    -- Example for reborn_skills (variable class code):
    class_code::SMALLINT,
    CASE validation_status
        WHEN 'pending'           THEN 1
        WHEN 'upgrade_queued'    THEN 1
        WHEN 'auto_failed'       THEN 1
        WHEN 'auto_passed'       THEN 2
        WHEN 'review_requested'  THEN 2
        WHEN 'rejected'          THEN 3
        WHEN 'garbage'           THEN 4
        ELSE 1
    END,
    COALESCE(review_attempts::INT, 0),  -- SCHEMA-01: ::INT cast on every arm (uniform)
    review_feedback,
    validation_errors,
    created_at
FROM reborn_skills
WHERE validation_status != 'validated'
-- Repeated for each of the 15 component tables with the correct class_code value:
-- the 13 existing tables + reborn_python_code (class 22) and reborn_extension_catalogues
-- (class 23). The two Phase B/C tables carry validation_status (the column that STAYS on
-- the component table — see §0.18) even though they never carried the five queue-tracking
-- columns, so their pending rows MUST be populated here too.
--
-- IMPLEMENTATION NOTE: the SELECT above references review_attempts / review_feedback /
-- validation_errors — columns the 13 EXISTING tables have. The two Phase B/C tables
-- (reborn_python_code, reborn_extension_catalogues) do NOT have these columns, so their
-- per-table INSERT arm must substitute literal defaults in place of the column refs:
--     COALESCE(NULL, 0) AS counter,           -- 0  (no prior attempts)
--     NULL       AS review_feedback,           -- no Q2 feedback yet
--     '{}'::TEXT[] AS validation_errors         -- no Q1 errors yet
-- (the scope columns tenant_id/user_id/agent_id/project_id and the CASE on
-- validation_status ARE available on all 15 tables). Failing to substitute the
-- literals would make the 22/23 arm reference a non-existent column and abort V059.
-- ⚠️ FIND-P9-11: this is V059 (not V058; after Decision 2 V-number shift).
ON CONFLICT DO NOTHING;

-- Step 3: add last_graduation_at to scope cursor
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN IF NOT EXISTS last_graduation_at TIMESTAMPTZ;

-- Step 4: trigger — bump last_graduation_at on queue row DELETE (= graduation)
-- Uses INSERT ... ON CONFLICT so the first graduation creates the cursor row
-- atomically even when no reborn_monty_vm_settings row exists yet for the scope
-- (every resource column has a NOT NULL DEFAULT in V034, so the 5-column INSERT
-- succeeds and the remaining columns take their defaults). This matches the
-- existing PgMontyVmSettingsStore::upsert pattern (pg_monty_vm_settings.rs:162-179,
-- an INSERT ... ON CONFLICT ON CONSTRAINT ... DO UPDATE). See the FINDING D
-- resolution note below.
CREATE OR REPLACE FUNCTION reborn_validation_queue_graduation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- Requires: reborn_monty_vm_settings_scope_unique UNIQUE constraint (V034).
    INSERT INTO reborn_monty_vm_settings
        (tenant_id, user_id, agent_id, project_id, last_graduation_at)
    VALUES
        (OLD.tenant_id, OLD.user_id, OLD.agent_id, OLD.project_id, now())
    ON CONFLICT (tenant_id, user_id, agent_id, project_id)
    DO UPDATE SET last_graduation_at = now();
    -- ⚠️ FIND-13 FIX: AFTER DELETE triggers should RETURN NULL.
    -- PostgreSQL ignores the return value of AFTER triggers. RETURN OLD works
    -- but is the idiom for BEFORE triggers (it signals "proceed with the row op").
    -- RETURN NULL is the correct idiom for AFTER triggers.
    RETURN NULL;
END;
$$;
CREATE TRIGGER reborn_validation_queue_on_delete
    AFTER DELETE ON reborn_validation_queue
    FOR EACH ROW EXECUTE FUNCTION reborn_validation_queue_graduation();

-- Step 5: drop redundant columns from component tables
-- (After data has been migrated to the queue)
ALTER TABLE reborn_skills
    DROP COLUMN IF EXISTS queue_code,
    DROP COLUMN IF EXISTS review_attempts,
    DROP COLUMN IF EXISTS review_feedback,
    DROP COLUMN IF EXISTS rejected_at,
    DROP COLUMN IF EXISTS validation_errors;
-- Repeated for all 13 EXISTING component tables (the ones that carried these columns).
-- The Phase B/C tables (reborn_python_code, reborn_extension_catalogues) never carried
-- these five columns (see §0.18 / Phase B+C "Do NOT include") so there is nothing to
-- drop from them — no ALTER is needed for those two.
-- validation_status is NOT dropped — it remains as the post-validation gate.
```

**Note on scope cursor:** `reborn_monty_vm_settings` has a guaranteed row for every
active scope.

> **✅ FINDING D — RESOLVED (pre-v3 audit): struck as stale / factually wrong.**
> The original block here asserted that `PgMontyVmSettingsStore::upsert` was "NOT a true
> INSERT … ON CONFLICT" and that the graduation trigger used a "bare `UPDATE`" that would be
> a "silent no-op". Both premises are **false** against the current code and this plan's own
> Step-4 SQL above. See the **Review note (pre-v3 audit)** immediately below for the full
> line-by-line verification. Summary: `upsert` (`pg_monty_vm_settings.rs:108`; SQL `:162–179`)
> **is** an `INSERT … ON CONFLICT ON CONSTRAINT … DO UPDATE` (creates the row if absent); the
> Step-4 trigger **already** uses `INSERT … ON CONFLICT DO UPDATE`; the V034 schema
> (`NOT NULL DEFAULT …` on every resource column) makes the 5-column INSERT valid. **No
> Phase N code change is required** — do not add a duplicate `INSERT … ON CONFLICT` "fix" and
> do not rewrite `upsert` (doing so risks introducing a real bug). The original "Required
> fix" / "bare UPDATE" / "silent no-op" text that occupied this block has been removed.

> **Review note (pre-v3 audit) — FINDING D is STALE and factually wrong; the N.1 SQL already
> does the right thing. Verify before implementing — do NOT "fix" what is not broken:**
> 1. **The premise is false.** `PgMontyVmSettingsStore::upsert`
>    (`crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs:108`; the write SQL is at
>    `:162–179`) **is** a true `INSERT INTO reborn_monty_vm_settings (…) VALUES (…) ON CONFLICT ON
>    CONSTRAINT reborn_monty_vm_settings_scope_unique DO UPDATE SET …`. It does read the current
>    row first (`self.get(…)` at `:125`) — but only to fill in *unchanged* fields for the
>    `DO UPDATE SET` clause; the write itself is an `INSERT … ON CONFLICT … DO UPDATE`, which
>    **creates** the row if absent. So "it will NOT create a row if one does not yet exist" is
>    incorrect. (The line cite "line 103" is also off — the `pub async fn upsert` signature is at
>    `:108`.)
> 2. **The N.1 SQL block already uses the recommended form.** The
>    `reborn_validation_queue_graduation()` trigger above (lines ~3371–3376) already issues
>    `INSERT INTO reborn_monty_vm_settings (tenant_id, user_id, agent_id, project_id,
>    last_graduation_at) VALUES (…) ON CONFLICT (tenant_id, user_id, agent_id, project_id) DO
>    UPDATE SET last_graduation_at = now()` — i.e. option (b) is *already implemented* in the
>    plan. FINDING D's claim that the block contains a "bare `UPDATE`" describes a state that
>    does not exist in this plan. The conflict target `(tenant_id, user_id, agent_id,
>    project_id)` is backed by the `reborn_monty_vm_settings_scope_unique` UNIQUE constraint
>    (`V034:67–69`), so the `ON CONFLICT (…)` resolves correctly.
> 3. **The INSERT is valid against the V034 schema.** `reborn_monty_vm_settings` (`V034:15–69`)
>    has `id DEFAULT gen_random_uuid()` and every resource column (`max_duration_secs`,
>    `max_allocations`, `max_memory_bytes`, `failure_rollback_threshold`,
>    `prior_knowledge_token_budget`, `q4_retention_days`, `forensic_packet_retention_days`) is
>    `NOT NULL DEFAULT …`; `created_at`/`updated_at` default to `now()`. Only
>    `active_orchestrator_id` is nullable. So the trigger's 5-column INSERT (plus
>    `last_graduation_at`, added nullable in Step 3 of this same migration) succeeds — the
>    remaining columns take their defaults — and the first graduation atomically creates the
>    cursor row. The "silent no-op" alarm is unfounded.
> **Action:** treat FINDING D as already-resolved/superseded. Do not add a second
> `INSERT … ON CONFLICT` "fix" on top of the existing trigger (it would be a no-op duplicate),
> and do not rewrite `PgMontyVmSettingsStore::upsert` — it is already correct. If anything,
> update the FINDING D prose to match the SQL (or delete it) so a future implementer does not
> "correct" a correct trigger and risk introducing a real bug.

No separate `reborn_scope_cursors` table is needed; the Step-4 trigger SQL above is already the `INSERT ... ON CONFLICT DO UPDATE` form (FINDING D resolved — see note above).

#### N.2 Application-layer write paths ✅ **ALREADY DONE — see Phase A.5 (Decision 2)**

> **Decision 2:** `ValidationQueueStore` was moved to Phase A.5. By the time Phase N is
> reached, `validation_queue.rs` already exists and the store is fully implemented.
> Mark this section [DONE] and skip implementation. The spec below is retained for
> reference only (verify the Phase A.5 impl matches it; if any method is missing, add it).

**File:** `crates/brassclaw_reborn_composition/src/validation_queue.rs` (already created in Phase A.5)

```rust
pub struct ValidationQueueStore { /* pool */ }

impl ValidationQueueStore {
    /// Submit a component to Q1 queue (state 1).
    /// Called when a component is created or edited.
    pub async fn submit(&self, scope, component_id, component_class) -> Result<()>;

    /// Transition state 1 → state 2. ONLY called by Gate 1 validator on clean pass.
    /// Returns Err if called from any other context (enforced by Rust visibility:
    /// this method is pub(crate) and only reachable from component_validator.rs).
    pub(crate) async fn gate1_pass(&self, scope, component_id, errors: &[]) -> Result<()>;

    /// Record Q1 failure — stays in state 1, increments nothing (author must fix and resubmit).
    pub(crate) async fn gate1_fail(&self, scope, component_id, errors: &[String]) -> Result<()>;

    /// Q2 rejection: state 2 → state 3. Increments counter. Promotes to state 4 if counter >= threshold.
    pub async fn reject(&self, scope, component_id, feedback: &str) -> Result<()>;

    /// Q2 approval: delete queue row → graduation. Updates component's validation_status = 'validated'.
    pub async fn approve(&self, scope, component_id) -> Result<()>;

    /// List all queue rows for a scope (WebUI validation view).
    pub async fn list(&self, scope, state_filter: Option<u8>) -> Result<Vec<QueueRow>>;

    /// Deletion candidate cleanup: delete state-4 rows and their components.
    pub async fn purge_deletion_candidates(&self, scope) -> Result<u64>;
}
```

**Visibility invariant for `gate1_pass`:** `pub(crate)` — only callable from within
`brassclaw_reborn_composition`. The Gate 1 validator lives in this crate. The API
layer (webui_v2, ingress) cannot call `gate1_pass` directly — it can only call `submit`.
This is the Rust-level enforcement of the state-2 write invariant.

#### N.3 Cache integration

The SplitResult memo-cache in `PostgresSource` gains a `last_graduation_at` check:

```rust
// On every cache hit, before returning the cached SplitResult:
// NOTE: Use tokio_postgres directly (the codebase does NOT use sqlx; pg_pool is
// brassclaw_pg::PgPool backed by deadpool-postgres / tokio-postgres, NOT sqlx).
// The sqlx::query_scalar! macro below is pseudocode only — replace with pool.get()
// + client.query_opt() as used throughout retrieval_source.rs:
let client = pool.get().await.map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
let row = client.query_opt(
    "SELECT last_graduation_at FROM reborn_monty_vm_settings
     WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4",
    &[&scope.tenant_id, &scope.user_id, &scope.agent_id, &scope.project_id],
).await.map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
let cursor: Option<chrono::DateTime<chrono::Utc>> = row
    .and_then(|r| r.get::<_, Option<chrono::DateTime<chrono::Utc>>>(0));
// Replace the sqlx pseudocode below with the pattern above when implementing.
//
// ORIGINAL PSEUDOCODE (sqlx — DO NOT USE):
// let cursor = sqlx::query_scalar!(
//     "SELECT last_graduation_at FROM reborn_monty_vm_settings
//      WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4",
//     scope.tenant_id, scope.user_id, scope.agent_id, scope.project_id
// ).fetch_optional(pool).await?;

// ⚠️ FIND-NEW-16: cursor is already Option<DateTime<Utc>> (not Option<Option<...>>).
// The double-Some pattern `if let Some(Some(...))` is WRONG here.
// `row.and_then(|r| r.get::<_, Option<DateTime<Utc>>>(0))` returns Option<DateTime<Utc>>.
if let Some(graduated_at) = cursor {
    if graduated_at > cache_entry.cached_at {
        cache.remove_scope(scope);
        // Recompute — fall through to full fetch_for_turn
    }
}
```

One PK read per cache hit. Sub-millisecond. No TTL needed. Cache entries for a scope
are evicted as a batch when any component in the scope graduates — conservative but
correct. Fine-grained per-component eviction is a future optimisation.

#### N.4 Component table cleanup

Remove from all 13 component tables: `queue_code`, `review_attempts`, `review_feedback`,
`rejected_at`, `validation_errors`.

> **Rust struct sync required:** After V059 drops these columns, ALL structs that read
> or write them must be updated atomically. Affected:
>
> - **`Recipe` + `ToolSkill` in `crates/brassclaw_engine/src/types/recipe.rs`:**
>   Confirmed by inspection — these structs DO carry `validation_errors: Vec<String>`,
>   `review_feedback: Option<String>`, `review_attempts: u32`, `rejected_at: Option<DateTime<Utc>>`.
>   They do NOT have `queue_code` (queue_code is only in `PgRecipe`, not the domain type).
>   Remove the four fields listed above.
>
> - **⚠️ SCHEMA-01 — `review_attempts` type inconsistency across tables:**
>   Verified against every migration (grep `review_attempts` + type): **10 tables define
>   `review_attempts SMALLINT`** — `reborn_extensions_unified` (V032), `reborn_recipes`
>   (V033), `reborn_specs` (V036), `reborn_tool_skills` (V037), `reborn_plans` (V038),
>   `reborn_summaries` (V039), `reborn_docus` (V040), `reborn_lessons` (V041),
>   `reborn_issues` (V042), `reborn_notes` (V043) — and **3 tables define it `INT`** —
>   `reborn_skills` (V027), `reborn_actions` (V029), `reborn_tools` (V030). `PgRecipe.
>   review_attempts` is `i16` (matching SMALLINT for recipes). This type inconsistency is
>   pre-existing and affects V059's populate step: each per-table arm's
>   `COALESCE(review_attempts, 0)` feeds `reborn_validation_queue.counter INT`. For the 10
>   SMALLINT arms the expression's type is SMALLINT; PostgreSQL *will* implicitly widen
>   SMALLINT→INT on INSERT into the INT column, so it does not hard-fail — BUT relying on
>   the implicit assignment cast is fragile (a future reader cannot tell if it is
>   intentional) and the plan's prior wording ("for reborn_recipes rows: cast needed")
>   only named ONE of the ten SMALLINT tables. **Cleanest fix (apply on EVERY per-table
>   arm, both SMALLINT and INT): `COALESCE(review_attempts::INT, 0)`.** `INT::INT` is a
>   harmless no-op for the 3 INT tables; the cast makes all 13 arms produce INT
>   deterministically, removing any reliance on implicit-cast rules and any ambiguity.
>   Also update the populate SQL example (the `counter` line) to use this cast. This is a
>   minor V059 migration detail but must be handled uniformly to avoid runtime type
>   surprises.
>
> - **`PgRecipe` in `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`:**
>   Confirmed by inspection — `RECIPE_SELECT` selects `queue_code`, `review_attempts`,
>   `review_feedback`, `rejected_at`, `validation_errors` and the struct has matching fields.
>   Remove all five from both `PgRecipe` and `RECIPE_SELECT`.
>
> - **`RecipeValidationStatusUpdate` in `pg_recipe_store.rs`:**
>   This param struct has `validation_errors`, `review_feedback`, `queue_code` fields.
>   Must be updated when the columns are dropped.
>
> - **`component_validator.rs`** — creates `Recipe` structs with `validation_errors`.
> - **`recipe_matcher.rs`** — reads `wilson_lower` + `tier` (NOT dropped by V059).
>   **⚠️ FIND-P5-08:** After removing `validation_errors` / `review_feedback` /
>   `review_attempts` / `rejected_at` from the Rust structs, `cargo check` will catch
>   ALL struct-field references that no longer exist. Run `cargo check --all` immediately
>   after the struct changes and resolve every error before running V059. The "audit
>   required" is the Rust compiler, not a manual inspection.
> - Any other caller that constructs or destructures these structs: identified fully by
>   `cargo check` after the struct fields are removed.
>
> **⚠️ FIND-P6-04 — `decode_recipe_row` re-index map after Phase N drops 5 columns.**
> Phase A appended `step_descriptions`, `variants`, `dependency_registry` at indices 31, 32, 33.
> Phase N drops 5 columns from the middle. After the drops, the NEW indices for `decode_recipe_row`
> are (mapping old index → new value for each field that SURVIVES):
>
> | Old index | Field | New index after drops |
> |-----------|-------|-----------------------|
> | 0 | id | 0 |
> | 1 | tenant_id | 1 |
> | 2 | user_id | 2 |
> | 3 | agent_id | 3 |
> | 4 | project_id | 4 |
> | 5 | name | 5 |
> | 6 | description | 6 |
> | 7 | trigger | 7 |
> | 8 | steps | 8 |
> | 9 | status | 9 |
> | 10 | prior_knowledge_content | 10 |
> | 11 | override_prompt_creation | 11 |
> | 12 | class_code | 12 |
> | 13 | prompt_uid | 13 |
> | 14 | consumer_tags | 14 |
> | 15 | intent_examples | 15 |
> | 16 | tier | 16 |
> | 17 | usage_count | 17 |
> | 18 | success_count | 18 |
> | 19 | failure_count | 19 |
> | 20 | wilson_lower | 20 |
> | 21 | validation_status | 21 |
> | 22 | ~~validation_errors~~ | **DROPPED** |
> | 23 | ~~review_feedback~~ | **DROPPED** |
> | 24 | ~~review_attempts~~ | **DROPPED** |
> | 25 | ~~rejected_at~~ | **DROPPED** |
> | 26 | ~~queue_code~~ | **DROPPED** |
> | 27 | source | **22** |
> | 28 | content_hash | **23** |
> | 29 | created_at | **24** |
> | 30 | updated_at | **25** |
> | 31 | step_descriptions (Phase A) | **26** |
> | 32 | variants (Phase A) | **27** |
> | 33 | dependency_registry (Phase A) | **28** |
>
> `RECIPE_SELECT` must also drop the 5 columns from its column list, and `PgRecipe` struct
> must lose those 5 fields. `decode_recipe_row` must be completely rewritten with these new indices.
> Run `cargo check --all` after the struct changes and resolve every compile error.

> **Two-phase deploy required (zero-downtime):**
> V059 drops columns. If the old binary is still running when V059 runs (rolling deploy),
> it will SELECT dropped columns → runtime panic on every request. Required deploy order:
> 1. Deploy new binary (with structs updated to use `Option<T>` + `#[serde(default)]`
>    for the fields being dropped — existing data still returns values, new `None` is fine).
> 2. Run V059 migration (now safe — binary no longer queries dropped columns as required).
> 3. Remove the `Option` wrappers in a follow-up cleanup commit.

> **✅ Review note (pre-v3 audit) — N.4 struct audit verified with exact line refs — RESOLVED:**
> the positional-re-index caveat is captured below (dropping a column from `RECIPE_SELECT`
> requires renumbering every later `row.get(N)` in `decode_recipe_row`); the N.4 task now
> carries that re-index instruction so the column-drop migration does not silently read the
> wrong column. This gates the Phase N column-drop migration — no code change yet.
> Original detail retained:
> Confirmed against current code: the engine `Recipe` struct (`types/recipe.rs:144`) carries
> `validation_errors: Vec<String>` (:167), `review_feedback: Option<String>` (:168),
> `review_attempts: u32` (:169), `rejected_at: Option<DateTime<Utc>>` (:170) and has **no**
> `queue_code` (matches N.4). `PgRecipe` (`pg_recipe_store.rs:117`) + `RECIPE_SELECT` (:208–217)
> select and decode all five incl. `queue_code` (:120, :236 area) — `decode_recipe_row` (:219) is
> positional, so dropping a column requires renumbering every `row.get(N)` index after it (the
> plan's "remove all five from both `PgRecipe` and `RECIPE_SELECT`" must therefore also re-index
> `decode_recipe_row`, not just delete the lines). `RecipeValidationStatusUpdate` (:170) does
> carry `validation_errors`/`review_feedback`/`queue_code`. `recipe_matcher.rs` reads
> `wilson_lower` + `tier` (not dropped) and does reference the dropped fields in conversion
> paths — the N.4 "audit required" is genuine. The two-phase deploy is the correct mitigation.

The 13 tables are: `reborn_skills`, `reborn_tools`, `reborn_tool_skills`,
`reborn_recipes`, `reborn_actions`, `reborn_specs`, `reborn_plans`, `reborn_summaries`,
`reborn_lessons`, `reborn_docus`, `reborn_issues`, `reborn_notes`,
`reborn_extensions_unified`, plus the new Phase B/C tables `reborn_python_code` and
`reborn_extension_catalogues` — which are designed without these columns from
the start (they use the queue from day one, as of Phase A.5 / V051).

**`reborn_python_code` and `reborn_extension_catalogues` (Phases B and C):** These
tables are created in V052/V053, **after** `reborn_validation_queue` exists (V051/Phase A.5).
They must NOT include `queue_code`, `review_attempts`,
`review_feedback`, `rejected_at`, or `validation_errors` columns (those five are
centralised on the queue). They DO carry `validation_status` (the post-validation gate,
which STAYS on the component table — see §0.18). WebUI-authored rows in these tables can
enter the queue from day one (Phase A.5 created the queue table before these tables exist).
The V059 populate step (Phase N) back-fills any pending rows from the 13 pre-existing
tables; for classes 22/23 there is no gap to back-fill since the queue already existed
when these tables were created.

#### N.5 Integrity check at boot

A boot-time check (in `brassclaw_reborn_composition` init sequence):

```sql
-- Components not in 'validated' state that have no queue row are inconsistent.
-- UNION ALL over EVERY component table that carries validation_status (15 today):
-- the 13 pre-existing tables PLUS the V052/V053 additions (classes 22/23).
-- Classes 22/23 have had a queue available since V051 (Phase A.5); their rows
-- should already have queue entries if Phase B/C submit() calls were implemented.
-- This check is still important for crash-recovery / manual imports.
SELECT id AS component_id, 'skills'      AS source FROM reborn_skills
WHERE validation_status != 'validated'
  AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'actions'     FROM reborn_actions            WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'tools'       FROM reborn_tools              WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'extensions'  FROM reborn_extensions_unified WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'recipes'     FROM reborn_recipes            WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'specs'       FROM reborn_specs              WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'tool_skills' FROM reborn_tool_skills        WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'plans'       FROM reborn_plans              WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'summaries'   FROM reborn_summaries          WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'docus'       FROM reborn_docus              WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'lessons'     FROM reborn_lessons            WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'issues'      FROM reborn_issues             WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'notes'       FROM reborn_notes              WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'python_code'    FROM reborn_python_code           -- V052 / class 22
                                           WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
UNION ALL SELECT id, 'ext_catalogues' FROM reborn_extension_catalogues  -- V053 / class 23
                                           WHERE validation_status != 'validated' AND id NOT IN (SELECT component_id FROM reborn_validation_queue WHERE tenant_id = $1 AND ...)
-- (The $1 tenant_id and per-table filter are applied per scope; the host-side
--  loop iterates the 15 tables, or this is emitted as one prepared statement.)
```

Inconsistent rows are logged as warnings and automatically submitted to state 1 as
a recovery action. This covers edge cases from the V059 data migration (crash-recovery)
and ensures all 15 tables are monitored uniformly. For classes 22/23, rows without
queue entries should be rare (Phase B/C `submit()` calls) but the safety net is still
required for manual imports, restored backups, or any path that bypasses the WebUI.

#### Tests

- Unit: `gate1_pass` is `pub(crate)` — not callable from outside the crate (compile-time)
- Unit: submit → `state = 1`
- Unit: `gate1_pass` → `state = 2`; `gate1_fail` → `state = 1`, errors populated
- Unit: `reject` → `state = 3`, counter incremented
- Unit: `reject` when `counter >= threshold` → `state = 4` (auto-promotion)
- Unit: `approve` → queue row deleted; component `validation_status = 'validated'`
- Integration: component approval → `last_graduation_at` bumped on scope cursor
- Integration: cache hit after graduation → cache entry discarded, SplitResult recomputed
- Integration: cache hit with no graduation → cached result returned, no recompute
- Integration: boot integrity check → components with missing queue rows auto-submitted
- Integration: `list(state_filter: Some(2))` → returns only Q1-passed components awaiting Q2

### Phase O — Global Token-Budget Kill Switch (§0.21)

**Status:** [ ] Pending

**Goal:** Add the operator-facing WebUI toggle that, when disabled, makes
every token budget in the codebase play no role in any decision or function
(§0.21). When re-enabled, all token budgets are enforced exactly as today.
Additive and independently shippable — no other phase depends on it.

**Migration:** `V060__reborn_monty_vm_settings_token_budgets_enabled.sql`

```sql
ALTER TABLE reborn_monty_vm_settings
    ADD COLUMN token_budgets_enabled BOOLEAN NOT NULL DEFAULT true;
```

Additive only. Existing rows backfill to `true` (today's behaviour). No
DROP, no rename. This is the *only* schema change in Phase O — the setting
rides the existing `reborn_monty_vm_settings` table (V034) that already
holds `prior_knowledge_token_budget`, so no new table is minted.

**Files to create / modify (in dependency order):**

1. **Migration** `V060__…sql` (above).
2. **`crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs`** —
   add `token_budgets_enabled: bool` (default `true`) to the read struct +
   `Option<bool>` to the update path; extend the `SELECT` column list and
   the `INSERT … ON CONFLICT … DO UPDATE` to set
   `token_budgets_enabled = EXCLUDED.token_budgets_enabled` (mirror the
   existing `prior_knowledge_token_budget` read at `:83` and upsert at
   `:142-190`). Co-locate the new `TokenBudgetPolicy { enabled: bool }`
   type + `pub fn cap_or_unlimited(cap: usize) -> usize` here (returns
   `cap` when `enabled`, `usize::MAX` when not). Add a
   `load_token_budget_policy(user_id, project_id)` read that returns the
   policy for the scope (cached for the turn).
3. **`crates/brassclaw_product_workflow/src/settings.rs`** — add
   `token_budgets_enabled: bool` (default `true`) to `MontyVmSettings`
   (near `:48`) and `Option<bool>` to `UpdateMontyVmSettingsRequest`
   (near `:64`); default the read to `true` at `:193`.
4. **Facade / HTTP** — extend the existing `GET` and `PUT
   /api/settings/monty-vm` handler to read + persist
   `token_budgets_enabled`. No new route, no new descriptor: the Monty-VM
   settings endpoint already exists; this is one more field on the same
   DTO. Update `tests/webui_v2_handlers_contract.rs` (the
   `prior_knowledge_token_budget: 2000` sites at `:893`/`:912`) to include
   `token_budgets_enabled`.
5. **Thread the policy** from composition (turn start) into the loop
   (`brassclaw_agent_loop`), the engine executor (`brassclaw_engine`), the
   retrieval source (`fetch_for_turn`), the interceptor, the skills
   registry, and the LLM layer. Read once per turn; pass `&TokenBudgetPolicy`
   (or a `bool`) down — no per-consumer DB reads.
6. **Apply `cap_or_unlimited` / `enabled()` at every consumer (§0.21 table):**
   - `crates/brassclaw_engine/src/executor/orchestrator.rs:2844
     handle_check_budget` — `tokens_remaining = u64::MAX` when disabled
     (leave `time_remaining_ms` / `usd_remaining` unchanged).
   - `crates/brassclaw_engine/src/types/thread.rs` — skip
     `max_tokens_total` enforcement when disabled.
   - `crates/brassclaw_agent_loop/src/token_budget.rs` — construct
     `TokenBudgetTracker` with `usize::MAX` when disabled; `would_exceed`
     returns `false`; message selection never drops on token budget.
   - `fetch_for_turn` budget argument → `usize::MAX` when disabled (full
     assembled prior knowledge injected, no truncation).
   - `crates/brassclaw_skills/src/registry.rs` — skill budget unlimited.
   - `crates/brassclaw_interceptor/src/packet.rs` — packet token budget
     unlimited.
   - `crates/brassclaw_llm/*` per-request output `max_tokens` → provider
     documented max (or omitted) when disabled.
7. **WebUI** `crates/brassclaw_webui_v2_static` — add a toggle card to the
   existing Settings (Monty-VM) page: "Token budgets enabled" (default on)
   with help text stating the cost/runaway implication and that time + USD
   limits remain. PUT `token_budgets_enabled` via the existing settings
   hook + `apiFetch` (no new endpoint). Add the i18n key (e.g.
   `"settings.tokenBudgetsEnabled"`) to `i18n/en.js` and all other language
   packs. `node --check` the changed JS.

**Tests:**
- Unit: `TokenBudgetPolicy::cap_or_unlimited(8000)` → `8000` when enabled,
  `usize::MAX` when disabled.
- Unit: `handle_check_budget` with policy disabled →
  `tokens_remaining == u64::MAX`; `time_remaining_ms` / `usd_remaining`
  still computed from config (unchanged).
- Unit: `TokenBudgetTracker` constructed under a disabled policy →
  `remaining()` never `0`, `would_exceed(n)` `false` for any `n`.
- Integration: `PUT token_budgets_enabled=false` → `GET` returns `false`;
  a turn runs with prior-knowledge injection uncapped (assert the full
  assembled blob is injected, no truncation); `__check_budget__` reports
  `tokens_remaining == u64::MAX`.
- Integration: toggle back to `true` → caps re-enforced (truncation +
  `tokens_remaining` depletion resume).
- Security: the toggle endpoint is bearer-authenticated; an unauthenticated
  PUT is rejected (mirror the existing settings endpoint's auth test).

**Safety / scope notes:** Disabling removes a cost/runaway guard; time and
USD budgets remain enforced as backstops and are **not** affected by this
switch (§0.21). The toggle is operator-only and logged. Out of scope: any
change to time/USD budgets.

### Phase P.0 — Validation-system extension: automated-but-auditable Q2 (prerequisite for Phase P; Answer 2)

**Status:** [ ] Pending

> **§0.23.9 + §0.23.10 fold-in:** Phase P.0's automated-but-auditable Q2 (V061
> `q2_actor`) also graduates the **Phase-L validation-system trusted-root
> components** — the seeder/automation is the recorded Q2 actor for every
> `source='system'` component, whether a builtin tool or a validation-system
> Extension/Recipe/formatter. No `source='system'` component bypasses Q1+Q2.

**Goal:** So that system-authored/builtin components — including the
doc-conversion mechanism's own artifacts (§0.22) and its converted docs —
graduate through Q1+Q2 with **no silent bypass** (Answer 2: "Nothing ever
bypasses the Q1+Q2 system"). This **revises §0.16 / Phase L**, which currently
let builtins skip the queue (Open Question #8 — now superseded by Answer 2),
and unblocks Phase P's no-bypass stance.

**What changes.** Every component — including `source='system'` builtin seeds
— enters `reborn_validation_queue` at `validation_status='pending'`, runs Q1
(Gate 1, `component_validator.rs`), and then a **recorded Q2 graduation**
(automated for system-authored: the seeder/automation is the Q2 *actor*,
recorded in the queue, never a silent skip). No code path writes `validated`
without a queue graduation record. `source` is provenance only and never gates
validation.

**Migration (TBD by subplan):** possibly one small additive column on
`reborn_validation_queue` (V051) to record the Q2 actor type
(`auto-system` vs `human`) so automated graduations are auditable/distinct.
To be confirmed against V051's actual columns when the subplan is written —
not invented here.

**Files (indicative):** `crates/brassclaw_reborn_composition/src/q1_orchestrator.rs`
(the cross-crate Q1 orchestration — FIND-P9-01), the Phase L
`builtin_bootstrap.rs` seeder (must enqueue + record Q2, **not** insert
`validated` directly), `ValidationQueueStore` (the automated-Q2 graduation
method), and the WebUI validation-queue tab (surface auto vs human Q2 actor).

**Depends on:** V051 (Phase A.5 — queue table).

**Tests:**
- Unit: a system-authored component submitted → Q1 passes → automated Q2
  graduation recorded (actor=`auto-system`) → row `validation_status='validated'`.
- Unit/security: no code path inserts `validation_status='validated'` without a
  queue graduation record (grep-enforced + a store-level guard).
- Integration: the Phase L builtin bootstrap seeds a builtin Tool through the
  queue and it graduates (no direct `validated` insert).

### Phase P.1 — Migrate on-disk system skills to DB rows through Q1+Q2 (prerequisite for Phase P; audit finding)

**Status:** [ ] Pending

**Goal:** Remove **both** pre-v3 filesystem-skill mechanisms (validation-bypass
audit finding 1): on-disk `SKILL.md` system skills become `reborn_skills` DB
rows that pass Q1+Q2 (via Phase P.0). Also satisfies v3 goal 5.1 (skills are
DB-stored; no physical `SKILL.md` — exportable on demand via the WebUI,
Phase K.1 §K.1.7).

**Background — two separate mechanisms, not one.** There are two independent
pre-v3 filesystem-skill paths that both need removal:

1. **`bundled_skills.rs`** (`crates/brassclaw_reborn_composition/src/bundled_skills.rs`) —
   a *composition-layer* boot installer. At build time,
   `crates/brassclaw_reborn_composition/build.rs` (`embed_reborn_skills()`) walks
   the `skills/` source tree and compiles each skill bundle into two `include_str!()`
   JSON blobs (`embedded_reborn_skill_summaries.json`,
   `embedded_reborn_skill_bundles.json`). At runtime,
   `ensure_bundled_reborn_skills_installed()` installs those blobs onto the virtual
   filesystem under `/projects/system/skills/` with a content-hash marker file for
   idempotent re-install and stale-removal. **This path is already inert:** the
   `skills/` source tree was deleted in Phase 6 and `archive/skills-v1/` is absent,
   so `build.rs` emits empty JSON arrays (`[]`) today. The entire path is also gated
   by the `skills-db` Cargo feature (`brassclaw_reborn_composition/Cargo.toml`) —
   when that feature is active the build emits empty arrays and the module is
   cfg-gated out. Phase P.1 **deletes** `embed_reborn_skills()` from `build.rs`,
   the two `include_str!()` blobs, and `ensure_bundled_reborn_skills_installed()` /
   its callers (or reduces them to a no-op stub while unwinding call sites
   separately). **Do this first** since it is already a no-op and removal is safe.

2. **`management.rs` `SkillSource::System`** (`crates/brassclaw_skills/src/management.rs`) —
   a *skills-subsystem* loader. `SYSTEM_SKILLS_ROOT="/system/skills"` (`:34`) is
   read at `:243`/`:263` and loaded as `SkillSource::System` via `parse_skill_md`
   — without ever becoming `reborn_skills` DB rows, so Q1+Q2 never run. This is
   the **active** validation bypass. Remove this second, after the skills are
   migrated to DB rows via the Phase P.0 path.

**Pre-flight check.** Before deleting `bundled_skills.rs`, confirm
`embed_reborn_skills()` already emits `[]` (grep for `skills/` directory
absence; run `cargo build -p brassclaw_reborn_composition` and inspect
`$OUT_DIR/embedded_reborn_skill_bundles.json`). The `skills-db` feature can
be used as an immediate kill-switch without code deletion if needed.

**What changes.**

*Step A — `bundled_skills.rs` removal (already inert):*
Remove `embed_reborn_skills()` and the `println!("cargo:rerun-if-changed=…")`
for `skills_dir`/`archive_skills_dir` from `build.rs`. Delete
`crates/brassclaw_reborn_composition/src/bundled_skills.rs` and its
`mod bundled_skills` declaration in `lib.rs`. Remove all call sites of
`ensure_bundled_reborn_skills_installed()` and `bundled_reborn_skill_summaries()`.

*Step B — `management.rs` `SkillSource::System` removal (active bypass):*
The on-disk system skills loaded at
`crates/brassclaw_skills/src/management.rs:243`/`:263`
(`SYSTEM_SKILLS_ROOT="/system/skills"`, `:34`) as `SkillSource::System` via
`parse_skill_md` — without becoming `reborn_skills` DB rows, so Q1+Q2 never
run — are migrated into `reborn_skills` rows seeded through the Phase P.0
path. The disk-loaded `SkillSource::System` path is removed (or relegated to a
one-time import). `crates/brassclaw_engine/src/executor/db_skill_loader.rs`
(`fetch_llm_skills_as_json` / `fetch_monty_skills_as_json` at `:67`/`:83`,
which call `DbSkillStore::fetch_for_consumer` — the actual
`validation_status = 'validated'` filter lives at
`crates/brassclaw_skills/src/db_store.rs:554`) then loads them as
first-class DB skills.

**Files (indicative):**
- Step A: `crates/brassclaw_reborn_composition/build.rs` (`embed_reborn_skills()`),
  `crates/brassclaw_reborn_composition/src/bundled_skills.rs` (delete file),
  `crates/brassclaw_reborn_composition/src/lib.rs` (`mod bundled_skills`),
  all call sites of `ensure_bundled_reborn_skills_installed()` /
  `bundled_reborn_skill_summaries()`.
- Step B: `crates/brassclaw_skills/src/management.rs`
  (`SYSTEM_SKILLS_ROOT`, the `:243`/`:263` load sites, `parse_skill_md`), the
  seeder, `crates/brassclaw_engine/src/executor/db_skill_loader.rs`
  (`:67`/`:83` runtime load site calling `fetch_for_consumer`), and
  `crates/brassclaw_skills/src/db_store.rs` (`:554` validated filter inside
  `fetch_for_consumer`).

**Depends on:** Phase P.0 (Step B only; Step A has no dependencies).

**Tests:**
- Step A: confirm `cargo build -p brassclaw_reborn_composition` succeeds
  after deletion; grep-enforced absence of `bundled_skills`, `SYSTEM_SKILLS_ROOT`
  in `brassclaw_reborn_composition`.
- Step B / Integration: a system skill seeded → appears as a `reborn_skills`
  row → passes Q1+Q2 → `validated` → loaded by `brassclaw_engine`'s
  `db_skill_loader` (`fetch_llm_skills_as_json`).
- Step B / Regression: no `SkillSource::System` disk load remains for
  migrated skills (grep-enforced).

### Phase P — Doc-Conversion Mechanism (§0.22; user repeat item 4)

**Status:** [ ] Pending

**Goal:** Implement the §0.22 mechanism — auto-convert each
`docs/agents-v3/*.md` to an LLM-optimized form, store both versions in
`reborn_docus`, keep them fresh on change events, and inject them into the
base-prompt prefix + per-turn retrieval — **as v3 agent artifacts** (Recipe +
Skills + Tools + PythonCode + Action), not Rust code.

**Prerequisites:** V040 (live — Docu table); V051 (Phase A.5 — validation
queue); V052/V053 (Phase B/C — `reborn_python_code` / `reborn_extension_catalogues`
tables); V056 (Phase K.1 — `reborn_basic_prompt_store`); V057
(`capability_id` for the `component_db` Tool); Phase K.1
(`PgBasicPromptStore::mark_stale`); **Phase P.0** (no-bypass Q2); **Phase P.1**
(system-skills DB migration).

**Migration:** **none.** Phase P adds no migration of its own. It reuses
`V040` (Docu table — **live today**) plus the migrations created by its
not-yet-implemented prerequisite phases — `V051` (Phase A.5), `V052`/`V053`
(Phase B/C), `V056` (Phase K.1), `V057` (Phase K.1) — none of which are live
yet (see §2). The only host code is the step-1 composition const edit and the
step-3 `component_db` Rust Tool (+ its ToolSkill DB row, seeded through Phase
P.0); everything else is v3 artifacts authored as DB rows through Q1+Q2.

**Steps (the §8 sequence; one at a time, commit + push after each):**

1. **Composition prerequisite:** add `("reborn_docus", 17)` to
   `COMPONENT_TABLES` and `17 => "Docu"` to `class_label` in
   `interceptor_config_service.rs`; unit test that `do_reassemble` includes a
   validated Docu row. (Small Rust edit; no migration.)
2. **PythonCode leaves (class 22):** author `sha256`, `hash_changed`,
   `markdown_section`, `format_component_header` — pure logic, one concern
   each, no I/O; Q1-scanned; through Q1+Q2 (Phase P.0). General-purpose →
   bootstrap candidates.
3. **The one generic DB Tool + ToolSkill (class 0 + 13):** author the **Rust**
   capability `component_db` (`op ∈ {read_hash, read_row, upsert, mark_stale}`,
   §0.22.4) + its single executor-facing ToolSkill. `upsert` does
   `INSERT … ON CONFLICT … DO UPDATE` into `reborn_docus` and **always sets
   `validation_status='pending'`**; `mark_stale` wraps
   `PgBasicPromptStore::mark_stale`. `read_file`/`glob`/`memory_*` reused
   as-is. (Host Rust — the only part besides step 1 that touches Postgres /
   the kernel boundary.) One generic Tool, not three.
4. **Leaf Orchestrator Skills (classes 1-3):** author the one-tool-each leaves
   — `file-list`, `file-read`, `hash-compute`, `hash-compare`, `db-read-hash`,
   `markdown-section`, `component-header-render`, `prompt-compress`, plus the
   doc-specific `db-upsert-docus` and `db-mark-prefix-stale`. The DB leaves
   bind to the one `component_db` Tool (different `op`); the rest bind to
   their own tool/pythoncode. Through Q1+Q2 (Phase P.0) — no bypass.
5. **Domain Orchestrator Skill (classes 1-3):** author `doc-convert-method`
   (§0.22.4) — the doc-specific overview referencing the leaves by name.
   Through Q1+Q2 — no bypass.
6. **Recipe (class 21):** author `doc-convert` (variants `by-extract` Tier 0,
   `by-llm-compress` Tier 1) with `step_descriptions` JSONB; steps `include`
   the step-4 leaf UUIDs + the step-5 domain skill + the step-3 `component_db`
   ToolSkill UUID. Through Q1+Q2 — no bypass.
7. **Action (class 16):** author `doc-sync` (`execute_action_procedure`, no
   LLM) — the scan/decide/extract/upsert/mark-stale driver composing the
   leaves; enqueues `by-llm-compress` for docs whose §7 extract needs
   compression (no budget gate — Answer 5). Through Q1+Q2 — no bypass.
8. **ExtensionCatalogue (class 23):** register `doc-sync` owning only the
   doc-specific parts (domain skill, `db-upsert-docus`/`db-mark-prefix-stale`
   leaves over the one `component_db` Tool, Recipe, Action); general-purpose
   leaves live in the matching builtin catalogue and are referenced. With
   `overview_doc`.
9. **Event wiring (Answer 4):** wire `doc-sync` to fire on (a) a file-watch on
   `docs/agents-v3/*.md` (source doc changed on disk), and (b) a
   `reborn_docus` row-change signal (doc edited via the WebUI Docs section, or
   a re-compression graduating). No idle-time loop, no cadence, no boot
   trigger (§0.22.5).
10. **WebUI Docs section (Answer 2):** list `reborn_docus` rows (source +
    converted, with validation status) + manual editing; **saving an edited
    doc sends it to the validation queue again** (`pending`, enqueued) — never
    writes `validated` directly. Mirror the existing validation-queue tab
    pattern (`validation-queue-tab.js`); add i18n keys for all packs.
11. **End-to-end test:** change a `docs/agents-v3/*.md` → `doc-sync` fires →
    `reborn_docus` rows updated (new `content_hash`, both source + converted
    versions) → base prompt `is_stale` → Prefix Tab regenerate pulls the new
    converted doc into the assembled bundle.

**Tests:** per-step unit tests + the step-11 e2e. Security: a converted doc
whose §7 quotes an injection payload fails Q1 (the converter must sanitize);
the WebUI Docs save never writes `validated` directly (store-level guard).

---

## 2. Migration Sequence

| Migration | Contents | Status |
|-----------|----------|--------|
| `V050__reborn_recipe_step_descriptions.sql` | `ADD COLUMN step_descriptions JSONB`, `variants JSONB`, `dependency_registry JSONB` to `reborn_recipes` (all three Phase A store columns — see VARPAT-COL-GAP / DEPREG-TIMING-GAP note in Phase A) | **Next** |
| **`V051__reborn_validation_queue.sql`** | **NEW (Decision 2 / Phase A.5):** `CREATE TABLE reborn_validation_queue` + indexes only. No data migration. No column drops. This enables all component classes (including the new 22/23) to enter the queue from their very first WebUI-authored save. `ValidationQueueStore` application layer also lands in Phase A.5. **§0.23.5 addition:** the table also carries `proposed_payload JSONB` (nullable) — the upgrade-copy payload used when an edit to a validated component is pending (live row stays validated+served; Q2 approval applies the payload). | |
| `V052__reborn_python_code.sql` | New table `reborn_python_code`, class 22 (**was V051** before Decision 2) | |
| `V053__reborn_extension_catalogues.sql` | New table `reborn_extension_catalogues`, class 23 (**was V052** before Decision 2) | |
| `V054__reborn_intent_inputs_step_link.sql` | `ADD COLUMN step_link TEXT` to `reborn_intent_inputs` (**was V053** before Decision 2) | |
| ~~`V055__reborn_skills_intent_examples.sql`~~ → **`V055__reborn_dependency_registry.sql`** | `ADD COLUMN dependency_registry JSONB` to all 13 component tables (**was V054** before Decision 2; see Phase J.2 — §0.19). The `intent_examples` ALTER is a **no-op** (V027 already has the column) and has been removed per FIND-12. The file must be named `V055__reborn_dependency_registry.sql`. **§0.23.4 addition:** this same all-tables migration also adds `formatted_content TEXT` (nullable) to all 13 component tables — the persisted LLM-formatted version computed at save time by the per-class formatter PythonCode (Phase J.2 builds the light in-process PythonCode executor; Phase L seeds the formatter components). | |
| `V056__reborn_basic_prompt_store.sql` | **Phase K single migration (folded — was V055 before Decision 2).** Carries **all** Phase K additive DDL: (a) new table `reborn_basic_prompt_store` — one row per scope, `bundle_json JSONB`, `is_stale BOOL`, `fingerprint TEXT`; (b) **§0.23.7 fold-in:** component-UUID reference column(s) on the interceptor packet/segment store so prompts reassemble **by reference** (enables the idle self-improvement sweep, §0.23.8) — **confirm exact column shape against the live `PgInterceptorStore` schema at Phase K**; (c) **§0.23.8 fold-in:** `reborn_monty_vm_settings` validation-improve cols (`validation_idle_threshold_minutes INT NOT NULL DEFAULT 120`, `validation_improve_start_hour INT NOT NULL DEFAULT 15`, `validation_improve_enabled BOOLEAN NOT NULL DEFAULT true`). **Not split into `V062`/`V063`** — refinery applies migrations in strict ascending order and the embedded PG data dir is persistent across boots, so a `V062`/`V063` landing in Phase K (sort_order 12) before `V057`–`V061` (Phases L–P.0) would silently skip those later lower-numbered migrations. Folding into `V056` keeps numbers ascending with execution order. See §0.23.10 ordering note. | |
| `V057__reborn_tools_capability_id_and_system_source.sql` | `ADD COLUMN capability_id TEXT` to `reborn_tools` + `source = 'system'` allowed on tools/tool_skills/skills (**was V056** before Decision 2) | |
| `V058__reborn_intent_inputs_template.sql` | `ADD COLUMN is_template BOOL`, `template_prefix TEXT`, `template_suffix TEXT` to `reborn_intent_inputs`; two new partial indexes for prefix/suffix-anchored template matching (**was V057** before Decision 2; see §0.17.2) | |
| `V059__reborn_validation_queue_populate.sql` | **Phase N only:** populate `reborn_validation_queue` from existing component table state; add `last_graduation_at` to scope cursor; graduation trigger; drop `queue_code`/`review_attempts`/`review_feedback`/`rejected_at`/`validation_errors` from all 13 component tables. `CREATE TABLE` is in V051. (**was V058** before Decision 2) | |
| `V060__reborn_monty_vm_settings_token_budgets_enabled.sql` | **Phase O (§0.21 — user item, Answer 5):** `ALTER TABLE reborn_monty_vm_settings ADD COLUMN token_budgets_enabled BOOLEAN NOT NULL DEFAULT true;` — the global token-budget kill switch. Additive only; existing rows backfill to `true` (today's behaviour). Independent of Phases A–N; shippable in any order after V034 exists (it already does, live). | |
| `V061__reborn_validation_queue_q2_actor.sql` | **Phase P.0 (§0.23.10):** `ALTER TABLE reborn_validation_queue ADD COLUMN q2_actor TEXT;` — records the automated-but-auditable Q2 actor for `source='system'` graduation (builtins + validation-system trusted root). Additive, nullable. Already tentatively noted under Phase P.0. | |

All additive-first. No DROP, no renames. No existing rows break. V059 is the only migration with DROP statements — all others (including V060–V061) are additive. (The §0.23.7/§0.23.8 Phase K additive DDL — interceptor packet component-UUID refs + `reborn_monty_vm_settings` validation-improve cols — is **folded into `V056`**, not separate `V062`/`V063` files; see the `V056` row above and §0.23.10 ordering note.)

> **Phase P (§0.22 — doc-conversion) adds NO migration.** It reuses the
> already-live `V040__reborn_docus` (Docu table — has `content_hash` + lineage
> + SCH-02 + `validation_status`), plus `V051` (validation queue), `V052`/`V053`
> (`reborn_python_code` / `reborn_extension_catalogues`), `V056`
> (`reborn_basic_prompt_store`), and `V057` (`capability_id` for the
> `component_db` Tool) — **of these, only `V040` is live today; `V051`–`V057`
> are created by their own not-yet-implemented prerequisite phases
> (A.5 / B / C / K.1).** The only host-Rust edits are the step-1
> `COMPONENT_TABLES`/`class_label` const (no migration) and the step-3
> `component_db` Tool. **Phase P.0** (validation-system extension) may add one
> small additive column to `reborn_validation_queue` (V051) to record the Q2
> actor type — to be confirmed by the P.0 subplan against V051's actual columns;
> if needed it would be `V061__reborn_validation_queue_q2_actor.sql` (additive
> only). **Phase P.1** (on-disk system-skills migration) adds no migration.

> **✅ Review note (pre-v3 audit) — §2 ordering hazard (validation queue vs. new classes
> 22/23) — RESOLVED (Decision 2: queue table split into V051 + V059):** The original
> "third path chosen" note below described a design where the queue table and gate logic
> both landed at V058 — classes 22/23 had "no queue row until Phase N" as a documented
> limitation. **That design has been superseded.** Decision 2 splits V058 into two migrations:
> V051 creates `reborn_validation_queue` (table + indexes only, no data migration), so the
> queue exists from Phase A.5 — immediately before classes 22 and 23 are created at V052/V053.
> V059 (Phase N) contains only the populate-from-existing-state + graduation trigger +
> DROP-legacy-columns work. The `ValidationQueueStore` application layer also moves to
> Phase A.5. This makes the "from day one" design literally true: from V052/V053 onward,
> every WebUI-authored PythonCode (class 22) / ExtensionCatalogue (class 23) save can
> immediately enqueue to `reborn_validation_queue`. Phase B/C WebUI save paths MUST call
> `ValidationQueueStore::submit(...)` on component creation. The Phase N populate (V059)
> still back-fills existing tables, but the classes 22/23 gap is now a non-issue. Old
> "third path chosen" text retained below for reference only (superseded):
>
> ~~**[SUPERSEDED — third path chosen]:** neither recommended option (a) nor (b) was
> adopted; a third, cleaner path was taken. V051/V052 carry **only** `validation_status`
> (no per-table queue columns, no queue table hoisted), and the plan now states plainly
> that the snippet→Q1→Q2 promotion is a **Phase N capability** — the queue + gate logic
> both land together at V058. Pre-Phase-N, a WebUI-authored PythonCode (class 22) /
> ExtensionCatalogue (class 23) row sits at `validation_status='pending'` with **no queue
> row**, and `fetch_component_by_id` only returns `validation_status='validated'` rows, so
> such rows are **not yet usable** in `type:"component"` steps — this is a documented
> pre-Phase-N limitation, not a contradiction. System-seeded rows (Phase L, inserted with
> `validation_status='validated'`, Q1 run at build time) and operator-validated rows remain
> usable throughout. V058 then **back-fills the gap**: its populate UNION ALLs over all 15
> tables (13 existing + the two Phase B/C tables), its boot-integrity check auto-submits
> every `pending`-with-no-queue row (including the backlogged class 22/23 rows) to the
> queue, and the Q1/Q2 gate logic makes the snippet→component promotion reachable the
> moment V059 lands. **[SUPERSEDED — implemented as Option (a): V051 creates the table
> early, V059 does populate+drops. The "not landable" issue is resolved. Classes 22/23 have
> a queue from V052/V053 onward. See Decision 2 note above.]**~~

**`step_link` is nullable** — existing intent rows without it use the existing
`fetch_component_by_id` path unchanged. Zero breakage on upgrade.

**`capability_id` is nullable** — existing Tool rows (user-authored) are unaffected.
Only system-seeded builtin rows carry a `capability_id`.

**No `reborn_pending_rust_context` table.** The earlier transient-table design is
superseded: `SplitResult.rust_items` is delivered directly by `RecipeStage` at runtime,
avoiding the DB round-trip entirely.

---

## 3. Open Questions

| # | Question | Recommendation |
|---|----------|----------------|
| 1 | Variable extraction: named capture groups vs. post-match LLM extraction? | **Resolved — see §0.17.** Intent expressions use `%` slot markers for matching (Phase M). Slot values are auto-extracted from template segments (positional names `slot0`, `slot1`, …). `variable_patterns` is optional post-extraction refinement for semantic naming and validation. LLM extraction remains a future opt-in via `llm_var_extraction_prompt` on `RecipeVariant` (out of scope). |
| 2 | BuildInstruction memoisation: per-process or always recompute? | **Resolved — see §0.7 (DESIGN-02 / §0.18 + Phase N).** Per-process SplitResult cache keyed on `sha256(step_link + "\|" + sha256(step_descriptions_json) + "\|" + sha256(variable_patterns_sorted_json))` per scope. **⚠️ FIND-P10-02: the old key formula `sha256(step_link + sorted_include_uuids.join(","))` was circular (required IBS compilation to be already done) — corrected in DESIGN-02 / §0.7 to use `step_descriptions_hash` which is computable BEFORE IBS. Do NOT implement the old formula.** Eviction is event-driven via `last_graduation_at` on the scope cursor (bumped by DB trigger when a component graduates from `reborn_validation_queue`). One sub-millisecond PK read per cache hit. No TTL required as primary mechanism. See §0.7 for the full memoisation key specification. |
| 3 | `required_skills` inclusion: always include vs. score against current query? | **Resolved — see §0.19.** `required_skills` does not exist. Dependencies are declared per-component in `dependency_registry` JSONB and referenced from StepDescription steps via typed traversal expressions (`1[all], 5[2,6], 17[3, 7[1,4]]`). Always resolved fully per the traversal expression — no scoring, no cap. KV-cache prefix absorbs token cost in steady state. |
| 4 | `step_formatter_id` scope: per-recipe, per-variant, or per-step? | **Resolved — not needed.** `step_formatter_id` does not exist. Formatting is achieved by authoring PythonCode component bodies with the correct content and prose style. `type: "text"` steps are WebUI annotations only with no runtime emission. All three intent-match cases (Recipe match, near-miss, full fallback) have their formatting handled by PythonCode bodies, prepared prompt templates, and the KV-cache prefix respectively. |
| 5 | StepDescription storage format: YAML files in git vs. JSONB in `reborn_recipes`? | **Resolved — JSONB (§0.5).** YAML files in git are structurally incompatible (no WebUI write path, no scope isolation, requires deploy cycle). JSONB column on `reborn_recipes` is the correct choice. Each JSONB element holds a dual representation: `yaml_source` (raw YAML, WebUI display) + `steps` (pre-parsed array, IBS reads). YAML is parsed once at WebUI save time — the IBS never parses YAML at runtime. |
| 6 | Legacy DocPlan → v3 translation? | **Resolved — see §3.1.** JSONB is the storage format (Q5). The translation pipeline creates new v3 components from legacy MemoryDoc rows; dependency registries are decided at authoring time, not inferred by translation. Action-format steps with name references must be resolved to UUIDs; unresolvable names are Q1 hard errors. Step type (text/component/snippet) and component class are orthogonal. |
| 7 | `__assemble_prior_knowledge__` removal timing? | **Not removed.** `__assemble_prior_knowledge__` IS the v3-upgraded primary function — it already calls `fetch_for_turn` and returns `{content, formatted_content, override_prompt_creation, matched_component_ids}`. Phase F extends it to handle `SplitResult` and `ActionShortCircuit`. Phase G removes the dead `__retrieve_docs__(goal, 5)` shim from step-0 (NOT the `__assemble_prior_knowledge__` call). Phase K removes `__retrieve_docs__` handler registration (the legacy MemoryDoc path). |
| 8 | Should builtin Tool/ToolSkill/Skill rows bypass Q2 (auto-validated)? | **SUPERSEDED — Answer 2 to the doc-conversion review: "Nothing ever bypasses the Q1+Q2 system."** The old answer (direct `validated` insert at seeder time, Q2 bypassed) is overruled. Builtins now go through Q1 + an **automated-but-auditable Q2** graduation recorded in the queue (never a silent skip; the seeder/automation is the Q2 *actor*). This is implemented by **Phase P.0**, which revises §0.16 / Phase L. ~~Yes — `source = "system"`, `validation_status = "validated"` at seeder insert. Q1 runs inside the seeder at build time. Q1 errors in seeder content are CI build failures.~~ (Retained for reference only — superseded.) |
| 9 | Should the MCP translator also be used for builtins? | No — wrong granularity (1:1 per tool, no task-level Skills, no PythonCode, no multi-ToolSkill Recipes). Use `builtin_bootstrap.rs` (Phase L) for builtins. MCP translator is for external third-party MCPs only. |
| 10 | What recipe variants should `builtin.shell` have? | Two: (a) known-safe commands (allowlist: `cargo build/test/fmt/clippy`, `git status/log/diff`, `npm install/build`) at Tier 1 high-confidence; (b) open-ended arbitrary command at Tier 1 always with explicit approval annotation. Both have `llm_call_required: true` — no shell is ever Tier 0. |
| 11 | How does the Rust execution layer resolve a Tool DB UUID to its registered capability handler? | Via `capability_id` column (V057 — was V056 before Decision 2). On tool dispatch: look up Tool row by UUID → read `capability_id` → look up handler in `FirstPartyCapabilityRegistry` by `capability_id`. For user-authored tools without `capability_id`, fall back to existing name-based resolution. |
| 12 | Should builtin Recipes also have `source = "system"` and bypass Q2? | **SUPERSEDED — same as Q8 (Answer 2): nothing bypasses Q1+Q2.** Builtin Recipes now go through Q1 + automated-but-auditable Q2 via Phase P.0. ~~Yes — same reasoning as Q8. Builtin Recipe StepDescriptions are hand-authored and IBS pre-flight-checked at seeder run time. Q2 bypass for `source = "system"` Recipes is consistent with Tools and ToolSkills.~~ (Retained for reference only — superseded.) |
| 13 | If `RecipeStage` already stashed the items (Tier 1), how does `assemble_prior_knowledge_with_hint` know not to call `fetch_for_turn` again? | **Resolved — stash/unstash protocol (Phase H §5, FIND-P9-15 + FIND-NEW-PASS12-01 corrected).** The composition host calls the new `pub` function `assemble_prior_knowledge_with_hint(thread, goal, ..., recipe_hint: Option<serde_json::Value>)` (NOT `handle_assemble_prior_knowledge` directly — that is private and takes `args: &[MontyObject]`). The correct flow: (1) `RecipeStage` (agent_loop) reads `state.recipe_hint` and passes it as a parameter to `ctx.host.run_step_zero(context, recipe_hint.as_ref())`; (2) the composition host calls `assemble_prior_knowledge_with_hint(..., recipe_hint.cloned())`; (3) `assemble_prior_knowledge_with_hint`: if `recipe_hint` is `Some(v)` → use the stashed orchestrator_items, skip `fetch_for_turn` entirely, deserialize, format, return `PkrAssemblyResult`; if `None`, call `fetch_for_turn` as before; (4) the STAGE clears `state.recipe_hint = None` AFTER `run_step_zero` returns. No double-fetch, no second `resolve_intent`, no second IBS compilation. |
| 14 | In Tier 0, `PromptStage` and `ModelStage` are skipped — but the Python script calls `__assemble_prior_knowledge__`. Where does Python execute in Tier 0? | The Python scripting engine is **not** the LLM call. `PromptStage` assembles the LLM input prompt; `ModelStage` sends it to the model. Both are skipped in Tier 0. `default.py` is invoked by a dedicated **`TierZeroExecutionStage`** (NOT `CapabilityStage` — see the resolution below). In Tier 0, Python runs step-0, calls `__assemble_prior_knowledge__` (gets the stash), and invokes skills/tools directly — no LLM round-trip in the middle. "Tier 0: no LLM" means no LLM call, not no Python execution. **✅ DESIGN GAP RESOLVED (Option 1 chosen — `TierZeroExecutionStage` + `LoopOrchestratorPort`):** In the normal pipeline `CapabilityStage` processes tool-call responses from the model, so it CANNOT kick Python in Tier 0 (there is no model output). The plan now specifies the mechanism: a new `TierZeroExecutionStage` inserted between `RecipeStage` and `AssistantReplyStage` in `canonical.rs` calls `ctx.host.run_tier_zero(context, &state.recipe_hint, &state.recipe_rust_context)` — a new `LoopOrchestratorPort` host port (15th `AgentLoopDriverHost` port, implemented by `brassclaw_reborn_composition`, the only crate depending on both `brassclaw_engine` and `brassclaw_agent_loop`). `CapabilityStage` is NOT bent and is simply skipped. Option 2 (synthetic signal into `LoopCapabilityPort`) is rejected — it would couple the capability port to Tier 0 routing. See Phase H.0 §H5 for the full port spec and §5 for the corrected Tier 0 turn-flow diagram. **⚠️ DRIVER-PREREQ:** this mechanism is exercised only in agent-loop tests until the agent-loop `DefaultExecutorPipeline` is wired as the production driver (today the engine `ExecutionLoop::run` drives turns with no stage pipeline); see DRIVER-GAP in the index. |
| 15 | What happens if `build_instruction` returns an `IbsError` during the builtin seeder (Phase L)? `panic!` or return an error? | **Debug builds: `panic!`** — seeder content is hand-authored; an IbsError here is a compile-time bug. **Release builds:** `error!`-log, skip the Recipe row, continue. The seeder is idempotent — skipped rows do not block boot. CI must run the seeder in debug mode so IbsErrors become build failures before reaching production. |

### 3.1 Legacy DocPlan → v3 Component Translation

The v2 system stored all knowledge as `MemoryDoc` rows. `component_import.rs` already
migrates these to class-specific tables. The gap: no v2 docs have StepDescriptions or
`step_link` formulae.

**One-time, operator-triggered pipeline (`brassclaw migrate-to-v3`):**

1. **Skills (class 1–3):** Extract imperative sentence candidates from skill `body` as
   seed intent examples. Generate StepDescription0:
   - Step 1: `knowledge: orchestrator`, `type: text`, `info`: summary of what this skill does (WebUI annotation)
   - Step 2: `knowledge: orchestrator`, `type: component`, `include`: [skill UUID]
   Default `step_link: "0:0-0:E"`. Route to Q1.

2. **ToolSkills (class 13):** Generate StepDescription0 with one rust-channel `component`
   step. No intent examples (ToolSkills are referenced by Skills, not intent-matched directly).

3. **Existing Recipes (class 21) without `step_descriptions`:** Map each entry in the
   existing `steps` JSONB (13-type Action format) to v3 StepDescription steps:
   - Steps with a component reference by **name**: resolve the name to a UUID by querying
     the component tables (any class) within the same scope. On success → `type: component`
     with the resolved UUID in `include`. On failure (name not found) → Q1 **hard error**
     on the translated Recipe ("unresolvable component reference: <name>"); the Recipe is
     flagged and requires manual correction before it can activate.
   - Steps with no component reference → `type: text` (WebUI annotation only; no runtime effect).
   Route to Q1. `yaml_source` is synthesised from the generated steps array.

4. **Specs, Lessons, Notes:** Leave as-is. Served by the UNION ALL path; no StepDescriptions needed.

Idempotent: components that already have `step_descriptions` are skipped.

### 3.2 Builtin Tool Bootstrap Pipeline

This is a **separate, automatic pipeline** that runs at every boot (not operator-triggered).
It seeds the full v3 component stack for the 23 first-party builtin tools if not already present.
See §0.16 for the full specification and Phase L for the implementation plan.

**Relationship to §3.1:**  
The `brassclaw migrate-to-v3` pipeline (§3.1) handles user-authored v2 documents.  
The builtin bootstrap (§3.2 / Phase L) handles system tools that never had v2 representations.  
They are independent — running one does not affect the other.

**Idempotency guard:** The seeder checks `SELECT COUNT(*) FROM reborn_tools WHERE source = 'system'`
for the current scope at boot. If ≥ 1 row exists, the seeder skips entirely. A full re-seed
can be triggered by deleting system-sourced rows (operator action only).

---

## 4. Out of Scope (Marked Postponed)

> **⚠️ Three items below are now IN SCOPE per §0.23** (v3 direction, folded into
> existing phases A.5 / J.2 / K / L / N / P.0 — see §0.23.11). They are retained
> here with strikethrough + an in-scope pointer for traceability:

- ~~Full self-improvement pipeline (Interceptor-driven Recipe auto-creation)~~ → **IN SCOPE (§0.23.6 + §0.23.8)** — generalised to all component types, not just recipes.
- ~~Component self-creation wizard~~ → **IN SCOPE (§0.23.6)** — the Sempai auto-creation path is the wizard; WebUI manual authoring remains as operator-in-the-loop review/edit.
- ~~Automatic Sempai-driven prompt rewrites~~ → **IN SCOPE (§0.23.8)** — the idle self-improvement sweep (≥2h idle + after 15:00 local) reassembles prompts + chat history and asks the Sempai for component creation/upgrades → Q1.
- LLM-based variable extraction fallback (Phase A uses regex only)
- Tier 0 production activation (requires Phases A–H complete + Wilson scoring validated in production for ≥2 weeks)
- `FormatOrchestratorPrompt` as a distinct step type (not needed — formatting handled by PythonCode component bodies)

---

## 5. Turn Flow Summary

### Tier 0 (intent match, no LLM)

> **Two execution models (DRIVER-GAP / Phase H.0 §H5 MODEL SELECTION):** the diagram below
> shows the **agent-loop path (Model B/C, target state)** — `RecipeStage` + `TierZeroExecutionStage`
> + `LoopOrchestratorPort`. **Current production uses Model A (engine `ExecutionLoop`)**,
> which has no `RecipeStage`/`InputStage`: Python step-0 calls `__assemble_prior_knowledge__`
> → `fetch_for_turn` → `SplitResult`; when `llm_call_required == false` the handler returns a
> DEDICATED `tier_zero: true` signal (NOT `override_prompt_creation` — that is the LLM
> Solution-Override path) and Python step-0's NEW `tier_zero` early-return branch runs the
> orchestrator channel deterministically without `__llm_complete__` (the
> `execute_action_procedure` pattern, `default.py:901`, generalised to Tier-0 Recipes). The
> two paths are functionally equivalent for Tier 0; the agent-loop path is exercised only in
> tests until the switchover (DRIVER-PREREQ). ⚠️ Tier 0 on the engine path does NOT work
> today — it lands when Phase H adds the `tier_zero` signal + early-return branch (item 3b).
> See item 3b for the engine-path wiring.

```
User types: "show all files including hidden in /tmp"
│
├─ [InputStage]
│   state.last_user_text = "show all files including hidden in /tmp"
│
├─ [RecipeStage]
│   fetch_for_turn(scope, last_user_text, budget, "02")
│     → resolve_intent → Match { component_id: uuid-local-files-reading,
│                                class_code: 21,
│                                step_link: "0:0-0:30+1:0-1:E" }
│     → fetch step_descriptions[0] (steps 0..30) + step_descriptions[1] (all)
│     → IBS: build_instruction("0:0-0:30+1:0-1:E", step_descriptions, variable_patterns)
│         variable capture: dir="/tmp", flags="-la"
│         rust_steps:         [component(uuid-ls-toolskill, knowledge:rust)]
│         orchestrator_steps: [text("info…"), component(uuid-ls-skill),
│                               component(uuid-ls-result-handler)]
│     → fetch rust_items:         [ComponentItem: ls-toolskill body]
│     → fetch orchestrator_items: [ComponentItem: ls-skill body,
│                                   ComponentItem: ls-result-handler body]
│     → FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }
│   routing.tier0_eligible = true (wilson_lower=0.82, tier=mature, validated)
│   routing.llm_call_required = false → Tier 0
│   apply rust_items to Rust execution context (silent, never forwarded to orchestrator)
│   serialize orchestrator_items → state.recipe_hint (JSONB stash, one-shot)
│   serialize rust_items        → state.recipe_rust_context (JSONB stash, one-shot)
│   return PostRecipeOutcome::TierZero
│
├─ [PromptStage SKIPPED — no LLM prompt assembly needed]
├─ [ModelStage  SKIPPED — no LLM call in Tier 0]
│
│   NOTE: The Python scripting engine (default.py) DOES run in Tier 0.
│   "No LLM" means PromptStage (prompt assembly) + ModelStage (LLM call) are skipped.
│   The Python engine is separate from the LLM call and is NOT skipped.
│
├─ [TierZeroExecutionStage — kicks Python via LoopOrchestratorPort (TIER0-GAP fix)]
│   CapabilityStage is NOT used in Tier 0 — it reacts to model output, and there is
│   none. A dedicated TierZeroExecutionStage calls the host port instead:
│     reply = ctx.host.run_tier_zero(context, &state.recipe_hint,
│                                    &state.recipe_rust_context)
│   The composition host delegates to the engine orchestrator's no-LLM entry point
│   (the same `__assemble_prior_knowledge__` + skill/PythonCode channel, but invoked
│   through the LoopOrchestratorPort bridge since brassclaw_agent_loop cannot import
│   brassclaw_engine). The stage extracts recipe_hint from state, passes as parameter:
│     # FIND-P9-15: stage passes recipe_hint as parameter, NOT read from state by handler
│     pkr = __assemble_prior_knowledge__(goal, budget, "02",
│               recipe_hint=<extracted from state.recipe_hint by stage>)
│     handler receives recipe_hint param → SET → use stashed value, skip fetch_for_turn
│     # Stage clears state.recipe_hint AFTER run_tier_zero returns (not the handler)
│     pkr["orchestrator_content"]:
│       ## [Skill: ls]
│         <ls-skill body>
│       ## [PythonCode: ls-result-handler]
│         <ls-result-handler body>
│     pkr["matched_component_ids"]: [uuid-ls-skill, uuid-ls-result-handler]
│     pkr["tier_zero"]: true            # NEW no-LLM signal (NOT override_prompt_creation)
│   → No ToolSkill bodies. No memories. No UNION ALL noise.
│   → _set_active_skills_from_matched_ids([uuid-ls-skill, uuid-ls-result-handler], state)
│   → Orchestrator invokes ls-skill → instructs Rust executioner: ls /tmp -la
│   → Rust reads ls-toolskill (pre-loaded in execution context), calls ls -la /tmp
│   → Rust returns stdout to orchestrator
│   → Orchestrator runs ls-result-handler PythonCode → formats output for chat
│   state.recipe_hint = None; state.recipe_rust_context = vec![];   // one-shot consume
│   If run_tier_zero returns None (NoOrchestrator host) → degrade to Tier 2 (NeedsPrompt).
│
│   NOTE: InterceptorStage position in Tier 0:
│   Per COMP-07 (Phase H §4), InterceptorStage MUST be skipped in Tier 0 —
│   it runs between PromptStage and ModelStage in canonical.rs and must not
│   open a ForensicPacket for a turn that has no model call to close it.
│   The InterceptorStage shown below is therefore NOT the same as the Tier 1/2
│   InterceptorStage (which precedes the LLM call). The composition plan recording
│   (Sempai telemetry) still happens, but through a separate lightweight mechanism
│   that does NOT open a ForensicPacket.
│
├─ [InterceptorStage SKIPPED — no ForensicPacket opened in Tier 0]
│   [Composition plan recorded via lightweight path if Sempai is connected]
└─ [AssistantReplyStage]  Emits formatted directory listing. Wilson score updated.
```

### Tier 1 (intent match, LLM-guided)

> **⚠️ FIND-P5-05 — prior diagram had wrong execution order (FIND-16). Redrawn.**
> Python step-0 (`__assemble_prior_knowledge__`) runs PRE-LLM inside PromptStage
> (via `run_step_zero`). `CapabilityStage` handles POST-LLM tool execution (steps 1+).
> The old wrong diagram is removed.

> **FIND-15 — Tier 1 dual-path intent (DELIBERATE):** the recipe hint reaches the LLM
> via TWO paths:
> (1) PromptStage calls `build_prompt_bundle` → `request.recipe_hint` is set →
>     host prepends orchestrator_items to the LLM prompt bundle (LLM sees it).
> (2) Python step-0 (via `run_step_zero` in PromptStage) calls `__assemble_prior_knowledge__` →
>     handler reads the stash → returns `pkr["orchestrator_content"]` (Python uses it to
>     direct the Rust executioner after the LLM responds).
> Both injections are INTENTIONAL and complementary: LLM path = pre-turn prompt guidance;
> Python path = post-LLM execution direction.

```
User types: "edit main.rs and refactor the error handler"
(wilson_lower = 0.61 — confident match, but llm_call_required = true)
│
├─ [InputStage]
│   state.last_user_text = "edit main.rs and refactor the error handler"
│
├─ [RecipeStage]
│   fetch_for_turn(scope, last_user_text, budget, "02")
│     → resolve_intent → Match { component_id: uuid-builtin-edit-file,
│                                class_code: 21,
│                                step_link: "0:0-0:E+2:0-2:E" }
│     → IBS: build_instruction("0:0-0:E+2:0-2:E", step_descriptions, variable_patterns)
│         rust_steps:         [component(uuid-edit-toolskill, knowledge:rust)]
│         orchestrator_steps: [component(uuid-edit-skill), component(uuid-patch-formatter)]
│         llm_call_required:  true
│     → fetch rust_items, orchestrator_items
│     → FetchForTurnResult::SplitResult { rust_items, orchestrator_items, routing }
│   routing.tier0_eligible = false (wilson_lower=0.61 < 0.70 or llm_call_required=true)
│   serialize orchestrator_items → state.recipe_hint        (JSONB stash)
│   serialize rust_items        → state.recipe_rust_context (JSONB stash)
│   return PostRecipeOutcome::NeedsPrompt (does NOT skip PromptStage/ModelStage)
│
├─ [PromptStage]
│   rust_items applied to Rust execution context from state.recipe_rust_context (pre-LLM)
│   PromptStage calls ctx.host.build_prompt_bundle(context_request).
│   FIND-11: PromptStage copies state.recipe_hint → request.recipe_hint.
│   Host reads request.recipe_hint, prepends orchestrator items to bundle (LLM sees it).
│   PromptStage does NOT clear state.recipe_hint (COMP-03 — the STAGE clears it AFTER
│   run_step_zero returns; Python step-0's handler has no &mut state access — see FIND-P9-15).
│
│   ctx.host.run_step_zero(context, state.recipe_hint.as_ref()) called here (PRE-LLM):
│     # FIND-P9-15: stage passes recipe_hint as parameter; handler never reads LoopExecutionState
│     → Python step-0 starts
│     → pkr = __assemble_prior_knowledge__(goal, budget, "02",
│                 recipe_hint=<parameter from stage>)
│     → handler receives recipe_hint param → SET → use stashed value (skips fetch_for_turn)
│     → Stage clears state.recipe_hint AFTER run_step_zero returns (not the handler)
│     → pkr["orchestrator_content"]:
│         ## [Skill: file-editing]
│           <skill body>
│         ## [PythonCode: patch-formatter]
│           <pythoncode body>
│     → pkr["matched_component_ids"]: [uuid-edit-skill, uuid-patch-formatter]
│     → pkr["tier_zero"]: false, pkr["override_prompt_creation"]: false (Tier 1 uses LLM)
│     → Python step-0 inserts orchestrator_content into working_messages
│     → run_step_zero returns PriorKnowledgeBundle to host
│   → prompt bundle assembled with orchestrator context
│
├─ [InterceptorStage]  Sempai reviews assembled prompt (recipe hint visible to Sempai)
│
├─ [ModelStage]        LLM call — guided by injected orchestrator_content
│
├─ [CapabilityStage / Python execution — POST-LLM, steps 1+]
│   Python receives LLM response. Tool calls in the response are executed by
│   the capability stage. Rust execution context already has rust_items applied
│   (loaded from state.recipe_rust_context before Python started).
│   → LLM-directed tool calls use pre-loaded skill bodies and ToolSkill bindings
│
└─ [AssistantReplyStage]  emit LLM response; Wilson score updated
```

### Tier 2 (no match — full LLM)

```
User types: "explain recursion to me"
│
├─ [InputStage]        last_user_text set
├─ [RecipeStage]       fetch_for_turn → NoMatch → fetch_for_consumer (UNION ALL)
│                       → FetchForTurnResult::Components([...])
│                       → RecipeStageOutcome::Continue (Tier 2, unchanged)
├─ [PromptStage]       normal assembly; all UNION ALL items in orchestrator_content
│                       volatile context injected separately (never mixed with prior knowledge)
├─ [InterceptorStage]  Sempai reviews if connected
├─ [ModelStage]        full LLM call
└─ [AssistantReplyStage]  emit LLM response (no Recipe outcome recorded)
```

### Action short-circuit (class 16)

```
User types: "run the daily-sync action"
│
├─ [InputStage]        last_user_text set
├─ [RecipeStage]       fetch_for_turn → Match { class_code: 16 }
│                       → FetchForTurnResult::ActionShortCircuit { component_id, name }
├─ [PromptStage skipped]
├─ [ModelStage  skipped]
│   Orchestrator receives: pkr["action_short_circuit"]: true
│   → action_doc = __fetch_component__(action_component_id, 16)
│   → execute_action_procedure(action_doc, goal, state)  [FIND-P7-02: existing fn, no new fn]
└─ [AssistantReplyStage]  Emits action result.
```
