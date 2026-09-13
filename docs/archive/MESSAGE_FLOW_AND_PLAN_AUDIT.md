# Message Flow Audit + saved_plan_to_v3.md Adjacency Review

> **Date:** Created after Goals_pre_v3_review completion  
> **Purpose:** (1) Audit the plan in `saved_plan_to_v3.md` for adjacency to the current codebase.
> (2) Document exactly what happens when a user sends a message — current codebase vs. plan vs. the user's description.  
> **No code changes are made by this document.**

---

## Part 1 — Plan Adjacency Audit: `saved_plan_to_v3.md`

### 1.1 Migration Number Collision

**Critical finding:** The plan assumes the next available migration is V047. However the codebase already has V047, V048, and V049 for unrelated work:

| Migration number | Plan expects | Codebase has |
|---|---|---|
| V047 | `reborn_recipe_step_descriptions.sql` (Phase A) | `llm_providers_is_builtin.sql` |
| V048 | `reborn_python_code.sql` (Phase B) | `seed_builtin_providers.sql` |
| V049 | `reborn_extension_catalogues.sql` (Phase C) | `session_threads_version.sql` |

**Consequence:** All migration numbers in the plan (V047–V055) need to be bumped by +3:

| Plan's migration | Correct next number | Phase |
|---|---|---|
| V047 | V050 | Phase A — `step_descriptions` column |
| V048 | V051 | Phase B — `reborn_python_code` table |
| V049 | V052 | Phase C — `reborn_extension_catalogues` table |
| V050 | V053 | Phase D — `step_link` column on intent inputs |
| V051 | V054 | Phase J — intent_examples + dependency_registry |
| V052 | V055 | Phase K — basic_prompt_store |
| V053 | V056 | Phase L — capability_id on reborn_tools |
| V054 | V057 | Phase M — template matching columns |
| V055 | V058 | Phase N — validation_queue |

**Action required:** Before starting Phase A, update the plan's migration numbers.

---

### 1.2 Phase Status — All 14 Phases are Pending

All phases (A through N) are marked `[ ] Pending`. Zero v3 implementation exists. This is confirmed:

- `crates/brassclaw_engine/src/memory/instruction_builder.rs` — **does not exist** (Phase A blocker)
- `step_descriptions JSONB` column on `reborn_recipes` — **does not exist** (V047 / now V050)
- `reborn_python_code` table — **does not exist** (V048 / now V051)
- `reborn_extension_catalogues` table — **does not exist** (V049 / now V052)
- `step_link` column on `reborn_intent_inputs` — **does not exist** (V050 / now V053)
- `FetchForTurnResult::SplitResult` variant — **does not exist** (Phase E)
- `FetchForTurnResult::ActionShortCircuit` variant — **does not exist** (Phase E)
- `RecipeStep::TierZero` variant — **does not exist** (Phase H)
- `reborn_validation_queue` table — **does not exist** (V055 / now V058)
- `BuildInstruction` struct — **does not exist** (Phase A)
- `IBS` module — **does not exist** (Phase A)

**Conclusion:** The v3 plan is entirely future work. Nothing from the new recipe/IBS/split-channel architecture has been built yet.

---

### 1.3 Adjacency Gaps — Plan Assumptions vs. Actual Code

These are places where the plan's descriptions of the "current codebase" need correction:

#### Gap 1: `IntentResolution::Match` has no `step_link` field

The plan's Phase D states "the current `IntentResolution::Match` has `{ component_id: Uuid, component_class_code: i32 }` — no `step_link`" — this is correct and consistent. Phase D adds `step_link: Option<String>` to the variant. ✅ Correct description.

#### Gap 2: `handle_assemble_prior_knowledge` — hardcoded tenant_id bug confirmed

The plan (Phase F security fix) documents a bug at `orchestrator.rs` line 2581:
```rust
ComponentScope {
    tenant_id: "default".to_string(),  // ← hardcoded
    agent_id: String::new(),           // ← empty
    ...
}
```
This bug **is confirmed at line 2575–2580** in the current code. The fix in Phase F is valid and urgent for multi-tenant correctness.

#### Gap 3: `RamSource` is the active production retrieval backend

