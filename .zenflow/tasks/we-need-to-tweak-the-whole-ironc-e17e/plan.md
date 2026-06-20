# Full SDD workflow

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: ea413754-89cc-4547-919d-4a0f1f88877b -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: c12a87aa-f6ea-4ba0-b007-809ada495926 -->

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: 6722fd65-ee1c-44f2-af96-71acf599248d -->

Create a detailed implementation plan based on `{@artifacts_path}/spec.md`.

1. Break down the work into concrete tasks
2. Each task should reference relevant contracts and include verification steps
3. Replace the Implementation step below with the planned tasks

Rule of thumb for step size: each step should represent a coherent unit of work (e.g., implement a component, add an API endpoint). Avoid steps that are too granular (single function) or too broad (entire feature).

Important: unit tests must be part of each implementation task, not separate tasks. Each task should implement the code and its tests together, if relevant.

If the feature is trivial and doesn't warrant full specification, update this workflow to remove unnecessary steps and explain the reasoning to the user.

Save to `{@artifacts_path}/plan.md`.

---

## Implementation Steps

> **Ordering rationale (spec §3.1):** The spec prescribes stripping v1 branches from shared files _before_ deleting v1-only files, to avoid dangling references inside `if engine_v2 { ... } else { v1_code }` branches. Steps 1-4 follow this stage order.

### [x] Step 1: Strip v1 branches from modified source files and remove ENGINE_V2 gate
<!-- chat-id: 249ca68f-9fe2-45c8-bcc1-6f33990bb58e -->

Modify existing source files to remove v1 code paths and the `ENGINE_V2` environment gate, as detailed in spec.md §2.1 "Modify" table and §3.1 stage order (stages 1-2). This must be done **before** deleting v1-only files to avoid compile errors from dangling references inside v1 branches.

Changes:
- `src/bridge/router.rs`: remove `is_engine_v2_enabled()`, its `ENGINE_V2` env check, and all `if engine_v2 { ... } else { ... }` branches in `handle_message()`. Remove v1 fallback paths
- `src/config/agent.rs`: remove `engine_v2: bool` field and `ENGINE_V2` env parse. Update `AgentConfig::for_testing()` to remove `engine_v2: false`. Update `AgentConfig::resolve()` to remove the `engine_v2` resolution
- `src/agent/agent_loop.rs`: remove all `if self.config.engine_v2` branches and the v1 code inside them
- `src/agent/thread_ops.rs`: remove v1 approval processing path from `process_approval()`, keep v2 path
- `src/gate/approval.rs`: remove v1/v2 inconsistency comment; unify auto-deny logic
- `src/channels/web/features/status/mod.rs`: remove `engine_v2_enabled: bool` from status response
- `src/app.rs`: remove `is_engine_v2_enabled()` call and conditional engine version string in startup log

Verification: `cargo check --workspace` compiles; no remaining reference to `engine_v2` field or `is_engine_v2_enabled`.

### [x] Step 2: Remove v1 agent execution path (files and modules)
<!-- chat-id: a36f3dd5-87b5-4116-b064-83bf123a864d -->

Delete all v1-only source files and directories listed in spec.md §2.1 (stage 3), then remove any dead `use` imports and `mod` declarations from parent modules.

Files and directories to delete:
- `src/agent/dispatcher.rs`
- `src/agent/session.rs`
- `src/agent/session_manager.rs`
- `src/agent/routine.rs`
- `src/agent/routine_engine.rs`
- `src/agent/agentic_loop.rs`
- `src/agent/scheduler.rs`
- `src/agent/submission.rs`
- `src/agent/job_monitor.rs`
- `src/agent/self_repair.rs`
- `src/agent/compaction.rs`
- `src/agent/context_monitor.rs`
- `src/agent/cost_guard.rs`
- `src/agent/undo.rs`
- `src/orchestrator/` (entire directory)
- `src/worker/job.rs`, `src/worker/acp_bridge.rs`, `src/worker/claude_bridge.rs`
- `src/hooks/` (entire directory)
- `src/skills/attenuation.rs`
- `src/context/` (entire directory)
- `src/history/` (entire directory)
- `src/evaluation/` (entire directory)
- `src/estimation/` (entire directory)

After deletion: remove all `mod` declarations and `use` imports referencing these modules from `src/agent/mod.rs`, `src/lib.rs`, `src/worker/mod.rs`, `src/skills/mod.rs`, and any other parent modules. Fix all resulting compile errors by removing dead code references.

Verification: `cargo check --workspace` compiles without errors related to these deleted modules.

### [x] Step 3: Remove online skill catalog (`SkillCatalog`)
<!-- chat-id: 30d581e5-3f68-45ec-8656-7b55d4612495 -->

Remove the `SkillCatalog` runtime dependency as described in spec.md §2.1 "Online skill catalog removal".

Changes:
- Delete `crates/brassclaw_skills/src/catalog.rs` entirely
- `src/app.rs`: remove `skill_catalog: Option<Arc<SkillCatalog>>` from `AppComponents`, remove `SkillCatalog` construction in `build_components()`, remove `.with_skill_catalog()` calls
- `src/channels/web/mod.rs`: remove `skill_catalog` field from `GatewayState`, remove `with_skill_catalog()` method, remove the `use brassclaw_skills::catalog::SkillCatalog` import
- `src/channels/web/handlers/skills.rs`: remove the catalog-based skill download branch; `list_catalog` handler returns empty JSON array with HTTP 200; `install_skill` retains only the direct-URL install path
- `src/cli/skills.rs`: remove `SkillCatalog::new()` usage and the `search`/`browse` subcommands
- Remove `skill_catalog: None` from all test helper construction sites: `src/channels/web/test_helpers.rs`, `src/testing/mod.rs`, test blocks in `src/agent/thread_ops.rs`, `src/channels/web/tests/multi_tenant.rs`

Verification: `cargo check --workspace` must compile with no reference to `SkillCatalog` or `catalog.rs`.

### [x] Step 4: Remove online-only registry entries and update bundle
<!-- chat-id: 1b0c0003-3076-464d-bd0e-76e7b59c63d5 -->

Remove online-only tool and MCP-server registry JSON files, and update `_bundles.json` as described in spec.md §2.1 (stage 4).

Registry files to delete:
- `registry/tools/web_search.json`
- `registry/tools/composio.json`
- `registry/tools/gmail.json`
- `registry/tools/google_calendar.json`
- `registry/tools/google_docs.json`
- `registry/tools/google_drive.json`
- `registry/tools/google_sheets.json`
- `registry/tools/google_slides.json`
- `registry/tools/slack_tool.json`
- `registry/tools/telegram_mtproto.json`
- `registry/tools/portfolio.json`
- `registry/tools/llm_context.json`
- `registry/mcp-servers/asana.json`
- `registry/mcp-servers/notion.json`
- `registry/mcp-servers/linear.json`
- `registry/mcp-servers/stripe.json`
- `registry/mcp-servers/intercom.json`
- `registry/mcp-servers/sentry.json`
- `registry/mcp-servers/nearai.json`
- `registry/mcp-servers/cloudflare.json`

Update `registry/_bundles.json`:
- Remove the `default` bundle
- Add a new `home` bundle containing only the new local tools: `browser` (playwright MCP), `caldav`, `notes`, `local-search`

