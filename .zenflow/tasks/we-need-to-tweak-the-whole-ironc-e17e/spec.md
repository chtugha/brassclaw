# Technical Specification — BrassClaw Home-Use Redesign

**Date:** 2026-05-07  
**Status:** Draft v1  
**PRD:** `requirements.md`

---

## 1. Technical Context

### 1.1 Language & Runtime

| Layer | Technology |
|---|---|
| Core application | Rust (2021 edition), async/tokio |
| Engine library | `crates/brassclaw_engine` — independent crate, no circular dep |
| Orchestrator script | Python 3 via Monty interpreter (`orchestrator/default.py`) |
| Wasm extensions | `crates/brassclaw_wasm`, `channels-src/` — compiled to wasm32-wasip2 |
| Database | PostgreSQL (server) / libSQL (local, `database_backend = "libsql"`) |
| Migrations | Flyway-style versioned `.sql` files in `migrations/` |

### 1.2 Key Dependencies to Know

- `crates/brassclaw_skills` — skill manifest parsing, token-budget selection (`selector.rs`)
- `crates/brassclaw_engine` — five-primitive execution model (Thread, Step, Capability, MemoryDoc, Project)
- `src/bridge/router.rs` — engine v2 router, wires LLM adapter + effect adapter to engine
- `src/agent/agent_loop.rs` — legacy v1 execution loop (to be removed)
- `src/config/skills.rs` — `SkillsConfig`, reads `SkillsSettings` from DB
- `src/settings.rs` — `Settings` struct, `SkillsSettings`, `AgentSettings`
- `registry/` — JSON manifests for tools/MCP-servers/channels (online-only entries to be removed)
- `skills/` — bundled `SKILL.md` files (cloud-only skills to be replaced/trimmed)

### 1.3 Token Counting Convention

Token counting uses a **hybrid script-detection approach**:

- **ASCII/Latin text**: byte-based approximation `(len(bytes) * 0.25)` — accurate to within 30% for English prose (≈ 4 bytes per token).
- **CJK/Arabic/Hangul text**: detected via `has_cjk_or_arabic()` Unicode range scan. Delegates to `tiktoken-rs` cl100k_base when the optional `tiktoken` Cargo feature is enabled. Falls back to `char_count * 1.5` (conservative over-estimate, errs on the side of caution for budget enforcement).

The Rust implementation lives in `token_guard::token_count()`. The Python orchestrator calls the `__count_tokens__` host function for consistency, with a local fallback using the same CJK/Arabic heuristic when the host function is unavailable.

---

## 2. Source Code Structure Changes

### 2.1 Files to Remove (v1 agent execution path)

The v1 execution path is gated by `ENGINE_V2=true` in `src/bridge/router.rs:is_engine_v2_enabled()` and the `engine_v2: bool` field in `AgentConfig`. After this change, engine v2 is unconditional and all v1-specific code is deleted.

**Remove entirely:**

| File/Directory | Reason |
|---|---|
| `src/agent/dispatcher.rs` | v1 dispatcher — routes sessions, jobs, routines in the old model |
| `src/agent/session.rs` | v1 session lifecycle |
| `src/agent/session_manager.rs` | v1 session manager |
| `src/agent/routine.rs` | v1 routine runner |
| `src/agent/routine_engine.rs` | v1 routine scheduling engine |
| `src/agent/agentic_loop.rs` | v1 agentic loop (separate from the v2 `agent_loop.rs` wrapper) |
| `src/agent/scheduler.rs` | v1 cron/routine scheduler (missions replace routines in v2) |
| `src/agent/submission.rs` | v1 job submission |
| `src/agent/job_monitor.rs` | v1 job monitor |
| `src/agent/self_repair.rs` | v1 self-repair (missions handle this in v2) |
| `src/agent/compaction.rs` | v1 context compaction (orchestrator handles this in v2) |
| `src/agent/context_monitor.rs` | v1 context overflow monitor |
| `src/agent/cost_guard.rs` | v1 cost guard (v2 has its own budget tracking) |
| `src/agent/undo.rs` | v1 undo stack |
| `src/orchestrator/` | v1 API surface for orchestrating jobs/sessions |
| `src/worker/job.rs`, `src/worker/acp_bridge.rs`, `src/worker/claude_bridge.rs` | v1 job worker, cloud bridge workers |
| `src/hooks/` | v1 hook system (replaced by missions in v2) |
| `src/skills/attenuation.rs` | v1 trust-based tool attenuation — v2 uses policy-level grants |
| `src/context/` | v1 context state management (`fallback.rs`, `manager.rs`, `memory.rs`, `state.rs`) |
| `src/history/` | v1 history store (v2 uses engine-owned store via `StoreAdapter`) |
| `src/evaluation/` | v1 session success/failure metrics tied to old session model |
| `src/estimation/` | v1 cost/time/value estimation (tied to v1 job model) |

**Modify:**

> **Naming note:** `loop_engine.rs` and `agent_loop.rs` are distinct files at different layers. `crates/brassclaw_engine/src/executor/loop_engine.rs` is the engine-crate Rust loop that directly runs threads in v2. `src/agent/agent_loop.rs` is the host-crate wrapper that calls into the engine. Both require changes.

| File | Change |
|---|---|
| `src/bridge/router.rs` | Remove `is_engine_v2_enabled()` and its `ENGINE_V2` env check. Remove all `if engine_v2 { ... } else { ... }` branches. Remove all v1 fallback paths in `handle_message()`. |
| `src/config/agent.rs` | Remove `engine_v2: bool` field and the `ENGINE_V2` env parse. Add `max_prompt_tokens: usize`, `plan_confidence_threshold: f64`, and `codeact_enabled: Option<bool>`. Update `AgentConfig::for_testing()` (exhaustive struct constructor at `agent.rs:55–79`): remove the `engine_v2: false` field, add `max_prompt_tokens: 8192`, `plan_confidence_threshold: 0.6`, `codeact_enabled: None`. Failing to update `for_testing()` causes a compile error because Rust requires all struct fields to be provided in exhaustive construction. Also update `AgentConfig::resolve()` to resolve the three new fields from `AgentSettings` using `db_first_or_default` for `max_prompt_tokens` and `plan_confidence_threshold`; `codeact_enabled` is read directly from `settings.agent.codeact_enabled` (no env var override). |
| `src/agent/agent_loop.rs` | Reduce to the v2-only path: remove all `if self.config.engine_v2` branches and the v1 code inside those branches. |
| `src/agent/thread_ops.rs` | Remove v1 approval processing (`process_approval()` v1 path, keep v2 path). |
| `src/gate/approval.rs` | Remove the comment noting v1/v2 inconsistency once v1 is gone; unify the auto-deny logic. |
| `src/channels/web/features/status/mod.rs` | Remove `engine_v2_enabled: bool` from the status response. |
| `src/app.rs` | Remove `is_engine_v2_enabled()` call and the conditional engine version string in the startup log. |
| `crates/brassclaw_engine/src/executor/loop_engine.rs` | Replace unconditional `build_codeact_system_prompt_with_docs()` call in `refresh_system_prompt()` with `should_use_tier0()` branch (§3.3.3). Add plan anchor read from checkpoint metadata. Generalize system prompt detection/upsert to handle both CodeAct and Tier 0 markers (§3.3.5). |
| `crates/brassclaw_engine/src/executor/prompt.rs` | Add `pub is_local_backend: bool` field to `PlatformInfo`. Apply 60-word cap in `compact_prompt_description()`. Add `TIER0_SYSTEM_PROMPT_MARKER`, `is_engine_system_prompt()`, `upsert_engine_system_prompt()`, `refresh_engine_system_prompt()` (§3.3.5). |
| `src/bridge/llm_adapter.rs` | Populate `PlatformInfo.is_local_backend` from the active provider's `is_local` flag. |
| `src/settings.rs` | Add `is_local: bool` to `CustomLlmProviderSettings`. Add `max_prompt_tokens: usize`, `plan_confidence_threshold: f64`, and `codeact_enabled: Option<bool>` to `AgentSettings`. Lower `default_skills_max_context_tokens()` to `2048`. Add new `LocalSearchSettings` struct (see §4.6) and a `local_search: LocalSearchSettings` field to the top-level `Settings` struct alongside the existing `agent`, `skills`, and `wasm` fields. |
| `crates/brassclaw_engine/src/memory/skill_tracker.rs` | Relax `record_usage()` doc type gate from `DocType::Skill`-only to `DocType::Skill | DocType::Plan`. Add Plan-specific metadata update branch (§3.5.4). |
| `crates/brassclaw_engine/src/executor/orchestrator.rs` (`handle_retrieve_docs`) | Extend the returned dict to include `doc_id` and `metadata` in addition to the existing `type`, `title`, `content` keys. Required for plan retrieval helpers in §3.5.2. See §3.5.2 for the exact shape. Add new host function handlers for `__apply_token_guard__` (§3.2.3) and `__save_plan_doc__` (§4.5). Extend `handle_llm_complete` to read `is_planning_call` from the Python config dict (§4.4). |
| `crates/brassclaw_engine/src/types/thread.rs` | Add 5 new fields to `ThreadConfig`: `max_prompt_tokens`, `skill_token_budget`, `codeact_enabled`, `decomposition_depth`, `plan_confidence_threshold` — all with `#[serde(default)]` attributes for checkpoint compatibility. Extend the `impl Default for ThreadConfig` block to include all five new fields. See §4.1. |
| `crates/brassclaw_engine/src/types/mission.rs` | Add `Idle { threshold_secs: u64 }` variant to `MissionCadence`. See §4.2. |
| `crates/brassclaw_engine/src/types/capability.rs` | Add `Syncing` variant to `CapabilityStatus`. See §4.3. |
| `crates/brassclaw_engine/src/executor/prompt.rs` (`capability_status_label`) | Add `CapabilityStatus::Syncing => "syncing"` arm to the exhaustive match at line 291. Without this, the new variant causes a compile error. |
| `src/bridge/tool_surface.rs` (`fallback_assignment`) | Add `CapabilityStatus::Syncing` arm to the exhaustive match at line 102. Syncing tools are not directly callable — assign `SurfaceAssignment::capabilities_only()` (same as `NeedsSetup`). |
| `src/bridge/action_projector.rs` (`provider_extension_rank`) | Add `CapabilityStatus::Syncing` arm to the exhaustive match at line 334. Rank `2` (same as `NeedsSetup` — transitional, not yet ready). |
| `crates/brassclaw_engine/src/traits/llm.rs` | Add `pub is_planning_call: bool` to `LlmCallConfig`. Auto-defaults to `false` via existing `#[derive(Default)]`. See §4.4. |
| `crates/brassclaw_engine/orchestrator/default.py` | Most heavily modified file. Add module-level helpers: `_token_count()`, `_write_last_response()`. Add planning functions: `find_plan_template()`, `find_cached_plan()`, `run_planning_phase()`, `run_minimal_planning_call()`, `run_miniplan_call()`, `is_trivial()`, `run_decomposition_loop()`, `should_invalidate_plan()`. Modify `complete_result()` to add plan confidence tracking. Modify `run_loop()` to call `run_planning_phase()` before the step loop and integrate `__apply_token_guard__`. Modify `select_skills()` call site to use `config.get("skill_token_budget", 2048)`. Add zero-budget pre-filter in `select_skills()`. Update `_skill_token_cost()` fallback default from `2000` to `0`. |

**Online skill catalog removal (REQ-1.5):**

The runtime online skill catalog (`SkillCatalog` in `crates/brassclaw_skills/src/catalog.rs`) queries ClawHub's cloud registry API for skill discovery and download. This entire module must be removed as a runtime dependency. Specifically:

| File | Change |
|---|---|
| `crates/brassclaw_skills/src/catalog.rs` | Remove entirely. The `SkillCatalog` struct, `shared_catalog()`, `skill_download_url()`, and all ClawHub API client code are deleted. |
| `src/app.rs` | Remove `skill_catalog: Option<Arc<SkillCatalog>>` from `AppComponents`. Remove the `SkillCatalog` construction in `build_components()`. Remove `.with_skill_catalog()` calls on the gateway builder. |
| `src/channels/web/mod.rs` | Remove `skill_catalog` field from `GatewayState`. Remove `with_skill_catalog()` method. Remove the `use brassclaw_skills::catalog::SkillCatalog` import. |
| `src/channels/web/handlers/skills.rs` | Remove the catalog-based skill download path (the branch that calls `skill_download_url(catalog.registry_url(), ...)` and `fetch_skill_payload()`). The `install_skill` handler retains only the direct-URL install path. The `list_catalog` handler returns an **empty JSON array with HTTP 200** — this preserves backward compatibility with existing UI clients and API consumers that expect a list response. |
| `src/cli/skills.rs` | Remove `SkillCatalog::new()` usage and the `search` / `browse` subcommands that query the remote catalog. |
| `src/channels/web/test_helpers.rs`, `src/testing/mod.rs`, `src/agent/thread_ops.rs` (test blocks), `src/channels/web/tests/multi_tenant.rs` | Remove `skill_catalog: None` from test state construction (field no longer exists). |

Post-condition: `cargo check --workspace` must compile without any reference to `SkillCatalog` or `catalog.rs`. Skills are installed only from local filesystem paths or direct URLs.

**Registry entries to remove (online-only tools with no local equivalent):**

| Entry | Path | Reason |
|---|---|---|
| `web_search` | `registry/tools/web_search.json` | Requires Brave API key. Replaced by local Playwright browser tool (REQ-4.1). |
| `composio` | `registry/tools/composio.json` | Cloud-only integration hub. No local equivalent. |
| `gmail` | `registry/tools/gmail.json` | Cloud-only; requires Google OAuth. Opt-in extra. |
| `google_calendar` | `registry/tools/google_calendar.json` | Cloud-only. CalDAV (REQ-4.10) is the local replacement. Opt-in extra. |
| `google_docs` | `registry/tools/google_docs.json` | Cloud-only. Opt-in extra. |
| `google_drive` | `registry/tools/google_drive.json` | Cloud-only. Opt-in extra. |
| `google_sheets` | `registry/tools/google_sheets.json` | Cloud-only. Opt-in extra. |
| `google_slides` | `registry/tools/google_slides.json` | Cloud-only. Opt-in extra. |
| `slack_tool` | `registry/tools/slack_tool.json` | Cloud service. Opt-in extra. |
| `telegram_mtproto` | `registry/tools/telegram_mtproto.json` | Cloud service. Channel integrations are separate from tools. Opt-in extra. |
| `portfolio` | `registry/tools/portfolio.json` | NEAR AI DeFi tool. Cloud-only. Remove. |
| `llm_context` | `registry/tools/llm_context.json` | Cloud LLM context injection — not meaningful for local. Remove. |

**MCP server registry entries to remove (online-only):**

