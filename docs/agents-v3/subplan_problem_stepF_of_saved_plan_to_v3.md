# Subplan — Phase F problem resolution (`saved_plan_to_v3.md`)

Parent plan: `saved_plan_to_v3.md` → Phase F (`lines 5004–5200`).
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase F step `45d899c7-d3ef-4dae-a07b-85f4affc939c`.

---

## 1. Why this subplan exists — the pre-E0-A-plan vs post-E0-A-codebase gap

Phase F was written **before** the E0-A re-target (long-term driver = agent-loop;
engine-as-library). The plan body assumes `handle_assemble_prior_knowledge` is
the *live* retrieval path and that `Thread` will gain `tenant_id`/`agent_id`.
Grounding the live codebase after E.0–E.6 revealed four gaps between that
assumption and the code as it now stands:

1. **The engine handler is dormant, but already has all four `FetchForTurnResult`
   arms** (orchestrator.rs:2597 `Components`, :2600 `Disambiguation`,
   :2624 `ActionShortCircuit`, :2643 `SplitResult`) — added in earlier phases.
   Its `SplitResult` arm binds only `orchestrator_items, ..` and ignores
   `rust_items` + `routing`, returning a plain `assemble_from_component_items`
   dict (no `tier_zero`, no `action_short_circuit: false`, no `disambiguation:
   false`, no `rust_items`, no `matched_component_ids` from routing). The plan
   §0.9 / DRIVER-GAP requires the full §0.9 routing dict. The live Tier-0/Tier-1
   dispatch is the Phase-H agent-loop consumer (via the composition
   `PgRetrievalLookup` bridge); this engine Python path is deliberately dormant
   but must be **plan-faithful if re-activated** (Q-F1 → upgrade it).

2. **`Thread` carries no `tenant_id` and no `agent_id`** (types/thread.rs:212–245;
   FIND-P8-01 confirmed). The handler's scope stub at orchestrator.rs:2586–2591
   uses `tenant_id: thread.user_id.clone()` + `agent_id: "default"`, and the
   `__list_skills__` skills-db stub at orchestrator.rs:3177–3182 uses
   `scope_from_thread_ids(&thread.user_id, &thread.user_id, "default", …)`.
   Now that `PostgresSource` is wired (Phase E.0), these drive the real scope
   filter and would cause cross-tenant intent leakage (Q-F2 → fix both).

3. **The LIVE agent-loop retrieval scope has the same tenant stub.**
   `build_component_scope` (retrieval_lookup_impl.rs:108–136) sets
   `tenant_id: user_id.clone()` (:131) — but the live `LoopRunContext.scope` is a
   `TurnScope` (brassclaw_turns/src/scope.rs:5–12) that **already carries a real,
   always-present `tenant_id: TenantId`** + `agent_id: Option<AgentId>` +
   `project_id: Option<ProjectId>`. So the live fix is to read
   `context.scope.tenant_id` (Q-F2 → fix live path too).

4. **`__fetch_component__(uuid, class_code)` is not registered.** The host-fn
   dispatch (orchestrator.rs:637–711) has arms for `__retrieve_docs__`,
   `__list_skills__`, `__assemble_prior_knowledge__`, etc., but no
   `__fetch_component__`. The plan requires it (used by `call_action` nested
   lookups, §0.9); Phase G depends on it (Q-F3 → register now).

**Engine-identity constraint (Q-F5 grounding):** `brassclaw_engine` has **no
`brassclaw_turns` dependency** and the engine `Thread`/`ThreadManager`/`spawn_*`
API has **zero composition/agent-loop callers** (cross-crate grep for
`ThreadManager`/`.spawn_thread`/`Thread::new` outside `brassclaw_engine` = 0
matches). So the engine spawn API (`spawn_thread` / `spawn_thread_with_title` /
`spawn_thread_with_history` at manager.rs:128/157/193, + conversation.rs:356)
has **no source** for real `tenant_id`/`agent_id` — only `user_id` exists there.
The only engine `Thread::new` site where tenant/agent are already in scope is
the **subagent child** at scripting.rs:1995, which can inherit
`parent_thread.tenant_id`/`agent_id`. The **LIVE** retrieval path gets real
tenant from F.4 (`context.scope.tenant_id`). Per Q-F5 → A, engine spawn-created
threads keep the `#[serde(default)]` empty-string default (documented as a known
limitation of the dormant engine path); both orchestrator.rs stubs read
`thread.tenant_id`/`thread.agent_id`; live correctness comes from F.4.

---

## 2. User design decisions (all confirmed via ask_user)