Verification: `cargo check --workspace` passes; no remaining reference to removed registry entries in Rust source. JSON files are valid.

### [x] Step 5: Update settings and data types for token budget and local backend
<!-- chat-id: 443723d2-3c57-48bc-ba88-7162a38d55d9 -->

Add new fields to settings structs and engine types as specified in spec.md §2.1 modify table, §3.2.1, and §4.1–§4.4.

Changes to `src/settings.rs`:
- Add `is_local: bool` (with `#[serde(default)]`) to `CustomLlmProviderSettings`
- Add `max_prompt_tokens: usize`, `plan_confidence_threshold: f64`, `codeact_enabled: Option<bool>` to `AgentSettings` with corresponding `default_*` functions and `#[serde(default = "...")]` attributes
- **Update `impl Default for AgentSettings`** to include all three new fields: `max_prompt_tokens: default_max_prompt_tokens()`, `plan_confidence_threshold: default_plan_confidence_threshold()`, `codeact_enabled: None`. Without this, the exhaustive constructor causes a compile error (spec §3.2.1 warning)
- Lower `default_skills_max_context_tokens()` from `4000` to `2048` (global skill budget ceiling)
- Add `LocalSearchSettings` struct with field `allow_global_scope: bool` (default `false`) per spec §4.6, and `local_search: LocalSearchSettings` field to top-level `Settings` with `#[serde(default)]`

Changes to `crates/brassclaw_skills/src/types.rs`:
- Change `default_max_context_tokens()` from `2000` to `0` (per-skill declared budget default — spec §3.4.3). This is distinct from `default_skills_max_context_tokens()` in `settings.rs` which is the global ceiling

Changes to `src/config/agent.rs`:
- Add `max_prompt_tokens: usize`, `plan_confidence_threshold: f64`, `codeact_enabled: Option<bool>` to `AgentConfig`
- Update `AgentConfig::for_testing()` to add the three new fields with defaults (`max_prompt_tokens: 8192`, `plan_confidence_threshold: 0.6`, `codeact_enabled: None`)
- Update `AgentConfig::resolve()` to resolve new fields from `AgentSettings` via `db_first_or_default`

Changes to `crates/brassclaw_engine/src/types/thread.rs`:
- Add 5 new fields to `ThreadConfig`: `max_prompt_tokens: usize`, `skill_token_budget: usize`, `codeact_enabled: Option<bool>`, `decomposition_depth: u8`, `plan_confidence_threshold: f64`
- All with `#[serde(default = "...")]` using named default functions per spec §4.1. Extend `impl Default for ThreadConfig` to include all five with defaults (8192, 2048, None, 0, 0.6)

Changes to `crates/brassclaw_engine/src/executor/orchestrator.rs` (config dict — **required alongside `ThreadConfig` changes**):
- Extend the `config` JSON dict at lines 2720–2736 to include all five new fields: `"max_prompt_tokens"`, `"skill_token_budget"`, `"codeact_enabled"`, `"decomposition_depth"`, `"plan_confidence_threshold"`. This dict is an explicit, manually-constructed list — it does **not** automatically reflect new `ThreadConfig` fields. Without this change, `config.get("skill_token_budget", 2048)` and all other Python reads of the new fields will always return the Python fallback defaults, silently ignoring any user-configured DB values.

Changes to `src/config/skills.rs`:
- Update the hardcoded `max_context_tokens: 6000` in `SkillsConfig::default()` (line 45) to `2048`. `SkillsConfig::default()` is a separate struct from `SkillsSettings`; lowering `default_skills_max_context_tokens()` in `settings.rs` does not affect this value. Tests using `SkillsConfig::default()` directly (in `router.rs`, `agent_loop.rs`, `thread_ops.rs`, `testing/mod.rs`) will otherwise silently use a 6,000-token skill budget — 3× the home-use target — making them unrepresentative of production behavior. Also update the comment on line 37 from `6000` to `2048`.

Changes to `crates/brassclaw_engine/src/types/mission.rs`:
- Add `Idle { threshold_secs: u64 }` variant to `MissionCadence`

Changes to `crates/brassclaw_engine/src/types/capability.rs`:
- Add `Syncing` variant to `CapabilityStatus`

Changes to `crates/brassclaw_engine/src/executor/prompt.rs` (`capability_status_label`):
- Add `CapabilityStatus::Syncing => "syncing"` arm

Changes to `src/bridge/tool_surface.rs` (`fallback_assignment`):
- Add `CapabilityStatus::Syncing` arm → `SurfaceAssignment::capabilities_only()`

Changes to `src/bridge/action_projector.rs` (`provider_extension_rank`):
- Add `CapabilityStatus::Syncing` arm → rank `2`

Changes to `crates/brassclaw_engine/src/traits/llm.rs`:
- Add `pub is_planning_call: bool` to `LlmCallConfig`

Create `migrations/V28__prompt_token_ceiling.sql`:
- Insert default DB settings row for `agent.max_prompt_tokens = 8192` (spec §3.2.1)
- Update `migrations/checksums.lock`

Verification: `cargo check --workspace` compiles; all exhaustive match arms satisfied.

### [x] Step 6: Add local-backend detection and loopback auto-detection
<!-- chat-id: 395c3c22-4621-42fa-893f-61e16b12a340 -->

Implement `is_local` flag propagation and loopback auto-detection as described in spec.md §3.3.1.

Changes:
- `src/channels/web/features/settings/mod.rs`: add `infer_is_local(base_url: Option<&str>, adapter: &str) -> bool` helper. Apply it server-side on the GET endpoint that returns provider registration form defaults to pre-populate the "Local model" checkbox. POST handler accepts explicit `is_local` from the submitted payload without re-running inference
- `src/bridge/llm_adapter.rs`: populate `PlatformInfo.is_local_backend` from the active provider's `is_local` flag. Add `pub is_local_backend: bool` to `PlatformInfo` in `crates/brassclaw_engine/src/executor/prompt.rs`
- Ollama built-in provider record always has `is_local: true`

Unit tests:
- Test `infer_is_local()` for ollama adapter, loopback URLs (localhost, 127.0.0.1, [::1]), and non-local URLs

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 7: Implement Tier 0 system prompt and `should_use_tier0` logic
<!-- chat-id: 2882037b-600a-4178-a6de-3ec8ee098df9 -->

Create the Tier 0 compact system prompt infrastructure as described in spec.md §3.3.2–§3.3.3.

New files:
- `crates/brassclaw_engine/prompts/tier0_system_prompt.md`: compact prompt ≤ 800 tokens static content, with `{plan_anchor}` placeholder. Must begin with `<!-- brassclaw:tier0-system-prompt -->` marker. Plain language, no Python, no CodeAct
- `crates/brassclaw_engine/src/executor/tier0_prompt.rs`: Rust module exporting `pub fn should_use_tier0(platform: Option<&PlatformInfo>, codeact_override: Option<bool>) -> bool` and `pub fn build_tier0_system_prompt(platform: Option<&PlatformInfo>, plan_anchor: Option<&str>) -> String`. The function reads `tier0_system_prompt.md` (embedded via `include_str!`), renders `{plan_anchor}` substitution, prepends `TIER0_SYSTEM_PROMPT_MARKER`

