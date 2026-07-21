# BrassClaw Design Transition — Technical Spec

Status: Draft v5 (incorporates v4 + Actions class rewrite: default.py executor
not Rust ActionExecutor, 10 step types, no size limits, token-budget
exemption, 8-step dispatch flow; Monty VM kernel-owned restart + status
polling; DB-less fallback file for intent system; "AI before User" flip
switch).
Scope: Fundamental redesign of Skills, Tools, Extensions, and the Memory/chunk
subsystem.

## 1. Glossary (resolved terms)

| Term | Meaning | Maps to today |
|------|---------|---------------|
| **Monty** | Embedded Python VM running the CodeAct orchestrator + user code (Tier 1). | `brassclaw_engine` `executor/scripting.rs`, `orchestrator/default.py` |
| **Rusty** | The Rust host/tool execution layer — `EffectExecutor` + `ToolRegistry` + host runtime (Tier 0). | `brassclaw_engine` `traits/effect.rs`, `brassclaw_host_runtime`, `brassclaw_capabilities` |
| **DocPlans** | Plan documents from `plan_library.rs` **and** the `plan-mode` skill's plan `MemoryDoc`s. In the new design these are **dissected** into their constituent tools/skills/recipe-steps. | `plan_library.rs`, `skills/plan-mode/SKILL.md`, `DocType::Plan` |
| **PlanA-Memory** | The `brassclaw_memory` MemoryDoc storage layer. In the new design it is **upgraded to the universal retrieval connector for the turn** (all turn data fetches go through it). | `brassclaw_memory` (backend, repo, filesystem, path) + `RetrievalEngine` |
| **Chunk system** | Chunking + embedding + hybrid-search retrieval. **Removed.** | `brassclaw_memory` `chunking.rs`, `indexer.rs`, `search.rs`, `embedding.rs` |
| **Two-step validation** | The existing validation pipeline: Step 1 auto (`RecipeValidator` + `SimilarityChecker`) → `AutoPassed`; Step 2 manual (WebUI queue, user clicks) → `Validated`. Only `Validated` components are used by the loop. **Becomes the sole trust gate.** | `recipe_validator.rs`, `similarity_checker.rs`, `recipe_store.rs` (`is_valid_transition`), WebUI `validate_recipe`/`validate_tool_skill` |
| **Class code** | 2-digit code on every "static" DB object (skill/tool/extension/former-doctype) used for deterministic prompt ordering. Tools-first, foundation-first. | new |
| **prompt_uid** | Monotonic integer id on every static DB object, assigned at creation, never reused/reordered. | new |
| **Actions** | A new component class (code 16). A self-contained deterministic procedure — a complete plan with parts for Rusty (tool calls) and parts for default.py (orchestration instructions) — that default.py executes **without an LLM call** when the intent system matches. Contains preconditions, ordered step descriptors (tool_call/conditional/set_var/loop/return/evaluate/call_skill/try_catch/parallel/call_action/spawn_subprocess/wait/emit_event — 13 step types), error handling, timeout, allowed_tools. Actions do **not** conform to skill size limits. | new |
| **Intent system** | Unified DB-driven routing replacing `signals_tool_intent()`, `signals_execution_intent()`, `score_skill()` keyword matching, `RetrievalEngine::extract_keywords()`, `extract_explicit_skills()`, `RecipeTrigger` matching, and v1 Rust `llm_signals_tool_intent()`/`user_signals_execution_intent()`. Uses `reborn_intent_inputs` table with input classes 1-4. | new |
| **Intent input class** | 1 = single word, 2 = partial sentence, 3 = full sentence, 4 = keyword (from RetrievalEngine fallback). Match order depends on query class (§3.12). | new |
| **DB-less fallback file** | A static content file created at installation time (when the user selects not to install a DB) containing selected compiled-in entries (skills, tools, Plans, etc.) up to ~5 original DocPlans in size (~256KB). In DB-less mode, the intent system is overridden and the RetrievalEngine falls back to keyword-based retrieval over this file. | new |
| **AI before User** | A flip switch in the WebUI chat window. When ON, the intent system silently activates the keyword-retrieval fallback after 10 failed LLM rephrase sentences — no "reformulate" message is sent to the user. When OFF (default), the user is asked to reformulate and offered the "or try it with AI" button. | new |

## 2. Current state (as-is)

### 2.1 Skills
- On-disk `SKILL.md` with YAML frontmatter (`brassclaw_skills::parser`); mirrored into engine v2 as `MemoryDoc` `DocType::Skill` with `V2SkillMetadata` jsonb.
- Selection: deterministic gating → scoring → budget → **attenuation** (`brassclaw_skills::selector`, Python `default.py::score_skill`).
- **Trust layer (to be removed):** `SkillTrust` enum (`Installed` < `Trusted`), `V2SkillMetadata.trust`, `default_trust() = Installed`, trust-by-source-directory in `registry.rs` (Workspace/User/Bundled = Trusted, Installed-dir = Installed), and the **skill-trust attenuation** phase (tool ceiling = `min(trust)` across active skills). `source` (`extracted`/`authored`/...) gates the confidence factor in `score_skill` (`0.5 + 0.5*confidence` only when `source == "extracted"`).

### 2.2 Two-step validation (verified working as intended)
- `ValidationStatus`: `Pending → AutoPassed/UpgradeQueued/AutoFailed → Validated` (or `ReviewRequested → Rejected → Garbage` after 3 failed reviews + 30-day window).
- Step 1 auto: `RecipeValidator` (structural, agentskills.io rules) + `SimilarityChecker` (Jaccard dedup: Recipe 0.70, ToolSkill 0.80, step-overlap 0.80).
- Step 2 manual: WebUI `validate_recipe` / `validate_tool_skill` PUT endpoints → `recipe_store.rs::is_valid_transition` guards (`AutoPassed → Validated` allowed; `Pending → Validated` blocked; `UpgradeQueued → Validated` blocked; `Garbage → *` blocked).
- `recipe_library.rs` filters to **only `ValidationStatus::Validated`** for the agent loop.

### 2.3 Tools, Recipes, Extensions, Memory
- Tools: capability/`ToolRegistry` surface, presented to both Tier 0 and Tier 1.
- Recipes/ToolSkills: `MemoryDoc` `DocType::Recipe`/`ToolSkill`; `recipe_store.rs` (REST) + `recipe_library.rs` (loop adapter); Wilson-scored tiers (Seedling/Growing/Mature/Candidate).
- Extensions: Manifest v2 (TOML, `runtime` wasm/mcp/script); `brassclaw_extensions` declarative; `pg_store.rs`.
- Memory: PlanA-Memory (`reborn_memory_*` + `FilesystemMemoryDocumentRepository`, scope tuple, versioning) + chunk system (chunking/indexer/embedding/hybrid-search). `RetrievalEngine::retrieve_context` is already the Python-facing seam via `__retrieve_docs__`.

### 2.3a Intent-detection functions (all replaced by the unified intent system)
The codebase has **8 separate intent-detection mechanisms** that the unified intent system (§3.12) replaces:

| Function | Location | What it does |
|----------|----------|-------------|
| `signals_tool_intent()` | `default.py:101` | Prefix+verb phrase matcher ("let me search", "I'll fetch") — nudges LLM to act |
| `signals_execution_intent()` | `default.py:147` | Imperative phrase matcher ("run it", "stop that") — enables obligation mode |
| `llm_signals_tool_intent()` | `brassclaw_llm/src/reasoning.rs:48` | Rust twin (v1, dead in v2 path) |
| `user_signals_execution_intent()` | `brassclaw_llm/src/reasoning.rs:121` | Rust twin (v1, dead in v2 path) |
| `score_skill()` keyword matching | `default.py:678` | Keyword exact/substring + tag + regex scoring for skill activation |
| `RetrievalEngine::extract_keywords()` | `retrieval.rs:80` | Stop-word-filtered keyword extraction for doc retrieval |
| `extract_explicit_skills()` | `default.py:754` | Slash-command matcher (`/skill-name`) — force-activates skills |
| `RecipeTrigger` matching | `recipe.rs:65-107` | Exact/Pattern/Keyword trigger matching for Recipes |

### 2.3b Formatting functions (all replaced by Rust-owned class-specific formatters)
| Function | Location | What it does | Fate |
|----------|----------|-------------|------|
| `format_docs(docs)` | `default.py:234` | Renders docs as `## Prior Knowledge`, 500-char truncation | **Deleted** — replaced by `__assemble_prior_knowledge__` (Rust) |
| `format_skills(skills)` | `default.py:932` | XML-tagged `<skill>` blocks with trust/version/bundle_path | **Deleted** — replaced by class-specific Rust formatters |
| `format_output(result)` | `default.py:194` | Formats code execution results (stdout, action results) | **Kept** in Python (tool output, not prior knowledge) |
| `format_docs_as_context(docs)` | `context.rs:78` | Rust twin of `format_docs` (dead in production) | **Resurrected** as the basis for Rust class-specific formatters |
| `append_system_append()` | `default.py:270` | Appends to System message (KV-cache-mutating) | **Deleted** — replaced by User-message-at-N-1 (Rust-owned) |
| `_reduce_prompt()` | `default.py:542` | Truncation/summarize/drop rules for prompt budget | **Kept** in Python (prompt reduction policy) |
| `compact_if_needed()` | `default.py:562` | LLM-call compaction at 0.85 threshold | **Kept** in Python (compaction policy) |

### 2.4 Binding validation criteria (agentskills.io, preserved)
Name `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$` (1–64, no `--`, no leading/trailing `-`); description 1–1024 chars + actionable verb; ≤5000 token budget; one-tool-per-skill coherence (warning >3 tool names); ToolSkill `tool_name` in capability surface; `param_template` JSON object; `param_schema` entries non-empty; Recipe ≥1 step referencing known skill + non-empty tool; trigger rules; `escape_skill_content`/`escape_xml_attr` XML-breakout defense; version `^[a-zA-Z0-9._\-+~]{1,32}$`; cardinality caps (≤20 keywords, ≤5 patterns, ≤10 tags, min len 3, ≤64 KiB). Full table in §2.6 of spec v1.

## 3. Target state (to-be)

### 3.1 DB-stored Skills (3-class, no trust)
- Skills live in `reborn_skills` (no `SKILL.md` files). agentskills.io frontmatter; the **`compatibility` field carries the class** (`brassclaw-class:llm|monty|rusty`).
- **No `SkillTrust` field.** A skill is usable iff `validation_status == Validated`. `Validated == trusted` (see §3.5).
- **Splitting rule:** one tool-usage-pattern per skill (hard warning >3 tool names); existing large `SKILL.md` files are split into multiple rows.
- **`intent_examples jsonb` column** (§3.12): array of `{input, class}` entries fed to the intent system on validation. Replaces `keywords[]`/`patterns[]`/`tags[]` as the activation mechanism (those columns remain for backward compat but the intent system is the primary routing path).
- **Settings UI:** browse + edit content + set class + edit intent examples; every save runs Step-1 validation.

### 3.2 DB-stored Tools (Rusty-only)
- Tools in `reborn_tools`; carry Rusty instructions only (name, param schema, effect type, preconditions, error handling). No Monty/LLM prompt text.
- The capability surface that `RecipeValidator` checks `tool_name` against is sourced from `reborn_tools`.

### 3.3 Unified Extensions (Extensions + DocPlans + Recipes merged)
- One entity in `reborn_extensions_unified` with class enum: `mcp_server`, `mcp_client`, `rusty`, `monty`, `llm`, `misc`.
- **DocPlans are dissected**, not migrated whole: each plan document is decomposed into its constituent tools/skills/recipe-steps, which become first-class rows in `reborn_skills`/`reborn_tools`/`reborn_extensions_unified`. The plan survives only as a thin `monty`-class extension (the orchestration recipe).
- **Recipes get their own class code 21** (spec §7 Q15 resolved — they are solution-class with a distinct schema: trigger + ordered steps + skill references; they have `override_prompt_creation` + `prior_knowledge_content` columns) in a dedicated `reborn_recipes` table. **ToolSkills get their own class code 13** in a dedicated `reborn_tool_skills` table (spec §4). Neither Recipes nor ToolSkills fold into `reborn_extensions_unified` — they have distinct schemas. The `RecipeLookup` trait boundary is preserved so `brassclaw_agent_loop` stays free of `brassclaw_engine`. The `reborn_extensions_unified` class 09 (Misc) retains non-Recipe Misc extensions only.

### 3.4 PlanA-Memory as the universal retrieval connector
- **The DB is the only source of information.** No files, no chunks.
- `RetrievalEngine` is promoted to the **universal turn-retrieval interface**: the turn asks it for durable memories, selected skills, active tools, active extensions, the active orchestrator, and the scaffold sections. Same calls regardless of backend.
- **Intent-driven retrieval (§3.13):** retrieval is now O(matched_components), not O(all_docs). The intent system (§3.12) resolves the query to component IDs; the RetrievalSource fetches those components by ID. "Load all docs" is eliminated.
- **Two backends behind one `RetrievalSource` trait:**
  - `PostgresSource` — production.
  - `RamSource` — DB-less; serves **compiled-in default** skills/tools/extensions + stores thread memories in RAM (reuses `InMemoryMemoryDocumentRepository`). Not for production.
- **Baked-in fallback system prompt + prior-knowledge** ship inside PlanA-memory and are served **only** by `RamSource` when no DB is present. This includes the compiled-in default orchestrator (`DEFAULT_ORCHESTRATOR`) and scaffold (`CODEACT_PREAMBLE` / `CODEACT_POSTAMBLE`). The prompt-composition code is **identical** for DB and DB-less — only the backend swaps.
- **DB-less intent-system fallback file (spec §7 Q17 resolved):** in DB-less mode, the intent system (§3.12) is **overridden** — it cannot query `reborn_intent_inputs` (no DB). Instead, a **fallback-content file is created at installation time** when the user selects not to install a DB. The file contains **selected compiled-in entries** — most of the skills, tools, Plans, Specs, Lessons, etc. — up to a **filesize approximately matching 5 of the original DocPlans combined** (~256KB, ~50,000 tokens). The `RamSource` loads this file into an in-memory index at startup. The RetrievalEngine then works **as it did before the architecture change**: keyword-based retrieval (the original `extract_keywords` + `keyword_match_score` + `doc_type_weight` path from `retrieval.rs`), not intent-driven retrieval. This means:
  - In DB-less mode, `fetch_for_turn` falls back to the pre-v4 keyword-retrieval path (load all fallback-file entries, score by keyword + type-weight, return top results within token budget).
  - The intent system's `__resolve_intent__` host function returns `{db_less_fallback: true}` in DB-less mode, signaling the orchestrator to use the keyword-retrieval path.
  - The "try it with AI" fallback and "AI before User" flip switch (§3.12 rule f-ai) are **not available** in DB-less mode (there is no intent system to fall back from).
  - The fallback file is **static** (created at installation time from compiled-in defaults); it does not learn or update during DB-less operation. Learned inputs from the intent system are only persisted when a DB is present.
  - **Priority for compiled-in inclusion:** Tools (class 00) → Scaffold (50) → Orchestrator (10) → Skills (01-03) by usage → Extensions (04-09) by usage, Monty-class first → Recipes (21) by usage → Specs/Lessons (12, 18) newest → Issues/Notes/Summaries (19, 20, 15) excluded (volatile/low-value for fallback).
- **"Try it with AI" fallback (§3.12 rule f-fallback):** when the intent system finds no match after 10 LLM rephrase sentences, the user gets a "reformulate your query" message + a **"or try it with AI"** button. On click, the RetrievalEngine fallback is reactivated for that single query: keywords are extracted (existing `extract_keywords`), sent to the intent system as **class-4 inputs**, and each keyword is matched (class 1 → 2 → 3 order). Matching keywords return component IDs; the RetrievalEngine fetches those components and builds a prompt. Goal: progressive learning — each fallback use teaches the intent system new mappings.