| Entry | Path | Reason |
|---|---|---|
| `asana` | `registry/mcp-servers/asana.json` | Cloud SaaS. Remove default. |
| `notion` | `registry/mcp-servers/notion.json` | Cloud SaaS. Remove default. |
| `linear` | `registry/mcp-servers/linear.json` | Cloud SaaS. Remove default. |
| `stripe` | `registry/mcp-servers/stripe.json` | Cloud payment service. Remove default. |
| `intercom` | `registry/mcp-servers/intercom.json` | Cloud customer support SaaS. Remove default. |
| `sentry` | `registry/mcp-servers/sentry.json` | Cloud monitoring SaaS. Remove default. |
| `nearai` | `registry/mcp-servers/nearai.json` | NEAR AI cloud service. Remove default. |
| `cloudflare` | `registry/mcp-servers/cloudflare.json` | Cloud CDN service. Remove default. |

**`registry/_bundles.json`:** Remove the `default` bundle (which included `tools/github`, Gmail, Google Calendar, etc.). Add a new `home` bundle containing only the new local tools (browser, caldav, notes, local-search).

**Bundled skills to remove or replace:**

Skills that require cloud API credentials and cannot activate without them must be removed from the default bundled set. Skills in the following list either gate on a credential check or are removed entirely:

| Skill | Current state | Action |
|---|---|---|
| `github` / `github-workflow` | Requires `GITHUB_TOKEN`. Already has `requires.env` — retain but gate activation on credential presence. | Keep as opt-in; add `requires.env: [GITHUB_TOKEN]` credential gate if missing. |
| `linear` | Requires Linear API key. | Keep as opt-in; add credential gate. |
| `ceo-setup`, `commitment-*`, `delegation-*` | Large prompts (~8.7KB SKILL.md), cloud-workflow-oriented. | Rewrite for local use at ≤ 128 token declared budget. |
| `content-creator-setup`, `developer-setup` | Large setup bundles (10–15KB). | Rewrite for local use at ≤ 256 token declared budget. |
| All others (coding, commit, decision-capture, idea-parking, etc.) | Locally applicable. | Trim to declared budget caps (see §3.4.3). |

### 2.2 New Files to Add

| File | Purpose |
|---|---|
| `crates/brassclaw_engine/prompts/tier0_system_prompt.md` | Compact system prompt for Tier 0 (≤ 1,024 tokens). Plain language, no Python. Includes plan anchor placeholder (`{plan_anchor}`). |
| `crates/brassclaw_engine/src/executor/tier0_prompt.rs` | Rust module: `build_tier0_system_prompt(platform, plan_anchor) -> String`. Called instead of `build_codeact_system_prompt_*` when backend is local. |
| `crates/brassclaw_engine/src/executor/token_guard.rs` | `TokenGuard` struct: `fn apply(budget: &PromptBudget, parts: &mut PromptParts) -> DroppedItems`. Implements priority-order degradation (§3.2.3). |
| `crates/brassclaw_engine/src/executor/planner.rs` (NEW in executor) | Rust-side plan support: `PlanAnchor` struct and `to_prompt_section()`. Planning logic (`is_trivial()`, `run_planning_phase()`, `run_minimal_planning_call()`) lives in the Python orchestrator (§3.5.2), not here. Separate from `capability/planner.rs` (capability grants). |
| `migrations/V28__prompt_token_ceiling.sql` | New DB column/setting: `agent.max_prompt_tokens` default 8192. |
| ~~`migrations/V29__mission_idle_cadence.sql`~~ | **Not needed.** Missions are stored in-memory (`HashMap` in `store_adapter.rs`) and serialized via `serde_json`, not in a SQL table. Adding the `Idle` variant to `MissionCadence` is a code-only change — existing mission data (if persisted to settings JSON) round-trips automatically because `MissionCadence` is a tagged enum. No migration required. |
| `registry/mcp-servers/playwright.json` | Local Playwright MCP server registry entry (no API key, auto-launch). |
| `registry/tools/caldav.json` | CalDAV built-in tool manifest. |
| `registry/tools/local_notes.json` | Local notes (Markdown) tool manifest. |
| `registry/tools/local_search.json` | Local file search tool manifest. |
| `skills/web-browse/SKILL.md` | Web browsing skill (≤ 256 tokens). Activates on search/URL/current-events keywords. |
| `skills/caldav/SKILL.md` | CalDAV calendar skill (≤ 256 tokens). |
| `skills/notes/SKILL.md` | Local notes skill (≤ 128 tokens). |
| `skills/local-search/SKILL.md` | Workspace file search skill (≤ 128 tokens). |
| `skills/plan-templates/SKILL.md` | Not a skill — this is a bundle loader for plan template MemoryDocs. |
| `docs/internal/plan-templates/` | Directory of plan template markdown files (one per task category from REQ-6.3). |

**New methods to add (not new files, but currently absent from the codebase):**

| Method | Location | Purpose |
|---|---|---|
| `MemoryStore::ensure_system_docs(docs: Vec<MemoryDoc>)` | `crates/brassclaw_engine/src/memory/store.rs` (or the `Store` trait impl) | Idempotent upsert for system-owned MemoryDocs (plan templates). Called at startup. Inserts each doc if absent by title+doc_type; skips if already present. This method does **not** currently exist and must be added. |
| `_write_last_response(state, working_messages)` | `crates/brassclaw_engine/orchestrator/default.py` (module-level, alongside `_token_count`) | Scans `working_messages` in reverse and writes the first non-empty assistant message's `content` to `state["_last_response"]`. Called before every terminal `return complete_result(...)` inside `run_loop`'s for loop, and before the max-iterations return. Used by `run_decomposition_loop` for subtask context forwarding. If no assistant message exists (budget-exhausted before any LLM step), the function is a no-op — callers read with `.get("_last_response", "")`. |

---

## 3. Implementation Approach

### 3.1 v1 Removal (REQ-1.x)

**Pattern:** Remove v1 code in a staged order to keep CI green at each commit boundary.

**Stage order:**
1. Remove the `ENGINE_V2` env gate. Set `engine_v2 = true` unconditionally in `AgentConfig`. This keeps both code paths compiling.
2. Remove v1 branches from `agent_loop.rs`, `router.rs`, `thread_ops.rs`, `app.rs`.
3. Delete the v1 source files listed in §2.1 (one directory at a time).
4. Delete the v1 registry entries listed in §2.1.
5. Remove `is_engine_v2_enabled()`, `engine_v2` field from `AgentConfig`.

**Trust-based attenuation (`src/skills/attenuation.rs`):** This module maps `SkillTrust` values to tool permission sets for the v1 dispatcher. It is removed entirely. The v2 engine uses `PolicyEngine` + capability leases for the same purpose. Callers of `attenuation.rs` in the v1 path are removed alongside their call sites.

**Post-condition verification:** After each stage, run:
```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### 3.2 Token Budget System (REQ-2.x)

#### 3.2.1 Settings Changes

**`src/settings.rs`:** Add `max_prompt_tokens: usize` to `AgentSettings`:

```rust
pub struct AgentSettings {
    // ... existing fields ...
    /// Hard ceiling on total tokens assembled per LLM prompt turn.
    /// Default: 8192 (home-use profile), 131072 (server profile).
    #[serde(default = "default_max_prompt_tokens")]
    pub max_prompt_tokens: usize,
    /// Minimum confidence score for reusing a cached plan.
    /// Plans below this threshold are discarded and a fresh planning call is made.
    /// Range: 0.0–1.0. Default: 0.6.
    #[serde(default = "default_plan_confidence_threshold")]
    pub plan_confidence_threshold: f64,
    /// Override for CodeAct (Python REPL) enabling/disabling.
    /// None = auto-detect by backend (local → off, cloud → on).
    /// Some(true) = always enable. Some(false) = always disable.
    #[serde(default)]
    pub codeact_enabled: Option<bool>,
}

fn default_max_prompt_tokens() -> usize {
    8192
}

fn default_plan_confidence_threshold() -> f64 {
    0.6
}
```

Also update `SkillsSettings`:
- Lower `default_skills_max_context_tokens()` from `4000` to `2048`.

> **`AgentSettings::default()` must be updated.** `AgentSettings` in `settings.rs` has an **explicit** `impl Default for AgentSettings` block that exhaustively lists every field. Adding `max_prompt_tokens`, `plan_confidence_threshold`, and `codeact_enabled` to the struct without also updating the `Default` impl causes a compile error: `"missing field 'max_prompt_tokens' in initializer of 'AgentSettings'"`. Update the `Default` impl to include all three new fields with their defaults: `max_prompt_tokens: default_max_prompt_tokens()`, `plan_confidence_threshold: default_plan_confidence_threshold()`, `codeact_enabled: None`. This is the same class of exhaustive-constructor issue as `AgentConfig::for_testing()` (noted separately in §2.1).

> **Naming clarification:** `SkillsSettings.max_context_tokens` (changed here from 4000 → 2048) is the **global skill budget ceiling** — the total token budget available for all selected skills combined. This is distinct from `ActivationCriteria.max_context_tokens` (changed in §3.4.3 from 2000 → 0), which is the **per-skill declared budget** — how many tokens an individual skill claims. The per-skill value defaults to 0 (excluded) when not declared in the skill manifest frontmatter. The global ceiling defaults to 2048 when not configured by the user. These are two separate defaults in two separate structs (`src/settings.rs` vs `crates/brassclaw_skills/src/types.rs`).

**`src/config/agent.rs`:** Add `max_prompt_tokens: usize` and `plan_confidence_threshold: f64` to `AgentConfig`, resolved via `db_first_or_default` reading their respective `AgentSettings` fields.

**`migrations/V28__prompt_token_ceiling.sql`:** Inserts the default DB settings row for `agent.max_prompt_tokens = 8192`.

> **Server deployment note:** The migration inserts `8192` as the initial DB value because migrations run before any profile TOML is applied. Server operators (and server/server-multitenant profile builds) **must** apply the profile override (`max_prompt_tokens = 131072` in `profiles/server.toml`) after migration, which the existing profile-loading mechanism already applies at startup. To make this safe-by-default: server profile startup code should emit a startup warning if `agent.max_prompt_tokens <= 16384` and `database_backend == "postgres"`, prompting the operator to verify their profile is loaded.

#### 3.2.2 Wire Skill Budget Through Engine v2

**Problem (finding 4.1):** `SkillsConfig.max_context_tokens` is read by the v1 `agent_loop.rs` when calling `prefilter_skills()`. Engine v2's Python orchestrator calls `__list_skills__()` host function which does **not** receive the per-user configured budget — it uses the hardcoded `MAX_SKILL_CONTEXT_TOKENS` constant in `selector.rs`.

**Fix:** In the engine v2 path, `handle_list_skills` in `orchestrator.rs` returns **all** skill docs and the Python orchestrator performs selection via its `select_skills()` helper. The budget enforcement is therefore a Python-level change: the existing call at `orchestrator/default.py` hardcodes `max_tokens=6000` — this must be updated to read from the runtime config:

```python
# Before (hardcoded, ignores user setting):
active_skills = select_skills(all_skills, goal, max_candidates=3, max_tokens=6000)