Changes to `crates/brassclaw_engine/src/executor/prompt.rs`:
- Apply 60-word cap in `compact_prompt_description()` for tool schema trimming
- Add `pub const TIER0_SYSTEM_PROMPT_MARKER: &str = "<!-- brassclaw:tier0-system-prompt -->\n";`
- Add `pub fn is_engine_system_prompt(content: &str) -> bool` — returns true for CodeAct marker, Tier 0 marker, or legacy CodeAct
- Add `pub fn refresh_engine_system_prompt(existing_content: &str, new_prompt: &str) -> String` — delegates to CodeAct-specific refresh for CodeAct prompts, full replacement for Tier 0
- Add `pub fn upsert_engine_system_prompt(messages: &mut Vec<ThreadMessage>, system_prompt: String) -> bool`

Changes to `crates/brassclaw_engine/src/executor/loop_engine.rs`:
- In `refresh_system_prompt()`: add `should_use_tier0()` branch replacing the unconditional `build_codeact_system_prompt_with_docs()` call; read `plan_steps`/`plan_current_step` from `checkpoint.persisted_state`; construct `PlanAnchor`; write `plan_anchor_text` back to `persisted_state` (empty string for CodeAct, actual text for Tier 0 — see spec §3.3.3)
- Update all call sites listed in spec.md §3.3.5 to use `*_engine_*` variants:
  - [ ] `RuntimeCheckpoint::has_working_messages_system_prompt()` (line 43): `is_codeact_system_prompt` → `is_engine_system_prompt`
  - [ ] `RuntimeCheckpoint::update_working_messages_system_prompt()` (line 61): `is_codeact_system_prompt` → `is_engine_system_prompt`, `refresh_codeact_system_prompt` → `refresh_engine_system_prompt`
  - [ ] `has_engine_owned_system_prompt()` (line 241): `is_codeact_system_prompt` → `is_engine_system_prompt`
  - [ ] `refresh_system_prompt()` (lines 305-322): `build_codeact_system_prompt_with_docs` → tier0 branch, `upsert_codeact_system_prompt` → `upsert_engine_system_prompt` at both call sites

Update `crates/brassclaw_engine/src/executor/mod.rs` to declare `pub mod tier0_prompt;`

Unit tests:
- `build_tier0_system_prompt(None, None)` compiles and returns a string containing the marker
- `build_tier0_system_prompt(None, Some("Step 1"))` includes plan anchor
- `is_engine_system_prompt()` returns true for both CodeAct and Tier 0 markers, false for arbitrary text
- `upsert_engine_system_prompt()` correctly replaces a Tier 0 system message on update

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 8: Implement `TokenGuard` and `__apply_token_guard__` host function
<!-- chat-id: f223b9ff-0ce3-4148-b89f-53c4c9eaf66b -->

Implement the token budget enforcement system as described in spec.md §3.2.

New file: `crates/brassclaw_engine/src/executor/token_guard.rs`
- Define `PromptBudget { total: usize, system_prompt_reserved: usize, skill_budget: usize, memory_doc_budget: usize, tool_schema_budget: usize }`
- Define `PromptParts { system_prompt: String, skills: Vec<ScoredItem>, memory_docs: Vec<ScoredItem>, tool_schemas: Vec<ScoredItem>, history: Vec<HistoryMessage>, plan_anchor_text: String }`
- Define `DroppedItems { memory_docs: usize, skills: usize, tool_descriptions_truncated: usize, history_messages: usize }`
- Implement `pub fn apply(budget: &PromptBudget, parts: &mut PromptParts) -> DroppedItems` using the priority-order degradation from REQ-2.4: drop lowest-scoring memory docs (exclude `DocType::Plan`) → drop lowest-scoring skills → truncate dynamically-registered tool descriptions to 60 words → remove system prompt droppable sections (`<!-- droppable-start -->` / `<!-- droppable-end -->`) → drop oldest history messages (never drop latest user message or plan anchor). Token counting uses byte-based approximation: `(bytes * 0.25) as usize`
- Return `{"dropped": [...], "fits": bool}` — `fits == false` signals decomposition trigger at step 0
- Add helper `fn token_count(text: &str) -> usize`

Changes to `crates/brassclaw_engine/src/executor/orchestrator.rs`:
- Add host function handler for `__apply_token_guard__`: receives `PromptParts` dict from Python, applies `TokenGuard::apply()`, returns dropped item list and `fits` boolean
- Extend `handle_retrieve_docs` to include `doc_id` and `metadata` in the returned dict (in addition to existing `type`, `title`, `content` keys) — required for plan retrieval helpers
- Add host function handler for `__save_plan_doc__` (goal, steps, is_decomposition — all positional args, no kwargs per spec §3.5.2 note) — creates a `DocType::Plan` MemoryDoc and persists it via the store. Returns doc ID string
- Extend `handle_llm_complete` to read `is_planning_call` from the Python config dict and pass it to `LlmCallConfig`
- Apply 60-word truncation to `a.description` in `handle_get_actions()` before serializing to the Python `actions_json` list (spec §3.4.4). The current code passes `a.description` raw; apply the same word-split/join logic as `compact_prompt_description()` in `prompt.rs`. Without this, `__get_actions__()` returns full-length tool descriptions to Python, bypassing the 60-word cap for the Python tool schema path even after the Rust prompt-building path is fixed in Step 7
- Update the `//!` module docstring to list `__apply_token_guard__` and `__save_plan_doc__`

Changes to Python `crates/brassclaw_engine/orchestrator/default.py`:
- Add `__apply_token_guard__` and `__save_plan_doc__` to the declared host functions header comment
- Add `_token_count(text: str) -> int` module-level helper using `int(len(text.encode("utf-8")) * 0.25)`

> **Note:** The actual `run_loop()` integration of `__apply_token_guard__` is deferred to Step 11, because the step-0 decomposition trigger path (depth guard → `run_miniplan_call`) depends on planning functions created in Step 11. See Step 11's `__apply_token_guard__` substep for full integration details.

Update `crates/brassclaw_engine/src/executor/mod.rs` to declare `pub mod token_guard;`

Unit tests for `TokenGuard`:
- Budget exactly satisfied → no drops
- Memory docs dropped when over budget
- Skill dropped when over budget after memory doc drop
- Plan anchor never dropped even when over budget
- `DocType::Plan` memory docs excluded from drop candidates
- Droppable system prompt sections removed when over budget

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 9: Implement `PlanAnchor` and `planner.rs` (Rust side)
<!-- chat-id: 0e4818f7-ba08-4ef2-b120-230d40bde3bd -->

Add the `PlanAnchor` struct and plan-side Rust support as described in spec.md §3.5.1–§3.5.3.

> **Note:** This creates `crates/brassclaw_engine/src/executor/planner.rs` — distinct from the existing `crates/brassclaw_engine/src/capability/planner.rs` (`LeasePlanner`). The two are in different module paths and serve different purposes.

New file: `crates/brassclaw_engine/src/executor/planner.rs`
- `pub struct PlanAnchor { pub steps: Vec<String>, pub current_step: usize }`
- `impl PlanAnchor { pub fn to_prompt_section(&self) -> String }` — formats the plan as a numbered list with current step highlighted, summary-truncated to ≤ 200 tokens using the byte-based approximation. Format: "## Current Plan\n1. step one\n**→ 2. step two** (current)\n3. step three"