### 3.5 Two-step validation = the sole trust gate (security simplification)
- **`Validated == trusted`.** The two-step validation system (§2.2) is the only gate to usability. No trust gradient.
- **Removed:** `SkillTrust` enum; `V2SkillMetadata.trust`; `default_trust()`; trust-by-source-directory in `registry.rs`; the **skill-trust attenuation** phase of the selection pipeline (tool ceiling by `min(trust)`); the `Installed`/`Trusted` tool-access distinction.
- **`source` becomes pure provenance** — retained for audit/display only, with **no behavioral effect**. The confidence factor (`0.5 + 0.5*confidence`) is **source-independent** and **kept as a fallback routing signal only**: in normal mode the intent system's score is the primary routing signal; the confidence factor is used only when the fallback mechanism is triggered (user "AI before User" switch ON, or intent system finds no match). Skills with no usage data default to confidence 1.0. The telemetry columns (`usage_count`, `success_count`, `failure_count`, `wilson_lower`, `ema_success`, `ema_latency`, `sample_count`) are displayed in the WebUI Reliability tab regardless of mode.
- **Kept (not source-driven security):** subagent capability attenuation (`brassclaw_loop_support::attenuate_child_capability_port` — child-run capability scoping, a kernel authority mechanism); install-bundle ingestion (registry import path — imported skills go through validation like everything else); credential specs (OAuth/API auth, separate from trust).
- **Validator expands to validate code** (the orchestrator Python [class 10] + Monty-class extension payloads + Scaffold overlay updates [class 50]), not just skill text. All code changes — including self-improvement mission patches — must pass validation before applying (versioned rollback retained). The orchestrator is an **Orchestrator-class component (10)**, not a Monty-class extension.
- **The validator is independent of the orchestrator.** The Step-1 validator runs as a **separate gating mechanism** — it is NOT part of default.py and cannot be patched by the self-improvement mission. This breaks the bootstrapping paradox: even if the self-improvement mission patches default.py (Level 1.5), it cannot modify the validator. The validator is Rust-side infrastructure (Stable Infrastructure layer) that gates all component writes before they enter the queue. The orchestrator calls the validator via a host function (`__validate_component__`); it cannot alter the validator's logic.
- **LLM code-audit for Orchestrator/Scaffold changes (Step-1→Step-2 gate).** For Orchestrator-class (10) and Scaffold-class (50) components specifically, the Q1→Q2 transition includes an extra step: a **kohai-provider LLM code-audit** that checks for security issues from the self-modification looping problem (validator bypass, infinite recursion, privilege escalation through self-patching, sandbox escape, secret exfiltration). The audit prompt is a minimal, Rust-side-constructed prompt (no orchestrator involvement) sent to the kohai provider. The Q2 manual "Validate" button is **disabled** in the WebUI until the LLM audit returns clean. If the audit flags issues, the component stays in Q1 and is routed to Q3 (revision) with the audit findings attached as `review_feedback`.

### 3.5.1 Validator tag + 4-queue validation lifecycle
- **The validator tag (code `05`, see §3.9) is special.** Every component that is **new or an updated version of an existing component** is automatically tagged with the validator tag during the build/import/patch process. While a component carries the validator tag:
  - **All its other tags are "greyed out" (inactive).** The component is **not** delivered to any consumer (Rusty/Monty/Orchestrator/LLM/Scaffold) even if it carries those tags, because the validator tag gates them off.
  - **Greyed-out tags can still be toggled** (tag/untag) in the WebUI so the operator can pre-set the intended consumer audience while the component is still in validation. The toggles persist; they just have no behavioral effect until the validator tag is removed.
  - **Update-candidates inherit the active version's tags as greyed-out.** When a new version of an existing component is created (e.g. a self-improvement mission patches the orchestrator, or an operator edits a skill), the update-candidate row is seeded with the same tag set as the currently-active validated version, but all non-validator tags are greyed out because the update-candidate carries the validator tag. This preserves the intended audience across version bumps without letting an unvalidated candidate reach production consumers.
  - **On final manual validation (Step 2, user clicks "Validate"), the validator tag is automatically removed.** The formerly greyed-out tags become active, and the component is immediately deliverable to the consumers it is tagged for.
- **A component carrying the validator tag must be in exactly one of 4 queues:**

  | Queue | Code | Purpose | Existing today? |
  |-------|------|---------|-----------------|
  | **Q1 — Auto-validation queue** | `q1_auto` | Step 1 automatic validation (`RecipeValidator` + `SimilarityChecker` + code validation). On pass → `AutoPassed`, moves to Q2. On fail → `AutoFailed`, moves to Q3 (revision) or Q4 (rejection) depending on failure class. | Partially: `RecipeReviewService` mutates status via engine `Store` directly. Formalize as a named queue. |
  | **Q2 — Manual validation queue** (WebUI) | `q2_manual` | Step 2 manual operator review, visible + editable in the WebUI Settings → **Validation** tab. On approve → `Validated` (validator tag removed, other tags activate). On reject → Q4. **For Orchestrator (10) + Scaffold (50) components:** the "Validate" button is **disabled** until the LLM code-audit (§3.5) returns clean. The audit findings are shown inline in the Q2 review panel. | **Yes:** `list_validation_queue` route filters `AutoPassed` / `ReviewRequested` / `UpgradeQueued` via `is_queue_status`. Review this existing tab and integrate. |
  | **Q3 — Revision queue** | `q3_revision` | Components rejected by Q1 or Q2 that are repairable. The **revision mission** — a scheduled Extension (class 09, tagged `01:monty`) connected to kohai/sempai — runs on schedule when the LLM is not busy. It reads rejected components from Q3, uses the kohai/sempai LLM to propose repairs based on `review_feedback`, and re-submits repaired candidates to Q1. On successful repair → re-enter Q1. After 3 failed review cycles → Q4. The revision mission is itself a validated Extension (goes through the same two-step validation gate). | **No:** rejected components have `review_attempts` + `rejected_at` but no automated revision path. **New work.** |
  | **Q4 — Rejection queue** | `q4_rejection` | Components that failed 3 review cycles or are unrepairable. Wait a configurable retention window (`q4_retention_days` in `reborn_monty_vm_settings`, default 30 — spec §7 Q7 resolved). After the window: either re-sent to Q3 (operator-initiated re-review) or **deleted and their creation-process data wiped** (provenance, similarity parent, source thread id, review feedback). | Partially: the 30-day → `Garbage` transition is described in `ValidationStatus` doc but the deletion+wipe is **not** implemented. **New work.** |

- **Queue-state invariant:** the `validation_status` field + the `validator_tag` presence together encode the queue. `Pending`/`AutoFailed` ↔ Q1; `AutoPassed`/`ReviewRequested`/`UpgradeQueued` ↔ Q2; `Rejected` with `review_attempts < 3` ↔ Q3; `Rejected` with `review_attempts >= 3` + `rejected_at` age < `q4_retention_days` (§3.10, default 30) ↔ Q4; `Garbage` ↔ Q4-post-window-awaiting-wipe. The `is_valid_transition` guard in `recipe_store.rs` is the existing seam; extend it to cover Q3↔Q1 (revision re-submit) and Q4→wipe transitions (§3.5.2).
- **Reward/scoring telemetry stays immediate-write** (§3.6) regardless of queue — usage metrics accrue even while a component is in Q3/Q4 so a repaired+re-validated component retains its track record.

### 3.5.2 WebUI validation tab — full route extension (Q9 resolved)
- **Existing infrastructure reviewed.** The current WebUI validation tab consists of:
  - **Routes** (`router.rs:194-221`): `GET /validation-queue`, `GET /validation-queue/count`, `PUT /recipes/{id}/validate`, `PUT /recipes/{id}/reject`, `PUT /recipes/{id}/review-request`, `PUT /tool-skills/{id}/validate`, `PUT /tool-skills/{id}/reject`, `PUT /tool-skills/{id}/review-request`, `POST /recipes/{id}/outcomes`.
  - **Handlers** (`handlers.rs:1041-1175`): thin wrappers calling `state.services().*`.
  - **Services** (`reborn_services.rs:2985-3170`): delegate to `recipe_store` methods (`list_validation_queue`, `count_by_status`, `update_recipe_validation_status`, `update_tool_skill_validation_status`).
  - **Store** (`recipe_store.rs`): `list_validation_queue` loads `DocType::Recipe` + `DocType::ToolSkill` MemoryDocs, filters by `is_queue_status` (AutoPassed/ReviewRequested/UpgradeQueued). `is_valid_transition` guards: AutoPassed→Validated, AutoPassed→Rejected, AutoPassed→ReviewRequested, ReviewRequested→Rejected, UpgradeQueued→Rejected. `count_by_status` counts by status label.
  - **ValidationStatus enum** (`recipe.rs:46-56`): Pending, UpgradeQueued, AutoFailed, AutoPassed, Validated, ReviewRequested, Rejected, Garbage.
  - **Frontend:** no dedicated validation page component found in `brassclaw_webui_v2_static/src/` — the validation queue is rendered as part of the Recipe Manager tab. The API lib (`lib/api.js`) has validation-related fetch paths.
- **Problem:** the existing routes are **recipe/tool_skill-specific** — they hardcode `DocType::Recipe` and `DocType::ToolSkill`. In the new design with ~20 class codes, this must be generalized. The `is_queue_status` function only covers Q2; `is_valid_transition` doesn't cover Q3↔Q1 (revision re-submit) or Q4→wipe.
- **Generalized route scheme** (replaces the recipe/tool_skill-specific routes):

  | Route | Method | Queue | Purpose | Replaces |
  |-------|--------|-------|---------|----------|
  | `GET /api/webchat/v2/validation-queue?q=auto` | GET | Q1 | List `AutoFailed` items (operator visibility into Step-1 failures) | New (extends existing `list_validation_queue` with `q` param) |
  | `GET /api/webchat/v2/validation-queue?q=manual` | GET | Q2 | List `AutoPassed`/`ReviewRequested`/`UpgradeQueued` items (Step-2 manual review) | Existing `list_validation_queue` (generalized to all classes) |
  | `GET /api/webchat/v2/validation-queue?q=revision` | GET | Q3 | List `Rejected` items with `review_attempts < 3` (in revision) | New |
  | `GET /api/webchat/v2/validation-queue?q=rejection` | GET | Q4 | List `Rejected` items with `review_attempts >= 3` + `rejected_at` age < retention window | New |
  | `GET /api/webchat/v2/validation-queue/count?q={queue}` | GET | all | Count per queue (tab badges) | Existing `count_validation_queue` (generalized with `q` param) |
  | `PUT /api/webchat/v2/components/{class_code}/{component_id}/validate` | PUT | Q2→Validated | Step-2 manual validation (pop validator tag, activate consumer tags) | `validate_recipe` / `validate_tool_skill` (generalized) |
  | `PUT /api/webchat/v2/components/{class_code}/{component_id}/reject` | PUT | Q2/Q3→Q4 | Reject (move to Q4 rejection queue) | `reject_recipe` / `reject_tool_skill` (generalized) |
  | `PUT /api/webchat/v2/components/{class_code}/{component_id}/send-to-revision` | PUT | Q2/Q4→Q3 | Send to revision queue (operator-initiated re-review) | `request_recipe_review` / `request_tool_skill_review` (generalized) |
  | `PUT /api/webchat/v2/components/{class_code}/{component_id}/re-review` | PUT | Q4→Q3 | Operator-initiated re-review from Q4 (re-send to revision before retention window expires) | New |
  | `DELETE /api/webchat/v2/components/{class_code}/{component_id}` | DELETE | Q4→wipe | Terminal wipe (after retention window; guarded — deletes component row + creation-process provenance, never thread data) | New |
  | `GET /api/webchat/v2/components/{class_code}/{component_id}/audit-status` | GET | Q2 (class 10/50) | LLM code-audit status + findings for Orchestrator/Scaffold components | New |
  | `GET /api/webchat/v2/components/{class_code}/{component_id}/revision-history` | GET | Q3 | Revision attempts + feedback history for a component in Q3 | New |

- **LLM code-audit guard (class 10 + 50 only):** the `PUT /components/{class_code}/{id}/validate` handler checks the LLM-audit-clean flag before allowing validation for class 10 (Orchestrator) and 50 (Scaffold) components. If the audit hasn't run or flagged issues, the handler returns `403 Forbidden` with `{error: "llm_audit_pending", findings: [...]}`. The WebUI "Validate" button is **disabled** in the Q2 review panel until `GET /components/{class_code}/{id}/audit-status` returns `{status: "clean"}`. The audit findings are shown inline in the Q2 review panel.
- **`is_queue_status` extension** (covers all 4 queues):
  - Q1 (auto): `Pending` OR `AutoFailed`
  - Q2 (manual): `AutoPassed` OR `ReviewRequested` OR `UpgradeQueued`
  - Q3 (revision): `Rejected` AND `review_attempts < 3`
  - Q4 (rejection): `Rejected` AND `review_attempts >= 3` AND `rejected_at` age < `q4_retention_days`
  - Post-window (awaiting wipe): `Garbage`
- **`is_valid_transition` extension** (new transitions added):
  - `AutoFailed → Pending` (Q1 auto-fail → Q3 revision repair → re-submit to Q1)
  - `Rejected → Pending` (Q3 revision repair → re-submit to Q1)
  - `Rejected → Garbage` (Q4 retention window expired → terminal)
  - `Garbage → (deleted)` (terminal wipe — handled by DELETE route, not a status transition)
  - Existing transitions retained: `AutoPassed → Validated`, `AutoPassed → Rejected`, `AutoPassed → ReviewRequested`, `ReviewRequested → Rejected`, `UpgradeQueued → Rejected`.
- **`ValidationQueueItem` response shape** (extended for all 4 queues):
  ```
  {
    component_id, class_code, class_label, name, description,
    queue_code ("q1_auto"|"q2_manual"|"q3_revision"|"q4_rejection"|"garbage"),
    validation_status, validation_errors[], review_feedback,
    review_attempts, rejected_at, validator_tag_present (bool),
    consumer_tags[] (with greyed-out state derived from validator_tag_present),
    llm_audit_status ("pending"|"clean"|"flagged"|"n/a"),  // class 10/50 only
    llm_audit_findings[],  // class 10/50 only
    created_at, updated_at
  }
  ```
- **Frontend — 4 queue tabs** (replaces the single Recipe Manager validation tab):
  - **Q1 Auto tab:** shows `AutoFailed` items with `validation_errors[]` inline. Read-only (operator visibility — the auto-validator runs server-side, no manual action here). Badge count.
  - **Q2 Manual tab:** shows `AutoPassed`/`ReviewRequested`/`UpgradeQueued` items with full component editor (content, tags, intent examples). "Validate" button (pops validator tag) + "Reject" button (sends to Q4). For class 10/50: "Validate" button disabled until LLM audit clean; audit findings shown inline. Badge count.
  - **Q3 Revision tab:** shows `Rejected` items with `review_attempts < 3` + `review_feedback`. Read-only (the scheduled revision Extension handles repairs automatically). Shows revision history per component. "Send to Q4" button (operator override — skip revision). Badge count.
  - **Q4 Rejection tab:** shows `Rejected` items with `review_attempts >= 3` + `rejected_at` age. "Re-review" button (Q4→Q3, operator-initiated). "Delete permanently" button (terminal wipe, confirmation dialog with retention window status). Badge count.
  - **Tag chip rendering:** each component editor in Q2 shows the tag set as toggleable chips. While `validator_tag_present` is true, non-validator chips render **greyed** but remain toggleable. The `05:validator` chip is read-only. On validation (Step 2), the validator tag pops and chips activate.
  - **Monty VM tab** (§3.10): shows resource limits, active orchestrator pointer, failure-rollback threshold, prior-knowledge token budget, `q4_retention_days` (Q7), "Restart Monty" button. Read-only host-function extensions display.
- **Route backward compatibility:** the old recipe/tool_skill-specific routes (`PUT /recipes/{id}/validate` etc.) are kept as aliases during the migration period, projecting to the generalized `PUT /components/{class_code}/{id}/validate` with `class_code` inferred from the route prefix. They are removed in Phase 7 (cleanup).
- **Per-class validation configuration (spec §7 Q14 resolved):** only Skills (classes 01-03) require the full agentskills.io validation (name pattern, description length + actionable, token budget 5000, activation criteria, `allowed_tools` presence, `param_schema`/`param_template` validation). All other component classes use **lighter validation** — name format + description length + content non-empty + soft token budget (warnings, not hard errors). Each class's validation thresholds are **configurable in the WebUI Settings → Validation tab** via a per-class configuration panel:
  - **`reborn_validation_config` table** (new): `(scope, class_code, name_min_len, name_max_len, name_pattern, description_min_len, description_max_len, token_budget, token_budget_hard_error, require_tool_name, require_param_schema, require_activation_criteria, updated_at)`. One row per `(scope, class_code)`.
  - **Defaults per class:**
    - **Skills (01-03):** full agentskills.io validation — `name_pattern = ^[a-z0-9-]+$`, `description_min_len = 10`, `token_budget = 5000`, `token_budget_hard_error = true`, `require_tool_name = false`, `require_param_schema = false`, `require_activation_criteria = true`.
    - **Tools (00):** `token_budget = 5000`, `token_budget_hard_error = true`, `require_tool_name = true`, `require_param_schema = true`, `require_activation_criteria = false`.
    - **Extensions (04-09):** `token_budget = 10000`, `token_budget_hard_error = false`, `require_tool_name = false`, `require_param_schema = false`, `require_activation_criteria = false`.
    - **Actions (16):** **no token budget** (Actions are exempt from size limits — §3.11), `require_activation_criteria = false`, `require_tool_name = false`.
    - **Former doctypes (12-15, 17-20):** `token_budget = 10000` (Notes: 2000), `token_budget_hard_error = false` (soft warning), `require_tool_name = false`, `require_param_schema = false`, `require_activation_criteria = false`.
    - **Recipes (21):** `token_budget = 10000`, `token_budget_hard_error = false`, `require_activation_criteria = true` (trigger validation), `require_tool_name = false`, `require_param_schema = false`.
    - **Orchestrator (10) + Scaffold (50):** code validation (LLM audit) — `token_budget = 50000`, `token_budget_hard_error = false`.
  - **WebUI Validation tab:** a sub-panel "Validation Config" shows each class code with its current thresholds as editable fields. Changes are immediate-write (knobs, not content/code) but do not retroactively re-validate existing components — they apply to the next validation cycle.
  - **Validator dispatch:** the validator (`ComponentValidator` — renamed from `RecipeValidator`) reads the `reborn_validation_config` row for the component's `class_code` and applies the corresponding checks. The dispatch is: `validate_by_class(class_code, component, config_row, available_tools, existing_skill_names)`.