# After (reads from ThreadConfig propagated into Python config dict):
skill_budget = config.get("skill_token_budget", 2048)
active_skills = select_skills(all_skills, goal, max_candidates=3, max_tokens=skill_budget)
```

Also update the `select_skills()` **function signature default** from `max_tokens=6000` to `max_tokens=2048` so the signature matches the new default budget. While the only current call site passes an explicit value, a stale `6000` default would silently apply the wrong budget if a future call site omits the parameter.

The `__list_skills__()` Rust host function does **not** gain a `token_budget` parameter — adding Rust-side enforcement would duplicate what Python's `select_skills()` already does and add unnecessary complexity. Budget enforcement stays in the Python layer.

`ThreadConfig` (in `crates/brassclaw_engine/src/types/thread.rs`) needs two new fields:
```rust
pub struct ThreadConfig {
    // ... existing ...
    /// Total prompt token ceiling for this thread.
    pub max_prompt_tokens: usize,
    /// Skill context sub-budget (subset of max_prompt_tokens).
    pub skill_token_budget: usize,
}
```

These are populated in `src/bridge/router.rs` when constructing `ThreadConfig` from the user's `AgentConfig` + `SkillsConfig`. The Monty VM exposes `ThreadConfig` to Python as the `config` dict; `skill_token_budget` is accessed via `config.get("skill_token_budget", 2048)`.

**`MAX_SKILL_CONTEXT_TOKENS` constant in `selector.rs`:** The `pub const MAX_SKILL_CONTEXT_TOKENS: usize = 4000` in `crates/brassclaw_skills/src/selector.rs` is only used by the v1 `agent_loop.rs` call site (which is removed in §3.1) and by tests. After v1 removal: update this constant to `2048` to match the new default and prevent stale references in any remaining tests. The `prefilter_skills()` function itself takes `max_context_tokens: usize` as a runtime parameter and is unaffected.

#### 3.2.3 Token Guard (`token_guard.rs`)

`TokenGuard` is called by the Python orchestrator via a new `__apply_token_guard__(parts)` host function before the first `__llm_complete__` call each turn.

`PromptParts` (passed as a dict from Python):
```
{
  "system_prompt": str,
  "plan_anchor": str,          # never dropped
  "skills": [{"name", "content", "score"}],
  "memory_docs": [{"id", "content", "score", "type"}],
  "tool_schemas": [{"name", "description", "params"}],
  "conversation_history": [{"role", "content"}],
  "budget": int,               # max_prompt_tokens
}
```

**Token accounting:** The guard computes the total token count by summing `system_prompt` (which already contains the plan anchor text), skills, memory docs, tool schemas, and conversation history. The plan anchor's tokens count against the budget (as part of the system prompt) — they are not a free reservation outside it. The plan anchor is marked **non-droppable**: it is never removed or truncated during degradation. This means the plan anchor effectively reduces the budget available for droppable content (skills, memory docs, history).

Note: The `plan_anchor` field in `PromptParts` is passed separately **as a read-only reference copy**. It is **not** used algorithmically by the Rust guard for drop protection. Drop protection is handled structurally: degradation step 4 removes only system prompt sections wrapped in sentinel comments (`<!-- droppable-start -->` / `<!-- droppable-end -->`); the plan anchor section in the template has no sentinel, so it is inherently non-droppable. The `plan_anchor` field exists for two purposes: (a) the guard can validate that the system prompt actually contains the expected plan text (defensive check), and (b) the `dropped` return value can report the plan anchor's token cost separately for observability. The guard does **not** add `plan_anchor` tokens to the total — it only sums `system_prompt` tokens (which already contain the injected plan anchor text) plus skills, memory docs, tool schemas, and conversation history. Under the 8,192 default, 8,192 − 1,024 = **7,168 tokens** remain for skills (2,048 max), memory docs, tools, and conversation history.

**Plan anchor validation failure behavior:** If the defensive check finds that `plan_anchor` text is not a substring of `system_prompt` (indicating the anchor was not injected into the template correctly), the guard **logs a `warn!` and continues** — it must not block execution or return an error. The system prompt is used as-is. This avoids a guard failure cascading into a broken thread. The warning text should identify the thread ID so it can be correlated in logs: `warn!(thread_id = %..., "plan_anchor not found in system_prompt — anchor may not have been injected")`.

Degradation order:
1. Drop memory docs with lowest score first (exclude `DocType::Plan`)
2. Drop skills with lowest score first
3. Truncate dynamically-registered tool action descriptions (MCP tools, user-installed extensions) to 60 words. Built-in tools are already statically capped at 60 words by §3.4.4; this step targets only tools whose descriptions bypass the static cap.
4. Remove system prompt postamble / non-essential sections (sections wrapped in `<!-- droppable-start -->` / `<!-- droppable-end -->` sentinel comments in the template)
5. Drop oldest conversation history messages (never drop the latest user message or plan anchor)

Returns `{"dropped": [...], "fits": bool}`. 

**Decomposition trigger — step 0 only.** If `fits == False` after all degradation **and the current step index is 0** (no LLM calls have been made yet for this task), the Python orchestrator triggers task decomposition (REQ-6.9). The trigger sequence is:

1. **Depth guard:** If `config.get("decomposition_depth", 0) >= 1`, decomposition is blocked (REQ-6.9.4 / C9). Call `__transition_to__("failed", "Prompt too large even after full degradation; subtask cannot be further decomposed.")` and `return complete_result(state, "failed")`. Do NOT call `run_miniplan_call`.

2. **Clear stale plan doc:** Call `state.pop("active_plan_doc_id", None)` before decomposing. `run_planning_phase()` may have already retrieved a regular cached plan (setting `state["active_plan_doc_id"]` to that plan's doc_id). That plan was never executed (the prompt was too large to use it). If this doc_id were left in state, `run_decomposition_loop` would capture it as `decomp_plan_doc_id` and — believing it is a cached decomposition plan — skip saving the new decomposition doc and track confidence against the wrong document. Clearing it ensures `run_decomposition_loop` treats this as a fresh decomposition and saves a new `DocType::Plan` MemoryDoc with `is_decomposition: true`.

3. **Decompose:** Call `run_miniplan_call(goal)` directly (defined in §3.5.2) and enter the subtask execution loop described in §3.5.6. No additional host function is needed; the existing `__llm_complete__`, `__transition_to__`, and `__save_plan_doc__` host functions are sufficient.

On `step > 0` (the conversation is partially executed — tool calls have already been made), decomposition mid-execution would be semantically wrong. Instead, the orchestrator proceeds with the maximally-degraded prompt and logs a `warn!` via `__emit_event__("token_budget_overflow", step=step)`. Conversation history is already dropped to a minimum by degradation step 5, keeping only the system prompt, plan anchor, and the latest user message. This is the best achievable behavior once execution has started; the user is informed at the next response.

**Python integration in `run_loop` — two distinct call sites:**

**At step 0 (before `append_system_append` for skills and memory docs):** The guard must be called BEFORE injecting skills and memory docs into `working_messages`. At this point `active_skills` and `docs` are local Python lists (returned by `__list_skills__()` / `select_skills()` and `__retrieve_docs__()`), so they can be passed as separate fields and filtered based on the `dropped` result. The guard decides which skills and docs to drop; Python then only calls `append_system_append` with the survivors. The `PromptParts` dict at step 0 contains:
- `"system_prompt"`: the system message already in `working_messages` (Rust-built, includes plan anchor)
- `"plan_anchor"`: `state.get("plan_anchor_text", "")` — the plan anchor text written by Rust on `refresh_system_prompt`
- `"skills"`: **reshaped** from `active_skills` — each entry must be `{"name": str, "content": str, "score": float}`. `active_skills` from `select_skills()` has the structure `{"metadata": {"name": ..., "activation": {...}}, "content": "..."}` — Python must remap before calling the guard: `{"name": s["metadata"]["name"], "content": s.get("content",""), "score": 1.0}`. The score is set to `1.0` for all (they're already filtered/ranked by `select_skills`); the guard drops skills in reverse list order (lowest priority last in the pre-sorted list).
- `"memory_docs"`: the `docs` list from `__retrieve_docs__` — each entry already has `"doc_id"`, `"content"`, `"type"` (shape extended from the `handle_retrieve_docs` change in §3.5.2). Remap to `{"id": d["doc_id"], "content": d.get("content",""), "score": 0.5, "type": d.get("type","")}`.
- `"tool_schemas"`: tool action definitions (read from `__get_actions__()` result if needed; may be left empty and measured via system_prompt total)
- `"conversation_history"`: the non-system messages in `working_messages`
- `"budget"`: `config.get("max_prompt_tokens", 8192)`

After the guard call at step 0: iterate over `guard_result["dropped"]`; filter out dropped skills from `active_skills` (match by `name`) and dropped memory docs from `docs` (match by `doc_id`) before calling `append_system_append`. If `fits == False` (even after dropping all skills and docs), trigger decomposition using the three-step sequence above (depth guard → clear stale plan doc → `run_miniplan_call`).

**At step > 0 (before every `__llm_complete__` call):** Skills and memory docs are already embedded in system messages and cannot be dropped. Pass the guard only:
- `"system_prompt"`: full system message from `working_messages` (includes embedded skills/docs)
- `"plan_anchor"`: `state.get("plan_anchor_text", "")`
- `"skills"`, `"memory_docs"`, `"tool_schemas"`: empty lists (nothing to drop separately)
- `"conversation_history"`: non-system messages from `working_messages`
- `"budget"`: `config.get("max_prompt_tokens", 8192)`

If `fits == False` at step > 0 (only possible if conversation history is extremely long and resisted compaction), emit a warning event and proceed — the guard has already applied degradation step 5 (drop oldest conversation history). Dropped conversation history entries are identified in `guard_result["dropped"]` by their index in `conversation_history`; Python removes those messages from `working_messages` before calling `__llm_complete__`.

> **Implementation note for Planning step:** The `guard_result["dropped"]` list contains dicts like `{"type": "skill"|"memory_doc"|"history", "id": <name or index>}`. For history entries, `"id"` is the zero-based index into the `conversation_history` array passed to the guard — **not** a raw index into `working_messages`. `working_messages` contains both system messages and non-system messages; `conversation_history` contains only the non-system messages. Python must resolve the mapping explicitly: collect `non_system = [m for m in working_messages if m.get("role") not in ("System", "system")]`, then for each dropped history entry with index `i`, remove `non_system[i]` from `working_messages`. The Rust guard must guarantee that dropped history entries are always the oldest messages and never include the last user message.

**Planning calls are exempt** — they are made from `run_planning_phase()`, which runs before `run_loop`'s step loop, so `__apply_token_guard__` is never called for them by code structure. No explicit check of `is_planning_call` is needed in the Python guard path.

#### 3.2.4 UI — New Settings Field

The Settings → Agents tab in `src/channels/web/features/settings/mod.rs` already renders `SkillsSettings.max_context_tokens` as "Skill context token size". Add a new numeric field "Max total prompt tokens" immediately below it, bound to `AgentSettings.max_prompt_tokens`.

Server-side validation in the settings handler: reject saves where `max_prompt_tokens < skill_context_tokens` with a 400 response and a user-facing message.

### 3.3 Tier 0 — Suppressed CodeAct for Local Backends (REQ-3.x)

#### 3.3.1 Backend Detection

`openai_compatible` as a backend name is not a reliable local indicator — it is used by cloud services like Together.ai, Groq, Anyscale, and Fireworks. Treating it as inherently local would silently suppress CodeAct for cloud users on those platforms.

The correct approach is an explicit **`is_local: bool`** flag on the custom provider configuration, set by the user at provider registration time and propagated into `PlatformInfo`:

**`src/settings.rs` — `CustomLlmProviderSettings`:** Add `is_local: bool` (default `false`). Convention: Ollama's built-in provider record sets `is_local: true`; all other built-in providers (Anthropic, OpenAI, Gemini, NEAR AI) set `false`; user-defined providers default to `false` and can opt in.

**`src/bridge/llm_adapter.rs`:** When building `PlatformInfo`, populate a new field `pub is_local_backend: bool` derived from the active provider's `is_local` flag.

**`crates/brassclaw_engine/src/executor/prompt.rs`:** Add `pub is_local_backend: bool` to `PlatformInfo` (default `false`).

The helper in `loop_engine.rs` becomes:

```rust
fn should_use_tier0(platform_info: Option<&PlatformInfo>, codeact_override: Option<bool>) -> bool {
    match codeact_override {
        Some(true) => false,   // user explicitly enabled CodeAct
        Some(false) => true,   // user explicitly disabled CodeAct
        None => platform_info.map(|p| p.is_local_backend).unwrap_or(false),
    }
}
```

`codeact_override` is sourced from `ThreadConfig.codeact_enabled` (see §4.1).

**Settings UI:** The "Enable CodeAct for local models" toggle in the Advanced section (§4.7) writes to `agent.codeact_enabled`. The provider registration form (Settings → LLM) exposes a "Local model" checkbox that sets `CustomLlmProviderSettings.is_local`.

**Loopback auto-detection:** To prevent users from accidentally receiving the full CodeAct prompt on a local model, the provider registration handler in `src/channels/web/features/settings/mod.rs` applies the following heuristic when `is_local` is not explicitly set by the user:

```rust
fn infer_is_local(base_url: Option<&str>, adapter: &str) -> bool {
    if adapter == "ollama" {
        return true;
    }
    match base_url {
        Some(url) => {
            let lower = url.to_lowercase();
            lower.contains("://localhost")
                || lower.contains("://127.0.0.1")
                || lower.contains("://[::1]")
                || lower.contains("://0.0.0.0")
        }
        None => false,
    }
}
```

**When the heuristic fires:** `infer_is_local()` is applied **server-side when pre-populating the provider registration form** (the GET endpoint that returns form defaults), not after form submission. The form renders with the "Local model" checkbox pre-checked when the heuristic returns `true`. The user sees the pre-populated state and can uncheck it before saving. On form submission (POST), the server accepts the explicit `is_local` boolean from the submitted payload without re-running inference — the user's explicit choice is final.

This ensures users with loopback-addressed cloud relays can uncheck the box, while users who don't know about the flag get the correct default automatically. The Ollama built-in provider record always has `is_local: true` regardless of this heuristic.

#### 3.3.2 Compact System Prompt (`tier0_system_prompt.md`)

New file at `crates/brassclaw_engine/prompts/tier0_system_prompt.md`. Target: ≤ 800 tokens for the static portion (leaving 224 tokens for the plan anchor injection, summing to ≤ 1,024 total).

Structure:
```
# BrassClaw — Your AI Assistant

You are BrassClaw. Use tools to complete tasks. Call one tool at a time.
...
{plan_anchor}
```

**`build_tier0_system_prompt(platform: Option<&PlatformInfo>, plan_anchor: Option<&str>) -> String`** in `tier0_prompt.rs`:
- Renders the static template
- Injects `{plan_anchor}` section when a plan is active (≤ 200 tokens, see §3.5)
- Injects platform identity (version, model name) from `PlatformInfo` when `Some`; omits the platform section when `None`

> **Signature note:** The first parameter is `Option<&PlatformInfo>` — not `&PlatformInfo` — for three reasons: (1) `ExecutionLoop.platform_info` is `Option<PlatformInfo>`, so `.as_ref()` yields `Option<&PlatformInfo>`; (2) this matches the existing `build_codeact_system_prompt_with_docs` signature at `prompt.rs:148`; (3) the unit test in §5.2 calls `build_tier0_system_prompt(None, None)`, which only compiles with `Option<&PlatformInfo>`. A `&PlatformInfo` parameter would require unwrapping at every call site and a special-case in the unit test.

#### 3.3.3 `refresh_system_prompt` in `loop_engine.rs`

Replace the unconditional call to `build_codeact_system_prompt_with_docs()` with a branch using `should_use_tier0()` (defined in §3.3.1). **First, read the plan anchor from the checkpoint's persisted state** (which is the Python `state` dict saved by `__save_checkpoint__`):

```rust
let codeact_override = self.thread.config.codeact_enabled;

// Read plan state that Python saved via __save_checkpoint__.
// checkpoint.persisted_state IS the Python state dict (working_messages,
// plan_steps, plan_current_step, etc. are all top-level keys).
let plan_steps: Vec<String> = checkpoint
    .persisted_state
    .get("plan_steps")
    .and_then(|v| serde_json::from_value(v.clone()).ok())
    .unwrap_or_default();
let plan_current_step: usize = checkpoint
    .persisted_state
    .get("plan_current_step")
    .and_then(|v| v.as_u64())
    .map(|v| v as usize)
    .unwrap_or(0);
let plan_anchor: Option<String> = if plan_steps.is_empty() {
    None
} else {
    Some(
        crate::executor::planner::PlanAnchor {
            steps: plan_steps,
            current_step: plan_current_step,
        }
        .to_prompt_section(),
    )
};

let use_tier0 = should_use_tier0(self.platform_info.as_ref(), codeact_override);
let system_prompt = if use_tier0 {
    build_tier0_system_prompt(self.platform_info.as_ref(), plan_anchor.as_deref())
} else {
    build_codeact_system_prompt_with_docs(...)
};