Update `crates/brassclaw_engine/src/executor/mod.rs` to declare `pub mod planner;`

Unit tests:
- `to_prompt_section()` output stays ≤ 200 tokens with 10 steps
- `to_prompt_section()` output stays ≤ 200 tokens even with 20 long steps (verifies truncation)
- Current step is highlighted correctly

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 10: Implement plan confidence tracking (`skill_tracker.rs` + `default.py`)
<!-- chat-id: d3aa1b82-7a92-4f47-aae6-99aa460eebf5 -->

Extend `SkillTracker::record_usage()` to accept `DocType::Plan` docs and implement plan tracking call in the Python orchestrator as described in spec.md §3.5.4.

Changes to `crates/brassclaw_engine/src/memory/skill_tracker.rs`:
- Relax the `DocType::Skill`-only gate to `DocType::Skill | DocType::Plan` using `matches!` macro
- Add Plan-specific metadata update branch (separate from Skill path) with direct JSON manipulation of `execution_count`, `failure_count`, and `confidence` fields. Confidence formula: `1.0 - (failure_count / (execution_count + 1))` clamped to [0.0, 1.0] per spec §3.5.4

Changes to `crates/brassclaw_engine/orchestrator/default.py`:
- Add `_write_last_response(state, working_messages)` module-level helper — finds the last non-empty assistant text in `working_messages` and stores it in `state["_last_response"]`
- Add `complete_result` plan tracking preamble: at top of function, read `state.get("active_plan_doc_id")`, if present call `__record_skill_usage__(plan_doc_id, outcome == "completed")`, then pop the key to prevent double-counting

Unit tests for `skill_tracker.rs`:
- `record_usage` succeeds for a `DocType::Plan` doc
- `execution_count`, `failure_count`, `confidence` values update correctly after success and failure
- `record_usage` still fails for `DocType::Note` (unchanged)

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 11: Implement planning pipeline in `default.py`
<!-- chat-id: af033abf-5574-4252-9c6e-6c639464aaa8 -->

Add the full plan-first execution pipeline to the Python orchestrator as described in spec.md §3.5.1–§3.5.3, §3.5.6–§3.5.7.

Changes to `crates/brassclaw_engine/orchestrator/default.py`:
- Add `find_plan_template(docs, goal) -> Optional[dict]` — filters retrieved docs for `DocType::Plan` with `is_template: True` in metadata, returns first match with steps and confidence. Parameter `docs` is the list from `__retrieve_docs__()` (not a separate call)
- Add `find_cached_plan(docs, goal) -> Optional[dict]` — filters retrieved docs for `DocType::Plan` with `is_template: False`, returns highest-confidence match. Includes `is_decomposition` flag in return value for decomposition routing
- Add `run_minimal_planning_call(goal, actions) -> Optional[List[str]]` — isolated LLM call with `{"force_text": True, "is_planning_call": True, "max_tokens": 200}`, 2-message list, budget ≤ 200 tokens. Returns `None` if goal alone exceeds budget or response yields no parseable numbered steps (triggers decomposition path)
- Add `run_miniplan_call(goal) -> Optional[List[str]]` — decomposition call, asks for 2–4 subtasks. Returns `None` and calls `__transition_to__("failed", ...)` if output is unparseable
- Add `is_trivial(goal, config) -> bool` — heuristic using `config.get("trivial_word_threshold", 8)`, checks for `?` without multi-step structure, known single-step patterns
- Add `run_decomposition_loop(subtasks, original_goal, actions, config, state) -> dict` — iterates subtasks; for each subtask creates `subtask_config = dict(config)` and sets `subtask_config["decomposition_depth"] = config.get("decomposition_depth", 0) + 1` (**required** — without the depth increment, the `depth >= 1` guard in `run_planning_phase` never triggers, allowing infinite recursive decomposition); runs full planning + loop per subtask with fresh `subtask_state = {}`; propagates context via `_last_response` (≤ 200 tokens); stops on first subtask failure. Captures `decomp_plan_doc_id` in local variable before subtask iteration to avoid loss via `complete_result` pops. Conditionally saves plan doc on success
- Add `run_planning_phase(goal, actions, config, state) -> Tuple[List[str], str]` — orchestrates: check trivial → use trivial plan; retrieve docs via `__retrieve_docs__(goal, 5)`; **check template match first** → use if found; check cached plan above threshold → reuse (route `is_decomposition` plans to "decompose" source); call `run_minimal_planning_call`; on `None` return → decompose via `run_miniplan_call`; returns `(steps, source)` where source ∈ `{"trivial", "cached", "template", "llm", "decompose", "failed"}`. **Order is critical** (spec §3.5.2 pseudocode): templates are authoritative and take priority over cached runtime plans
- Add `should_invalidate_plan(user_message, current_plan, goal) -> bool` with the keyword heuristic from spec.md §3.5.7 (prefixes: "instead ", "forget ", "stop ", "cancel ", "actually ", "new task", "switch to"; phrases: "do this instead", "change of plan", "never mind", "start over")
- Modify `run_loop()`:
  - [ ] Call `run_planning_phase()` before the step loop; handle all 6 source values (`failed` → return, `decompose` → `run_decomposition_loop`, others → set `state["plan_steps"]` and `state.setdefault("plan_current_step", 0)`)
  - [ ] Call `_write_last_response(state, working_messages)` before **every** `return complete_result(...)` inside the step loop and after the max_iterations exit (~11 call sites per spec §3.5.2)
  - [ ] Advance `state["plan_current_step"]` after each tool call succeeds
  - [ ] Check `should_invalidate_plan()` on each injected user message from `__check_signals__`; if true, clear plan state and re-plan with the new goal (handle decompose/failed per spec §3.5.7)
  - [ ] Integrate `__apply_token_guard__` into `run_loop()` at two distinct call sites per spec §3.2.3:
    - **At step 0 (before skills/docs injection):** pass full PromptParts with reshaped skills and memory docs; filter dropped items from `active_skills`/`docs` before `append_system_append`; if `fits == False`, trigger decomposition (depth guard → clear stale plan doc → `run_miniplan_call`)
    - **At step > 0 (before each `__llm_complete__` call):** pass only system_prompt + conversation_history (skills/docs already embedded); if `fits == False`, emit warning event and remove dropped history messages from `working_messages`
- Modify `select_skills()`:
  - [ ] Change default parameter from `max_tokens=6000` to `max_tokens=2048`
  - [ ] Call site passes `config.get("skill_token_budget", 2048)` as budget
  - [ ] Add zero-budget pre-filter at start: exclude skills with `metadata.activation.max_context_tokens == 0` before scoring
  - [ ] Update `_skill_token_cost()` fallback from `2000` to `0`: `declared = max(activation.get("max_context_tokens", 0), 1)`

Verification: Python syntax check (`python3 -m py_compile default.py`). Manual trace of planning + decomposition flow.

### [x] Step 12: Implement plan templates loader
<!-- chat-id: 30d087d5-0897-49e2-969e-3b9075f132d8 -->

Load bundled plan templates at startup as described in spec.md §3.5.5.