### 3.6 Validation split (gated vs immediate-write)
| Write kind | Columns / payloads | Path |
|------------|---------------------|------|
| **Validation-gated** | code (orchestrator, Monty payloads), content (name/description/body/class/license/allowed_tools), activation (keywords/patterns/budget/gating), **tag membership** (the set of consumer tags — gated because it controls delivery), tool param schemas, extension payloads, `validation_status` (re-validates by definition) | save → Step-1 validation + content-safety + code validation → commit only on pass; Step-2 manual gate for new/updated components. **The validator tag is added/removed by the queue lifecycle, not by direct edit** (§3.5.1). |
| **Immediate-write (no validation)** | reward/scoring telemetry: `tier`, `usage_count`, `success_count`, `failure_count`, `wilson_lower`, `confidence` (derived), `ema_success`, `ema_latency`, `sample_count`, `last_audit_at`, `audit_failure_count`, `review_attempts`, `source` (provenance) | save directly |

> `trust` is gone (§3.5), so the earlier trust/source gated-vs-immediate question is moot.

- **Self-improvement mission writes are validation-gated.** Today the self-improvement mission uses `memory_write` to directly patch `prompt:codeact_preamble` (Level 1) and `orchestrator:main` (Level 1.5) — no validation gate. Under the new design, these writes are **rerouted through the validation gate**: a `memory_write` for a code/component change creates an **update-candidate** component (with `05:validator` tag, greyed-out consumer tags) that enters Q1. The mission's `memory_write` host function is intercepted by the Rust bridge and routed to `__validate_component__` instead of writing directly. The existing 3-failure auto-rollback (`load_orchestrator` in `orchestrator.rs`) is retained as a safety net behind the validation gate. Reward/scoring telemetry from mission threads remains immediate-write.

### 3.7 Prompt composition ordering (KV-cache discipline)
- Every "static" DB object (skill/tool/extension — **not** volatile memories) gets a **`class_code`** (2-digit) + a **`prompt_uid`** (monotonic, never reordered).
- **Class codes — Tools-first, foundation-first** (lower = more foundational = deeper in the cache prefix). Future classes get the next number; existing codes are never renumbered.

| Code | Class |
|------|-------|
| 00 | Tool (Rusty capability surface) |
| 01 | Skill (Rusty) |
| 02 | Skill (Monty) |
| 03 | Skill (LLM) |
| 04 | Extension (Rusty) |
| 05 | Extension (Monty) |
| 06 | Extension (MCP-Server) |
| 07 | Extension (MCP-Client) |
| 08 | Extension (LLM) |
| 09 | Extension (Misc — excludes Recipes, see class 21) |
| 10 | Orchestrator (self-modifiable Python loop driver — runs inside Monty, NOT in the prompt) |
| 11 | _(reserved — formerly Scaffold, renumbered to 50)_ |
| 12 | Spec (missing-capability request / spec document — former `DocType::Spec`) |
| 13 | ToolSkill (tight description of ONE tool usage pattern — former `DocType::ToolSkill`) |
| 14 | Plan (structured execution plan with steps, status, progress — former `DocType::Plan`) |
| 15 | Summary (thread-completion summary — former `DocType::Summary`) |
| 16 | Actions (LLM-free deterministic execution unit — see §3.11) |
| 17 | Docu (general documentation / reference articles) |
| 18 | Lesson (durable learning from experience — former `DocType::Lesson`) |
| 19 | Issue (detected problem for follow-up — former `DocType::Issue`) |
| 20 | Note (working memory / scratch notes — former `DocType::Note`) |
| 21 | Recipe (cookbook entry: trigger + ordered tool-skill invocation steps — former `DocType::Recipe`; solution-class with `override_prompt_creation` + `prior_knowledge_content` — spec §7 Q15 resolved) |
| 22–49 | reserved for future classes |
| 50 | Scaffold (base system-prompt text: preamble / postamble / platform / overlay — IS the prompt base, not append-ordered) |
| 51+ | reserved for future classes |

- **Composition rule:** static objects are appended at the **bottom** of the system prompt, ordered by `(class_code asc, prompt_uid asc)`. Volatile memories (prior-knowledge) are injected as a **User message at N-1** (the resurrected path from `deduplication-plan.md` Finding 1 Option A), so they never mutate the cached system prefix. Same selection → same bytes → KV-cache hit.
- **Scaffold (50) is special:** it is the **base** of the system prompt, not an appended object. Tools/skills/extensions are appended *to* it. Its `class_code` is for tagging + WebUI organization, not for the append-ordering. Placing it at 50 (high) ensures it sorts last in any accidental append-ordering, reinforcing its role as the base layer. **Orchestrator (10) is special:** it is not in the prompt at all — it *runs* the assembly. Its `class_code` is for tagging + WebUI organization only.

### 3.8 Proposals for removing other file chunk systems
1. `brassclaw_embeddings`: **fully removed.** The crate, its dependencies, and all embedding-based search paths are deleted. The intent system (§3.12) replaces all runtime similarity/search needs. Install-time dedup is handled by the validator's content-hash check + exact-name uniqueness constraint (no embedding similarity needed).
2. Chat memory chunking (`pg_chat_memory_record_store.rs`, `chat_memory.rs`, `MemoryChunkWrite.chat_record_id`): stop writing chat chunks; chat records become flat `Note` MemoryDocs (class 20) with no embedding index, retrieved by the intent system or project-scope lookup like any other component.
3. Index specs / root filesystem events (migrations V29/V30): filesystem metadata only, no retrieval index.
4. Skill migration blob (`migrated_skills.rs`, `bundled_skills.rs`): replaced by the one-shot DB import migration; deleted.

### 3.9 Component tag system (consumer gating, orthogonal to class)
- **Tags are orthogonal to classes.** A component has **exactly one `class_code`** (what it *is*) but **zero or more consumer tags** (who may *receive* it). Tags gate visibility per consumer, saving tokens: a component not tagged for a consumer never enters that consumer's turn.
- **Tag codes** (2-digit, assigned in discovery order; future tags get the next number, never renumbered):

  | Tag code | Tag | Consumer | Meaning |
  |----------|-----|----------|---------|
  | `00` | `rusty` | Rusty (Tier 0 host) | Component is delivered to the Rust host/tool execution layer. Tool definitions (class 00) always carry this. |
  | `01` | `monty` | Monty (the Python VM) | Component is delivered to the Monty VM — either as a host-function binding (extension) or as a callable the orchestrator may invoke (skill). |
  | `02` | `orchestrator` | Orchestrator (the loop driver) | Component is delivered to the orchestrator's turn context — skills it may select, extensions it may load, scaffold sections it assembles into the prompt. |
  | `03` | `llm` | LLM (the model) | Component is delivered into the prompt the LLM sees — LLM-class skills, scaffold sections, prompt-template extensions. |
  | `04` | `scaffold` | Scaffold (prompt base) | Component is a scaffold section (preamble/postamble/platform/overlay) and is assembled into the prompt base, not appended. |
  | `05` | `validator` | Validator (queue lifecycle) | Component is in a validation queue (§3.5.1). **Special:** while present, all other tags are greyed out / inactive. Auto-added on create/update; auto-removed on Step-2 validation. |
  | `06+` | reserved | reserved for future consumers | — |

- **Delivery rule:** at turn-assembly time, `RetrievalSource::fetch_for_consumer(consumer_tag)` returns only components that (a) carry the requested consumer tag, (b) do **not** carry the `validator` tag, and (c) are `validation_status == Validated`. A component tagged `monty` + `orchestrator` but **not** `llm` never enters the LLM's prompt — the orchestrator may call it, but the LLM never sees its body. This is the token-saving mechanism.
- **Tag storage:** tags are stored as an explicit `tags[]` text array column on every component table (skills/tools/extensions/orchestrators/scaffolds), with a CHECK constraint that each entry matches `^[0-9]{2}(:[a-z0-9-]+)?$` (code + optional slug). The validator tag is `05:validator`. Greyed-out state is derived (not stored) — a tag is greyed iff the row also carries `05:validator`. This means the same `tags[]` column serves both validated and in-validation rows without a separate "pending tags" column.
- **Update-candidate tag inheritance (§3.5.1):** when a new version row is created, its `tags[]` is seeded as `active_version.tags[] ∪ {05:validator}`. The validator tag greys out the inherited tags; on Step-2 validation the validator tag is popped and the inherited audience takes effect immediately.
- **WebUI:** each component editor shows the tag set as toggleable chips; while the validator tag is present the non-validator chips render greyed but remain toggleable. The class code is shown read-only (class is intrinsic to the row).

### 3.10 Monty VM settings (DB-stored, editable)
- Monty is the embedded Python VM running the orchestrator + user code (Tier 1). Today its `ResourceLimits` are compiled-in constants (`orchestrator.rs:122-128`) overridable only by `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS`. In the new design these become **DB-stored, operator-editable settings** in `reborn_monty_vm_settings`, served through PlanA-memory's `RetrievalSource` (PostgresSource in production; compiled-in defaults via `RamSource` in DB-less mode).
- **Settings table** (single row per scope tuple — confirmed, spec §7 Q8 resolved; per-orchestrator-version granularity is unnecessary: if a future orchestrator version needs different limits, the operator changes the single row when switching `active_orchestrator_id`; a `reborn_monty_vm_settings_overrides` table keyed by `orchestrator_id` can be added later without breaking the base row):

  | Column | Type | Default | Notes |
  |--------|------|---------|-------|
  | `max_duration_secs` | int | `300` | Wall-clock budget for one orchestrator run. Floor 30, ceiling 3600 (existing clamps). |
  | `max_allocations` | bigint | `5_000_000` | PyObject allocation cap. |
  | `max_memory_bytes` | bigint | `134_217_728` (128 MB) | Heap cap. |
  | `failure_rollback_threshold` | int | `3` | Consecutive failures before auto-rollback to previous orchestrator version. |
  | `active_orchestrator_id` | uuid | null | FK to `reborn_orchestrators.id`; null → compiled-in `DEFAULT_ORCHESTRATOR`. |
  | `prior_knowledge_token_budget` | int | `2000` | Max tokens to spend on prior-knowledge assembly per turn (replaces the hardcoded 5-doc limit). Editable in WebUI. |
  | `q4_retention_days` | int | `30` | Q4 rejection queue retention window in days before terminal wipe (spec §7 Q7 resolved). Operator can shorten for dev profiles (e.g. `local_yolo` → 1 day) or lengthen for hosted production (e.g. 90 days). The wipe guard reads this value instead of a hardcoded constant. |

- **Scope:** `(tenant_id, user_id, agent_id, project_id)` — same scope tuple as all other tables. DB-less mode uses compiled-in defaults via `RamSource`.
- **Validation:** settings changes are **immediate-write** (they are knobs, not content/code) but the `active_orchestrator_id` switch is gated by the validator (the pointed-to orchestrator row must be `Validated`). This prevents pointing Monty at an unvalidated orchestrator.
- **Monty has extensions (host functions), not skills.** The Python↔Rust bridge functions (`__llm_complete__`, `__execute_action__`, `__retrieve_docs__`, `__regex_match__`) are Monty-class extensions in `reborn_extensions_unified` (class 05, tag 01). They are not user-editable code; they are kernel-owned host functions. The WebUI shows them read-only.
- **Monty has no self-optimizing routines.** The self-improvement mission patches the orchestrator Python (class 10), not Monty the Rust VM. Monty's failure classification (`VmPanic`/`RuntimeError`/`ResourceLimit`/`ToolError`/`OsDenied`) feeds the rollback counter but is not itself a tuning surface.
- **WebUI:** a "Monty VM" sub-section under Settings (or its own tab) shows the resource limits as editable fields, the active orchestrator pointer, the failure-rollback threshold, and the prior-knowledge token budget. Read-only display of the host-function extensions. **A "Restart Monty" button** stops the current Python VM instance, applies the new settings from `reborn_monty_vm_settings`, and starts a fresh instance. A confirmation dialog warns that restart interrupts any in-flight turns. A status indicator shows whether Monty is running, restarting, or stopped. The restart is a **kernel-owned runtime operation** — it goes through the kernel's Monty VM lifecycle manager, not through default.py (which runs inside Monty and cannot restart itself).

### 3.11 Actions class (code 16 — LLM-free deterministic execution by default.py)
- An **Action** is a self-contained deterministic procedure — a **complete plan** with parts for Rusty (tool calls via the Rust bridge) and parts for default.py (orchestration instructions) — that default.py executes **without an LLM call** when the intent system (§3.12) matches the user's query to an Action. Actions do **not** conform to skill creation rules or size limits — they can be as large as needed to describe a complete task.
- **Schema (`reborn_actions` table, §4):**
  - `name`, `description` (shown in disambiguation UI), `class_code` = 16, `prompt_uid`, `consumer_tags[]` (default `{01:monty,02:orchestrator}`; `05:validator` while in queue).
  - `intent_examples jsonb` — array of `{input, class}` fed to the intent system.
  - `preconditions jsonb` — array of condition objects evaluated before execution: `{type: "workspace_exists", path: ".git"}`, `{type: "env_set", name: "GITHUB_TOKEN"}`, `{type: "tool_available", tool: "shell"}`.
  - `steps jsonb` — ordered array of step descriptors. Each step is one of:
    - `{type: "tool_call", tool: "shell", params: {command: "git status"}, save_output_as: "git_status", on_error: "abort"}`
    - `{type: "conditional", condition: {op: "contains", var: "git_status", value: "nothing to commit"}, then_step: 5, else_step: 2}`
    - `{type: "set_var", var: "branch_name", value: "auto-fix-{$date}"}`
    - `{type: "loop", over: "items", var: "item", body_step: 3, exit_condition: {op: "eq", var: "done", value: true}}`
    - `{type: "return", value: "Done: committed {$commit_count} files"}`
    - `{type: "evaluate", expr: "len(git_status.splitlines()) > 0", save_result_as: "has_changes"}` — run a Python expression in default.py's scope to evaluate a tool result and decide next steps. Richer than `conditional` for real-world result evaluation.
    - `{type: "call_skill", skill: "git_commit_skill", params: {message: "auto-fix"}, save_output_as: "commit_result"}` — invoke a Rusty/Monty skill (not just a raw tool) as part of the Action.
    - `{type: "try_catch", try_steps: [3, 4, 5], catch_steps: [6, 7], on_error_var: "err"}` — wrap a sub-sequence of steps with per-block error recovery (try tool calls, catch an error, run a fallback).
    - `{type: "parallel", steps: [{tool: "fetch", params: {url: "url1"}}, {tool: "fetch", params: {url: "url2"}}], join: "all", save_outputs_as: "fetch_results"}` — run multiple tool calls concurrently and wait for all (or first-success).
    - `{type: "call_action", action: "deploy_action", params: {env: "staging"}}` — invoke another Action (Action-to-Action chaining).
    - `{type: "spawn_subprocess", command: "cargo test", args: ["--release"], cwd: "{$workspace}", save_output_as: "test_output", timeout_secs: 120}` — spawn a subprocess via the **host runtime's script lane** (`services/script_runtime` per AGENTS.md) — NOT a raw `subprocess.Popen`. The script lane enforces capability lease + approval gate + sandbox boundary (SEC-08, §6.1). The Action's `allowed_tools[]` must include `spawn_subprocess` explicitly. `command`/`args`/`cwd` are validated against the script lane's allowlist. Output is captured; timeout bounds the run.
    - `{type: "wait", duration_secs: 5, condition: {op: "file_exists", path: "/tmp/done.flag"}}` — pause execution for a fixed duration or until a condition is met. Supports polling conditions (file_exists, env_set, var_eq) with a timeout. Useful for waiting on async side effects (e.g. wait for a deployment to finish, wait for a file to appear).
    - `{type: "emit_event", event: "action_completed", payload: {action: "deploy", status: "success"}}` — emit a structured event to the event bus (for webhook triggers, extension notifications, or operator dashboards). The event is dispatched via the existing event system (`brassclaw_events` table, migration V013). This enables event-driven Actions: one Action's `emit_event` can trigger another Action's execution via a trigger binding.
  - `error_handling jsonb` — `{on_tool_error: "abort_and_report", on_precondition_fail: "abort_and_report", on_timeout: "abort_and_report", retry: {max_attempts: 1, backoff_secs: 0}}`. Per-block error handling is via `try_catch` steps; the global `error_handling` is the fallback for uncaught errors.
  - `timeout_secs` — wall-clock budget for the whole Action.
  - `allowed_tools[]` — capability subset this Action may call. **Enforced at BOTH layers** (SEC-07, §6.1): default.py checks before dispatch (Python tunable) AND the Rust `EffectExecutor` bridge checks before execution (Rust stable — defense in depth). `spawn_subprocess` must be in `allowed_tools[]` for `spawn_subprocess` steps.
  - `param_schema jsonb` / `param_template jsonb` — parameters the caller may pass or that the intent system extracts from the query.
  - Same validation/queue/lineage columns as all components.