1. **Q-F1 (dormant handler):** Upgrade `handle_assemble_prior_knowledge`'s
   `SplitResult` arm to the **full plan-§0.9 routing dict** (plan-faithful;
   satisfies the Phase F unit tests; keeps the dormant path consistent if
   re-activated). Do **not** leave the plain `assemble_from_component_items`
   return.
2. **Q-F2 (security fix location):** Fix **BOTH** paths. (a) LIVE
   `build_component_scope` uses `context.scope.tenant_id`; (b) add
   `tenant_id`/`agent_id` to `Thread` + thread real values at the call site
   where they exist (subagent child) + fix **both** orchestrator.rs stubs
   (2586 `handle_assemble_prior_knowledge` AND 3177 `__list_skills__`).
3. **Q-F3 (`__fetch_component__`):** Register **now** in Phase F (plan-faithful;
   Phase G depends on it). Handler calls `fetch_component_by_id(uuid,
   class_code)` directly; returns a single-item dict or `None`.
4. **Q-F4 (`tier_zero` signal):** `RetrievalTurnResult.tier0_eligible` **IS** the
   signal on the live path — nothing new needed there (Phase H consumes it). On
   the **dormant** engine handler path, the §0.9 dict emits `tier_zero: true`
   when `SplitResult.routing.llm_call_required == false` (the dormant-path
   equivalent of `tier0_eligible`).
5. **Q-F5 (engine spawn API threading):** **Option A** — subagent child inherits
   `parent_thread.tenant_id`/`agent_id` (scripting.rs:1995); all other engine
   `Thread::new` sites (spawn API) keep `#[serde(default)]` empty strings (no
   tenant/agent source exists in the engine); both orchestrator.rs stubs read
   `thread.tenant_id`/`thread.agent_id`; LIVE path correctness via F.4. Document
   the engine spawn default-empty as a known limitation of the dormant engine
   path (E0-A).

---

## 3. Ordered substeps (run strictly one after another)

### F.1 — `Thread` gains `tenant_id` / `agent_id` + builder

**File:** `crates/brassclaw_engine/src/types/thread.rs`

- Add two fields to `struct Thread` (after `user_id`, ~line 226), each
  `#[serde(default)]` so legacy checkpoints deserialize (empty string):
  ```rust
  /// Tenant identifier. Added v3 Phase F. `#[serde(default)]` = "" for legacy threads.
  #[serde(default)]
  pub tenant_id: String,
  /// Agent context identifier. Added v3 Phase F. `#[serde(default)]` = "" for legacy threads.
  #[serde(default)]
  pub agent_id: String,
  ```
- Initialize both to `String::new()` in `Thread::new` (so every construction
  site compiles unchanged — the builder opt-in is per FIND-P8-01).
- Add a builder:
  ```rust
  /// Set the tenant + agent identity (v3 Phase F). Engine spawn-created
  /// threads leave these empty (no `brassclaw_turns` identity source in the
  /// engine); the subagent child inherits the parent's values; tests set
  /// explicit values. The LIVE retrieval path sources tenant from
  /// `LoopRunContext.scope.tenant_id` (F.4), not from `Thread`.
  pub fn with_tenant_agent(mut self, tenant_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
      self.tenant_id = tenant_id.into();
      self.agent_id = agent_id.into();
      self
  }
  ```

### F.2 — Subagent child inherits parent identity

**File:** `crates/brassclaw_engine/src/executor/scripting.rs` (~line 1995)

- After `Thread::new(...).with_parent(parent_thread.id)`, chain
  `.with_tenant_agent(parent_thread.tenant_id.clone(), parent_thread.agent_id.clone())`
  so the child inherits the parent's tenant/agent (the one engine site where
  real values are in scope — Q-F5 → A).
- No change to the spawn API (`spawn_thread*` / conversation.rs:356): they have
  no tenant/agent source and keep the empty-string default (documented in F.1's
  builder doc + §1 engine-identity constraint).

### F.3 — Fix both orchestrator.rs scope stubs

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs`

- Stub 1 (`handle_assemble_prior_knowledge`, :2586–2591): replace
  `tenant_id: thread.user_id.clone()` → `thread.tenant_id.clone()` and
  `agent_id: "default".to_string()` → `thread.agent_id.clone()`. Keep
  `user_id: thread.user_id.clone()` and `project_id: thread.project_id.to_string()`
  unchanged. Update the surrounding comment to reference Phase F (done) +
  F.4/Q-F5.