// Write plan_anchor_text back into the Python persisted state so that
// state["plan_anchor_text"] is accessible to __apply_token_guard__.
// For Tier 0: write the actual plan anchor text (it IS in the system prompt).
// For CodeAct: write an empty string — the plan anchor is NOT injected into
// the CodeAct prompt (plan-anchor injection is a Tier 0 feature only).
// Without this distinction, __apply_token_guard__'s defensive check would
// emit a warn! every turn for CodeAct backends with an active plan, because
// the plan anchor text would not be found in the CodeAct system prompt.
let plan_anchor_text_for_state = if use_tier0 {
    plan_anchor.as_deref().unwrap_or("").to_string()
} else {
    String::new()
};
if let Some(obj) = checkpoint.persisted_state.as_object_mut() {
    obj.insert(
        "plan_anchor_text".to_string(),
        serde_json::Value::String(plan_anchor_text_for_state),
    );
}
```

> **Field path notes:**
> - `self.thread.config.codeact_enabled` — `ExecutionLoop` does not have a `thread_config` field; the config is accessed via the owned `Thread` at `self.thread.config` (`ThreadConfig`).
> - `self.platform_info.as_ref()` — `platform_info: Option<PlatformInfo>` is a plain struct field; `.as_ref()` gives `Option<&PlatformInfo>` which matches the `should_use_tier0` parameter type. `.as_deref()` would only compile if `PlatformInfo` implemented `Deref`, which it does not.
> - `checkpoint.persisted_state` structure: the Python orchestrator calls `__save_checkpoint__(state, counters)` where `state` is its runtime state dict. The Rust handler serializes `state` as the `persisted_state` blob. `working_messages`, `plan_steps`, `plan_current_step`, `active_plan_doc_id`, `_last_response`, and `plan_anchor_text` are all top-level keys inside `persisted_state` (not nested under a `"state"` sub-key). This matches the existing `RuntimeCheckpoint::has_working_messages_system_prompt()` which accesses `self.persisted_state.get("working_messages")` directly (line 37 of `loop_engine.rs`).
> - The `plan_anchor_text` write-back **must happen before** `store_runtime_checkpoint(checkpoint)` is called, so the written value is included in the saved checkpoint. If `refresh_system_prompt` does not call `store_runtime_checkpoint` (because no system prompt change was detected), the `plan_anchor_text` write is still correct — `persisted_state` is the in-memory view; a subsequent `__save_checkpoint__` call from Python will overwrite it anyway. The write-back ensures the value is available to the current Python execution session even if the checkpoint is not immediately re-persisted.

`should_use_tier0()` reads the explicit `PlatformInfo.is_local_backend` flag (populated from the provider's `is_local: bool` setting — see §3.3.1). `is_codeact_system_prompt()` and related prompt helpers in `prompt.rs` continue to work for cloud backends unchanged.

#### 3.3.4 Planning Calls Always Text-Only (REQ-3.5)

The planning call path in the Python orchestrator passes `{"force_text": true, "is_planning_call": true}` in the `__llm_complete__` config. **CodeAct preamble injection is bypassed naturally** because planning calls pass explicit custom messages (a system message like "List steps to complete the task..." and a user message containing the goal) — not the thread's accumulated messages. The Rust host's `refresh_llm_messages_for_current_surface()` checks whether the message list contains a CodeAct system prompt (via `is_codeact_system_prompt()`) and returns early if none is found. Since planning messages never contain a CodeAct prompt, no injection occurs.

`force_text: true` is set to prevent the LLM from returning tool call responses (the Rust handler always provides the full action list to `llm.complete()`, so without `force_text` a local LLM could respond with tool calls instead of a numbered step list). `is_planning_call: true` identifies this call as a planning call on the Rust side (for telemetry and cost-tracking separation). The Python orchestrator does NOT need this flag to bypass `__apply_token_guard__` — planning calls are made from `run_planning_phase()`, which executes *before* `run_loop`'s step loop where `__apply_token_guard__` is applied. The guard is simply never invoked for planning calls by the code structure; no explicit skip is needed at the Python level. The `is_planning_call` flag's value is therefore in Rust-side observability (distinguishing planning LLM calls from execution LLM calls in logs, cost summaries, and traces).

#### 3.3.5 Generalized System Prompt Marker and Upsert Logic

The existing system prompt lifecycle in `loop_engine.rs` and `prompt.rs` relies on `is_codeact_system_prompt()` — a function that checks for `CODEACT_SYSTEM_PROMPT_MARKER` (`<!-- brassclaw:codeact-system-prompt -->`) to detect, update, and upsert the engine-owned system message. A Tier 0 prompt does not carry this marker. Without generalization, Tier 0 threads suffer three failures:

1. **`has_engine_owned_system_prompt()`** returns `false` → the "skip refresh when inputs incomplete" guard at line 293 is never triggered, causing unnecessary prompt rebuilds with partial data.
2. **`upsert_codeact_system_prompt()`** cannot find the Tier 0 system message → at line 253–257, it sees an existing System message (the Tier 0 one from the first refresh), returns `false` without updating. Plan anchor changes are silently lost.
3. **`RuntimeCheckpoint::update_working_messages_system_prompt()`** cannot find the Tier 0 prompt in the checkpoint JSON → the plan anchor is never updated in persisted state. On thread resume, the plan anchor shows the stale step index.

**Fix — add a Tier 0 marker and generalize detection:**

**`prompt.rs`:**

```rust
pub const TIER0_SYSTEM_PROMPT_MARKER: &str = "<!-- brassclaw:tier0-system-prompt -->\n";

/// Returns true if the content is an engine-owned system prompt (CodeAct or Tier 0).
pub fn is_engine_system_prompt(content: &str) -> bool {
    content.starts_with(CODEACT_SYSTEM_PROMPT_MARKER)
        || content.starts_with(TIER0_SYSTEM_PROMPT_MARKER)
        || is_legacy_codeact_system_prompt(content)
}

/// Replace the engine-owned system prompt content. Works for both CodeAct and Tier 0.
/// For CodeAct prompts, delegates to the existing `refresh_codeact_system_prompt()`
/// which preserves user-appended sections. For Tier 0, replaces the full content
/// (Tier 0 has no user-appended sections to preserve).
pub fn refresh_engine_system_prompt(existing_content: &str, new_prompt: &str) -> String {
    if existing_content.starts_with(CODEACT_SYSTEM_PROMPT_MARKER)
        || is_legacy_codeact_system_prompt(existing_content)
    {
        refresh_codeact_system_prompt(existing_content, new_prompt)
    } else {
        new_prompt.to_string()
    }
}

/// Upsert an engine-owned system prompt (CodeAct or Tier 0) into the message list.
pub fn upsert_engine_system_prompt(
    messages: &mut Vec<ThreadMessage>,
    system_prompt: String,
) -> bool {
    if let Some(message) = messages.iter_mut().find(|message| {
        message.role == MessageRole::System && is_engine_system_prompt(&message.content)
    }) {
        let refreshed = refresh_engine_system_prompt(&message.content, &system_prompt);
        if message.content == refreshed {
            return false;
        }
        message.content = refreshed;
        return true;
    }
    if messages.iter().any(|message| message.role == MessageRole::System) {
        return false;
    }
    messages.insert(0, ThreadMessage::system(system_prompt));
    true
}
```

**`build_tier0_system_prompt()` must prepend `TIER0_SYSTEM_PROMPT_MARKER`** to the generated prompt string (same pattern as `build_codeact_system_prompt_inner` prepending `CODEACT_SYSTEM_PROMPT_MARKER` at `prompt.rs:161`).

**`loop_engine.rs` — five call sites to update:**

1. `RuntimeCheckpoint::has_working_messages_system_prompt()` (line 43): change `is_codeact_system_prompt` → `is_engine_system_prompt`.
2. `RuntimeCheckpoint::update_working_messages_system_prompt()` (line 61): change `is_codeact_system_prompt` → `is_engine_system_prompt`. Change `refresh_codeact_system_prompt(content, system_prompt)` → `refresh_engine_system_prompt(content, system_prompt)`.
3. `has_engine_owned_system_prompt()` (line 241): change `is_codeact_system_prompt` → `is_engine_system_prompt`.
4. `refresh_system_prompt()` (line 305–324): replace `build_codeact_system_prompt_with_docs(...)` with the `should_use_tier0()` branch from §3.3.3. Replace `upsert_codeact_system_prompt(...)` → `upsert_engine_system_prompt(...)` at both call sites (lines 312–313, 319–320).

The existing `is_codeact_system_prompt()`, `upsert_codeact_system_prompt()`, and `refresh_codeact_system_prompt()` are retained (they are still used by `is_engine_system_prompt` / `refresh_engine_system_prompt` internally for the CodeAct-specific paths). No callers should use them directly after this change — all external call sites switch to the `*_engine_*` variants.

### 3.4 Local-First Integrations (REQ-4.x)

#### 3.4.1 Local Browser Tool — Playwright MCP

**Registry entry:** `registry/mcp-servers/playwright.json`

```json
{
  "name": "playwright",
  "display_name": "Local Browser",
  "runtime": "mcp_stdio",
  "auto_launch": true,
  "command": "npx",
  "args": ["@playwright/mcp@latest", "--no-file-access"],
  "env": {"PLAYWRIGHT_BROWSERS_PATH": "0"},
  "requires": {"bins": ["node", "npx"]},
  "install_hint": "Install Node.js from https://nodejs.org to enable browser tools.",
  "tags": ["default", "local", "browser"]
}
```

`auto_launch: true` signals the MCP manager (in `src/tools/mcp/`) to attempt to start the MCP server process on first use. If `node`/`npx` are absent, the manager logs a startup warning and marks the capability as `CapabilityStatus::NeedsSetup` with the `install_hint` as the routing hint.

**Capabilities (REQ-4.2):** The `@playwright/mcp@latest` package exposes the following actions that map to REQ-4.2: `navigate` (navigate to URL), `search` (keyword web search via a search engine), `get_text` (extract page text), `screenshot` (take screenshot), `click` (click element by selector), `fill` (fill form field). The implementation step must verify at integration test time that these actions are available from the installed MCP package version. If any are missing, a wrapper action must be added in `src/tools/mcp/playwright_shim.rs`.

**Sandboxing (REQ-4.3):** The Playwright MCP server must not have access to the local filesystem. This is enforced at two levels:

1. **Launch args:** The registry entry includes `"env": {"PLAYWRIGHT_BROWSERS_PATH": "0"}` to prevent user-profile directory access. The `args` array includes `"--no-file-access"` (if supported by the MCP server) to reject `file://` navigation.
2. **URL filtering in MCP manager:** The MCP request interceptor in `src/tools/mcp/` must reject any action whose parameters contain a URL using the `file://` scheme. The check scans **all string-valued parameters** of every outgoing MCP request (not only `navigate`), because future Playwright MCP versions or other MCP servers may accept URLs in parameters named `url`, `href`, `src`, `goto`, etc. The interceptor returns an error response: `"file:// URLs are blocked for security. Use the local_search tool for filesystem access."` This filter applies regardless of the MCP server's own sandboxing support, providing defense in depth.
3. **Localhost access is allowed** for development workflows (REQ-4.3 permits `localhost` and public HTTP/HTTPS).

**`registry/tools/web_search.json`** is removed and replaced by the Playwright MCP server entry above. The existing Brave Search WASM tool is removed from the registry and bundled set.

#### 3.4.2 Built-in Tool Manifests

**`registry/tools/caldav.json`** — CalDAV calendar/to-do (WASM extension). Actions: `list_calendars`, `list_events`, `create_event`, `update_event`, `delete_event`, `list_todos`, `create_todo`, `update_todo`, `delete_todo`. Config: `server_url`, `username`; password in secrets vault. Secrets stored via existing `SecretsStore`. `verify_ssl` defaults to `true` (secure by default). When explicitly set to `false`, the tool emits a startup warning: `"CalDAV SSL verification disabled — connections are vulnerable to MITM attacks."` The `update_*` and `delete_*` actions perform live network calls (like `create_*`) and trigger an immediate cache refresh on success.

**CalDAV background cache (REQ-4.10):** The CalDAV tool must not block agent tool calls on live network fetches. Implementation:

- A background task (`CalDavCacheWorker`) runs alongside the tool extension, refreshing a local in-memory + on-disk cache of calendar data on a configurable interval (default: 15 minutes, stored in `caldav.refresh_interval_secs` setting).
- The cache stores the last successful fetch result per calendar in `~/.brassclaw/cache/caldav/` as JSON files keyed by calendar URL hash. The cache directory and files must be created with restrictive permissions (directory: `0700`, files: `0600`) because they may contain sensitive calendar data (event titles, attendees, locations).
- Agent tool calls (`list_events`, `list_todos`, `list_calendars`) read from the cache and return immediately. Each response includes a `cached_at` timestamp so the agent can inform the user of data freshness.
- All mutation actions (`create_event`, `update_event`, `delete_event`, `create_todo`, `update_todo`, `delete_todo`) perform a live network call (writes cannot be cached) and trigger an immediate cache refresh for the affected calendar on success.
- If the CalDAV server is unreachable during a background refresh, the worker logs a warning and retains the stale cache. The cache entries include a `stale: bool` flag set to `true` when the last refresh failed; this is surfaced in tool responses.
- On first startup with no cache, the worker spawns an initial fetch as a **background task with a 10-second timeout** rather than blocking application startup. While the initial fetch is in progress, the tool is marked `CapabilityStatus::Syncing` (a transitional state rendered as "CalDAV: syncing..." in the UI). Read actions (`list_events`, etc.) during this window return an empty result with `"syncing": true` in the response, allowing the agent to inform the user. If the initial fetch succeeds, the tool transitions to ready. If it fails or times out, the tool is marked `CapabilityStatus::NeedsSetup` with a user-facing message identifying the connection error. This avoids coupling BrassClaw's availability to the CalDAV server (which in a home-use scenario may be a slow NAS waking from sleep).

**`registry/tools/local_search.json`** — Workspace file text search. No external process, built-in Rust implementation in `src/tools/builtin/`. Actions: `search_files`. Scope: workspace root by default; additional paths user-configurable in settings. Whole-filesystem search (`scope: "global"`) is gated by a **user-level settings flag** `local_search.allow_global_scope: bool` (default `false`). When an agent passes `scope: "global"` and the flag is `false`, the action returns an error: `"Global filesystem search is disabled. Enable it in Settings → Tools → Local Search."` This ensures REQ-4.9 compliance: an overly-eager or prompt-injected agent cannot scan the entire filesystem without the user explicitly enabling it.

**`registry/tools/local_notes.json`** — Markdown notes append/read. No external process. Actions: `append_note`, `read_notes`, `search_notes`. Storage path: `~/.brassclaw/notes.md` by default (user-configurable in settings).

#### 3.4.3 Skill Manifest Requirements (REQ-4.5 — Breaking Change)

`ActivationCriteria.max_context_tokens` already has a default of `2000` via `default_max_context_tokens()`. After this change:

- Skills without an explicit `max_context_tokens` declaration in frontmatter use `0` (excluded from injection).
- Change `default_max_context_tokens()` in `crates/brassclaw_skills/src/types.rs` from `2000` to `0`.
- In `prefilter_skills()` in `selector.rs`, add an early filter step **before** the scoring loop that removes any skill with `manifest.activation.max_context_tokens == 0`. This filter runs before any cost accumulation, avoiding the risk of overflow from sentinel values. The warning is emitted at skill **load time** (in the skill registry loader), not in the hot selection path.
- **Python `select_skills()` must also receive the zero-budget pre-filter.** The v2 execution path uses Python for skill selection (not the Rust `prefilter_skills()`), so without a matching Python-side filter, skills with `max_context_tokens: 0` are still injected. Add an early filter at the start of `select_skills()` in `orchestrator/default.py` (before the `scored` list is built):
  ```python
  # Exclude skills that declare max_context_tokens == 0 (no prompt budget)
  skills = [
      s for s in skills
      if s.get("metadata", {}).get("activation", {}).get("max_context_tokens", 0) != 0
  ]
  ```
  Also update the `_skill_token_cost()` fallback default from `2000` to `0` so that if a skill's metadata dict ever lacks the key (e.g., very old stored data), the behavior matches the new Rust default rather than granting an unintentional 2000-token budget:
  ```python
  declared = max(activation.get("max_context_tokens", 0), 1)
  ```
  The `max(..., 1)` clamp is retained for defensive correctness (the `approx > declared * 2` check must not divide by or compare against zero for skills that somehow pass the pre-filter).
