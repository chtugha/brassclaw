# Subplan — Phase E.0 (Re-targeted E0-A): make `PostgresSource::fetch_for_turn` reachable in **live agent-loop turns**

Parent plan: `saved_plan_to_v3.md` → Phase E.0 (`lines 4486–4692`).
Zenflow task: `e81125fc-ce63-449e-922a-dfa80b964019`. Chat: `be1470ab-f612-4526-bc95-e1e37c8f4527`.
Inserted as a **substep** under the Zenflow Phase E.0 step `f5051a5d`.

---

## 1. Why this subplan exists — the contradiction E.0 grounding uncovered

The plan's Phase E.0 (`saved_plan_to_v3.md:4486–4692`) is written assuming the **engine `ExecutionLoop`/`ThreadManager`/`execute_orchestrator`/`handle_assemble_prior_knowledge`/`fetch_for_turn`/`PostgresSource`** chain is the **live production turn driver**, and that E.0's job is to swap `RamSource` → `PostgresSource` at `manager.rs:377–383` inside that engine path. The plan even carries a stale "DRIVER-GAP RESOLVED" note asserting "the engine `ExecutionLoop::run` is unambiguously the actual production driver."

**Code traces prove this is wrong / dormant.** Verified across two independent research passes:

1. **Engine `ExecutionLoop`/`ThreadManager` path is test-only / dormant.**
   - All 8 `ThreadManager::new` call sites are `#[cfg(test)]` (`manager.rs:1132/1160/1192`, `mission.rs:3916/4250`, `conversation.rs:856/925/1041`).
   - `execute_orchestrator` has exactly **1 caller** (`loop_engine.rs:471`), itself unreachable from production.
   - `ExecutionLoop::with_pg_pool` has **0 callers**.
   - `PostgresSource` is **never instantiated** anywhere in composition.
   - `manager.rs:377–383` (the E.0 target site) wires `RamSource`, but `manager.rs` spawn path is never reached by a production turn.

2. **The live production turn driver is the agent-loop stack**, with **no retrieval wired today**:
   - `RebornRuntime::send_user_message_internal` (composition `runtime.rs:1044`) → `TurnCoordinator::submit_turn` (persist Queued, pull-model) → `TurnRunnerWorker` (`turn_runner.rs:262`) → `PlannedDriver::run` (`planned_driver.rs:110`) → `CanonicalAgentLoopExecutor::execute_family` → the stage pipeline via `AgentLoopDriverHost` (13 ports).
   - `LoopContextPort.memory_snippets` is hard-coded `Vec::new()` (`loop_support/lib.rs:360`) — **production turns run with effectively no retrieval today**.
   - `LoopRecipePort`/`RecipeLookup` IS wired (`PgRecipeLibrary` reading `reborn_recipes`, composition `runtime.rs:2533`), but `RecipeStage::process` (`recipe.rs:55`) is a documented no-op stub that always falls through to Tier-2 because **the pipeline never exposes raw user text to the stages**.
   - **`brassclaw_reborn` has NO dependency on `brassclaw_engine`.**

3. The plan's **own target-state** (`AGENTS.md` Products/Loops/Kernel + Phase H's `LoopOrchestratorPort` design) confirms: the **agent-loop is the permanent production driver**; `brassclaw_engine` becomes a **permanent library** called via **narrow one-way ports**; the engine Python `ExecutionLoop`/`default.py` is **retired as test-only legacy** (deleted later by Phases F/G/K.3 — NOT now). This matches the reconciled code truth, not the stale "DRIVER-GAP RESOLVED" note.

### User decisions (this portion)