The plan describes `PostgresSource` as the intent-driven backend (§0.8, fetch_for_turn). This is true for the code — `PostgresSource` exists and is correct. However, the plan does not explicitly state that **`PostgresSource` is never wired in production today** — it's only used in tests. The production backend is `RamSource` (wired in `manager.rs`). Phase K removes `RamSource`; but Phase A–H can't benefit from intent-driven retrieval until Phase K wires `PostgresSource`.

**Recommendation:** Add a note in Phase A that `PostgresSource` wiring into `manager.rs` must happen before or during Phase H, not deferred to Phase K.

#### Gap 4: RecipeStage stub is more explicit than the plan describes

The plan (§0.10) says "RecipeStage (step 4) always falls through to Tier 2. Phase H closes this." The codebase goes further: the module comment in `recipe.rs` explicitly names the two resolution options (add `last_user_text` to state, or reposition the stage). The plan's Phase H aligns with Option 1. ✅ Consistent.

#### Gap 5: Disambiguation UX is not wired in Python

The plan documents disambiguation handling (§3.12 in CLAUDE.md) but doesn't call out that `default.py` currently has no Python-side handler for `pkr.get("disambiguation")`. The Rust side returns `{disambiguation: true, candidates: [...]}` correctly (orchestrator.rs line 2609), but `default.py` lines 999-1008 only check for `override_prompt_creation` and `formatted_content` — there is no `if pkr.get("disambiguation")` branch. Phase G must add this alongside removing the dead shim.

#### Gap 6: `__retrieve_docs__` shim — the dead code check is wrong

The plan (§0.9 Problem 1) correctly identifies this as a "dead Action-detection shim" because `metadata.get("class_code") == 16` never fires. This is confirmed: `handle_retrieve_docs` in `orchestrator.rs` uses the legacy `RetrievalEngine::retrieve_context` path which returns `{type, title, content}` dicts — **without `class_code`** in the metadata. The check at `default.py:1022` (`metadata.get("class_code") == 16`) is indeed dead code. ✅ Plan is correct.

#### Gap 7: `formatted_content` is JSON, not prose string

The plan (§0.9 FINDING F) correctly warns that `formatted_content` is currently a JSON-encoded object from `assemble_from_component_items` (structured as `{"prior_knowledge": [...], "matched_components": [...]}`), not a flat prose string. Phase F must change this. This is accurate — confirmed in `orchestrator.rs` line 2674.

#### Gap 8: `action_short_circuit` field name

The plan says `default.py` should check `pkr.get("action_short_circuit")`. The current code at `default.py` line ~1010 checks for Actions via a separate `__retrieve_docs__` call (the shim). Phase G removes the shim and adds the `action_short_circuit` check. The Python-side code for this does not yet exist. ✅ Consistent with plan.

#### Gap 9: `Step 11` references in `brassclaw.service` 

The comment referencing "Phase 11" was in `deploy/brassclaw.service` and has been removed in Goals_pre_v3_review Step 1. ✅ Fixed.

---

### 1.4 Plan Sections That Are Accurate and Well-Grounded

- §0.10 Turn Pipeline — matches `canonical.rs` exactly (stage sequence confirmed)
- §0.7 IBS memoisation key — correct design (no implementation yet)
- §0.8 `FetchForTurnResult` current state — correctly describes the 2-variant enum
- §0.9 three-call block in `default.py` step-0 — accurately describes current code
- §0.12 Actions (`class 16`) override path — accurately describes `override_prompt_creation` behaviour
- §0.16 Builtin tool bootstrap — the gap it describes (zero DB rows for builtins) is confirmed
- §0.18 Validation queue — the gap it describes (no queue table) is confirmed
- Phase N note about `PgMontyVmSettingsStore::upsert` not being INSERT+ON CONFLICT — confirmed correct finding

---

### 1.5 Plan Corrections Required

The following changes to the plan text are recommended before starting Phase A:

1. **Renumber all migrations** from V047–V055 to V050–V058 (see §1.1 table above)
2. **Add a note** in Phase A that `PostgresSource` wiring into `manager.rs` must happen alongside or before Phase H to benefit from intent resolution
3. **Add Phase G sub-task**: wire disambiguation UX in `default.py` (check `pkr.get("disambiguation")` and surface candidates)
4. **Update Phase H note**: the `RecipeStep::Continue` variant is the only current variant (not `RecipeStep::TierZero` or `RecipeStep::ActionExecuted`) — the plan already says this but should emphasize it's the sole current variant