- At skill load time (in `src/skills/mod.rs` or the registry discovery pass), emit `warn!("Skill '{}' loaded from {} has no declared max_context_tokens and will not be injected into prompts", name, path)` for any discovered skill with `max_context_tokens: 0`. One warning per skill per process lifetime (use a `HashSet` of already-warned skill names).
- Do **not** use `usize::MAX` or any sentinel cost value — skills with zero budget are removed from the candidate slice before `skill_token_cost()` is ever called on them.

**All bundled skills** under `skills/` must have `max_context_tokens` in frontmatter before release. Budgets:

| Category | Cap |
|---|---|
| Brief behavioral guidance (commit, idea-parking, decision-capture, delegation) | ≤ 128 |
| Tool usage / domain procedure (coding, web-browse, caldav, notes, local-search) | ≤ 256 |
| Setup/onboarding bundles | ≤ 256 per skill in the bundle |

#### 3.4.4 Tool Schema Verbosity (REQ-4.7)

`render_enabled_tool()` in `prompt.rs` already normalizes whitespace in descriptions via `compact_prompt_description()`. Extend this to also enforce a 60-word cap:

```rust
fn compact_prompt_description(description: &str) -> String {
    let words: Vec<&str> = description.split_whitespace().collect();
    if words.len() <= 60 {
        words.join(" ")
    } else {
        words[..60].join(" ") + "..."
    }
}
```

In `crates/brassclaw_engine/src/executor/orchestrator.rs`, the `__get_actions__()` host function that serializes `ActionDef` to Python must also apply the 60-word truncation to `description` before returning.

**On-demand tool registration (REQ-4.7 / C7):** Tools not in the always-present core set must be activated via `tool_activate(name=...)` (already supported by the `Activatable Integrations` pattern). The 1,024-token tool budget constrains always-present tools to ≈ 8–12. Any new local tool added triggers a review of the always-present list.

### 3.5 Plan-First Execution (REQ-6.x)

#### 3.5.1 Data Model — No Changes Required

`DocType::Plan` already exists in `crates/brassclaw_engine/src/types/memory.rs` with retrieval weight `0.3` in `retrieval.rs`. No DB schema change is needed for plan storage — plans are `MemoryDoc` rows with `doc_type = Plan`.

`MemoryDoc.metadata` (JSONB field) carries plan-specific fields as an extension, avoiding schema changes:

```json
{
  "plan_steps": ["step 1", "step 2", ...],
  "confidence": 0.8,
  "goal_keywords": ["install", "package"],
  "execution_count": 3,
  "failure_count": 1,
  "is_decomposition": false,
  "is_template": false
}
```

`is_template: true` is set on plan docs loaded at startup from `docs/internal/plan-templates/` (via `ensure_system_docs()`). Runtime-generated plans always have `is_template: false`. This flag is what `find_plan_template()` uses to distinguish bundled templates from cached runtime plans — the two share the same `DocType::Plan` type and both surface via `__retrieve_docs__`, so without this flag there is no way to tell them apart.

#### 3.5.2 Planning Logic in the Python Orchestrator

New section at the top of `orchestrator/default.py` (`run_loop()` function, before the step loop):

> **`__retrieve_docs__` return-shape note:** The current `handle_retrieve_docs` in `orchestrator.rs` returns dicts with keys `"type"`, `"title"`, `"content"`. It does **not** return `"doc_id"` or `"metadata"`. The plan retrieval helpers below require all five fields. `handle_retrieve_docs` must be extended to also return `"doc_id": d.id.0.to_string()` and `"metadata": d.metadata` — keeping the existing `"type"`, `"title"`, `"content"` keys unchanged so that `format_docs()` (which reads `"type"` and `"content"`) continues to work. The `"type"` key uses Rust's `{:?}` Debug output: `DocType::Plan` → `"Plan"`, `DocType::Skill` → `"Skill"`, etc. The Python helpers use `doc.get("type") == "Plan"` to match this.

**Helper: `find_plan_template(docs, goal)`**

```python
def find_plan_template(docs, goal):
    """
    Find a bundled plan template from the retrieved docs.
    Returns the first doc with is_template=True in its metadata, or None.
    Template confidence is always >= threshold (bundled templates are authoritative).

    docs is the list returned by __retrieve_docs__. Each doc has keys:
    "type" (Debug repr e.g. "Plan"), "doc_id", "title", "content", "metadata".
    """
    for doc in docs:
        meta = doc.get("metadata", {})
        if meta.get("is_template") and doc.get("type") == "Plan":
            steps = meta.get("plan_steps", [])
            if steps:
                return {"id": doc["doc_id"], "steps": steps, "confidence": meta.get("confidence", 1.0)}
    return None
```

**Helper: `find_cached_plan(docs, goal)`**

```python
def find_cached_plan(docs, goal):
    """
    Find the highest-confidence cached runtime plan from the retrieved docs.
    Only considers docs with is_template=False (or missing) in metadata.
    Returns a dict with keys: id, steps, confidence, is_decomposition.
    Returns None if no suitable cached plan is found.

    is_decomposition is preserved in the return value so that run_planning_phase
    can return source="decompose" for cached decomposition plans — this routes
    them through the subtask execution loop (§3.5.6) rather than treating the
    stored sub-goal strings as direct execution steps.

    docs is the list returned by __retrieve_docs__. Each doc has keys:
    "type" (Debug repr e.g. "Plan"), "doc_id", "title", "content", "metadata".
    """
    best = None
    for doc in docs:
        meta = doc.get("metadata", {})
        if doc.get("type") == "Plan" and not meta.get("is_template"):
            steps = meta.get("plan_steps", [])
            conf = meta.get("confidence", 0.0)
            if steps and (best is None or conf > best["confidence"]):
                best = {
                    "id": doc["doc_id"],
                    "steps": steps,
                    "confidence": conf,
                    "is_decomposition": meta.get("is_decomposition", False),
                }
    return best
```

**`run_planning_phase(goal, actions, config, state)`**

```python
def run_planning_phase(goal, actions, config, state):
    """
    Returns (plan_steps: list[str], source: str) where source is
    'template', 'cached', 'llm', 'trivial', 'decompose', or 'failed'.

    Side effect: always sets state["active_plan_doc_id"] when a plan doc is
    available (template, cached, or newly saved llm plan). Trivial and failed
    paths leave active_plan_doc_id unset (or None).

    When source is 'failed', plan_steps is [] and __transition_to__("failed", ...)
    has already been called. The caller (run_loop) MUST check for this and return
    immediately — __transition_to__ is a non-blocking state write, not an exception.
    """
    depth = config.get("decomposition_depth", 0)

    if is_trivial(goal, config):
        return ([], "trivial")

    # Try template match
    docs = __retrieve_docs__(goal, 5)
    plan_doc = find_plan_template(docs, goal)
    if plan_doc:
        state["active_plan_doc_id"] = plan_doc["id"]
        return (plan_doc["steps"], "template")

    # Try cached plan
    cached = find_cached_plan(docs, goal)
    if cached and cached["confidence"] >= config.get("plan_confidence_threshold", 0.6):
        if cached["is_decomposition"]:
            if depth >= 1:
                # Decomposition depth limit reached (REQ-6.9.4 / C9).
                # Fail rather than recursing further.
                # Do NOT set state["active_plan_doc_id"] here — the plan was never
                # executed, so recording a failure would unfairly lower its confidence.
                __transition_to__("failed", "Subtask too complex for single-level decomposition.")
                return ([], "failed")
            # Cached decomposition plan is usable — set active_plan_doc_id so that
            # run_decomposition_loop knows not to create a duplicate doc (§3.5.6 step 4)
            # and so complete_result can record confidence on the correct existing doc.
            state["active_plan_doc_id"] = cached["id"]
            return (cached["steps"], "decompose")
        state["active_plan_doc_id"] = cached["id"]
        return (cached["steps"], "cached")

    # Runtime planning call (≤ 200 tokens total)
    steps = run_minimal_planning_call(goal, actions)
    if steps is None:
        if depth >= 1:
            # Decomposition depth limit reached (REQ-6.9.4 / C9).
            # Fail rather than recursing further.
            __transition_to__("failed", "Subtask too complex for single-level decomposition.")
            return ([], "failed")
        # Could not plan (goal too large for budget, or LLM returned unparseable output) — decompose
        subtasks = run_miniplan_call(goal)
        if subtasks is None:
            # run_miniplan_call already called __transition_to__("failed", ...)
            return ([], "failed")
        return (subtasks, "decompose")

    # Cache the new plan and store its doc_id for confidence tracking
    doc_id = __save_plan_doc__(goal, steps, False)
    state["active_plan_doc_id"] = doc_id
    return (steps, "llm")
```

**Integration with `run_loop`:** The `run_loop()` function calls `run_planning_phase()` before the step loop. It must handle all six possible `source` values:

```python
plan_steps, source = run_planning_phase(goal, actions, config, state)
if source == "failed":
    return complete_result(state, "failed")
if source == "decompose":
    # plan_steps is a list of subtask goal strings; route to the subtask
    # execution loop defined in §3.5.6. Do NOT set state["plan_steps"] here —
    # each subtask gets its own plan_steps via its own run_planning_phase call.
    return run_decomposition_loop(plan_steps, goal, actions, config, state)
# For "template", "cached", "llm", and "trivial" sources: initialize plan
# state before the normal step loop.
state["plan_steps"] = plan_steps        # may be [] for "trivial"
state.setdefault("plan_current_step", 0)
```

`__transition_to__("failed", ...)` is a **non-blocking state write** (consistent with all existing `__transition_to__` usage in `default.py` — see the budget exhaustion and signal stop paths which always call `__transition_to__` followed by `return`). It does not raise an exception and does not halt execution. If `run_loop` does not check `source == "failed"`, it will proceed with `plan_steps = []`, which would be misinterpreted as a trivial task. `state["plan_steps"]` must be set before the step loop so that `refresh_system_prompt()` (Rust side) can read it from checkpoint metadata for plan anchor injection (§3.5.3). Using `setdefault` for `plan_current_step` ensures a resumed thread (with a checkpoint-persisted step index) does not reset mid-execution.

**`run_loop` must write `state["_last_response"]` before every terminal `return complete_result(...)` call inside the step loop.** This field is used by `run_decomposition_loop` (§3.5.6) to forward context between subtasks. Because `run_loop` is unaware of whether it is executing as a subtask, the write happens unconditionally; non-decomposition callers simply never read it.

Add the following module-level helper alongside `_token_count` in `orchestrator/default.py`:

```python
def _write_last_response(state, working_messages):
    for msg in reversed(working_messages):
        if msg.get("role") in ("Assistant", "assistant") and msg.get("content"):
            state["_last_response"] = msg["content"]
            return
    # No assistant message found (e.g. budget-exhausted before first LLM step).
    # Leave any prior value untouched; caller reads with .get("_last_response", "").
```

Call `_write_last_response(state, working_messages)` immediately before **every** `return complete_result(...)` statement inside `run_loop`'s `for` loop, and before the `return complete_result(state, "max_iterations")` statement after the loop ends. Do NOT call it before the non-terminal `return` paths (`gate_paused`, `need_approval`, `need_authentication`) — those are paused states, not terminal exits.

> **Why this must be at every call site (not just the loop-exit):** `run_loop` contains approximately 11 distinct `return complete_result(...)` call sites spread across the stop-signal, budget-exhausted (token/time/cost), text-response, text-FINAL, code-FINAL, consecutive-code-error, tool-FINAL, consecutive-action-error, and max-iterations paths. Adding the write at only one location (the literal end of the for loop) would mean subtasks that complete via FINAL() or a plain text response never set `_last_response`, silently breaking context forwarding for all normally-completing subtasks. A shared helper avoids code duplication and makes the intent clear at each call site.

`run_decomposition_loop(subtasks, original_goal, actions, config, state)` is the helper that implements §3.5.6 — it iterates over the subtask goal strings, runs each through the full `run_planning_phase → step-loop` sequence with a fresh `working_messages`, threads summary context forward, and on successful completion conditionally saves the plan.

**Critical: preserve decomposition doc_id across subtask runs.** Each subtask's `complete_result` call (§3.5.4) pops `state["active_plan_doc_id"]` as part of plan confidence tracking. This means that by the time all subtasks finish, `state["active_plan_doc_id"]` is always `None` — even when a cached decomposition plan was loaded. To prevent this from triggering a duplicate save, the decomposition-level doc_id must be captured in a **local variable** before any subtask runs:

```python
def run_decomposition_loop(subtasks, original_goal, actions, config, state):
    # Capture the decomposition-level plan doc_id BEFORE subtasks run.
    # Each subtask's complete_result will pop state["active_plan_doc_id"]
    # (for that subtask's own plan), so we cannot rely on state to preserve
    # the outer decomposition doc_id across the subtask iteration.
    decomp_plan_doc_id = state.get("active_plan_doc_id")

    prior_summary = ""
    for subtask in subtasks:
        # Each subtask runs through the full execution loop with its own fresh state
        # so that plan_steps / plan_current_step / active_plan_doc_id from one subtask
        # do not bleed into the next. The outer state is NOT shared directly.
        subtask_state = {}
        subtask_config = dict(config)
        subtask_config["decomposition_depth"] = config.get("decomposition_depth", 0) + 1

        # Prepend prior subtask output as context (≤ 200 tokens), per §3.5.6.
        # prior_summary comes from subtask_state["_last_response"] written by
        # run_loop before its complete_result call (see §3.5.2 note above).
        goal_with_context = (prior_summary + "\n" + subtask).strip() if prior_summary else subtask

        # Calls the existing run_loop() with a fresh context (no prior messages).
        # Internally this triggers run_planning_phase() → the step loop.
        subtask_result = run_loop([], goal_with_context, actions, subtask_state, subtask_config)

        # Read the last assistant response run_loop wrote to subtask_state (see §3.5.6).
        # _last_response is written by run_loop before complete_result and is always
        # present when the subtask's step loop produced any assistant text.
        prior_summary = subtask_state.get("_last_response", "")
        if _token_count(prior_summary) > 200:
            prior_summary = " ".join(prior_summary.split()[:160])

        if subtask_result.get("outcome") != "completed":
            # Propagate any non-completed subtask outcome immediately — do NOT
            # continue to remaining subtasks and do NOT save the plan doc.
            # Non-completed outcomes include: "failed" (too many errors), "stopped"
            # (stop signal from user), "max_iterations" (loop exhausted without FINAL),
            # and gate/approval states. All are treated as a failure for plan confidence
            # purposes because the decomposition did not fully execute.
            # If this was a cached decomposition plan, restore its doc_id so that
            # complete_result records a failure penalty against it. Without this,
            # state["active_plan_doc_id"] is None (cleared by the subtask's own
            # complete_result) and the cached plan's confidence would never be
            # penalized, causing it to keep being reused despite repeated failures.
            if decomp_plan_doc_id:
                state["active_plan_doc_id"] = decomp_plan_doc_id
            # Propagate the subtask's own outcome so the caller's thread transitions
            # to the correct terminal state (e.g. "stopped" rather than "failed").
            return complete_result(state, subtask_result.get("outcome", "failed"))

    # Conditionally save the decomposition plan using the captured local variable.
    if not decomp_plan_doc_id:
        # Fresh decomposition (from run_miniplan_call, not cache).
        doc_id = __save_plan_doc__(original_goal, subtasks, True)
        state["active_plan_doc_id"] = doc_id
    else:
        # Cached decomposition reuse — restore the original doc_id so that
        # the caller's complete_result tracks confidence on the correct doc.
        state["active_plan_doc_id"] = decomp_plan_doc_id

    # All subtasks succeeded — track confidence and return completion.
    return complete_result(state, "completed")
```