Changes:
- Create plan template files (at minimum 7 seed templates covering all REQ-6.3 categories: `install-software.md`, `web-search.md`, `calendar-events.md`, `notes.md`, `file-search.md`, `git-operations.md`, `system-info.md`). Each uses YAML frontmatter with `title`, `keywords`, `tags`, `confidence`, `is_template: true` and a numbered step list body. Location: either `docs/internal/plan-templates/` or embedded inline via `include_str!` in the Rust loader (implementer's choice per spec §3.5.5). The spec §5.3 integration test 10 asserts `>= 7` templates are discovered at startup, so all 7 must be present
- Add `MemoryStore::ensure_system_docs()` loader (or extend existing if it already exists) to parse plan template files, extract YAML frontmatter via `serde_yaml`, construct `MemoryDoc { doc_type: DocType::Plan, ... }` with `is_template` propagated from frontmatter, and upsert into the system project's memory store. Emit a startup `warn!` for any template missing the `is_template` frontmatter field (still insert with `is_template: false`)
- Wire `ensure_system_docs()` into the application startup sequence in `src/app.rs` or `src/bootstrap.rs`

Unit tests:
- Template with valid frontmatter parses correctly into `MemoryDoc`
- Template missing `is_template` emits a warn but still inserts with `is_template: false`

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 13: Implement `MissionManager` idle mode
<!-- chat-id: b4ac2a14-8f49-4d04-b7b1-34fae49cd9bc -->

Add the `Idle` cadence and `is_system_idle()` logic as described in spec.md §3.6.

Changes to `crates/brassclaw_engine/src/types/mission.rs`:
- `Idle { threshold_secs: u64 }` variant already added in Step 5 — verify it's present

Changes to the `MissionManager` implementation in `crates/brassclaw_engine/src/runtime/mission.rs`:
- Add `last_activity_at: DateTime<Utc>` tracking field
- Add `fn is_system_idle(&self, threshold_secs: u64) -> bool` — checks no thread in Running/Waiting state, no pending job/routine, and `now - last_activity_at >= threshold_secs`
- Update `last_activity_at` on every user message received and every thread completion event
- Persist `last_activity_at` to the DB settings table as `"system.last_activity_at"` via raw settings store API (direct `INSERT OR REPLACE`, not through `Settings` struct — spec §3.6.2)
- Extend `tick()` loop to evaluate `Idle` missions using `is_system_idle()`
- Idle missions appear in the routines panel (same display path as Cron missions)

Unit tests:
- `is_system_idle()` returns false when a thread is running
- `is_system_idle()` returns true after threshold elapsed with no activity
- `Idle` variant serializes/deserializes correctly (backward compat with existing missions)

Verification: `cargo check --workspace` compiles; unit tests pass.

### [x] Step 14: Add local browser tool (Playwright MCP) registry entry and URL filter
<!-- chat-id: cd76f2f0-b9e3-43bb-b625-bd7fe85174ee -->

Add the local browser tool registry entry and the MCP request interceptor as described in spec.md §3.4.1.

Changes:
- Create `registry/mcp-servers/playwright.json` with the spec-defined content (name, display_name, runtime, auto_launch, command, args with `--no-file-access`, env, requires.bins, install_hint, tags)
- `src/tools/mcp/` — add URL filtering interceptor in the MCP request path: scan all string-valued parameters of every outgoing MCP request for `file://` scheme URLs; reject with error message `"file:// URLs are blocked for security. Use the local_search tool for filesystem access."`. Localhost access allowed. If any required actions from REQ-4.2 are absent from the MCP package, add stubs in `src/tools/mcp/playwright_shim.rs`
- Mark the `playwright` capability as `CapabilityStatus::NeedsSetup` with the `install_hint` if `node`/`npx` binaries are absent at startup

Unit tests:
- `file://` URL in any parameter is rejected
- Loopback URL passes through
- HTTPS URL passes through

Verification: `cargo check --workspace` compiles; unit tests for URL filter pass.

### [x] Step 15: Add local tool registry manifests (CalDAV, notes, local-search)
<!-- chat-id: ba62bb38-a353-4f97-af8f-5d617699b443 -->

Create the remaining local tool registry entries described in spec.md §3.4.2.

Changes:
- Create `registry/tools/caldav.json` — CalDAV WASM tool manifest with actions: `list_calendars`, `list_events`, `create_event`, `update_event`, `delete_event`, `list_todos`, `create_todo`, `update_todo`, `delete_todo`. Requires `CALDAV_URL`, `CALDAV_USERNAME`, `CALDAV_PASSWORD` env/secrets. Tags: `["opt-in", "local", "calendar"]`
- Create `registry/tools/local_notes.json` — local plain-text notes tool manifest. Actions: `append_note`, `read_notes`, `search_notes`. Tags: `["default", "local", "notes"]`. Storage path: `~/.brassclaw/notes.md` (per spec §3.4.2)
- Create `registry/tools/local_search.json` — local file search tool manifest. Actions: `search_files`. Tags: `["default", "local", "search"]`. Scope gated by `local_search.allow_global_scope` setting (per spec §3.4.2)
- Create `src/tools/builtin/local_search.rs` — built-in Rust implementation of the `search_files` action (spec §3.4.2, §4.6). Reads `LocalSearchSettings.allow_global_scope` from settings at call time. When `allow_global_scope: false` and the caller passes `scope: "global"`, returns a user-facing error: `"Global filesystem search is disabled. Enable it in Settings → Tools → Local Search."`. May reuse the existing grep/glob infrastructure from `src/tools/builtin/grep_tool.rs` and `glob_tool.rs` for the workspace-scoped path. Register the new tool handler in `src/tools/builtin/mod.rs` and ensure it is wired into the tool dispatch table. This is a **security boundary** (REQ-4.9): without this file the `search_files` action has no implementation, the scope gate doesn't exist, and the mandatory unit test (spec §5.2) cannot be written
- Create `skills/caldav/SKILL.md` — CalDAV calendar skill (≤ 256 tokens declared budget). Activates on calendar/event/meeting/schedule keywords. Guides the LLM to use CalDAV tool actions (spec §2.2)
- Create `skills/notes/SKILL.md` — Local notes skill (≤ 128 tokens declared budget). Activates on note/remember/memo/jot keywords. Guides the LLM to use local_notes tool actions (spec §2.2)
- Create `skills/local-search/SKILL.md` — Workspace file search skill (≤ 128 tokens declared budget). Activates on search/find/grep/locate keywords. Guides the LLM to use local_search tool actions (spec §2.2)

> **No SQL migration needed for LocalSearchSettings.** Per spec §4.6, `LocalSearchSettings` uses the generic `Settings::to_db_map()`/`from_db_map()` persistence mechanism — adding the field with `#[serde(default)]` is sufficient for DB persistence to work automatically. The spec §3.6.1 explicitly states "The V29 migration listed in §2.2 is struck."

Unit tests for `src/tools/builtin/local_search.rs`:
- `search_files` with `scope: "global"` and `LocalSearchSettings { allow_global_scope: false }` → returns error containing "Enable it in Settings"
- `search_files` with `scope: "global"` and `LocalSearchSettings { allow_global_scope: true }` → succeeds
- (The second case is the security gate inverse — both must be tested per spec §5.2)

Verification: `cargo check --workspace` compiles; JSON files are valid; unit tests pass.

### [x] Step 16: Rewrite and trim bundled skills for local LLM use
<!-- chat-id: 32a8461d-6b0e-4afa-8d62-e47fb57e9ccd -->

Rewrite or trim all bundled skills to comply with token budget caps as specified in spec.md §3.4.3 and requirements.md REQ-4.5, REQ-4.6.

For each skill directory under `skills/`:
- Add or update `SKILL.md` frontmatter to declare `max_context_tokens` (YAML field)
- Trim content to fit within the declared cap:
  - Behavioral guidance skills (`commit`, `decision-capture`, `idea-parking`, `code-review`, `qa-review`, `review-checklist`, `review-readiness`, `security-review`, `tech-debt-tracker`): ≤ 128 tokens
  - Tool/procedure skills (`coding`, `local-test`, `new-project`, `project-setup`): ≤ 256 tokens
  - Cloud-credential-gated skills (`github`, `github-workflow`, `linear`): add `requires.env` credential gate if missing; keep as opt-in
  - Large cloud-workflow skills (`ceo-setup`, `commitment-digest`, `commitment-setup`, `commitment-triage`, `content-creator-setup`, `developer-setup`, `delegation`, `delegation-tracker`): rewrite for local use at ≤ 128–256 token declared budget
  - Remove entirely: `portfolio`, `trader-setup`, `llm-council` (cloud-only, no local equivalent)
  - Remaining skills (`plan-mode`, `product-prioritization`, `routine-advisor`, `web-ui-test`): review and trim to appropriate budget cap based on category
- Create new `skills/web-browse/SKILL.md` skill for the Playwright browser tool (≤ 256 tokens, activates on web search / URL query keywords, per spec §2.2)

Update skill selector in `crates/brassclaw_skills/src/selector.rs`:
- Zero-budget pre-filter: add early filter step **before** the scoring loop that removes any skill with `manifest.activation.max_context_tokens == 0` (spec §3.4.3)
- Update `MAX_SKILL_CONTEXT_TOKENS` constant from `4000` to `2048` (matches new default, used by remaining tests)

Add startup warning in `src/skills/mod.rs` (or registry discovery pass):
- Emit `warn!("Skill '{}' loaded from {} has no declared max_context_tokens and will not be injected into prompts", name, path)` for any skill with `max_context_tokens: 0`. One warning per skill per process lifetime (use `HashSet`)

Verification: `cargo check --workspace` compiles; all `SKILL.md` files parse correctly. Each skill's content byte count × 0.25 ≤ declared `max_context_tokens`.

### [x] Step 17: Wire token budget settings through engine path and add UI field
<!-- chat-id: bbbcd562-3dc1-4908-92ce-177c81005688 -->

Wire the token budget settings through the engine v2 path, add the UI field, and update profile defaults as described in spec.md §3.2, §3.7, and requirements.md REQ-2.2, REQ-2.3.

Changes — Settings UI:
- Add the new "Max total prompt tokens" numeric field to the Settings → Agents tab in the web UI (`src/channels/web/features/settings/`). Place it directly below the existing "Skill context token size" field
- Add "Plan confidence threshold" numeric input and "Enable CodeAct for local models" toggle in an advanced section (spec §4.7)
- Add "Allow whole-filesystem search" toggle in Settings → Tools → Local Search section

Changes — Server-side validation:
- Reject saves where `max_prompt_tokens < skill_token_budget` with user-facing error "Max total prompt tokens must be greater than or equal to skill context token size"

Changes — Wiring `ThreadConfig`:
- In `src/bridge/router.rs` (or wherever `ThreadConfig` is constructed for new threads): populate `ThreadConfig.max_prompt_tokens` from `AgentConfig.max_prompt_tokens` and `ThreadConfig.skill_token_budget` from `SkillsConfig.max_context_tokens` (note: the skill budget comes from `SkillsSettings.max_context_tokens`, not from `AgentSettings`)
- Confirm the `orchestrator.rs` config dict (updated in Step 5) now includes `"skill_token_budget"` and `"max_prompt_tokens"` so that `config.get("skill_token_budget", 2048)` in Python resolves from the resolved `ThreadConfig`, not the hardcoded Python fallback

Changes — Profile defaults (spec §3.7):
- `profiles/local.toml`: add `[agent] max_prompt_tokens = 8192` and `[skills] max_context_tokens = 2048`
- `profiles/server.toml` and `profiles/server-multitenant.toml`: add `[agent] max_prompt_tokens = 131072`

Unit tests:
- Saving `max_prompt_tokens < skill_token_budget` returns a validation error
- Saving `plan_confidence_threshold` outside `[0.0, 1.0]` (e.g. `-0.1` or `1.1`) returns 400 (spec §5.2)

Verification: `cargo check --workspace` compiles; settings round-trip test passes.

### [x] Step 18: Final integration verification
<!-- chat-id: 11e537a8-3c72-4902-b680-a06618bfc997 -->

Run the full test suite and lint checks. Fix any remaining compile errors or test failures.

Verification steps:
- `cargo check --workspace` — no errors
- `cargo clippy --workspace -- -D warnings` — no warnings promoted to errors
- `cargo test --workspace` — all tests pass (skip tests requiring live credentials or network)
- `cargo nextest run` (if configured in `.config/nextest.toml`) — all tests pass
- `python3 -m py_compile crates/brassclaw_engine/orchestrator/default.py` — no syntax errors
- Spot-check: start the agent with `database_backend = "libsql"` (local profile), configure an Ollama provider, verify that `PlatformInfo.is_local_backend = true`, and confirm the Tier 0 system prompt is used (no CodeAct preamble in the assembled prompt)
- Confirm `cargo check --workspace` produces no reference to `SkillCatalog`, `engine_v2` (field), `is_engine_v2_enabled`, `agentic_loop`, `session_manager`, `dispatcher`, `compaction`, `context_monitor`

---

## Follow-Up Steps (identified during review rounds 8–10)

> These items were identified as architectural improvements that go beyond single-fix scope.
> Each is tracked as its own step below. REQ and spec references are appended inline.

### [x] Step 19: Propagate operator-configured ThreadConfig to mission threads
<!-- chat-id: 95e07d90-979e-4e81-99ff-c9837c32d569 -->

Currently, `MissionManager::fire_mission()` uses `ThreadConfig::default()` when spawning mission threads (line 883 of `crates/brassclaw_engine/src/runtime/mission.rs`). This means mission threads always get the hardcoded defaults (`max_prompt_tokens: 8192`, `skill_token_budget: 2048`, `plan_confidence_threshold: 0.6`) regardless of the operator's settings. A user who configures a larger local context window (e.g. for a 70B model) will see mission threads silently truncated at 8192 while foreground threads use the configured value.

Changes:
- Add a `default_thread_config: ThreadConfig` field to `MissionManager` (or pass it through `MissionManagerConfig`)
- Populate it at construction time from the resolved `AgentConfig` (same path that `router.rs` uses for user-message threads)
- Replace `ThreadConfig::default()` in `fire_mission()` with `self.default_thread_config.clone()`
- When a `Mission` struct has its own per-mission override fields (future), prefer those over the default

Verification:
- Unit test: `fire_mission` spawns a thread whose `ThreadConfig.max_prompt_tokens` matches the injected config, not the default
- Integration: configure `max_prompt_tokens = 16384` in settings, fire a mission, verify the spawned thread's config reflects 16384

### [x] Step 20: Optimize `count_running_threads` to avoid O(N) store round-trips
<!-- chat-id: 57f25549-343c-4087-ac6d-d57672c28d39 -->

`MissionManager::count_running_threads()` iterates `mission.thread_history` in reverse and calls `self.store.load_thread(tid)` for each entry. For long-running missions that have accumulated hundreds of thread IDs, this performs O(N) store calls on every `fire_mission()` invocation (the `max_concurrent` gate).

Changes:
- Option A (preferred): Add a `running_thread_count: AtomicUsize` field to `MissionManager` (or a `HashMap<MissionId, usize>`). Increment on `fire_mission()` success, decrement in `spawn_mission_outcome_watcher()` when the thread completes. `count_running_threads()` becomes an O(1) read
- Option B: Add a `Store::count_threads_by_status(mission_id, &[ThreadState::Running, ThreadState::Waiting])` query that pushes the filter to the DB layer as a single SQL `SELECT COUNT(*)` instead of loading N full thread records
- Whichever option is chosen, retain the current `load_thread` loop as a fallback for correctness during the migration or when the in-memory counter is suspect (e.g. after a process restart)

Verification:
- Unit test: fire 5 mission threads, complete 3, verify count returns 2 without loading all 5 from the store
- Benchmark: with 200 thread_history entries, verify `fire_mission` does not regress to O(N) store calls

### [x] Step 21: Deduplicate plan docs in `__save_plan_doc__` to prevent redundant cache entries
<!-- chat-id: 6a6c2ade-4c04-4aae-a2df-f87c70dc75a3 -->

`handle_save_plan_doc` in `orchestrator.rs` (line 2892) always creates a new `MemoryDoc` with a fresh `DocId` (UUID). Two threads with the same goal (or goals sharing a 64-char prefix) each create independent plan docs with identical titles. The `retrieve_context` path surfaces both, wasting token budget and potentially confusing plan-confidence scoring.

Changes:
- Before creating a new doc, query the store for an existing `DocType::Plan` doc with the same title (`"plan:{goal_prefix}"`) in the same project
- If found: update the existing doc's `steps`, `confidence`, and `updated_at` fields. Preserve the existing `execution_count` and `failure_count` — do not reset them
- If not found: create a new doc as currently done
- The upsert must use `with_trusted_internal_writes` since plan docs carry the `plan:` title prefix

Verification:
- Unit test: save two plans with the same goal — verify only one doc exists in the store
- Unit test: save a plan, then save again with updated steps — verify the existing doc was updated and execution_count is preserved

### [x] Step 22: Integrate a proper tokenizer for accurate CJK/multilingual token counting
<!-- chat-id: 5941a3aa-8f91-4196-bb22-3b25f58c03bf -->

The current token counting approximation (`len(bytes) * 0.25`) under-estimates token counts for CJK, Arabic, and emoji-heavy text by up to 50–100%. For ASCII-heavy English content the approximation is acceptable, but for multilingual home-use scenarios this creates a real gap where the token guard approves prompts that exceed the actual model's context window.

**Approach — hybrid script detection:**
- Detect whether the input text contains CJK or Arabic characters (Unicode ranges: CJK Unified Ideographs U+4E00–U+9FFF, CJK Extension A/B, Katakana, Hiragana, Hangul, Arabic U+0600–U+06FF, Arabic Supplement/Extended)
- If no CJK/Arabic characters are present: keep the existing `len(bytes) * 0.25` approximation (accurate for ASCII/Latin prose)
- If CJK or Arabic characters are detected: delegate to `tiktoken-rs` (cl100k_base encoding as universal fallback) for an accurate per-token count. Fall back to byte approximation if the library is unavailable or the model is unknown

Changes:
- Add `tiktoken-rs` (or `tiktoken` crate) as an optional dependency to `brassclaw_engine/Cargo.toml` behind a `tiktoken` feature flag so the crate compiles without it in constrained environments
- Add `fn has_cjk_or_arabic(text: &str) -> bool` in `token_guard.rs` — scans chars for the Unicode ranges above
- Update `token_count(text: &str) -> usize` in `token_guard.rs` to call `has_cjk_or_arabic` first and branch accordingly
- Add a `__count_tokens__` host function in `orchestrator.rs` that calls the Rust `token_count()` and returns the result to Python
- Update `_token_count(text)` in `default.py` to call `__count_tokens__(text)` so Python and Rust stay in sync; keep the byte-based inline formula as a fallback comment for environments where the host function is unavailable
- Update the Python `_token_count()` fallback inline formula to also apply the same CJK/Arabic heuristic (use `len(text.encode("utf-8"))` normally; if any char ordinal falls in a CJK/Arabic range use `len(text)` × 1.5 as conservative estimate when `__count_tokens__` is unavailable)

Verification:
- Unit test: `has_cjk_or_arabic` returns `true` for a Chinese sentence, an Arabic sentence, and mixed CJK+Latin; returns `false` for pure ASCII and Latin-1 text
- Unit test: `token_count` on a 100-character CJK string produces a count within 20% of tiktoken's cl100k_base output
- Unit test: `token_count` on pure ASCII still uses the byte-based path (verify no tiktoken call via a counter or cfg flag)
- Unit test: ASCII strings produce estimates within 30% of tiktoken
- Constraint C3 in the spec must be updated to reflect the hybrid approach

### [x] Step 23: Remote Deployment and Concept Proof
<!-- chat-id: b0892492-c911-4ad5-85d7-0c646b1578f5 -->
- Connect to `root@192.168.10.169` with the pw `L1l4pause`.
- Uninstall/delete any existing brassclaw copies that were downloaded from nearai and not from our repo. If brassclaw came from our repo - update it.
- The system is debian dietpi on intel and has a nvidia gpu with 16gb vram.
- Look if vllm is installed as a systemd service and runs qwen2.5-7B-AWQ - if not do so.
- Now install our brassclaw and run it with the local qwen model.
- Debug and fix until everything runs smooth.
- Also test and improve tool usage etc. to prove our concept - intelligence and speed is our expectation.
- Improve where we failed until we meet those expectations.
- Use the UI mandatorily with playwright (also mandatory for subagents).

### [x] Step 24: Connect and test local tools execution, playwright, and UI stability
<!-- chat-id: 24c2d54e-0d5f-4c31-b68a-a9c83c5256e1 -->

Test, debug, and verify local tools and Playwright browser execution via the brassclaw-ui on the remote test machine (`192.168.10.169`, credentials: `root` / `L1l4pause`).

Detailed verification and execution instructions:
1. **GitHub Synchronization**: Commit any local modifications (like documentation/repository url updates) and push to the `chtugha/brassclaw` main branch on GitHub:
   ```bash
   git add .
   git commit -m "update install docs and clarify step 24"
   git push origin main
   ```
2. **SSH Connection**: Establish an SSH session with the remote test machine:
   - **Command**: `ssh root@192.168.10.169` (Password: `L1l4pause`)
3. **Repository Update**: Navigate to the repository directory `/root/brassclaw` (not the old nearai one), ensure no dangling nearai processes or directories are interfering, and pull the latest changes:
   ```bash
   cd /root/brassclaw
   git pull origin main
   ```
4. **Rebuild**: Recompile the binary in release mode:
   ```bash
   cargo build --release
   ```
5. **Service Restart**: Restart the systemd service to run the newly compiled binary from our active repo:
   ```bash
   systemctl restart brassclaw.service
   ```
6. **Log Monitoring**: Follow the logs to ensure successful startup:
   ```bash
   journalctl -u brassclaw -n 50 -f
   ```
7. **UI and Reconnect Bug Test**: Connect to the BrassClaw UI at `http://192.168.10.169:3000`. Test WebSocket and SSE connections, simulating disconnects to verify that the active SSE reconnection scheduler with exponential backoff works perfectly and prevents any "reconnecting" lockup.
8. **Local Tools and Playwright Testing**: Run chats via the web UI to trigger local tools (`caldav`, `local_notes`, `local_search`) and Playwright-based browser tasks (e.g. visiting a website or searching) and verify the agent and UI remain perfectly stable without any crashes under local model constraints. Ensure all 33 skills can be selected and routed smoothly over playwright without crashing the agent or gateway.

### [x] Step 25: Handle large MCP playwright screenshots safely to prevent UI crashes
<!-- chat-id: 258ea8b7-6347-4fd5-97d8-37ec2b52a4c3 -->
- **Technical Context**: Playwright MCP tool returns massive base64 screenshots inside JSON. Streaming large base64 strings crashes the client/UI (Reconnecting bug).
- **Changes**:
  1. Modify `registry/mcp-servers/playwright.json` to instruct the playwright server to write screenshots to a local file or temporary directory instead of returning inline base64 data, or write a custom playwright MCP client wrapper in Rust (`src/tools/mcp/client.rs` or `playwright_shim.rs`) to capture the screenshot output, save it to `/root/.brassclaw/temp/` (static file directory), and return a relative path.
  2. Implement a safe static file server route or static file handler in our web gateway (`src/channels/web/mod.rs`) to serve images from the `.brassclaw/temp` directory.
  3. Ensure that if base64 screenshots are received, we automatically compress or truncate/discard them from the SSE message payload, providing a clean warning/placeholder instead to preserve WebSocket/SSE stability.
- **Verification**: Run Playwright screenshots and verify that no websocket disconnects or gateway buffer overruns happen, and that images are loaded cleanly.

### [x] Step 26: Optimize and deduplicate planning template and cache documents (C12)
<!-- chat-id: 19554d73-432e-4221-b96f-b4be8dd687b0 -->
- **Technical Context**: `__save_plan_doc__` in `orchestrator.rs` appends redundant plans with identical goals, bloating the memory store and wasting prompt tokens.
- **Changes**:
  1. Modify `crates/brassclaw_engine/src/executor/orchestrator.rs` (under the `__save_plan_doc__` host function) to check the memory store first for any existing `DocType::Plan` doc in the project matching the goal prefix.
  2. If found, update the existing document's step list, confidence, and timestamp, maintaining its original ID and executing statistics.
  3. If not found, create a new document.
- **Verification**: `cargo test -p brassclaw_engine` passes and plan saving does not create duplicate documents with identical goals in integration tests.

### [x] Step 27: Enforce tiktoken dependency and accurate multilingual token counts (C13)
<!-- chat-id: 0ea484b1-a5c2-461e-b19d-cade51c1c5df -->
- **Technical Context**: When built without the `tiktoken` cargo feature, non-ASCII language (CJK/Arabic) token count falls back to byte approximation which is highly inaccurate.
- **Changes**:
  1. Modify `crates/brassclaw_engine/Cargo.toml` to enable the `tiktoken` feature by default, or change the fallback logic in `crates/brassclaw_engine/src/executor/token_guard.rs` to use a conservative estimate factor of `1.5` times length for detected CJK or Arabic characters instead of `0.25` when tiktoken is absent.
  2. Add `default-features = true` or explicitly list `tiktoken` in workspace/release builds.
- **Verification**: Run `cargo check` and run token guard tests verifying both ASCII and non-ASCII string estimates are accurate.




### [x] Step 28: Create release packaging and bundle builder script
<!-- chat-id: a5130e35-49be-4c1b-bb11-49bc7b386054 -->
- **Technical Context**: To build standalone precompiled archives containing the binary and static resources (skills, tool registries) for distribution.
- **Changes**:
  - Implement a bash script `scripts/package-release.sh` to compile `brassclaw` with `--release` for target architectures.
  - Automatically copy `skills/` and `registry/` folders into the target archive.
  - Package as `.tar.gz` for Unix and `.zip` for Windows.
- **Verification**: Run `bash scripts/package-release.sh` and verify generated package structures contain `brassclaw`, `skills/`, `registry/`, and `.env.example`.

### [x] Step 29: Implement POSIX-compliant shell installation script (`install.sh`)
<!-- chat-id: 315fd692-0d18-4353-9fb3-50d2ee6a3ce0 -->
- **Technical Context**: A robust shell installer that auto-detects architecture, downloads assets, sets up the `$HOME/.brassclaw` workspace, and configures environments.
- **Changes**:
  - Create `scripts/install.sh` to auto-detect system triple (`uname -s` and `uname -m`).
  - Retrieve the latest release from the repository, with fallback options.
  - Create `$HOME/.brassclaw/`, `$HOME/.brassclaw/temp`, `$HOME/.brassclaw/skills`, and `$HOME/.brassclaw/registry`.
  - Copy extracted skills and registry tools to `$HOME/.brassclaw/`.
  - Securely generate random 64-hex-char `SECRETS_MASTER_KEY` and 32-hex-char `GATEWAY_AUTH_TOKEN` using `/dev/urandom`.
  - Generate default `~/.brassclaw/.env` targeting a local Ollama instance (`http://localhost:11434/v1`) with libSQL configuration.
  - Prompt user to add `~/.local/bin` to PATH if not already present.
  - Create systemd service templates (`~/.config/systemd/user/brassclaw.service` and `/etc/systemd/system/brassclaw.service`) if systemd is available.
- **Verification**: Test the script on a fresh container or unprivileged user account and verify correct file placement, path prompting, and service creation.

### [x] Step 30: Implement Windows PowerShell installation script (`install.ps1`)
<!-- chat-id: f82a9997-8ac6-4c3e-b499-9a4233a2dd15 -->
- **Technical Context**: Equivalent native setup utility for Windows environments.
- **Changes**:
  - Create `scripts/install.ps1` to detect environment architecture.
  - Download and unpack target Windows `.zip` archive.
  - Add install directory to user PATH.
  - Create `$HOME/.brassclaw` workspace.
  - Generate secure randomized credentials using standard Cryptography assemblies.
- **Verification**: Run `powershell.exe -ExecutionPolicy Bypass -File scripts/install.ps1` in Windows sandbox and verify PATH and environment updates.

### [x] Step 31: Implement installer integration tests and template validators
<!-- chat-id: e2ec583f-8189-4473-be19-7040c1bdf5d9 -->
- **Technical Context**: Ensure installer templates remain in sync with configuration changes and that architecture detection is robust.
- **Changes**:
  - Add test fixtures verifying random generation logic and default value mappings.
  - Add tests validating that `.env.example` parses cleanly with `dotenvy`.
- **Verification**: `cargo test` passes.