- **Long-term driver confirmed**: agent-loop (`PlannedDriver`/`CanonicalAgentLoopExecutor`); `brassclaw_engine` = permanent library via one-way ports; engine Python `ExecutionLoop`/`default.py` retired as test-only legacy (not deleted now).
- **E.0 re-target chosen: E0-A** — pull `LoopRetrievalPort` forward from Phase H (H4) and make `PostgresSource::fetch_for_turn` fire inside **live agent-loop turns** (the agent-loop `RecipeStage`), via a composition bridge.
- **E0-A scope boundary (user's Option 1, plan-ordered)**:
  - **IN scope**: H3 (user-text plumbing) + H4 (retrieval port + `RetrievalTurnResult` + `NoRetrieval` + 14th supertrait + blanket impl) + composition impl delegating to `PostgresSource::fetch_for_turn` + `RecipeStage` call site that **fires** `fetch_for_turn`.
  - **Routing booleans** (`tier0_eligible`, `llm_call_required`) are **conservative defaults** (`llm_call_required = true`, `tier0_eligible = false`) until **Phase E's `SplitResult`** populates them for real.
  - **OUT of scope (deliberately later)**: H5 `LoopOrchestratorPort` (the Tier-0/Tier-1 **consumer** — Phase H) and Phase E's `SplitResult` split of `rust_items`/`orchestrator_items`.
- User directive: *"Stick to the plan's implementations. Our goal is the final version as the plan suggests."* So this subplan implements the plan's H3+H4 design **faithfully**, adapted only where the plan's literal spec is **not compilable** (orphan-rule / dependency constraints — see §2 deviation, which the user's own E0-A description already mandated).

---

## 2. Faithful deviation from the plan's H4 literal spec (and why it is unavoidable)

The plan's H4 spec (`saved_plan_to_v3.md:5326–5340`) puts `fetch_for_turn` **directly** on `LoopRetrievalPort` and says "composition implements `LoopRetrievalPort`." That literal shape is **not compilable**:

- `RebornLoopDriverHost` (the production host that implements the driver-host ports) is defined in **`brassclaw_reborn`**.
- A `LoopRetrievalPort` impl that delegates to `PostgresSource::fetch_for_turn` and serializes engine `ComponentItem`s into `RetrievalTurnResult` **requires `brassclaw_engine` types** — but `brassclaw_reborn` does **not** (and must not) depend on `brassclaw_engine`.
- Moving that impl to **composition** is blocked by the **orphan rule**: composition cannot `impl LoopRetrievalPort` (defined in `brassclaw_turns`) for `RebornLoopDriverHost` (defined in `brassclaw_reborn`) — neither is local to composition.

**Resolution (already mandated by the user's E0-A description): mirror the established `RecipeLookup` precedent exactly.** That precedent already solves the identical problem:
- `RecipeLookup` trait lives in `brassclaw_turns` (`run_profile/recipe_lookup.rs`) — narrow, turns-native DTOs, **no engine types**.
- `LoopRecipePort` (turns) exposes a **sync accessor** `fn recipe_lookup(&self) -> Option<&dyn RecipeLookup>;` + a `NoRecipeLookup` default impl.
- `RebornLoopDriverHost` (brassclaw_reborn) implements `LoopRecipePort` by holding `Option<Arc<dyn RecipeLookup>>` (turns trait — **no engine dep**) and returning `as_deref()`. No engine types touch `brassclaw_reborn`.
- The engine-backed `PgRecipeLibrary` **impl of `RecipeLookup`** lives in **composition** (`pg_recipe_store.rs`), threaded in via `with_recipe_lookup`.

So E0-A introduces the **same shape for retrieval**:
- New `RetrievalLookup` trait in `brassclaw_turns` (`run_profile/retrieval_lookup.rs`) with `async fn fetch_for_turn(...) -> Result<Option<RetrievalTurnResult>, RetrievalLookupError>;` — turns-native return type, **no engine types**.
- `LoopRetrievalPort` (turns) exposes a **sync accessor** `fn retrieval_lookup(&self) -> Option<&dyn RetrievalLookup>;` + `NoRetrieval` default impl returning `None` (mirror `NoRecipeLookup`).
- `RebornLoopDriverHost` implements `LoopRetrievalPort` by holding `Option<Arc<dyn RetrievalLookup>>` (turns trait — **no engine dep**) and returning `as_deref()`.
- The engine-backed **impl of `RetrievalLookup`** lives in **composition**, backed by `PostgresSource::fetch_for_turn` (engine types available there), serializing engine `ComponentItem`s → turns-native `RetrievalTurnResult`. Threaded in via `with_retrieval_lookup`.

This is the **identical crate-boundary discipline** the plan already uses for recipes, satisfies the plan's H4 *intent* ("`RecipeStage` calls `ctx.host.fetch_for_turn(...)` — NOT a direct `PostgresSource` import"; "composition delegates to the wired `PostgresSource::fetch_for_turn`"), and keeps `brassclaw_reborn` engine-free. `RecipeStage` reaches retrieval via `ctx.host.retrieval_lookup()` then `lookup.fetch_for_turn(...)` — exactly mirroring how it would reach `ctx.host.recipe_lookup()`.

> **The same deviation applies to H3** (`resolve_message_text` reading raw text from `messages_by_run` in `brassclaw_first_party_extension_ports`): the production host can't import that crate's private `messages_by_run`, so a new turns-layer `MessageTextResolver` trait + composition impl (backed by a new public non-consuming accessor on `SelectableSkillContextSource`) is used — mirroring the RetrievalLookup/RecipeLookup pattern.

---

## 3. Exact edit sites (verified)

### Step 1 — H4: retrieval port + types (turns layer) + `Unimplemented` error variant + host impls

**`crates/brassclaw_turns/src/run_profile/retrieval_lookup.rs`** (NEW — mirror `recipe_lookup.rs`):
- `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)] pub struct RetrievalTurnResult { pub tier0_eligible: bool, pub llm_call_required: bool, pub rust_items: serde_json::Value, pub orchestrator_items: serde_json::Value, pub routing_meta: serde_json::Value }` — turns-native (matches plan H4 shape, `saved_plan_to_v3.md:5273–5289`). E.0 fills booleans conservatively; `rust_items`/`orchestrator_items` are serialized `ComponentItem` arrays (`serde_json::Value`, NOT engine types — the composition impl does the `ComponentItem → Value` serialization at the boundary).
- `#[derive(Debug, thiserror::Error)] pub enum RetrievalLookupError { #[error("retrieval lookup db error: {0}")] Db(String), #[error("retrieval lookup internal: {0}")] Internal(String) }` — mirror `RecipeLookupError`.
- `#[async_trait] pub trait RetrievalLookup: Send + Sync { async fn fetch_for_turn(&self, context: &LoopRunContext, query: &str, token_budget: usize, sender_class_code: &str) -> Result<Option<RetrievalTurnResult>, RetrievalLookupError>; }`
- Re-export from `crates/brassclaw_turns/src/run_profile/mod.rs` (mirror `pub use recipe_lookup::{RecipeLookup, RecipeLookupError, RecipeMatchDto};`) and from the crate root if `RecipeLookup` is re-exported there.

**`crates/brassclaw_turns/src/run_profile/host.rs`**:
- `AgentLoopHostErrorKind` (`:602–630`): add `Unimplemented,` variant (after `Internal,` or before it) + its `as_str` arm (`:632–651`, `"unimplemented"`). Then grep every exhaustive `match` on `AgentLoopHostErrorKind` and add the arm (or confirm `_` wildcard coverage). The plan's FIND-28 mandates the default `resolve_message_text` body returns `Err(Unimplemented)`, so this variant is required.
- After `NoRecipeLookup` (`:2088–2093`), add `LoopRetrievalPort` + `NoRetrieval` (mirror `LoopRecipePort`/`NoRecipeLookup`):
  ```rust
  pub trait LoopRetrievalPort: Send + Sync {
      fn retrieval_lookup(&self) -> Option<&dyn crate::run_profile::RetrievalLookup>;
  }
  pub struct NoRetrieval;
  impl LoopRetrievalPort for NoRetrieval {
      fn retrieval_lookup(&self) -> Option<&dyn crate::run_profile::RetrievalLookup> { None }
  }
  ```
- `AgentLoopDriverHost` supertrait (`:2185–2201`): add `+ LoopRetrievalPort` after `+ LoopRecipePort` (→ 14th port).
- Blanket impl where-clause (`:2204–2220`): add `+ LoopRetrievalPort` after `+ LoopRecipePort`.

**Five full-host implementors need `LoopRetrievalPort`** (the blanket `impl AgentLoopDriverHost for T where T: … + LoopRetrievalPort` now requires it — without it these types stop implementing `AgentLoopDriverHost` and won't compile as hosts):
- `RebornLoopDriverHost` (`crates/brassclaw_reborn/src/loop_driver_host.rs:1861`) — real impl delegating to a new field (see Step 3).
- `ResumePayloadHost` (`crates/brassclaw_agent_loop/src/executor/planned_driver.rs:813`, test) — `NoRetrieval`-style: `impl LoopRetrievalPort for ResumePayloadHost { fn retrieval_lookup(&self) -> Option<…> { None } }`.
- `RecordingAgentLoopHost` (`crates/brassclaw_turns/tests/agent_loop_host_contract.rs:2717`, test) — `None` accessor.
- `MockHost` (`crates/brassclaw_agent_loop/src/executor/tests/support.rs:748`, test) — `None` accessor.
- `MockAgentLoopDriverHost` (`crates/brassclaw_agent_loop/tests/test_support/mod.rs:882`, test) — `None` accessor.
(Grep `impl LoopRecipePort` across the repo to find any additional implementor and mirror it — do not rely only on the listed five.)

**Tests for Step 1**: `RetrievalTurnResult` serde round-trip; `NoRetrieval` returns `None`; a tiny host that impls `LoopRetrievalPort` returning `Some` via a stub `RetrievalLookup` proves the port is reachable from `AgentLoopDriverHost`. (`NoRetrieval`/stub `RetrievalLookup` are *test doubles*, not production stubs — the production impl is the real `PostgresSource`-backed one in Step 3.)

### Step 2 — H3: user-text plumbing (`resolve_message_text` + `last_user_text` + InputStage)

**`crates/brassclaw_turns/src/run_profile/message_text_resolver.rs`** (NEW — mirror `retrieval_lookup.rs`/`recipe_lookup.rs`):
- `#[async_trait] pub trait MessageTextResolver: Send + Sync { async fn resolve_message_text(&self, context: &LoopRunContext, message_ref: &LoopMessageRef) -> Result<Option<String>, AgentLoopHostError>; }` — turns-native; returns the **raw** accepted-message body (`Some(text)`) or `None` (no message recorded for that ref). Error is `AgentLoopHostError` (already turns-native).
- Re-export from `run_profile/mod.rs`.

**`crates/brassclaw_turns/src/run_profile/host.rs`**:
- `LoopContextPort` (`:778–784`): add a **default** method (mirror the no-op default discipline):
  ```rust
  async fn resolve_message_text(
      &self,
      _context: &LoopRunContext,
      _message_ref: &LoopMessageRef,
  ) -> Result<String, AgentLoopHostError> {
      Err(AgentLoopHostError::new(
          AgentLoopHostErrorKind::Unimplemented,
          "resolve_message_text not wired on this host",
      ))
  }
  ```
  Returns `Result<String, _>` (per plan H3 signature, `saved_plan_to_v3.md:5380–5386`). Default = `Err(Unimplemented)` per plan FIND-28. **All existing `LoopContextPort` impls inherit the default** — no churn to `ThreadBackedLoopContextPort`/`StaticLoopContextPort`/`PanicContextPort`/`StubContextPort`/`ForbiddenResumeHost`. Only `RebornLoopDriverHost` overrides (Step 3).

**`crates/brassclaw_agent_loop/src/state.rs`**:
- `LoopExecutionState` (`:47–103`): after `spawn_subagent_hint` (`:102`), add `#[serde(default)] pub last_user_text: Option<String>,`.
- `initial_for_run` (`:113–140`): add `last_user_text: None,` before the closing brace.
- (Also add a minimal stash field for Step 4: `#[serde(default)] pub last_retrieval_result: Option<brassclaw_turns::run_profile::RetrievalTurnResult>,` + `last_retrieval_result: None,` in `initial_for_run`. This is E.0's own durable stash — Phase H will introduce the plan's `recipe_hint`/`recipe_rust_context` split and migrate/split this. Keeping it avoids a "fire-and-discard" result that would read as a stub.)

**`crates/brassclaw_agent_loop/src/executor/input.rs`**:
- `consume_drainable_inputs` (`:154–211`): currently returns `(bool, Vec<LoopInputAckToken>, Option<LoopCancelledReasonKind>)`. Per plan FIND-P5-04, change to a 4-tuple adding the last consumed **user-facing** `message_ref`: `(bool, Vec<LoopInputAckToken>, Option<LoopCancelledReasonKind>, Option<LoopMessageRef>)`. In the drain-mode arm (`:170–174`), bind `LoopInput::UserMessage { message_ref }` / `FollowUp { message_ref }` / `Steering { message_ref }` and capture the last one. (Need to read the exact `LoopInput` enum shapes at `host.rs:841–850` to bind `message_ref` correctly.)
- `drain` (`:130–152`): destructure the new 4th element as `last_message_ref`; if `Some`, call `ctx.host.resolve_message_text(&run_context, &last_message_ref).await` (using the `LoopRunContext` available on `ctx` via `ctx.host.run_context()`), and on `Ok(text)` store `state.last_user_text = Some(text)`. On `Err`/`None`, leave `last_user_text = None` and `debug!`-log (Tier-2 still works). Update `DrainedInputs` consumers (`drain_initial`/`drain_followup`) for the new tuple arity.

**`crates/brassclaw_first_party_extension_ports/src/activation.rs`**:
- `SelectableSkillContextSource` (`:190–199`): `messages_by_run` (`:195`) is `Mutex<HashMap<SkillActivationMessageKey, SkillActivationMessage>>`; key/value (`:709–728`) are **private**; only read accessor `take_message_for_run` (`:365–378`) is `pub(crate)` and **consuming**. **Add a non-consuming public read accessor**:
  ```rust
  /// Non-consuming read of the raw accepted-message text recorded for
  /// `(scope, accepted_message_ref)`. Returns `None` when no message is
  /// recorded (already taken or never written). Does NOT remove the entry.
  pub fn peek_message_text(
      &self,
      scope: &TurnScope,
      accepted_message_ref: &AcceptedMessageRef,
  ) -> Result<Option<String>, SkillActivationSelectionError> {
      Ok(self
          .messages_by_run
          .lock()
          .map_err(|_| SkillActivationSelectionError::Internal)?
          .get(&SkillActivationMessageKey::new(scope.clone(), accepted_message_ref.clone()))
          .map(|m| m.text.clone()))
  }
  ```
  This is the **raw** text (`SkillActivationMessage.text`), NOT the sanitized `safe_summary` — exactly what plan H3 (`saved_plan_to_v3.md:5387–5390`) mandates so intent matching is not corrupted by `[redacted]`.

**Tests for Step 2**: `peek_message_text` returns the recorded raw text and is non-consuming (a second call returns the same text, unlike `take_message_for_run`); a host with a stub `MessageTextResolver` returns `Some` text via `resolve_message_text`; `last_user_text` round-trips through serde in a checkpoint; an `InputStage`-driven drain populates `state.last_user_text` from the resolver (integration-tier test through the actual `drain` call site, per AGENTS.md "test through the caller").

### Step 3 — composition: `RetrievalLookup` impl (PostgresSource-backed) + `MessageTextResolver` impl (messages_by_run-backed) + host wiring

**`crates/brassclaw_reborn/src/loop_driver_host.rs`** (mirror the `recipe_lookup` 5-point pattern exactly):
- `RebornLoopDriverHostFactory` field (`:964` area): `retrieval_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RetrievalLookup>>,` + `message_text_resolver: Option<Arc<dyn brassclaw_turns::run_profile::MessageTextResolver>>,`.
- Factory default (`:1049` area): `retrieval_lookup: None,` + `message_text_resolver: None,`.
- Builders (mirror `with_recipe_lookup` `:1364–1370`): `with_retrieval_lookup(mut self, lookup: Arc<dyn RetrievalLookup>) -> Self { self.retrieval_lookup = Some(lookup); self }` + `with_message_text_resolver(mut self, resolver: Arc<dyn MessageTextResolver>) -> Self { self.message_text_resolver = Some(resolver); self }`.
- Per-build clone (`:1742` area): `retrieval_lookup: self.retrieval_lookup.clone(),` + `message_text_resolver: self.message_text_resolver.clone(),`.
- `RebornLoopDriverHost` struct fields (`:1817` area): `retrieval_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RetrievalLookup>>,` + `message_text_resolver: Option<Arc<dyn brassclaw_turns::run_profile::MessageTextResolver>>,`.
- `impl LoopRetrievalPort for RebornLoopDriverHost` (after `impl LoopRecipePort` `:1861–1869`): `fn retrieval_lookup(&self) -> Option<&dyn brassclaw_turns::run_profile::RetrievalLookup> { self.retrieval_lookup.as_deref() }`.
- `impl LoopContextPort for RebornLoopDriverHost` (`:2233–2240`): **override** `resolve_message_text` to use `self.message_text_resolver`:
  ```rust
  async fn resolve_message_text(
      &self,
      context: &LoopRunContext,
      message_ref: &LoopMessageRef,
  ) -> Result<String, AgentLoopHostError> {
      match &self.message_text_resolver {
          Some(resolver) => match resolver.resolve_message_text(context, message_ref).await {
              Ok(Some(text)) => Ok(text),
              Ok(None) => Err(AgentLoopHostError::new(
                  AgentLoopHostErrorKind::Unimplemented,
                  "no raw message text recorded for this message_ref",
              )),
              Err(e) => Err(e),
          },
          None => Err(AgentLoopHostError::new(
              AgentLoopHostErrorKind::Unimplemented,
              "resolve_message_text not wired on this host",
          )),
      }
  }
  ```
  (Note: `RebornLoopDriverHost::load_loop_context` still delegates to `self.context`; only `resolve_message_text` is overridden to use the host's own resolver — because `self.context` (`ThreadBackedLoopContextPort`) cannot reach `messages_by_run`.)

**`crates/brassclaw_reborn/src/runtime.rs`** (mirror the `recipe_lookup` wiring):
- `DefaultPlannedRuntimeParts` (`:155–181`): after `recipe_lookup` (`:159`), add `pub retrieval_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RetrievalLookup>>,` + `pub message_text_resolver: Option<Arc<dyn brassclaw_turns::run_profile::MessageTextResolver>>,`.
- `build_default_planned_runtime` (`:578–579`): mirror the `if let Some(lookup) = parts.recipe_lookup { host_factory = host_factory.with_recipe_lookup(lookup); }` block for `retrieval_lookup` → `with_retrieval_lookup` and `message_text_resolver` → `with_message_text_resolver`.
- Update every construction site of `DefaultPlannedRuntimeParts` (grep `DefaultPlannedRuntimeParts {` / `..DefaultPlannedRuntimeParts::default()`) to initialize the two new fields (`None` for test/non-composition sites, or rely on `Default` if the struct derives it — verify whether `DefaultPlannedRuntimeParts` has a manual `Default` and extend it).

**`crates/brassclaw_reborn_composition/src/...`** (composition supplies the engine-backed impls + wires them):
- NEW file (e.g. `crates/brassclaw_reborn_composition/src/retrieval_lookup_impl.rs`): `#[cfg(feature = "skills-db")] impl brassclaw_turns::run_profile::RetrievalLookup for PgRetrievalLookup { … }` where `PgRetrievalLookup { source: Arc<brassclaw_engine::memory::PostgresSource> }`. `fetch_for_turn` builds a `ComponentScope` from `context.scope` (tenant/user/agent/project), calls `self.source.fetch_for_turn(scope, query, token_budget, sender_class_code).await`, maps `FetchForTurnResult::Components(items)` → `Some(RetrievalTurnResult { tier0_eligible: false, llm_call_required: true, rust_items: serde_json::to_value(&items)?, orchestrator_items: serde_json::to_value(&items)?, routing_meta: json!({"variant":"components","count":items.len()}) })`, `Disambiguation(cands)` → `Some(RetrievalTurnResult { … orchestrator_items: json!(cands), routing_meta: json!({"variant":"disambiguation"}) … })`, errors → `RetrievalLookupError`. (E.0 conservative booleans: no Tier-0 short-circuit. `rust_items` == `orchestrator_items` until Phase E `SplitResult` does the real split.) Export from `lib.rs` / `mod.rs` (mirror how `pg_recipe_store::PgRecipeLibrary` is exported).
- NEW impl of `MessageTextResolver` (e.g. `SkillActivationMessageTextResolver { source: Arc<LocalDevSelectableSkillContextSource> }`) in composition: `resolve_message_text` extracts `scope` + `accepted_message_ref` from the `LoopRunContext`/`LoopMessageRef` and calls `source.peek_message_text(&scope, &accepted_message_ref)`. (Need to map `LoopMessageRef` → `AcceptedMessageRef` — verify the relationship at `ids.rs:242` and the `AcceptedMessageRef` type.)
- Composition `runtime.rs`: build `retrieval_lookup` (`#[cfg(feature = "skills-db")]`, from `services.pg_pool` — mirror `recipe_lookup` at `:2533–2543` but gate on `skills-db` not `postgres`), and `message_text_resolver` (from `skill_activation_source`, the `Option<Arc<LocalDevSelectableSkillContextSource>>` at `:233`). Pass both into `DefaultPlannedRuntimeParts { …, retrieval_lookup, message_text_resolver, … }` at `:2646`.

**Feature-gating**: `PostgresSource` is behind `brassclaw_engine/skills-db`; composition enables that via its own `skills-db` feature (`Cargo.toml:23`), which is **NOT** in composition's default `["postgres","root-llm-provider"]` (`:18`). So the `RetrievalLookup` impl + `retrieval_lookup` wiring are `#[cfg(feature = "skills-db")]`; when off, `retrieval_lookup = None` → `RecipeStage` falls through to Tier-2 (correct explicit behaviour). `MessageTextResolver` is NOT engine-gated (`SelectableSkillContextSource` is always available) — wire it unconditionally (or `#[cfg(feature = "postgres")]` if it needs the runtime to be DB-backed; decide at implementation, prefer unconditional since raw-text resolution doesn't need PG).

**Tests for Step 3**: `PgRetrievalLookup::fetch_for_turn` maps `Components`/`Disambiguation` correctly (use the existing composition test PgRig from `tests/common/mod.rs` to seed a component row and assert the serialized `RetrievalTurnResult`); `SkillActivationMessageTextResolver` returns the recorded raw text; an integration test boots the runtime with a `pg_pool` + `skill_activation_source` and asserts the host's `retrieval_lookup()` is `Some` (a `PostgresSource`-backed impl) and `resolve_message_text` returns raw text for a recorded message — this is the E.0 acceptance test's core (run in CI when Docker/PG available; skip-cleanly here).

### Step 4 — `RecipeStage` fires `fetch_for_turn` in live turns

**`crates/brassclaw_agent_loop/src/executor/recipe.rs`**:
- Rewrite `RecipeStage::process` (`:55–84`) to actually consult retrieval when `last_user_text` is present:
  - If `ctx.host.retrieval_lookup()` is `Some(lookup)` AND `input.state.last_user_text` is `Some(user_text)`:
    - Call `lookup.fetch_for_turn(ctx.host.run_context(), &user_text, token_budget, "02").await` (`token_budget` = a constant for now, e.g. the same budget the prompt stage uses; Phase E refines). Map `Ok(Some(result))` → stash into `state.last_retrieval_result = Some(result)`; `debug!`-log the routing meta. `Ok(None)` / `Err` → `debug!`-log and leave `None`.
    - Return `RecipeStep::Continue { state }` (Tier-2 fall-through — Tier-0/Tier-1 dispatch is Phase H's consumer, deliberately NOT implemented here).
  - If no lookup or no user text: `debug!`-log "no retrieval / no user text — Tier-2" and `Continue`.
- This is **not a stub**: `PostgresSource::fetch_for_turn` + `resolve_intent` genuinely run against the DB in a live turn (E.0 reachability achieved), and the result is durably stashed for Phase H's consumer.

**Tests for Step 4**: an `agent_loop` integration test driving `RecipeStage::process` through `StageContext` with a stub `RetrievalLookup` returning `Some(RetrievalTurnResult{…})` + a `last_user_text`-populated state asserts: (a) `fetch_for_turn` was called with the user text + `"02"`, (b) `state.last_retrieval_result` is `Some`, (c) the stage still returns `Continue` (Tier-2 preserved). Mock must capture every arg the production caller passes (AGENTS.md). This test runs without PG (stub lookup), so it executes here.

### Step 5 — verify, accept, commit, push

- `cargo fmt` (all touched crates).
- `cargo clippy -p brassclaw_turns -p brassclaw_agent_loop -p brassclaw_reborn -p brassclaw_reborn_composition -p brassclaw_first_party_extension_ports -p brassclaw_loop_support --all-targets -- -D warnings` (zero warnings). Fix every `unreachable_pub`/`too_many_arguments`/unused-import surfacing from the new code.
- `cargo test -p brassclaw_turns -p brassclaw_agent_loop -p brassclaw_first_party_extension_ports` (runs here; the composition PG integration tests skip-cleanly here, run in CI).
- **Acceptance (E.0 goal — fetch_for_turn fires in a LIVE agent-loop turn)**: the Step 4 integration test proves the call fires through the real `StageContext` → `ctx.host.retrieval_lookup()` path; the Step 3 composition integration test (CI) proves the production host's `retrieval_lookup()` is a `PostgresSource`-backed `RetrievalLookup` and returns a real `RetrievalTurnResult` for a seeded component. Both together satisfy "PostgresSource::fetch_for_turn is reachable from a live production turn" — the agent-loop adaptation of the plan's original E.0 acceptance (`saved_plan_to_v3.md:4660–4691`).
- Review the full diff (security: no new unsanitized-text leakage beyond the existing raw-text store; no auth/secret/sandbox regressions; the raw text is already host-side in `messages_by_run`, only now readable via a non-consuming accessor). Update `CHANGELOG.md` if the project keeps one for this layer.
- Commit (scoped to E.0) + push to `origin/main`. Then mark the Zenflow E0 substep Completed and resume the parent E.0 step.

---

## 4. What this E0-A deliberately does NOT do (later phases own it)

- **Phase E** upgrades `PostgresSource::fetch_for_turn` to return `FetchForTurnResult::SplitResult`, splits `rust_items`/`orchestrator_items`, and populates the real `tier0_eligible`/`llm_call_required` routing booleans (from `SplitResult`/`ActionShortCircuit`). E0-A's composition mapping uses conservative booleans + unsplit items, clearly marked for Phase E to replace.
- **Phase H** adds the **consumer**: `LoopOrchestratorPort` (H5) + `TierZeroExecutionStage` read `state.last_retrieval_result` (migrated to the plan's `recipe_hint`/`recipe_rust_context`) and perform Tier-0/Tier-1 dispatch; `run_step_zero`/`run_tier_zero` + `assemble_prior_knowledge_with_hint`/`execute_tier_zero_channel` engine library functions; `PriorKnowledgeBundle`/`TierZeroReply` turns-native types. E0-A only **produces** the retrieval result and stashes it.
- **Phases F/G/K.3** retire the dormant engine Python `ExecutionLoop`/`default.py`/`RamSource` (deletion — NOT touched now; `RamSource` stays importable, the `TODO(Phase K)` at `manager.rs:377` stays since that path is dormant/test-only).

---

## 5. Open verification items resolved at implementation time (not design questions)

These are mechanical confirmations, not user design decisions:
- Exact `LoopInput::UserMessage`/`FollowUp`/`Steering` field names for binding `message_ref` (`host.rs:841–850`).
- `LoopMessageRef` ↔ `AcceptedMessageRef` mapping (`ids.rs:242`).
- Whether `DefaultPlannedRuntimeParts` has a manual `Default` to extend (grep `impl Default for DefaultPlannedRuntimeParts`).
- Exhaustive `match`es on `AgentLoopHostErrorKind` that need the new `Unimplemented` arm (grep `AgentLoopHostErrorKind::` match sites).
- Any additional `impl LoopRecipePort`/`impl LoopContextPort` sites beyond the listed ones (grep both).
- Composition export surface for the new impls (mirror `pg_recipe_store` export path).