---

## Part 2 — What Actually Happens After a User Sends a Message

### 2.1 Current Codebase Flow (Today)

```
User types a message → chat sends it to the backend
│
├─ [Gateway/WebUI ingress]
│   The message arrives as an HTTP request. The WebUI v2 gateway
│   (brassclaw_reborn_webui_ingress) validates the bearer token and
│   maps the request to a turn submission.
│
├─ [Turn submission → loop runner]
│   The turn runner wakes the agent loop for the thread.
│   A LoopInput::UserMessage is enqueued.
│
└─ [Agent loop — DefaultExecutorPipeline::execute()]
```

The agent loop runs these stages in order:

```
1. CheckpointStage.cancel_if_requested()
   → Polls for cancellation. If cancelled, return exit.

2. BudgetStage
   → Checks iteration count and token budget.
   → If exceeded, send budget-exceeded message and exit.

3. InputStage
   → Drains UserMessage inputs from the queue.
   → Updates state.input_cursor.
   ⚠️ NOTE: Does NOT populate last_user_text (the field does not exist yet).

4. RecipeStage            ← COMPLETE STUB
   → Checks if recipe_lookup is wired (it is, via PgRecipeLibrary).
   → But since last_user_text is missing, skips lookup entirely.
   → Always returns RecipeStep::Continue (Tier 2).
   → No recipe matching, no Tier 0, no Tier 1.

5. PromptStage
   → Calls ctx.host.build_prompt_bundle(context_request).
   → This assembles the LLM prompt: system prompt, conversation history,
     prior knowledge, tool descriptions.
   → Prior knowledge comes from fetch_for_consumer via the host layer.
     (NOT from fetch_for_turn / resolve_intent — the intent path is not
      wired in production. RamSource is the backend, not PostgresSource.)

6. InterceptorStage
   → Sends the assembled prompt to the Sempai interceptor (if wired).
   → Creates a ForensicPacket for telemetry.

7. ModelStage
   → Sends the prompt to the LLM (via the configured provider).
   → Streams the response back.
   → Notifies the interceptor of the response.

8. ReplyAdmissionStage OR CapabilityStage
   → If the model output is plain text:
     ReplyAdmissionStage validates it, AssistantReplyStage sends it to the user.
   → If the model output contains tool calls (structured):
     CapabilityStage executes them, results injected back, loop continues.
   → Inside CapabilityStage, the Python orchestrator (default.py) runs via
     the Monty VM (brassclaw_engine).

9. StopStage / StopObservationStage
   → Decides whether to run another iteration or exit.

10. ExitStage
    → Cleans up and returns LoopExit.
```

### 2.2 What Happens Inside the Python Orchestrator (default.py Step 0)

When `CapabilityStage` or the Python execution path runs `default.py`, step 0 executes:

```python
# Step 0 — prior knowledge assembly (current code):
if step == 0:
    pkr = __assemble_prior_knowledge__(goal, token_budget, "02")   # ← CALL 1
    if isinstance(pkr, dict):
        if pkr.get("override_prompt_creation"):
            working_messages = [{"role": "User",
                                  "content": pkr.get("formatted_content", "")}]
        elif pkr.get("formatted_content"):
            insert_as_user_message_at_n_minus_1(working_messages,
                                                pkr["formatted_content"])
        insert_volatile_context_at_n_minus_1(working_messages)  # ← no-op placeholder

    # DEAD SHIM — called but never fires class_code detection:
    docs = __retrieve_docs__(goal, 5)    # ← CALL 2 (dead, class_code always absent)
    if docs:
        for doc in docs:
            metadata = doc.get("metadata", {}) if isinstance(doc, dict) else {}
            if metadata.get("class_code") == 16:   # ← NEVER TRUE
                # ... action execution (never reached)

    # all_skills round-trip (calls __list_skills__):
    all_skills = __list_skills__()           # ← CALL 3
    active_skills = select_skills(all_skills, goal, ...)   # scores by keyword
```