- Stub 2 (`__list_skills__` skills-db path, :3177–3182): replace
  `scope_from_thread_ids(&thread.user_id, &thread.user_id, "default", …)` →
  `scope_from_thread_ids(&thread.tenant_id, &thread.user_id, &thread.agent_id, …)`.
  Update the comment likewise.
- **Safety verified:** the existing `handle_assemble_prior_knowledge` unit test
  (orchestrator.rs:7150) calls with `retrieval=None, retrieval_source=None` →
  legacy fallback, never hits stub 1. No test asserts the old `__list_skills__`
  skills-db stub. So neither fix breaks existing tests.

### F.4 — Fix the LIVE `build_component_scope`

**File:** `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs`
(:108–136, `#[cfg(feature = "skills-db")]`)

- Replace `tenant_id: user_id.clone()` (:131) with the real
  `context.scope.tenant_id` (a `TenantId`, always present per
  brassclaw_turns/src/scope.rs:6). `TenantId` → `String` via its `as_str`/`to_string`.
  Keep `user_id` sourced from the actor/owner/system-sentinel chain (:110–119 —
  that is the *user*, distinct from the tenant), `agent_id` from
  `scope.agent_id` (:120–124, already real), `project_id` from `scope.project_id`
  (:125–129, already real).
- Update the doc comment (:101–106) to record that real tenancy arrived in
  Phase F (Q-F2/F4). The `tenant_id mirrors user_id` stub is retired.

### F.5 — Upgrade the dormant `SplitResult` arm to the full §0.9 dict

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs` (:2643–2658)

- Bind the full `SplitResult { rust_items, orchestrator_items, routing, .. }`
  (currently `orchestrator_items, ..`).
- Build `orchestrator_content` via the existing
  `assemble_from_component_items` *value* extraction (reuse the raw-content +
  formatted-content assembly the helper already produces), but emit the §0.9
  dict directly (not the helper's `ExtFunctionResult`) so we can add the routing
  fields. Concretely:
  - `orchestrator_content`: the LLM-facing assembled content for the
    orchestrator channel (formatted JSON string + raw alias — same shape
    `assemble_from_component_items` returns under `formatted_content` / `content`).
  - `formatted_content`: **alias** of `orchestrator_content` (plan §0.9 + Phase F
    test #2 "alias preserved").
  - `action_short_circuit: false`, `disambiguation: false`.
  - `tier_zero: !routing.llm_call_required` (Q-F4: dormant-path equivalent of
    `tier0_eligible`; true when no LLM is required → Tier-0 deterministic).
  - `matched_component_ids`: `routing.matched_component_ids` (the orchestrator-
    channel UUID identity set from E.4).
  - `override_prompt_creation`: `routing.override_prompt_creation`.
  - `rust_items`: serialized list (informational — the plan §0.9 note says the
    Python side never applies rust_items directly; `RecipeStage` does that on the
    live path). Emit each as `{ id, class_code, name, content }` (the same fields
    `assemble_from_component_items` uses).
  - `variant_label`, `step_link`, `wilson_lower`, `llm_call_required`,
    `tier0_eligible` carried from `routing` for the Phase-H-shaped consumer.
- Keep the `ActionShortCircuit` arm (:2624) but align its keys to §0.9: it already
  emits `action_short_circuit: true`, `action_component_id`, `action_name`,
  `override_prompt_creation: false`, `matched_component_ids`. Add
  `orchestrator_content: ""` and `formatted_content: ""` (plan §0.9 ActionShortCircuit
  shape) — currently it uses `content: ""`/`formatted_content: ""`; add the
  `orchestrator_content` alias for consistency with the SplitResult arm.
- `Components` + `Disambiguation` arms unchanged (already §0.9-correct).
- Preserve the legacy `retrieve_context` fallback (:2666–2705) unchanged
  (Phase K.3 removes it — plan explicit).

### F.6 — Register `__fetch_component__(uuid, class_code)`

**File:** `crates/brassclaw_engine/src/executor/orchestrator.rs`

- Add a new async handler `handle_fetch_component(args, thread, retrieval_source)`:
  - `uuid = args[0]` (string → `uuid::Uuid`).
  - `class_code = args[1]` (int → `i32`).
  - If `retrieval_source` is `Some`, call
    `fetch_component_by_id(pool, scope, uuid, class_code)` (scope built with the
    F.3-fixed `thread.tenant_id`/`agent_id`). On a single item, return a dict
    `{ id, class_code, name, description, content, override_prompt_creation }`
    (the `ComponentItem` fields). On none / error / no source, return
    `ExtFunctionResult::Return(json_to_monty(&serde_json::Value::Null))` (None).
  - `fetch_component_by_id` lives in `memory/retrieval_source.rs` and is
    `#[cfg(feature = "skills-db")]`; gate the DB call accordingly and return
    `Null` when the feature is off (consistent with how `__list_skills__` gates
    its skills-db path).