- **Size limit exemption + hard limits (PERF-18, §6.1):** Actions are **exempt from the `prior_knowledge_token_budget` truncation** (§3.13). When the RetrievalEngine fetches an Action, the Action's full content is included in prior knowledge regardless of the token budget. The budget applies to other retrieved components, not to the Action being executed. This is because an Action is a complete procedure — truncating it would break execution. **Hard limits** (compiled-in, not configurable): max content size = 256KB, max step count = 500, max `allowed_tools[]` = 50. An Action exceeding any limit is rejected at validation.
- **Recursion bounding (SEC-09, §6.1):** `call_action` chaining is bounded: max depth = 5, cycle detection (call stack tracked), total step budget = 1000 across all nesting levels. Enforced in default.py with Rust-side `timeout_secs` hard cap.
- **Execution path:** Actions are executed by **default.py** (the Python orchestrator), not by a separate Rust `ActionExecutor`. The flow:
  1. **Step 0:** the orchestrator sends the goal query to `__resolve_intent__(goal, "02")`.
  2. The intent system responds with the unique id of an Action element (class_code 16).
  3. The RetrievalEngine pulls that Action element by id (same as any other component retrieval).
  4. **Prior knowledge is created as usual** — the Action's content is formatted by `format_action` (§3.13) and included in the prior-knowledge section. The Action is exempt from token-budget truncation (full content included).
  5. Prior knowledge is given to default.py.
  6. default.py recognizes class_code 16 in the retrieved components and **stops further prompt creation** — does not call `__llm_complete__`, does not build an LLM prompt.
  7. default.py **performs the Action directly** by following its instructions: tool calls go through the normal Rust bridge (`EffectExecutor`), result evaluation via `evaluate`/`conditional`, branching, looping, skill invocation, parallel execution, error recovery.
  8. The Action's `return` value becomes the turn result.
- **No separate Rust executor:** there is no `action_executor.rs` file and no `__execute_action_procedure__` host function. default.py is the executor. Tool calls within an Action go through the same Rust `EffectExecutor` bridge as any other tool call — the Action just orchestrates them deterministically without LLM decisions.
- **Step vocabulary is Python tunable logic:** since default.py interprets the step descriptors, new step types can be added by patching default.py (through the validation gate), not by changing Rust code. The step vocabulary is extensible without Rust changes.
- **`allowed_tools[]` enforcement (defense in depth — SEC-07, §6.1):** default.py checks each `tool_call`/`call_skill`/`spawn_subprocess` step against the Action's `allowed_tools[]` before dispatch (Python tunable), AND the Rust `EffectExecutor` bridge checks the calling Action's `allowed_tools[]` before executing any tool (Rust stable). The Rust bridge receives the Action's `allowed_tools[]` as part of the turn context, not from the orchestrator's self-reported list. A tool not in `allowed_tools[]` is rejected at both layers.
- **WebUI:** a "Skills & Actions" tab in Settings (§5 Phase 6) showing Actions with their intent examples, step editor (visual step list or JSON, draggable, all 13 step types), preconditions, error handling, `allowed_tools[]` multi-select, and a test runner ("dry-run this Action against a sample query"). Actions are shown without size limits — the editor does not enforce a token budget on Action content.

### 3.12 Intent system (unified DB-driven routing)
- **Replaces** all 8 intent-detection mechanisms listed in §2.3a: `signals_tool_intent()`, `signals_execution_intent()`, `score_skill()` keyword matching, `RetrievalEngine::extract_keywords()`, `extract_explicit_skills()`, `RecipeTrigger` matching, and v1 Rust `llm_signals_tool_intent()`/`user_signals_execution_intent()`.
- **`reborn_intent_inputs` table (§4):**
  - `id` uuid, `scope` tuple, `input_text` text, `input_class` int (1=word, 2=partial sentence, 3=full sentence, 4=keyword from RetrievalEngine fallback), `component_id` uuid FK, `component_class_code` int (denormalized for fast filtering), `score` int (starts at 1, +1 per disambiguation win), `source` text (`seeded`/`learned_user`/`learned_llm`/`learned_fallback`), timestamps.
  - Composite unique: `(scope, input_text, input_class, component_id)`.
  - Indexes: `(scope, input_text)`, `(scope, input_class, input_text)`, `(scope, component_id)`, GIN trigram on `input_text` for fuzzy partial matching. **The `pg_trgm` extension is installed at brassclaw installation time** (spec §7 Q16 resolved) — the standalone/embedded Postgres setup script runs `CREATE EXTENSION IF NOT EXISTS pg_trgm` alongside the existing `pgvector` extension (`V000__shared_triggers.sql`). For external Postgres operators, the installation script checks for `pg_trgm` availability and fails with a clear error if not installed (same pattern as `pgvector`). The GIN trigram index is created in the `reborn_intent_inputs` migration.
- **Every component table** (skills/tools/extensions/actions/plans/lessons/etc.) gets an `intent_examples jsonb` column: `[{input: "commit my changes", class: 3}, {input: "git commit", class: 2}, {input: "commit", class: 1}]`. On component validation, these are **exploded into `reborn_intent_inputs` rows** — one row per example, each with `score = 1`.
- **`__resolve_intent__(query, sender_class_code)` host function (Rust):**
  1. **Classify the query** (spec §7 Q10 resolved — simple heuristic sufficient for v1, covers all 4 input classes):
     - **Class 3 (full sentence):** ≥5 words OR ends with `.`/`!`/`?` (the `?` rule: a 3-word question like "why this fails?" is class 3 regardless of word count — questions are always full sentences).
     - **Class 2 (partial sentence):** 2–4 words AND does not end with `.`/`!`/`?`.
     - **Class 1 (single word):** exactly 1 word AND does not end with `.`/`!`/`?`.
     - **Class 4 (keyword from RetrievalEngine fallback):** not a user query classification — class 4 inputs are only created by the RetrievalEngine fallback path (`extract_keywords` → stop-word-filtered lowercase tokens sent one by one). The user query classifier never produces class 4. Class 4 is the keyword-fallback-only class.
     - **Refinement over the original heuristic:** the `?` rule is added because a short question ("why this fails?" — 3 words) is semantically a full sentence, not a partial. The `.` and `!` rules already existed; `?` is now treated the same way. A more sophisticated classifier (NLP sentence boundary detection) is deferred to a future tunable-logic upgrade — the classifier only affects match order, not correctness, and the learning mechanism compensates over time.
  2. **Match order** (per query class):
     - Query class 3 → search input class 3, then 2, then 1.
     - Query class 2 → search input class 2, then 3, then 1.
     - Query class 1 → search input class 1, then 2, then 3.
     - Query class 4 (keyword fallback only) → search input class 1, then 2, then 3 (keyword first, then partial, then full sentence — per §3.12 rule f-fallback).
  3. **Exact match (PERF-02, §6.1 — single query with CASE ordering):** `SELECT * FROM reborn_intent_inputs WHERE scope = ? AND input_text = ? AND input_class IN (ordered_set_for_query_class) AND component_class_code IN (allowed_for_sender) ORDER BY CASE input_class WHEN [first] THEN 0 WHEN [second] THEN 1 ELSE 2 END, score DESC LIMIT 10`. One query, one round-trip. The B-tree index on `(scope, input_text, input_class)` serves the exact-match path (PERF-01).
  4. **Single match with classes 3 or 2 on both sides (rule d):** increment score by 1, return `{component_id, component_class_code, score}`.
  5. **Multiple matches or scores within 2 points (rule f — spec §7 Q11 resolved):** return `{disambiguation: true, candidates: [top 3 by score]}`. The orchestrator emits a **special `disambiguation` chat message type** (not a regular text message) with clickable buttons. The message payload is `{type: "disambiguation", candidates: [{component_id, component_class_code, description, class_label}]}`. The WebUI renders each candidate as a button showing the component's `description` + `class_label`. On user click, the WebUI sends a structured payload `{disambiguation_choice: component_id}` directly back to `__resolve_intent__` via a host function call — no intent detection needed for the reply, no ambiguity. `__resolve_intent__` increments the clicked match's score by 1 and returns `{component_id, component_class_code, score}`. After a component wins 3 times for the same input, its score is 3+ points higher → no more asking. A regular text message ("1. commit changes, 2. git push, 3. ...") is explicitly rejected because it requires the user to type a number — friction, and the orchestrator would need another intent-detection round to parse the reply, defeating the purpose of the disambiguation.
  6. **No match (rule e):** return `{no_match: true}`.
- **No-match handling without LLM (rule e):** the orchestrator sends "Your query's intent could not be matched. Please try to ask in a different way." If the next query matches, the orchestrator asks "Does '[matched query]' and '[unmatched query]' have the same intent?" On "yes" → create a new `reborn_intent_inputs` row with the unmatched query + copy the matched component IDs. Source = `learned_user`.
- **No-match handling with LLM (rule f):** if a "kohai" LLM provider is connected, the orchestrator sends a minimal prompt (no additional text/tokens) asking for 5 alternative phrasings. Each alternative is run through `__resolve_intent__`. On match → user confirmation: "Your query's intent could not be matched and was reinterpreted. Does '[matching LLM sentence]' and your sentence '[unmatched sentence]' have the exact same meaning?" On "yes" → create new `reborn_intent_inputs` row with the unmatched query + copy the matched component IDs. Source = `learned_llm`. On "no" → try next alternative. After 10 alternatives (2 rounds × 5) with no match or all rejected → fall back to rule e.
- **"Try it with AI" fallback (rule f-fallback — spec §7 Q12 resolved):** after 10 LLM sentences with no match, the "reformulate your query" message includes a button **"or try it with AI"**. On click, the RetrievalEngine fallback is reactivated for that single query. **The fallback runs entirely in Rust** — the orchestrator passes a `fallback: true` flag to `__resolve_intent__`, and the Rust side runs the keyword extraction + class-4 matching without re-entering Python. The orchestrator only sees the final result:
  1. Keywords are extracted from the query (existing `extract_keywords` — stop-word-filtered lowercase tokens) **in Rust**.
  2. Keywords are sent to the intent system as **class-4 inputs** **in Rust** (no Python host-function call per keyword).
  3. Each keyword is run one by one through the Rust-internal intent matcher with match order: class 1 → 2 → 3 (keyword first, then partial, then full sentence).
  4. Non-matching keywords get marked with result 0; matching keywords get marked with all matched component IDs.
  5. After all keywords are processed, the component ID list is returned to the RetrievalEngine **in Rust**.
  6. The RetrievalEngine fetches all content for all unique IDs and builds a prompt **in Rust**.
  7. The orchestrator receives the assembled prior knowledge — no Python→Rust→Python round-trip.
  8. **Goal:** progressive learning — each fallback use teaches the intent system new mappings so fewer fallbacks are needed over time and fewer tokens are spent on the LLM.
- **"AI before User" flip switch (rule f-ai):** the WebUI chat window has a small flip-switch at the top labeled **"AI before User"**. When this switch is **ON**, the intent system changes its behavior after 10 unsuccessful LLM-sentence matches:
  - **No message is sent to the user.** The "reformulate your query" message + "or try it with AI" button are **suppressed**.
  - Instead, the RetrievalEngine fallback mechanism is **activated automatically and silently**: the query is run through the keyword-retrieval mode (same as the "try it with AI" fallback: `extract_keywords` → class-4 intent matching → component IDs), and the "prior knowledge" is created from the matched components.
  - The user simply sees the turn proceed with whatever prior knowledge the keyword fallback assembled — no interruption, no disambiguation prompt, no "reformulate" message.
  - When the switch is **OFF** (default), the behavior is as described in rules e/f/f-fallback: the user is asked to reformulate and offered the "or try it with AI" button.
  - **Rationale:** the "AI before User" mode prioritizes flow continuity over user confirmation. It assumes the AI (keyword fallback) is good enough to keep the conversation going without interrupting the user. The tradeoff is that the user is not asked to confirm whether the fallback result matches their intent — the system just proceeds. This mode is for operators who trust the keyword fallback and want minimal interruptions.
  - **Learning still applies:** if the keyword fallback produces component IDs, those IDs are used for prior knowledge but **no new `reborn_intent_inputs` rows are created** from the "AI before User" path (unlike the "try it with AI" button path which does learn). The rationale: the user did not confirm the match, so the system should not learn from an unconfirmed fallback. Learning only happens when the user explicitly confirms (rules d/e/f) or clicks the "try it with AI" button (rule f-fallback).
  - **Persistence (spec §7 Q18 resolved):** the switch defaults to **OFF**. It is **per-user** (not per-scope), stored in a `reborn_user_preferences` table (new — simple key-value: `(user_id, preference_key, preference_value)`; key = `ai_before_user`, value = `true`/`false`). The switch is **visible in the chat window only** — it is NOT shown in the Settings UI (it is a user UX preference, not an operator-managed configuration). The `reborn_user_preferences` table is not exposed in the Settings UI. The switch is **hidden/disabled in DB-less mode** (the intent system is overridden in DB-less mode, so the flip switch has no effect).
- **Architecture layer split:**
  - **Rust (Stable Infrastructure):** `__resolve_intent__` host function, DB queries, score increment logic, `reborn_intent_inputs` table, **class-4 keyword fallback matching (entirely Rust-side — §7 Q12 resolved: the orchestrator passes `fallback: true` flag, Rust runs extract_keywords + class-4 matching + component fetch + prompt assembly without re-entering Python)**.
  - **Python (Tunable LOGIC):** LLM rephrase loop, disambiguation UI message formatting, "try asking differently" message, user-confirmation dialogs, **"try it with AI" button logic (passes `fallback: true` flag to `__resolve_intent__` — the heavy lifting is Rust-side)**, **"AI before User" flip-switch logic** (suppress the reformulate message + silently pass `fallback: true` flag to `__resolve_intent__` when switch is ON), **Action execution (default.py follows Action step descriptors, enforces `allowed_tools[]`, interprets step vocabulary — all 13 step types)**.
  - **DB (Tunable VALUES):** `intent_examples` on each component, `reborn_intent_inputs` rows, scores, **`reborn_user_preferences` (AI before User switch state — §7 Q18 resolved)**.

### 3.13 Intent-driven retrieval + prior knowledge assembly (Q19 resolved — "content is king" + Solution Override)
- **Retrieval replaces "load all docs":** `RetrievalSource::fetch_for_turn(goal, sender_class_code, token_budget)` calls `__resolve_intent__` internally → gets matched component IDs + class codes → fetches those components from their tables by ID → also fetches volatile thread memories (Summary/Note for this project) → returns a structured `PriorKnowledge` object. This is **O(matched_components)**, not O(all_docs).
- **Token-budget limit (replaces 5-doc limit):** `token_budget` comes from `reborn_monty_vm_settings.prior_knowledge_token_budget` (default 2000, editable in WebUI). The RetrievalSource iterates matched components in score order, accumulating token estimates, until the budget is exhausted. Components that don't fit are skipped entirely (not truncated mid-sentence). **Actions (class 16) are exempt** — full content always included regardless of budget.
- **`__assemble_prior_knowledge__(goal, token_budget, sender_class_code)` host function (Rust):**
  1. Calls `__resolve_intent__` internally.
  2. Fetches matched components via `RetrievalSource`.
  3. Classifies the match: **Solution Override** (single matched solution component with `override_prompt_creation: true`) vs. **Normal Assembly** (multiple components or no override flag).
  4. **Solution Override path:** returns `prior_knowledge_content` (or `content` if PKC is NULL) as the COMPLETE prompt text — no headers, no section wrapper. Sets `override_prompt_creation: true` in the return shape. The orchestrator skips the LLM call (for Actions) or uses the content as the full prompt (for Plans/Extensions).
  5. **Normal Assembly path:** assembles components in `(class_code asc, prompt_uid asc)` order. Each component contributes its `prior_knowledge_content` (or `content` if PKC is NULL) under a per-item header:
     ```
     ## Prior Knowledge

     ### [00:TOOL] {name}
     {prior_knowledge_content or content}

     ### [01:SKILL-RUSTY] {name}
     {prior_knowledge_content or content}
     ```
     The `## Prior Knowledge` section header appears once. The `###` per-item header uses `{class_code}:{CLASS-LABEL}` from a static Rust lookup table (`00`→`TOOL`, `01`→`SKILL-RUSTY`, `02`→`SKILL-MONTY`, `03`→`SKILL-LLM`, `04`→`EXT-RUSTY`, `05`→`EXT-MONTY`, `06`→`EXT-MCP-SERVER`, `07`→`EXT-MCP-CLIENT`, `08`→`EXT-LLM`, `09`→`EXT-MISC`, `10`→`ORCHESTRATOR`, `11`→`RESERVED`, `12`→`SPEC`, `13`→`TOOLSKILL`, `14`→`PLAN`, `15`→`SUMMARY`, `16`→`ACTION`, `17`→`DOCU`, `18`→`LESSON`, `19`→`ISSUE`, `20`→`NOTE`, `21`→`RECIPE`, `50`→`SCAFFOLD`). Sets `override_prompt_creation: false`.
  6. Returns `PriorKnowledgeResult { content: String, override_prompt_creation: bool, matched_component_ids: Vec<Uuid> }`.