`__assemble_prior_knowledge__` at the Rust side:
1. Calls `RamSource::fetch_for_turn()` (= `RamSource::fetch_for_consumer()` because `RamSource` uses the default implementation)
2. `RamSource::fetch_for_consumer()` calls `RetrievalEngine::retrieve_context()` (legacy MemoryDoc path)
3. Returns `{content, formatted_content, override_prompt_creation, matched_component_ids}`
4. `formatted_content` is a JSON-encoded object (not prose): `{"prior_knowledge":[...], "matched_components":[...]}`

**Important:** The intent system (`resolve_intent`, `PostgresSource`) is **not reached** in production today because `RamSource` is wired (not `PostgresSource`).

### 2.3 Complete Current Message Flow (Summarized)

```
User message received
  ↓
Turn submitted to agent loop
  ↓
InputStage drains message (no last_user_text saved)
  ↓
RecipeStage: always skipped (stub — missing last_user_text)
  ↓
PromptStage: assembles prompt using RamSource keyword scan
             (NO intent resolution, NO recipe matching)
  ↓
InterceptorStage: telemetry
  ↓
ModelStage: LLM call
  ↓
    → Text reply? → AssistantReplyStage → sent to user
    → Tool calls? → CapabilityStage → Python orchestrator runs:
        Step 0: __assemble_prior_knowledge__ (RamSource keyword scan)
                __retrieve_docs__ (dead shim, never fires)
                __list_skills__() + select_skills()
        Steps 1–N: LLM response handling, tool execution, result formatting
  ↓
StopStage: decide to continue or exit
  ↓
Response visible in chat
```

---

## Part 3 — Comparison: User's Description vs. Plan vs. Current Codebase

### 3.1 User's Description

The user described this flow after a message is entered:

1. Message sent from chat to the orchestrator
2. Orchestrator sends message to the intention system
3a. **Match found** → response contains a "recipe" → orchestrator performs recipe steps
3b. **No match** → LLM prompt created with head+body; one part is chat+history+memories,
    the other is the "base prompt" from KV-cache. "base-prompt" placeholder replaced by
    sempai-kohai system just before tokenizing.
4. Answer sent back to chat

### 3.2 How This Maps to the Current Codebase

| User's description | Current reality | Plan target (v3) |
|---|---|---|
| "Message sent to orchestrator" | Message arrives via gateway, goes through agent loop stages before reaching Python orchestrator | Same — gateway/loop architecture unchanged |
| "Orchestrator sends to intention system" | ❌ NOT HAPPENING TODAY. RecipeStage is a stub. Intention system (`resolve_intent`) is never called because `last_user_text` is missing from state and `PostgresSource` is not wired | ✅ Phase H adds `last_user_text`, Phase D–E wire intent lookup |
| "Match found → recipe" | ❌ NOT HAPPENING TODAY. No recipe is ever selected | ✅ Phase H activates Tier 0/1 dispatch |
| "Recipe contains all info for orchestrator/rust" | ❌ NO IBS, no SplitResult, no two-channel delivery | ✅ Phases A–H build IBS + SplitResult + channel delivery |
| "No match → LLM prompt created" | ✅ This IS happening (every turn is effectively "no match" today) | ✅ Tier 2 path preserved |
| "Head = base prompt from KV-cache" | ❌ No `reborn_basic_prompt_store` table exists. No KV-cache pre-compilation. The interceptor has "pre-warm" UI but the underlying store is not implemented | ✅ Phase K.1 adds `reborn_basic_prompt_store` |
| "base-prompt placeholder replaced by sempai-kohai" | ❌ The interceptor exists but the "base-prompt" line substitution is not implemented | ✅ Phase K.1 wires this into the interceptor |
| "Body = chat+history+selected memories" | ✅ `PromptStage` does assemble history + prior knowledge (via `RamSource` keyword scan) | ✅ In v3 this becomes the "orchestrator_content" from the recipe or UNION ALL |
| "Answer sent back to chat" | ✅ `AssistantReplyStage` sends the reply | ✅ Same |

### 3.3 Where the User's Description Is Correct