All three arguments to `__save_plan_doc__` are positional (not kwarg — Monty's `_kwargs` parameter is intentionally unused by all host function handlers; keyword-argument calls would go into `_kwargs` and be silently ignored, leaving `is_decomposition` as `false`). It is defined in `orchestrator/default.py` alongside `run_planning_phase`.

**`is_trivial(goal, config)`** — heuristic:
- Contains `?` without multi-step structure
- Word count < `config.get("trivial_word_threshold", 8)`
- Matches a known single-step pattern (regex list)

**Module-level helper (add near the other helpers, before `run_minimal_planning_call`):**

> **Scope note:** `_token_count` MUST be defined at **module level** in `orchestrator/default.py`, not inside `run_minimal_planning_call`. Both `run_minimal_planning_call` and `run_miniplan_call` call it, so it must be accessible to both.

```python
def _token_count(text):
    # Match the Rust convention: len() in Rust returns bytes; Python len() returns
    # Unicode character count. Use encoded byte length so non-ASCII text (CJK, Arabic,
    # emoji) is measured consistently with the Rust selector and TokenGuard.
    return len(text.encode("utf-8")) * 0.25
```

**`run_minimal_planning_call(goal, actions)`**:
- System message: `"List steps to complete the task. Number each step. No preamble."` (≈ 15 tokens)
- User message construction and budget check (per REQ-6.4 — decomposition only when goal *alone* is too large):

```python
system_message = "List steps to complete the task. Number each step. No preamble."
system_tokens = _token_count(system_message)
goal_tokens = _token_count(goal)
if system_tokens + goal_tokens > 200:
    # Goal alone overflows budget — cannot plan, must decompose
    return None
# Try with tool names appended; if too long, silently drop tool names
tool_suffix = "\nTools: " + ", ".join(a["name"] for a in actions)
user_message = goal + tool_suffix
user_tokens = _token_count(user_message)
if system_tokens + user_tokens > 200:
    # Tool names pushed it over — use goal only (goal alone fits per check above)
    user_message = goal
```

- `config = {"force_text": True, "is_planning_call": True, "max_tokens": 200}`
- Parse numbered list from response; strip any preamble before item 1. **Return `None` if the response yields no parseable numbered steps** (empty response, all preamble, or the LLM returned prose rather than a list). This ensures the same fallback path as "goal too large" is taken — the caller in `run_planning_phase` will attempt decomposition (depth 0) or fail (depth 1). Do NOT return an empty list `[]` on parse failure, as that would be silently saved as an empty plan doc and treated as a trivial task by `run_loop`.

**`run_miniplan_call(goal)`** (decomposition):
- System message: `"Break task into subtasks. One per line. Brief."` (≈ 10 tokens by `_token_count`)
- User message: full `goal` verbatim (no truncation per REQ-6.9.1)
- Token counting in this function uses `_token_count(text)` (same helper as `run_minimal_planning_call`) — not bare `len()` — to stay consistent with the Rust byte-count convention.
- `config = {"force_text": True, "is_planning_call": True}` — same flags as `run_minimal_planning_call` (no `max_tokens` cap here since the goal itself may be large)
- Parse 2–4 line-separated subtasks; if output empty/incoherent (no numbered or line-separated items detected), call `__transition_to__("failed", "Could not decompose task into subtasks.")` and **return `None`**. The caller checks for `None` and returns `([], "failed")` — the orchestrator's main `run_loop` handles the `"failed"` source by skipping execution.

#### 3.5.3 Plan Anchor Injection (REQ-6.7)

`PlanAnchor` struct in new `executor/planner.rs`:
```rust
pub struct PlanAnchor {
    pub steps: Vec<String>,
    pub current_step: usize,
}

impl PlanAnchor {
    pub fn to_prompt_section(&self) -> String { ... }  // ≤ 200 tokens; summarizes if needed
}
```

The plan anchor is built from `checkpoint.persisted_state["plan_steps"]` and `checkpoint.persisted_state["plan_current_step"]` (see §3.3.3 for the complete Rust code that reads these keys and constructs the `PlanAnchor`). The anchor is passed to `build_tier0_system_prompt()` for Tier 0 (local) backends only. Cloud/CodeAct backends do **not** receive plan anchor injection — `plan_anchor_text_for_state` is set to an empty string for CodeAct (§3.3.3), keeping C6 ("Cloud path unaffected") intact.

Per-turn: the Python orchestrator updates `state["plan_current_step"]` after each tool call succeeds. The Rust host reads this from the checkpoint on `refresh_system_prompt()`.

#### 3.5.4 Plan Confidence Tracking (REQ-6.6)

On thread completion, the Python orchestrator reuses the **existing** `__record_skill_usage__(doc_id, success)` host function — no new host function is introduced. Plan docs share the same `memory_docs` table and the same `SkillTracker::record_usage()` call path. The `doc_id` recorded here is the ID of the `DocType::Plan` MemoryDoc that was active for this thread (stored in `state["active_plan_doc_id"]` at planning time).

> **Skill-tracking background (existing code):** `__record_skill_usage__` is declared as an available host function in the Python header (line 20 of `default.py`) but is not currently called from Python — skill usage tracking today is performed exclusively by the Rust side in `mission.rs` after thread completion events, using `thread.active_skills()`. Plan docs are NOT in `active_skills`; they are tracked via `state["active_plan_doc_id"]`. Therefore the Python side MUST call `__record_skill_usage__` to implement plan confidence tracking; the existing Rust-side path will not pick it up.

**Call site: add plan tracking to `complete_result`.** `complete_result` (at `default.py:714`) is the single terminal return helper used by every terminal exit path in `run_loop` and `run_decomposition_loop` (gate_paused / need_approval / need_authentication returns bypass it and should NOT trigger plan tracking since the thread is paused, not terminal). Add plan tracking at the top of `complete_result` before building the result dict:

```python
def complete_result(state, outcome, response=None, error=None, extra=None):
    # Track plan confidence on terminal exit if a plan doc was active this run.
    plan_doc_id = state.get("active_plan_doc_id")
    if plan_doc_id:
        __record_skill_usage__(plan_doc_id, outcome == "completed")
        # Clear so resumed or sub-called threads don't double-count the same doc.
        state.pop("active_plan_doc_id", None)
    result = {"outcome": outcome, "state": state}
    ...
```

This design automatically handles all terminal cases: each inner `run_loop` call (for subtasks within `run_decomposition_loop`) tracks its subtask plan when it completes; the outer decomposition plan is tracked when `run_decomposition_loop` sets `state["active_plan_doc_id"]` to the decomposition doc_id and then calls `return complete_result(state, "completed", ...)` after all subtasks succeed.

**Required change to `skill_tracker.rs`:** The current `record_usage()` method explicitly rejects non-Skill docs (`if doc.doc_type != DocType::Skill { return Err(...) }`). This gate must be relaxed to also accept `DocType::Plan`. Change the type check to:

```rust
if !matches!(doc.doc_type, DocType::Skill | DocType::Plan) {
    return Err(EngineError::Skill {
        reason: format!("doc {} is not a skill or plan (type: {:?})", doc_id.0, doc.doc_type),
    });
}
```

The `V2SkillMetadata` deserialization that follows the type check operates on the `metadata` JSONB field, which for Plan docs uses a different schema (§3.5.1). Add a branch after the type check: if `doc.doc_type == DocType::Plan`, update metadata directly (plan docs do not use `V2SkillMetadata`) and save, then return early. The Skill path remains unchanged. The full Plan branch:

```rust
if doc.doc_type == DocType::Plan {
    let mut meta = doc.metadata.clone();
    let obj = meta.as_object_mut().ok_or_else(|| EngineError::Skill {
        reason: format!("plan doc {} has non-object metadata", doc_id.0),
    })?;

    let exec_count = obj.entry("execution_count")
        .or_insert(serde_json::json!(0))
        .as_u64()
        .unwrap_or(0) + 1;
    obj.insert("execution_count".into(), serde_json::json!(exec_count));

    if !success {
        let fail_count = obj.get("failure_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) + 1;
        obj.insert("failure_count".into(), serde_json::json!(fail_count));
    }

    // Update confidence: success rate with dampening to avoid rapid swings.
    // Formula: confidence = 1.0 - (failure_count / (execution_count + 1))
    // Clamped to [0.0, 1.0]. The +1 in the denominator is a damping term —
    // exec_count is already >= 1 here (just incremented), so division by zero
    // cannot occur. The +1 prevents a single failure from instantly dropping
    // confidence from 1.0 to 0.0, giving new plans a small grace period.
    let fail_count = obj.get("failure_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let confidence = 1.0 - (fail_count as f64 / (exec_count + 1) as f64);
    let confidence = confidence.clamp(0.0, 1.0);
    obj.insert("confidence".into(), serde_json::json!(confidence));

    // Save the updated doc back to the store (mirrors the Skill path convention:
    // &MemoryDoc reference, and updated_at refreshed on every mutation).
    let updated_doc = MemoryDoc {
        metadata: meta,
        updated_at: chrono::Utc::now(),
        ..doc
    };
    self.store.save_memory_doc(&updated_doc).await.map_err(|e| EngineError::Skill {
        reason: format!("failed to save plan doc {}: {e}", doc_id.0),
    })?;
    return Ok(());
}
```

The confidence threshold for plan reuse is `config.get("plan_confidence_threshold", 0.6)` read from `ThreadConfig` in the orchestrator.

#### 3.5.5 Plan Templates (REQ-6.3)

Plan templates are `DocType::Plan` MemoryDocs loaded at startup from `docs/internal/plan-templates/*.md` (or bundled inline in `src/bridge/router.rs` alongside `load_bundled_skills()`). They are inserted into the shared system project's memory store via `MemoryStore::ensure_system_docs()` at startup.

Template format (YAML frontmatter + step list):
```yaml
---
title: "Install software package"
keywords: ["install", "package", "apt", "brew", "pip"]
tags: ["system", "package"]
confidence: 0.9
is_template: true
---
1. Identify the package name and package manager
2. Run install command
3. Verify installation succeeded
```

The `is_template: true` field in frontmatter is **required** in every plan template file — it is what `find_plan_template()` uses to distinguish bundled templates from cached runtime plans. The `ensure_system_docs()` loader must propagate this field verbatim from the YAML frontmatter into the MemoryDoc's `metadata` JSONB when constructing the doc. Do **not** hard-code the flag inside the loader — read it from frontmatter so the same loader works for all template files. If `is_template` is missing from a template file's frontmatter, the loader must emit a startup `warn!` and still insert the doc (with `is_template: false` as a safe default), so that missing fields cause visible warnings at startup rather than silent runtime failures.

#### 3.5.6 Task Decomposition (REQ-6.9)

When `run_planning_phase` returns `source = "decompose"`:
1. `subtasks` is the list of 2–4 single-line goal strings from `run_miniplan_call`
2. The main orchestrator loop iterates over subtasks:
   - Each subtask runs through the full `run_planning_phase` → `run_loop` sequence with a fresh `working_messages` and an isolated `subtask_state = {}`
   - **Subtask summary forwarding:** `run_loop` stores the last non-empty assistant text into `state["_last_response"]` immediately before every terminal `return complete_result(...)` call inside the for loop (via the `_write_last_response(state, working_messages)` helper defined in §3.5.2). `run_decomposition_loop` reads `subtask_state.get("_last_response", "")` after each subtask's `run_loop` completes and prepends it (truncated to ≤ 200 tokens) to the next subtask's `goal` string. This requires no decomposition-awareness in `run_loop` — the field is always written and simply ignored by non-decomposition callers. If no LLM step ran before the exit (e.g. budget exhausted on the first iteration), `_last_response` is absent or empty and no context is prepended.
   - **Subtask non-completion propagation:** If any subtask's `run_loop` returns an outcome other than `"completed"` (this includes `"failed"`, `"stopped"`, `"max_iterations"`, gate states), `run_decomposition_loop` must **immediately** stop — remaining subtasks are NOT executed and the plan doc is NOT saved. The subtask's own outcome is propagated to the caller via `return complete_result(state, subtask_result.get("outcome", "failed"))`. Before that call, restore `state["active_plan_doc_id"] = decomp_plan_doc_id` **if** `decomp_plan_doc_id` is not `None` (cached reuse case). This ensures `complete_result` records a failure penalty against the cached decomposition plan. Without this restoration, `state["active_plan_doc_id"]` is `None` (cleared by the subtask's own `complete_result`) and the cached plan's confidence score would never be penalized. For fresh decompositions (`decomp_plan_doc_id is None`), no restoration is needed and no doc is penalized. Note: "stopped" outcomes (stop signal from user) also record a plan penalty — this is intentional and correct because the user-facing task did not complete, regardless of the reason.
3. Decomposition depth guard: `config["decomposition_depth"]` defaults to `0`; subtask runs pass `decomposition_depth: 1`. When `decomposition_depth >= 1`, `run_planning_phase` never returns `"decompose"` — it fails the subtask instead.
4. On successful decomposition completion, the miniplan is **conditionally** stored using the `decomp_plan_doc_id` local variable captured at the start of `run_decomposition_loop` (see §3.5.2 for the full code). **Do not use `state.get("active_plan_doc_id")` for this check** — each subtask's `complete_result` call (§3.5.4) pops `state["active_plan_doc_id"]` as part of plan confidence tracking, so it is always `None` by the time all subtasks complete. The local variable preserves the decomposition-level doc_id across subtask runs. If `decomp_plan_doc_id` is `None` (fresh decomposition from `run_miniplan_call`), the plan is saved: `__save_plan_doc__(goal, subtasks, True)` and the resulting doc_id is set in `state["active_plan_doc_id"]`. All three arguments are positional (see §3.5.2 for why kwarg style silently fails). The `is_decomposition: true` metadata flag (set server-side from the third positional arg) ensures that when this plan is retrieved for a future similar task, the orchestrator routes through the subtask execution loop rather than treating the stored steps as direct execution steps. If `decomp_plan_doc_id` is set (cached decomposition reuse), do NOT call `__save_plan_doc__` — the doc already exists. Instead, restore `state["active_plan_doc_id"] = decomp_plan_doc_id` so that the caller's `complete_result` tracks confidence against the correct existing doc.

#### 3.5.7 Plan Invalidation on Goal Change (REQ-6.8)

When a user injects a new message mid-execution that materially changes the task goal, the current plan must be discarded and a new planning cycle started before the next execution step.

**Detection heuristic** (in the Python orchestrator, checked after each user message injection via `__check_signals__`):

```python
def should_invalidate_plan(user_message, current_plan, goal):
    """
    Returns True if the user message is a new task directive that
    contradicts or replaces the current plan.
    """
    if not current_plan:
        return False
    text = user_message.lower().strip()
    if text.startswith(("instead ", "forget ", "stop ", "cancel ", "actually ", "new task", "switch to")):
        return True
    if any(kw in text for kw in ["do this instead", "change of plan", "never mind", "start over"]):
        return True
    if text.endswith("?") and len(text.split()) < 12:
        return False
    if text.startswith(("yes", "no", "ok", "sure", "correct", "right", "exactly")):
        return False
    return False
```

**Invalidation action:** When `should_invalidate_plan()` returns `True`:

```python
# Clear stale plan state
state["plan_steps"] = None
state["plan_current_step"] = 0
state.pop("active_plan_doc_id", None)

# Use the full injected user message (stripped) as the new goal.
# No prefix stripping (e.g. "instead, ") is performed — the full text
# is the most unambiguous representation for run_planning_phase.
new_goal = user_message.strip()

# Re-plan with the new goal
new_plan_steps, new_source = run_planning_phase(new_goal, actions, config, state)
if new_source == "failed":
    return complete_result(state, "failed")
if new_source == "decompose":
    return run_decomposition_loop(new_plan_steps, new_goal, actions, config, state)

# Update plan state before resuming the step loop
state["plan_steps"] = new_plan_steps
state["plan_current_step"] = 0
```

The `new_goal` is extracted from the injected user message text. The `state["active_plan_doc_id"]` is cleared via `pop` (not set to `None`) so that the `__record_skill_usage__` call at thread completion — which guards on `state.get("active_plan_doc_id")` — naturally skips confidence tracking for the aborted plan (which saw zero execution steps) and correctly tracks the new plan if it runs to completion.

**Conservative design:** The heuristic intentionally errs on the side of *not* invalidating. False negatives (continuing with a stale plan when the user changed direction) are handled by the user repeating the instruction more explicitly. False positives (discarding a valid plan on a clarification) waste one planning call and slow down execution. The keyword list can be extended over time based on usage data.

### 3.6 Mission System — Idle Mode (REQ-5.x)

#### 3.6.1 Data Model

Add `Idle` variant to `MissionCadence` in `crates/brassclaw_engine/src/types/mission.rs`:

```rust
pub enum MissionCadence {
    // ... existing variants ...
    /// Fire only when the system has been idle for at least `threshold_secs`.
    Idle {
        threshold_secs: u64,
    },
}
```

Add `last_activity_at: DateTime<Utc>` to the engine's `Store` trait (or persist via `settings` DB table with key `mission.last_activity_at`) for idle threshold checks that survive restarts (REQ-5.6).

**No migration required.** Missions are stored in-memory (`HashMap<MissionId, Mission>` in `store_adapter.rs`), not in a SQL table. `MissionCadence` is a serde-tagged enum; adding the `Idle` variant is backward-compatible — existing serialized missions without this variant continue to deserialize correctly. The V29 migration listed in §2.2 is struck.

#### 3.6.2 `MissionManager` Changes (`runtime/mission.rs`)

New method: `fn is_system_idle(&self) -> bool` — checks that:
- No thread is in `Running` or `Waiting` state
- No routine/job is pending (bridge layer provides this signal via a callback or a shared atomic)
- `now - last_activity_at >= threshold_secs`

`last_activity_at` is updated on every user message received and on every thread completion event. It is persisted to the DB settings table as a standalone key `"system.last_activity_at"` via the raw settings store API (direct `INSERT OR REPLACE` into the settings key-value table). This key is **not** part of the `Settings` struct — `Settings` has no `system` sub-struct — so it bypasses the `to_db_map()`/`from_db_map()` auto-serialization. The `MissionManager` reads it back via a direct settings table query at startup and on each `tick()` cycle.

The `tick()` loop in `MissionManager` (which currently evaluates `Cron` missions) is extended to also evaluate `Idle` missions: when `is_system_idle()` and no other idle mission is currently running, fire the first eligible idle mission in declaration order (REQ-5.3).

#### 3.6.3 UI — Routines Panel and Mission Configuration (REQ-5.2, REQ-5.4, REQ-5.5)

**Routines panel display:** `Idle`-mode missions appear in the routines panel (both TUI and web UI) with status:
- `"Waiting (idle)"` when idle conditions are not yet met
- `"Running"` when the mission thread is active
- Normal completion states after finishing

No new DB fields needed — status is computed dynamically from `mission.status` + `is_system_idle()` check in the API handler.

**Mission configuration UI (Settings → Missions / web):** Each mission's configuration panel must include:
- A **cadence selector** dropdown with options: `Event-triggered` (current default), `Cron`, `Idle`, `Manual`, `Disabled`. When `Idle` is selected, the panel expands to show:
  - **Idle threshold** numeric input (minutes), default `10`, minimum `1`, stored as `threshold_secs` on the `MissionCadence::Idle` variant (UI converts minutes ↔ seconds).
- The four built-in learning missions (self-improvement, skill-repair, skill-extraction, conversation-insights) each independently support all cadence options. The default remains their current cadence (event-triggered) to avoid breaking existing server deployments (REQ-5.4).

**TUI missions panel:** The TUI routines list (`crates/brassclaw_tui/`) renders idle missions with the same `"Waiting (idle)"` / `"Running"` status text. Configuration changes (cadence selection, threshold editing) are performed via the web UI only; the TUI is read-only for mission configuration.

### 3.7 Profile Defaults

`profiles/local.toml` gains new defaults for home-use deployments:

```toml
[agent]
max_prompt_tokens = 8192

[skills]
max_context_tokens = 2048
```

`profiles/server.toml` and `profiles/server-multitenant.toml` gain:

```toml
[agent]
max_prompt_tokens = 131072
```

---

### 3.8 Local Installation Script & Release Artifact Packaging (REQ-7.1)

To transition from the online-only cloud-hosted model to a capable offline home-use system, we require a repeatable mechanism to build, distribute, and install BrassClaw as self-contained precompiled binaries with bundled resources (skills and registry tools).

#### 3.8.1 Release Archive Structure & Targets

The release pipeline compiles and packages the system for the following target triples:
- **`x86_64-unknown-linux-gnu`**: Linux (Intel/AMD)
- **`aarch64-unknown-linux-gnu`**: Linux (ARM64, e.g., Raspberry Pi 4/5, DietPi)
- **`x86_64-apple-darwin`**: macOS (Intel)
- **`aarch64-apple-darwin`**: macOS (Apple Silicon)
- **`x86_64-pc-windows-msvc`**: Windows (x64)

Each release asset is packaged as a compressed archive (`.tar.gz` for Unix, `.zip` for Windows) with the following structure:
```
brassclaw-<version>-<target>/
├── brassclaw                     # Precompiled executable binary
├── skills/                      # Copy of skills/ directory (all 32 pruned skills)
├── registry/                    # Copy of registry/ directory (home-bundle tools)
└── .env.example                 # Standardized offline environment template
```

#### 3.8.2 Shell Installation Script (`install.sh`)

For macOS and Linux, a portable POSIX-compliant shell script is provided to automate download, verification, path configuration, and default local environment generation.

**Core Operations:**
1. **Platform Auto-Detection**:
   - OS is resolved via `uname -s`.
   - CPU Architecture is resolved via `uname -m`.
   - Maps combination to the appropriate Rust target triple.
2. **Download & Integrity Verification**:
   - Downloads the corresponding `.tar.gz` archive from the GitHub repository release assets.
   - Downloads and verifies SHA256 checksums from `SHA256SUMS` to guarantee binary authenticity and guard against corrupted downloads.
3. **Binary Placement & Path Provisioning**:
   - Installs the executable to `~/.local/bin/brassclaw` (for unprivileged users) or `/usr/local/bin/brassclaw` (if run as root).
   - Explicitly checks whether the destination directory is in the user's `$PATH`. If absent, outputs shell export instructions and appends path initialization to the active shell configuration (`~/.bashrc`, `~/.zshrc`, or `~/.profile`).
4. **Offline Resource Provisioning**:
   - Extracts the bundled `skills/` and `registry/` directories to the home-use system directory: `~/.brassclaw/skills/` and `~/.brassclaw/registry/`. This allows completely offline operation without external network dependencies.
5. **Secure Local Configuration Bootstrapping**:
   - If `~/.brassclaw/.env` does not exist, it is generated using a default template.
   - **`SECRETS_MASTER_KEY` Generation**: Generates a secure, cryptographically random 32-byte (64 hex characters) encryption master key utilizing `/dev/urandom`:
     ```bash
     SECRETS_MASTER_KEY=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
     ```
   - **`GATEWAY_AUTH_TOKEN` Generation**: Generates a secure, random connection token for Web UI authentication:
     ```bash
     GATEWAY_AUTH_TOKEN=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
     ```
   - Configures default local database connection using libSQL:
     ```env
     DATABASE_BACKEND="libsql"
     LIBSQL_PATH="~/.brassclaw/brassclaw.db"
     ```
   - Populates default local LLM configurations targeting a local Ollama instance:
     ```env
     LLM_BACKEND=openai_compatible
     LLM_BASE_URL=http://localhost:11434/v1
     LLM_MODEL=qwen2.5:7b-instruct
     ```
6. **Systemd Service Setup (Linux only)**:
   - Detects if `systemd` is available.
   - Installs a user-level service file at `~/.config/systemd/user/brassclaw.service` or a system-level service file at `/etc/systemd/system/brassclaw.service` (depending on root status).
   - Generates the systemd unit file with automatic environment sourcing from `~/.brassclaw/.env` and `Restart=always` behavior.

#### 3.8.3 PowerShell Installation Script (`install.ps1`)

For Windows, a native PowerShell script provides equivalent capabilities. It creates the `~/.brassclaw` directory, configures PATH environment variables, downloads the Windows `.zip` archive, extracts the binary, and securely generates random keys using `System.Security.Cryptography.RNGCryptoServiceProvider`.

---

## 4. Data Model / API / Interface Changes

### 4.1 `ThreadConfig` — New Fields

All five new fields **must carry `#[serde(default)]` attributes** so that existing `ThreadConfig` JSON snapshots (stored in DB checkpoints before the upgrade) deserialize correctly when the new keys are absent. Without these attributes, any thread checkpointed before the upgrade would fail to resume after deployment.

```rust
pub struct ThreadConfig {
    // ... existing fields ...
    #[serde(default = "default_thread_max_prompt_tokens")]
    pub max_prompt_tokens: usize,              // default: 8192
    #[serde(default = "default_thread_skill_token_budget")]
    pub skill_token_budget: usize,             // default: 2048, must be <= max_prompt_tokens
    #[serde(default)]
    pub codeact_enabled: Option<bool>,         // None = auto-detect by backend
    #[serde(default)]
    pub decomposition_depth: u8,               // internal, default 0
    #[serde(default = "default_thread_plan_confidence_threshold")]
    pub plan_confidence_threshold: f64,        // default: 0.6, range 0.0–1.0
}

fn default_thread_max_prompt_tokens() -> usize { 8192 }
fn default_thread_skill_token_budget() -> usize { 2048 }
fn default_thread_plan_confidence_threshold() -> f64 { 0.6 }
```

The `impl Default for ThreadConfig` block (currently in `crates/brassclaw_engine/src/types/thread.rs`) must also be extended to include all five new fields with the same values so that code using `ThreadConfig::default()` or `..ThreadConfig::default()` struct update syntax compiles correctly.

### 4.2 `MissionCadence` — New Variant

```rust
pub enum MissionCadence {
    Cron { expression: String, timezone: Option<ValidTimezone> },
    OnEvent { event_pattern: String, channel: Option<String> },
    OnSystemEvent { source: String, event_type: String, filters: HashMap<String, Value> },
    Webhook { path: String, secret: Option<String> },
    Manual,
    Idle { threshold_secs: u64 },   // NEW
}
```

### 4.3 `CapabilityStatus` — New Variant

`CapabilityStatus` (used by the tool/capability lifecycle) gains a new transitional variant for tools that require an initial data fetch before becoming ready (e.g., CalDAV):

```rust
pub enum CapabilityStatus {
    // ... existing variants (Ready, NeedsSetup, Error, ...) ...
    Syncing,   // NEW — initial data fetch in progress; tool not yet ready
}
```

While in `Syncing` state, read actions return empty results with `"syncing": true` in the response payload. The tool transitions to `Ready` on successful fetch or `NeedsSetup` on failure/timeout.

**Exhaustive match updates required by this new variant:**
- `prompt.rs` — `capability_status_label()`: add `CapabilityStatus::Syncing => "syncing"`.
- `tool_surface.rs` — `fallback_assignment()`: add `CapabilityStatus::Syncing` alongside `NeedsSetup` → `SurfaceAssignment::capabilities_only()` (syncing tools are not directly callable).
- `action_projector.rs` — `provider_extension_rank()`: add `CapabilityStatus::Syncing => 2` (same rank as `NeedsSetup` — transitional, not yet ready).

### 4.4 `LlmCallConfig` — New Field

```rust
pub struct LlmCallConfig {
    // ... existing fields (max_tokens, temperature, force_text, depth, model, metadata) ...
    pub is_planning_call: bool,  // true for planning calls: Rust-side telemetry marker
}
```

Planning calls set `force_text: true` (existing field) to prevent tool call responses, and `is_planning_call: true` (new field) to identify the call as a planning call on the Rust side for telemetry. CodeAct preamble suppression does **not** require a separate config flag — planning calls pass explicit custom messages that naturally bypass `refresh_llm_messages_for_current_surface()` (see §3.3.4). The `__apply_token_guard__` is similarly bypassed by code structure (planning calls happen before `run_loop`'s step loop), not by checking `is_planning_call` from Python.

The `handle_llm_complete` handler in `orchestrator.rs` must be extended to read `is_planning_call` from the Python config dict:
```rust
is_planning_call: explicit_config
    .as_ref()
    .and_then(|cfg| cfg.get("is_planning_call"))
    .and_then(|v| v.as_bool())
    .unwrap_or(false),
```

### 4.5 New Host Functions (Python → Rust boundary)

These functions are added to `crates/brassclaw_engine/src/executor/orchestrator.rs` and declared in the Python orchestrator's header comment block alongside the existing host function list.

| Function | Signature | Purpose |
|---|---|---|
| `__save_plan_doc__` | `(goal: str, steps: list[str], is_decomposition: bool) -> str` | Persists a new `DocType::Plan` MemoryDoc. All three arguments are required — Monty host functions do not support Python-style default parameter values, so every call site must pass `is_decomposition` explicitly (`False` for normal plans, `True` for decomposition plans). Keywords are extracted from the `goal` text server-side (split on whitespace, lowercased, stop-words removed) — the Python caller does not need to construct them. When `is_decomposition=True`, the Rust implementation sets `metadata["is_decomposition"] = true` on the MemoryDoc, marking it as a decomposition plan so that the retrieval system can route cached decomposition plans back through the subtask execution loop (§3.5.6) rather than treating the stored steps as direct execution steps. Returns the doc ID string so the orchestrator can store it in `state["active_plan_doc_id"]` for later confidence tracking via `__record_skill_usage__`. |
| `__apply_token_guard__` | `(parts: dict) -> dict` | Applies priority-order budget degradation. See §3.2.3 for the `parts` schema and return shape. |

> **Rust module docstring:** The `//!` module comment block at the top of `crates/brassclaw_engine/src/executor/orchestrator.rs` (lines 8–20) lists existing host functions. After adding `__apply_token_guard__` and `__save_plan_doc__`, append both to that list so the Rust-side documentation stays in sync with the Python header. Format matches the existing entries: `//! - __apply_token_guard__(parts) -> dict` and `//! - __save_plan_doc__(goal, steps, is_decomposition) -> str`.

**Interface design decision for `__apply_token_guard__`:** The Monty VM runs in-process (no network boundary between Python and Rust), so serialization cost is dominated by memory copies, not I/O. For this release, the **full-content interface** described in §3.2.3 is the binding design:

- Python passes the complete `PromptParts` dict (with full `content` strings for skills, memory docs, conversation history).
- Rust `TokenGuard` applies degradation in-place and returns `{"dropped": [{"type": "skill"|"memory_doc"|"history", "id": ...}], "fits": bool}`.
- Python removes the dropped items from its working message list before calling `__llm_complete__`.

This avoids the complexity of maintaining parallel content ownership between Python and Rust. A future optimization (passing only counts+scores and returning indices) may be introduced if profiling shows the full-content pass is a bottleneck at high conversation lengths, but that is out of scope for this release.

### 4.6 Settings API — New Fields

| Key | Type | Default | Where |
|---|---|---|---|
| `agent.max_prompt_tokens` | `usize` | `8192` | `AgentSettings`, DB settings table, UI field |
| `agent.plan_confidence_threshold` | `f64` | `0.6` | `AgentSettings`, DB settings table, UI field (advanced) |
| `agent.codeact_enabled` | `Option<bool>` | `None` | `AgentSettings`, DB settings table, UI toggle (advanced) |
| `skills.max_context_tokens` | `usize` | `2048` (lowered from 4000) | `SkillsSettings`, existing DB key |
| `local_search.allow_global_scope` | `bool` | `false` | New `LocalSearchSettings` struct in `src/settings.rs`, DB settings table, UI toggle |

**New `LocalSearchSettings` struct** (in `src/settings.rs`, alongside `AgentSettings` and `SkillsSettings`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSearchSettings {
    /// Whether the agent is permitted to search the entire filesystem.
    /// When false (default), search is restricted to the workspace root
    /// and any additional paths the user explicitly added in settings.
    /// When an agent passes scope: "global" and this flag is false, the
    /// action handler returns a user-facing error rather than scanning.
    #[serde(default)]
    pub allow_global_scope: bool,
}

impl Default for LocalSearchSettings {
    fn default() -> Self {
        Self {
            allow_global_scope: false,
        }
    }
}
```

`LocalSearchSettings` is added to the top-level `Settings` struct as `pub local_search: LocalSearchSettings` alongside the existing `agent`, `skills`, and `wasm` fields. The generic `Settings::to_db_map()`/`from_db_map()` persistence mechanism (in `src/settings.rs`) automatically handles the new field: `to_db_map()` serializes the whole `Settings` struct to JSON and then flattens it to `{"dotted.path": value}` pairs via `collect_settings_json()`, which will produce the key `"local_search.allow_global_scope"` for the new field. `from_db_map()` starts from `Settings::default()` (which includes `local_search: LocalSearchSettings::default()`) and applies each key via `Settings::set()`. No additional wiring code is needed — adding the field with `#[serde(default)]` is sufficient for DB persistence to work automatically. This is the same pattern used by `agent`, `skills`, and `wasm`. The `local_search` action handler in `src/tools/builtin/local_search.rs` reads this field at call time to enforce the scope gate.

### 4.7 Settings UI — New Fields (Settings → Agents Tab)

After existing "Skill context token size" field:
- **Max total prompt tokens** (numeric input, min: 512, max: 1,048,576)
  - Default: 8192
  - Description: "Total token budget per LLM prompt turn. Includes system prompt, skills, memory, tools, and conversation history. Home-use default (8,192) leaves ≈3,500 tokens for conversation (roughly 7–10 exchanges)."
  - Validation: must be ≥ skill context token size; error message if not.

Advanced section (collapsed by default):
- **Plan confidence threshold** (numeric input, min: 0.0, max: 1.0, step: 0.1)
  - Default: 0.6
  - Description: "Minimum confidence score for reusing a cached execution plan. Lower values reuse more plans (faster but riskier); higher values force fresh planning calls more often (slower but more accurate). Adjust downward if your local model produces acceptable plans that are being discarded."
  - Validation: must be in range [0.0, 1.0]; reject with error message if not.
- **Enable CodeAct for local models** (toggle)
  - Description: "Enable Python REPL execution for local models (Ollama/OpenAI-compatible). Only enable if running a large local model (≥ 30B parameters)."

**Settings → Tools → Local Search section** (separate tab or subsection from Agents):
- **Allow whole-filesystem search** (toggle, default: off)
  - Description: "When enabled, the agent may search the entire filesystem when asked. When disabled (default), search is limited to the workspace and any additional folders configured below. Enable only if you trust the agent with full filesystem read access."
  - Saving updates `local_search.allow_global_scope` in `LocalSearchSettings`.

---

## 5. Verification Approach

### 5.1 Build & Type Check

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Run after each v1 removal stage and after each new module addition.

### 5.2 Unit Tests

**`crates/brassclaw_engine`:**
- `executor/token_guard.rs` — unit tests for each degradation step and budget fits/overflow logic
- `executor/planner.rs` — unit test for `PlanAnchor::to_prompt_section()`: assert output `len * 0.25 <= 200` for plans with up to 10 steps (verifies plan anchor ≤ 200 tokens success metric)
- `executor/tier0_prompt.rs` — token count assertion: `build_tier0_system_prompt(None, None).len() * 0.25 <= 1024`. Assert output starts with `TIER0_SYSTEM_PROMPT_MARKER`.
- `executor/prompt.rs` — `is_engine_system_prompt()`: assert `true` for strings starting with `CODEACT_SYSTEM_PROMPT_MARKER`, `true` for strings starting with `TIER0_SYSTEM_PROMPT_MARKER`, `false` for arbitrary text. `upsert_engine_system_prompt()`: assert it correctly replaces a Tier 0 system message on update (not insert a duplicate).
- `types/mission.rs` — round-trip serialization test for new `Idle` variant

**`crates/brassclaw_skills`:**
- `selector.rs` — test that a skill with `max_context_tokens: 0` is excluded (after changing default to 0)
- `selector.rs` — test that declared budget is enforced at lower default (2048 vs old 4000)

**`src/`:**
- `config/agent.rs` — remove `engine_v2` test, add `max_prompt_tokens` and `plan_confidence_threshold` resolver tests
- `channels/web/features/settings/mod.rs` — test that saving `max_prompt_tokens < skill_context_tokens` returns 400
- `channels/web/features/settings/mod.rs` — test that saving `plan_confidence_threshold` outside `[0.0, 1.0]` returns 400
- `channels/web/features/settings/mod.rs` — test `infer_is_local()`: assert `true` for `http://localhost:11434`, `http://127.0.0.1:8080`, `http://[::1]:5000`, `http://0.0.0.0:8080`; assert `false` for `https://api.together.ai`, `None`
- `tools/builtin/local_search.rs` — unit test for scope gate: assert `search_files` action with `scope: "global"` returns an error (containing the "Enable it in Settings" message) when `LocalSearchSettings { allow_global_scope: false }`; assert the same call succeeds when `allow_global_scope: true`. This is a security boundary and must be unit-tested, not only manually verified.

### 5.3 Integration / E2E

```bash
cargo nextest run --workspace
```

Key integration scenarios to verify:
1. v1 code paths removed: `cargo test -p brassclaw` must not reference `dispatcher`, `session_manager`, `agent_loop` v1 paths, or `SkillCatalog`.
2. Token guard degradation: end-to-end orchestrator test with a prompt that exceeds budget; verify memory docs are dropped in score order.
3. Plan-first (`run_planning_phase()` Python path): orchestrator test with a non-trivial goal; verify `state["plan_steps"]` is populated before first tool call. Test `is_trivial()` heuristic: verify it returns `True` for single-word goals, question-mark-only inputs, and goals below `trivial_word_threshold`; verify it returns `False` for multi-step goals. Test `run_planning_phase()` returns `"trivial"` source for trivial goals.
4. Tier 0 suppression: configure backend as `"ollama"` with `is_local: true`; assert the generated system message starts with `TIER0_SYSTEM_PROMPT_MARKER` (not `CODEACT_SYSTEM_PROMPT_MARKER`). Assert `is_engine_system_prompt()` returns `true` for both prompt types. Separately verify that an `openai_compatible` provider with `is_local: false` continues to receive the full CodeAct system prompt.
5. Idle mission: integration test where `MissionCadence::Idle` mission fires after `last_activity_at` + threshold elapses with no active threads.
6. **libSQL migration verification**: apply `V28` migration against a fresh libSQL database file and verify it applies cleanly. libSQL is the primary backend for home-use deployments. (V29 was struck — missions are in-memory, no SQL migration needed for the `Idle` cadence variant.)
7. **Planning call budget**: orchestrator test that constructs a `run_minimal_planning_call()` invocation and asserts the total input (system message + user message) is `<= 200` tokens (`len * 0.25`). Verify that the system message is `<= 20` tokens.
8. **Idle timer persistence**: integration test that writes `system.last_activity_at` to the settings store, simulates a restart (re-read from DB), and verifies the timestamp survives.
9. **Decomposition depth enforcement**: orchestrator test where a subtask (with `decomposition_depth: 1`) exceeds budget after full degradation; verify it fails with an error rather than triggering further decomposition.
10. **Plan template count**: startup test that loads `docs/internal/plan-templates/` and asserts `>= 7` templates are discovered and inserted via `ensure_system_docs()`.
11. **Fresh single-skill prompt size**: orchestrator test with one active skill, empty conversation history, and default settings; assert total assembled prompt `<= 4,000` tokens.
12. **Decomposition outcome propagation**: orchestrator test where the first subtask in a two-subtask decomposition returns `"stopped"` (simulated stop signal); verify that `run_decomposition_loop` returns `"stopped"` (not `"completed"`) and that neither the plan doc nor the second subtask is executed.
13. **`_write_last_response` helper coverage**: unit test with a mock `working_messages` list containing at least one assistant message; assert that after calling `_write_last_response(state, working_messages)`, `state["_last_response"]` equals the last non-empty assistant message content. Additionally test with an empty messages list to verify no KeyError is raised and state is unchanged.

### 5.4 Manual Verification Checklist

- Skills without `max_context_tokens` declared produce a startup warning in the log.
- Settings → Agents tab shows new "Max total prompt tokens" field.
- Saving `max_prompt_tokens = 1000` when `skill_context_tokens = 2048` is rejected with an error message.
- Playwright browser tool shows `NeedsSetup` status (not an error) when Node.js is not installed.
- Playwright browser tool rejects `file://` URLs with a clear error message (REQ-4.3 sandboxing).
- CalDAV tool defaults to `verify_ssl: true`. When explicitly set to `false`, emits a visible startup warning about MITM vulnerability.
- All bundled skills have `max_context_tokens` declared in their frontmatter (checked by a startup assertion).
- Miniplan system message (`"Break task into subtasks. One per line. Brief."`) is ≤ 15 tokens (verify by `len * 0.25`).
- After a successful non-trivial task execution, verify a `DocType::Plan` MemoryDoc was cached for the goal (plan caching success metric).
- With `local_search.allow_global_scope = false` (default), agent calls to `search_files` with `scope: "global"` must return a user-facing error and log a warning. With the flag set to `true`, the same call must succeed. Verify the setting toggle in Settings → Tools → Local Search updates this behavior without restart.

---

## 6. Constraints Summary

| Constraint | Technical mapping |
|---|---|
| C1 — Five primitives intact | `DocType::Plan` already defined; `MissionCadence::Idle` added via new variant, no table change needed |
| C2 — No circular dep | `crates/brassclaw_engine` gains `TokenGuard`, `PlanAnchor`, `Tier0Prompt` modules — all self-contained, no `src/` import |
| C3 — Hybrid token approximation | ASCII/Latin text uses `len(bytes) * 0.25` (fast byte-based). CJK/Arabic/Hangul text delegates to `tiktoken-rs` cl100k_base (optional `tiktoken` feature) for accurate counts, falling back to `char_count * 1.5` (conservative over-estimate). Python mirrors via `__count_tokens__` host function with same fallback chain. |
| C4 — Node.js optional | `playwright.json` marked `auto_launch: true`; absence → `NeedsSetup` status, not a startup failure |
| C5 — Staged v1 removal | Stage order in §3.1; each stage verified with `cargo check + test` |
| C6 — Cloud path unaffected | `build_codeact_system_prompt_with_docs()` unchanged; only the `is_local_backend()` branch uses Tier 0 |
| C7 — 1,024-token tool budget | Always-present tool list audited; new local tools registered as on-demand |
| C8 — Planning call 200-token cap | Hard-coded in `run_minimal_planning_call()` config dict |
| C9 — No decomposition recursion | `decomposition_depth` guard in `ThreadConfig` enforced in orchestrator |
| R1 — Skill budget breaking change | Default changes to 0; startup warning emitted per skill; documentation updated |
| C10 — Mission ThreadConfig (deferred) | `MissionManager::fire_mission()` uses `ThreadConfig::default()` — must inject resolved `AgentConfig` into `MissionManager` at construction. Plan Step 19 |
| C11 — count_running_threads O(N) (deferred) | Iterates `thread_history` with per-entry `load_thread` store call. Replace with atomic counter or DB-side `COUNT(*)`. Plan Step 20 |
| C12 — Plan doc deduplication (deferred) | `handle_save_plan_doc` always creates new `MemoryDoc`. Needs upsert-on-title-match to prevent cache bloat. Plan Step 21 |
| C13 — CJK token approximation (deferred) | `bytes × 0.25` under-counts CJK/emoji. Integrate `tiktoken-rs` or adjust multiplier to `0.33`. Must update both Rust `token_count()` and Python `_token_count()`. Plan Step 22 |