- Add the dispatch arm at :705 (after `__assemble_prior_knowledge__`):
  ```rust
  "__fetch_component__" => {
      handle_fetch_component(args, thread, retrieval_source).await
  }
  ```
- Note: `fetch_component_by_id` takes `&brassclaw_pg::PgPool`, not the
  `&dyn RetrievalSource` trait. Verify how the pool reaches this handler —
  `retrieval_source: Option<&Arc<dyn RetrievalSource>>` is the trait object. If
  the pool isn't directly in scope at the dispatch site, surface this in F.6
  grounding (the `PostgresSource` is behind the trait; either add a
  `fetch_component_by_id` method to the `RetrievalSource` trait, or thread the
  `pg_pool` into the dispatch like `__list_skills__` does at :667). Resolve before
  implementing F.6 (do NOT stub).

### F.7 — Tests + verification + commit

**Tests (the plan's Phase F list, saved_plan:5190–5198), split pure-unit vs DB:**
- Pure-unit (engine `orchestrator.rs::tests`, both configs):
  1. `SplitResult` → `orchestrator_content` contains Skill + PythonCode bodies;
     does NOT contain ToolSkill bodies; no `type:text` step info.
  2. `SplitResult` → `formatted_content` == `orchestrator_content` (alias).
  3. `ActionShortCircuit` → `action_short_circuit: true`, `orchestrator_content: ""`.
  4. `Components` (no-match) → `orchestrator_content` contains all items.
  5. `Disambiguation` → `disambiguation: true` with candidates.
  6. `handle_retrieve_docs` untouched → flat `[{type,title,content}]`.
  7. `handle_assemble_prior_knowledge` scope uses `thread.tenant_id`/`agent_id`
     (build a `Thread::new(..).with_tenant_agent("t","a")`, assert the scope
     passed to a mock `RetrievalSource` carries those — needs a mock source that
     captures the scope; the engine already has `MockRetrievalSource`-style test
     doubles — verify during F.7).
- DB-integration (composition `tests/`, `#![cfg(feature="skills-db")]`,
  testcontainer + skip-if-no-docker, mirroring `fetch_for_turn.rs`):
  8. `__fetch_component__(uuid, 16)` → correct Action item returned.
  9. Two-tenant setup → tenant A's intents do NOT match for tenant B's thread
     (the cross-tenant isolation proof — the core security fix).

**Verification (both configs — default + `--features brassclaw_reborn_composition/skills-db`):**
- `cargo fmt --all -- --check`
- `cargo clippy -p brassclaw_engine --all-targets -- -D warnings` (default + skills-db)
- `cargo clippy -p brassclaw_reborn_composition --features brassclaw_reborn_composition/skills-db --all-targets -- -D warnings`
- `cargo test -p brassclaw_engine --lib` (default + skills-db — pure-unit tests)
- DB-integration tests skip on this host (no docker) — correct-by-grounding
  against migrations + `resolve_intent` SQL + the F.3/F.4 scope fixes.

**Commit + push** each completed substep to `origin/main` before the next.

---

## 4. Verification state

(updated as substeps complete)

- F.1 — `Thread` carries `tenant_id` + `agent_id` (both `#[serde(default)]`).
  Committed `8ad91ddf`. **Done.**
- F.2 — subagent child inherits parent's tenant/agent; engine spawn
  default-empty. Committed `82aaf7cd`. **Done.**
- F.3 — live `build_component_scope` + both orchestrator.rs scope stubs fixed.
  Committed `a3bf2807`. **Done.**
- F.4 — LIVE retrieval scope sourced from `LoopRunContext.scope.tenant_id`.
  Committed `e1443ca3`. **Done.**
- F.5 — dormant `handle_assemble_prior_knowledge` `SplitResult` + `ActionShortCircuit`
  arms upgraded to §0.9 routing dict. Committed `46d64d31` (JSON stub) →
  **F.5-stub subplan** (`subplan_stub_stepF5_saved_plan_to_v3.md`) replaced the
  JSON with the prose StepContextSpec-headed block (FINDING F) + reworked the
  `Components` arm. See SF5.1–SF5.5 verification there. **Done.**
- F.6 — `__fetch_component__(uuid, class_code)` registered (cfg-gated pool
  pattern, dict-or-Null). Committed `756ac551`. **Done.**
- F.7 — Phase F tests (9: pure-unit #1–#7 + DB-integration #8/#9). **Pending.**