- The overall architecture shape is correct: gateway → orchestrator → intent → recipe or LLM.
- The "no match → full LLM with context" path correctly describes what happens today (all turns).
- The separation of "base prompt" (pre-compiled, KV-cached) from "per-turn content" (history+memories) is the correct target design per the plan.
- The sempai-kohai system replacing the "base-prompt" placeholder just before tokenizing is the correct design per §0.13 and Phase K.

### 3.4 Where the User's Description Differs from Current Reality

1. **The intent system is not in the loop today.** The RecipeStage is a stub. No recipe is ever selected. Every message goes directly to the full LLM path (Tier 2).

2. **The "base prompt" KV-cache doesn't exist yet.** No pre-compiled base prompt, no `reborn_basic_prompt_store`, no "base-prompt" placeholder mechanism. The interceptor UI button for "Pre-warm Sempai KV-cache" exists in the WebUI but the store is not implemented.

3. **The two-channel delivery (rust_items / orchestrator_items) doesn't exist.** The IBS, `BuildInstruction`, `SplitResult`, `TierZero` routing — none of these exist yet. All of this is Phase A–H work.

4. **"Selected memories"** — the user says "selected memories formatted for LLM". Currently, `__assemble_prior_knowledge__` returns component items (Specs, ToolSkills, Recipes, etc.) via `RamSource` keyword scan — not "memories" in the traditional sense, but it is analogous to what the plan calls "orchestrator_content" from the UNION ALL path.

### 3.5 What the Plan Envisions for the Future (v3 Target)

After all phases complete, the correct flow will be:

```
User types: "show all files including hidden in /tmp"
  ↓
[InputStage] saves last_user_text = "show all files..."
  ↓
[RecipeStage] calls fetch_for_turn(scope, last_user_text, budget, "02")
   → resolve_intent(pool, scope, "show all files...")
   → Match: recipe_id = uuid-local-files-reading, class_code = 21,
            step_link = "0:0-0:30+1:0-1:E"
   → IBS: build_instruction(step_link, step_descriptions, variable_patterns)
   → fetch rust_items (ToolSkill bodies) + orchestrator_items (Skill + PythonCode bodies)
   → SplitResult { rust_items, orchestrator_items, routing }
   
   IF tier0_eligible (wilson >= 0.70, validated, no llm required):
     → apply rust_items to Rust execution context
     → stash orchestrator_items in state.recipe_hint
     → return PostRecipeOutcome::TierZero
     → [PromptStage SKIPPED] [ModelStage SKIPPED]
   
   ELSE (Tier 1 — LLM guided):
     → stash both in state
     → return PostRecipeOutcome::NeedsPrompt
     → [PromptStage]: builds prompt, injects stashed orchestrator_items as context
                       (reads state.recipe_hint, does NOT consume it)
     → [InterceptorStage]: sempai reviews prompt + recipe hint
     → [ModelStage]: LLM call (guided by recipe context)
  ↓
[CapabilityStage / Python orchestrator]
  default.py step 0:
    pkr = __assemble_prior_knowledge__(goal, budget, "02")
    # handler: finds state.recipe_hint → unstash, skip fetch_for_turn
    # pkr["orchestrator_content"] = formatted Skill bodies + PythonCode bodies
    # pkr["override_prompt_creation"] = True (Tier 0) or False (Tier 1+)
    # _set_active_skills_from_matched_ids(pkr["matched_component_ids"])
    # NO __retrieve_docs__() call (removed in Phase G)
    # NO __list_skills__() call (IBS already selected by UUID)
  ↓
[AssistantReplyStage]: emit result; Wilson score updated
```

For **no-match** (Tier 2 — "explain recursion to me"):
```
[RecipeStage]: fetch_for_turn → NoMatch → fetch_for_consumer (UNION ALL)
              → FetchForTurnResult::Components([...keywords matched])
              → RecipeStep::Continue (Tier 2)
[PromptStage]: normal assembly with UNION ALL content
[ModelStage]: full LLM reasoning
[CapabilityStage]: Python step 0 calls __assemble_prior_knowledge__
                   → handler: state.recipe_hint is None → calls fetch_for_turn
                   → returns UNION ALL items as orchestrator_content
```