- **`prior_knowledge_content` field (solution classes only):** Extensions, Plans, Recipes, and Actions have an optional `prior_knowledge_content TEXT NULL` column. If present, it overrides `content` for prior-knowledge assembly. This lets a solution component provide a curated prompt representation that differs from its editorial content (e.g. an Action's `content` is the full step list for WebUI editing; `prior_knowledge_content` is a compact execution-ready summary for the prompt). If NULL, Rust uses `content`. Non-solution classes (Tools, Skills, Specs, Lessons, Notes, etc.) do NOT have this column — they always use `content`.
- **`override_prompt_creation` flag (solution classes only):** Extensions, Plans, Recipes, and Actions have a `override_prompt_creation BOOLEAN NOT NULL DEFAULT false` column. If `true` AND the intent system matches to this single component, `__assemble_prior_knowledge__` returns the PKC/content as the complete prompt text (Solution Override path). The orchestrator skips the LLM call (Actions) or uses the content as the full prompt (Plans/Extensions). Non-solution classes do NOT have this column — they are always assembled normally (ingredients, not solutions).
- **No per-class Rust formatters.** There is one `__assemble_prior_knowledge__` function + a static class-code→label lookup table. The operator writes `content` (and optionally `prior_knowledge_content`) in the WebUI; that's exactly what goes into the prompt. No formatting templates, no 13 formatters.
- **Python functions deleted:** `format_docs()`, `format_skills()`, `score_skill()`, `select_skills()`, `extract_explicit_skills()`, `append_system_append()`, `signals_tool_intent()`, `signals_execution_intent()` are all deleted from `default.py`. The orchestrator's step-0 block becomes:
  ```python
  if step == 0:
      token_budget = config.get("prior_knowledge_token_budget", 2000)
      result = __assemble_prior_knowledge__(goal, token_budget, "02")
      if result.override_prompt_creation:
          # Solution Override — use result.content as the complete prompt
          working_messages = [{"role": "User", "content": result.content}]
      elif result.content:
          # Normal Assembly — inject as User message at N-1
          insert_as_user_message_at_n_minus_1(working_messages, result.content)
  ```
- **Python functions kept:** `format_output()` (tool output, not prior knowledge), `_reduce_prompt()` (prompt reduction policy), `compact_if_needed()` (compaction policy), `run_loop()` (simplified — no skill selection, no intent detection, no formatting), the LLM rephrase loop for intent no-match handling.

### 3.14 Formatting policy (Q19 resolved — "content is king" + Solution Override)
- **Resolution:** components store their content as the exact text that goes into the prior knowledge. Rust does not "format" — it concatenates `content`/`prior_knowledge_content` fields in `(class_code asc, prompt_uid asc)` order with a trivially-derivable per-item header (`### [{class_code}:{CLASS-LABEL}] {name}`). A matched "solution" component (Extension/Plan/Recipe/Action) with `override_prompt_creation: true` tells Rust exactly what to put in the prior knowledge and overrides normal prompt creation (Solution Override path — no headers, PKC/content IS the complete prompt).
- **No per-class Rust formatters.** One `__assemble_prior_knowledge__` function + a static lookup table. This eliminates the original v5 design's 13 class-specific formatters.
- **DB columns added to solution-class tables** (Extensions, Plans, Recipes, Actions):
  - `prior_knowledge_content TEXT NULL` — if present, overrides `content` for prior-knowledge assembly
  - `override_prompt_creation BOOLEAN NOT NULL DEFAULT false` — if true, Solution Override path
- **Non-solution classes** (Tools, Skills, Specs, ToolSkills, Summaries, Docus, Lessons, Issues, Notes) do NOT have these columns — they always use `content` and are always assembled normally.
- **What is certain (and now fully defined):**
  1. The orchestrator cannot format — `format_docs`/`format_skills`/`append_system_append` are deleted from Python.
  2. The intent system + `__assemble_prior_knowledge__` are Rust host functions.
  3. KV-cache discipline requires deterministic assembly order (`class_code asc, prompt_uid asc`).
  4. Actions (class 16) are exempt from token-budget truncation.
  5. The validator is independent of the orchestrator (§3.5) — the self-improvement mission cannot patch the validator.
  6. The self-improvement mission CAN tune content by patching `content`/`prior_knowledge_content` fields of components through the validation gate. It CANNOT patch the assembly mechanism (Rust).
- **Updated three-layer boundary (Suggestion F revised):**

  | Layer | What lives here | Who edits it |
  |-------|----------------|-------------|
  | **Tunable LOGIC (Python, self-modifiable)** | loop-driver policy: when to compact, when to nudge, LLM rephrase loop for intent no-match, disambiguation UI, obligation mode, retry strategy, "try it with AI" button logic, **"AI before User" flip-switch logic**, **Action execution (default.py follows Action step descriptors, enforces `allowed_tools[]`, interprets step vocabulary)** | self-improvement mission (versioned + rollback, validation-gated) |
  | **Tunable VALUES (DB, explicit columns)** | component content, **`prior_knowledge_content` (solution classes)**, **`override_prompt_creation` (solution classes)**, intent examples, activation criteria, reward metrics, token budgets, Monty VM settings | WebUI (operator) + self-improvement mission (validation-gated) |
  | **Stable INFRASTRUCTURE (Rust)** | LLM calls, tool exec, safety, DB I/O, injection, **prior knowledge assembly mechanism (`__assemble_prior_knowledge__` — content concatenation + Solution Override detection, NO per-class formatters)**, **intent system (`__resolve_intent__`)**, **validator (independent of orchestrator, §3.5)**, **LLM code-audit for Orchestrator/Scaffold (§3.5)**, KV-cache-ordered assembly | Rust code changes (validated like any other code) |

- **The orchestrator CAN:** decide WHEN to retrieve (step 0), set the token budget (from DB config), handle no-match (LLM rephrase loop, disambiguation UI), trigger the "try it with AI" fallback, **honor the "AI before User" flip switch** (suppress the reformulate message + silently trigger keyword fallback when switch is ON), **honor Solution Override** (skip LLM call / use content as full prompt when `override_prompt_creation: true`).
- **The orchestrator CANNOT:** format retrieved content, select which skills to activate, detect tool/execution intent, mutate the System message.

### 3.15 Interceptor architecture (Sempai–Kohai forensic audit + rerouting)

The interceptor sits between prior-knowledge assembly (`__assemble_prior_knowledge__`) and the Kohai LLM call (`__llm_complete__`). It captures a **ForensicPacket** per turn for audit, and optionally reroutes the prompt through a **Sempai** (senior reviewer) provider before forwarding to Kohai.

**Data flow (adapted to the intent-driven architecture):**
```
Step 0 (default.py)
  └── __assemble_prior_knowledge__(goal, token_budget, "02")
        └── __resolve_intent__(goal, "02") → matched_component_ids
        └── fetch matched components via reborn_component_catalog
        └── assemble: content in (class_code asc, prompt_uid asc) order
        └── returns PriorKnowledgeResult { content, override_prompt_creation, matched_component_ids }
              │
              ▼
InterceptorStage (AFTER __assemble_prior_knowledge__, BEFORE final prompt composition)
  └── host.on_prompt_assembled(snapshot)
        saves ForensicPacket (component_refs + volatile_tail, NOT full prompt)
        returns Option<InterceptorResult { packet_id, adjusted_messages }>
              │
              ├── Routing mode: adjusted_messages = None → final prompt composed normally → Kohai receives original
              │
              └── Rerouting mode: adjusted_messages = Some(...) → Sempai-adjusted messages used for final prompt
                    │
                    ▼
              final prompt composition (system + prior knowledge + thread history)
                    │
                    ▼
              __llm_complete__(final_prompt) → Kohai response
```

**Interceptor timing (Q29 resolved):** the `on_prompt_assembled` hook is called **after** `__assemble_prior_knowledge__` returns (the interceptor needs `matched_component_ids` to know which components were matched) but **before** the final prompt version is composed (the Sempai's `adjusted_messages` can shape the volatile tail before the final prompt is assembled). The interceptor must NOT be called after the final prompt is composed — it needs to intercept the components, not the final prompt bytes. This ensures the Sempai can adjust the prior knowledge content before it's inserted into the final prompt, and the KV-cache prefix (stable base) is not mutated by the Sempai's adjustments.

**Actions (class 16) bypass interception:** Actions are Python-only — the orchestrator (default.py) performs them directly without creating a prompt for the Kohai LLM. When the intent system matches an Action with `override_prompt_creation: true`, the prompt creation process is **disrupted**: the orchestrator does not proceed to `__assemble_prior_knowledge__` → interceptor → `__llm_complete__`. Instead, it dispatches the Action's steps directly (§3.11). The interceptor sits between prior-knowledge assembly and the LLM call — but for Action-only turns, this pipeline is never entered. The interceptor **cannot intercept** because there is no prompt to assemble and no LLM call to adjust. No `__llm_complete__` call → no interception → no ForensicPacket. This is not a design choice but a structural consequence: the interceptor's hook point (`on_prompt_assembled`) is never reached for Action-only turns. However, Action schemas ARE included in Part A (below) so the Sempai can audit the agent's Action-selection decisions in non-Action turns.

**DB-less mode:** the interceptor requires a Postgres `PgInterceptorStore` to persist ForensicPackets. In DB-less mode (`RamSource`), the interceptor is **disabled** — `interceptor_store: None`, `sempai_swappable: None`, `interceptor_mode: None`. The `on_prompt_assembled` hook is a no-op. This is acceptable because DB-less mode is not for production (§3.4, SEC-10).

**`SharedInterceptorMode` (Routing/Rerouting):** an `AtomicBool` flag shared between the composition and the loop driver host. `Routing` = forensic logging only (save packet, forward unchanged). `Rerouting` = Sempai review + adjusted messages. The mode flips when `set_active(Sempai)` is called with a non-empty provider ID; it flips back to `Routing` when Sempai is cleared.

**`InterceptorResult` trait change:** `on_prompt_assembled` returns `Option<InterceptorResult>` (was `Option<String>`). `InterceptorResult { packet_id: String, adjusted_messages: Option<Vec<(String, String)>> }`. When `adjusted_messages` is `Some`, the resolved (role, text) pairs replace the prompt that `__llm_complete__` forwards to Kohai — bypassing the normal message resolution path.

**`SempaiReviewOutcome` struct (adapted):**
```rust
pub struct SempaiReviewOutcome {
    pub adjusted_volatile_messages: Vec<(String, String)>,  // volatile tail, as adjusted
    pub bridge_messages: Vec<(String, String)>,             // inserted between base and tail
    pub composition_summary: String,                         // Sempai's analysis
    pub proposed_recipe_updates: Vec<serde_json::Value>,     // routed to Q1 validation queue (§3.5)
    pub proposed_intent_examples: Vec<serde_json::Value>,    // routed to Q1 validation queue (Q30)
    pub settings_adjustments: Vec<serde_json::Value>,        // stored, not auto-applied
}
```
`proposed_recipe_updates` and `proposed_intent_examples` are **routed through the Q1 validation queue** (Phase 3's `ComponentValidator`) — the Sempai cannot directly create components or intent inputs; it proposes them, and they go through the same two-step validation gate as any other component. `proposed_intent_examples` are new `intent_examples` entries that the Sempai suggests for existing components (e.g., "this skill should also match the query 'how do I deploy to staging'"). Once validated, they are added to the component's `intent_examples` and seeded into `reborn_intent_inputs` by the validator. This is consistent with §3.5 (validator independence) and §3.6 (self-improvement writes are validation-gated).

**Three-part Sempai prompt:**

#### Part A — Static base prompt (full DB snapshot, KV-cache prefix)
Assembled by querying **all `Validated` components** via **direct SQL to individual component tables** in `(class_code asc, prompt_uid asc)` order. Filter: `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)`. The full content of every passing row is included — tools, skills, extensions, orchestrator Python, scaffold, actions schemas, former-doctype components. The direct SQL approach queries each component table (reborn_tools, reborn_skills, reborn_extensions_unified, reborn_recipes, reborn_orchestrators, reborn_scaffolds, reborn_actions, reborn_specs, reborn_tool_skills, reborn_plans, reborn_summaries, reborn_docus, reborn_lessons, reborn_issues, reborn_notes) with the same filter, then merges + sorts the results. This is a manual rebuild (not per-turn), so the fan-out cost is acceptable — it runs only when the operator clicks "Reassemble Basic Prompt".

**This is a manual rebuild, not per-turn.** The operator clicks "Reassemble Basic Prompt" in the interceptor config tab. Automatic rebuilds on every component change would destroy the Sempai's KV-cache prefix. The assembled string is stored in `brassclaw_config` under key `interceptor.sempai_base_prompt`. The runtime reads it once at startup; no per-turn DB query.

**Uses direct SQL (NOT `reborn_component_catalog`):** the interceptor's `reassemble_base_prompt()` uses direct SQL to individual component tables, NOT the `reborn_component_catalog` read model. The catalog is the RetrievalEngine's interface (PERF-05, §3.13) for intent-driven retrieval. The interceptor's Part A is a "load all validated components" operation — a different access pattern from intent-driven retrieval. Using direct SQL keeps the interceptor independent of the catalog's refresh timing and schema. The unification with the PlanA-System is at the **storage** layer (ForensicPacket stores `component_refs` — IDs only, not full prompt content — preventing double-saving), NOT at the **retrieval** layer.

**Orchestrator (class 10) is included in Part A** (unlike the Kohai prompt, where it is excluded) because the Sempai needs the orchestrator source to audit agent decision-making.

#### Part B — Persona/role definition (per-deployment, editable in WebUI)
Text block stored in `brassclaw_config` under key `interceptor.sempai_persona`. Default text loaded from `crates/brassclaw_engine/prompts/sempai_audit.md` via `include_str!()`. Editable from the interceptor config tab. Editing does not invalidate Part A or require KV-cache re-warm.

#### Part C — Per-turn volatile tail + component manifest
The only part that varies turn-to-turn. Assembled from the resolved Kohai messages at intercept time:

**Stripped:** all static components (those in Part A) are removed from the per-turn message. Their content is already in Part A; sending it again would duplicate tokens.

**Replaced with a component manifest:** a structured list of every component that was in the Kohai prompt's prior knowledge. Each line: `{class_code}:{prompt_uid}  {type}  "{name}"`. **This manifest is derived from `PriorKnowledgeResult.matched_component_ids`** — the same list the intent system returned. No separate tracking needed. The manifest is also stored in the `ForensicPacket.component_refs` JSONB column.

**What remains after stripping:** the volatile tail — thread history messages and inline nudges (the User message at N-1 containing prior knowledge is stripped; only the actual conversation history remains).

**Full layout sent to Sempai:**
```
[SYSTEM — Part A, from brassclaw_config key interceptor.sempai_base_prompt]
  All Validated DB components ordered by (class_code asc, prompt_uid asc):
    Tools (00) · Skills (01–03) · Extensions (04–09) · Orchestrator (10) · Scaffolds (50)
    · Phase 5 tables: (12 Spec · 13 ToolSkill · 14 Plan · 15 Summary · 16 Actions
      · 17 Docu · 18 Lesson · 19 Issue · 20 Note · 21 Recipe)
  SempaiReviewOutcome JSON schema

[SYSTEM — Part B, from brassclaw_config key interceptor.sempai_persona]
  Sempai role definition and task description

[USER — Part C, per-turn]
  --- Component manifest (from matched_component_ids) ---
  00:0001  tool       "bash_exec"
  01:0003  skill      "deploy-workflow"
  ...
  ---
  --- Kohai volatile tail (thread history + inline nudges) ---
  user: {content}
  assistant: {content}
  ...
  ---
  iteration: N  message_count: N  token_budget_remaining: N
  bundle_fingerprint: <sha256 of stable-base content>

[USER — request]
  Respond with a SempaiReviewOutcome JSON object.
```

**Recomposition after Sempai response:** the host takes the stable-base messages (Part A content) + `outcome.bridge_messages` + `outcome.adjusted_volatile_messages` → produces the complete adjusted Kohai prompt. `ModelStage` forwards it to Kohai, which sees its normal stable prefix (KV-cache hit) followed by the Sempai-adjusted volatile tail.

**KV-cache pre-warm (manual button):** the operator clicks "Pre-warm Sempai KV-cache" in the interceptor config tab. The handler reads `interceptor.sempai_base_prompt` from config, sends it as a single system message to the Sempai provider, discards the response. This pushes the Part A prefix into the Sempai's KV-cache so subsequent turns are fast. Rate-limited to 1 request per minute per caller.

**`set_active(Sempai)` live-swap:** when the operator configures a Sempai provider via the WebUI, `set_active(Sempai)` writes the config to DB AND immediately swaps the running `sempai_swappable` provider AND flips `SharedInterceptorMode` to `Rerouting`. Clearing Sempai (`set_active(Sempai, "")`) swaps back to `PlaceholderLlmProvider` and flips mode to `Routing`.

**Wiring gaps addressed (from interceptor2.md):**
1. `brassclaw_forensic_packets` table — already exists as V026; ALTER migration V044 adds `component_refs` + `volatile_tail` columns (our plan uses V027–V043 for component tables).
2. `PgInterceptorStore` — wired in composition when `postgres` feature is on.
3. `sempai_swappable` — allocated in `wrap_swappable_gateway` alongside Kohai.
4. `SharedInterceptorMode` — created in composition, threaded through `RebornLoopDriverHostFactory`.
5. `on_prompt_assembled` return type — changed to `Option<InterceptorResult>` with `adjusted_messages`.

**WebUI:** a 10th tab in Settings (§5 Phase 6) — "Interceptor Config" — showing Sempai status (connected/disconnected, mode routing/rerouting, **warning if same model as Kohai — Q20**), base prompt info (last assembled timestamp + char count + "Reassemble Basic Prompt" button + **`components_since_rebuild` badge — Q23**), KV-cache info (last pre-warm timestamp + "Pre-warm Sempai KV-cache" button), a persona editor textarea (**immediate-write, no validation gate — Q27**), and a **"Recent Sempai Suggestions" list** (Q22 — last 10 packets with `settings_adjustments` non-null; each suggestion has an "Apply" button that writes the adjustment to `brassclaw_config` immediate-write + a "Dismiss" button that marks it as reviewed). **Hidden in DB-less mode** (interceptor disabled). **Feature flag `interceptor` (default off — Q31)** gates the tab visibility.

## 4. Data model (DB)

All tables: Postgres, scope tuple `(tenant_id, user_id, agent_id, project_id)`, unique `(scope, name)`. **Explicit columns** (not jsonb blobs) for everything tunable.

### `reborn_skills`
Content: `name`, `description`, `body`, `compatibility` (class), `license`, `allowed_tools`, `version`, `class_code`, `prompt_uid`.
Activation: `keywords[]`, `exclude_keywords[]`, `patterns[]`, `tags[]` (legacy activation tags — broad category matching for backward compat; ≤10, min len 3), `max_context_tokens`, `setup_marker`, `required_binaries[]`, `required_env[]`, `required_config[]`.
**Intent examples: `intent_examples jsonb`** (§3.12 — `[{input, class}]` array; primary activation mechanism via the intent system; exploded into `reborn_intent_inputs` on validation).
**Consumer gating: `consumer_tags[]`** (new — the §3.9 tag codes; CHECK `^[0-9]{2}(:[a-z0-9-]+)?$`; `05:validator` greys out the rest).
Reward (immediate-write): `tier`, `usage_count`, `success_count`, `failure_count`, `wilson_lower`, `confidence` (generated).
Provenance (immediate-write): `source`.
Decision (gated): `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code` (derived from `validation_status` + `review_attempts` + `rejected_at` age per §3.5.1; stored for query convenience).
Lineage/audit: `similarity_parent_id`, `replaces_id`, `parent_version`, `content_hash`, `last_audit_at`, `audit_failure_count`, `parent_mission_id` (for D, when unblocked).
**No `trust` column.**

### `reborn_tools`
`name`, `description`, `param_schema jsonb`, `param_template jsonb`, `effect_type`, `preconditions`, `error_handling`, `class_code` (00), `prompt_uid`, **`intent_examples jsonb`** (§3.12), **`consumer_tags[]`** (§3.9; tools always carry `00:rusty`), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, `content_hash`, `source`, lineage, timestamps.

### `reborn_extensions_unified`
`name`, `description`, `class` (enum), `class_code`, `prompt_uid`, **`intent_examples jsonb`** (§3.12), **`consumer_tags[]`** (§3.9), `payload jsonb` (class-specific body: manifest TOML / recipe step list / plan doc / orchestrator Python), **`prior_knowledge_content TEXT NULL`** (§3.13/§3.14 — solution-class override for prior-knowledge assembly), **`override_prompt_creation BOOLEAN NOT NULL DEFAULT false`** (§3.13/§3.14 — if true, Solution Override path), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, reward columns (`tier`, `usage_count`, `success_count`, `failure_count`, `wilson_lower`), `source`, lineage, `content_hash`, timestamps.

### `reborn_actions` (new, §3.11)
`name`, `description`, `class_code` (16), `prompt_uid`, **`intent_examples jsonb`** (§3.12), **`consumer_tags[]`** (§3.9; default `{01:monty,02:orchestrator}`), `preconditions jsonb`, `steps jsonb`, `error_handling jsonb`, `timeout_secs`, `allowed_tools[]`, `param_schema jsonb`, `param_template jsonb`, **`prior_knowledge_content TEXT NULL`** (§3.13/§3.14 — compact execution-ready summary for the prompt; if NULL, `steps`+`preconditions`+`error_handling` are assembled as `content`), **`override_prompt_creation BOOLEAN NOT NULL DEFAULT true`** (§3.13/§3.14 — Actions default to Solution Override since they skip the LLM call), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, `content_hash`, `source`, lineage, timestamps.

### `reborn_intent_inputs` (new, §3.12)
`id` uuid PK, `scope` tuple, `input_text` text, `input_class` int (1=word, 2=partial, 3=sentence, 4=keyword-fallback), `component_id` uuid, `component_class_code` int, `score` int (default 1, **hard cap 100 — PERF-03/SEC-05 §6.1**), `source` text (`seeded`/`learned_user`/`learned_llm`/`learned_fallback`), `needs_review bool` (default false; true for `learned_llm` — SEC-05), `created_at`, `updated_at`. Composite unique: `(scope, input_text, input_class, component_id)`. Indexes: **B-tree on `(scope, input_text, input_class)`** (exact-match path — PERF-01), `(scope, input_text)`, `(scope, component_id)`, GIN trigram on `input_text` (fuzzy partial — future). Score increments use atomic `UPDATE ... SET score = score + 1 RETURNING score` (PERF-03).

### `reborn_component_catalog` (new, §3.13/§6.1 — PERF-05 read model)
**Read-only** materialized view (or `UNION ALL` view) across all component tables. The RetrievalEngine queries this catalog by `component_id` + `component_class_code` in a **single query** instead of fan-out to 15+ class-specific tables. Columns: `id` uuid, `scope` tuple, `class_code` int, `name` text, `content` text (or `prior_knowledge_content` if non-null for solution classes), `override_prompt_creation` bool, `validation_status` text, `consumer_tags[]` text, `prompt_uid` int. Filtered by `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)` at query time (SEC-01). Class-specific tables remain the write path; the catalog is read-only (refreshed via trigger or `REFRESH MATERIALIZED VIEW`). In DB-less mode, `RamSource` builds an in-memory catalog from the fallback file.

### Former-doctype component tables (new, §3.7 class codes 12-20)
Each follows the same shape: `name`, `title`, `content`/`body`, `class_code`, `prompt_uid`, **`intent_examples jsonb`** (§3.12), **`consumer_tags[]`** (§3.9), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, `content_hash`, `source`, `source_thread_id` (optional FK to the thread that produced it), lineage, timestamps.
- `reborn_specs` (class 12) — `title`, `content`.
- `reborn_tool_skills` (class 13) — `name`, `description`, `tool_name`, `param_template jsonb`, `param_schema jsonb`, `content`.
- `reborn_plans` (class 14) — `title`, `steps jsonb`, `status`, `content`, **`prior_knowledge_content TEXT NULL`** (§3.13/§3.14 — solution-class), **`override_prompt_creation BOOLEAN NOT NULL DEFAULT false`** (§3.13/§3.14 — solution-class).
- `reborn_summaries` (class 15) — `title`, `content`, `source_thread_id`.
- `reborn_docus` (class 17) — `title`, `content`, `doc_type` (sub-category enum: `reference`/`tutorial`/`guide`/`general`).
- `reborn_lessons` (class 18) — `title`, `content`, `source_thread_id`.
- `reborn_issues` (class 19) — `title`, `content`, `status` (open/resolved), `source_thread_id`.
- `reborn_notes` (class 20) — `title`, `content`, `source_thread_id`. Durable notes only; transient notes stay in `reborn_memory_*`.

### `reborn_action_reliability`
`(scope, action_name, ema_success, ema_latency, sample_count, last_updated_at)` — per-action EMA, immediate-write, operator-resettable.

### `reborn_orchestrators`
`name`, `body` (the Python loop-driver code), `version`, `parent_version`, `failure_count` (for 3-strike rollback), `active` (bool — which version is live), `class_code` (10), `prompt_uid`, **`consumer_tags[]`** (§3.9; default `{02:orchestrator}`; `05:validator` while in queue), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, `content_hash`, `source`, timestamps.
Compiled-in default: `DEFAULT_ORCHESTRATOR` (`include_str!`'d `default.py`) — served by `RamSource` in DB-less mode. (Orchestrators do not carry `intent_examples` — they are not routed-to by the intent system; they are the active loop driver pointed to by `reborn_monty_vm_settings.active_orchestrator_id`.)

### `reborn_scaffolds`
`name`, `section_type` (enum: `preamble`/`postamble`/`platform`/`overlay`), `body`, `version`, `parent_version`, `class_code` (50), `prompt_uid`, **`consumer_tags[]`** (§3.9; default `{03:llm,04:scaffold}`; `05:validator` while in queue), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, `content_hash`, `source`, timestamps.
Compiled-in defaults: `CODEACT_PREAMBLE` / `CODEACT_POSTAMBLE` (`include_str!`'d prompt files) — served by `RamSource` in DB-less mode. (Scaffolds do not carry `intent_examples` — they are always injected as the prompt base, not routed-to by the intent system.)

### `reborn_monty_vm_settings` (new, §3.10)
`(scope, max_duration_secs, max_allocations, max_memory_bytes, failure_rollback_threshold, active_orchestrator_id, prior_knowledge_token_budget, q4_retention_days, forensic_packet_retention_days, updated_at)`. Single row per scope (upsert — spec §7 Q8 resolved). `active_orchestrator_id` FK → `reborn_orchestrators.id` (must be `Validated`). Immediate-write for the knobs; the `active_orchestrator_id` switch is gated by the target row's `validation_status == Validated`. `q4_retention_days` (default 30, spec §7 Q7 resolved) controls the Q4 rejection queue retention window before terminal wipe. `forensic_packet_retention_days` (default 90, spec §7 Q21 resolved) controls the `brassclaw_forensic_packets` retention window — a scheduled daily cleanup task deletes packets older than this; set to 0 to disable pruning. Compiled-in defaults served by `RamSource` in DB-less mode.

### `reborn_recipes` (new, §3.7 class 21 — spec §7 Q15 resolved)
`name`, `description`, `class_code` (21), `prompt_uid`, **`intent_examples jsonb`** (§3.12), **`consumer_tags[]`** (§3.9; default `{02:orchestrator,03:llm}`), `trigger jsonb` (trigger condition: type + payload), `steps jsonb` (ordered array of `{skill, params}` references to validated ToolSkills/Skills), `status`, **`prior_knowledge_content TEXT NULL`** (§3.13/§3.14 — solution-class override for prior-knowledge assembly), **`override_prompt_creation BOOLEAN NOT NULL DEFAULT false`** (§3.13/§3.14 — if true, Solution Override path), `validation_status`, `validation_errors[]`, `review_feedback`, `review_attempts`, `rejected_at`, `queue_code`, reward columns (`tier`, `usage_count`, `success_count`, `failure_count`, `wilson_lower`), `source`, lineage, `content_hash`, timestamps. The `RecipeLookup` trait boundary is preserved — `brassclaw_agent_loop` stays free of `brassclaw_engine`.

### `reborn_validation_config` (new, §3.5.2 — spec §7 Q14 resolved)
`(scope, class_code, name_min_len, name_max_len, name_pattern, description_min_len, description_max_len, token_budget, token_budget_hard_error, require_tool_name, require_param_schema, require_activation_criteria, updated_at)`. One row per `(scope, class_code)`. Immediate-write (knobs, not content/code). Defaults per class as specified in §3.5.2. The validator (`ComponentValidator`) reads the config row for the component's `class_code` and applies the corresponding checks.

### `reborn_user_preferences` (new, §3.12 — spec §7 Q18 resolved)
`(user_id, preference_key, preference_value, updated_at)`. Simple key-value store for per-user UX preferences. Composite unique: `(user_id, preference_key)`. Current keys: `ai_before_user` (boolean, default `false`). Not exposed in the Settings UI — these are runtime preferences, not operator-managed configurations. Used by the chat-surface "AI before User" flip switch (§3.12 rule f-ai).

### `reborn_memory_*` (PlanA-Memory, retained — volatile thread-scoped only)
**All former `DocType` variants are now first-class component classes** (§3.7 codes 12-20) with their own tables. `reborn_memory_*` retains only **volatile thread-scoped memories**: in-progress notes, transient state, and thread-local context that doesn't warrant a validated component row. The `DocType` enum is **fully retired**. Chunk columns/tables dropped after a retention window.

### `brassclaw_forensic_packets` (new, §3.15 — interceptor)
Captures one `ForensicPacket` per agent-loop turn: the component references (NOT full prompt — prevents double-saving), optional Sempai review outcome, and the Kohai response with actual token usage.

```sql
CREATE TABLE IF NOT EXISTS brassclaw_forensic_packets (
    id                                TEXT         NOT NULL,
    tenant_id                         TEXT         NOT NULL,
    run_id                            TEXT         NOT NULL,
    iteration                         INTEGER      NOT NULL,
    status                            TEXT         NOT NULL
                                                     CHECK (status IN
                                                       ('awaiting_kohai',
                                                        'complete',
                                                        'sempai_reviewed')),
    captured_at                       TIMESTAMPTZ  NOT NULL,
    completed_at                      TIMESTAMPTZ,

    -- Component references (NOT full prompt — prevents double-saving).
    -- Array of {class_code, prompt_uid, component_id} derived from
    -- PriorKnowledgeResult.matched_component_ids.
    component_refs                    JSONB        NOT NULL,

    -- Volatile tail (thread history only — the per-turn part that changes).
    -- Stored as text, not JSONB, since it is conversation history.
    volatile_tail                     TEXT,

    -- Kohai response text and token usage (NULL until ModelStage completes).
    kohai_response                    TEXT,
    kohai_input_tokens                INTEGER,
    kohai_output_tokens               INTEGER,
    kohai_cache_read_input_tokens     INTEGER,
    kohai_cache_creation_input_tokens INTEGER,

    -- Sempai review outcome (NULL in routing state).
    sempai_review                     JSONB,

    -- Retroactive join to chat-memory records (written post-turn by
    -- PgChatMemoryRecordStore via link_chat_record()).
    chat_record_id                    TEXT,

    updated_at                        TIMESTAMPTZ  NOT NULL DEFAULT now(),

    PRIMARY KEY (id),
    UNIQUE (tenant_id, run_id, iteration)
);
```

**Why `component_refs` instead of `prompt JSONB`:** the prompt content is already in the DB as component rows (tools, skills, extensions, etc.). Saving the full prompt as JSONB would double-save the same content. Instead, we save only the **references** (which components were in the prompt) + the **volatile tail** (thread history, which is NOT in the component tables). The full prompt can be reconstructed from `component_refs` + `volatile_tail` if needed for forensic analysis. This is the unification the user requested: "prompt creation information should mostly be recorded by saving a list of unique ids + classes instead of complete files."

Indexes: `(tenant_id, captured_at DESC)` for recent-packet queries, `(tenant_id, run_id, iteration)` for per-run lookup.

`brassclaw_config` keys (no new table — uses existing config store):
- `interceptor.sempai_base_prompt` — assembled Part A string (manual rebuild)
- `interceptor.sempai_base_prompt_assembled_at` — ISO 8601 timestamp
- `interceptor.sempai_persona` — editable Part B text
- `interceptor.sempai_prewarm_last_at` — ISO 8601 timestamp

## 5. Migration strategy (phased)

- **Phase 0** — spec/plan (this document + `plan.md`).
- **Phase 1** — DB Skills (explicit columns, class codes, prompt_uid, `consumer_tags[]`, no trust, **`intent_examples jsonb`**) + **intent system** (`reborn_intent_inputs` table, `__resolve_intent__` host function, `intent_examples` explosion) + **Actions class** (`reborn_actions` table, **default.py is the executor — no separate Rust `__execute_action_procedure__` host function**, step descriptor schema). The intent system is a prerequisite for skill selection replacement and retrieval replacement.
- **Phase 1.5 (UNBLOCKED)** — prompt-path dedup + self-modification boundary + **formatting dedup** (delete Python `format_docs`/`format_skills`/`score_skill`/`select_skills`/`signals_*`, `__assemble_prior_knowledge__` host function — "content is king" + Solution Override, no per-class Rust formatters). Validator independence confirmed (§3.5); Q19 resolved.
- **Phase 2** — DB Tools (Rusty-only, `consumer_tags[]` = `{00:rusty}`, **`intent_examples jsonb`**).
- **Phase 3** — Remove `SkillTrust` + source-driven security + skill-trust attenuation; validator expanded to validate code; `Validated == trusted`; **4-queue validation lifecycle (Q1 auto / Q2 manual-WebUI / Q3 revision / Q4 rejection+wipe) with validator-tag greyed-out mechanism**.
- **Phase 4** — Unified Extensions (dissect DocPlans, merge Recipes; `consumer_tags[]`, **`intent_examples jsonb`**).
- **Phase 5** — PlanA-memory universal connector + de-chunk + `RamSource` + baked-in fallback (incl. default orchestrator + scaffold); **`fetch_for_consumer(consumer_tag)` tag-gated retrieval**; **intent-driven retrieval** (`fetch_for_turn` replacing "load all docs"); **Rust-owned formatting** (`__assemble_prior_knowledge__` — "content is king" + Solution Override, no per-class formatters); **token-budget prior-knowledge limit** (`prior_knowledge_token_budget`); **"try it with AI" fallback** (class-4 keyword intent matching); **former-doctype component tables** (classes 12-20: `reborn_specs`/`reborn_tool_skills`/`reborn_plans`/`reborn_summaries`/`reborn_docus`/`reborn_lessons`/`reborn_issues`/`reborn_notes`); **general component importer** (dissect all MemoryDoc types into component rows).
- **Phase 5.5** — **Interceptor activation** (§3.15): `brassclaw_forensic_packets` table ALTER (V044 — table already exists as V026; ALTER adds `component_refs` + `volatile_tail` columns; `prompt JSONB` kept for backward compat); `InterceptorResult` trait change (`on_prompt_assembled` returns `Option<InterceptorResult>` with `adjusted_messages`); wire `PgInterceptorStore` + allocate `sempai_swappable` + create `SharedInterceptorMode`; `set_active(Sempai)` live-swap + mode flip; Sempai gateway + rerouting branch + 3-part prompt (Part A via **direct SQL to individual component tables**, Part B persona, Part C volatile tail + component manifest from `matched_component_ids`); KV-cache pre-warm; interceptor config service; `proposed_recipe_updates` routed to Q1 validation queue; **Actions bypass interception** (no LLM call); **DB-less mode disables interceptor**. Depends on Phase 5 (`__assemble_prior_knowledge__` + `matched_component_ids`).
- **Phase 6** — Settings UI (**10-tab** editor: Skills & Actions / Tools / Extensions / Orchestrator / Scaffold / Knowledge Base / Monty VM / Validation Queue / Reliability / **Interceptor Config**); review existing validation-queue WebUI tab and integrate; **intent-examples editor** in every component tab; **Action step editor** in Skills & Actions tab; **Interceptor Config tab** (Sempai status, Reassemble button, Pre-warm button, persona editor).
- **Phase 7** — Cleanup.

Each phase shippable behind a feature flag; old path stays until verification passes.

## 6. Security & precedence invariants (updated)

- Do not weaken bearer-token auth, webhook auth, CORS/origin, body limits, rate limits, allowlists, secret handling.
- **Trust is now binary: `validation_status == Validated` AND no `05:validator` consumer tag ↔ usable.** No source/directory-based trust. Removing `SkillTrust` must not create a path where an unvalidated component reaches the prompt — `recipe_library`-style `Validated`-only filtering **plus** consumer-tag gating is the invariant.
- **Consumer-tag gating is a token-saving mechanism, not a security boundary.** A component not tagged for a consumer is simply not delivered to that consumer. Security is still the validation gate (§3.5). Do not let tag toggling bypass validation — toggling tags on a validated component is immediate-write, but toggling tags on a validator-tagged (in-queue) component is a gated write because tag membership controls delivery once validated.
- **Subagent capability attenuation is preserved** (child-run scoping); it is unrelated to skill trust and must not be removed with the trust cleanup.
- Skill selection stays deterministic (no ambient time/network/fs effects in scoring). **The intent system is deterministic** — same query + same DB state → same match. Score increments are immediate-write but do not introduce nondeterminism (the highest score wins deterministically).
- Validation is fail-closed; the WebUI manual gate (Q2, Step 2) is mandatory for new/updated components. The validator tag (§3.5.1) is the lifecycle marker that prevents an in-queue component from being delivered.
- **Q4 rejection+wipe must not delete LLM output** (thread messages/steps/events). The wipe is scoped to the component row + its creation-process provenance (similarity parent, source thread id, review feedback). Thread data is never wiped.
- Scope isolation on all new tables: full `(tenant_id, user_id, agent_id, project_id)` tuple; uniqueness `(scope, name)`.
- `escape_skill_content` / `escape_xml_attr` still run on any skill body interpolated into prompts.
- Kernel boundaries: product/loop code must not mint `TrustedInboundTurnRequest` or bypass the loop runner.
- Never delete LLM output (thread messages/steps/events).
- DB-less mode (`RamSource`) is not for production; it must not bypass safety, policy, or scope rules — only the persistence backend swaps. Monty VM settings fall back to compiled-in defaults.
- **Monty host-function extensions (`__llm_complete__` etc.) are kernel-owned and read-only in the WebUI** — operators may not edit the Python↔Rust bridge.
- **Formatting is Rust-owned (§3.14).** The self-improvement mission cannot patch the class-specific formatters or `__assemble_prior_knowledge__`. This protects the KV-cache invariant: the formatting shape is byte-identical across turns.
- **Actions execute via default.py (§3.11).** An Action's `steps` are declarative descriptors that default.py interprets and follows. Tool calls within an Action go through the normal Rust `EffectExecutor` bridge. The `allowed_tools[]` column is a capability subset — default.py enforces this before each tool dispatch: a tool not in `allowed_tools[]` is rejected. Actions are exempt from `prior_knowledge_token_budget` truncation (full content included). The step vocabulary is Python tunable logic — new step types are added by patching default.py through the validation gate, not by Rust changes. Actions must be `Validated` before they can execute.
- **Intent system learned inputs are scoped.** A `reborn_intent_inputs` row learned in one scope tuple does not leak to another. The `source` field (`learned_user`/`learned_llm`/`learned_fallback`) tracks provenance for audit. Learned inputs are immediate-write (no validation gate) — they are routing hints, not content/code.
- **"Try it with AI" fallback is per-query.** The RetrievalEngine fallback reactivates only for the single query that triggered it; it does not persist as a mode. The fallback uses the existing `extract_keywords` + the intent system's class-4 path; it does not bypass the validation gate or consumer-tag gating.

### 6.1 Review findings (v5.5 comprehensive review — security + performance mitigations)

The v5.5 comprehensive review (4 parallel subagents: codebase verification, security, correctness, performance) identified the following issues. All are mitigated in this section; the mitigations are normative (binding on the implementation).

**SEC-08 CRITICAL — `spawn_subprocess` sandbox (§3.11):** The `spawn_subprocess` step type does NOT call `subprocess.Popen` directly. It dispatches through the **host runtime's script lane** (`services/script_runtime` per AGENTS.md) which enforces the same **capability lease + approval gate + sandbox boundary** as any other tool call. The script lane checks the Action's `allowed_tools[]` (which must include `spawn_subprocess` explicitly), enforces `timeout_secs`, captures output, and runs in the configured sandbox. An Action without `spawn_subprocess` in `allowed_tools[]` cannot spawn subprocesses. The `command`/`args`/`cwd` are validated against the script lane's allowlist (no arbitrary path traversal). This is a **Rust-side enforcement** — default.py calls the `__spawn_subprocess__` host function which dispatches to the script lane; default.py cannot bypass the sandbox.

**SEC-01 HIGH — by-ID retrieval validation gate (§3.13):** The RetrievalEngine's by-ID fetch path (used when the intent system returns component IDs) **must filter `validation_status == Validated` AND `05:validator NOT IN consumer_tags`** — the same gate as any other retrieval. An intent-resolved ID that points to an in-queue or rejected component is silently dropped (the orchestrator receives an empty result and falls back to the no-match path). This prevents a learned intent mapping from bypassing the validation gate by directly fetching an unvalidated component.

**SEC-02 HIGH — `reborn_validation_config` weakening (§3.5.2):** Validation config changes are immediate-write BUT **cannot loosen below compiled-in safety floors**. The `ComponentValidator` enforces a compiled-in minimum for each class regardless of the config row: `token_budget` cannot be set above the compiled-in hard cap (e.g. Skills ≤5000, Orchestrator ≤50000), `require_tool_name`/`require_param_schema` cannot be set to `false` for Tools (class 00), `require_activation_criteria` cannot be `false` for Skills (01-03). The WebUI Validation Config panel shows the compiled-in floor as a disabled minimum and prevents saving values below it. A config row that violates the floor is rejected at save time with a clear error.

**SEC-05 HIGH — intent poisoning + score manipulation (§3.12):** Score increments are **bounded and rate-limited**: (a) maximum score per `(scope, input_text, input_class, component_id)` row = **100** (hard cap — further increments are no-ops); (b) score increments are rate-limited to **50 per scope per hour** (burst allowance for active learning sessions, but prevents automated poisoning); (c) learned inputs from `learned_llm` source are flagged `needs_review: true` and are **purged if the source component is rejected/wiped** (Q4 wipe cascades to learned inputs that reference the wiped component); (d) the `source` field tracks provenance — an operator can filter/purge all `learned_llm` inputs from the WebUI Validation tab if a kohai provider is compromised. Seeded inputs (from validated component `intent_examples`) are the trust anchor; learned inputs are routing hints with bounded influence.

**SEC-07 HIGH — Actions `allowed_tools[]` defense in depth (§3.11):** `allowed_tools[]` is enforced at **BOTH** layers: (a) default.py checks each `tool_call`/`call_skill`/`spawn_subprocess` step against `allowed_tools[]` before dispatch (Python-side, tunable); (b) the Rust `EffectExecutor` bridge **also checks** the calling Action's `allowed_tools[]` before executing any tool (Rust-side, stable). A tool not in `allowed_tools[]` is rejected at both layers. This is defense in depth — even if a compromised orchestrator skips the Python check, the Rust bridge blocks the call. The Rust bridge receives the Action's `allowed_tools[]` as part of the turn context (not from the orchestrator's self-reported list).

**SEC-04 MEDIUM — rollback CAS protection (§3.10):** Orchestrator rollback uses **compare-and-swap (CAS)** on `reborn_orchestrators.active` — the rollback transaction checks `WHERE id = ? AND failure_count = ?` (the failure count at detection time) to prevent a concurrent turn from racing the rollback. Concurrent turns during rollback are admitted but immediately fail with `OrchestratorRollbackInProgress` (no partial execution).

**SEC-09 MEDIUM — Action recursion bounding (§3.11):** `call_action` chaining is bounded: (a) **max depth = 5** (nested `call_action` calls beyond depth 5 are rejected with `ActionRecursionLimitExceeded`); (b) **cycle detection** — the call stack is tracked; a `call_action` to an Action already in the stack is rejected with `ActionCycleDetected`; (c) **total step budget = 1000** (all steps across all nesting levels combined — a `loop` that spawns 500 `call_action` steps hits the budget and is rejected). These are enforced in default.py (Python tunable logic) with a Rust-side hard cap on total wall-clock time (`timeout_secs`).

**SEC-10 MEDIUM — DB-less production prohibition (§3.4):** The `RamSource` backend checks `BRASSCLAW_RUNTIME_PROFILE` at startup — if the profile is not `local_dev`/`local_safe`/`local_yolo`, `RamSource` refuses to start with a clear error ("DB-less mode is not supported for profile '{profile}' — set BRASSCLAW_PG_URL or switch to a local profile"). This prevents an operator from accidentally running a hosted production deployment without a DB. The fallback-content file integrity is verified at startup (content hash check — tampering is detected and refused).

**SEC-11 MEDIUM — Q4 wipe transactional guarantee (§3.5.1):** The Q4 wipe is a **single transaction** that deletes the component row + all `reborn_intent_inputs` rows referencing that component + all `reborn_action_reliability` rows for that component + all creation-process provenance (similarity parent, review feedback). The wipe is wrapped in `BEGIN ... COMMIT` — a race between wipe and a concurrent turn's intent lookup is impossible because the intent lookup reads committed rows only (READ COMMITTED isolation). A concurrent turn that already fetched the component before the wipe completes with the stale data (acceptable — the component was valid at fetch time).

**SEC-12 LOW — consumer-tag editing exposure:** Already mitigated by §6: toggling tags on a validator-tagged (in-queue) component is a gated write. A validated component's tags are immediate-write but the component is already trusted — tag toggling only affects delivery routing, not security.

**PERF-05 HIGH — cross-table fan-out (§3.13):** A **`reborn_component_catalog` read model** (materialized view or UNION ALL view across all component tables) is added. The RetrievalEngine queries the catalog by `component_id` + `component_class_code` in a **single query** instead of fan-out to 15+ class-specific tables. The catalog exposes `id`, `class_code`, `name`, `content`/`prior_knowledge_content`, `override_prompt_creation`, `validation_status`, `consumer_tags[]`, `prompt_uid` — enough for prior-knowledge assembly + validation gate filtering. The class-specific tables remain the write path; the catalog is read-only (refreshed via trigger or materialized view refresh). In DB-less mode, `RamSource` builds an in-memory catalog from the fallback file.

**PERF-16 HIGH — Monty restart drain (§3.10):** Monty restart uses **drain + admission control**: (a) on restart request, the kernel sets an `admission_paused` flag — new turns are queued (not rejected) with a `MontyRestartPending` status; (b) in-flight turns are allowed to complete or timeout (max `max_duration_secs`); (c) once all in-flight turns complete, the kernel stops the VM, applies the new orchestrator/scaffold, and restarts; (d) queued turns are admitted in order. This prevents losing in-flight work and provides a clean cutover. The WebUI Monty VM tab shows the restart status (draining / restarting / ready).

**PERF-18 HIGH — Action size limits (§3.11):** Actions are exempt from `prior_knowledge_token_budget` truncation BUT have **hard limits**: (a) **max content size = 256KB** (hard error at validation — an Action larger than 256KB is rejected); (b) **max step count = 500** (hard error — an Action with >500 steps is rejected); (c) **max `allowed_tools[]` = 50** (hard error — an Action referencing >50 tools is rejected). These prevent a single Action from consuming the entire context window or running unboundedly. The limits are compiled-in constants (not configurable in `reborn_validation_config` — they are safety floors, not tunable thresholds).

**PERF-03 HIGH — score increment contention (§3.12):** Score increments use a **single UPDATE ... SET score = score + 1 WHERE ... RETURNING score** statement (atomic read-modify-write, no explicit SELECT-then-UPDATE). The composite unique constraint + the 100-score hard cap + the 50/hour rate limit bound the hot path. For high-concurrency scopes, the rate limiter is a token bucket (not a global lock).

**PERF-02 MEDIUM — 3 sequential queries per intent resolution (§3.12):** The match-order logic (query class 3 → search class 3, then 2, then 1) is collapsed into a **single query with `CASE WHEN` ordering**: `SELECT ... WHERE input_text = ? AND input_class IN (3,2,1) AND ... ORDER BY CASE input_class WHEN 3 THEN 0 WHEN 2 THEN 1 WHEN 1 THEN 2 END, score DESC LIMIT 10`. One query, one round-trip. The query class determines the `CASE` ordering. Class-4 keyword fallback runs one query per keyword (acceptable — keywords are typically 3-10 tokens, and the fallback is rare).

**PERF-01 MEDIUM — GIN trigram vs B-tree for exact match (§3.12):** The exact-match path uses the **B-tree index on `(scope, input_text, input_class)`** (the composite unique constraint). The GIN trigram index is used **only** for fuzzy partial matching (a future feature — not on the v1 critical path). The migration creates both indexes but the exact-match path is B-tree-driven.

**FMT-02 MAJOR — token-budget exemption algorithm (§3.13):** The `__assemble_prior_knowledge__` token-budget algorithm is: (1) fetch all matched component IDs from the intent system; (2) query the `reborn_component_catalog` for those IDs (single query — PERF-05); (3) sort by `(class_code asc, prompt_uid asc)`; (4) iterate in order, accumulating token count; (5) **Actions (class 16) are always included first** (exempt from budget — full content); (6) **Solution-class components with `override_prompt_creation: true`** are returned as the complete prior knowledge (Solution Override path — no further assembly); (7) for Normal Assembly, add each component's content + header to the accumulated text until the `prior_knowledge_token_budget` is exhausted; (8) components beyond the budget are dropped (not truncated — a component is either fully included or excluded). The algorithm is deterministic: same match set + same budget → same output.

**INT-01 MAJOR — disambiguation timeout/abandonment (§3.12):** The disambiguation flow has a **30-second timeout** — if the user doesn't click a candidate within 30 seconds, the orchestrator abandons the disambiguation and falls back to the highest-scored candidate (auto-select the top). If all candidates have equal scores, the first one (by `prompt_uid` asc) is selected. The timeout is a Rust-side timer (not Python) — the orchestrator is not blocked waiting for the user. The user can still click after the timeout (the click increments the score for future turns but doesn't change the current turn's result).

**SEC-06 MEDIUM — `reborn_user_preferences` scope isolation (§3.12):** The `reborn_user_preferences` table uses `(user_id, preference_key)` as the composite unique key, NOT the full scope tuple. This is intentional — user preferences (like "AI before User") are per-user, not per-project/agent/tenant. A user's preference applies regardless of which project or agent they're interacting with. This is NOT a scope isolation violation — it's a different scope dimension (user-level vs component-level). Component tables still use the full `(tenant_id, user_id, agent_id, project_id)` scope tuple. The `user_id` in `reborn_user_preferences` is the same `user_id` from the scope tuple — it's a user-level override, not a bypass of component scope isolation.

**PERF-06 MEDIUM — large match sets deserialized before budget filtering (§3.13):** The `__assemble_prior_knowledge__` function queries the `reborn_component_catalog` with `LIMIT 50` (max 50 matched components per turn — the intent system's `LIMIT 10` per query + disambiguation top-3 + keyword fallback multi-keyword accumulation is bounded). The catalog query returns only the columns needed for assembly (`id`, `class_code`, `name`, `content`/`prior_knowledge_content`, `override_prompt_creation`, `prompt_uid`) — not full component rows. The token-budget filter runs on this bounded set (50 items max), not on an unbounded match set. If the intent system returns more than 50 IDs (unlikely — 10 per query + disambiguation), the excess is dropped by `LIMIT 50`.

**PERF-07 MEDIUM — score order vs class_code order conflict (§3.13):** The intent system returns matched components ordered by `score DESC` (highest score first). The `__assemble_prior_knowledge__` function **re-sorts** by `(class_code asc, prompt_uid asc)` for KV-cache discipline. This is intentional — score determines WHICH components are matched (the intent system's `LIMIT 10` by score), but class_code/prompt_uid determines the ORDER they appear in the prompt (KV-cache discipline). A high-score Tool (class 00) still appears before a lower-score Skill (class 01) in the assembled prior knowledge. The re-sort is O(n log n) on ≤50 items — negligible.

## 7. Open questions / blockers

1. **Phase 1.5 — UNBLOCKED (validator independence + Q19 resolved).** The self-modification boundary is now fully defined: (a) the validator is independent of the orchestrator (§3.5) — it's Rust-side infrastructure that the self-improvement mission cannot patch; (b) all self-improvement mission writes are validation-gated (§3.6) — `memory_write` for code/component changes creates update-candidates that enter Q1; (c) Orchestrator (10) + Scaffold (50) components require an LLM code-audit before Q2 manual validation (§3.5); (d) the formatting design is RESOLVED (§3.14, Q19 — "content is king" + Solution Override). No remaining design blockers.
2. **Confidence factor — RESOLVED.** The confidence factor (`0.5 + 0.5*confidence`) is kept but **only used as a routing signal when the fallback mechanism is triggered** (user "AI before User" switch ON, or intent system finds no match). In normal mode the intent system's score is the primary routing signal. Source-independent. Telemetry columns displayed in WebUI Reliability tab regardless of mode.
3. **`brassclaw_embeddings` — RESOLVED.** Fully removed. The crate, its dependencies, and all embedding-based search paths are deleted. The intent system replaces all runtime similarity/search needs. Install-time dedup uses content-hash + exact-name uniqueness.
4. **Chat memory — RESOLVED.** Flat `Note` MemoryDocs (class 20) with no embedding index, retrieved by the intent system or project-scope lookup like any other component. No separate chat record table.
5. **Class-code sub-ordering — RESOLVED.** Scaffold renumbered from 11 to 50 (the "never renumber" rule is for running systems; this is still in planning). Class 11 is now reserved. Scaffold at 50 sorts last in `(class_code asc, prompt_uid asc)` order, reinforcing its role as the base layer. Classes 12-20 stay in place. No other renumbering.
6. **Q3 revision mechanism — RESOLVED.** Q3 is automated via a **scheduled revision Extension** (class 09, tagged `01:monty`) connected to kohai/sempai. The revision mission runs on schedule when the LLM is not busy, reads rejected components from Q3, uses the kohai/sempai LLM to propose repairs based on `review_feedback`, and re-submits repaired candidates to Q1. The revision mission is itself a validated Extension (goes through the same two-step validation gate). After 3 failed review cycles → Q4.
7. **Q4 retention window length — RESOLVED.** Configurable via a single `q4_retention_days` column in `reborn_monty_vm_settings` (default 30, spec §3.10). Not per-class — one knob is sufficient. The operator can shorten for dev profiles (`local_yolo` → 1 day) or lengthen for hosted production (90 days). The wipe guard reads this value instead of a hardcoded constant.
8. **`reborn_monty_vm_settings` granularity — RESOLVED.** Single row per scope tuple. Per-orchestrator-version granularity is unnecessary: if a future orchestrator version needs different limits, the operator changes the single row when switching `active_orchestrator_id`. A `reborn_monty_vm_settings_overrides` table keyed by `orchestrator_id` can be added later without breaking the base row.
9. **Existing WebUI validation tab review — RESOLVED.** The existing routes (recipe/tool_skill-specific) project cleanly onto Q2 but need generalization to all ~20 class codes. Full route extension detailed in §3.5.2: generalized `PUT /components/{class_code}/{id}/validate|reject|send-to-revision|re-review` routes replace the recipe/tool_skill-specific ones; new Q1 visibility filter, Q3 tab, Q4 wipe route (`DELETE`), LLM-audit guard for class 10/50, `is_queue_status` + `is_valid_transition` extended for all 4 queues, `ValidationQueueItem` response shape extended with `class_code`/`queue_code`/`validator_tag_present`/`consumer_tags[]`/`llm_audit_status`/`llm_audit_findings`, frontend gets 4 queue tabs with badge counts + tag chip greyed-out rendering + Monty VM tab. Old routes kept as aliases during migration, removed in Phase 7.
10. **Intent system query classification heuristic — RESOLVED.** Simple heuristic sufficient for v1, covers all 4 input classes. Class 3 (full sentence): ≥5 words OR ends with `.`/`!`/`?` (the `?` rule added — a 3-word question is class 3). Class 2 (partial): 2–4 words, no terminal punctuation. Class 1 (single word): 1 word, no terminal punctuation. Class 4 (keyword fallback): only created by RetrievalEngine fallback, never by user query classification. NLP sentence boundary detection deferred to a future tunable-logic upgrade — the classifier only affects match order, not correctness, and the learning mechanism compensates over time.
11. **Intent system disambiguation UX — RESOLVED.** Special `disambiguation` chat message type with clickable buttons. Payload: `{type: "disambiguation", candidates: [{component_id, component_class_code, description, class_label}]}`. The WebUI renders each candidate as a button. On click, a structured payload `{disambiguation_choice: component_id}` is sent directly back to `__resolve_intent__` — no intent detection for the reply, no ambiguity. A regular text message is explicitly rejected (friction + requires another intent-detection round).
12. **"Try it with AI" fallback integration — RESOLVED** (§3.12 rule f-fallback): the fallback runs **entirely in Rust**. The orchestrator passes a `fallback: true` flag to `__resolve_intent__`, and the Rust side runs `extract_keywords` + class-4 keyword matching + component fetch + prompt assembly without re-entering Python. The orchestrator only sees the final assembled prior knowledge — no Python→Rust→Python round-trip.
13. **Action step descriptor completeness — RESOLVED** (§3.11): the step types have been expanded to **13** (tool_call/conditional/set_var/loop/return/evaluate/call_skill/try_catch/parallel/call_action/**spawn_subprocess**/**wait**/**emit_event**). The 3 new types cover: `spawn_subprocess` (direct subprocess execution via host runtime script lane), `wait` (pause for duration or polling condition), `emit_event` (structured event emission to the event bus for webhook/extension notifications). Since the step vocabulary is Python tunable logic (default.py interprets the descriptors), more step types can be added later by patching default.py through the validation gate — no Rust changes needed.
14. **Former-doctype component validation — RESOLVED** (§3.5.2 per-class validation config): only Skills (classes 01-03) require the full agentskills.io validation. All other component classes use **lighter validation** (name format + description length + content non-empty + soft token budget as warnings). Each class's validation thresholds are **configurable in the WebUI Settings → Validation tab** via a `reborn_validation_config` table (one row per `(scope, class_code)`). Defaults: Skills 5000 tokens hard error + activation criteria; Tools 5000 hard + require tool_name + param_schema; Extensions 10000 soft; Actions no token budget; former doctypes 10000 soft (Notes 2000); Recipes 10000 soft + trigger validation; Orchestrator/Scaffold 50000 soft + LLM code-audit. The validator is renamed `ComponentValidator` (from `RecipeValidator`) and dispatches by class code.
15. **Recipe class placement — RESOLVED** (§3.7): Recipes get their **own class code 21**. They are solution-class with a distinct schema (trigger + ordered steps + skill references) and have `override_prompt_creation` + `prior_knowledge_content` columns. The `reborn_extensions_unified` class 09 (Misc) retains non-Recipe Misc extensions only. A dedicated `reborn_recipes` table is added (§4). The `RecipeLookup` trait boundary is preserved.
16. **Intent system trigram index — RESOLVED** (§3.12): the `pg_trgm` extension is **installed at brassclaw installation time** — the standalone/embedded Postgres setup script runs `CREATE EXTENSION IF NOT EXISTS pg_trgm` alongside the existing `pgvector` extension. For external Postgres operators, the installation script checks for `pg_trgm` availability and fails with a clear error if not installed (same pattern as `pgvector`). The GIN trigram index is created in the `reborn_intent_inputs` migration.
17. **DB-less fallback file generation — RESOLVED** (§3.4): the fallback file is **created at installation time** when the user selects not to install a DB. It contains **selected compiled-in entries** (not exported from the DB — that is impossible in a DB-less installation). Filesize target: ~256KB (~50,000 tokens, ~5 original DocPlans). Priority for compiled-in inclusion: Tools (00) → Scaffold (50) → Orchestrator (10) → Skills (01-03) → Extensions (04-09) Monty-class first → Recipes (21) → Specs/Lessons (12, 18) → Issues/Notes/Summaries excluded. The `RamSource` loads this file into an in-memory index at startup.
18. **"AI before User" flip switch default + persistence — RESOLVED** (§3.12 rule f-ai): the switch defaults to **OFF**. It is **per-user** (not per-scope), stored in a `reborn_user_preferences` table (new — simple key-value: `(user_id, preference_key, preference_value)`; key = `ai_before_user`, value = `true`/`false`). The switch is **visible in the chat window only** — NOT shown in the Settings UI (it is a user UX preference, not an operator-managed configuration). The `reborn_user_preferences` table is not exposed in the Settings UI. The switch is **hidden/disabled in DB-less mode**.
19. **Formatting design — RESOLVED** (§3.13/§3.14): "content is king" + Solution Override. Components store their content as the exact prior-knowledge text. Rust concatenates `content`/`prior_knowledge_content` fields in `(class_code asc, prompt_uid asc)` order with a per-item header (`### [{class_code}:{CLASS-LABEL}] {name}`). No per-class Rust formatters — one `__assemble_prior_knowledge__` function + a static class-code→label lookup table. Solution-class components (Extensions/Plans/Recipes/Actions) have `prior_knowledge_content TEXT NULL` (overrides `content` for prompt assembly) and `override_prompt_creation BOOLEAN NOT NULL DEFAULT false` (if true, Solution Override path — PKC/content IS the complete prompt, no headers). Actions default to `override_prompt_creation: true` since they skip the LLM call. Non-solution classes do NOT have these columns. The orchestrator honors Solution Override (skips LLM call / uses content as full prompt) but cannot format.

### Interceptor integration questions (Q20–Q30) — ALL RESOLVED

These questions arose from integrating `interceptor2.md` into the v5.5 plan (Phase 5.5, §3.15). All have been resolved with the operator's decisions (documented below).

20. **Sempai provider model selection — RESOLVED.** The operator is free to choose. The WebUI Interceptor Config tab shows a warning if Sempai and Kohai use the same model (UX guardrail, not a hard constraint).

21. **ForensicPacket retention policy — RESOLVED.** A `forensic_packet_retention_days` column is added to `reborn_monty_vm_settings` (default 90). A scheduled cleanup task (runs daily) deletes packets older than the retention window. The operator can set it to 0 to disable pruning. Mirrors the `q4_retention_days` pattern (Q7).

22. **Sempai `settings_adjustments` application path — RESOLVED.** Stored in the `ForensicPacket.sempai_review` JSONB (as part of the outcome object). The WebUI Interceptor Config tab shows a "Recent Sempai Suggestions" list (last 10 packets with `settings_adjustments` non-null). Each suggestion has an "Apply" button that writes the adjustment to `brassclaw_config` (immediate-write, no validation gate — these are config values, not code). The operator can dismiss a suggestion (marks it as reviewed).

23. **Part A rebuild trigger automation — RESOLVED.** The `InterceptorConfigSnapshot` includes a `components_since_rebuild: i64` field (count of `Validated` components with `updated_at > sempai_base_prompt_assembled_at`). The WebUI shows this as a badge. Passive nudge, not an automatic rebuild. Part A rebuild is **manual only** (Q24 — KV-cache discipline).

24. **Sempai timeout and retry behavior — RESOLVED.** Timeout = 120 seconds (Sempai audit is a complex prompt), 0 retries (on error, fall back to `adjusted_messages: None` — Kohai receives the original prompt). The ForensicPacket records the failure in `sempai_review` as `{error: "..."}`. The operator can see failed Sempai audits in the WebUI.

25. **`component_refs` schema versioning — RESOLVED.** Each `component_refs` object includes `schema_version: 1`. Cheap (a few bytes per ref), allows future migration without a table rewrite. The deserializer ignores unknown fields (forward-compatible). **Note (Q25 update):** the "Actions bypass interception" reasoning is structural — Actions are Python-only, the prompt creation process is **disrupted** (orchestrator dispatches Action steps directly, never enters `__assemble_prior_knowledge__` → interceptor → `__llm_complete__`), so the interceptor's hook point is never reached. The interceptor **cannot intercept** because there is no prompt to assemble. See §3.15 "Actions (class 16) bypass interception."

26. **Interceptor mode visibility in the chat UI — RESOLVED.** The `SharedInterceptorMode` (Routing/Rerouting) is visible in the Settings Interceptor Config tab **only** — NOT in the chat window. The Sempai audit is an operator-level concern, not a user-level concern. The chat UI stays clean.

27. **Sempai persona editing and validation — RESOLVED.** The Part B persona is config text (stored in `brassclaw_config`), NOT a component. It is **immediate-write** (no validation gate). The persona is NOT code and does NOT affect the orchestrator's logic — it only shapes the Sempai's review tone. The operator can break the Sempai's review quality with a bad persona, but cannot compromise security (the Sempai's `adjusted_messages` only affect the volatile tail, and `proposed_recipe_updates` + `proposed_intent_examples` go through Q1 validation).

28. **KV-cache pre-warm concurrency — RESOLVED.** The rate limit is per-caller (identified by the `WebUiAuthenticatedCaller` token), so two different operators can each pre-warm once per minute. A second pre-warm while the first is in flight returns `429` with `retry_after_seconds: 60`.

29. **`reassemble_base_prompt()` refresh timing for Part A — RESOLVED.** Part A uses **direct SQL** to individual component tables (Q20 — NOT `reborn_component_catalog`). The "Reassemble" handler always reads fresh data from the live tables — any component validated since the last rebuild is included. No materialized view refresh needed. **Interceptor timing (Q29):** the `on_prompt_assembled` hook is called **after** `__assemble_prior_knowledge__` returns (needs `matched_component_ids`) but **before** the final prompt version is composed (Sempai's `adjusted_messages` can shape the volatile tail before final assembly). The interceptor must NOT be called after the final prompt is composed — it intercepts components, not final prompt bytes. See §3.15 "Interceptor timing."

30. **Sempai `proposed_intent_examples` — RESOLVED.** The `SempaiReviewOutcome` struct includes `proposed_intent_examples: Vec<serde_json::Value>`. These are new `intent_examples` entries the Sempai suggests for existing components. They are **routed through the Q1 validation queue** (same as `proposed_recipe_updates`) — the Sempai cannot directly create intent inputs. Once validated, they are added to the component's `intent_examples` and seeded into `reborn_intent_inputs` by the validator. See §3.15 "SempaiReviewOutcome struct."

31. **Interceptor feature flag — RESOLVED.** Feature flag `interceptor` (default: off). The flag gates the `PgInterceptorStore` wiring, `sempai_swappable` allocation, `SharedInterceptorMode` creation, and the rerouting branch. When off, the interceptor is fully disabled (no `ForensicPacket` saved, no Sempai call). The flag is turned on per-deployment in `brassclaw_config` (key `interceptor.enabled`, default `false`). The migration V044 (ALTER of existing V026 table) runs regardless (the new columns exist but are unused when the flag is off). This allows Phase 5.5 to ship without affecting existing deployments.

**All 31 open questions (Q1–Q31) are RESOLVED.** No remaining design blockers. Phase 0 is ready for sign-off.