The **base-prompt KV-cache** layer (Phase K):
```
Before the LLM call is dispatched, the Interceptor's pre-compiled
base-prompt (stored in reborn_basic_prompt_store) is prepended to
the prompt bundle. The "base-prompt" placeholder line in the assembled
prompt is replaced by the sempai-kohai system with the actual pre-compiled
content. This means the LLM's KV-cache prefix already contains all Skill
bodies, ToolSkill descriptions, PythonCode helpers, and catalogue overviews
— the per-turn recipe patch only adds what's new.

If the base-prompt is not yet compiled (first boot or stale), a short
minimal context is substituted instead, with a note that full precompilation
is pending.
```

---

## Part 4 — Critical Gaps Summary

### Gaps Blocking v3 Intent-Driven Flow

| Gap | Impact | Phase that fixes it |
|-----|--------|---------------------|
| `last_user_text` missing from `LoopExecutionState` | RecipeStage never runs | Phase H |
| `PostgresSource` not wired in production | Intent system dead in production | Phase K (wiring task) |
| `IBS` module (`instruction_builder.rs`) doesn't exist | No BuildInstruction, no channel split | Phase A |
| `step_descriptions` column missing on `reborn_recipes` | No StepDescription storage | Phase A (migration V050*) |
| `SplitResult`/`ActionShortCircuit` variants missing from `FetchForTurnResult` | No recipe routing | Phase E |
| `RecipeStep::TierZero`/`ActionExecuted` missing | No Tier 0/1 dispatch | Phase H |
| `step_link` column missing on `reborn_intent_inputs` | No variant-aware intent matching | Phase D (migration V053*) |
| Disambiguation handler missing in `default.py` | Disambiguation UX never surfaces | Phase G |
| `reborn_basic_prompt_store` table missing | No KV-cache base prompt | Phase K.1 (migration V055*) |
| `reborn_validation_queue` table missing | No new lifecycle queue | Phase N (migration V058*) |
| Hardcoded `tenant_id: "default"` in `handle_assemble_prior_knowledge` | Multi-tenant isolation broken | Phase F security fix |

*Migration numbers corrected from plan's numbering — see §1.1.

### Pre-existing Gaps Fixed in Goals_pre_v3_review

| Gap | Fix |
|-----|-----|
| `BRASSCLAW_REBORN_PROFILE` in deploy files | Step 1 — replaced with `BRASSCLAW_RUNTIME_PROFILE` |
| Outdated documentation | Step 2 — updated README, docs/ |
| `InMemoryBoundedSubagentGoalStore` production fallback | Step 3 — fail hard |
| `InMemoryOutboundStateStore` production fallback | Step 4 — fail hard |
| `StoreBackedRecipeStore` / `RecipeLibrary` MemoryDoc fallbacks | Steps 6, 7 — removed |
| `DbLessFallback` enum variant (unreachable) | Step 9 — removed |

---

## Part 5 — Recommendations for the Plan

1. **Update migration numbers** in `saved_plan_to_v3.md` before starting Phase A. The plan's V047–V055 are now V050–V058.

2. **Add PostgresSource wiring** as a sub-task in Phase H or between Phase E and Phase H. Without wiring `PostgresSource` into the production `manager.rs` code path, Phases A–G provide infrastructure that is built but never exercised in production.

3. **Add disambiguation Python handler** as an explicit Phase G sub-task. The Rust side returns `{disambiguation: true, candidates: [...]}` correctly, but `default.py` has no handler for it.

4. **Phase K ordering clarification:** Phase K removes `RamSource` and `retrieval_dbless.rs`. This means Phase K must happen AFTER the `PostgresSource` wiring mentioned in #2 above. If Phase K removes `RamSource` before `PostgresSource` is wired, the production retrieval path is broken.

5. **Builtin bootstrap (Phase L) dependency:** Phase L calls `build_instruction` from the IBS. This requires Phase A (IBS) and Phase D (`step_link` column) to be complete before Phase L can be tested.

6. **Validation queue (Phase N) ordering:** Phase N drops columns from component tables. This is a two-step deploy requiring zero-downtime care. The plan (§N.4) documents this correctly. Emphasize: run Phase N LAST — it is the most destructive migration.
